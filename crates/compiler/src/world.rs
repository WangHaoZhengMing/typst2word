//! Self-contained Typst 0.15 `World` implementation with system + embedded fonts,
//! virtual file overlays, and `@preview` package resolution (including CeTZ).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use parking_lot::RwLock;
use typst::diag::{FileError, FileResult, PackageResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryBuilder, World};
use typst_kit::datetime::Time;
use typst_kit::downloader::SystemDownloader;
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::SystemPackages;

use crate::compile::CUSTOM_ROUTINES;

/// Shared global Typst compiler resources (standard library, fonts, package storage).
pub struct GlobalTypstResources {
    /// Standard library wired to our [`CUSTOM_ROUTINES`] (since Typst 0.15 the
    /// routines live in the library rather than in the `Engine`).
    pub library: LazyHash<Library>,
    /// System fonts followed by Typst's embedded fonts (same order as 0.14's `FontSearcher`).
    pub fonts: FontStore,
    /// Local data/cache package directories plus Typst Universe downloads.
    pub packages: SystemPackages,
}

impl GlobalTypstResources {
    pub fn get() -> &'static Self {
        static INSTANCE: LazyLock<GlobalTypstResources> = LazyLock::new(|| {
            let features: Features = [Feature::Html, Feature::A11yExtras].into_iter().collect();
            let library = LibraryBuilder::from_routines(&CUSTOM_ROUTINES)
                .with_features(features)
                .build();
            let mut fonts = FontStore::new();
            fonts.extend(fonts::system());
            fonts.extend(fonts::embedded());
            let packages = SystemPackages::new(SystemDownloader::new(concat!(
                "typst2word/",
                env!("CARGO_PKG_VERSION")
            )));
            GlobalTypstResources {
                library: LazyHash::new(library),
                fonts,
                packages,
            }
        });
        &INSTANCE
    }
}

#[derive(Default)]
struct FileSlot {
    source: Option<FileResult<Source>>,
    bytes: Option<FileResult<Bytes>>,
}

/// A self-contained `typst::World` for compiling `.typ` files or in-memory strings.
pub struct TypstWorld {
    root: PathBuf,
    main_id: FileId,
    virtual_files: RwLock<HashMap<FileId, Bytes>>,
    slots: RwLock<HashMap<FileId, FileSlot>>,
    now: Time,
    custom_library: Option<LazyHash<Library>>,
}

impl TypstWorld {
    /// Construct a `TypstWorld` for a `.typ` file on disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let canonical = fs::canonicalize(path)
            .map_err(|e| format!("failed to resolve input file '{}': {e}", path.display()))?;
        let root = canonical
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let file_name = canonical
            .file_name()
            .ok_or_else(|| format!("invalid file name '{}'", canonical.display()))?;
        let file_name = file_name.to_str().ok_or_else(|| {
            format!(
                "input file name is not valid UTF-8: '{}'",
                canonical.display()
            )
        })?;
        let main_id = project_file_id(file_name)?;

