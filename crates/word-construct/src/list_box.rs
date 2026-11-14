//! Microsoft Word List & Numbering Construct (`WordListItemLayout`).
//!
//! Models `<w:abstractNum>`, `<w:lvl>`, `<w:lvlJc>`, `<w:ind w:left w:hanging/>`,
//! `<w:tab w:val="num"/>`, and per-item paragraph spacing for bullet (`List`) and
//! numbered (`Enum`) lists.

use serde::{Deserialize, Serialize};

use crate::units::pt_to_dxa;

/// Semantic classification of the second block (`blocks[1]`) inside a list/enum item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FollowingBlockKind {
    /// Item has only a single block (`blocks.len() <= 1`).
    #[default]
    None,
    /// Followed by a non-semantic layout grid or styled container table.
    GridTable,
    /// Followed by a tight child `List` or `Enum`.
    TightList,
    /// Followed by a continuation `Paragraph`.
    Paragraph,
    /// Followed by any other block (`CodeBlock`, `Figure`, `EquationBlock`, semantic `Table`, etc.).
    Other,
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

/// Structured input for [`WordListItemLayout::solve`].
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
    pub fn solve(input: ListItemLayoutInput) -> Self {
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
            // Respect the list/enum's actual `item_gap_dxa` (derived from `par.spacing - par.leading`)
            // without forcing an artificial `132 dxa` (`6.6pt`) floor on tighter custom `par(spacing: ...)`.
            (FollowingBlockKind::Paragraph, true, false) => input.item_gap_dxa,
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
}
