use serde::{Deserialize, Serialize};

/// Top-level typesetting test suite category mirroring Typst's typesetting reference chapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Category {
    Model,
    Text,
    Math,
    Symbols,
    Layout,
    Visualize,
    Compositions,
    Packages,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Model,
        Category::Text,
        Category::Math,
        Category::Symbols,
        Category::Layout,
        Category::Visualize,
        Category::Compositions,
        Category::Packages,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Model => "01_model",
            Self::Text => "02_text",
            Self::Math => "03_math",
            Self::Symbols => "04_symbols",
            Self::Layout => "05_layout",
            Self::Visualize => "06_visualize",
            Self::Compositions => "90_compositions",
            Self::Packages => "99_packages",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Model => "01 · Model",
            Self::Text => "02 · Text",
            Self::Math => "03 · Math",
            Self::Symbols => "04 · Symbols",
            Self::Layout => "05 · Layout",
            Self::Visualize => "06 · Visualize",
            Self::Compositions => "90 · Compositions",
            Self::Packages => "99 · Packages",
        }
    }

    pub fn short_title(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Text => "Text",
            Self::Math => "Math",
            Self::Symbols => "Symbols",
            Self::Layout => "Layout",
            Self::Visualize => "Visualize",
            Self::Compositions => "Compositions",
            Self::Packages => "Packages",
        }
    }

    pub fn from_dir_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.slug() == slug)
    }
}

/// Standard uniform page margin (`10pt` on top, right, bottom, left) used across
/// single-variable visual test cases so Word never clips edge strokes while keeping
/// clean, symmetric 10pt reference borders on all four sides.
pub const DEFAULT_MARGIN_PT: f64 = 10.0;

pub fn default_margins_pt() -> [f64; 4] {
    [
        DEFAULT_MARGIN_PT,
        DEFAULT_MARGIN_PT,
        DEFAULT_MARGIN_PT,
        DEFAULT_MARGIN_PT,
    ]
}

/// Declarative structural & OOXML expectation for a single-variable test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectationSpec {
    /// Expected `[top, right, bottom, left]` page margins in pt (`[10, 10, 10, 10]` by default).
    #[serde(default = "default_margins_pt")]
    pub expected_margins_pt: [f64; 4],
    /// Optional per-section `[top, right, bottom, left]` margins in pt for multi-section cases.
    #[serde(default)]
    pub expected_section_margins_pt: Vec<[f64; 4]>,
    /// Optional explicit expectation for two-sided `mirrorMargins` (`word/settings.xml`).
    #[serde(default)]
    pub expected_mirror_margins: Option<bool>,
    /// Expected `BlockNode` variant tag(s) at top level (e.g. `"Paragraph"`, `"Table"`, `"CetzCanvas"`).
    #[serde(default)]
    pub required_ir_kinds: Vec<String>,
    /// Substrings that MUST appear in `word/*.xml`.
    #[serde(default)]
    pub required_xml_snippets: Vec<String>,
    /// Whether `UnsupportedBlock` / `UnsupportedInline` is intentionally expected.
    #[serde(default)]
    pub allow_unsupported: bool,
    /// Maximum allowed `|Δleft|` in pt before flagging `Drift`.
    pub max_delta_left_pt: f64,
    /// Maximum allowed `|Δtop|` in pt before flagging `Drift`.
    pub max_delta_top_pt: f64,
    /// Maximum allowed `|Δwidth|` in pt before flagging `Drift`.
    pub max_delta_width_pt: f64,
    /// Maximum allowed `|Δheight|` in pt before flagging `Drift`.
    pub max_delta_height_pt: f64,
}

impl Default for ExpectationSpec {
    fn default() -> Self {
        Self::default_margin()
    }
}

impl ExpectationSpec {
    pub fn default_margin() -> Self {
        Self {
            expected_margins_pt: default_margins_pt(),
            expected_section_margins_pt: Vec::new(),
            expected_mirror_margins: None,
            required_ir_kinds: Vec::new(),
            required_xml_snippets: Vec::new(),
            allow_unsupported: false,
            max_delta_left_pt: 3.5,
            max_delta_top_pt: 4.5,
            max_delta_width_pt: 14.0,
            max_delta_height_pt: 12.0,
        }
    }