        Ok(Self {
            root,
            main_id,
            virtual_files: RwLock::new(HashMap::new()),
            slots: RwLock::new(HashMap::new()),
            now: Time::system(),
            custom_library: None,
        })
    }

    /// Construct an in-memory `TypstWorld` from a `.typ` source string and optional root directory.
    pub fn from_str(source_text: impl Into<String>, root_dir: Option<PathBuf>) -> Self {
        let root = root_dir
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let main_id = project_file_id("main.typ").expect("`main.typ` is a valid virtual path");
        let mut virtual_files = HashMap::new();
        let text: String = source_text.into();
        virtual_files.insert(main_id, Bytes::from_string(text));

        Self {
            root,
            main_id,
            virtual_files: RwLock::new(virtual_files),
            slots: RwLock::new(HashMap::new()),
            now: Time::system(),
            custom_library: None,
        }
    }

    /// Configure `sys.inputs` dictionary for template compilation (`--input key=val`).
    pub fn with_inputs(mut self, inputs: typst::foundations::Dict) -> Self {
        let features: Features = [Feature::Html, Feature::A11yExtras].into_iter().collect();
        let library = LibraryBuilder::from_routines(&CUSTOM_ROUTINES)
            .with_features(features)
            .with_inputs(inputs)
            .build();
        self.custom_library = Some(LazyHash::new(library));
        self
    }

    /// Configure `sys.inputs` from string key/value pairs (CLI `--input key=value`).
    ///
    /// An empty iterator keeps the shared global library unchanged.
    pub fn with_string_inputs<K: AsRef<str>, V: AsRef<str>>(
        self,
        inputs: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        let dict: typst::foundations::Dict = inputs
            .into_iter()
            .map(|(k, v)| {
                (
                    typst::foundations::Str::from(k.as_ref()),
                    typst::foundations::Value::Str(typst::foundations::Str::from(v.as_ref())),
                )
            })
            .collect();
        if dict.is_empty() {
            self
        } else {
            self.with_inputs(dict)
        }
    }

    /// Register an additional virtual file (e.g., an imported `.typ`, `.csv`, `.svg`, or `.png` in memory).
    ///
    /// Paths use forward slashes (Typst 0.15 rejects backslashes in virtual paths).
    pub fn add_virtual_file(
        &self,
        rel_path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<FileId, String> {
        let id = project_file_id(rel_path.as_ref())?;
        self.virtual_files
            .write()
            .insert(id, Bytes::new(data.into()));
        self.slots.write().remove(&id);
        Ok(id)
    }

    /// Root directory of the project.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a `FileId` to an on-disk path (handling both project files and `@preview` packages).
    pub fn resolve_system_path(&self, id: FileId) -> FileResult<PathBuf> {
        let base_dir = match id.root() {
            VirtualRoot::Package(spec) => self.resolve_package_dir(spec)?,
            VirtualRoot::Project => self.root.clone(),
        };

        let vpath = id.vpath();
        if let Ok(resolved) = vpath.realize(&base_dir) {
            if resolved.exists() {
                return Ok(resolved);
            }
        }

        // Also allow relative parent traversal for local project files if needed.
        let candidate = base_dir.join(vpath.get_without_slash());
        if candidate.exists() {
            return Ok(candidate);
        }

        Err(FileError::NotFound(candidate))
    }

    fn resolve_package_dir(&self, spec: &PackageSpec) -> PackageResult<PathBuf> {
        let candidate_roots = Self::candidate_package_roots();
        let subdir = format!("{}/{}/{}", spec.namespace, spec.name, spec.version);

        for root in &candidate_roots {
            let dir = root.join(&subdir);
            if dir.join("typst.toml").exists() {
                return Ok(dir);
            }
        }

        // A validated source must resolve the exact version it requests. Never
        // silently substitute a cached version: its API and output may differ.
        let root = GlobalTypstResources::get().packages.obtain(spec)?;
        Ok(root.path().to_path_buf())
    }

    fn candidate_package_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(cache) = dirs::cache_dir() {
            roots.push(cache.join("typst").join("packages"));
        }
        if let Some(data) = dirs::data_dir() {
            let p = data.join("typst").join("packages");
            if !roots.contains(&p) {
                roots.push(p);
            }
        }
        if let Some(local) = dirs::data_local_dir() {
            let p = local.join("typst").join("packages");
            if !roots.contains(&p) {
                roots.push(p);
            }
        }
        roots
    }

    fn read_raw_bytes(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(bytes) = self.virtual_files.read().get(&id) {
            return Ok(bytes.clone());
        }
        let path = self.resolve_system_path(id)?;
        if path.is_dir() {
            return Err(FileError::IsDirectory);
        }
        let data = fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(data))
    }
}

impl World for TypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        self.custom_library
            .as_ref()
            .unwrap_or(&GlobalTypstResources::get().library)
    }

    fn book(&self) -> &LazyHash<FontBook> {
        GlobalTypstResources::get().fonts.book()
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(slot) = self.slots.read().get(&id)
            && let Some(res) = &slot.source
        {
            return res.clone();
        }

        let res = self.read_raw_bytes(id).and_then(|bytes| {
            let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
            Ok(Source::new(id, text.to_owned()))
        });

        self.slots.write().entry(id).or_default().source = Some(res.clone());
        res
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(slot) = self.slots.read().get(&id)
            && let Some(res) = &slot.bytes
        {
            return res.clone();
        }

        let res = self.read_raw_bytes(id);
        self.slots.write().entry(id).or_default().bytes = Some(res.clone());
        res
    }

    fn font(&self, index: usize) -> Option<Font> {
        GlobalTypstResources::get().fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}

/// Build a `FileId` for a file in the project root.
fn project_file_id(path: &str) -> Result<FileId, String> {
    let vpath =
        VirtualPath::new(path).map_err(|e| format!("invalid virtual path '{path}': {e}"))?;
    Ok(RootedPath::new(VirtualRoot::Project, vpath).intern())
}
