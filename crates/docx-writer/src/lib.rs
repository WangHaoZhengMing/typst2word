//! OpenXML `.docx` Writer (`DocumentIr` -> `.docx` ZIP package).
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
//! - `blocks`: one file per block-level Typst object
//! - `inlines`: one file per inline-level Typst object
//! - `drawingml`: shapes and CeTZ canvases as native DrawingML

mod blocks;
mod drawingml;
mod inlines;
pub(crate) mod math;
mod package;
mod parts;
mod writer;
pub mod xml;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::writer::DocxWriter;
use ir::DocumentIr;

/// Write `DocumentIr` to a `.docx` file on disk.
pub fn write_docx_to_path(ir: &DocumentIr, output_path: &Path) -> Result<(), String> {
    let bytes = write_docx_to_bytes(ir)?;
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

/// Serialize `DocumentIr` into an in-memory `.docx` byte vector.
pub fn write_docx_to_bytes(ir: &DocumentIr) -> Result<Vec<u8>, String> {
    let mut builder = DocxWriter::new(ir);
    builder.build()
}

/// Export options for controlled comparison of page-margin calibration.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocxWriteOptions {
    pub page_margin_calibration: word_construct::page_calibration::PageMarginCalibration,
}

pub fn write_docx_to_bytes_with_options(ir: &DocumentIr, options: DocxWriteOptions) -> Result<Vec<u8>, String> {
    let mut builder = DocxWriter::new(ir);
    builder.options = options;
    builder.build()
}
