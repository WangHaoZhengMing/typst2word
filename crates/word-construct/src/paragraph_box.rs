//! Microsoft Word Paragraph Box (`WordParagraphConstruct`) and Inter-Block Vertical Spacing Solver.
//!
//! Models ISO/IEC 29500-1 §17.3.1.33 (`<w:spacing>`) and §17.3.1.12 (`<w:ind>`),
//! solving for `w:before`, `w:after`, and `w:line` from Typst's layout geometry and
//! Word's `OS/2` line-box internal leading.

use serde::{Deserialize, Serialize};

use crate::font_metrics::WordLineBox;

/// Horizontal justification in Word (`<w:jc w:val="..."/>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordJc {
    Left,
    Center,
    Right,
    Both,
}

impl WordJc {
    #[inline]
    pub const fn as_xml_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Both => "both",
        }
    }
}

/// Typographic scale profile of a paragraph relative to the document's default font size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParagraphScaleProfile {
    /// Terminated by an explicit `linebreak()` (`\`), suppressing inter-paragraph spacing.
    ExplicitLinebreakTerminated,
    /// Large display-sized paragraph (`base_font_pt > default_font_pt * 1.4`, e.g. `19.8pt` title).
    DisplayTitle,
    /// Medium heading/subtitle scale (`base_font_pt >= default_font_pt * 1.2`).
    SubHeading,
    /// Normal body paragraph.
    BodyText,
}

impl ParagraphScaleProfile {
    #[inline]
    pub fn classify(
        ends_with_explicit_linebreak: bool,
        base_font_pt: f64,
        default_font_pt: f64,
    ) -> Self {
        match (
            ends_with_explicit_linebreak,
            base_font_pt / default_font_pt.max(1.0),
        ) {
            (true, _) => Self::ExplicitLinebreakTerminated,
            (false, ratio) if ratio > 1.4 => Self::DisplayTitle,
            (false, ratio) if ratio >= 1.2 => Self::SubHeading,
            (false, _) => Self::BodyText,
        }
    }
}

/// Classification of the successor block $B$ following paragraph $A$ on the same page/column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NextBlockSpacingContext {
    /// Followed by a tight `List` or `Enum`.
    TightList,
    /// Followed by another `Paragraph`.
    Paragraph,
    /// Followed by any other block type.
    Other,
}

impl NextBlockSpacingContext {
    #[inline]
    pub const fn from_flags(next_is_tight_list: bool, next_is_paragraph: bool) -> Self {
        match (next_is_tight_list, next_is_paragraph) {
            (true, _) => Self::TightList,
            (false, true) => Self::Paragraph,
            (false, false) => Self::Other,
        }
    }
}

/// Complete spatial construct for a Word `<w:pPr>` paragraph box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WordParagraphConstruct {
    pub jc: WordJc,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_box: WordLineBox,
    pub left_dxa: i64,
    pub right_dxa: i64,
    pub first_line_dxa: i64,
    pub hanging_dxa: i64,
    pub right_tab_stop_dxa: Option<i64>,
}

impl WordParagraphConstruct {
    /// Declaratively solve a paragraph's initial `space_after_pt` from Typst's `par.spacing`,
    /// `par.leading`, and [`ParagraphScaleProfile`].
    pub fn solve_initial_space_after_pt(
        ends_with_explicit_linebreak: bool,
        base_font_pt: f64,
        default_font_pt: f64,
        spacing_pt: f64,
        leading_pt: f64,
    ) -> f64 {
        match ParagraphScaleProfile::classify(
            ends_with_explicit_linebreak,
            base_font_pt,
            default_font_pt,
        ) {
            ParagraphScaleProfile::ExplicitLinebreakTerminated => 0.0,
            // Large display-sized paragraphs (e.g. 1.8em = 19.8pt title lines):
            // Word's 1.0x (`w:line="240"`) line box has built-in bottom leading of `~0.28 * S`,
            // while Typst's inter-paragraph gap is `1.2 * S_default` + display ascent difference.
            ParagraphScaleProfile::DisplayTitle => spacing_pt + 2.0,
            ParagraphScaleProfile::SubHeading => (spacing_pt - 3.0).max(2.0),
            ParagraphScaleProfile::BodyText => (spacing_pt - leading_pt).max(1.5),
        }
    }

    /// Declaratively solve the required `space_after_pt` on block $A$ when followed on the same
    /// page/column by block $B$ (`next_context`) with measured Typst box gap `gap_pt = top_b - bottom_a`.
    pub fn solve_inter_block_gap_for_context(
        gap_pt: f64,
        font_size_a_pt: f64,
        font_size_b_pt: f64,
        next_context: NextBlockSpacingContext,
    ) -> InterBlockGapAction {
        match (
            next_context,
            font_size_a_pt >= 15.0,
            font_size_b_pt >= 15.0,
            gap_pt,
        ) {
            (NextBlockSpacingContext::TightList, _, _, g) if g <= 10.0 => {
                InterBlockGapAction::ZeroSpaceAfter
            }
            (NextBlockSpacingContext::Paragraph, a_large, b_large, g)
                if (a_large || b_large) && g > 10.0 && g < 60.0 =>
            {
                // Causal baseline-to-baseline equation between paragraph A (size S_A) and paragraph B (size S_B):
                //   y_B - y_A = (0.320 + 1.380 * (M_A - 1)) * S_A + A_i + 1.060 * S_B
                //             = 0.200 * S_A + gap_pt + 0.800 * S_B
                // => A_i = gap_pt - (0.120 + 1.380 * (M_A - 1)) * S_A - 0.260 * S_B
                let m_a = match a_large {
                    true => 1.0,
                    false => 285.0 / 240.0,
                };
                let below_a_pt = match (a_large, b_large) {
                    (true, false) => 0.0,
                    _ => (0.120 + 1.380 * (m_a - 1.0)) * font_size_a_pt,
                };
                let above_b_pt = 0.260 * font_size_b_pt;
                let target_after_pt = (g - below_a_pt - above_b_pt).max(0.0);
                InterBlockGapAction::SetExactSpaceAfter(target_after_pt)
            }
            (_, _, _, g) if g > 24.0 && g < 500.0 => {
                InterBlockGapAction::AddExtraSpaceAfter((g - 12.0).max(0.0))
            }
            _ => InterBlockGapAction::KeepDefault,
        }
    }

    /// Legacy boolean-flag entry point delegating to [`Self::solve_inter_block_gap_for_context`].
    #[inline]
    pub fn solve_inter_block_gap_adjustment(
        gap_pt: f64,
        font_size_a_pt: f64,
        font_size_b_pt: f64,
        next_is_tight_list: bool,
        next_is_paragraph: bool,
    ) -> InterBlockGapAction {
        Self::solve_inter_block_gap_for_context(
            gap_pt,
            font_size_a_pt,
            font_size_b_pt,
            NextBlockSpacingContext::from_flags(next_is_tight_list, next_is_paragraph),
        )
    }
}

/// Action returned by the inter-block vertical gap solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterBlockGapAction {
    /// Keep the block's default `space_after_pt`.
    KeepDefault,
    /// Collapse `space_after_pt` to `0.0` (e.g. before a tight list).
    ZeroSpaceAfter,
    /// Set `space_after_pt` to an exact solved value in `pt`.
    SetExactSpaceAfter(f64),
    /// Add extra `#v(...)` vertical whitespace in `pt` to `space_after_pt`.
    AddExtraSpaceAfter(f64),
}
