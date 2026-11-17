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
            Self::SemanticTable { has_list: false } => 0.6,
            Self::LayoutGrid { has_list: false } => 2.1,
        }
    }

    /// Solved bottom cell margin in `dxa`.
    #[inline]
    pub fn resolve_bottom_dxa(self, inset_top_pt: f64, inset_bottom_pt: f64) -> i64 {
        match (self, inset_bottom_pt > 0.5) {
            (Self::StyledContainer, true) => pt_to_dxa((inset_bottom_pt - 1.0).max(0.0)),
            (Self::StyledContainer, false) => 0,
            (Self::SemanticTable { has_list: false }, _) => {
                pt_to_dxa((inset_bottom_pt - 5.4).max(0.0))
            }
            (Self::LayoutGrid { has_list: false }, _) => {
                // Conserve total internal leading (~4.2pt = 2.1pt top + 2.1pt bottom) of Word's
                // compact `LayoutGrid` cell line box relative to Typst's `cap-height` box: when
                // `inset_top_pt == 0` (e.g. `#grid(gutter: ...)`), `tcMar.top` is clamped at 0 and
                // absorbs 0pt, so `tcMar.bottom` absorbs the full 4.2pt excess.
                let top_absorbed_pt = inset_top_pt.clamp(0.0, 2.1);
                let rem_trim_pt = 4.2 - top_absorbed_pt;
                pt_to_dxa((inset_bottom_pt - rem_trim_pt).max(0.0))
            }
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
                (inset_left_pt - 0.2).max(0.0),
                (inset_right_pt - 0.2).max(0.0),
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

    /// Solve `<w:tcMar>` in row context, zeroing vertical padding on exact-height rows and
    /// restoring Word's visual bottom breathing room (`+70 dxa`) on the final row of semantic tables.
    #[inline]
    pub fn solve_for_row(
        inset_top_pt: f64,
        inset_bottom_pt: f64,
        inset_left_pt: f64,
        inset_right_pt: f64,
        role: TableCellRole,
        is_exact_height: bool,
        is_last_row: bool,
    ) -> Self {
        let mut margins = Self::solve(
            inset_top_pt,
            inset_bottom_pt,
            inset_left_pt,
            inset_right_pt,
            role,
        );
        if is_exact_height {
            margins.top_dxa = 0;
            margins.bottom_dxa = 0;
        } else if matches!(role, TableCellRole::SemanticTable { .. })
            && is_last_row
            && inset_bottom_pt >= 2.5
        {
            margins.bottom_dxa += 70;
        }
        margins
    }
}
