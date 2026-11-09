//! Exhaustive PDF physical measurement enums extracted from Typst's laid-out `Frame`s.
//!
//! Keeps physical PDF layout measurements (`width_pt`, `height_pt`, `top_y_pt`, `bottom_y_pt`,
//! glyph size bounds, and inter-block vertical gaps) strictly separated from semantic AST definitions.

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

/// PDF & Typst physical spacing parameters of a `ParElem` before Word line-box compensation.
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

/// Exhaustive classification of the vertical flow relationship between two consecutive blocks.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum InterBlockFlowIr {
    /// Standard flow without special cross-block overrides.
    #[default]
    Standard,
    /// Immediately precedes a tight `List` or `Enum` on the same page (`gap_pt <= 10.0`).
    BeforeTightList { gap_pt: f64 },
    /// Measured vertical gap between two consecutive paragraphs on the same page.
    SamePageParagraphGap {
        gap_pt: f64,
        prev_font_pt: f64,
        next_font_pt: f64,
    },
    /// Measured vertical gap between this block and a non-paragraph block on the same page.
    SamePageBlockGap {
        gap_pt: f64,
        prev_font_pt: f64,
        next_font_pt: f64,
    },
}
