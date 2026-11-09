//! Tables and grids (`table` / `grid` / styled container blocks).

use serde::{Deserialize, Serialize};

use super::{BlockNode, BordersIr, ColorHex, HAlignIr, SidesPtIr, VAlignIr};

/// High-level semantic classification of a `TableIr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TableKindIr {
    /// Explicit `#table(...)` with semantic borders and cell insets.
    #[default]
    SemanticTable,
    /// Borderless layout `#grid(...)`.
    LayoutGrid,
    /// Bordered/filled `#block(...)` container modeled as a breakable 1x1 table.
    StyledContainer,
}

impl TableKindIr {
    #[inline]
    pub const fn is_semantic(self) -> bool {
        matches!(self, Self::SemanticTable)
    }

    #[inline]
    pub const fn is_layout_grid(self) -> bool {
        matches!(self, Self::LayoutGrid)
    }

    #[inline]
    pub const fn is_styled_container(self) -> bool {
        matches!(self, Self::StyledContainer)
    }
}

/// Table or Grid intermediate representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIr {
    #[serde(default)]
    pub kind: TableKindIr,
    pub col_widths_pt: Vec<f64>,
    pub row_heights_pt: Vec<f64>,
    pub header_rows: usize,
    pub rows: Vec<TableRowIr>,
    pub align: HAlignIr,
    #[serde(default)]
    pub left_indent_pt: f64,
    pub summary: Option<String>,
}

impl TableIr {
    #[inline]
    pub const fn is_semantic(&self) -> bool {
        self.kind.is_semantic()
    }

    #[inline]
    pub const fn is_layout_grid(&self) -> bool {
        self.kind.is_layout_grid()
    }

    #[inline]
    pub const fn is_styled_container(&self) -> bool {
        self.kind.is_styled_container()
    }
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
