//! Word Figure & Caption Construct (`src/construct/figure.rs`).
//!
//! Solves figure body indentation, indented figure table width adjustments, and
//! `Caption` paragraph spacing / `keep_with_next` propagation.

use crate::ir::{BlockNode, CaptionPositionIr, HAlignIr, TableIr};
use crate::units::pt_to_dxa;

/// Solved Word construct for a `BlockNode::Figure` and its `Caption` paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FigureConstruct {
    pub caption_pos: CaptionPositionIr,
    pub eff_indent_dxa: usize,
    pub caption_before_dxa: i64,
    pub caption_after_dxa: i64,
    pub caption_keep_with_next: bool,
}

impl FigureConstruct {
    pub fn solve(
        body: &[BlockNode],
        caption_pos: CaptionPositionIr,
        indent_left_dxa: usize,
        caption_after_dxa: i64,
        caption_keep_with_next: bool,
    ) -> Self {
        let body_indent_dxa = body
            .iter()
            .find_map(|b| match b {
                BlockNode::Table(t) if t.left_indent_pt > 0.5 => {
                    Some(pt_to_dxa(t.left_indent_pt).max(0) as usize)
                }
                _ => None,
            })
            .unwrap_or(0);
        let eff_indent_dxa = indent_left_dxa.max(body_indent_dxa);
        Self {
            caption_pos,
            eff_indent_dxa,
            caption_before_dxa: 20,
            caption_after_dxa,
            caption_keep_with_next,
        }
    }
}

/// Adjust a `TableIr` inside an indented figure body so its columns fit within the available width.
pub fn solve_indented_figure_table(
    tbl: &TableIr,
    indent_left_dxa: usize,
    content_width_dxa: i64,
) -> TableIr {
    let target_left_pt = indent_left_dxa as f64 / 20.0;
    let avail_w_pt = ((content_width_dxa as f64 / 20.0) - target_left_pt).max(80.0);
    let mut adjusted = tbl.clone();
    adjusted.align = HAlignIr::Left;
    let mut total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
    if total_col_w > avail_w_pt + 2.0 {
        let excess = total_col_w - avail_w_pt;
        if let Some(last_w) = adjusted.col_widths_pt.last_mut()
            && *last_w > excess + 40.0
        {
            *last_w -= excess;
            total_col_w = avail_w_pt;
        }
    }
    if adjusted.left_indent_pt <= 0.5 {
        adjusted.left_indent_pt = target_left_pt + ((avail_w_pt - total_col_w).max(0.0) * 0.5);
    }
    adjusted
}
