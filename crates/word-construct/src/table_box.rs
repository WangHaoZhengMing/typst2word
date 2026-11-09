//! Microsoft Word Table & Cell Construct (`WordCellMarginsDxa`).
//!
//! Models `<w:tblPr>`, `<w:tblLayout w:type="fixed"/>`, `<w:tblGrid>`, `<w:gridCol>`,
//! `<w:tcPr>`, `<w:tcW>`, and `<w:tcMar>`.

use serde::{Deserialize, Serialize};

use crate::units::pt_to_dxa;

/// Semantic role of a table cell in WordprocessingML, determining `<w:tcMar>` padding compensation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableCellRole {
    /// A single-cell `#block(fill: ..., stroke: ...)` styled container table.
    StyledContainer,
    /// A semantic data table cell (`#table(...)`).
    SemanticTable { has_list: bool },
    /// A layout grid cell (`#grid(...)`).
    LayoutGrid { has_list: bool },
}

impl TableCellRole {
    /// Vertical leading trim (`pt`) subtracted from `inset_top_pt` to compensate for Word's
    /// built-in line-box internal leading.
    #[inline]
    pub const fn vertical_top_trim_pt(self, inset_top_pt: f64) -> f64 {
        match self {
            Self::StyledContainer if inset_top_pt > 0.5 => -1.5,
            Self::StyledContainer => 0.0,
            Self::SemanticTable { has_list: true } | Self::LayoutGrid { has_list: true } => 3.8,
            Self::SemanticTable { has_list: false } => 1.1,
            Self::LayoutGrid { has_list: false } => 2.2,
        }
    }

    /// Solved bottom cell margin in `dxa`.
    #[inline]
    pub fn resolve_bottom_dxa(self, inset_top_pt: f64, inset_bottom_pt: f64) -> i64 {
        match (self, inset_bottom_pt > 0.5) {
            (Self::StyledContainer, true) => pt_to_dxa((inset_bottom_pt - 1.0).max(0.0)),
            (Self::StyledContainer, false) => 0,
            (role, _) => {
                let v_sub = role.vertical_top_trim_pt(inset_top_pt);
                pt_to_dxa((inset_bottom_pt - v_sub).max(0.0))
            }
        }
    }

    /// Solved `(left_pt, right_pt)` horizontal cell padding in points.
    #[inline]
    pub fn resolve_horizontal_pt(self, inset_left_pt: f64, inset_right_pt: f64) -> (f64, f64) {
        match self {
            Self::StyledContainer => (inset_left_pt.max(0.0), inset_right_pt + 3.0),
            Self::SemanticTable { .. } => (
                (inset_left_pt - 1.2).max(0.0),
                (inset_right_pt - 1.2).max(0.0),
            ),
            Self::LayoutGrid { .. } => (inset_left_pt.max(0.0), inset_right_pt.max(0.0)),
        }
    }
}

/// Solved `<w:tcMar>` cell padding in `dxa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordCellMarginsDxa {
    pub top_dxa: i64,
    pub bottom_dxa: i64,
    pub left_dxa: i64,
    pub right_dxa: i64,
}

impl WordCellMarginsDxa {
    /// Declaratively solve `<w:tcMar>` from Typst's `inset` (in `pt`) and a [`TableCellRole`].
    #[inline]
    pub fn solve(
        inset_top_pt: f64,
        inset_bottom_pt: f64,
        inset_left_pt: f64,
        inset_right_pt: f64,
        role: TableCellRole,
    ) -> Self {
        let v_sub = role.vertical_top_trim_pt(inset_top_pt);
        let top_dxa = pt_to_dxa((inset_top_pt - v_sub).max(0.0));
        let bottom_dxa = role.resolve_bottom_dxa(inset_top_pt, inset_bottom_pt);
        let (left_pt, right_pt) = role.resolve_horizontal_pt(inset_left_pt, inset_right_pt);

        Self {
            top_dxa,
            bottom_dxa,
            left_dxa: pt_to_dxa(left_pt),
            right_dxa: pt_to_dxa(right_pt),
        }
    }
}
