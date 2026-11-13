//! `table` / `grid` -> `BlockNode::Table` (column/row sizing, spans, borders, fills, insets).

mod props;
mod sizing;

use std::collections::HashMap;

use typst::foundations::Content;
use typst::introspection::Location;
use typst::layout::grid::resolve::{CellGrid, Entry};
use typst::layout::{Size, Sizing};
use typst::syntax::Span;
use typst::visualize::Paint;

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    HAlignIr, SemanticRoleIr, SidesPtIr, TableCellIr, TableIr, TableKindIr, TableRowIr, VAlignIr,
};

pub(super) type CellNodeMap = HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)>;

impl<'a> ExtractorContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract_table_or_grid(
        &mut self,
        cell_grid: Option<&CellGrid>,
        span: Span,
        loc_opt: Option<Location>,
        kind: TableKindIr,
        summary: Option<String>,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<TableIr> {
        let is_semantic_table = kind == TableKindIr::SemanticTable;
        let grid = cell_grid?;
        let num_cols = grid.non_gutter_column_count();
        let num_rows = grid.non_gutter_row_count();
        if num_cols == 0 || num_rows == 0 {
            return None;
        }

        let role = if is_semantic_table {
            SemanticRoleIr::Table
        } else {
            SemanticRoleIr::Grid
        };
        let table_id = self.alloc_node_with_fragments(span, loc_opt, Some(role), children);
        self.parent_node_stack.push(table_id);

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
        let mut dims = self.resolve_table_dimensions(
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
        let footer_range = grid
            .footer
            .as_ref()
            .map(|f| (f.start / factor)..((f.end + factor - 1) / factor));
        let footer_rows = footer_range
            .as_ref()
            .map_or(0, |r| r.end.saturating_sub(r.start));

        // 3. Extract rows and cells.
        let mut rows = Vec::with_capacity(num_rows);
        for y in 0..num_rows {
            let is_header = y < header_rows;
            let is_footer = footer_range.as_ref().is_some_and(|r| r.contains(&y));
            let row_cells = self.extract_table_row_cells(
                grid,
                &cell_nodes_map,
                &dims.col_widths_pt,
                avail_w_pt,
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
                is_footer,
                cells: row_cells,
            });
        }
        self.parent_node_stack.pop();

        // Shift horizontal column gutter from a bottom-bordered field column `x` to the next column `x + 1`'s left inset.
        if !is_semantic_table && grid.has_gutter && num_cols > 1 {
            for x in 0..(num_cols - 1) {
                let col_has_bottom_border = rows.iter().any(|r| {
                    r.cells
                        .iter()
                        .any(|c| c.x == x && c.borders.bottom.is_some())
                });
                let next_col_has_bottom_border = rows.iter().any(|r| {
                    r.cells
                        .iter()
                        .any(|c| c.x == x + 1 && c.borders.bottom.is_some())
                });
                if let Some(Sizing::Rel(rel)) = grid.cols.get(x * 2 + 1)
                {
                    let col_gutter_pt =
                        (Self::resolve_length_pt(rel.abs, 11.0) + rel.rel.get() * avail_w_pt)
                            .max(0.0);
                    if col_has_bottom_border
                        && !next_col_has_bottom_border
                        && col_gutter_pt > 0.5
                        && dims.col_widths_pt[x] > col_gutter_pt + 20.0
                    {
                        dims.col_widths_pt[x] -= col_gutter_pt;
                        dims.col_widths_pt[x + 1] += col_gutter_pt;
                        for r in &mut rows {
                            for c in &mut r.cells {
                                if c.x == x + 1 {
                                    c.inset.left += col_gutter_pt;
                                }
                            }
                        }
                    } else if !col_has_bottom_border
                        && col_gutter_pt >= 4.0
                        && dims.col_widths_pt[x] > col_gutter_pt + 20.0
                    {
                        for r in &mut rows {
                            for c in &mut r.cells {
                                if c.x == x {
                                    c.inset.right = c.inset.right.max(col_gutter_pt);
                                }
                            }
                        }
                    }
                }
            }
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
            id: table_id,
            kind,
            col_widths_pt: dims.col_widths_pt,
            row_heights_pt: dims.row_heights_pt,
            header_rows,
            footer_rows,
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
        col_widths_pt: &[f64],
        avail_w_pt: f64,
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

                    if let Some(Paint::Tiling(_)) = &resolved_cell.fill {
                        self.record_unsupported_paint("table.cell.fill", resolved_cell.body.span());
                    }
                    let fill = resolved_cell.fill.as_ref().and_then(paint_to_hex);
                    let mut borders = Self::extract_cell_borders(
                        grid,
                        resolved_cell,
                        x,
                        y,
                        colspan,
                        rowspan,
                        num_cols,
                        num_rows,
                        is_semantic_table,
                    );

                    let (inset, halign, valign, is_header_cell, cell_blocks) =
                        if let Some((cell_elem, cnodes, _)) = cell_nodes_map.get(&(x, y)) {
                            let (mut ins, ha, va, ih) =
                                Self::extract_cell_props(cell_elem, is_header, is_semantic_table);
                            let prev_inset = self.container_inset_pt;
                            let prev_group_bounds = self.active_group_bounds_pt;
                            self.container_inset_pt += ins.left + ins.right + 1.0;
                            let table_base_x = prev_group_bounds
                                .map(|(l, _)| l)
                                .unwrap_or(self.content_left_pt);
                            let col_left: f64 =
                                table_base_x + col_widths_pt.iter().take(x).sum::<f64>();
                            let gutter_pt = if grid.has_gutter && x + 1 < num_cols {
                                match grid.cols.get(x * 2 + 1) {
                                    Some(Sizing::Rel(rel)) => {
                                        (Self::resolve_length_pt(rel.abs, 11.0)
                                            + rel.rel.get() * avail_w_pt)
                                            .max(0.0)
                                    }
                                    _ => 0.0,
                                }
                            } else {
                                0.0
                            };
                            let span_w: f64 = col_widths_pt
                                .iter()
                                .skip(x)
                                .take(colspan.max(1))
                                .sum::<f64>()
                                - gutter_pt;
                            if span_w > 12.0 {
                                self.active_group_bounds_pt = Some((col_left, col_left + span_w));
                            }
                            let mut blks = self.extract_blocks_from_nodes(cnodes, 0, ha);
                            self.active_group_bounds_pt = prev_group_bounds;
                            self.container_inset_pt = prev_inset;
                            if !is_semantic_table
                                && fill.is_none()
                                && !borders.has_any()
                                && blks.len() == 1
                            {
                                if matches!(
                                    &blks[0],
                                    ir::BlockNode::Container(inner)
                                        if inner.style.fill.is_none()
                                            && inner.style.borders.left.is_none()
                                            && inner.style.borders.right.is_none()
                                            && inner.style.borders.has_any()
                                ) {
                                    let Some(ir::BlockNode::Container(inner)) = blks.pop() else {
                                        unreachable!()
                                    };
                                    borders = inner.style.borders;
                                    ins = inner.style.inset;
                                    blks = inner.body;
                                } else if matches!(
                                    &blks[0],
                                    ir::BlockNode::Table(inner)
                                        if inner.kind == TableKindIr::StyledContainer
                                            && inner.rows.len() == 1
                                            && inner.rows[0].cells.len() == 1
                                            && inner.rows[0].cells[0].fill.is_none()
                                            && inner.rows[0].cells[0].borders.left.is_none()
                                            && inner.rows[0].cells[0].borders.right.is_none()
                                            && inner.rows[0].cells[0].borders.has_any()
                                ) {
                                    let Some(ir::BlockNode::Table(mut inner)) = blks.pop() else {
                                        unreachable!()
                                    };
                                    let inner_cell = inner.rows.remove(0).cells.remove(0);
                                    borders = inner_cell.borders;
                                    ins = inner_cell.inset;
                                    blks = inner_cell.blocks;
                                    self.report.tables_count =
                                        self.report.tables_count.saturating_sub(1);
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

                    row_cells.push(TableCellIr {
                        x,
                        y,
                        colspan,
                        rowspan,
                        fill,
                        borders,
                        inset,
                        halign,
                        valign,
                        is_header_cell,
                        blocks: cell_blocks,
                    });
                }
                Entry::Merged { .. } => {}
            }
        }

        if !is_semantic_table && grid.has_gutter {
            let row_has_bottom_border = row_cells.iter().any(|c| c.borders.bottom.is_some());
            if row_has_bottom_border {
                if y > 0
                    && let Some(Sizing::Rel(rel)) = grid.rows.get(raw_y - 1)
                    && rel.rel.is_zero()
                {
                    let row_gutter_pt = Self::resolve_length_pt(rel.abs, 11.0);
                    if row_gutter_pt > 1.3 {
                        for c in &mut row_cells {
                            c.inset.top += row_gutter_pt - 1.3;
                        }
                    }
                }
            } else if y + 1 < num_rows
                && let Some(Sizing::Rel(rel)) = grid.rows.get(raw_y + 1)
                && rel.rel.is_zero()
            {
                let row_gutter_pt = Self::resolve_length_pt(rel.abs, 11.0);
                if row_gutter_pt > 1.3 {
                    for c in &mut row_cells {
                        c.inset.bottom += row_gutter_pt - 1.3;
                    }
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