    pub fn with_ir(mut self, kinds: &[&str]) -> Self {
        self.required_ir_kinds = kinds.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn with_xml(mut self, snippets: &[&str]) -> Self {
        self.required_xml_snippets = snippets.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn with_tolerances(mut self, left: f64, top: f64, width: f64, height: f64) -> Self {
        self.max_delta_left_pt = left;
        self.max_delta_top_pt = top;
        self.max_delta_width_pt = width;
        self.max_delta_height_pt = height;
        self
    }
}

/// Atomic single-variable Typst test specification loaded from `suites/**/*.typ`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseSpec {
    pub id: String,
    pub category: Category,
    #[serde(default)]
    pub suite_rel_path: String,
    #[serde(default)]
    pub target_func: String,
    #[serde(default)]
    pub target_param: String,
    pub title: String,
    /// Exact single variable isolated by this case (e.g. `par.leading = 1.2em`).
    pub isolated_variable: String,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub typst_source: String,
    pub expectation: ExpectationSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaseStatus {
    /// All IR/OOXML checks passed AND Word visual/geometric parity is within tolerance.
    Pass,
    /// IR/OOXML checks passed, but Word visual rendering exceeds geometric tolerance.
    Drift,
    /// Compilation error, IR/OOXML structural assertion failure, or missing output.
    Fail,
    /// IR/OOXML checks passed; Word PDF rendering was skipped (`--skip-word`).
    IrOnly,
}

impl CaseStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Drift => "DRIFT",
            Self::Fail => "FAIL",
            Self::IrOnly => "IR-OK",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckItem {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BBoxPt {
    #[serde(rename = "leftPt")]
    pub left_pt: f64,
    #[serde(rename = "topPt")]
    pub top_pt: f64,
    #[serde(rename = "rightPt")]
    pub right_pt: f64,
    #[serde(rename = "bottomPt")]
    pub bottom_pt: f64,
    #[serde(rename = "widthPt")]
    pub width_pt: f64,
    #[serde(rename = "heightPt")]
    pub height_pt: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineMetric {
    pub page: usize,
    pub text: String,
    #[serde(rename = "xPt")]
    pub x_pt: f64,
    #[serde(rename = "yTopPt")]
    pub y_top_pt: f64,
    #[serde(rename = "widthPt")]
    pub width_pt: f64,
    #[serde(rename = "heightPt")]
    pub height_pt: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfVisualSummary {
    #[serde(rename = "pageCount")]
    pub page_count: usize,
    #[serde(rename = "pageWidthPt")]
    pub page_width_pt: f64,
    #[serde(rename = "pageHeightPt")]
    pub page_height_pt: f64,
    #[serde(rename = "inkBBox")]
    pub ink_bbox: Option<BBoxPt>,
    #[serde(rename = "inkPixels")]
    pub ink_pixels: usize,
    pub lines: Vec<LineMetric>,
    #[serde(rename = "pngPath")]
    pub png_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairComparison {
    #[serde(rename = "deltaLeftPt")]
    pub delta_left_pt: f64,
    #[serde(rename = "deltaTopPt")]
    pub delta_top_pt: f64,
    #[serde(rename = "deltaRightPt")]
    pub delta_right_pt: f64,
    #[serde(rename = "deltaBottomPt")]
    pub delta_bottom_pt: f64,
    #[serde(rename = "deltaWidthPt")]
    pub delta_width_pt: f64,
    #[serde(rename = "deltaHeightPt")]
    pub delta_height_pt: f64,
    #[serde(rename = "rawIou")]
    pub raw_iou: f64,
    #[serde(rename = "alignedIou")]
    pub aligned_iou: f64,
    #[serde(rename = "overlapPixels")]
    pub overlap_pixels: usize,
    #[serde(rename = "unionPixels")]
    pub union_pixels: usize,
    #[serde(rename = "lineCountDelta")]
    pub line_count_delta: isize,
    #[serde(rename = "overlayPngPath")]
    pub overlay_png_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IrSummary {
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub margins_pt: [f64; 4], // top, right, bottom, left
    #[serde(default)]
    pub mirror_margins: bool,
    #[serde(default)]
    pub section_margins_pt: Vec<[f64; 4]>,
    pub header_footer_margin_pt: [f64; 2],
    pub sections_count: usize,
    pub top_block_kinds: Vec<String>,
    pub anchor_lines_count: usize,
    pub anchor_bbox: Option<BBoxPt>,
    pub has_cjk: bool,
    pub has_cetz_canvas: bool,
    pub pg_mar_xml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseReport {
    pub id: String,
    pub category: Category,
    #[serde(default)]
    pub suite_rel_path: String,
    #[serde(default)]
    pub target_func: String,
    #[serde(default)]
    pub target_param: String,
    pub title: String,
    pub isolated_variable: String,
    pub status: CaseStatus,
    pub status_reasons: Vec<String>,
    pub typst_source: String,
    pub case_dir: String,
    pub ir_json_path: String,
    pub document_xml_path: String,
    pub docx_path: String,
    pub typst_pdf_path: String,
    pub word_pdf_path: Option<String>,
    pub ir_summary: IrSummary,
    pub checks: Vec<CheckItem>,
    pub typst_visual: Option<PdfVisualSummary>,
    pub word_visual: Option<PdfVisualSummary>,
    pub comparison: Option<PairComparison>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateStats {
    pub total_cases: usize,
    pub pass_count: usize,
    pub drift_count: usize,
    pub fail_count: usize,
    pub ir_only_count: usize,
    pub mean_abs_delta_left_pt: f64,
    pub mean_abs_delta_top_pt: f64,
    pub mean_abs_delta_width_pt: f64,
    pub mean_abs_delta_height_pt: f64,
    pub mean_raw_iou: f64,
    pub mean_aligned_iou: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabReport {
    pub generated_at_unix: u64,
    pub out_dir: String,
    pub word_enabled: bool,
    pub stats: AggregateStats,
    pub cases: Vec<CaseReport>,
}
