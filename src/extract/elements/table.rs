//! `table` / `grid` -> `BlockNode::Table` (column/row sizing, spans, borders, fills, insets).

use std::collections::HashMap;

use typst::foundations::{Content, Smart, StyleChain};
use typst::layout::grid::resolve::{Cell, CellGrid, Entry};
use typst::layout::{GridCell, GridElem, Size, Sizing};
use typst::model::{EnumElem, ListElem, TableCell, TableElem};
use typst::pdf::TableCellKind;
use typst::visualize::Paint;

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{
    BordersIr, HAlignIr, SidesPtIr, TableCellIr, TableIr, TableRowIr, VAlignIr, VMergeState,
};

type CellNodeMap = HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)>;

struct ResolvedTableDimensions {
    col_widths_pt: Vec<f64>,
    row_heights_pt: Vec<f64>,
    row_has_explicit_height: Vec<bool>,
}

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

        Some(TableIr {
            is_semantic_table,
            is_styled_container: false,
            col_widths_pt: dims.col_widths_pt,
            row_heights_pt: dims.row_heights_pt,
            header_rows,
            rows,
            align: inherited_align,
            left_indent_pt,
            summary,
        })
    }

    fn resolve_table_dimensions(
        &self,
        grid: &CellGrid,
        cell_nodes_map: &CellNodeMap,
        is_semantic_table: bool,
        avail_w_pt: f64,
        factor: usize,
    ) -> ResolvedTableDimensions {
        let num_cols = grid.non_gutter_column_count();
        let num_rows = grid.non_gutter_row_count();
        let mut col_widths_pt = vec![0.0_f64; num_cols];
        let mut natural_col_w = vec![0.0_f64; num_cols];
        let mut col_min_x = vec![f64::INFINITY; num_cols];
        let mut col_fr = vec![0.0_f64; num_cols];
        let mut row_heights_pt = vec![0.0_f64; num_rows];
        let mut row_has_explicit_height = vec![false; num_rows];

        let has_fr_col = grid.cols.iter().any(|c| matches!(c, Sizing::Fr(_)));

        for (&(cx, cy), (cell_elem, cnodes, csize)) in cell_nodes_map {
            let colspan = Self::get_cell_colspan(cell_elem);
            let rowspan = Self::get_cell_rowspan(cell_elem);
            let (bbox_x, _, bbox_w, bbox_h) = Self::estimate_nodes_bbox(cnodes);
            let (ins, _, _, _) = Self::extract_cell_props(cell_elem, false, is_semantic_table);
            let pad_w = if is_semantic_table {
                let list_pad = if Self::nodes_contain_list(cnodes) {
                    15.0
                } else {
                    0.0
                };
                let extra = if has_fr_col { 3.5 + list_pad } else { 0.5 };
                ins.left + ins.right + extra
            } else {
                6.0
            };
            let w_pt = csize.x.to_pt().max(bbox_w + pad_w);
            let h_pt = csize.y.to_pt().max(bbox_h + 8.0);
            if colspan == 1 && cx < num_cols {
                natural_col_w[cx] = natural_col_w[cx].max(w_pt);
                if bbox_w > 0.5 && bbox_x.is_finite() {
                    col_min_x[cx] = col_min_x[cx].min(bbox_x);
                }
            }
            if rowspan == 1 && cy < num_rows && !row_has_explicit_height[cy] {
                row_heights_pt[cy] = row_heights_pt[cy].max(h_pt);
            }
        }

        for x in 0..num_cols {
            let raw_x = x * factor;
            match grid.cols.get(raw_x) {
                Some(Sizing::Rel(rel)) => {
                    let abs_pt = Self::resolve_length_pt(rel.abs, 11.0);
                    let w = abs_pt + rel.rel.get() * avail_w_pt;
                    if w > 1.0 {
                        col_widths_pt[x] = w;
                    }
                }
                Some(Sizing::Fr(fr)) => {
                    col_fr[x] = fr.get().max(0.0);
                }
                Some(Sizing::Auto) | None => {
                    col_widths_pt[x] = natural_col_w[x].max(24.0);
                }
            }
        }

        // Resolve horizontal column gutters (`gutter: 1fr` or `gutter: 10pt`) and `Sizing::Fr` columns.
        let num_gutters = num_cols.saturating_sub(1);
        let mut gutter_abs_pt = vec![0.0_f64; num_gutters];
        let mut gutter_fr = vec![0.0_f64; num_gutters];
        if grid.has_gutter && num_gutters > 0 {
            for x in 0..num_gutters {
                let raw_g = x * 2 + 1;
                match grid.cols.get(raw_g) {
                    Some(Sizing::Rel(rel)) => {
                        let abs_pt = Self::resolve_length_pt(rel.abs, 11.0);
                        gutter_abs_pt[x] = (abs_pt + rel.rel.get() * avail_w_pt).max(0.0);
                    }
                    Some(Sizing::Fr(fr)) => {
                        gutter_fr[x] = fr.get().max(0.0);
                    }
                    _ => {}
                }
            }
        }

        let total_fr: f64 = col_fr.iter().sum::<f64>() + gutter_fr.iter().sum::<f64>();
        if total_fr > 0.0 {
            if !is_semantic_table
                && gutter_fr.iter().any(|&gf| gf > 0.0)
                && col_fr[num_cols - 1] == 0.0
            {
                col_widths_pt[num_cols - 1] += 4.0;
            }
            let used_w: f64 = col_widths_pt.iter().sum::<f64>() + gutter_abs_pt.iter().sum::<f64>();
            let rem_w = (avail_w_pt - used_w).max(0.0);
            for x in 0..num_cols {
                if col_fr[x] > 0.0 {
                    let fr_w = (col_fr[x] / total_fr) * rem_w;
                    col_widths_pt[x] = fr_w.max(24.0);
                }
            }
            for x in 0..num_gutters {
                if gutter_fr[x] > 0.0 {
                    gutter_abs_pt[x] += (gutter_fr[x] / total_fr) * rem_w;
                }
            }
        }

        for x in 0..num_gutters {
            if gutter_abs_pt[x] > 0.0 {
                col_widths_pt[x] += gutter_abs_pt[x];
            }
        }

        // For non-semantic multi-column grids where all columns have laid-out X coordinates,
        // use exact inter-column X deltas from Typst's layout engine (`col_min_x[x+1] - col_min_x[x]`).
        if !is_semantic_table
            && num_cols > 1
            && col_min_x.iter().all(|x| x.is_finite())
            && col_min_x.windows(2).all(|w| w[1] > w[0] + 12.0)
        {
            for x in 0..(num_cols - 1) {
                col_widths_pt[x] = col_min_x[x + 1] - col_min_x[x];
            }
            let col_w = self.current_column_width_pt();
            let (col_count, col_gutter) = self.active_columns;
            let right_bound = if col_count > 1 && col_min_x[0] >= self.content_left_pt - 4.0 {
                let cidx = (((col_min_x[0] - self.content_left_pt).max(0.0))
                    / (col_w + col_gutter).max(1.0))
                .floor() as usize;
                self.content_left_pt
                    + (cidx.min(col_count - 1) as f64) * (col_w + col_gutter)
                    + col_w
            } else {
                self.content_right_pt
            };
            col_widths_pt[num_cols - 1] = (right_bound - col_min_x[num_cols - 1] + 4.0).max(24.0);
        }

        for y in 0..num_rows {
            let raw_y = y * factor;
            if let Some(Sizing::Rel(rel)) = grid.rows.get(raw_y) {
                let abs_pt = Self::resolve_length_pt(rel.abs, 11.0);
                if abs_pt > 1.0 && rel.rel.is_zero() {
                    row_heights_pt[y] = abs_pt;
                    row_has_explicit_height[y] = true;
                }
            }
        }

        for (x, w) in col_widths_pt.iter_mut().enumerate() {
            if *w <= 1.0 {
                *w = natural_col_w[x].max(60.0);
            }
        }

        ResolvedTableDimensions {
            col_widths_pt,
            row_heights_pt,
            row_has_explicit_height,
        }
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

        for x in 0..num_cols {
            let raw_x = x * factor;
            let Some(entry) = grid.entry(raw_x, raw_y) else {
                continue;
            };

            match entry {
                Entry::Cell(resolved_cell) => {
                    let colspan = resolved_cell.colspan.get();
                    let rowspan = resolved_cell.rowspan.get();
                    let vmerge = if rowspan > 1 {
                        Some(VMergeState::Restart)
                    } else {
                        None
                    };

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
                            let (ins, ha, va, ih) =
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
                    let vmerge = if y > parent_y && x == parent_x {
                        Some(VMergeState::Continue)
                    } else {
                        None
                    };

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

    pub(crate) fn collect_table_cell_nodes(nodes: &[TreeNode], out: &mut CellNodeMap) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(tc) = elem.to_packed::<TableCell>() {
                        if tc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(x), Smart::Custom(y)) = (
                            tc.x.get(StyleChain::default()),
                            tc.y.get(StyleChain::default()),
                        ) {
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    if let Some(gc) = elem.to_packed::<GridCell>() {
                        if gc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(x), Smart::Custom(y)) = (
                            gc.x.get(StyleChain::default()),
                            gc.y.get(StyleChain::default()),
                        ) {
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    if elem.is::<TableElem>()
                        || elem.is::<GridElem>()
                        || elem.is::<ListElem>()
                        || elem.is::<EnumElem>()
                    {
                        continue;
                    }
                    Self::collect_table_cell_nodes(children, out);
                }
                TreeNode::Group {
                    group, children, ..
                } => {
                    if children.len() == 1
                        && let TreeNode::Element { elem, .. } = &children[0]
                        && (elem.is::<TableCell>() || elem.is::<GridCell>())
                    {
                        let mut temp = HashMap::new();
                        Self::collect_table_cell_nodes(children, &mut temp);
                        for ((x, y), (ce, cc, sz)) in temp {
                            let final_sz = if sz.x.to_pt() > 0.0 {
                                sz
                            } else {
                                group.frame.size()
                            };
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(cc.iter().cloned());
                                })
                                .or_insert((ce, cc, final_sz));
                        }
                        continue;
                    }
                    Self::collect_table_cell_nodes(children, out);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn nodes_contain_list(nodes: &[TreeNode]) -> bool {
        nodes.iter().any(|n| match n {
            TreeNode::Element { elem, children, .. } => {
                elem.is::<ListElem>() || elem.is::<EnumElem>() || Self::nodes_contain_list(children)
            }
            TreeNode::Group { children, .. } => Self::nodes_contain_list(children),
            _ => false,
        })
    }

    pub(crate) fn get_cell_colspan(elem: &Content) -> usize {
        elem.to_packed::<TableCell>()
            .map(|tc| tc.colspan.get(StyleChain::default()).get())
            .or_else(|| {
                elem.to_packed::<GridCell>()
                    .map(|gc| gc.colspan.get(StyleChain::default()).get())
            })
            .unwrap_or(1)
    }

    pub(crate) fn get_cell_rowspan(elem: &Content) -> usize {
        elem.to_packed::<TableCell>()
            .map(|tc| tc.rowspan.get(StyleChain::default()).get())
            .or_else(|| {
                elem.to_packed::<GridCell>()
                    .map(|gc| gc.rowspan.get(StyleChain::default()).get())
            })
            .unwrap_or(1)
    }

    pub(crate) fn extract_cell_borders(cell: &Cell, _is_semantic_table: bool) -> BordersIr {
        BordersIr {
            top: cell.stroke.top.as_deref().map(Self::abs_stroke_to_ir),
            bottom: cell.stroke.bottom.as_deref().map(Self::abs_stroke_to_ir),
            left: cell.stroke.left.as_deref().map(Self::abs_stroke_to_ir),
            right: cell.stroke.right.as_deref().map(Self::abs_stroke_to_ir),
        }
    }

    pub(crate) fn extract_cell_props(
        elem: &Content,
        default_is_header: bool,
        is_semantic_table: bool,
    ) -> (SidesPtIr, HAlignIr, VAlignIr, bool) {
        let grid_def = SidesPtIr::default();
        if let Some(tc) = elem.to_packed::<TableCell>() {
            let (ha, va) = Self::alignment_to_ir(tc.align.get(StyleChain::default()));
            let inset = Self::sides_rel_length_to_pt(&tc.inset.get(StyleChain::default()));
            let is_header = matches!(
                tc.kind.get(StyleChain::default()),
                Smart::Custom(TableCellKind::Header(..))
            ) || default_is_header;
            (inset, ha, va, is_header)
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            let (ha, va) = Self::alignment_to_ir(gc.align.get(StyleChain::default()));
            let inset = Self::sides_rel_length_to_pt_with_default(
                &gc.inset.get(StyleChain::default()),
                grid_def,
            );
            (inset, ha, va, default_is_header)
        } else {
            let inset = if is_semantic_table {
                SidesPtIr::default()
            } else {
                grid_def
            };
            (inset, HAlignIr::Left, VAlignIr::Top, default_is_header)
        }
    }
}
