//! Self-contained Typst 0.15 `World` implementation supporting both native OS resources
//! (system fonts, on-disk project files, `@preview` package cache/downloads) and pure in-memory
//! WebAssembly (`wasm32-unknown-unknown`) workspaces with dynamic fonts and virtual packages.

use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use parking_lot::RwLock;
use typst::diag::{FileError, FileResult, PackageError};
#[cfg(not(target_arch = "wasm32"))]
use typst::diag::PackageResult;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryBuilder, World};
#[cfg(not(target_arch = "wasm32"))]
use typst_kit::datetime::Time;
#[cfg(not(target_arch = "wasm32"))]
use typst_kit::downloader::SystemDownloader;
use typst_kit::fonts::{self, FontStore};
#[cfg(not(target_arch = "wasm32"))]
use typst_kit::packages::SystemPackages;

use crate::compile::CUSTOM_ROUTINES;

struct EmbeddedFontBundle {
    store: FontStore,
    infos: Vec<FontInfo>,
}

impl EmbeddedFontBundle {
    fn get() -> &'static Self {
        static EMBEDDED: LazyLock<EmbeddedFontBundle> = LazyLock::new(|| {
            let mut store = FontStore::new();
            let mut infos = Vec::new();
            for (slot, info) in fonts::embedded() {
                infos.push(info.clone());
                store.push((slot, info));
            }
            EmbeddedFontBundle { store, infos }
        });
        &EMBEDDED
    }
}

/// Shareable custom font registry combining caller-registered font binaries (`.ttf`/`.otf`/`.ttc`)
/// ahead of Typst's embedded fallback fonts.
///
/// Designed to live across incremental compilations (e.g. inside a Web Worker) so `LazyHash<FontBook>`
/// preserves its memoization hash across keystrokes for `comemo`.
#[derive(Clone)]
pub struct CustomFontRegistry {
    book: LazyHash<FontBook>,
    custom_fonts: Vec<(Font, bool)>,
    system_families: HashSet<String>,
}

impl Default for CustomFontRegistry {
    fn default() -> Self {
        Self::with_embedded()
    }
}

impl CustomFontRegistry {
    /// Construct a registry seeded with Typst's embedded fallback fonts.
    pub fn with_embedded() -> Self {
        let embedded = EmbeddedFontBundle::get();
        let book = FontBook::from_infos(embedded.infos.clone());
        Self {
            book: LazyHash::new(book),
            custom_fonts: Vec::new(),
            system_families: HashSet::new(),
        }
    }

    /// Parse all faces from a `.ttf` / `.otf` / `.ttc` binary buffer and prepend them ahead of
    /// Typst's embedded fallback fonts.
    ///
    /// Pass `is_system_installed = true` when the bytes come from a font installed on the user's
    /// host OS (e.g. via Chrome/Edge `window.queryLocalFonts()`), so downstream Word line-box
    /// calibration knows Microsoft Word will open the exact same binary rather than substituting
    /// `<w:altName>`. Returns the number of font faces added.
    pub fn add_font_bytes(&mut self, bytes: Vec<u8>, is_system_installed: bool) -> usize {
        let buf = Bytes::new(bytes);
        let mut added = 0usize;
        for font in Font::iter(buf) {
            if is_system_installed {
                self.system_families
                    .insert(font.info().family.trim().to_ascii_lowercase());
            }
            self.custom_fonts.push((font, is_system_installed));
            added += 1;
        }
        if added > 0 {
            self.rebuild_book();
        }
        added
    }

    fn rebuild_book(&mut self) {
        let embedded = EmbeddedFontBundle::get();
        let mut infos = Vec::with_capacity(self.custom_fonts.len() + embedded.infos.len());
        for (font, _) in &self.custom_fonts {
            infos.push(font.info().clone());
        }
        infos.extend(embedded.infos.iter().cloned());
        self.book = LazyHash::new(FontBook::from_infos(infos));
    }

    #[inline]
    pub fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    #[inline]
    pub fn font(&self, index: usize) -> Option<Font> {
        if index < self.custom_fonts.len() {
            Some(self.custom_fonts[index].0.clone())
        } else {
            EmbeddedFontBundle::get()
                .store
                .font(index - self.custom_fonts.len())
        }
    }

    #[inline]
    pub fn is_system_font_family(&self, family: &str) -> bool {
        self.system_families
            .contains(&family.trim().to_ascii_lowercase())
    }

    /// Return sorted unique `(family_name, is_system_installed)` pairs across custom and embedded fonts.
    pub fn families(&self) -> Vec<(String, bool)> {
        let mut map: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        for info in &EmbeddedFontBundle::get().infos {
            map.entry(info.family.clone()).or_insert(false);
        }
        for (font, is_sys) in &self.custom_fonts {
            let entry = map.entry(font.info().family.clone()).or_insert(false);
            *entry |= *is_sys;
        }
        map.into_iter().collect()
    }
}

