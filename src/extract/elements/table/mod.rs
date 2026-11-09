//! `table` / `grid` -> `BlockNode::Table` (column/row sizing, spans, borders, fills, insets).

mod props;
mod sizing;

use std::collections::HashMap;

use typst::foundations::Content;
use typst::layout::grid::resolve::{CellGrid, Entry};
use typst::layout::{Size, Sizing};
use typst::visualize::Paint;

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{
    HAlignIr, SidesPtIr, TableCellIr, TableIr, TableRowIr, VAlignIr, VMergeState,
};

pub(super) type CellNodeMap = HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)>;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_table_or_grid(
        &mut self,
        cell_grid: Option<&CellGrid>,
        is_semantic_table: bool,
        summary: Option<String>,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<TableIr> {
        let grid = cell_grid?;
        let num_cols = grid.non_gutter_column_count();
        let num_rows = grid.non_gutter_row_count();
        if num_cols == 0 || num_rows == 0 {
            return None;
        }

        // 1. Collect laid-out non-repeated cell nodes keyed by `(x, y)` (non-gutter indices).
        let factor = if grid.has_gutter { 2 } else { 1 };
        let mut cell_nodes_map: CellNodeMap = HashMap::new();
        Self::collect_table_cell_nodes(children, &mut cell_nodes_map);

        let left_indent_pt = if list_level > 0 && inherited_align == HAlignIr::Left {
            15.0 * list_level as f64
        } else {
            0.0
        };
        let content_avail_w =
            (self.current_column_width_pt() - left_indent_pt - self.container_inset_pt).max(120.0);
        let outer_group_w = Self::find_group_size(children).x.to_pt();
        let avail_w_pt = if outer_group_w > 40.0 {
            outer_group_w
        } else {
            content_avail_w
        };

        // 2. Resolve column widths and row heights from explicit sizings and laid-out frames.
        let dims = self.resolve_table_dimensions(
            grid,
            &cell_nodes_map,
            is_semantic_table,
            avail_w_pt,
            factor,
        );

        let header_rows = grid
            .headers
            .iter()
            .map(|h| h.range.end / factor)
            .max()
            .unwrap_or(0);

        // 3. Extract rows and cells.
        let mut rows = Vec::with_capacity(num_rows);
        for y in 0..num_rows {
            let is_header = y < header_rows;
            let row_cells = self.extract_table_row_cells(
                grid,
                &cell_nodes_map,
                y,
                num_cols,
                num_rows,
                factor,
                is_header,
                is_semantic_table,
            );
            rows.push(TableRowIr {
                height_pt: if dims.row_has_explicit_height[y] && dims.row_heights_pt[y] > 1.0 {
                    Some(dims.row_heights_pt[y])
                } else {
                    None
                },
                is_header,
                cells: row_cells,
            });
        }

        let align = if inherited_align == HAlignIr::Left && dims.inferred_center_align {
            HAlignIr::Center
        } else {
            inherited_align
        };
        let eff_left_indent_pt = if align == HAlignIr::Center {
            0.0
        } else {
            left_indent_pt
        };

        Some(TableIr {
            is_semantic_table,
            is_styled_container: false,
            col_widths_pt: dims.col_widths_pt,
            row_heights_pt: dims.row_heights_pt,
            header_rows,
            rows,
            align,
            left_indent_pt: eff_left_indent_pt,
            summary,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn extract_table_row_cells(
        &mut self,
        grid: &CellGrid,
        cell_nodes_map: &CellNodeMap,
        y: usize,
        num_cols: usize,
        num_rows: usize,
        factor: usize,
        is_header: bool,
        is_semantic_table: bool,
    ) -> Vec<TableCellIr> {
        let raw_y = y * factor;
        let mut row_cells = Vec::with_capacity(num_cols);
        let row_min_y = if !is_semantic_table && num_cols > 1 {
            (0..num_cols)
                .filter_map(|cx| cell_nodes_map.get(&(cx, y)))
                .filter_map(|(_, cnodes, _)| Self::first_figure_or_table_y(cnodes))
                .fold(f64::INFINITY, f64::min)
        } else {
            f64::INFINITY
        };

        for x in 0..num_cols {
            let raw_x = x * factor;
            let Some(entry) = grid.entry(raw_x, raw_y) else {
                continue;
            };

            match entry {
                Entry::Cell(resolved_cell) => {
                    let colspan = resolved_cell.colspan.get();
                    let rowspan = resolved_cell.rowspan.get();
                    let vmerge = (rowspan > 1).then_some(VMergeState::Restart);

                    if let Some(Paint::Tiling(_)) = &resolved_cell.fill {
                        self.record_unsupported_paint("table.cell.fill", resolved_cell.body.span());
                    }
                    let fill = resolved_cell.fill.as_ref().and_then(paint_to_hex);
                    let borders = Self::extract_cell_borders(resolved_cell, is_semantic_table);

                    let (mut inset, halign, valign, is_header_cell, cell_blocks) =
                        if let Some((cell_elem, cnodes, _)) = cell_nodes_map.get(&(x, y)) {
                            Self::collect_linebreak_preceding_texts(
                                &resolved_cell.body,
                                &mut self.linebreak_preceding_texts,
                            );
                            let (mut ins, ha, va, ih) =
                                Self::extract_cell_props(cell_elem, is_header, is_semantic_table);
                            let prev_inset = self.container_inset_pt;
                            self.container_inset_pt += ins.left + ins.right + 1.0;
                            let mut blks = self.extract_blocks_from_nodes(cnodes, 0, ha);
                            self.container_inset_pt = prev_inset;
                             if num_cols > 1 && Self::find_explicit_align_elem(cnodes).is_none() {
                                for blk in &mut blks {
                                    if let crate::ir::BlockNode::Paragraph { style, .. } = blk {
                                        style.align = ha;
                                    }
                                }
                            }
                            if !is_semantic_table
                                && row_min_y.is_finite()
                                && let Some(cell_fig_y) = Self::first_figure_or_table_y(cnodes)
                            {
                                let top_offset = cell_fig_y - row_min_y;
                                if (4.0..=60.0).contains(&top_offset) {
                                    ins.top += top_offset;
                                }
                            }
                            (ins, ha, va, ih, blks)
                        } else {
                            let ins = if is_semantic_table {
                                SidesPtIr::default()
                            } else {
                                SidesPtIr {
                                    top: 1.0,
                                    bottom: 1.0,
                                    left: 0.0,
                                    right: 2.0,
                                }
                            };
                            (ins, HAlignIr::Left, VAlignIr::Top, is_header, Vec::new())
                        };

                    if !is_semantic_table
                        && grid.has_gutter
                        && y + 1 < num_rows
                        && let Some(Sizing::Rel(rel)) = grid.rows.get(raw_y + 1)
                        && rel.rel.is_zero()
                    {
                        let row_gutter_pt = Self::resolve_length_pt(rel.abs, 11.0);
                        if row_gutter_pt > 1.3 {
                            inset.bottom += row_gutter_pt - 1.3;
                        }
                    }

                    row_cells.push(TableCellIr {
                        x,
                        y,
                        colspan,
                        rowspan,
                        vmerge,
                        hmerged_covered: false,
                        fill,
                        borders,
                        inset,
                        halign,
                        valign,
                        is_header_cell,
                        blocks: cell_blocks,
                    });
                }
                Entry::Merged { parent } => {
                    let parent_x = *parent % num_cols;
                    let parent_y = *parent / num_cols;
                    let parent_cell = grid.cell(parent_x * factor, parent_y * factor);
                    let colspan = parent_cell.map(|c| c.colspan.get()).unwrap_or(1);
                    let fill = parent_cell.and_then(|c| c.fill.as_ref().and_then(paint_to_hex));
                    let borders = parent_cell
                        .map(|c| Self::extract_cell_borders(c, is_semantic_table))
                        .unwrap_or_default();

                    let hmerged_covered = x > parent_x;
                    let vmerge = (y > parent_y && x == parent_x).then_some(VMergeState::Continue);

                    row_cells.push(TableCellIr {
                        x,
                        y,
                        colspan: if x == parent_x { colspan } else { 1 },
                        rowspan: 1,
                        vmerge,
                        hmerged_covered,
                        fill,
                        borders,
                        inset: SidesPtIr::default(),
                        halign: HAlignIr::Left,
                        valign: VAlignIr::Top,
                        is_header_cell: is_header,
                        blocks: Vec::new(),
                    });
                }
            }
        }

        row_cells
    }

    fn first_figure_or_table_y(nodes: &[TreeNode]) -> Option<f64> {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    pos,
                    children,
                    ..
                } => {
                    if elem.is::<typst::model::FigureElem>() || elem.is::<typst::model::TableElem>()
                    {
                        let y = pos.y.to_pt();
                        if y > 0.0 {
                            return Some(y);
                        }
                    }
                    if let Some(y) = Self::first_figure_or_table_y(children) {
                        return Some(y);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(y) = Self::first_figure_or_table_y(children) {
                        return Some(y);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
