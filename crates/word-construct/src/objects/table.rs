//! Word Table & Cell Construct (`<w:tbl>`, `<w:tr>`, `<w:tc>`).
//!
//! Solves table indentation, column widths, row splitting policy (`RowSplitPolicy`),
//! Word `<w:vMerge/>` (`VMergeState`) and `<w:gridSpan/>` slot expansion (`TableCellSlotConstruct`),
//! `<w:tcMar>` cell padding via [`crate::WordCellMarginsDxa`], cell-internal
//! paragraph style overrides, and child table width/indent adjustments inside list items.

use crate::TableCellRole;
use crate::objects::paragraph::content_has_cjk_chars;
use crate::units::pt_to_dxa;
use ir::{
    BlockNode, BordersIr, ColorHex, HAlignIr, InlineNode, ParagraphStyleIr, SidesPtIr,
    TableCellIr, TableIr, TableKindIr, VAlignIr,
};

/// Vertical merge state for Word `<w:vMerge/>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VMergeState {
    Restart,
    Continue,
}

/// Solved `<w:tc>` slot in a Word table row (`Primary` cell or `VMergeContinue` placeholder).
#[derive(Debug, Clone)]
pub enum TableCellSlotConstruct<'a> {
    /// Primary Typst cell starting at `(cell.x, cell.y)`.
    Primary {
        cell: &'a TableCellIr,
        vmerge: Option<VMergeState>,
        width_dxa: i64,
    },
    /// Word `<w:vMerge/>` continuation placeholder for a cell spanning from an earlier row.
    VMergeContinue {
        x: usize,
        colspan: usize,
        width_dxa: i64,
        fill: Option<ColorHex>,
        borders: BordersIr,
        inset: SidesPtIr,
        valign: VAlignIr,
    },
}

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

/// Solved construct for a table header/body row (`<w:trPr>` + expanded `<w:tc>` slots).
#[derive(Debug, Clone)]
pub struct TableRowConstruct<'a> {
    pub split_policy: RowSplitPolicy,
    pub is_header: bool,
    pub min_height_dxa: Option<i64>,
    pub keep_rows_together: bool,
    pub cells: Vec<TableCellSlotConstruct<'a>>,
}

/// Solved top-level construct for a `<w:tbl>` element.
#[derive(Debug, Clone)]
pub struct TableConstruct<'a> {
    pub kind: TableKindIr,
    pub align: HAlignIr,
    pub indent_dxa: Option<i64>,
    pub width: TableWidthConstruct,
    pub grid_cols_dxa: Vec<i64>,
    pub rows: Vec<TableRowConstruct<'a>>,
}

impl<'a> TableConstruct<'a> {
    pub fn solve(table: &'a TableIr) -> Self {
        let kind = table.kind;

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

        let num_cols = table
            .col_widths_pt
            .len()
            .max(
                table
                    .rows
                    .iter()
                    .flat_map(|r| r.cells.iter().map(|c| c.x + c.colspan.max(1)))
                    .max()
                    .unwrap_or(1),
            );
        let num_rows = table.rows.len();
        let rows = table
            .rows
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                let next_row_is_complex_layout = kind != TableKindIr::SemanticTable
                    && table.rows.get(row_idx + 1).is_some_and(|next_r| {
                        next_r.cells.iter().any(|c| {
                            c.blocks.len() > 1
                                || c.blocks
                                    .iter()
                                    .any(|b| !matches!(b, BlockNode::Paragraph { .. }))
                        })
                    });
                let row_is_complex_layout = kind != TableKindIr::SemanticTable
                    && row.cells.iter().any(|c| {
                        c.blocks.len() > 2
                            || c.blocks.iter().any(|b| {
                                matches!(b, BlockNode::Table(_) | BlockNode::Figure { .. })
                            })
                    });
                let keep_rows_together = if kind == TableKindIr::StyledContainer {
                    num_rows == 2 && row_idx == 0 && row.cells.iter().all(|c| c.blocks.len() <= 1)
                } else {
                    !next_row_is_complex_layout && num_rows <= 15 && row_idx + 1 < num_rows
                };
                let split_policy =
                    if (kind == TableKindIr::StyledContainer && row.height_pt.is_none())
                        || row_is_complex_layout
                    {
                        RowSplitPolicy::AllowSplit
                    } else {
                        RowSplitPolicy::CantSplit
                    };
                let min_height_dxa = row
                    .height_pt
                    .map(pt_to_dxa)
                    .filter(|&h_dxa| h_dxa > 40);

                let cells = Self::expand_row_cell_slots(table, row_idx, num_cols);

                TableRowConstruct {
                    split_policy,
                    is_header: row.is_header,
                    min_height_dxa,
                    keep_rows_together,
                    cells,
                }
            })
            .collect();

        Self {
            kind,
            align: table.align,
            indent_dxa,
            width,
            grid_cols_dxa,
            rows,
        }
    }

    fn expand_row_cell_slots(
        table: &'a TableIr,
        row_idx: usize,
        num_cols: usize,
    ) -> Vec<TableCellSlotConstruct<'a>> {
        let mut slots = Vec::new();
        let row = &table.rows[row_idx];
        let mut x = 0;
        while x < num_cols {
            if let Some(cell) = row.cells.iter().find(|c| c.x == x) {
                let span_cols = cell.colspan.max(1);
                let cell_w_pt: f64 = table.col_widths_pt.iter().skip(x).take(span_cols).sum();
                let width_dxa = pt_to_dxa(cell_w_pt).max(144);
                let vmerge = (cell.rowspan > 1).then_some(VMergeState::Restart);
                slots.push(TableCellSlotConstruct::Primary {
                    cell,
                    vmerge,
                    width_dxa,
                });
                x += span_cols;
                continue;
            }

            // Check if a cell from an earlier row `parent_y < row_idx` vertically spans `(x, row_idx)`.
            let mut found_vmerge_parent = None;
            for parent_y in (0..row_idx).rev() {
                if let Some(parent_cell) = table.rows[parent_y].cells.iter().find(|c| {
                    c.x == x && parent_y + c.rowspan.max(1) > row_idx
                }) {
                    found_vmerge_parent = Some(parent_cell);
                    break;
                }
            }

            if let Some(parent_cell) = found_vmerge_parent {
                let span_cols = parent_cell.colspan.max(1);
                let cell_w_pt: f64 = table.col_widths_pt.iter().skip(x).take(span_cols).sum();
                let width_dxa = pt_to_dxa(cell_w_pt).max(144);
                slots.push(TableCellSlotConstruct::VMergeContinue {
                    x,
                    colspan: span_cols,
                    width_dxa,
                    fill: parent_cell.fill.clone(),
                    borders: parent_cell.borders.clone(),
                    inset: SidesPtIr::default(),
                    valign: VAlignIr::Top,
                });
                x += span_cols;
            } else {
                x += 1;
            }
        }
        slots
    }
}