/// Shared global Typst compiler resources (standard library, fonts, package storage).
pub struct GlobalTypstResources {
    /// Standard library wired to our [`CUSTOM_ROUTINES`] (since Typst 0.15 the
    /// routines live in the library rather than in the `Engine`).
    pub library: LazyHash<Library>,
    /// System fonts (on native targets) followed by Typst's embedded fonts.
    pub fonts: FontStore,
    /// Lowercase normalized family names installed in the host OS font catalog
    /// (excluding Typst's embedded fallback fonts).
    pub system_font_families: HashSet<String>,
    /// Local data/cache package directories plus Typst Universe downloads (native targets).
    #[cfg(not(target_arch = "wasm32"))]
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
            #[allow(unused_mut)]
            let mut system_font_families = HashSet::new();

            #[cfg(not(target_arch = "wasm32"))]
            {
                // Record host OS font family names BEFORE appending Typst's embedded fallback bundle
                // (`Libertinus Serif`, `DejaVu Sans Mono`, `New Computer Modern Math`), so downstream
                // metric calibration knows whether Word will load the exact same binary or substitute
                // `<w:altName>` from `word/fontTable.xml`.
                for (source, info) in fonts::system() {
                    system_font_families.insert(info.family.trim().to_ascii_lowercase());
                    fonts.push((source, info));
                }
            }

            fonts.extend(fonts::embedded());

            #[cfg(not(target_arch = "wasm32"))]
            let packages = SystemPackages::new(SystemDownloader::new(concat!(
                "typst2word/",
                env!("CARGO_PKG_VERSION")
            )));

            GlobalTypstResources {
                library: LazyHash::new(library),
                fonts,
                system_font_families,
                #[cfg(not(target_arch = "wasm32"))]
                packages,
            }
        });
        &INSTANCE
    }

    /// Return `true` if `family` is installed in the host OS font catalog.
    #[inline]
    pub fn is_system_font_family(&self, family: &str) -> bool {
        self.system_font_families
            .contains(&family.trim().to_ascii_lowercase())
    }
}

#[derive(Default)]
struct FileSlot {
    source: Option<FileResult<Source>>,
    bytes: Option<FileResult<Bytes>>,
}

/// A self-contained `typst::World` for compiling `.typ` files or in-memory virtual workspaces.
pub struct TypstWorld {
    root: PathBuf,
    main_id: FileId,
    virtual_files: RwLock<HashMap<FileId, Bytes>>,
    package_files: Option<Arc<RwLock<HashMap<FileId, Bytes>>>>,
    missing_packages: RwLock<HashSet<PackageSpec>>,
    slots: RwLock<HashMap<FileId, FileSlot>>,
    #[cfg(not(target_arch = "wasm32"))]
    now: Time,
    custom_today: Option<Datetime>,
    custom_library: Option<LazyHash<Library>>,
    custom_fonts: Option<Arc<CustomFontRegistry>>,
    virtual_only: bool,
}

