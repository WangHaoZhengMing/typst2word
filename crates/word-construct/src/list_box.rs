//! Microsoft Word List & Numbering Construct (`WordListConstruct`).
//!
//! Models `<w:abstractNum>`, `<w:lvl>`, `<w:lvlJc>`, `<w:ind w:left w:hanging/>`,
//! `<w:tab w:val="num"/>`, and per-item paragraph spacing for bullet (`List`) and
//! numbered (`Enum`) lists.
//!
//! ## Key Spatial Invariants
//! 1. **Single-Paragraph vs Multi-Paragraph Item Spacing**:
//!    In Word, every list item is a sequence of `<w:p>` elements where the first `<w:p>`
//!    carries `<w:numPr>`. When an item has only a single `<w:p>` (`blocks.len() == 1`),
//!    that `<w:p>` is BOTH the first block and the last block of the item, so its `w:after`
//!    MUST honor `max(item_end_after_dxa, pt_to_dxa(style.space_after_pt))` — preserving
//!    any explicit `#v(...)` answer space after the item!
//! 2. **Right-Aligned Enum Numbers via Per-Item `w:hanging`**:
//!    In Typst, `enum` item numbers (`1.`..`15.`) are right-aligned within the widest
//!    marker slot `max_label_w_pt`, followed by `body_indent_pt` before the item text.
//!    In Word, `<w:ind w:left="L" w:hanging="H"/>` places the body text (and wrapped lines)
//!    at `L` and the marker's left edge at `L - H`. Therefore, in a multi-digit list
//!    (`max_hanging_dxa >= 360`), keeping `L` fixed at `outer_indent + max_hanging_dxa` while
//!    setting `H = max_hanging_dxa - 88` (`~4.4pt` = 1 digit width) on single-digit items
//!    (`1.`..`9.`) right-aligns `1.`..`9.` with `10.`..`15.` while keeping all body text
//!    strictly aligned at `L`!

use serde::{Deserialize, Serialize};

use crate::units::pt_to_dxa;

/// Semantic classification of the second block (`blocks[1]`) inside a list/enum item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FollowingBlockKind {
    /// Item has only a single block (`blocks.len() <= 1`).
    #[default]
    None,
    /// Followed by a non-semantic layout grid table (`BlockNode::Table` with `!is_semantic_table`).
    GridTable,
    /// Followed by a tight child `List` or `Enum`.
    TightList,
    /// Followed by a continuation `Paragraph`.
    Paragraph,
    /// Followed by any other block (`CodeBlock`, `Figure`, `EquationBlock`, semantic `Table`, etc.).
    Other,
}

impl FollowingBlockKind {
    /// Classify the following block from legacy boolean flags.
    #[inline]
    pub const fn from_flags(
        num_blocks: usize,
        followed_by_grid_table: bool,
        followed_by_tight_list: bool,
        followed_by_paragraph: bool,
    ) -> Self {
        match (
            followed_by_grid_table,
            num_blocks > 1,
            followed_by_tight_list,
            followed_by_paragraph,
        ) {
            (true, _, _, _) => Self::GridTable,
            (false, false, _, _) => Self::None,
            (false, true, true, _) => Self::TightList,
            (false, true, false, true) => Self::Paragraph,
            (false, true, false, false) => Self::Other,
        }
    }
}

/// Width classification of an `enum` or `list` item's marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EnumMarkerWidthKind {
    /// Standard bullet marker or multi-digit enum marker (`10.`+).
    #[default]
    Standard,
    /// Single-digit decimal enum marker (`1.`..`9.`) that right-aligns inside a multi-digit list (`w:hanging - 88 dxa`).
    SingleDigitDecimal,
}

impl EnumMarkerWidthKind {
    #[inline]
    pub const fn from_bool(is_single_digit_enum_item: bool) -> Self {
        match is_single_digit_enum_item {
            true => Self::SingleDigitDecimal,
            false => Self::Standard,
        }
    }
}

/// Structured input for [`WordListItemLayout::solve_from_input`], replacing positional boolean flags
/// with self-documenting domain enums.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListItemLayoutInput {
    pub ilvl: usize,
    pub tight: bool,
    pub indent_pt: f64,
    pub hanging_pt: f64,
    pub marker_width: EnumMarkerWidthKind,
    pub item_gap_dxa: i64,
    pub item_end_after_dxa: i64,
    pub before_dxa: i64,
    pub num_blocks: usize,
    pub first_para_space_after_pt: Option<f64>,
    pub first_para_line_spacing_pt: Option<f64>,
    pub following_block: FollowingBlockKind,
}

