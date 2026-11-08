//! Self-contained Typst 0.14 `World` implementation with system + embedded fonts,
//! virtual file overlays, and `@preview` package resolution (including CeTZ).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

use chrono::{DateTime, Datelike, FixedOffset, Local, Utc};
use parking_lot::RwLock;
use typst::diag::{FileError, FileResult, PackageError, PackageResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryExt, World};
use typst_kit::download::{Downloader, Progress};
use typst_kit::fonts::{FontSearcher, FontSlot};
use typst_kit::package::PackageStorage;

/// Shared global Typst compiler resources (standard library, font book, package storage).
pub struct GlobalTypstResources {
    pub library: LazyHash<Library>,
    pub book: LazyHash<FontBook>,
    pub fonts: Vec<FontSlot>,
    pub package_storage: PackageStorage,
}

impl GlobalTypstResources {
    pub fn get() -> &'static Self {
        static INSTANCE: LazyLock<GlobalTypstResources> = LazyLock::new(|| {
            let features: Features = [Feature::Html, Feature::A11yExtras].into_iter().collect();
            let library = Library::builder().with_features(features).build();
            let mut searcher = FontSearcher::new();
            searcher.include_system_fonts(true);
            let fonts = searcher.search();
            let package_storage = PackageStorage::new(
                None,
                None,
                Downloader::new(concat!("typst2word/", env!("CARGO_PKG_VERSION"))),
            );
            GlobalTypstResources {
                library: LazyHash::new(library),
                book: LazyHash::new(fonts.book),
                fonts: fonts.fonts,
                package_storage,
            }
        });
        &INSTANCE
    }
}

struct SilentProgress;

impl Progress for SilentProgress {
    fn print_start(&mut self) {}
    fn print_progress(&mut self, _state: &typst_kit::download::DownloadState) {}
    fn print_finish(&mut self, _state: &typst_kit::download::DownloadState) {}
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
    now: OnceLock<DateTime<Utc>>,
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
        let main_id = FileId::new(None, VirtualPath::new(file_name));

        Ok(Self {
            root,
            main_id,
            virtual_files: RwLock::new(HashMap::new()),
            slots: RwLock::new(HashMap::new()),
            now: OnceLock::new(),
        })
    }

    /// Construct an in-memory `TypstWorld` from a `.typ` source string and optional root directory.
    pub fn from_str(source_text: impl Into<String>, root_dir: Option<PathBuf>) -> Self {
        let root = root_dir
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let mut virtual_files = HashMap::new();
        let text: String = source_text.into();
        virtual_files.insert(main_id, Bytes::from_string(text));

        Self {
            root,
            main_id,
            virtual_files: RwLock::new(virtual_files),
            slots: RwLock::new(HashMap::new()),
            now: OnceLock::new(),
        }
    }

    /// Register an additional virtual file (e.g., an imported `.typ`, `.csv`, `.svg`, or `.png` in memory).
    pub fn add_virtual_file(&self, rel_path: impl AsRef<Path>, data: impl Into<Vec<u8>>) -> FileId {
        let id = FileId::new(None, VirtualPath::new(rel_path.as_ref()));
        self.virtual_files
            .write()
            .insert(id, Bytes::new(data.into()));
        self.slots.write().remove(&id);
        id
    }

    /// Root directory of the project.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a `FileId` to an on-disk path (handling both project files and `@preview` packages).
    pub fn resolve_system_path(&self, id: FileId) -> FileResult<PathBuf> {
        let base_dir = if let Some(spec) = id.package() {
            self.resolve_package_dir(spec)?
        } else {
            self.root.clone()
        };

        let vpath = id.vpath();
        if let Some(resolved) = vpath.resolve(&base_dir) {
            if resolved.exists() {
                return Ok(resolved);
            }
        }

        // Also allow relative parent traversal for local project files if needed.
        let rel = vpath.as_rootless_path();
        let candidate = base_dir.join(rel);
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

        // Try Typst's PackageStorage (which may download if network is available).
        let resources = GlobalTypstResources::get();
        if let Ok(dir) = resources
            .package_storage
            .prepare_package(spec, &mut SilentProgress)
        {
            return Ok(dir);
        }

        // Fallback: if exact version is not cached and download failed, check if another
        // compatible version of the same package is cached locally on disk.
        let pkg_parent_sub = format!("{}/{}", spec.namespace, spec.name);
        for root in &candidate_roots {
            let pkg_dir = root.join(&pkg_parent_sub);
            if let Ok(entries) = fs::read_dir(&pkg_dir) {
                let mut versions: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.join("typst.toml").exists())
                    .collect();
                versions.sort();
                if let Some(best) = versions.pop() {
                    return Ok(best);
                }
            }
        }

        Err(PackageError::NotFound(spec.clone()))
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
        &GlobalTypstResources::get().library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &GlobalTypstResources::get().book
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
        GlobalTypstResources::get()
            .fonts
            .get(index)
            .and_then(FontSlot::get)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = self.now.get_or_init(Utc::now);
        let with_offset = match offset {
            None => now.with_timezone(&Local).fixed_offset(),
            Some(hours) => {
                let secs = i32::try_from(hours).ok()?.checked_mul(3600)?;
                now.with_timezone(&FixedOffset::east_opt(secs)?)
            }
        };
        Datetime::from_ymd(
            with_offset.year(),
            with_offset.month().try_into().ok()?,
            with_offset.day().try_into().ok()?,
        )
    }
}
