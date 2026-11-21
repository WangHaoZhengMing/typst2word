//! `table` / `grid` -> `BlockNode::Table` (column/row sizing, spans, borders, fills, insets).

mod props;
mod sizing;

use std::collections::HashMap;

use typst::foundations::Content;
use typst::introspection::Location;
use typst::layout::grid::resolve::{CellGrid, Entry};
use typst::layout::{Point, Size, Sizing};
use typst::syntax::Span;
use typst::visualize::Paint;

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    HAlignIr, SemanticRoleIr, SidesPtIr, TableCellIr, TableColTrackIr, TableIr, TableKindIr,
    TableRowIr, VAlignIr,
};

pub(super) type CellNodeMap = HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)>;

impl<'a> ExtractorContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract_table_or_grid(
        &mut self,
        cell_grid: Option<&CellGrid>,
        span: Span,
        loc_opt: Option<Location>,
        pos: Point,
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
        let in_quote = self.parent_node_stack.iter().any(|pid| {
            self.origins
                .get(pid)
                .is_some_and(|o| matches!(o.role, Some(SemanticRoleIr::Quote)))
        });
        let table_id = self.alloc_node_with_fragments(span, loc_opt, Some(role), children);
        self.parent_node_stack.push(table_id);

        // 1. Collect laid-out non-repeated cell nodes keyed by `(x, y)` (non-gutter indices).
        let factor = if grid.has_gutter { 2 } else { 1 };
        let mut cell_nodes_map: CellNodeMap = HashMap::new();
        Self::collect_table_cell_nodes(children, &mut cell_nodes_map);

        let (bbox_x, _, bbox_w, _) = Self::estimate_nodes_bbox(children);
        let table_origin_x = match children {
            [TreeNode::Group { pos: gpos, .. }] => gpos.x.to_pt(),
            _ => pos.x.to_pt(),
        };
        let col_w = self.current_column_width_pt();
        let (col_count, col_gutter) = self.active_columns;
        let ref_x = if bbox_w > 0.5 && bbox_x.is_finite() {
            bbox_x.min(table_origin_x)
        } else {
            table_origin_x
        };
        let (left_bound, right_bound) = if let Some((gl, gr)) = self.scope.active_group_bounds_pt {
            if col_count > 1 && ref_x >= gl - 4.0 && bbox_w <= col_w + 12.0 {
                let col_idx = (((ref_x - gl + 4.0).max(0.0)) / (col_w + col_gutter).max(1.0))
                    .floor() as usize;
                let cl = gl + (col_idx.min(col_count - 1) as f64) * (col_w + col_gutter);
                (cl, cl + col_w)
            } else {
                (gl, gr.max(gl + 40.0))
            }
        } else if col_count > 1 && ref_x >= self.content_left_pt - 4.0 && bbox_w <= col_w + 12.0 {
            let col_idx = (((ref_x - self.content_left_pt + 4.0).max(0.0))
                / (col_w + col_gutter).max(1.0))
            .floor() as usize;
            let cl =
                self.content_left_pt + (col_idx.min(col_count - 1) as f64) * (col_w + col_gutter);
            (cl, cl + col_w)
        } else {
            (self.content_left_pt, self.content_right_pt)
        };

        // Block-level `#align(center/right)[#table(...)]` / `#set align(center)` realizes as a
        // style on the element's `StyleChain` (without an enclosing `AlignElem` tag), so honor
        // `AlignElem::alignment` when the caller only inherited `Left`.
        let eff_inherited_align = if inherited_align == HAlignIr::Left {
            let chain = self.scoped_style_chain(loc_opt);
            let (style_align, _) = Self::alignment_to_ir(typst::foundations::Smart::Custom(
                chain.get(typst::layout::AlignElem::alignment),
            ));
            if matches!(style_align, HAlignIr::Center | HAlignIr::Right) {
                style_align
            } else {
                inherited_align
            }
        } else {
            inherited_align
        };

        // Capture untagged soft `#pad(...)` horizontal offsets (e.g. full-bleed `#pad(x: -36pt)`
        // slide header/footer grids or `#pad(left: ...)` inside `#show` / `context`).
        let untagged_pad_offset_pt = if eff_inherited_align == HAlignIr::Left
            && self.scope.container_inset_pt <= 0.1
            && list_level == 0
            && !in_quote
        {
            let raw_off = table_origin_x - left_bound;
            if (-144.0..=-1.5).contains(&raw_off) || (2.0..=48.0).contains(&raw_off) {
                raw_off
            } else {
                0.0
            }
        } else {
            0.0
        };

        let left_indent_pt = if list_level > 0 && eff_inherited_align == HAlignIr::Left {
            15.0 * list_level as f64
        } else {
            untagged_pad_offset_pt
        };
        let base_avail_w = if self.scope.container_inset_pt <= 0.1
            && let Some((l, r)) = self.scope.active_group_bounds_pt
        {
            (r - l).max(40.0)
        } else {
            self.current_column_width_pt() - self.scope.container_inset_pt
        };
        let content_avail_w = (base_avail_w - left_indent_pt.max(0.0)).max(120.0);
        let single_outer_group_w = match children {
            [TreeNode::Group { group, .. }] => group.frame.size().x.to_pt(),
            _ => 0.0,
        };
        let laid_out_span_w = if bbox_w > 0.5 && bbox_x.is_finite() {
            (bbox_x + bbox_w - table_origin_x.min(bbox_x)).max(bbox_w)
        } else {
            0.0
        };
        let avail_w_pt = if single_outer_group_w > 40.0 {
            single_outer_group_w
        } else if laid_out_span_w > content_avail_w + 1.0 {
            laid_out_span_w
        } else {
            content_avail_w
        };

        // 2. Resolve column widths and row heights from explicit sizings and laid-out frames.
        let base_font_pt = Self::resolve_length_pt(
            self.scoped_style_chain(loc_opt)
                .get(typst::text::TextElem::size)
                .0,
            11.0,
        );
        let mut dims = self.resolve_table_dimensions(
            grid,
            &cell_nodes_map,
            is_semantic_table,
            avail_w_pt,
            base_font_pt,
            factor,
        );

        let raw_header_rows = grid
            .headers
            .iter()
            .map(|h| h.range.end / factor)
            .max()
            .unwrap_or(0);
        let footer_range = grid
            .footer
            .as_ref()
            .map(|f| (f.start / factor)..((f.end + factor - 1) / factor));

        // Filter out rows that were not laid out in this fragment (e.g. a table split across
        // Word sections where trailing rows spilled onto the next section/page), while preserving
        // rows that either have at least one laid-out cell in `cell_nodes_map` OR are pure
        // `Entry::Merged` continuations of an already-kept row's `rowspan`.
        let kept_rows: Vec<usize> = if cell_nodes_map.is_empty() {
            (0..num_rows).collect()
        } else {
            let mut kept = Vec::with_capacity(num_rows);
            let mut row_present = vec![false; num_rows];
            for y in 0..num_rows {
                let has_laid_out_cell = (0..num_cols).any(|x| cell_nodes_map.contains_key(&(x, y)));
                if has_laid_out_cell {
                    row_present[y] = true;
                    kept.push(y);
                    continue;
                }
                let raw_y = y * factor;
                let has_own_cell = (0..num_cols).any(|x| {
                    matches!(grid.entry(x * factor, raw_y), Some(Entry::Cell(_)))
                });
                let covered_by_kept_rowspan = !has_own_cell
                    && (0..num_cols).any(|x| {
                        grid.parent_cell_position(x * factor, raw_y)
                            .is_some_and(|p| {
                                let py = p.y / factor;
                                py < y && row_present[py]
                            })
                    });
                if covered_by_kept_rowspan {
                    row_present[y] = true;
                    kept.push(y);
                }
            }
            kept
        };
        if kept_rows.is_empty() {
            self.parent_node_stack.pop();
            return None;
        }

        let table_left_x = if table_origin_x.is_finite() && table_origin_x > 0.5 {
            table_origin_x
        } else if bbox_w > 0.5 && bbox_x.is_finite() {
            bbox_x
        } else {
            self.scope
                .active_group_bounds_pt
                .map(|(l, _)| l)
                .unwrap_or(self.content_left_pt)
        };

        // 3. Extract rows and cells.
        let kept_len = kept_rows.len();
        let mut rows = Vec::with_capacity(kept_len);
        let mut row_heights_pt = Vec::with_capacity(kept_len);
        for (new_y, &y) in kept_rows.iter().enumerate() {
            let is_header = y < raw_header_rows;
            let is_footer = footer_range.as_ref().is_some_and(|r| r.contains(&y));
            let mut row_cells = self.extract_table_row_cells(
                grid,
                &cell_nodes_map,
                &dims.cols,
                table_left_x,
                avail_w_pt,
                y,
                num_cols,
                num_rows,
                factor,
                is_header,
                is_semantic_table,
            );
            for cell in &mut row_cells {
                cell.y = new_y;
                cell.rowspan = cell.rowspan.min(kept_len - new_y).max(1);
            }
            row_heights_pt.push(dims.row_heights_pt[y]);
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
        dims.row_heights_pt = row_heights_pt;
        let header_rows = rows.iter().take_while(|r| r.is_header).count();
        let footer_rows = rows.iter().filter(|r| r.is_footer).count();
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
                        && dims.cols[x].width_pt() > col_gutter_pt + 20.0
                    {
                        *dims.cols[x].width_pt_mut() -= col_gutter_pt;
                        *dims.cols[x + 1].width_pt_mut() += col_gutter_pt;
                        for r in &mut rows {
                            for c in &mut r.cells {
                                if c.x == x + 1 {
                                    c.inset.left += col_gutter_pt;
                                }
                            }
                        }
                    } else if !col_has_bottom_border
                        && col_gutter_pt >= 4.0
                        && dims.cols[x].width_pt() > col_gutter_pt + 20.0
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

        let total_table_w: f64 = dims.cols.iter().map(|c| c.width_pt()).sum();
        let left_gap = table_origin_x - left_bound;
        let right_gap = right_bound - (table_origin_x + total_table_w);
        let is_centered_narrow = eff_inherited_align == HAlignIr::Left
            && list_level == 0
            && left_indent_pt <= 0.5
            && total_table_w < (right_bound - left_bound) - 24.0
            && left_gap > 12.0
            && right_gap > 12.0
            && (left_gap - right_gap).abs() <= 12.0;
        let is_right_flushed_narrow = eff_inherited_align == HAlignIr::Left
            && list_level == 0
            && left_indent_pt <= 0.5
            && !dims.inferred_center_align
            && !is_centered_narrow
            && total_table_w < (right_bound - left_bound) - 24.0
            && left_gap > 12.0
            && right_gap.abs() <= 4.0;
        let align = if eff_inherited_align == HAlignIr::Left
            && (dims.inferred_center_align || is_centered_narrow)
        {
            HAlignIr::Center
        } else if is_right_flushed_narrow {
            HAlignIr::Right
        } else {
            eff_inherited_align
        };
        let eff_left_indent_pt = if align != HAlignIr::Left {
            0.0
        } else {
            left_indent_pt
        };

        Some(TableIr {
            id: table_id,
            kind,
            cols: dims.cols,
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
        cols: &[TableColTrackIr],
        table_left_x: f64,
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
                            let mut blks = self.with_scope(|ctx| {
                                let table_base_x = table_left_x;
                                ctx.scope.container_inset_pt += ins.left + ins.right + 1.0;
                                let col_left: f64 =
                                    table_base_x + cols.iter().take(x).map(|c| c.width_pt()).sum::<f64>();
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
                                let span_w: f64 = cols
                                    .iter()
                                    .skip(x)
                                    .take(colspan.max(1))
                                    .map(|c| c.width_pt())
                                    .sum::<f64>()
                                    - gutter_pt;
                                if span_w > 12.0 {
                                    ctx.scope.active_group_bounds_pt =
                                        Some((col_left, col_left + span_w));
                                }
                                ctx.extract_blocks_from_nodes(cnodes, 0, ha)
                            });
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
                                SidesPtIr::zero()
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

        // Preserve Typst's exact `grid(gutter: ...)` row gutter in IR; Word line-box leading
        // compensation belongs exclusively in `word-construct` (`WordCellMarginsDxa`).
        if !is_semantic_table && grid.has_gutter {
            let base_font_pt = Self::resolve_length_pt(
                self.scoped_style_chain(None).get(typst::text::TextElem::size).0,
                11.0,
            );
            let row_has_bottom_border = row_cells.iter().any(|c| c.borders.bottom.is_some());
            if row_has_bottom_border {
                if y > 0
                    && let Some(Sizing::Rel(rel)) = grid.rows.get(raw_y - 1)
                    && rel.rel.is_zero()
                {
                    let row_gutter_pt = Self::resolve_length_pt(rel.abs, base_font_pt);
                    if row_gutter_pt > 0.0 {
                        for c in &mut row_cells {
                            c.inset.top += row_gutter_pt;
                        }
                    }
                }
            } else if y + 1 < num_rows
                && let Some(Sizing::Rel(rel)) = grid.rows.get(raw_y + 1)
                && rel.rel.is_zero()
            {
                let row_gutter_pt = Self::resolve_length_pt(rel.abs, base_font_pt);
                if row_gutter_pt > 0.0 {
                    for c in &mut row_cells {
                        c.inset.bottom += row_gutter_pt;
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
