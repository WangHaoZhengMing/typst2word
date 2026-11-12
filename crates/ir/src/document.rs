//! Document-level metadata, page setup and the conversion report.

use serde::{Deserialize, Serialize};

use super::{BlockNode, ColorHex, UnsupportedIr};

/// Document-level metadata extracted from `DocumentInfo` (`#set document(...)`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataIr {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

/// Page setup extracted from `#set page(...)` and first page geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSetupIr {
    pub width_pt: f64,
    pub height_pt: f64,
    pub margin_top_pt: f64,
    pub margin_bottom_pt: f64,
    pub margin_left_pt: f64,
    pub margin_right_pt: f64,
    pub header_margin_pt: f64,
    pub footer_margin_pt: f64,
    /// Two-sided book binding (`margin: (inside: ..., outside: ...)` -> Word `<w:mirrorMargins/>`).
    #[serde(default)]
    pub mirror_margins: bool,
    pub columns: usize,
    pub fill: Option<ColorHex>,
    pub header: Vec<BlockNode>,
    pub footer: Vec<BlockNode>,
}

impl Default for PageSetupIr {
    fn default() -> Self {
        // Default A4 dimensions in points (595.28 x 841.89 pt) with 2.5cm (~70.87pt) margins.
        Self {
            width_pt: 595.276,
            height_pt: 841.89,
            margin_top_pt: 70.866,
            margin_bottom_pt: 70.866,
            margin_left_pt: 70.866,
            margin_right_pt: 70.866,
            header_margin_pt: 42.5,
            footer_margin_pt: 42.5,
            mirror_margins: false,
            columns: 1,
            fill: None,
            header: Vec::new(),
            footer: Vec::new(),
        }
    }
}

/// Summary report of extracted elements and warnings during conversion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionReport {
    pub pages_count: usize,
    pub headings_count: usize,
    pub paragraphs_count: usize,
    pub lists_count: usize,
    pub tables_count: usize,
    pub equations_count: usize,
    pub shapes_count: usize,
    pub cetz_canvases_count: usize,
    pub images_count: usize,
    pub quotes_count: usize,
    pub code_blocks_count: usize,
    pub figures_count: usize,
    pub footnotes_count: usize,
    pub links_count: usize,
    pub unsupported_elements: Vec<UnsupportedIr>,
    pub compiler_warnings: Vec<String>,
}