/// Map [`TableKindIr`] and cell content to [`TableCellRole`].
#[inline]
pub fn resolve_table_cell_role(kind: TableKindIr, cell_blocks: &[BlockNode]) -> TableCellRole {
    match kind {
        TableKindIr::StyledContainer => TableCellRole::StyledContainer,
        TableKindIr::SemanticTable => TableCellRole::SemanticTable {
            has_list: cell_blocks
                .iter()
                .any(|b| matches!(b, BlockNode::List { .. } | BlockNode::Enum { .. })),
        },
        TableKindIr::LayoutGrid => TableCellRole::LayoutGrid {
            has_list: cell_blocks
                .iter()
                .any(|b| matches!(b, BlockNode::List { .. } | BlockNode::Enum { .. })),
        },
    }
}

/// Solve `<w:pPr>` style adjustments for a `Paragraph` at `idx` inside a table cell's `blocks` slice.
pub fn solve_table_cell_paragraph_style(
    content: &[InlineNode],
    style: &ParagraphStyleIr,
    idx: usize,
    len: usize,
    prev_block: Option<&BlockNode>,
    next_block: Option<&BlockNode>,
    table_kind: TableKindIr,
) -> ParagraphStyleIr {
    let mut cell_style = style.clone();
    if idx == 0 {
        cell_style.space_before_pt = 0.0;
    } else if matches!(prev_block, Some(BlockNode::Table(_))) {
        cell_style.space_before_pt = cell_style.space_before_pt.max(11.0);
    }
    if idx + 1 == len {
        cell_style.space_after_pt = 0.0;
    } else if table_kind != TableKindIr::StyledContainer {
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
    match table_kind {
        TableKindIr::StyledContainer => {}
        TableKindIr::SemanticTable => {
            if content_has_cjk_chars(content) {
                cell_style.line_spacing_pt = Some(4.5);
            } else if cell_style.line_spacing_pt.is_some() {
                cell_style.line_spacing_pt = Some(6.5);
            } else {
                cell_style.line_spacing_pt = Some(6.0);
            }
        }
        TableKindIr::LayoutGrid => {
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
    if adjusted.kind != TableKindIr::SemanticTable && adjusted.align == HAlignIr::Center {
        let avail_w_pt = ((content_width_dxa as f64 / 20.0) - target_left_pt).max(100.0);
        let total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
        adjusted.align = HAlignIr::Left;
        adjusted.left_indent_pt = target_left_pt + ((avail_w_pt - total_col_w).max(0.0) * 0.5);
    } else if adjusted.kind != TableKindIr::SemanticTable && adjusted.align == HAlignIr::Left {
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
