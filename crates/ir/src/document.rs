//! Document-level metadata, page setup and the conversion report.

use serde::{Deserialize, Serialize};

use super::{BlockNode, ColorHex, LocaleIr, TextDirIr, UnsupportedIr};

/// Document-level metadata extracted from `DocumentInfo` (`#set document(...)`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataIr {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    /// Document-level primary locale (`DocumentInfo::locale` / top-level `#set text(lang: ...)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<LocaleIr>,
    /// Document-level primary text progression direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<TextDirIr>,
}

/// Page numbering format (`<w:pgNumType w:fmt="..."/>`), mirroring `typst-pdf`'s `PageLabel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageNumberFormatIr {
    Arabic,
    LowerRoman,
    UpperRoman,
    LowerAlpha,
    UpperAlpha,
}

impl PageNumberFormatIr {
    pub const fn as_word_fmt(self) -> &'static str {
        match self {
            Self::Arabic => "decimal",
            Self::LowerRoman => "lowerRoman",
            Self::UpperRoman => "upperRoman",
            Self::LowerAlpha => "lowerLetter",
            Self::UpperAlpha => "upperLetter",
        }
    }
}

/// Page setup extracted from `#set page(...)` and section geometry.
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
    /// Default (or odd-page when `different_odd_even` is true) header.
    pub header: Vec<BlockNode>,
    /// Default (or odd-page when `different_odd_even` is true) footer.
    pub footer: Vec<BlockNode>,
    /// Optional distinct first-page header (`<w:titlePg/>` + `w:type="first"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_page_header: Option<Vec<BlockNode>>,
    /// Optional distinct first-page footer (`<w:titlePg/>` + `w:type="first"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_page_footer: Option<Vec<BlockNode>>,
    /// Optional distinct even-page header (`<w:evenAndOddHeaders/>` + `w:type="even"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_page_header: Option<Vec<BlockNode>>,
    /// Optional distinct even-page footer (`<w:evenAndOddHeaders/>` + `w:type="even"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_page_footer: Option<Vec<BlockNode>>,
    #[serde(default)]
    pub different_first_page: bool,
    #[serde(default)]
    pub different_odd_even: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_number_format: Option<PageNumberFormatIr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_number_start: Option<u64>,
}

impl PageSetupIr {
    /// Check whether two `PageSetupIr` instances share identical physical page geometry
    /// (allowing first-page / odd-even header differences to be consolidated into one Word section).
    pub fn has_same_geometry(&self, other: &Self) -> bool {
        self.has_same_page_box(other)
            && (self.header_margin_pt - other.header_margin_pt).abs() <= 0.2
            && (self.footer_margin_pt - other.footer_margin_pt).abs() <= 0.2
    }

    /// Paper size, body margins, columns, fill, and numbering format — excluding the
    /// header/footer distance, which is derived from whether furniture is present on a page
    /// (an empty first-page header measures differently from a populated running header).
    pub fn has_same_page_box(&self, other: &Self) -> bool {
        (self.width_pt - other.width_pt).abs() <= 0.1
            && (self.height_pt - other.height_pt).abs() <= 0.1
            && (self.margin_top_pt - other.margin_top_pt).abs() <= 0.1
            && (self.margin_bottom_pt - other.margin_bottom_pt).abs() <= 0.1
            && (self.margin_left_pt - other.margin_left_pt).abs() <= 0.1
            && (self.margin_right_pt - other.margin_right_pt).abs() <= 0.1
            && self.mirror_margins == other.mirror_margins
            && self.columns == other.columns
            && self.fill == other.fill
            && self.page_number_format == other.page_number_format
    }
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
            first_page_header: None,
            first_page_footer: None,
            even_page_header: None,
            even_page_footer: None,
            different_first_page: false,
            different_odd_even: false,
            page_number_format: None,
            page_number_start: None,
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
