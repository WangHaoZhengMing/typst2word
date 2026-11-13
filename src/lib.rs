//! `typst2word` — Semantic + Visual Typst (`.typ`) to Microsoft Word (`.docx`) converter.
//!
//! Workspace Crate Pipeline:
//! 1. [`compiler`]: Self-contained Typst 0.15.1 compiler & query engine (`TypstWorld`,
//!    `CUSTOM_ROUTINES`, `CompiledTypstDocument`) preserving both pre-layout `Content`
//!    semantics (`Introspector`) and laid-out `PagedDocument` `Frame` geometry (`Tag::Start` / `Tag::End`).
//! 2. [`extract`]: Pure Typst semantic & PDF physical layout extractor producing structurally
//!    normalized [`DocumentIr`] (zero Word-specific logic).
//! 3. [`ir`]: Exhaustive enum-driven Semantic + PDF Metrics Intermediate Representation (`DocumentIr`,
//!    `ir::pdf`, `ir::normalize`, `ir::units`, `ir::anchor`).
//! 4. [`word_construct`]: Word layout & compensation engine (`build_word_document(&DocumentIr) -> WordDocument`):
//!    - `word_construct::pipeline`: Word recognition & spacing solvers (`FontRecognition`,
//!      `LanguageAndScriptRecognition`, `InterBlockFlowAndSpacing`).
//!    - `word_construct::objects`: Per-object Word Construct builders (`paragraph`, `heading`, `list`,
//!      `table`, `equation`, `figure`, `code`, `text_run`, `shape`, `cetz`).
//! 5. [`docx_writer`]: Pure OpenXML & ZIP package writer (`DocxWriter` consuming `&WordDocument`).

pub use compiler;
pub use docx_writer;
pub use extract;
pub use ir;
pub use word_construct;

use std::path::Path;

use compiler::{TypstWorld, compile_world};
use docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options, write_docx_to_path};
use extract::extract_document_ir;
use ir::DocumentIr;
use word_construct::build_word_document;

/// Compile a Typst `.typ` file and extract its pure `DocumentIr` (never mutated by Word passes).
pub fn compile_file_to_ir(input_path: &Path) -> Result<DocumentIr, String> {
    compile_file_to_ir_with_inputs(input_path, std::iter::empty::<(&str, &str)>())
}

/// Compile a Typst `.typ` file with `sys.inputs` key-value pairs and extract its pure `DocumentIr`.
pub fn compile_file_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_file(input_path)?.with_string_inputs(inputs);
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir(&compiled, &world))
}

/// Compile an in-memory Typst source string and extract its pure `DocumentIr` (never mutated by Word passes).
pub fn compile_str_to_ir(source: &str, root_dir: Option<&Path>) -> Result<DocumentIr, String> {
    compile_str_to_ir_with_inputs(source, root_dir, std::iter::empty::<(&str, &str)>())
}

/// Compile an in-memory Typst source string with `sys.inputs` key-value pairs and extract its pure `DocumentIr`.
pub fn compile_str_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    source: &str,
    root_dir: Option<&Path>,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_str(source, root_dir.map(|p| p.to_path_buf())).with_string_inputs(inputs);
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir(&compiled, &world))
}

/// Convert a Typst `.typ` file directly to a `.docx` file on disk, returning the `DocumentIr`.
pub fn convert_typst_file_to_docx(
    input_path: &Path,
    output_path: &Path,
) -> Result<DocumentIr, String> {
    convert_typst_file_to_docx_with_inputs(
        input_path,
        output_path,
        std::iter::empty::<(&str, &str)>(),
    )
}

/// Convert a Typst `.typ` file with `sys.inputs` directly to a `.docx` file on disk.
pub fn convert_typst_file_to_docx_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    let ir = compile_file_to_ir_with_inputs(input_path, inputs)?;
    let word_doc = build_word_document(&ir);
    write_docx_to_path(&word_doc, output_path)?;
    Ok(ir)
}

/// Convert an in-memory Typst source string to `.docx` bytes, returning `(docx_bytes, DocumentIr)`.
pub fn convert_typst_str_to_docx_bytes(
    source: &str,
    root_dir: Option<&Path>,
) -> Result<(Vec<u8>, DocumentIr), String> {
    convert_typst_str_to_docx_bytes_with_options(source, root_dir, DocxWriteOptions::default())
}

/// Convert an in-memory Typst source string to `.docx` bytes with explicit [`DocxWriteOptions`].
pub fn convert_typst_str_to_docx_bytes_with_options(
    source: &str,
    root_dir: Option<&Path>,
    options: DocxWriteOptions,
) -> Result<(Vec<u8>, DocumentIr), String> {
    let ir = compile_str_to_ir(source, root_dir)?;
    let word_doc = build_word_document(&ir);
    let bytes = write_docx_to_bytes_with_options(&word_doc, options)?;
    Ok((bytes, ir))
}
