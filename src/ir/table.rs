//! Tables and grids (`table` / `grid`).

use serde::{Deserialize, Serialize};

use super::{BlockNode, BordersIr, ColorHex, HAlignIr, SidesPtIr, VAlignIr};

/// Table or Grid intermediate representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIr {
    pub is_semantic_table: bool,
    pub col_widths_pt: Vec<f64>,
    pub row_heights_pt: Vec<f64>,
    pub header_rows: usize,
    pub rows: Vec<TableRowIr>,
    pub align: HAlignIr,
    #[serde(default)]
    pub left_indent_pt: f64,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRowIr {
    pub height_pt: Option<f64>,
    pub is_header: bool,
    pub cells: Vec<TableCellIr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCellIr {
    pub x: usize,
    pub y: usize,
    pub colspan: usize,
    pub rowspan: usize,
    /// When `vmerge` is `Some(VMergeState::Restart)`, this cell starts a vertical span;
    /// when `Some(VMergeState::Continue)`, this cell is a continuation placeholder in Word.
    pub vmerge: Option<VMergeState>,
    /// True if this slot is covered horizontally by a preceding cell's `colspan` in the same row.
    pub hmerged_covered: bool,
    pub fill: Option<ColorHex>,
    pub borders: BordersIr,
    pub inset: SidesPtIr,
    pub halign: HAlignIr,
    pub valign: VAlignIr,
    pub is_header_cell: bool,
    pub blocks: Vec<BlockNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VMergeState {
    Restart,
    Continue,
}
