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
#[cfg(not(target_arch = "wasm32"))]
use docx_writer::write_docx_to_path;
use docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options};
use extract::{extract_document_ir, extract_document_ir_with_embed_mode};
use ir::{DocumentIr, FontEmbedMode};
use word_construct::build_word_document;

/// Compile a Typst `.typ` file and extract its pure `DocumentIr` (never mutated by Word passes).
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_file_to_ir(input_path: &Path) -> Result<DocumentIr, String> {
    compile_file_to_ir_with_inputs(input_path, std::iter::empty::<(&str, &str)>())
}

/// Compile a Typst `.typ` file with `sys.inputs` key-value pairs and extract its pure `DocumentIr`.
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_file_to_ir_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    compile_file_to_ir_with_options(input_path, inputs, FontEmbedMode::default())
}

/// Compile a Typst `.typ` file with `sys.inputs` and explicit [`FontEmbedMode`].
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_file_to_ir_with_options<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
    embed_mode: FontEmbedMode,
) -> Result<DocumentIr, String> {
    let world = TypstWorld::from_file(input_path)?.with_string_inputs(inputs);
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir_with_embed_mode(&compiled, &world, embed_mode))
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
    let world =
        TypstWorld::from_str(source, root_dir.map(|p| p.to_path_buf())).with_string_inputs(inputs);
    let compiled = compile_world(&world)?;
    Ok(extract_document_ir(&compiled, &world))
}

/// Convert a Typst `.typ` file directly to a `.docx` file on disk, returning the `DocumentIr`.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_typst_file_to_docx_with_inputs<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
) -> Result<DocumentIr, String> {
    convert_typst_file_to_docx_with_options(
        input_path,
        output_path,
        inputs,
        FontEmbedMode::default(),
    )
}

/// Convert a Typst `.typ` file with `sys.inputs` and explicit [`FontEmbedMode`] to a `.docx` file.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_typst_file_to_docx_with_options<K: AsRef<str>, V: AsRef<str>>(
    input_path: &Path,
    output_path: &Path,
    inputs: impl IntoIterator<Item = (K, V)>,
    embed_mode: FontEmbedMode,
) -> Result<DocumentIr, String> {
    let ir = compile_file_to_ir_with_options(input_path, inputs, embed_mode)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_auto_table_preserves_alignment_bold_cells_and_single_line_width() {
        let source = r#"
#align(center)[
  #table(
    columns: (auto, auto),
    align: (right + horizon, center + horizon),
    stroke: none,
    inset: 5pt,
    [#text(weight: "bold")[姓名：]], [王浩然], table.hline(start: 1),
    [#text(weight: "bold")[班级：]], [计算机学科学与技术2301], table.hline(start: 1),
  )
]
"#;
        let world = TypstWorld::from_str(source, None);
        let compiled = compile_world(&world).expect("typst compile should succeed");
        let ir = extract_document_ir_with_embed_mode(&compiled, &world, FontEmbedMode::None);

        let table = ir
            .blocks
            .iter()
            .find_map(|b| match b {
                ir::BlockNode::Table(t) => Some(t),
                _ => None,
            })
            .expect("expected extracted table");
        assert_eq!(table.align, ir::HAlignIr::Center);
        assert_eq!(table.cols.len(), 2);
        assert!(
            table.cols.iter().all(|c| c.is_auto()),
            "columns: (auto, auto) must preserve TableColTrackIr::Auto, got {:?}",
            table.cols
        );
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].cells[0].halign, ir::HAlignIr::Right);
        assert_eq!(table.rows[0].cells[1].halign, ir::HAlignIr::Center);

        let first_cell_bold = match &table.rows[0].cells[0].blocks[0] {
            ir::BlockNode::Paragraph { content, .. } => content.iter().all(|inl| match inl {
                ir::InlineNode::Text(r) => r.bold,
                _ => true,
            }),
            _ => false,
        };
        assert!(
            first_cell_bold,
            "inline-fragment #text(weight: \"bold\")[...] inside table cell must preserve bold"
        );

        let word_doc = build_word_document(&ir);
        let solved_table = word_doc
            .blocks
            .iter()
            .find_map(|b| match b {
                ir::BlockNode::Table(t) => Some(t),
                _ => None,
            })
            .expect("expected solved table");
        assert!(
            solved_table.col_width_pt(0) >= 45.5,
            "col 0 must be wide enough for bold 3-char CJK+colon in Word, got {}",
            solved_table.col_width_pt(0)
        );
        assert_eq!(
            word_construct::table::TableConstruct::solve(solved_table).layout_mode,
            word_construct::table::TableLayoutMode::Autofit
        );
        let docx_bytes = docx_writer::write_docx_to_bytes(&word_doc)
            .expect("docx serialization should succeed");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(docx_bytes))
            .expect("valid docx zip archive");
        let mut doc_xml = String::new();
        std::io::Read::read_to_string(
            &mut zip.by_name("word/document.xml").expect("word/document.xml"),
            &mut doc_xml,
        )
        .expect("read word/document.xml");
        assert!(
            doc_xml.contains(r#"<w:tblPr><w:jc w:val="center"/>"#),
            "centered table must emit <w:jc w:val=\"center\"/>"
        );
        assert!(
            doc_xml.contains(r#"<w:tblLayout w:type="autofit"/>"#),
            "all-auto semantic table must emit <w:tblLayout w:type=\"autofit\"/>"
        );
    }
}
