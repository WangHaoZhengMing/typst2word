//! Word List, Enum & Terms Construct (`src/construct/list.rs`).
//!
//! Unifies `BlockNode::List` (bullet lists) and `BlockNode::Enum` (numbered lists) via
//! exhaustive enums (`ListSequenceSource`, `ListMarkerConstruct`) and solves per-item
//! indentation, hanging markers, and vertical spacing via [`word_construct::WordListItemLayout`].

use crate::ir::{BlockNode, EnumItemIr, ListItemIr};
use crate::units::pt_to_dxa;

/// Type-safe representation of a list/enum item's marker style in Word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMarkerConstruct<'a> {
    /// Standard Word `<w:numPr>` at a specific `ilvl`.
    NumberedLevel(usize),
    /// Literal marker text prefix (e.g. custom bullet or `(1)` enum marker).
    CustomPrefix(&'a str),
}

/// Exhaustive source sequence kind unifying `BlockNode::List` and `BlockNode::Enum`.
#[derive(Debug, Clone, Copy)]
pub enum ListSequenceSource<'a> {
    Bullet {
        items: &'a [ListItemIr],
    },
    DecimalEnum {
        items: &'a [EnumItemIr],
        num_id: usize,
        start_val: u64,
    },
}

/// Solved layout and numbering context for rendering a single list or enum item.
#[derive(Debug, Clone, Copy)]
pub struct ListItemConstruct<'a> {
    pub num_id: usize,
    pub ilvl: usize,
    pub marker: ListMarkerConstruct<'a>,
    pub is_single_digit_enum_item: bool,
    pub tight: bool,
    pub item_gap_dxa: i64,
    pub item_end_after_dxa: i64,
    pub before_dxa: i64,
    pub indent_pt: f64,
    pub hanging_pt: f64,
    pub is_last_top: bool,
    pub line_dxa_override: Option<i64>,
    pub blocks: &'a [BlockNode],
}

impl<'a> ListItemConstruct<'a> {
    /// Solve [`word_construct::WordListItemLayout`] for this item's block slice.
    pub fn solve_item_layout(&self) -> word_construct::WordListItemLayout {
        let blocks = self.blocks;
        let followed_by_grid_table = matches!(
            blocks.get(1),
            Some(BlockNode::Table(t)) if !t.is_semantic()
        );
        let followed_by_tight_list = matches!(
            blocks.get(1),
            Some(BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. })
        );
        let followed_by_paragraph = matches!(blocks.get(1), Some(BlockNode::Paragraph { .. }));
        let (first_para_space_after_pt, first_para_line_spacing_pt) = match blocks.first() {
            Some(BlockNode::Paragraph { style, .. }) => {
                (Some(style.space_after_pt), style.line_spacing_pt)
            }
            _ => (None, None),
        };
        let following_block = word_construct::FollowingBlockKind::from_flags(
            blocks.len(),
            followed_by_grid_table,
            followed_by_tight_list,
            followed_by_paragraph,
        );
        word_construct::WordListItemLayout::solve_from_input(word_construct::ListItemLayoutInput {
            ilvl: self.ilvl,
            tight: self.tight,
            indent_pt: self.indent_pt,
            hanging_pt: self.hanging_pt,
            marker_width: word_construct::EnumMarkerWidthKind::from_bool(
                self.is_single_digit_enum_item,
            ),
            item_gap_dxa: self.item_gap_dxa,
            item_end_after_dxa: self.item_end_after_dxa,
            before_dxa: self.before_dxa,
            num_blocks: blocks.len(),
            first_para_space_after_pt,
            first_para_line_spacing_pt,
            following_block,
        })
    }
}

