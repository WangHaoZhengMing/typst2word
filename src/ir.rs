//! Rich Semantic + Visual Intermediate Representation (`DocumentIr`) for `typst2word`.
//!
//! Captures both high-level Typst semantics (headings, paragraphs, lists, enums,
//! terms, tables/grids with spans/borders/fills, blockquotes, figures, footnotes,
//! outlines, bibliographies, links, refs, citations, equations) and exact visual
//! geometry (native shapes, CeTZ vector canvases, syntax-highlighted code blocks,
//! typography, page setup) plus explicit unsupported-element markers.

use serde::{Deserialize, Serialize};

/// 6-digit uppercase hex RGB color string (e.g., `"1A2B3C"`).
pub type ColorHex = String;

/// Complete intermediate representation of a compiled Typst document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentIr {
    pub metadata: MetadataIr,
    pub page_setup: PageSetupIr,
    pub blocks: Vec<BlockNode>,
    pub report: ConversionReport,
}

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
            columns: 1,
            fill: None,
            header: Vec::new(),
            footer: Vec::new(),
        }
    }
}

/// Horizontal alignment in paragraphs, cells, and blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HAlignIr {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Vertical alignment in cells and boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VAlignIr {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Paragraph layout and spacing properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParagraphStyleIr {
    pub align: HAlignIr,
    pub first_line_indent_pt: f64,
    pub hanging_indent_pt: f64,
    pub left_indent_pt: f64,
    pub right_indent_pt: f64,
    pub space_before_pt: f64,
    pub space_after_pt: f64,
    pub line_spacing_pt: Option<f64>,
}

impl Default for ParagraphStyleIr {
    fn default() -> Self {
        Self {
            align: HAlignIr::Left,
            first_line_indent_pt: 0.0,
            hanging_indent_pt: 0.0,
            left_indent_pt: 0.0,
            right_indent_pt: 0.0,
            space_before_pt: 0.0,
            space_after_pt: 6.0,
            line_spacing_pt: None,
        }
    }
}

/// Stroke style for borders, lines, and vector shapes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeIr {
    pub color: ColorHex,
    pub thickness_pt: f64,
    pub dash: DashPatternIr,
    pub cap: LineCapIr,
    pub join: LineJoinIr,
}

