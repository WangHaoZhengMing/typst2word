//! Semantic Intermediate Representation (`DocumentIr`) for `typst2word`.
//!
//! Strictly separates three layers of information:
//! 1. **Source Intent & Resolved Semantics** (`BlockNode`, `InlineNode`, `ParagraphStyleIr`, `ContainerIr`).
//! 2. **Unified Node Identity & Provenance** (`ir::origin`: `NodeId`, `NodeOrigin`, `SemanticRoleIr`).
//! 3. **Read-Only Typst Reference Layout** (`ir::reference_layout`: `NodeReferenceLayout`, `LayoutFragmentIr`).
//!
//! Word-specific output decisions (`dxa`, `w:line`, font slot routing, compatibility compensations)
//! belong exclusively to `word-construct`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod anchor;
mod bibliography;
mod cetz;
pub mod container;
mod document;
mod equation;
mod image;
mod list;
pub mod locale;
pub mod normalize;
mod outline;
pub mod origin;
pub mod pdf;
pub mod reference_layout;
mod shape;
mod style;
mod table;
mod text;
pub mod units;
mod unsupported;

pub use anchor::AnchorLineIr;
pub use bibliography::BibEntryIr;
pub use cetz::{CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr};
pub use container::{ContainerIr, ContainerStyleIr};
pub use document::{ConversionReport, MetadataIr, PageNumberFormatIr, PageSetupIr};
pub use equation::{EquationIr, MathFracStyleIr, MathNodeIr, MathRunIr, MathScriptIr, MathStyleIr};
pub use image::{ImageFormatIr, ImageIr};
pub use list::{EnumItemIr, ListItemIr, TermItemIr};
pub use locale::{LocaleIr, TextDirIr};
pub use normalize::{
    clean_redundant_pagebreaks, coalesce_adjacent_text_runs, merge_split_containers,
    merge_split_styled_containers, normalize_document_ir, visit_block_list_mut,
    visit_document_block_lists_mut, visit_document_inline_lists_mut, visit_page_setup_blocks_mut,
};
pub use origin::{NodeId, NodeOrigin, SemanticRoleIr};
pub use outline::OutlineEntryIr;
pub use pdf::{BlockPdfSpanIr, EquationPdfMetricsIr, FlowMeasureIr, NextBlockMeasureIr};
pub use reference_layout::{
    BlockLayoutSpanIr, EquationLayoutMetricsIr, FlowReferenceIr, LayoutFragmentIr, NextBlockKindIr,
    NextBlockLayoutMeasureIr, NodeReferenceLayout,
};
pub use shape::{CurveSegmentIr, ShapeIr, ShapeKindIr};
pub use style::{
    BordersIr, DashPatternIr, HAlignIr, LineCapIr, LineJoinIr, LinebreaksIr, ParagraphStyleIr,
    SidesPtIr, StrokeIr, VAlignIr,
};
pub use table::{TableCellIr, TableIr, TableKindIr, TableRowIr};
pub use text::{ScriptKindIr, TextRunIr, is_cjk_char};
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
    /// Unified node provenance and semantic roles indexed by `NodeId`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub origins: BTreeMap<NodeId, NodeOrigin>,
    /// Read-only Typst reference layout measurements (1-to-N page fragments & flow gaps) indexed by `NodeId`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reference_layout: BTreeMap<NodeId, NodeReferenceLayout>,
    /// Read-only verification line anchors extracted from reference layout (never read by generator).
    #[serde(default)]
    pub anchors: Vec<AnchorLineIr>,
    #[serde(skip)]
    next_node_id: u32,
}

impl DocumentIr {
    /// Allocate a fresh [`NodeId`] and register its [`NodeOrigin`].
    pub fn alloc_node(&mut self, origin: NodeOrigin) -> NodeId {
        if self.next_node_id == 0 {
            self.next_node_id = self
                .origins
                .keys()
                .chain(self.reference_layout.keys())
                .map(|k| k.0)
                .max()
                .unwrap_or(0);
        }
        self.next_node_id += 1;
        let id = NodeId(self.next_node_id);
        self.origins.insert(id, origin);
        id
    }

    /// Allocate a fresh [`NodeId`] with default origin metadata.
    #[inline]
    pub fn alloc_empty_node(&mut self) -> NodeId {
        self.alloc_node(NodeOrigin::default())
    }

    /// Look up the [`NodeOrigin`] of `id`.
    #[inline]
    pub fn origin(&self, id: NodeId) -> Option<&NodeOrigin> {
        (!id.is_unset()).then(|| self.origins.get(&id)).flatten()
    }

    /// Look up mutable [`NodeOrigin`] of `id`.
    #[inline]
    pub fn origin_mut(&mut self, id: NodeId) -> Option<&mut NodeOrigin> {
        (!id.is_unset())
            .then(|| self.origins.get_mut(&id))
            .flatten()
    }

    /// Look up the [`NodeReferenceLayout`] of `id`.
    #[inline]
    pub fn layout(&self, id: NodeId) -> Option<&NodeReferenceLayout> {
        (!id.is_unset())
            .then(|| self.reference_layout.get(&id))
            .flatten()
    }

    /// Access or insert mutable [`NodeReferenceLayout`] for `id`.
    #[inline]
    pub fn layout_mut(&mut self, id: NodeId) -> &mut NodeReferenceLayout {
        self.reference_layout.entry(id).or_default()
    }

