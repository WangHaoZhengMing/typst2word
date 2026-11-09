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
        Self {
            align: shape.align,
            left_indent_dxa,
            before_dxa: 20,
            after_dxa: 80,
            line_dxa: 240,
        }
    }
}
