//! Word CeTZ Canvas Construct (`<wpg:wgp>` wrapper paragraph).

use ir::{CetzCanvasIr, HAlignIr};

/// Solved Word paragraph wrapper construct for `BlockNode::CetzCanvas`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CetzCanvasConstruct {
    pub align: HAlignIr,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
    pub mark_sz_half_pt: u32,
}

impl CetzCanvasConstruct {
    pub fn solve(canvas: &CetzCanvasIr) -> Self {
        Self {
            align: canvas.align,
            before_dxa: 35,
            after_dxa: 295,
            line_dxa: 240,
            mark_sz_half_pt: 4,
        }
    }
}
