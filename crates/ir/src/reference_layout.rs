//! Read-only Typst Reference Layout Measurements (`ReferenceLayout`).
//!
//! Captures geometric measurements from Typst's laid-out `PagedDocument` `Frame`s
//! (1-to-N page/column fragments, bounding boxes, and sibling block vertical gaps)
//! strictly separated from source document intent (`ParagraphStyleIr`, `BlockNode`)
//! and Microsoft Word output constructs (`word-construct`).

use serde::{Deserialize, Serialize};

/// A single laid-out 2D fragment of a semantic node on a specific Typst page.
///
/// A single semantic node (e.g., a paragraph, styled container, or table cell) may produce
/// multiple [`LayoutFragmentIr`] entries when broken across columns or pages.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct LayoutFragmentIr {
    pub page_idx: usize,
    pub x_pt: f64,
    pub top_y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
}

impl LayoutFragmentIr {
    #[inline]
    pub const fn bottom_y_pt(&self) -> f64 {
        self.top_y_pt + self.height_pt
    }
}

/// Reference layout page and vertical Y-span measurement of a laid-out block element.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum BlockLayoutSpanIr {
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

impl BlockLayoutSpanIr {
    /// Construct a [`BlockLayoutSpanIr`] from optional `(page_idx, top_y_pt, x_pt)` and `(page_idx, bottom_y_pt, x_pt)`.
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

    /// Derive aggregated vertical span from 1-to-N [`LayoutFragmentIr`] slices.
    pub fn from_fragments(fragments: &[LayoutFragmentIr]) -> Self {
        let Some(first) = fragments.first() else {
            return Self::Unmeasured;
        };
        let last = fragments.last().unwrap_or(first);
        if first.page_idx == last.page_idx {
            let min_top = fragments
                .iter()
                .map(|f| f.top_y_pt)
                .fold(first.top_y_pt, f64::min);
            let max_bottom = fragments
                .iter()
                .map(LayoutFragmentIr::bottom_y_pt)
                .fold(first.bottom_y_pt(), f64::max);
            Self::SinglePage {
                page_idx: first.page_idx,
                top_y_pt: min_top,
                bottom_y_pt: max_bottom,
                x_pt: first.x_pt,
            }
        } else {
            Self::MultiPage {
                start_page_idx: first.page_idx,
                top_y_pt: first.top_y_pt,
                end_page_idx: last.page_idx,
                bottom_y_pt: last.bottom_y_pt(),
            }
        }
    }

    /// Merge two spans across sequential fragments of the same logical node.
    #[inline]
    pub fn merge_with(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unmeasured, b) => b,
            (a, Self::Unmeasured) => a,
            (a, b) => {
                let Some((ps, top_a)) = a.start_page_and_top_y() else {
                    return b;
                };
                let Some((pe, bottom_b)) = b.end_page_and_bottom_y() else {
                    return a;
                };
                let x_a = match a {
                    Self::SinglePage { x_pt, .. } => x_pt,
                    _ => 0.0,
                };
                if ps == pe {
                    Self::SinglePage {
                        page_idx: ps,
                        top_y_pt: top_a,
                        bottom_y_pt: bottom_b,
                        x_pt: x_a,
                    }
                } else {
                    Self::MultiPage {
                        start_page_idx: ps,
                        top_y_pt: top_a,
                        end_page_idx: pe,
                        bottom_y_pt: bottom_b,
                    }
                }
            }
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

/// Reference layout measurements of a laid-out Typst equation (`EquationElem`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EquationLayoutMetricsIr {
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

impl Default for EquationLayoutMetricsIr {
    fn default() -> Self {
        Self::Unmeasured {
            context_font_size_pt: 11.0,
            max_glyph_pt: None,
            min_glyph_pt: None,
            max_available_width_pt: None,
        }
    }
}

impl EquationLayoutMetricsIr {
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

/// Semantic kind of the following sibling block in reference layout vertical flow measurements.
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

/// Reference layout measurement of a following sibling block relative to a preceding block.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct NextBlockLayoutMeasureIr {
    pub kind: NextBlockKindIr,
    pub span: BlockLayoutSpanIr,
    pub prev_font_pt: f64,
    pub next_font_pt: f64,
}

/// Reference vertical flow measurements attached to a node (`self_span` and sibling transitions).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FlowReferenceIr {
    pub self_span: BlockLayoutSpanIr,
    pub next: Option<NextBlockLayoutMeasureIr>,
    /// Whether this paragraph was the trailing block inside a multi-block layout group.
    #[serde(default)]
    pub in_multi_block_group_tail: bool,
    /// Additional trailing block-to-block measurements (e.g. list item sibling gaps or outer block gaps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailing_transitions: Vec<(BlockLayoutSpanIr, NextBlockLayoutMeasureIr)>,
}

/// Read-only baseline-to-baseline line geometry measured from Typst's laid-out paragraph frames.
///
/// Captures Typst's exact baseline step ($\Delta y = H_{\text{cap}} + L_{\text{leading}}$ under
/// default `top-edge: "cap-height", bottom-edge: "baseline"`) so `word-construct` can calibrate
/// Word's `<w:spacing w:line="..." w:lineRule="auto"/>` without guessing font cap-heights.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ParagraphLineMetricsIr {
    /// Total number of normal text lines laid out across all fragments of this paragraph.
    pub line_count: usize,
    /// Decoded page-local Y coordinate (`pt`) of the first line's baseline.
    pub first_baseline_y_pt: f64,
    /// Decoded page-local Y coordinate (`pt`) of the last line's baseline.
    pub last_baseline_y_pt: f64,
    /// Median baseline-to-baseline step (`pt`) across adjacent normal text lines on the same page/column
    /// (`None` for single-line paragraphs or when every line step is distorted by a tall inline object).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniform_baseline_step_pt: Option<f64>,
    /// Whether any line of this paragraph contains an inline shape/image/box taller than normal text.
    #[serde(default)]
    pub has_tall_inline_object: bool,
}

/// Measured distance from an inline box's top/bottom edge to an adjacent text baseline in the same paragraph.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AdjacentTextLineIr {
    /// Vertical distance (`pt`) between the inline box edge and the adjacent text line's baseline.
    pub gap_pt: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    pub font_size_pt: f64,
    #[serde(default)]
    pub is_cjk: bool,
}

/// Read-only surrounding paragraph-line context for an inline box/shape (`InlineNode::ShapeInline`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InlineBoxLineContextIr {
    /// True when text in the parent paragraph shares the same line horizontally with this inline box.
    #[serde(default)]
    pub shares_line_with_text: bool,
    /// Nearest preceding text line in the same paragraph (`None` when at the start of the paragraph).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_line: Option<AdjacentTextLineIr>,
    /// Nearest following text line in the same paragraph (`None` when at the end of the paragraph).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_line: Option<AdjacentTextLineIr>,
}

/// Complete reference layout entry for a semantic [`crate::NodeId`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NodeReferenceLayout {
    /// 1-to-N laid-out fragments across pages/columns in Typst's reference layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fragments: Vec<LayoutFragmentIr>,
    /// Aggregated vertical span in Typst's reference layout.
    #[serde(default)]
    pub span: BlockLayoutSpanIr,
    /// Optional sibling block vertical flow measurements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<FlowReferenceIr>,
    /// Optional intra-paragraph baseline-to-baseline step measurements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph_lines: Option<ParagraphLineMetricsIr>,
    /// Optional surrounding line clearances for an inline `#box` / `ShapeInline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_box_context: Option<InlineBoxLineContextIr>,
}
