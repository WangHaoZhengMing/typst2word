//! Word Native Shape Construct (`<wps:wsp>` wrapper paragraph).

use ir::{HAlignIr, ShapeIr};

/// Solved Word paragraph wrapper construct for `BlockNode::ShapeBlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeBlockConstruct {
    pub align: HAlignIr,
    pub left_indent_dxa: usize,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
}

impl ShapeBlockConstruct {
    pub fn solve(shape: &ShapeIr, left_indent_dxa: usize) -> Self {
        Self::solve_with_collapse(shape, left_indent_dxa, false)
    }

    pub fn solve_with_collapse(
        shape: &ShapeIr,
        left_indent_dxa: usize,
        collapse_before: bool,
    ) -> Self {
        let (raw_before, after_dxa) = if shape.width_pt >= 350.0 {
            (0, 100)
        } else {
            (70, 210)
        };
        let before_dxa = if collapse_before { 0 } else { raw_before };
        Self {
            align: shape.align,
            left_indent_dxa,
            before_dxa,
            after_dxa,
            line_dxa: 240,
        }
    }
}