impl Default for StrokeIr {
    fn default() -> Self {
        Self {
            color: "000000".to_string(),
            thickness_pt: 1.0,
            dash: DashPatternIr::Solid,
            cap: LineCapIr::Butt,
            join: LineJoinIr::Miter,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DashPatternIr {
    #[default]
    Solid,
    Dotted,
    Dashed,
    DashDot,
    DashDotDot,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineCapIr {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineJoinIr {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Four-sided border strokes (e.g. for table cells or rectangular blocks).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BordersIr {
    pub top: Option<StrokeIr>,
    pub bottom: Option<StrokeIr>,
    pub left: Option<StrokeIr>,
    pub right: Option<StrokeIr>,
}

impl BordersIr {
    pub fn splat(stroke: Option<StrokeIr>) -> Self {
        Self {
            top: stroke.clone(),
            bottom: stroke.clone(),
            left: stroke.clone(),
            right: stroke,
        }
    }

    pub fn has_any(&self) -> bool {
        self.top.is_some()
            || self.bottom.is_some()
            || self.left.is_some()
            || self.right.is_some()
    }
}

/// Four-sided padding / inset in points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SidesPtIr {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl Default for SidesPtIr {
    fn default() -> Self {
        Self {
            top: 5.0,
            bottom: 5.0,
            left: 5.0,
            right: 5.0,
        }
    }
}

/// Block-level document node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockNode {
    /// Document title (`#title[...]`).
    Title {
        content: Vec<InlineNode>,
        align: HAlignIr,
    },
    /// Section heading (`= Heading`, `== Subheading`, etc.).
    Heading {
        level: usize,
        numbering: Option<String>,
        content: Vec<InlineNode>,
        bookmark: Option<String>,
        align: HAlignIr,
    },
    /// Standard or styled paragraph.
    Paragraph {
        content: Vec<InlineNode>,
        style: ParagraphStyleIr,
        bookmark: Option<String>,
    },
    /// Unordered bullet list (`- item`).
    List {
        items: Vec<ListItemIr>,
        tight: bool,
        level: usize,
    },
    /// Ordered numbered list (`+ item` or `1. item`).
    Enum {
        items: Vec<EnumItemIr>,
        tight: bool,
        level: usize,
    },
    /// Definition / term list (`/ Term: Description`).
    Terms {
        items: Vec<TermItemIr>,
        tight: bool,
    },
    /// Semantic table (`#table(...)`) or layout grid (`#grid(...)`).
    Table(TableIr),
    /// Block quote (`#quote(block: true, attribution: [...])[...]`).
    Quote {
        body: Vec<BlockNode>,
        attribution: Option<Vec<InlineNode>>,
    },
    /// Fenced or block raw code (` ```rust ... ``` `) with syntax-highlighted runs per line.
    CodeBlock {
        lang: Option<String>,
        lines: Vec<Vec<TextRunIr>>,
        fill: Option<ColorHex>,
        stroke: Option<StrokeIr>,
    },
    /// Figure with caption (`#figure(..., caption: [...])`).
    Figure {
        body: Vec<BlockNode>,
        caption: Option<Vec<InlineNode>>,
        caption_top: bool,
        kind: String,
        bookmark: Option<String>,
    },
    /// Display / block equation (`$ ... $`).
    EquationBlock(EquationIr),
    /// Native Word shape object (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, `path`, `curve`).
    ShapeBlock(ShapeIr),
    /// CeTZ vector drawing canvas (`cetz.canvas(...)`).
    CetzCanvas(CetzCanvasIr),
    /// Standalone image block.
    ImageBlock(ImageIr),
    /// Table of contents (`#outline(...)`).
    Outline {
        title: Option<Vec<InlineNode>>,
        entries: Vec<OutlineEntryIr>,
    },
    /// Bibliography (`#bibliography(...)`).
    Bibliography {
        title: Option<Vec<InlineNode>>,
        entries: Vec<BibEntryIr>,
    },
    /// Horizontal divider line (`#line(length: 100%)`).
    HorizontalRule {
        length_pt: f64,
        stroke: StrokeIr,
        align: HAlignIr,
    },
    /// Explicit page break (`#pagebreak()`).
    PageBreak,
    /// Explicit column break (`#colbreak()`).
    ColumnBreak,
    /// Unsupported Typst element rendered visibly in Word.
    UnsupportedBlock(UnsupportedIr),
}

/// Inline-level document node inside paragraphs, headings, captions, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InlineNode {
    /// Styled text run.
    Text(TextRunIr),
    /// Inline math equation (`$...$`).
    EquationInline(EquationIr),
    /// Inline shape (`#box(rect(...))` or `#circle[A]`).
    ShapeInline(ShapeIr),
    /// Inline CeTZ canvas.
    CetzInline(CetzCanvasIr),
    /// Inline image.
    ImageInline(ImageIr),
    /// Hyperlink (`#link("https://...")[...]` or intra-document link).
    Link {
        url: Option<String>,
        anchor: Option<String>,
        content: Vec<InlineNode>,
    },
    /// Cross-reference (`@label` / `#ref(<label>)`).
    Ref {
        target: String,
        content: Vec<InlineNode>,
    },
    /// Citation (`@citekey` / `#cite(<key>)`).
    Citation {
        keys: Vec<String>,
        content: Vec<InlineNode>,
    },
    /// Footnote reference + content (`#footnote[...]`).
    Footnote {
        id: usize,
        number_text: String,
        content: Vec<BlockNode>,
    },
    /// Hard line break (`\`).
    LineBreak,
    /// Tab stop (`\t`).
    Tab,
    /// Explicit horizontal spacing (`#h(12pt)`).
    HorizontalSpace { width_pt: f64 },
    /// Unsupported inline element rendered visibly (`[不支持的元素: ...]`).
    UnsupportedInline(UnsupportedIr),
}

/// Richly styled text run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TextRunIr {
    pub text: String,
    pub font_family: Option<String>,
    pub font_size_pt: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub underline: Option<StrokeIr>,
    pub strike: Option<StrokeIr>,
    pub overline: Option<StrokeIr>,
    pub color: Option<ColorHex>,
    pub highlight: Option<ColorHex>,
    pub script: ScriptKindIr,
    pub smallcaps: bool,
    pub is_code: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScriptKindIr {
    #[default]
    Normal,
    Sub,
    Super,
}

/// Bullet list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemIr {
    pub marker: String,
    pub blocks: Vec<BlockNode>,
}

/// Numbered list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumItemIr {
    pub number: Option<u64>,
    pub marker_text: String,
    pub blocks: Vec<BlockNode>,
}

/// Term / definition list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermItemIr {
    pub term: Vec<InlineNode>,
    pub description: Vec<BlockNode>,
}

