//! OpenXML `.docx` Writer (`WordDocument` -> `.docx` ZIP package).
//!
//! Generates a complete, valid Microsoft Word `.docx` archive with:
//! - Native Word DrawingML shapes (`<wps:wsp>`) and CeTZ vector groups (`<wpg:wgp>`)
//! - Native Word Office Math (`<m:oMath>` & `<m:oMathPara>`)
//! - Semantic tables & grids (`<w:gridSpan>`, `<w:vMerge>`, `<w:tcBorders>`, `<w:shd>`, `<w:tblHeader>`)
//! - Headings, titles, outlines (TOC), bibliographies, blockquotes, figures, footnotes, links, refs
//! - Syntax-highlighted code blocks (`RawLine` token colors preserved)
//! - Page setup (dimensions, margins, columns, background fill, header & footer)
//! - Clear visual badges/boxes for unsupported Typst elements (`[不支持的元素: ...]`)
//!
//! Layout:
//! - `writer`: shared state (`DocxWriter`) and helpers
//! - `package`: zip / OPC package assembly
//! - `parts`: fixed XML parts (`styles.xml`, `numbering.xml`, ...)
//! - `blocks`: one file per block-level Word object
//! - `inlines`: one file per inline-level Word object
//! - `drawingml`: shapes and CeTZ canvases as native DrawingML

mod blocks;
mod drawingml;
mod inlines;
pub(crate) mod math;
mod package;
mod parts;
mod writer;
pub mod xml;
pub(crate) mod xml_helpers;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use word_construct::WordDocument;

use crate::writer::DocxWriter;

/// Write a solved [`WordDocument`] to a `.docx` file on disk.
pub fn write_docx_to_path(doc: &WordDocument, output_path: &Path) -> Result<(), String> {
    let bytes = write_docx_to_bytes(doc)?;
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create output directory: {e}"))?;
        }
    }
    let mut file =
        File::create(output_path).map_err(|e| format!("failed to create output file: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("failed to write docx bytes: {e}"))?;
    Ok(())
}

/// Serialize a solved [`WordDocument`] into an in-memory `.docx` byte vector.
pub fn write_docx_to_bytes(doc: &WordDocument) -> Result<Vec<u8>, String> {
    let mut builder = DocxWriter::new(doc);
    builder.build()
}

/// Export options for controlled comparison of page-margin calibration.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocxWriteOptions {
    pub page_margin_calibration: word_construct::page_calibration::PageMarginCalibration,
}

