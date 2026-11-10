//! Exhaustive PDF physical measurement enums extracted from Typst's laid-out `Frame`s.
//!
//! Keeps physical PDF layout measurements (`width_pt`, `height_pt`, `top_y_pt`, `bottom_y_pt`,
//! glyph size bounds, and raw block spans) strictly separated from semantic AST definitions
//! and Word-specific compensation rules.

use serde::{Deserialize, Serialize};

/// PDF physical measurements of a laid-out Typst equation (`EquationElem`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EquationPdfMetricsIr {
    /// No tight bounding box available (e.g. line-wrapped inline equation or synthetic AST).
    Unmeasured {
        context_font_size_pt: f64,
        max_glyph_pt: Option<f64>,
        min_glyph_pt: Option<f64>,
        max_available_width_pt: Option<f64>,
    },
    /// Tight 2D bounding box measured from Typst's laid-out equation frame on a single line/page.
    Measured {
        context_font_size_pt: f64,
        width_pt: f64,
        height_pt: f64,
        max_glyph_pt: Option<f64>,
        min_glyph_pt: Option<f64>,
        max_available_width_pt: Option<f64>,
    },
}

impl Default for EquationPdfMetricsIr {
    fn default() -> Self {
        Self::Unmeasured {
            context_font_size_pt: 11.0,
            max_glyph_pt: None,
            min_glyph_pt: None,
            max_available_width_pt: None,
        }
    }
}

impl EquationPdfMetricsIr {
    /// Construct from optional measured `(width_pt, height_pt)` and typography bounds.
    #[inline]
    pub const fn from_bbox(
        bbox_pt: Option<(f64, f64)>,
        context_font_size_pt: f64,
        max_glyph_pt: Option<f64>,
        min_glyph_pt: Option<f64>,
        max_available_width_pt: Option<f64>,
    ) -> Self {
        match bbox_pt {
            Some((width_pt, height_pt)) => Self::Measured {
                context_font_size_pt,
                width_pt,
                height_pt,
                max_glyph_pt,
                min_glyph_pt,
                max_available_width_pt,
            },
            None => Self::Unmeasured {
                context_font_size_pt,
                max_glyph_pt,
                min_glyph_pt,
                max_available_width_pt,
            },
        }
    }
}

/// PDF page and vertical Y-span measurement of a laid-out block element.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum BlockPdfSpanIr {
    #[default]
    Unmeasured,
    SinglePage {
        page_idx: usize,
        top_y_pt: f64,
        bottom_y_pt: f64,
        x_pt: f64,
    },
    MultiPage {
        start_page_idx: usize,
        top_y_pt: f64,
        end_page_idx: usize,
        bottom_y_pt: f64,
    },
}

impl BlockPdfSpanIr {
    /// Construct a [`BlockPdfSpanIr`] from optional `(page_idx, top_y_pt, x_pt)` and `(page_idx, bottom_y_pt, x_pt)`.
    #[inline]
    pub const fn from_start_end(
        start: Option<(usize, f64, f64)>,
        end: Option<(usize, f64, f64)>,
    ) -> Self {
        match (start, end) {
            (Some((ps, top_y_pt, x_pt)), Some((pe, bottom_y_pt, _))) if ps == pe => {
                Self::SinglePage {
                    page_idx: ps,
                    top_y_pt,
                    bottom_y_pt,
                    x_pt,
                }
            }
            (Some((ps, top_y_pt, _)), Some((pe, bottom_y_pt, _))) => Self::MultiPage {
                start_page_idx: ps,
                top_y_pt,
                end_page_idx: pe,
                bottom_y_pt,
            },
            (Some((ps, top_y_pt, x_pt)), None) => Self::SinglePage {
                page_idx: ps,
                top_y_pt,
                bottom_y_pt: top_y_pt,
                x_pt,
            },
            (None, Some((pe, bottom_y_pt, x_pt))) => Self::SinglePage {
                page_idx: pe,
                top_y_pt: bottom_y_pt,
                bottom_y_pt,
                x_pt,
            },
            (None, None) => Self::Unmeasured,
        }
    }

    /// Return `(start_page_idx, top_y_pt)` if measured.
    #[inline]
    pub const fn start_page_and_top_y(self) -> Option<(usize, f64)> {
        match self {
            Self::Unmeasured => None,
            Self::SinglePage {
                page_idx, top_y_pt, ..
            } => Some((page_idx, top_y_pt)),
            Self::MultiPage {
                start_page_idx,
                top_y_pt,
                ..
            } => Some((start_page_idx, top_y_pt)),
        }
    }

    /// Return `(end_page_idx, bottom_y_pt)` if measured.
    #[inline]
    pub const fn end_page_and_bottom_y(self) -> Option<(usize, f64)> {
        match self {
            Self::Unmeasured => None,
            Self::SinglePage {
                page_idx,
                bottom_y_pt,
                ..
            } => Some((page_idx, bottom_y_pt)),
            Self::MultiPage {
                end_page_idx,
                bottom_y_pt,
                ..
            } => Some((end_page_idx, bottom_y_pt)),
        }
    }
}

/// PDF & Typst physical spacing parameters of a `ParElem` before layout compensation.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ParagraphPdfMetricsIr {
    /// Synthetic or unmeasured paragraph (e.g. stray inline flush or header/footer); uses `space_after_pt` directly.
    #[default]
    Unmeasured,
    /// Extracted from a Typst `ParElem` with resolved `par.spacing`, `par.leading`, and font scales.
    Measured {
        ends_with_explicit_linebreak: bool,
        base_font_pt: f64,
        default_font_pt: f64,
        par_spacing_pt: f64,
        par_leading_pt: f64,
        in_multi_block_group_tail: bool,
    },
}

/// Semantic kind of the following sibling block in vertical flow measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NextBlockKindIr {
    #[default]
    Other,
    Paragraph,
    TightList,
    HeadingFollowedByParagraph,
    ListItemSibling {
        list_level: usize,
        active_columns: usize,
        prev_item_block_count: usize,
        next_item_starts_with_paragraph: bool,
    },
}

/// Raw physical measurement of the following block relative to a preceding block.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct NextBlockMeasureIr {
    pub kind: NextBlockKindIr,
    pub span: BlockPdfSpanIr,
    pub prev_font_pt: f64,
    pub next_font_pt: f64,
}

/// Raw PDF vertical flow measurements attached to a paragraph (`self_span` and `next` block measurement).
/// ┌───────────────────────┐  <- 上一个块的 top_y_pt = 100pt
/// │ 第一段文字……            │
/// └───────────────────────┘  <- 上一个块的 bottom_y_pt = 130pt
///           ↕ 这里的空白就是 gap_pt = 142 - 130 = 12pt
/// ┌───────────────────────┐  <- 下一个块的 top_y_pt = 142pt
/// │ 第二段文字（或者一个列表  │
/// └───────────────────────┘
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FlowMeasureIr {
    pub self_span: BlockPdfSpanIr,
    pub next: Option<NextBlockMeasureIr>,
    /// Additional trailing block-to-block measurements (e.g. when a paragraph is the last block
    /// of a list item followed by another list item or an outer block).
    #[serde(default)]
    pub trailing_transitions: Vec<(BlockPdfSpanIr, NextBlockMeasureIr)>,
}
