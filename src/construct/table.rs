//! Word Table & Cell Construct (`<w:tbl>`, `<w:tr>`, `<w:tc>`).
//!
//! Solves table indentation, column widths, row splitting policy (`RowSplitPolicy`),
//! `<w:tcMar>` cell padding via [`word_construct::WordCellMarginsDxa`], cell-internal
//! paragraph style overrides, and child table width/indent adjustments inside list items.

use crate::construct::paragraph::content_has_cjk_chars;
use crate::ir::{BlockNode, HAlignIr, InlineNode, ParagraphStyleIr, TableIr};
use crate::units::pt_to_dxa;

/// Exhaustive table width specification for `<w:tblW>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableWidthConstruct {
    /// Exact width in `dxa` (`<w:tblW w:w="{0}" w:type="dxa"/>`).
    ExactDxa(i64),
    /// 100% page width (`<w:tblW w:w="5000" w:type="pct"/>`).
    FullWidthPct5000,
}

/// Exhaustive row split policy for `<w:trPr>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowSplitPolicy {
    /// Omit `<w:cantSplit/>` so multi-block or styled-container rows can break across pages.
    AllowSplit,
    /// Emit `<w:cantSplit/>` to prevent splitting the row across a page boundary.
    CantSplit,
}

/// Solved construct for a table header/body row (`<w:trPr>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowConstruct {
    pub split_policy: RowSplitPolicy,
    pub is_header: bool,
    pub min_height_dxa: Option<i64>,
    pub keep_rows_together: bool,
}

/// Solved top-level construct for a `<w:tbl>` element.
#[derive(Debug, Clone)]
pub struct TableConstruct {
    pub is_styled_container: bool,
    pub is_semantic_table: bool,
    pub align: HAlignIr,
    pub indent_dxa: Option<i64>,
    pub width: TableWidthConstruct,
    pub grid_cols_dxa: Vec<i64>,
    pub rows: Vec<TableRowConstruct>,
}

impl TableConstruct {
    pub fn solve(table: &TableIr) -> Self {
        let is_styled_container = table.is_styled_container();
        let is_semantic_table = table.is_semantic();

        let indent_dxa = (table.left_indent_pt > 0.5 && table.align == HAlignIr::Left).then(|| {
            let first_cell_left_inset = table
                .rows
                .first()
                .and_then(|r| r.cells.first())
                .filter(|c| c.borders.left.is_some())
                .map(|c| c.inset.left)
                .unwrap_or(0.0);
            pt_to_dxa(table.left_indent_pt + first_cell_left_inset)
        });

        let total_w_dxa: i64 = table.col_widths_pt.iter().map(|&w| pt_to_dxa(w)).sum();
        let width = if total_w_dxa > 0 {
            TableWidthConstruct::ExactDxa(total_w_dxa)
        } else {
            TableWidthConstruct::FullWidthPct5000
        };

        let grid_cols_dxa = table
            .col_widths_pt
            .iter()
            .map(|&col_w| pt_to_dxa(col_w).max(144))
            .collect();

        let num_rows = table.rows.len();
        let rows = table
            .rows
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                let next_row_is_complex_layout = !is_semantic_table
                    && table.rows.get(row_idx + 1).is_some_and(|next_r| {
                        next_r.cells.iter().any(|c| {
                            c.blocks.len() > 1
                                || c.blocks
                                    .iter()
                                    .any(|b| !matches!(b, BlockNode::Paragraph { .. }))
                        })
                    });
                let row_is_complex_layout = !is_semantic_table
                    && row.cells.iter().any(|c| {
                        c.blocks.len() > 2
                            || c.blocks.iter().any(|b| {
                                matches!(b, BlockNode::Table(_) | BlockNode::Figure { .. })
                            })
                    });
                let keep_rows_together = !is_styled_container
                    && !next_row_is_complex_layout
                    && num_rows <= 15
                    && row_idx + 1 < num_rows;
                let split_policy =
                    if (is_styled_container && row.height_pt.is_none()) || row_is_complex_layout {
                        RowSplitPolicy::AllowSplit
                    } else {
                        RowSplitPolicy::CantSplit
                    };
                let min_height_dxa = row
                    .height_pt
                    .map(pt_to_dxa)
                    .filter(|&h_dxa| h_dxa > 40);

                TableRowConstruct {
                    split_policy,
                    is_header: row.is_header,
                    min_height_dxa,
                    keep_rows_together,
                }
            })
            .collect();

        Self {
            is_styled_container,
            is_semantic_table,
            align: table.align,
            indent_dxa,
            width,
            grid_cols_dxa,
            rows,
        }
    }
}

