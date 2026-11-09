//! `typst2word` — Semantic + Visual Typst (`.typ`) to Microsoft Word (`.docx`) converter.
//!
//! Architecture:
//! 1. [`compiler`]: Self-contained Typst 0.15.1 compiler & query engine (`TypstWorld`,
//!    `CUSTOM_ROUTINES`, `CompiledTypstDocument`) that preserves pre-layout `Content`
//!    semantics, introspection `Introspector` queries, and laid-out `PagedDocument`
//!    `Frame` tags (`Tag::Start` / `Tag::End`).
//! 2. [`ir`]: Exhaustive enum-driven Semantic + PDF Metrics Intermediate Representation (`DocumentIr`).
//! 3. [`extract`]: Pure Typst semantic & PDF physical layout extractor (zero Word-specific logic).
//! 4. [`passes`]: 5-stage sequential pre-processing pipeline (`FontRecognition`, `InlineCoalescing`,
//!    `LanguageAndScriptRecognition`, `ContainerAndPageBreakMerge`, `InterBlockFlowAndSpacing`).
//! 5. [`construct`]: Per-object Word Construct builders (`paragraph`, `heading`, `list`, `table`,
//!    `equation`, `figure`, `code`, `text_run`, `shape`, `cetz`) encapsulating all Word layout math.
//! 6. [`docx_writer`]: Pure OpenXML & ZIP package writer (`DocxWriter`).
//!
//! Shared helpers: [`xml`] (escaping), [`units`] (pt -> dxa / EMU), [`color`] (paint -> hex).

pub mod color;
pub mod compiler;
pub mod construct;
pub mod docx_writer;
pub mod extract;
pub mod ir;
pub mod passes;
pub mod units;
pub mod xml;

use std::path::Path;

use compiler::{TypstWorld, compile_world};
use docx_writer::{write_docx_to_bytes, write_docx_to_path};
use extract::extract_document_ir;
use ir::DocumentIr;
use passes::run_pipeline_passes;

/// Compile a Typst `.typ` file and extract its pipeline-processed `DocumentIr`.
pub fn compile_file_to_ir(input_path: &Path) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_file(input_path)?;
    let compiled = compile_world(&world)?;
    let mut ir = extract_document_ir(&compiled, &world);
    run_pipeline_passes(&mut ir);
    Ok(ir)
}

/// Compile an in-memory Typst source string and extract its pipeline-processed `DocumentIr`.
pub fn compile_str_to_ir(source: &str, root_dir: Option<&Path>) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_str(source, root_dir.map(|p| p.to_path_buf()));
    let compiled = compile_world(&world)?;
    let mut ir = extract_document_ir(&compiled, &world);
    run_pipeline_passes(&mut ir);
    Ok(ir)
}

/// Convert a Typst `.typ` file directly to a `.docx` file on disk, returning the `DocumentIr`.
pub fn convert_typst_file_to_docx(
    input_path: &Path,
    output_path: &Path,
) -> Result<DocumentIr, String> {
    let ir = compile_file_to_ir(input_path)?;
    write_docx_to_path(&ir, output_path)?;
    Ok(ir)
}

/// Convert an in-memory Typst source string to `.docx` bytes, returning `(docx_bytes, DocumentIr)`.
pub fn convert_typst_str_to_docx_bytes(
    source: &str,
    root_dir: Option<&Path>,
) -> Result<(Vec<u8>, DocumentIr), String> {
    let ir = compile_str_to_ir(source, root_dir)?;
    let bytes = write_docx_to_bytes(&ir)?;
    Ok((bytes, ir))
}