/// Unified solver for both `BlockNode::List` and `BlockNode::Enum` item sequences.
#[allow(clippy::too_many_arguments)]
pub fn solve_list_sequence_construct<'a>(
    source: ListSequenceSource<'a>,
    tight: bool,
    level: usize,
    spacing_pt: Option<f64>,
    indent_pt: f64,
    hanging_pt: f64,
    trailing_after_dxa: Option<i64>,
    line_dxa_override: Option<i64>,
) -> Vec<ListItemConstruct<'a>> {
    let ilvl = level.min(8);
    let item_gap_dxa = compute_list_item_gap_dxa(tight, spacing_pt);

    match source {
        ListSequenceSource::Bullet { items } => {
            let num_items = items.len();
            items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let is_last_in_list = idx + 1 == num_items;
                    let is_last_top = level == 0 && is_last_in_list;
                    let item_end_after_dxa =
                        resolve_item_end_after_dxa(is_last_in_list, is_last_top, item_gap_dxa, trailing_after_dxa);
                    let prev_ended_with_table =
                        idx > 0 && matches!(items[idx - 1].blocks.last(), Some(BlockNode::Table(_)));
                    let before_dxa = if prev_ended_with_table && item_gap_dxa > 0 {
                        item_gap_dxa + 64
                    } else {
                        0
                    };
                    let marker = match item.marker.as_str() {
                        "•" => ListMarkerConstruct::NumberedLevel(0),
                        "‣" => ListMarkerConstruct::NumberedLevel(1),
                        "–" => ListMarkerConstruct::NumberedLevel(2),
                        _ => ListMarkerConstruct::NumberedLevel(ilvl),
                    };
                    ListItemConstruct {
                        num_id: 1,
                        ilvl,
                        marker,
                        is_single_digit_enum_item: false,
                        tight,
                        item_gap_dxa,
                        item_end_after_dxa,
                        before_dxa,
                        indent_pt,
                        hanging_pt,
                        is_last_top,
                        line_dxa_override,
                        blocks: &item.blocks,
                    }
                })
                .collect()
        }
        ListSequenceSource::DecimalEnum {
            items,
            num_id,
            start_val,
        } => {
            let num_items = items.len();
            items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let is_last_in_list = idx + 1 == num_items;
                    let is_last_top = level == 0 && is_last_in_list;
                    let item_end_after_dxa =
                        resolve_item_end_after_dxa(is_last_in_list, is_last_top, item_gap_dxa, trailing_after_dxa);
                    let prev_ended_with_table =
                        idx > 0 && matches!(items[idx - 1].blocks.last(), Some(BlockNode::Table(_)));
                    let before_dxa = if prev_ended_with_table && item_gap_dxa > 0 {
                        item_gap_dxa + 64
                    } else if idx == 0 && start_val > 1 {
                        item_gap_dxa.max(180)
                    } else {
                        0
                    };
                    let marker = if is_standard_decimal_marker(&item.marker_text) {
                        ListMarkerConstruct::NumberedLevel(ilvl)
                    } else {
                        ListMarkerConstruct::CustomPrefix(item.marker_text.as_str())
                    };
                    let is_single_digit_enum_item = item.marker_text.trim().len() == 2
                        && is_standard_decimal_marker(&item.marker_text);
                    ListItemConstruct {
                        num_id,
                        ilvl,
                        marker,
                        is_single_digit_enum_item,
                        tight,
                        item_gap_dxa,
                        item_end_after_dxa,
                        before_dxa,
                        indent_pt,
                        hanging_pt,
                        is_last_top,
                        line_dxa_override,
                        blocks: &item.blocks,
                    }
                })
                .collect()
        }
    }
}

fn resolve_item_end_after_dxa(
    is_last_in_list: bool,
    is_last_top: bool,
    item_gap_dxa: i64,
    trailing_after_dxa: Option<i64>,
) -> i64 {
    if is_last_in_list {
        trailing_after_dxa.unwrap_or(if is_last_top {
            item_gap_dxa.max(40)
        } else {
            item_gap_dxa
        })
    } else {
        item_gap_dxa
    }
}

pub fn compute_list_item_gap_dxa(tight: bool, spacing_pt: Option<f64>) -> i64 {
    if let Some(sp_pt) = spacing_pt {
        pt_to_dxa(sp_pt).max(0)
    } else if tight {
        0
    } else {
        90
    }
}

pub fn is_standard_decimal_marker(marker: &str) -> bool {
    let trimmed = marker.trim();
    trimmed.ends_with('.')
        && trimmed[..trimmed.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit())
}