/// Solve `<w:pPr>` style adjustments for a `Paragraph` at `idx` inside a table cell's `blocks` slice.
#[allow(clippy::too_many_arguments)]
pub fn solve_table_cell_paragraph_style(
    content: &[InlineNode],
    style: &ParagraphStyleIr,
    idx: usize,
    len: usize,
    prev_block: Option<&BlockNode>,
    next_block: Option<&BlockNode>,
    is_styled_container: bool,
    is_semantic_table: bool,
) -> ParagraphStyleIr {
    let mut cell_style = style.clone();
    if idx == 0 {
        cell_style.space_before_pt = 0.0;
    } else if matches!(prev_block, Some(BlockNode::Table(_))) {
        cell_style.space_before_pt = cell_style.space_before_pt.max(11.0);
    }
    if idx + 1 == len {
        cell_style.space_after_pt = 0.0;
    } else if !is_styled_container {
        cell_style.space_after_pt = cell_style.space_after_pt.min(2.0);
    } else {
        match next_block {
            Some(BlockNode::List { tight: true, .. } | BlockNode::Enum { tight: true, .. }) => {
                cell_style.space_after_pt = cell_style.space_after_pt.min(0.5);
            }
            Some(BlockNode::HorizontalRule { .. }) => {
                cell_style.space_after_pt = cell_style.space_after_pt.min(2.0);
            }
            _ => {}
        }
    }
    if !is_styled_container {
        let char_cnt: usize = content
            .iter()
            .map(|inl| match inl {
                InlineNode::Text(r) => r.text.chars().count(),
                _ => 1,
            })
            .sum();
        if is_semantic_table && char_cnt > 12 {
            if content_has_cjk_chars(content) {
                cell_style.line_spacing_pt = Some(4.5);
            } else {
                cell_style.line_spacing_pt = cell_style.line_spacing_pt.or(Some(6.0));
            }
        } else {
            cell_style.line_spacing_pt = Some(2.0);
        }
    }
    cell_style
}

/// Solve indentation, column widths, and bottom padding for a child `TableIr` nested inside a list/enum item.
pub fn solve_list_item_child_table(
    tbl: &TableIr,
    indent_left_dxa: usize,
    content_width_dxa: i64,
    bottom_pad_pt: f64,
) -> TableIr {
    let mut adjusted = tbl.clone();
    let target_left_pt = indent_left_dxa as f64 / 20.0;
    if !adjusted.is_semantic() && adjusted.align == HAlignIr::Center {
        let avail_w_pt = ((content_width_dxa as f64 / 20.0) - target_left_pt).max(100.0);
        let total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
        adjusted.align = HAlignIr::Left;
        adjusted.left_indent_pt = target_left_pt + ((avail_w_pt - total_col_w).max(0.0) * 0.5);
    } else if !adjusted.is_semantic() && adjusted.align == HAlignIr::Left {
        adjusted.left_indent_pt = target_left_pt;
        let avail_w_pt = ((content_width_dxa as f64 / 20.0) - target_left_pt).max(100.0);
        let total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
        if total_col_w > avail_w_pt + 6.0 {
            let excess = total_col_w - avail_w_pt;
            let ncols = adjusted.col_widths_pt.len();
            if ncols > 1 {
                let head_sum: f64 = adjusted.col_widths_pt[..ncols - 1].iter().sum();
                if head_sum > excess + 24.0 * (ncols - 1) as f64 {
                    for w in &mut adjusted.col_widths_pt[..ncols - 1] {
                        *w -= excess * (*w / head_sum);
                    }
                } else {
                    let scale = avail_w_pt / total_col_w;
                    for w in &mut adjusted.col_widths_pt {
                        *w *= scale;
                    }
                }
            } else {
                adjusted.col_widths_pt[0] = avail_w_pt;
            }
        }
    }
    if bottom_pad_pt > 0.0
        && let Some(last_row) = adjusted.rows.last_mut()
    {
        for cell in &mut last_row.cells {
            cell.inset.bottom += bottom_pad_pt;
        }
    }
    adjusted
}
