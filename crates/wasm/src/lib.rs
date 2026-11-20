//! WebAssembly (`wasm-bindgen`) entry points for running `typst2word` in a browser / Web Worker.
//!
//! Provides a stateful [`WasmWorkspace`] that maintains:
//! - An in-memory virtual project file tree (`.typ`, `.bib`, `.csv`, `.png`, `.jpg`, `.svg`, etc.)
//! - A shared [`CustomFontRegistry`] preserving `LazyHash<FontBook>` across incremental edits
//! - An in-memory `@preview/name:version` package store unpacked from `.tar.gz` archives
//! - Unified single-pass Typst SVG page preview (`typst-svg`) and `.docx` export (`docx-writer`)

#[cfg(target_arch = "wasm32")]
use getrandom_02 as _;
#[cfg(target_arch = "wasm32")]
use getrandom_03 as _;
#[cfg(target_arch = "wasm32")]
use getrandom_04 as _;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Cursor, Read};
use std::sync::Arc;

use compiler::{
    CustomFontRegistry, TypstWorld, compile_world, unpack_package_tar_gz,
};
use docx_writer::write_docx_to_bytes;
use extract::extract_document_ir_with_embed_mode;
use ir::{ConversionReport, FontEmbedMode};
use parking_lot::RwLock;
use serde::Serialize;
use typst::foundations::Bytes;
use typst::syntax::FileId;
use typst::syntax::package::{PackageSpec, PackageVersion};
use wasm_bindgen::prelude::*;
use word_construct::build_word_document;

/// Metadata describing an unresolved `@preview/name:version` package that the Web Worker
/// should fetch (e.g. from `download_url`), cache in IndexedDB, and register via
/// [`WasmWorkspace::register_package_tar_gz`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MissingPackageDto {
    pub spec: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub download_url: String,
}

