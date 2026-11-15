//! Word List, Enum & Terms Construct (`src/construct/list.rs`).
//!
//! Unifies `BlockNode::List` (bullet lists) and `BlockNode::Enum` (numbered lists) via
//! exhaustive enums (`ListSequenceSource`, `ListMarkerConstruct`) and solves per-item
//! indentation, hanging markers, and vertical spacing via [`crate::WordListItemLayout`].

use crate::{EnumMarkerWidthKind, FollowingBlockKind, ListItemLayoutInput, WordListItemLayout};

use ir::{BlockNode, EnumItemIr, ListItemIr, TableIr, TableKindIr};
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
    pub marker_width: EnumMarkerWidthKind,
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
    /// Solve [`WordListItemLayout`] for this item's block slice.
    pub fn solve_item_layout(&self) -> WordListItemLayout {
        let blocks = self.blocks;
        let (first_para_space_after_pt, first_para_line_spacing_pt) = match blocks.first() {
            Some(BlockNode::Paragraph { style, .. }) => {
                (Some(style.space_after_pt), style.line_spacing_pt)
            }
            _ => (None, None),
        };
        let following_block = match blocks.get(1) {
            None => FollowingBlockKind::None,
            Some(BlockNode::Table(t)) if t.kind != TableKindIr::SemanticTable => {
                FollowingBlockKind::GridTable
            }
            Some(BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. }) => {
                FollowingBlockKind::TightList
            }
            Some(BlockNode::List { tight: false, .. } | BlockNode::Enum { tight: false, .. }) => {
                FollowingBlockKind::LooseList
            }
            Some(BlockNode::Paragraph { .. }) => FollowingBlockKind::Paragraph,
            Some(BlockNode::CodeBlock { .. }) => FollowingBlockKind::CodeBlock,
            Some(_) => FollowingBlockKind::Other,
        };
        WordListItemLayout::solve(ListItemLayoutInput {
            ilvl: self.ilvl,
            tight: self.tight,
            indent_pt: self.indent_pt,
            hanging_pt: self.hanging_pt,
            marker_width: self.marker_width,
            item_gap_dxa: self.item_gap_dxa,
            item_end_after_dxa: self.item_end_after_dxa,
            before_dxa: self.before_dxa,
            num_blocks: blocks.len(),
            first_para_space_after_pt,
            first_para_line_spacing_pt,
            following_block,
        })
    }

    /// Solve extra bottom inset (`pt`) for a child `TableIr` at `idx` inside this item so that
    /// subsequent blocks in the same item (or after the final top-level item) honor `par.spacing`
    /// after accounting for `WordCellMarginsDxa`'s internal-leading trim.
    pub fn solve_child_table_bottom_pad_pt(&self, idx: usize, tbl: &TableIr) -> f64 {
        let is_last_block = idx + 1 == self.blocks.len();
        if is_last_block {
            if self.item_gap_dxa > 0 && self.is_last_top {
                4.5
            } else {
                0.0
            }
        } else if tbl.kind != TableKindIr::SemanticTable {
            let item_gap_pt = (self.item_gap_dxa as f64 / 20.0).max(0.0);
            let par_leading_pt = self
                .blocks
                .first()
                .and_then(|b| match b {
                    BlockNode::Paragraph { style, .. } => {
                        style.resolved_par_leading_pt.or(style.line_spacing_pt)
                    }
                    _ => None,
                })
                .unwrap_or(7.15);
            match self.blocks.get(idx + 1) {
                // Text paragraphs / lists contribute their own top line-box leading above cap-height.
                Some(
                    BlockNode::Paragraph { .. }
                    | BlockNode::List { .. }
                    | BlockNode::Enum { .. },
                ) => item_gap_pt + 0.5 * par_leading_pt,
                // Solid block boxes (`ShapeBlock`, `CodeBlock`, `Figure`, `Table`) have zero top
                // internal leading, so pass the full `par.spacing = item_gap + par_leading`.
                _ => item_gap_pt + par_leading_pt,
            }
        } else {
            0.0
        }
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
                    let item_end_after_dxa = resolve_item_end_after_dxa(
                        is_last_in_list,
                        is_last_top,
                        item_gap_dxa,
                        trailing_after_dxa,
                    );
                    let prev_table_extra_dxa = if idx > 0 {
                        trailing_table_descent_deficit_dxa(items[idx - 1].blocks.last())
                    } else {
                        None
                    };
                    let before_dxa = if let Some(extra_dxa) = prev_table_extra_dxa
                        && item_gap_dxa > 0
                    {
                        item_gap_dxa + extra_dxa
                    } else if idx == 0 && level == 0 && trailing_after_dxa.is_none() {
                        104
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
                        marker_width: EnumMarkerWidthKind::Standard,
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
            start_val: _,
        } => {
            let num_items = items.len();
            items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let is_last_in_list = idx + 1 == num_items;
                    let is_last_top = level == 0 && is_last_in_list;
                    let item_end_after_dxa = resolve_item_end_after_dxa(
                        is_last_in_list,
                        is_last_top,
                        item_gap_dxa,
                        trailing_after_dxa,
                    );
                    let prev_table_extra_dxa = if idx > 0 {
                        trailing_table_descent_deficit_dxa(items[idx - 1].blocks.last())
                    } else {
                        None
                    };
                    // Keep `before_dxa = 0` at `idx == 0` regardless of `start_val` so preceding
                    // headings, paragraphs, and figures control inter-block spacing cleanly
                    // (post-table gaps are supplied via `solved_block_before_dxa`).
                    let before_dxa = if let Some(extra_dxa) = prev_table_extra_dxa
                        && item_gap_dxa > 0
                    {
                        item_gap_dxa + extra_dxa
                    } else {
                        0
                    };
                    let is_std_decimal = is_standard_decimal_marker(&item.marker_text);
                    let marker = if is_std_decimal {
                        ListMarkerConstruct::NumberedLevel(ilvl)
                    } else {
                        ListMarkerConstruct::CustomPrefix(item.marker_text.as_str())
                    };
                    let marker_width = if is_std_decimal && item.marker_text.trim().len() == 2 {
                        EnumMarkerWidthKind::SingleDigitDecimal
                    } else {
                        EnumMarkerWidthKind::Standard
                    };
                    ListItemConstruct {
                        num_id,
                        ilvl,
                        marker,
                        marker_width,
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

fn trailing_table_descent_deficit_dxa(last_block: Option<&BlockNode>) -> Option<i64> {
    match last_block {
        Some(BlockNode::Table(tbl)) => match tbl.kind {
            TableKindIr::SemanticTable => Some(64),
            // A compact `LayoutGrid` / `StyledContainer` trailing row (`w:line="164"`) has ~0.9pt
            // (`18 dxa`) less bottom line-box descent than a body paragraph (`w:line="199"`).
            TableKindIr::LayoutGrid | TableKindIr::StyledContainer => Some(18),
        },
        _ => None,
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
