//! Rich Semantic + Visual Intermediate Representation (`DocumentIr`) for `typst2word`.
//!
//! Captures both high-level Typst semantics (headings, paragraphs, lists, enums,
//! terms, tables/grids with spans/borders/fills, blockquotes, figures, footnotes,
//! outlines, bibliographies, links, refs, citations, equations) and exact visual
//! geometry (native shapes, CeTZ vector canvases, syntax-highlighted code blocks,
//! typography, page setup) plus explicit unsupported-element markers.
//!
//! One file per Typst object family; everything is re-exported from here so
//! `crate::ir::*` keeps working.

use serde::{Deserialize, Serialize};

mod bibliography;
mod cetz;
mod document;
mod equation;
mod image;
mod list;
mod outline;
mod shape;
mod style;
mod table;
mod text;
mod unsupported;

pub use bibliography::BibEntryIr;
pub use cetz::{CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr};
pub use document::{ConversionReport, MetadataIr, PageSetupIr};
pub use equation::EquationIr;
pub use image::{ImageFormatIr, ImageIr};
pub use list::{EnumItemIr, ListItemIr, TermItemIr};
pub use outline::OutlineEntryIr;
pub use shape::{CurveSegmentIr, ShapeIr, ShapeKindIr};
pub use style::{
    BordersIr, DashPatternIr, HAlignIr, LineCapIr, LineJoinIr, ParagraphStyleIr, SidesPtIr,
    StrokeIr, VAlignIr,
};
pub use table::{TableCellIr, TableIr, TableRowIr, VMergeState};
pub use text::{ScriptKindIr, TextRunIr};
pub use unsupported::UnsupportedIr;

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
    /// Multi-column layout section (`#columns(N, gutter: ...)[...]`).
    Columns {
        count: usize,
        gutter_pt: f64,
        body: Vec<BlockNode>,
    },
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
