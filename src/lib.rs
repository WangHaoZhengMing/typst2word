//! `typst2word` — Semantic + Visual Typst (`.typ`) to Microsoft Word (`.docx`) converter.
//!
//! Architecture:
//! 1. [`compiler`]: Self-contained Typst 0.15.1 compiler & query engine (`TypstWorld`,
//!    `CUSTOM_ROUTINES`, `CompiledTypstDocument`) that preserves pre-layout `Content`
//!    semantics, introspection `Introspector` queries, and laid-out `PagedDocument`
//!    `Frame` tags (`Tag::Start` / `Tag::End`) including custom realization locations
//!    for native shapes, transforms, and HTML elements.
//! 2. [`ir`]: Unified Semantic + Visual Intermediate Representation (`DocumentIr`).
//! 3. [`extract`]: Combines introspection queries, `CellGrid` table metadata, and
//!    laid-out `Frame` trees to extract headings, paragraphs, inline styles, lists,
//!    enums, terms, tables/grids, quotes, syntax-highlighted code blocks, figures,
//!    outlines, bibliographies, footnotes, hyperlinks, cross-references, native shapes,
//!    OMML math equations, CeTZ vector drawings, and unsupported element placeholders.
//! 4. [`math`]: Converts Typst Math `Content` AST into native Word Office Math (`<m:oMath>`).
//! 5. [`docx`]: Packages `DocumentIr` into a standards-compliant `.docx` archive, including
//!    native DrawingML shapes and CeTZ canvases (`<wps:wsp>` and `<wpg:wgp>`).
//!
//! Each Typst object family has a mirrored file on both sides of the IR, e.g.
//! `extract/elements/table.rs` -> `ir/table.rs` -> `docx/blocks/table.rs`.
//!
//! Shared helpers: [`xml`] (escaping), [`units`] (pt -> dxa / EMU), [`color`] (paint -> hex).

pub mod color;
pub mod compiler;
pub mod docx;
pub mod extract;
pub mod ir;
pub mod units;
pub mod xml;

use std::path::Path;

use compiler::{TypstWorld, compile_world};
use docx::{write_docx_to_bytes, write_docx_to_path};
use extract::extract_document_ir;
use ir::DocumentIr;

/// Compile a Typst `.typ` file and extract its `DocumentIr`.
pub fn compile_file_to_ir(input_path: &Path) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_file(input_path)?;
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir(&compiled, &world))
}

/// Compile an in-memory Typst source string and extract its `DocumentIr`.
pub fn compile_str_to_ir(source: &str, root_dir: Option<&Path>) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_str(source, root_dir.map(|p| p.to_path_buf()));
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir(&compiled, &world))
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