impl TypstWorld {
    /// Construct a `TypstWorld` for a `.typ` file on disk (native targets).
    #[cfg(not(target_arch = "wasm32"))]
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
            package_files: None,
            missing_packages: RwLock::new(HashSet::new()),
            slots: RwLock::new(HashMap::new()),
            now: Time::system(),
            custom_today: None,
            custom_library: None,
            custom_fonts: None,
            virtual_only: false,
        })
    }

    /// Construct an in-memory `TypstWorld` from a `.typ` source string and optional root directory.
    pub fn from_str(source_text: impl Into<String>, root_dir: Option<PathBuf>) -> Self {
        Self::from_virtual_main("main.typ", source_text, root_dir)
            .expect("`main.typ` is a valid virtual path")
    }

    /// Construct an in-memory `TypstWorld` with an explicit virtual main entry path (e.g. `"main.typ"`).
    pub fn from_virtual_main(
        main_rel_path: &str,
        source_text: impl Into<String>,
        root_dir: Option<PathBuf>,
    ) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        let root = root_dir
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        #[cfg(target_arch = "wasm32")]
        let root = root_dir.unwrap_or_else(|| PathBuf::from("/"));

        let main_id = project_file_id(main_rel_path)?;
        let mut virtual_files = HashMap::new();
        let text: String = source_text.into();
        virtual_files.insert(main_id, Bytes::from_string(text));

        Ok(Self {
            root,
            main_id,
            virtual_files: RwLock::new(virtual_files),
            package_files: None,
            missing_packages: RwLock::new(HashSet::new()),
            slots: RwLock::new(HashMap::new()),
            #[cfg(not(target_arch = "wasm32"))]
            now: Time::system(),
            custom_today: None,
            custom_library: None,
            custom_fonts: None,
            virtual_only: cfg!(target_arch = "wasm32"),
        })
    }

    /// Attach a shared [`CustomFontRegistry`] (used by WASM and custom-font callers).
    pub fn with_font_registry(mut self, registry: Arc<CustomFontRegistry>) -> Self {
        self.custom_fonts = Some(registry);
        self
    }

    /// Attach a shared in-memory `@preview` package file table.
    pub fn with_package_files(
        mut self,
        package_files: Arc<RwLock<HashMap<FileId, Bytes>>>,
    ) -> Self {
        self.package_files = Some(package_files);
        self
    }

    /// Restrict file and package lookups strictly to in-memory virtual stores (disabling host disk
    /// reads and `SystemPackages` network downloads even on native targets).
    pub fn with_virtual_only(mut self, virtual_only: bool) -> Self {
        self.virtual_only = virtual_only;
        self
    }

    /// Override `#datetime.today()` with an explicit calendar date.
    pub fn with_today_ymd(mut self, year: i32, month: u8, day: u8) -> Self {
        self.custom_today = Datetime::from_ymd(year, month, day);
        self
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

    /// Return whether `family` is installed on the host OS (either via native font scan or
    /// caller-registered system fonts in [`CustomFontRegistry`]).
    #[inline]
    pub fn is_system_font_family(&self, family: &str) -> bool {
        if let Some(reg) = &self.custom_fonts {
            return reg.is_system_font_family(family);
        }
        GlobalTypstResources::get().is_system_font_family(family)
    }

    /// Return all unresolved `@preview/name:version` package specs requested during compilation.
    pub fn missing_packages(&self) -> Vec<PackageSpec> {
        let mut list: Vec<PackageSpec> = self.missing_packages.read().iter().cloned().collect();
        list.sort_by(|a, b| {
            (&a.namespace, &a.name, a.version.major, a.version.minor, a.version.patch).cmp(&(
                &b.namespace,
                &b.name,
                b.version.major,
                b.version.minor,
                b.version.patch,
            ))
        });
        list
    }

    /// Root directory of the project.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a `FileId` to an on-disk path (handling both project files and `@preview` packages).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn resolve_system_path(&self, id: FileId) -> FileResult<PathBuf> {
        let base_dir = match id.root() {
            VirtualRoot::Package(spec) => self.resolve_package_dir(spec).map_err(|err| {
                if matches!(err, PackageError::NotFound(_)) {
                    self.missing_packages.write().insert(spec.clone());
                }
                FileError::Package(err)
            })?,
            VirtualRoot::Project => self.root.clone(),
        };

        let vpath = id.vpath();
        if let Ok(resolved) = vpath.realize(&base_dir)
            && resolved.exists()
        {
            return Ok(resolved);
        }

        // Also allow relative parent traversal for local project files if needed.
        let candidate = base_dir.join(vpath.get_without_slash());
        if candidate.exists() {
            return Ok(candidate);
        }

        Err(FileError::NotFound(candidate))
    }

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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
        if let Some(pkg_store) = &self.package_files
            && let Some(bytes) = pkg_store.read().get(&id)
        {
            return Ok(bytes.clone());
        }

        if self.virtual_only {
            return match id.root() {
                VirtualRoot::Package(spec) => {
                    self.missing_packages.write().insert(spec.clone());
                    Err(FileError::Package(PackageError::NotFound(spec.clone())))
                }
                VirtualRoot::Project => Err(FileError::NotFound(PathBuf::from(
                    id.vpath().get_without_slash(),
                ))),
            };
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.resolve_system_path(id)?;
            if path.is_dir() {
                return Err(FileError::IsDirectory);
            }
            let data = fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
            Ok(Bytes::new(data))
        }

        #[cfg(target_arch = "wasm32")]
        {
            unreachable!("virtual_only is always true on wasm32")
        }
    }
}

impl World for TypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        self.custom_library
            .as_ref()
            .unwrap_or(&GlobalTypstResources::get().library)
    }

    fn book(&self) -> &LazyHash<FontBook> {
        if let Some(reg) = &self.custom_fonts {
            reg.book()
        } else {
            GlobalTypstResources::get().fonts.book()
        }
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
        if let Some(reg) = &self.custom_fonts {
            reg.font(index)
        } else {
            GlobalTypstResources::get().fonts.font(index)
        }
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        if let Some(dt) = self.custom_today {
            return Some(dt);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.now.today(offset)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = offset;
            None
        }
    }
}

/// Build a `FileId` for a file in the project root.
pub fn project_file_id(path: &str) -> Result<FileId, String> {
    let normalized = path.trim_start_matches("./").trim_start_matches('/');
    let vpath = VirtualPath::new(normalized)
        .map_err(|e| format!("invalid virtual path '{path}': {e}"))?;
    Ok(RootedPath::new(VirtualRoot::Project, vpath).intern())
}