pub fn write_docx_to_bytes_with_options(
    doc: &WordDocument,
    options: DocxWriteOptions,
) -> Result<Vec<u8>, String> {
    let mut builder = DocxWriter::new(doc);
    builder.options = options;
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};
    use word_construct::model::{
        BlockNode, CetzCanvasIr, CetzLabelDrawableIr, EquationIr, HAlignIr, InlineNode, MathNodeIr,
        MathRunIr, NodeId, ShapeIr, ShapeKindIr,
    };

    fn read_zip_entry(bytes: &[u8], name: &str) -> String {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("valid docx zip");
        let mut entry = archive.by_name(name).expect("zip entry exists");
        let mut s = String::new();
        entry.read_to_string(&mut s).expect("valid utf8 xml");
        s
    }

    #[test]
    fn settings_xml_emits_do_not_expand_shift_return_only_inside_compat() {
        for has_cjk in [false, true] {
            let mut doc = WordDocument::default();
            doc.has_cjk = has_cjk;
            let bytes = write_docx_to_bytes(&doc).expect("build docx");
            let settings = read_zip_entry(&bytes, "word/settings.xml");
            assert_eq!(
                settings.matches("<w:doNotExpandShiftReturn/>").count(),
                1,
                "expected single <w:doNotExpandShiftReturn/> (has_cjk={has_cjk})"
            );
            let compat_start = settings.find("<w:compat>").expect("<w:compat> exists");
            let compat_end = settings.find("</w:compat>").expect("</w:compat> exists");
            let tag_pos = settings
                .find("<w:doNotExpandShiftReturn/>")
                .expect("tag exists");
            assert!(
                tag_pos > compat_start && tag_pos < compat_end,
                "<w:doNotExpandShiftReturn/> must live inside <w:compat>"
            );
        }
    }

    #[test]
    fn drawing_ids_are_deterministic_and_textbox_equations_inherit_math_font() {
        let eq = EquationIr {
            id: NodeId::UNSET,
            math_font_family: None,
            is_block: false,
            body: vec![MathNodeIr::Run(MathRunIr {
                text: "x".to_string(),
                ..Default::default()
            })],
            source_text: String::new(),
            numbering_text: None,
            numbering_runs: Vec::new(),
            bookmark: None,
            font_size_pt: Some(11.0),
            layout_metrics: Default::default(),
        };
        let mut doc = WordDocument::default();
        doc.math_font_family = Some("CustomMathFont".to_string());
        doc.blocks = vec![
            BlockNode::ShapeBlock(ShapeIr {
                id: NodeId::UNSET,
                kind: ShapeKindIr::Rect,
                width_pt: 100.0,
                height_pt: 40.0,
                fill: None,
                fill_rule_even_odd: false,
                stroke: None,
                borders: Default::default(),
                corner_radius_pt: None,
                inset: Default::default(),
                rotation_deg: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                body: vec![BlockNode::Paragraph {
                    id: NodeId::UNSET,
                    content: vec![InlineNode::EquationInline(eq.clone())],
                    style: Default::default(),
                    bookmark: None,
                }],
                align: HAlignIr::Left,
                placed_offset_pt: None,
                baseline_shift_pt: None,
            }),
            BlockNode::CetzCanvas(CetzCanvasIr {
                id: NodeId::UNSET,
                width_pt: 120.0,
                height_pt: 80.0,
                background: None,
                paths: Vec::new(),
                labels: vec![CetzLabelDrawableIr {
                    x_pt: 10.0,
                    y_pt: 20.0,
                    width_pt: 20.0,
                    height_pt: 12.0,
                    rotation_deg: 0.0,
                    content: vec![InlineNode::EquationInline(eq)],
                }],
                align: HAlignIr::Left,
            }),
        ];

        let bytes1 = write_docx_to_bytes(&doc).expect("first export");
        let bytes2 = write_docx_to_bytes(&doc).expect("second export");
        let doc1 = read_zip_entry(&bytes1, "word/document.xml");
        let doc2 = read_zip_entry(&bytes2, "word/document.xml");

        assert_eq!(doc1, doc2, "repeated exports must produce identical drawing IDs");
        assert_eq!(
            doc1.matches(r#"w:ascii="CustomMathFont""#).count(),
            2,
            "both textbox and CeTZ inline equations should inherit doc.math_font_family"
        );
    }

    #[test]
    fn section_pages_field_serializes_with_charformat_complex_field() {
        use word_construct::model::{FieldKindIr, ParagraphStyleIr, TextRunIr};

        let mut doc = WordDocument::default();
        doc.page_setup.footer = vec![BlockNode::Paragraph {
            id: NodeId::UNSET,
            content: vec![InlineNode::Field {
                kind: FieldKindIr::SectionPages,
                format: Some("Arabic".into()),
                content: vec![InlineNode::Text(TextRunIr {
                    text: "41".to_string(),
                    font_family: Some("Times New Roman".to_string()),
                    font_size_pt: Some(9.0),
                    ..Default::default()
                })],
            }],
            style: ParagraphStyleIr::default(),
            bookmark: None,
        }];

        let bytes = write_docx_to_bytes(&doc).expect("build docx");
        let footer = read_zip_entry(&bytes, "word/footer1.xml");
        assert!(
            footer.contains("SECTIONPAGES \\* Arabic \\* CHARFORMAT"),
            "expected SECTIONPAGES complex field with CHARFORMAT, got: {footer}"
        );
    }
}