impl MissingPackageDto {
    fn from_spec(spec: &PackageSpec) -> Self {
        let ver = format!(
            "{}.{}.{}",
            spec.version.major, spec.version.minor, spec.version.patch
        );
        let download_url = format!(
            "https://packages.typst.org/{}/{}-{}.tar.gz",
            spec.namespace, spec.name, ver
        );
        Self {
            spec: format!("@{}/{}:{}", spec.namespace, spec.name, ver),
            namespace: spec.namespace.to_string(),
            name: spec.name.to_string(),
            version: ver,
            download_url,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FontFamilyDto {
    pub family: String,
    pub is_system_installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewResponseDto {
    /// `"ok"` | `"missing_packages"` | `"error"`
    pub status: &'static str,
    pub page_count: usize,
    pub pages_svg: Vec<String>,
    pub compiler_warnings: Vec<String>,
    pub report: Option<ConversionReport>,
    pub document_title: Option<String>,
    pub missing_packages: Vec<MissingPackageDto>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenXmlFileEntryDto {
    pub path: String,
    pub size_bytes: usize,
    pub xml: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenXmlPackageDto {
    pub docx_size_bytes: usize,
    pub files: Vec<OpenXmlFileEntryDto>,
}

/// Result returned by [`WasmWorkspace::compile_and_convert`] containing `.docx` bytes (`Uint8Array`),
/// optional SVG pages, `ConversionReport` JSON, and missing-package / diagnostic metadata.
#[wasm_bindgen]
pub struct WasmConversionOutput {
    status: &'static str,
    docx_bytes: Vec<u8>,
    pages_svg: Vec<String>,
    report_json: String,
    missing_packages: Vec<MissingPackageDto>,
    error: Option<String>,
}

#[wasm_bindgen]
impl WasmConversionOutput {
    /// `"ok"` | `"missing_packages"` | `"error"`
    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.status.to_string()
    }

    /// Generated `.docx` binary payload (`Uint8Array` in JavaScript). Empty when `status != "ok"`.
    #[wasm_bindgen]
    pub fn docx_bytes(&self) -> Vec<u8> {
        self.docx_bytes.clone()
    }

    /// JSON array of rendered SVG strings (`string[]`), one per page.
    #[wasm_bindgen]
    pub fn pages_svg_json(&self) -> String {
        serde_json::to_string(&self.pages_svg).unwrap_or_else(|_| "[]".to_string())
    }

    /// Serialized `ir::ConversionReport` JSON (`pages_count`, `equations_count`, `font_fallback_warnings`, etc.).
    #[wasm_bindgen(getter)]
    pub fn report_json(&self) -> String {
        self.report_json.clone()
    }

    /// JSON array of [`MissingPackageDto`] entries when `status == "missing_packages"`.
    #[wasm_bindgen]
    pub fn missing_packages_json(&self) -> String {
        serde_json::to_string(&self.missing_packages).unwrap_or_else(|_| "[]".to_string())
    }

    /// Compiler or serializer error message when `status != "ok"`.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }
}

/// Stateful Typst-to-Word workspace designed to live inside a browser Dedicated Web Worker.
#[wasm_bindgen]
pub struct WasmWorkspace {
    main_path: String,
    project_files: BTreeMap<String, Vec<u8>>,
    package_files: Arc<RwLock<HashMap<FileId, Bytes>>>,
    loaded_packages: BTreeSet<String>,
    fonts: Arc<CustomFontRegistry>,
    inputs: BTreeMap<String, String>,
    today_ymd: Option<(i32, u8, u8)>,
    embed_fonts_mode: FontEmbedMode,
}

impl Default for WasmWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmWorkspace {
    /// Create a new in-memory workspace seeded with Typst's embedded Latin/Math/Mono fonts.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        console_error_panic_hook::set_once();

        let today_ymd = Self::detect_browser_date();
        Self {
            main_path: "main.typ".to_string(),
            project_files: BTreeMap::new(),
            package_files: Arc::new(RwLock::new(HashMap::new())),
            loaded_packages: BTreeSet::new(),
            fonts: Arc::new(CustomFontRegistry::with_embedded()),
            inputs: BTreeMap::new(),
            today_ymd,
            embed_fonts_mode: FontEmbedMode::default(),
        }
    }

    /// Configure font embedding mode for `.docx` export (`"subset"`, `"full"`, or `"none"`).
    #[wasm_bindgen]
    pub fn set_embed_fonts_mode(&mut self, mode: &str) {
        if let Ok(parsed) = FontEmbedMode::parse_cli(mode) {
            self.embed_fonts_mode = parsed;
        }
    }

    /// Set the active main `.typ` entry file path and its UTF-8 source text.
    #[wasm_bindgen]
    pub fn set_main_source(&mut self, path: &str, source: &str) {
        let clean = normalize_rel_path(path);
        let main = if clean.is_empty() {
            "main.typ".to_string()
        } else {
            clean
        };
        self.project_files
            .insert(main.clone(), source.as_bytes().to_vec());
        self.main_path = main;
    }

    /// Add or overwrite a virtual file in the project workspace (`.typ`, `.bib`, `.png`, `.svg`, etc.).
    #[wasm_bindgen]
    pub fn put_file(&mut self, path: &str, bytes: &[u8]) {
        let clean = normalize_rel_path(path);
        if !clean.is_empty() {
            self.project_files.insert(clean, bytes.to_vec());
        }
    }

    /// Remove a virtual file from the project workspace. Returns `true` if the file existed.
    #[wasm_bindgen]
    pub fn remove_file(&mut self, path: &str) -> bool {
        let clean = normalize_rel_path(path);
        self.project_files.remove(&clean).is_some()
    }

    /// Clear all virtual project files (preserving loaded fonts and `@preview` packages).
    #[wasm_bindgen]
    pub fn clear_files(&mut self) {
        self.project_files.clear();
    }

    /// Return the active main `.typ` entry file path.
    #[wasm_bindgen(getter)]
    pub fn main_path(&self) -> String {
        self.main_path.clone()
    }

    /// Switch the active main `.typ` entry file path (must exist or be set subsequently).
    #[wasm_bindgen]
    pub fn set_main_path(&mut self, path: &str) {
        let clean = normalize_rel_path(path);
        if !clean.is_empty() {
            self.main_path = clean;
        }
    }

    /// Read raw bytes of a virtual project file, if present.
    #[wasm_bindgen]
    pub fn get_file_bytes(&self, path: &str) -> Option<Vec<u8>> {
        let clean = normalize_rel_path(path);
        self.project_files.get(&clean).cloned()
    }

    /// Return a JSON array listing all virtual project file paths.
    #[wasm_bindgen]
    pub fn list_files_json(&self) -> String {
        let keys: Vec<&str> = self.project_files.keys().map(String::as_str).collect();
        serde_json::to_string(&keys).unwrap_or_else(|_| "[]".to_string())
    }

    /// Set `sys.inputs` from a JSON object of string key-value pairs (`{"version": "1.0"}`).
    #[wasm_bindgen]
    pub fn set_inputs_json(&mut self, json: &str) -> Result<(), JsError> {
        if json.trim().is_empty() {
            self.inputs.clear();
            return Ok(());
        }
        let parsed: BTreeMap<String, String> = serde_json::from_str(json)
            .map_err(|e| JsError::new(&format!("invalid sys.inputs JSON object: {e}")))?;
        self.inputs = parsed;
        Ok(())
    }

    /// Override `#datetime.today()` with an explicit `(year, month, day)` calendar date.
    #[wasm_bindgen]
    pub fn set_today_ymd(&mut self, year: i32, month: u8, day: u8) {
        self.today_ymd = Some((year, month, day));
    }

    /// Register all faces from a `.ttf` / `.otf` / `.ttc` binary buffer into the workspace font book.
    ///
    /// Pass `is_system_installed = true` when loaded from the user's host OS (e.g. via
    /// `window.queryLocalFonts()`), or `false` for web/CDN fallback fonts.
    /// Returns the number of font faces added.
    #[wasm_bindgen]
    pub fn add_font_bytes(&mut self, bytes: &[u8], is_system_installed: bool) -> u32 {
        Arc::make_mut(&mut self.fonts).add_font_bytes(bytes.to_vec(), is_system_installed) as u32
    }

    /// Return a JSON array of `{ family, is_system_installed }` entries currently available.
    #[wasm_bindgen]
    pub fn list_font_families_json(&self) -> String {
        let families: Vec<FontFamilyDto> = self
            .fonts
            .families()
            .into_iter()
            .map(|(family, is_system_installed)| FontFamilyDto {
                family,
                is_system_installed,
            })
            .collect();
        serde_json::to_string(&families).unwrap_or_else(|_| "[]".to_string())
    }

    /// Unpack a `.tar.gz` Typst package archive (e.g. `cetz-0.3.4.tar.gz`) into the in-memory
    /// `@namespace/name:version` package store. Returns the number of unpacked files.
    #[wasm_bindgen]
    pub fn register_package_tar_gz(
        &mut self,
        namespace: &str,
        name: &str,
        version: &str,
        tar_gz_bytes: &[u8],
    ) -> Result<u32, JsError> {
        let pkg_ver = parse_package_version(version)
            .map_err(|e| JsError::new(&e))?;
        let spec = PackageSpec {
            namespace: namespace.into(),
            name: name.into(),
            version: pkg_ver,
        };
        let mut guard = self.package_files.write();
        let count = unpack_package_tar_gz(&spec, tar_gz_bytes, &mut guard)
            .map_err(|e| JsError::new(&e))?;
        self.loaded_packages
            .insert(format!("@{namespace}/{name}:{version}"));
        Ok(count as u32)
    }

    /// Check whether `@namespace/name:version` has already been unpacked into this workspace.
    #[wasm_bindgen]
    pub fn has_package(&self, namespace: &str, name: &str, version: &str) -> bool {
        self.loaded_packages
            .contains(&format!("@{namespace}/{name}:{version}"))
    }

    /// Return a JSON array of all registered `@namespace/name:version` package identifiers.
    #[wasm_bindgen]
    pub fn loaded_packages_json(&self) -> String {
        serde_json::to_string(&self.loaded_packages).unwrap_or_else(|_| "[]".to_string())
    }

    /// Compile the current workspace and render live Typst SVG page previews, returning a JSON
    /// [`PreviewResponseDto`] (`status`: `"ok"` | `"missing_packages"` | `"error"`).
    #[wasm_bindgen]
    pub fn compile_preview_json(&mut self) -> String {
        let dto = self.compile_preview_dto();
        serde_json::to_string(&dto).unwrap_or_else(|e| {
            format!(
                r#"{{"status":"error","page_count":0,"pages_svg":[],"compiler_warnings":[],"report":null,"document_title":null,"missing_packages":[],"error":"{e}"}}"#
            )
        })
    }

    /// Compile the current workspace and convert it to `.docx` bytes (`Uint8Array`), optionally
    /// also rendering SVG page previews (`include_svg_preview = true`) from the exact same pass.
    #[wasm_bindgen]
    pub fn compile_and_convert(&mut self, include_svg_preview: bool) -> WasmConversionOutput {
        let world = match self.build_world() {
            Ok(w) => w,
            Err(err) => {
                return WasmConversionOutput {
                    status: "error",
                    docx_bytes: Vec::new(),
                    pages_svg: Vec::new(),
                    report_json: "{}".to_string(),
                    missing_packages: Vec::new(),
                    error: Some(err),
                };
            }
        };

        let compiled = match compile_world(&world) {
            Ok(c) => c,
            Err(err) => {
                comemo::evict(10);
                let missing: Vec<MissingPackageDto> = world
                    .missing_packages()
                    .iter()
                    .map(MissingPackageDto::from_spec)
                    .collect();
                let status = if missing.is_empty() {
                    "error"
                } else {
                    "missing_packages"
                };
                return WasmConversionOutput {
                    status,
                    docx_bytes: Vec::new(),
                    pages_svg: Vec::new(),
                    report_json: "{}".to_string(),
                    missing_packages: missing,
                    error: Some(err),
                };
            }
        };

        let pages_svg = if include_svg_preview {
            let svg_opts = typst_svg::SvgOptions::default();
            compiled
                .document
                .pages()
                .iter()
                .map(|page| typst_svg::svg(page, &svg_opts))
                .collect()
        } else {
            Vec::new()
        };

        let ir = extract_document_ir_with_embed_mode(&compiled, &world, self.embed_fonts_mode);
        let report_json = serde_json::to_string(&ir.report).unwrap_or_else(|_| "{}".to_string());
        let word_doc = build_word_document(&ir);
        comemo::evict(10);

        match write_docx_to_bytes(&word_doc) {
            Ok(docx_bytes) => WasmConversionOutput {
                status: "ok",
                docx_bytes,
                pages_svg,
                report_json,
                missing_packages: Vec::new(),
                error: None,
            },
            Err(err) => WasmConversionOutput {
                status: "error",
                docx_bytes: Vec::new(),
                pages_svg,
                report_json,
                missing_packages: Vec::new(),
                error: Some(err),
            },
        }
    }

    /// Compile the current workspace and return the pretty-printed `DocumentIr` JSON for debugging.
    #[wasm_bindgen]
    pub fn export_ir_json(&mut self) -> Result<String, JsError> {
        let world = self.build_world().map_err(|e| JsError::new(&e))?;
        let compiled = compile_world(&world).map_err(|e| {
            comemo::evict(10);
            JsError::new(&e)
        })?;
        let ir = extract_document_ir_with_embed_mode(&compiled, &world, self.embed_fonts_mode);
        comemo::evict(10);
        serde_json::to_string_pretty(&ir).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Compile the current workspace into an in-memory `.docx` OPC package and return pretty-printed
    /// OpenXML parts (`word/document.xml`, `word/styles.xml`, `word/fontTable.xml`, etc.) as JSON.
    #[wasm_bindgen]
    pub fn export_openxml_json(&mut self) -> Result<String, JsError> {
        let dto = self
            .export_openxml_dto()
            .map_err(|e| JsError::new(&e))?;
        serde_json::to_string(&dto).map_err(|e| JsError::new(&e.to_string()))
    }
}

impl WasmWorkspace {
    /// Rust-native helper returning [`OpenXmlPackageDto`] directly.
    pub fn export_openxml_dto(&mut self) -> Result<OpenXmlPackageDto, String> {
        let world = self.build_world()?;
        let compiled = compile_world(&world).inspect_err(|_| {
            comemo::evict(10);
        })?;
        let ir = extract_document_ir_with_embed_mode(&compiled, &world, self.embed_fonts_mode);
        let word_doc = build_word_document(&ir);
        comemo::evict(10);

        let docx_bytes = write_docx_to_bytes(&word_doc)?;
        let docx_size_bytes = docx_bytes.len();
        let mut archive = zip::ZipArchive::new(Cursor::new(docx_bytes))
            .map_err(|e| format!("failed to inspect generated .docx archive: {e}"))?;

        let mut files = Vec::new();
        for idx in 0..archive.len() {
            let mut entry = archive
                .by_index(idx)
                .map_err(|e| format!("failed to read zip entry #{idx}: {e}"))?;
            let Ok(name) = entry.name() else {
                continue;
            };
            let path = name.into_owned();
            if !(path.ends_with(".xml") || path.ends_with(".rels")) {
                continue;
            }
            let mut raw = String::new();
            if entry.read_to_string(&mut raw).is_ok() {
                let size_bytes = raw.len();
                let xml = pretty_print_xml(&raw);
                files.push(OpenXmlFileEntryDto {
                    path,
                    size_bytes,
                    xml,
                });
            }
        }

        // Prioritize `word/document.xml` first, then other `word/*` parts, then metadata/rels.
        files.sort_by(|a, b| {
            fn rank(p: &str) -> u8 {
                match p {
                    "word/document.xml" => 0,
                    "word/styles.xml" => 1,
                    "word/settings.xml" => 2,
                    "word/fontTable.xml" => 3,
                    "word/numbering.xml" => 4,
                    "word/footnotes.xml" => 5,
                    _ if p.starts_with("word/") => 6,
                    _ => 7,
                }
            }
            (rank(&a.path), &a.path).cmp(&(rank(&b.path), &b.path))
        });

        Ok(OpenXmlPackageDto {
            docx_size_bytes,
            files,
        })
    }

    /// Rust-native helper returning [`PreviewResponseDto`] directly (also used in unit tests).
    pub fn compile_preview_dto(&mut self) -> PreviewResponseDto {
        let world = match self.build_world() {
            Ok(w) => w,
            Err(err) => {
                return PreviewResponseDto {
                    status: "error",
                    page_count: 0,
                    pages_svg: Vec::new(),
                    compiler_warnings: Vec::new(),
                    report: None,
                    document_title: None,
                    missing_packages: Vec::new(),
                    error: Some(err),
                };
            }
        };

        match compile_world(&world) {
            Ok(compiled) => {
                let svg_opts = typst_svg::SvgOptions::default();
                let pages_svg: Vec<String> = compiled
                    .document
                    .pages()
                    .iter()
                    .map(|page| typst_svg::svg(page, &svg_opts))
                    .collect();
                let ir = extract_document_ir_with_embed_mode(&compiled, &world, self.embed_fonts_mode);
                let mut seen = BTreeSet::new();
                let mut warnings = Vec::new();
                for diag in &compiled.warnings {
                    let msg = diag.message.to_string();
                    if seen.insert(msg.clone()) {
                        warnings.push(msg);
                    }
                }
                let document_title = ir.metadata.title.clone();
                let report = Some(ir.report);
                comemo::evict(10);
                PreviewResponseDto {
                    status: "ok",
                    page_count: pages_svg.len(),
                    pages_svg,
                    compiler_warnings: warnings,
                    report,
                    document_title,
                    missing_packages: Vec::new(),
                    error: None,
                }
            }
            Err(err) => {
                comemo::evict(10);
                let missing: Vec<MissingPackageDto> = world
                    .missing_packages()
                    .iter()
                    .map(MissingPackageDto::from_spec)
                    .collect();
                let status = if missing.is_empty() {
                    "error"
                } else {
                    "missing_packages"
                };
                PreviewResponseDto {
                    status,
                    page_count: 0,
                    pages_svg: Vec::new(),
                    compiler_warnings: Vec::new(),
                    report: None,
                    document_title: None,
                    missing_packages: missing,
                    error: Some(err),
                }
            }
        }
    }

    fn build_world(&self) -> Result<TypstWorld, String> {
        let main_bytes = self
            .project_files
            .get(&self.main_path)
            .ok_or_else(|| format!("main entry file '{}' is not set in workspace", self.main_path))?;
        let main_source = std::str::from_utf8(main_bytes)
            .map_err(|_| format!("main entry file '{}' is not valid UTF-8", self.main_path))?
            .to_owned();

        let mut world = TypstWorld::from_virtual_main(&self.main_path, main_source, None)?
            .with_virtual_only(true)
            .with_font_registry(Arc::clone(&self.fonts))
            .with_package_files(Arc::clone(&self.package_files))
            .with_string_inputs(self.inputs.iter().map(|(k, v)| (k.as_str(), v.as_str())));

        if let Some((y, m, d)) = self.today_ymd {
            world = world.with_today_ymd(y, m, d);
        }

        for (path, bytes) in &self.project_files {
            if path != &self.main_path {
                world.add_virtual_file(path, bytes.clone())?;
            }
        }

        Ok(world)
    }

    #[cfg(target_arch = "wasm32")]
    fn detect_browser_date() -> Option<(i32, u8, u8)> {
        let date = js_sys::Date::new_0();
        let year = date.get_full_year() as i32;
        let month = (date.get_month() + 1) as u8;
        let day = date.get_date() as u8;
        Some((year, month, day))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn detect_browser_date() -> Option<(i32, u8, u8)> {
        None
    }
}

/// Stateless one-call Typst source string -> `.docx` `Uint8Array` converter for quick JS usage.
#[wasm_bindgen]
pub fn convert_typst_to_docx(source: &str) -> Result<Vec<u8>, JsError> {
    let mut ws = WasmWorkspace::new();
    ws.set_main_source("main.typ", source);
    let out = ws.compile_and_convert(false);
    if out.status == "ok" {
        Ok(out.docx_bytes)
    } else {
        Err(JsError::new(
            out.error
                .as_deref()
                .unwrap_or("Typst-to-Word conversion failed"),
        ))
    }
}

fn normalize_rel_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn parse_package_version(version: &str) -> Result<PackageVersion, String> {
    let v = version.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("invalid package version '{version}'"))?;
    let minor: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("invalid package version '{version}'"))?;
    let patch: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("invalid package version '{version}'"))?;
    if parts.next().is_some() {
        return Err(format!("invalid package version '{version}'"));
    }
    Ok(PackageVersion {
        major,
        minor,
        patch,
    })
}

/// Non-destructive structural XML pretty-printer that only inserts newlines and 2-space
/// indentation across `><` tag boundaries, preserving inline `<w:t>` and `<m:t>` text verbatim.
fn pretty_print_xml(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + raw.len() / 5);
    let mut depth: usize = 0;
    let mut idx = 0usize;
    let bytes = raw.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'<' {
            // Find matching `>` (respecting attribute quotes).
            let mut end = idx + 1;
            let mut in_quote: Option<u8> = None;
            while end < bytes.len() {
                let b = bytes[end];
                if let Some(q) = in_quote {
                    if b == q {
                        in_quote = None;
                    }
                } else if b == b'"' || b == b'\'' {
                    in_quote = Some(b);
                } else if b == b'>' {
                    end += 1;
                    break;
                }
                end += 1;
            }

            let tag = &raw[idx..end];
            let is_closing = tag.starts_with("</");
            let is_decl_or_comment = tag.starts_with("<?") || tag.starts_with("<!");
            let is_self_closing = tag.ends_with("/>");

            if is_closing {
                depth = depth.saturating_sub(1);
            }

            // If we just emitted a newline at the end of the previous tag, indent now.
            if out.ends_with('\n') {
                for _ in 0..depth {
                    out.push_str("  ");
                }
            }

            out.push_str(tag);

            if !is_closing && !is_decl_or_comment && !is_self_closing {
                // Check if this opening tag is immediately closed (`<w:t>text</w:t>`):
                // if the next `<` is `</`, keep `<tag>text</tag>` on a single line!
                if let Some(next_lt_rel) = raw[end..].find('<') {
                    let next_lt = end + next_lt_rel;
                    if next_lt > end && raw[next_lt..].starts_with("</") {
                        // Inline text element: copy text + closing tag onto the same line.
                        if let Some(close_gt_rel) = raw[next_lt..].find('>') {
                            let close_end = next_lt + close_gt_rel + 1;
                            out.push_str(&raw[end..close_end]);
                            idx = close_end;
                            if idx < bytes.len() && bytes[idx] == b'<' {
                                out.push('\n');
                            }
                            continue;
                        }
                    }
                }
                depth += 1;
            }

            idx = end;
            if idx < bytes.len() && bytes[idx] == b'<' {
                out.push('\n');
            }
        } else {
            let next_lt = raw[idx..]
                .find('<')
                .map(|rel| idx + rel)
                .unwrap_or(bytes.len());
            out.push_str(&raw[idx..next_lt]);
            idx = next_lt;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::{Cursor, Read, Write};

    fn build_tar_entry(path: &str, data: &[u8]) -> Vec<u8> {
        let mut blk = vec![0u8; 512];
        blk[..path.len()].copy_from_slice(path.as_bytes());
        blk[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
        blk[156] = b'0';
        blk[257..263].copy_from_slice(b"ustar\0");
        blk[148..156].fill(b' ');
        let sum: u32 = blk.iter().map(|&b| b as u32).sum();
        blk[148..156].copy_from_slice(format!("{:06o}\0 ", sum).as_bytes());
        blk.extend_from_slice(data);
        let rem = data.len() % 512;
        if rem != 0 {
            blk.resize(blk.len() + (512 - rem), 0);
        }
        blk
    }

    #[test]
    fn wasm_workspace_compiles_multifile_preview_and_docx_with_virtual_package() {
        let mut ws = WasmWorkspace::new();
        ws.set_today_ymd(2026, 11, 17);
        ws.set_inputs_json(r#"{"author": "WASM Studio"}"#).unwrap();
        ws.put_file(
            "chapters/math.typ",
            br#"#let theorem_eq = $ integral_0^oo e^(-x^2) dif x = sqrt(pi) / 2 $"#,
        );
        ws.set_main_source(
            "main.typ",
            r#"
            #import "@preview/mockpkg:0.1.0": badge
            #import "chapters/math.typ": theorem_eq

            = #sys.inputs.at("author")
            #badge[Ready] Inline $E = m c^2$:
            #theorem_eq
            "#,
        );

        // 1. Before registering `@preview/mockpkg:0.1.0`, `missing_packages` reports its download URL
        //    (when not installed on disk either).
        let preview_missing = ws.compile_preview_dto();
        assert_eq!(preview_missing.status, "missing_packages");
        assert_eq!(preview_missing.missing_packages.len(), 1);
        assert_eq!(
            preview_missing.missing_packages[0].download_url,
            "https://packages.typst.org/preview/mockpkg-0.1.0.tar.gz"
        );

        // 2. Register an in-memory `.tar.gz` for `@preview/mockpkg:0.1.0` and re-run!
        let mut tar = Vec::new();
        tar.extend_from_slice(&build_tar_entry(
            "typst.toml",
            b"[package]\nname = \"mockpkg\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
        ));
        tar.extend_from_slice(&build_tar_entry(
            "lib.typ",
            b"#let badge(body) = strong(body)\n",
        ));
        tar.resize(tar.len() + 1024, 0);
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).unwrap();
        let tar_gz = gz.finish().unwrap();

        let unpacked = ws
            .register_package_tar_gz("preview", "mockpkg", "0.1.0", &tar_gz)
            .unwrap();
        assert_eq!(unpacked, 2);
        assert!(ws.has_package("preview", "mockpkg", "0.1.0"));

        // 3. Now preview telemetry, `.docx` export, and OpenXML inspection all succeed from memory!
        let preview_ok = ws.compile_preview_dto();
        assert_eq!(preview_ok.status, "ok");
        assert_eq!(preview_ok.report.as_ref().unwrap().equations_count, 2);

        let out = ws.compile_and_convert(true);
        assert_eq!(out.status(), "ok", "unexpected error: {:?}", out.error());
        assert!(!out.docx_bytes().is_empty());

        let openxml = ws.export_openxml_dto().unwrap();
        assert!(openxml.docx_size_bytes > 0);
        assert_eq!(openxml.files[0].path, "word/document.xml");
        assert!(openxml.files[0].xml.contains("<m:oMathPara>"));

        let svgs: Vec<String> = serde_json::from_str(&out.pages_svg_json()).unwrap();
        assert_eq!(svgs.len(), 1);
        assert!(svgs[0].contains("<svg"));

        // Verify valid OPC `.docx` archive containing `word/document.xml` with `<m:oMathPara>` and `"WASM Studio"`.
        let mut archive = zip::ZipArchive::new(Cursor::new(out.docx_bytes())).unwrap();
        let mut doc_xml = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc_xml)
            .unwrap();
        assert!(doc_xml.contains("WASM Studio"));
        assert!(doc_xml.contains("<m:oMathPara>"));
    }

    #[test]
    fn wasm_stateless_convert_and_font_listing_work() {
        let ws = WasmWorkspace::new();
        let families: Vec<serde_json::Value> =
            serde_json::from_str(&ws.list_font_families_json()).unwrap();
        assert!(
            families
                .iter()
                .any(|f| f["family"] == "Libertinus Serif" && f["is_system_installed"] == false),
            "embedded Libertinus Serif should be listed as non-system font"
        );

        let bytes = convert_typst_to_docx("= Quick Test\nHello $a + b = c$!").unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(archive.by_name("word/document.xml").is_ok());
        assert!(archive.by_name("word/styles.xml").is_ok());
        assert!(archive.by_name("word/settings.xml").is_ok());
    }
}