    /// Append a laid-out [`LayoutFragmentIr`] to `id` and update its aggregated span.
    pub fn push_layout_fragment(&mut self, id: NodeId, fragment: LayoutFragmentIr) {
        if id.is_unset() {
            return;
        }
        let entry = self.reference_layout.entry(id).or_default();
        entry.fragments.push(fragment);
        entry.span = BlockLayoutSpanIr::from_fragments(&entry.fragments);
    }
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
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        content: Vec<InlineNode>,
        align: HAlignIr,
    },
    /// Section heading (`= Heading`, `== Subheading`, etc.), optionally carrying
    /// realized container styling (`#show heading: it => block(fill: ...)[#it.body]`).
    Heading {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        level: usize,
        numbering: Option<String>,
        content: Vec<InlineNode>,
        bookmark: Option<String>,
        align: HAlignIr,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        box_style: Option<ContainerStyleIr>,
    },
    /// Standard or styled paragraph.
    Paragraph {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        content: Vec<InlineNode>,
        style: ParagraphStyleIr,
        bookmark: Option<String>,
    },
    /// Unordered bullet list (`- item`).
    List {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
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
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
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
    Terms {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        items: Vec<TermItemIr>,
        tight: bool,
    },
    /// Styled block container (`#block(fill: ..., stroke: ..., inset: ...)[...]`).
    Container(ContainerIr),
    /// Semantic table (`#table(...)`) or layout grid (`#grid(...)`).
    Table(TableIr),
    /// Block quote (`#quote(block: true, attribution: [...])[...]`).
    Quote {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        body: Vec<BlockNode>,
        attribution: Option<Vec<InlineNode>>,
    },
    /// Fenced or block raw code (` ```rust ... ``` `) with syntax-highlighted runs per line.
    CodeBlock {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        lang: Option<String>,
        lines: Vec<Vec<TextRunIr>>,
        fill: Option<ColorHex>,
        stroke: Option<StrokeIr>,
    },
    /// Figure with caption (`#figure(..., caption: [...])`).
    Figure {
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        body: Vec<BlockNode>,
        caption: Option<Vec<InlineNode>>,
        caption_pos: CaptionPositionIr,
        kind: FigureKindIr,
        bookmark: Option<String>,
    },
    /// Display / block equation (`$ ... $`).
    EquationBlock(EquationIr),
    /// Typst shape object (`rect`, `square`, `circle`, `ellipse`, `line`, `polygon`, `path`, `curve`).
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
        #[serde(default, skip_serializing_if = "NodeId::is_unset")]
        id: NodeId,
        count: usize,
        gutter_pt: f64,
        body: Vec<BlockNode>,
    },
    /// Unsupported Typst element marker.
    UnsupportedBlock(UnsupportedIr),
}

impl BlockNode {
    /// Return the semantic [`NodeId`] of this block (`NodeId::UNSET` if unassigned).
    pub const fn node_id(&self) -> NodeId {
        match self {
            Self::Title { id, .. }
            | Self::Heading { id, .. }
            | Self::Paragraph { id, .. }
            | Self::List { id, .. }
            | Self::Enum { id, .. }
            | Self::Terms { id, .. }
            | Self::Quote { id, .. }
            | Self::CodeBlock { id, .. }
            | Self::Figure { id, .. }
            | Self::Columns { id, .. } => *id,
            Self::Container(c) => c.id,
            Self::Table(t) => t.id,
            Self::EquationBlock(eq) => eq.id,
            Self::ShapeBlock(s) => s.id,
            Self::CetzCanvas(c) => c.id,
            Self::ImageBlock(i) => i.id,
            Self::Outline { .. }
            | Self::Bibliography { .. }
            | Self::HorizontalRule { .. }
            | Self::PageBreak
            | Self::ColumnBreak
            | Self::UnsupportedBlock(_) => NodeId::UNSET,
        }
    }

    /// Assign a semantic [`NodeId`] to this block if supported.
    pub fn set_node_id(&mut self, new_id: NodeId) {
        match self {
            Self::Title { id, .. }
            | Self::Heading { id, .. }
            | Self::Paragraph { id, .. }
            | Self::List { id, .. }
            | Self::Enum { id, .. }
            | Self::Terms { id, .. }
            | Self::Quote { id, .. }
            | Self::CodeBlock { id, .. }
            | Self::Figure { id, .. }
            | Self::Columns { id, .. } => *id = new_id,
            Self::Container(c) => c.id = new_id,
            Self::Table(t) => t.id = new_id,
            Self::EquationBlock(eq) => eq.id = new_id,
            Self::ShapeBlock(s) => s.id = new_id,
            Self::CetzCanvas(c) => c.id = new_id,
            Self::ImageBlock(i) => i.id = new_id,
            Self::Outline { .. }
            | Self::Bibliography { .. }
            | Self::HorizontalRule { .. }
            | Self::PageBreak
            | Self::ColumnBreak
            | Self::UnsupportedBlock(_) => {}
        }
    }
}

/// Dynamic document field (`counter(page)`, page-ref), with a cached display value retained from Typst.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldKindIr {
    Page,
    NumPages,
    PageRef { bookmark: String },
}

/// Inline-level document node inside paragraphs, headings, captions, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InlineNode {
    Field {
        kind: FieldKindIr,
        format: Option<String>,
        content: Vec<InlineNode>,
    },
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
    /// Unsupported inline element marker.
    UnsupportedInline(UnsupportedIr),
}