/// Table or Grid intermediate representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIr {
    pub is_semantic_table: bool,
    pub col_widths_pt: Vec<f64>,
    pub row_heights_pt: Vec<f64>,
    pub header_rows: usize,
    pub rows: Vec<TableRowIr>,
    pub align: HAlignIr,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRowIr {
    pub height_pt: Option<f64>,
    pub is_header: bool,
    pub cells: Vec<TableCellIr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCellIr {
    pub x: usize,
    pub y: usize,
    pub colspan: usize,
    pub rowspan: usize,
    /// When `vmerge` is `Some(VMergeState::Restart)`, this cell starts a vertical span;
    /// when `Some(VMergeState::Continue)`, this cell is a continuation placeholder in Word.
    pub vmerge: Option<VMergeState>,
    /// True if this slot is covered horizontally by a preceding cell's `colspan` in the same row.
    pub hmerged_covered: bool,
    pub fill: Option<ColorHex>,
    pub borders: BordersIr,
    pub inset: SidesPtIr,
    pub halign: HAlignIr,
    pub valign: VAlignIr,
    pub is_header_cell: bool,
    pub blocks: Vec<BlockNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VMergeState {
    Restart,
    Continue,
}

/// Mathematical equation IR with native Word OMML (`<m:oMath>` / `<m:oMathPara>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationIr {
    pub is_block: bool,
    /// Complete `<m:oMath>...</m:oMath>` XML fragment.
    pub omml_xml: String,
    /// Plain-text/TeX-like fallback representation for accessibility or debugging.
    pub source_text: String,
    /// Optional equation number text (e.g., `"(1)"` or `"(2.1)"`).
    pub numbering_text: Option<String>,
    /// Optional bookmark name for `@ref` cross-referencing.
    pub bookmark: Option<String>,
}

/// Native Word DrawingML shape representation (`rect`, `square`, `circle`, `ellipse`,
/// `line`, `polygon`, `path`, `curve`, or styled `block`/`box`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeIr {
    pub kind: ShapeKindIr,
    pub width_pt: f64,
    pub height_pt: f64,
    pub fill: Option<ColorHex>,
    pub fill_rule_even_odd: bool,
    pub stroke: Option<StrokeIr>,
    pub borders: BordersIr,
    pub corner_radius_pt: Option<f64>,
    pub inset: SidesPtIr,
    pub rotation_deg: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Optional content inside the shape (e.g. `#rect[Hello]` or `#circle[A]`),
    /// rendered inside `<wps:txbx><w:txbxContent>` in Word.
    pub body: Vec<BlockNode>,
    pub align: HAlignIr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShapeKindIr {
    /// Rectangle (`rect` or `square`), optionally with rounded corners (`roundRect`).
    Rect,
    /// Circle or ellipse (`circle` or `ellipse`).
    Ellipse,
    /// Straight line segment (`line`) from `(x1, y1)` to `(x2, y2)` in points.
    Line {
        x1_pt: f64,
        y1_pt: f64,
        x2_pt: f64,
        y2_pt: f64,
    },
    /// Closed polygon (`polygon`) with vertices in points.
    Polygon { points_pt: Vec<(f64, f64)> },
    /// General Bézier/line path or curve (`path` or `curve`).
    CustomPath { segments: Vec<CurveSegmentIr> },
}

/// Segment of a `Curve` or `Path` in points relative to the shape's top-left origin.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CurveSegmentIr {
    MoveTo { x_pt: f64, y_pt: f64 },
    LineTo { x_pt: f64, y_pt: f64 },
    CubicTo {
        cx1_pt: f64,
        cy1_pt: f64,
        cx2_pt: f64,
        cy2_pt: f64,
        x_pt: f64,
        y_pt: f64,
    },
    Close,
}

/// CeTZ vector drawing canvas (`#import "@preview/cetz:..."`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CetzCanvasIr {
    pub width_pt: f64,
    pub height_pt: f64,
    pub background: Option<ColorHex>,
    pub paths: Vec<CetzPathDrawableIr>,
    pub labels: Vec<CetzLabelDrawableIr>,
    /// Optional rendered SVG fallback bytes if needed by viewers.
    pub svg_fallback: Option<Vec<u8>>,
    pub align: HAlignIr,
}

/// A single vector path/shape inside a CeTZ canvas at exact canvas coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CetzPathDrawableIr {
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub segments: Vec<CurveSegmentIr>,
    pub fill: Option<ColorHex>,
    pub fill_rule_even_odd: bool,
    pub stroke: Option<StrokeIr>,
}

/// A positioned text or formula label inside a CeTZ canvas (`content((x, y), [...])`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CetzLabelDrawableIr {
    pub x_pt: f64,
    pub y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub rotation_deg: f64,
    pub content: Vec<InlineNode>,
}

/// Raster or vector image node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageIr {
    pub data: Vec<u8>,
    pub format: ImageFormatIr,
    pub width_pt: f64,
    pub height_pt: f64,
    pub alt: Option<String>,
    pub align: HAlignIr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormatIr {
    Png,
    Jpeg,
    Gif,
    Svg,
}

impl ImageFormatIr {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Svg => "svg",
        }
    }
}

/// Entry in an `#outline(...)` (Table of Contents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineEntryIr {
    pub level: usize,
    pub prefix: Option<String>,
    pub title: Vec<InlineNode>,
    pub page: String,
    pub target_bookmark: Option<String>,
}

/// Entry in a `#bibliography(...)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BibEntryIr {
    pub prefix: Option<String>,
    pub content: Vec<InlineNode>,
    pub bookmark: Option<String>,
}

/// Explicit representation of an unsupported Typst element or feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsupportedIr {
    pub element_name: String,
    pub reason: String,
    pub source_location: Option<String>,
}

impl UnsupportedIr {
    pub fn display_badge(&self) -> String {
        if self.reason.is_empty() {
            format!("[不支持的元素: {}]", self.element_name)
        } else {
            format!("[不支持的元素: {} ({})]", self.element_name, self.reason)
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
