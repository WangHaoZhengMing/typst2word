//! Tables and grids (`table` / `grid` / lowered container tables).

use serde::{Deserialize, Serialize};

use super::{BlockNode, BordersIr, ColorHex, HAlignIr, NodeId, SidesPtIr, VAlignIr};

/// Exhaustive semantic classification of a `TableIr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TableKindIr {
    /// Explicit `#table(...)` with semantic borders and cell insets.
    #[default]
    SemanticTable,
    /// Borderless layout `#grid(...)`.
    LayoutGrid,
    /// Breakable 1x1 container table lowered from `BlockNode::Container` in `word-construct`.
    StyledContainer,
}

/// Table or Grid intermediate representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIr {
    #[serde(default, skip_serializing_if = "NodeId::is_unset")]
    pub id: NodeId,
    #[serde(default)]
    pub kind: TableKindIr,
    pub col_widths_pt: Vec<f64>,
    pub row_heights_pt: Vec<f64>,
    pub header_rows: usize,
    #[serde(default)]
    pub footer_rows: usize,
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
    #[serde(default)]
    pub is_footer: bool,
    pub cells: Vec<TableCellIr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCellIr {
    pub x: usize,
    pub y: usize,
    pub colspan: usize,
    pub rowspan: usize,
    pub fill: Option<ColorHex>,
    pub borders: BordersIr,
    pub inset: SidesPtIr,
    pub halign: HAlignIr,
    pub valign: VAlignIr,
    pub is_header_cell: bool,
    pub blocks: Vec<BlockNode>,
}