/// Spatial parameters solved for a single list or enum item in Word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordListItemLayout {
    /// Total left indent of the item's body text (`w:ind w:left` and `<w:tab w:val="num" w:pos/>`) in `dxa`.
    pub indent_left_dxa: usize,
    /// Hanging indent (`w:ind w:hanging`) for this item's marker in `dxa`.
    pub hanging_dxa: i64,
    /// `w:before` on the first paragraph of the item in `dxa`.
    pub first_before_dxa: i64,
    /// `w:after` on the first paragraph of the item in `dxa`.
    pub first_after_dxa: i64,
}

impl WordListItemLayout {
    /// Declaratively solve Word indentation (`w:left`, `w:hanging`) and first-paragraph
    /// `w:after` from a structured [`ListItemLayoutInput`].
    pub fn solve_from_input(input: ListItemLayoutInput) -> Self {
        let first_line_gap_dxa = match (
            input.first_para_line_spacing_pt,
            input.first_para_space_after_pt,
        ) {
            (Some(ls), _) => pt_to_dxa(ls.max(4.0)),
            (None, Some(sa)) => pt_to_dxa(sa.max(4.0)),
            (None, None) => 156,
        };

        let first_after_dxa = match (input.following_block, input.num_blocks > 1, input.tight) {
            (FollowingBlockKind::GridTable, _, _) => first_line_gap_dxa,
            (FollowingBlockKind::TightList, true, _) | (_, true, true) => 0,
            (FollowingBlockKind::Paragraph, true, false) => input.item_gap_dxa.max(132),
            (_, true, false) => 10,
            (_, false, _) => {
                // Single-block item: this first paragraph is ALSO the last block of the item!
                // If it carries an explicit `space_after_pt` (e.g. from `#v(3cm)` / `#v(4cm)`),
                // preserve it so Word renders the vertical answer whitespace.
                let explicit_after_dxa = input
                    .first_para_space_after_pt
                    .filter(|&sa| sa > 12.0)
                    .map(pt_to_dxa)
                    .unwrap_or(0);
                input.item_end_after_dxa.max(explicit_after_dxa)
            }
        };

        let max_hanging_dxa = match input.hanging_pt {
            h if h > 1.0 => pt_to_dxa(h).max(180),
            _ => 380,
        };
        let outer_indent_dxa = pt_to_dxa(input.indent_pt).max(0);
        let indent_left_dxa = match (input.indent_pt > 0.1, input.ilvl) {
            (true, _) | (false, 0) => (outer_indent_dxa + max_hanging_dxa).max(0) as usize,
            (false, ilvl) => {
                (outer_indent_dxa + max_hanging_dxa * (ilvl as i64 + 1)).max(0) as usize
            }
        };

        // Right-align 1-digit enum numbers (`1.`..`9.`) inside a multi-digit enum (`10.`+)
        // by reducing `w:hanging` by 1 digit width (`88 dxa = 4.4pt`) while keeping `w:left`
        // fixed at `indent_left_dxa`.
        let hanging_dxa = match (input.marker_width, max_hanging_dxa) {
            (EnumMarkerWidthKind::SingleDigitDecimal, h) if h >= 360 => (h - 88).max(280),
            (_, h) => h,
        };

        Self {
            indent_left_dxa,
            hanging_dxa,
            first_before_dxa: input.before_dxa,
            first_after_dxa,
        }
    }

    /// Legacy positional entry point delegating to [`Self::solve_from_input`].
    #[allow(clippy::too_many_arguments)]
    pub fn solve(
        ilvl: usize,
        tight: bool,
        indent_pt: f64,
        hanging_pt: f64,
        is_single_digit_enum_item: bool,
        item_gap_dxa: i64,
        item_end_after_dxa: i64,
        before_dxa: i64,
        num_blocks: usize,
        first_para_space_after_pt: Option<f64>,
        first_para_line_spacing_pt: Option<f64>,
        followed_by_grid_table: bool,
        followed_by_tight_list: bool,
        followed_by_paragraph: bool,
    ) -> Self {
        Self::solve_from_input(ListItemLayoutInput {
            ilvl,
            tight,
            indent_pt,
            hanging_pt,
            marker_width: EnumMarkerWidthKind::from_bool(is_single_digit_enum_item),
            item_gap_dxa,
            item_end_after_dxa,
            before_dxa,
            num_blocks,
            first_para_space_after_pt,
            first_para_line_spacing_pt,
            following_block: FollowingBlockKind::from_flags(
                num_blocks,
                followed_by_grid_table,
                followed_by_tight_list,
                followed_by_paragraph,
            ),
        })
    }
}
