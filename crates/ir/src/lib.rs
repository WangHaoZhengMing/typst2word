//! Rich Semantic + PDF Metrics Intermediate Representation (`DocumentIr`) for `typst2word`.
//!
//! Captures both high-level Typst semantics (headings, paragraphs, lists, enums,
//! terms, tables/grids with spans/borders/fills, blockquotes, figures, footnotes,
//! outlines, bibliographies, links, refs, citations, equations) and laid-out PDF
//! geometry (`ir::pdf`) plus explicit unsupported-element markers.
//!
//! One file per Typst object family; everything is re-exported from here so
//! `crate::ir::*` keeps working.

use serde::{Deserialize, Serialize};

pub mod anchor;
mod bibliography;
mod cetz;
mod document;
mod equation;
mod image;
mod list;
pub mod normalize;
mod outline;
pub mod pdf;
mod shape;
mod style;
mod table;
mod text;
pub mod units;
mod unsupported;

pub use anchor::{
    AnchorDiffEntry, AnchorDiffReport, AnchorLineIr, AnchorMismatchKind, compare_line_anchors,
};
pub use bibliography::BibEntryIr;
pub use cetz::{CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr};
pub use document::{ConversionReport, MetadataIr, PageSetupIr};
pub use equation::{EquationIr, MathFracStyleIr, MathNodeIr, MathRunIr, MathScriptIr, MathStyleIr};
pub use image::{ImageFormatIr, ImageIr};
pub use list::{EnumItemIr, ListItemIr, TermItemIr};
pub use normalize::{
    clean_redundant_pagebreaks, coalesce_adjacent_text_runs, merge_split_styled_containers,
    normalize_document_ir, visit_block_list_mut, visit_document_block_lists_mut,
    visit_document_inline_lists_mut,
};
pub use outline::OutlineEntryIr;
pub use pdf::{BlockPdfSpanIr, EquationPdfMetricsIr, InterBlockFlowIr, ParagraphPdfMetricsIr};
pub use shape::{CurveSegmentIr, ShapeIr, ShapeKindIr};
pub use style::{
    BordersIr, DashPatternIr, HAlignIr, LineCapIr, LineJoinIr, LinebreaksIr, ParagraphStyleIr,
    SidesPtIr, StrokeIr, VAlignIr,
};
pub use table::{TableCellIr, TableIr, TableKindIr, TableRowIr, VMergeState};
pub use text::{
    FontFamilyClassIr, ScriptKindIr, TextRunIr, TextScriptClassIr, is_cjk_char,
    is_east_asian_symbol_char,
};
pub use units::Pt;
pub use unsupported::UnsupportedIr;

/// 6-digit uppercase hex RGB color string (e.g., `"1A2B3C"`).
pub type ColorHex = String;

/// Semantic kind of a `BlockNode::Figure` (`image`, `table`, `raw`, or custom).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FigureKindIr {
    #[default]
    Image,
    Table,
    Code,
    Custom(String),
}

impl FigureKindIr {
    pub fn from_typst_repr(repr: &str) -> Self {
        let lower = repr.to_ascii_lowercase();
        if lower.contains("table") {
            Self::Table
        } else if lower.contains("raw") || lower.contains("code") {
            Self::Code
        } else if lower.contains("image") || lower == "auto" {
            Self::Image
        } else {
            Self::Custom(repr.to_string())
        }
    }
}

/// Position of a figure caption relative to its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CaptionPositionIr {
    Top,
    #[default]
    Bottom,
}

/// Target destination of an inline hyperlink (`#link(...)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkTargetIr {
    /// External URI (`https://...`, `mailto:...`).
    Url(String),
    /// Intra-document bookmark anchor (`_typst_loc_...`).
    Anchor(String),
}

/// Complete intermediate representation of a compiled Typst document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentIr {
    /// Subsequent sections; `page_setup` and `blocks` retain the first section.
    #[serde(default)]
    pub sections: Vec<DocumentSectionIr>,
    pub metadata: MetadataIr,
    pub page_setup: PageSetupIr,
    pub blocks: Vec<BlockNode>,
    pub report: ConversionReport,
    /// Read-only verification line anchors extracted from reference layout (never read by generator).
    #[serde(default)]
    pub anchors: Vec<AnchorLineIr>,
}

/// A new page section with independent furniture and page geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSectionIr {
    pub page_setup: PageSetupIr,
    pub blocks: Vec<BlockNode>,
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
        #[serde(default)]
        spacing_pt: Option<f64>,
        #[serde(default)]
        indent_pt: f64,
        #[serde(default)]
        hanging_pt: f64,
    },
    /// Ordered numbered list (`+ item` or `1. item`).
    Enum {
        items: Vec<EnumItemIr>,
        tight: bool,
        level: usize,
        #[serde(default)]
        spacing_pt: Option<f64>,
        #[serde(default)]
        indent_pt: f64,
        #[serde(default)]
        hanging_pt: f64,
    },
    /// Definition / term list (`/ Term: Description`).
    Terms { items: Vec<TermItemIr>, tight: bool },
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
        caption_pos: CaptionPositionIr,
        kind: FigureKindIr,
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
    /// Multi-column layout section (`#columns(N, gutter: ...)[...]`).
    Columns {
        count: usize,
        gutter_pt: f64,
        body: Vec<BlockNode>,
    },
    /// Unsupported Typst element rendered visibly in Word.
    UnsupportedBlock(UnsupportedIr),
}

/// Dynamic Word field, with a cached display value retained from Typst.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldKindIr {
    Page,
    NumPages,
    PageRef { bookmark: String },
}

/// Inline-level document node inside paragraphs, headings, captions, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InlineNode {
    Field { kind: FieldKindIr, format: Option<String>, content: Vec<InlineNode> },
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
        target: Option<LinkTargetIr>,
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
