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

/// Exhaustive Typst column track sizing paired with its resolved width (`pt`).
///
/// Preserves the distinction between content-fitted `auto` columns, explicit fixed/relative
/// columns (`100pt`, `30%`, etc.), and flexible fractional columns (`1fr`, `2fr`, etc.) across
/// `extract`, `word-construct`, and `docx-writer` instead of flattening everything to bare `f64`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TableColTrackIr {
    /// Content-sized `auto` column (`columns: auto`).
    Auto { width_pt: f64 },
    /// Explicit fixed or relative length (`100pt`, `30%`, `2cm`, ...).
    Fixed { width_pt: f64 },
    /// Flexible fractional track (`1fr`, `2fr`, ...) with its Typst-resolved width (`pt`).
    Fraction { fr: f64, width_pt: f64 },
}

impl TableColTrackIr {
    /// Resolved column width in points (`pt`).
    #[inline]
    pub const fn width_pt(self) -> f64 {
        match self {
            Self::Auto { width_pt }
            | Self::Fixed { width_pt }
            | Self::Fraction { width_pt, .. } => width_pt,
        }
    }

    /// Mutable reference to the resolved column width (`pt`) while preserving the track variant.
    #[inline]
    pub fn width_pt_mut(&mut self) -> &mut f64 {
        match self {
            Self::Auto { width_pt }
            | Self::Fixed { width_pt }
            | Self::Fraction { width_pt, .. } => width_pt,
        }
    }

    #[inline]
    pub const fn is_auto(self) -> bool {
        matches!(self, Self::Auto { .. })
    }

    #[inline]
    pub const fn is_fixed(self) -> bool {
        matches!(self, Self::Fixed { .. })
    }

    #[inline]
    pub const fn is_fraction(self) -> bool {
        matches!(self, Self::Fraction { .. })
    }
}

/// Table or Grid intermediate representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIr {
    #[serde(default, skip_serializing_if = "NodeId::is_unset")]
    pub id: NodeId,
    #[serde(default)]
    pub kind: TableKindIr,
    pub cols: Vec<TableColTrackIr>,
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

impl TableIr {
    /// Sum of resolved column widths in points (`pt`).
    #[inline]
    pub fn total_width_pt(&self) -> f64 {
        self.cols.iter().map(|c| c.width_pt()).sum()
    }

    /// Resolved width (`pt`) of column `col`, or `0.0` if out of bounds.
    #[inline]
    pub fn col_width_pt(&self, col: usize) -> f64 {
        self.cols.get(col).map_or(0.0, |c| c.width_pt())
    }
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
