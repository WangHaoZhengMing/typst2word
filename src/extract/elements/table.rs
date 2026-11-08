//! `table` / `grid` -> `BlockNode::Table` (spans, borders, fills, insets).

use std::collections::HashMap;

use typst::foundations::{Content, Smart, StyleChain};
use typst::layout::{GridCell, GridElem, Size};
use typst::model::{EnumElem, ListElem, TableCell, TableElem};
use typst::pdf::TableCellKind;
use typst::visualize::Paint;

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{
    BordersIr, HAlignIr, SidesPtIr, TableCellIr, TableIr, TableRowIr, VAlignIr, VMergeState,
};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Tables & Grids (`TableElem`, `GridElem`)
    // =========================================================================
    pub(crate) fn extract_table_or_grid(
        &mut self,
        cell_grid: Option<&typst::layout::grid::resolve::CellGrid>,
        is_semantic_table: bool,
        summary: Option<String>,
        children: &[TreeNode],
        inherited_align: HAlignIr,
    ) -> Option<TableIr> {
        let grid = cell_grid?;
        let num_cols = grid.non_gutter_column_count();
        let num_rows = grid.non_gutter_row_count();
        if num_cols == 0 || num_rows == 0 {
            return None;
        }

        // Collect laid-out non-repeated cell nodes keyed by `(x, y)` (non-gutter indices).
        let factor = if grid.has_gutter { 2 } else { 1 };
        let mut cell_nodes_map: HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)> =
            HashMap::new();
        Self::collect_table_cell_nodes(children, factor, &mut cell_nodes_map);

        // Estimate column widths and row heights from explicit grid sizings and laid-out cell frames.
        let mut col_widths_pt = vec![0.0_f64; num_cols];
        let mut row_heights_pt = vec![0.0_f64; num_rows];
        let mut row_has_explicit_height = vec![false; num_rows];

        for x in 0..num_cols {
            let raw_x = x * factor;
            if let Some(typst::layout::Sizing::Rel(rel)) = grid.cols.get(raw_x) {
                let abs_pt = rel.abs.abs.to_pt();
                if abs_pt > 1.0 && rel.rel.is_zero() {
                    col_widths_pt[x] = abs_pt;
                }
            }
        }
        for y in 0..num_rows {
            let raw_y = y * factor;
            if let Some(typst::layout::Sizing::Rel(rel)) = grid.rows.get(raw_y) {
                let abs_pt = rel.abs.abs.to_pt();
                if abs_pt > 1.0 && rel.rel.is_zero() {
                    row_heights_pt[y] = abs_pt;
                    row_has_explicit_height[y] = true;
                }
            }
        }

        for (&(cx, cy), (cell_elem, cnodes, csize)) in &cell_nodes_map {
            let colspan = Self::get_cell_colspan(cell_elem);
            let rowspan = Self::get_cell_rowspan(cell_elem);
            let w_pt = csize
                .x
                .to_pt()
                .max(Self::estimate_nodes_bbox(cnodes).2 + 12.0);
            let h_pt = csize
                .y
                .to_pt()
                .max(Self::estimate_nodes_bbox(cnodes).3 + 8.0);
            if colspan == 1 && cx < num_cols && col_widths_pt[cx] <= 1.0 {
                col_widths_pt[cx] = col_widths_pt[cx].max(w_pt);
            }
            if rowspan == 1 && cy < num_rows && !row_has_explicit_height[cy] {
                row_heights_pt[cy] = row_heights_pt[cy].max(h_pt);
            }
        }

        // Fill in any zero-width columns with a sensible default (e.g. from colspan cells or 90pt).
        for w in &mut col_widths_pt {
            if *w <= 1.0 {
                *w = 90.0;
            }
        }

        let header_rows = grid
            .headers
            .iter()
            .map(|h| h.range.end / factor)
            .max()
            .unwrap_or(0);

        let mut rows = Vec::with_capacity(num_rows);
        for y in 0..num_rows {
            let raw_y = y * factor;
            let is_header = y < header_rows;
            let mut row_cells = Vec::with_capacity(num_cols);

            for x in 0..num_cols {
                let raw_x = x * factor;
                let Some(entry) = grid.entry(raw_x, raw_y) else {
                    continue;
                };

                match entry {
                    typst::layout::grid::resolve::Entry::Cell(resolved_cell) => {
                        let colspan = resolved_cell.colspan.get();
                        let rowspan = resolved_cell.rowspan.get();
                        let vmerge = if rowspan > 1 {
                            Some(VMergeState::Restart)
                        } else {
                            None
                        };

                        if let Some(Paint::Tiling(_)) = &resolved_cell.fill {
                            self.record_unsupported_paint(
                                "table.cell.fill",
                                resolved_cell.body.span(),
                            );
                        }
                        let fill = resolved_cell.fill.as_ref().and_then(paint_to_hex);
                        let borders = Self::extract_cell_borders(resolved_cell, is_semantic_table);

                        let (inset, halign, valign, is_header_cell, cell_blocks) =
                            if let Some((cell_elem, cnodes, _)) = cell_nodes_map.get(&(x, y)) {
                                let (ins, ha, va, ih) =
                                    Self::extract_cell_props(cell_elem, is_header);
                                let blks = self.extract_blocks_from_nodes(cnodes, 0, ha);
                                (ins, ha, va, ih, blks)
                            } else {
                                (
                                    SidesPtIr::default(),
                                    HAlignIr::Left,
                                    VAlignIr::Top,
                                    is_header,
                                    Vec::new(),
                                )
                            };

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
                    typst::layout::grid::resolve::Entry::Merged { parent } => {
                        let parent_x = *parent % num_cols;
                        let parent_y = *parent / num_cols;
                        // Get parent cell to know its colspan/rowspan/fill/borders.
                        let parent_raw_x = parent_x * factor;
                        let parent_raw_y = parent_y * factor;
                        let parent_cell = grid.cell(parent_raw_x, parent_raw_y);
                        let colspan = parent_cell.map(|c| c.colspan.get()).unwrap_or(1);
                        let _rowspan = parent_cell.map(|c| c.rowspan.get()).unwrap_or(1);
                        let fill = parent_cell.and_then(|c| c.fill.as_ref().and_then(paint_to_hex));
                        let borders = parent_cell
                            .map(|c| Self::extract_cell_borders(c, is_semantic_table))
                            .unwrap_or_default();

                        // If `x > parent_x`, this slot is covered horizontally by `gridSpan` (`colspan`).
                        // If `x == parent_x && y > parent_y`, this slot starts a `<w:vMerge w:val="continue"/>` cell with `gridSpan = colspan`.
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

            rows.push(TableRowIr {
                height_pt: if row_has_explicit_height[y] && row_heights_pt[y] > 1.0 {
                    Some(row_heights_pt[y])
                } else {
                    None
                },
                is_header,
                cells: row_cells,
            });
        }

        Some(TableIr {
            is_semantic_table,
            col_widths_pt,
            row_heights_pt,
            header_rows,
            rows,
            align: inherited_align,
            summary,
        })
    }

    pub(crate) fn collect_table_cell_nodes(
        nodes: &[TreeNode],
        factor: usize,
        out: &mut HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(tc) = elem.to_packed::<TableCell>() {
                        if tc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(raw_x), Smart::Custom(raw_y)) = (
                            tc.x.get(StyleChain::default()),
                            tc.y.get(StyleChain::default()),
                        ) {
                            let x = raw_x / factor;
                            let y = raw_y / factor;
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
                        if let (Smart::Custom(raw_x), Smart::Custom(raw_y)) = (
                            gc.x.get(StyleChain::default()),
                            gc.y.get(StyleChain::default()),
                        ) {
                            let x = raw_x / factor;
                            let y = raw_y / factor;
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    // Do not recurse into nested tables/grids!
                    if elem.is::<TableElem>()
                        || elem.is::<GridElem>()
                        || elem.is::<ListElem>()
                        || elem.is::<EnumElem>()
                    {
                        continue;
                    }
                    Self::collect_table_cell_nodes(children, factor, out);
                }
                TreeNode::Group {
                    group, children, ..
                } => {
                    // If the group directly wraps a single cell element, pass its size down.
                    if children.len() == 1 {
                        if let TreeNode::Element {
                            elem,
                            children: cchildren,
                            ..
                        } = &children[0]
                        {
                            if elem.is::<TableCell>() || elem.is::<GridCell>() {
                                let mut temp = HashMap::new();
                                Self::collect_table_cell_nodes(children, factor, &mut temp);
                                for ((x, y), (ce, cc, sz)) in temp {
                                    let final_sz = if sz.x.to_pt() > 0.0 {
                                        sz
                                    } else {
                                        group.frame.size()
                                    };
                                    let _ = cchildren;
                                    out.entry((x, y))
                                        .and_modify(|(_, existing_children, _)| {
                                            existing_children.extend(cc.iter().cloned());
                                        })
                                        .or_insert((ce, cc, final_sz));
                                }
                                continue;
                            }
                        }
                    }
                    Self::collect_table_cell_nodes(children, factor, out);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn get_cell_colspan(elem: &Content) -> usize {
        if let Some(tc) = elem.to_packed::<TableCell>() {
            tc.colspan.get(StyleChain::default()).get()
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            gc.colspan.get(StyleChain::default()).get()
        } else {
            1
        }
    }

    pub(crate) fn get_cell_rowspan(elem: &Content) -> usize {
        if let Some(tc) = elem.to_packed::<TableCell>() {
            tc.rowspan.get(StyleChain::default()).get()
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            gc.rowspan.get(StyleChain::default()).get()
        } else {
            1
        }
    }

    pub(crate) fn extract_cell_borders(
        cell: &typst::layout::grid::resolve::Cell,
        is_semantic_table: bool,
    ) -> BordersIr {
        let mut borders = BordersIr {
            top: cell.stroke.top.as_deref().map(Self::abs_stroke_to_ir),
            bottom: cell.stroke.bottom.as_deref().map(Self::abs_stroke_to_ir),
            left: cell.stroke.left.as_deref().map(Self::abs_stroke_to_ir),
            right: cell.stroke.right.as_deref().map(Self::abs_stroke_to_ir),
        };
        if is_semantic_table && !borders.has_any() && !cell.stroke_overridden.iter().any(|&b| b) {
            // Default 0.5pt black table border if not explicitly overridden to `none`.
            // Note: if `cell.stroke` has `None` on all sides because user set `stroke: none`,
            // `cell.stroke` is already `None`, so we respect `cell.stroke` directly!
        }
        let _ = &mut borders;
        borders
    }

    pub(crate) fn extract_cell_props(
        elem: &Content,
        default_is_header: bool,
    ) -> (SidesPtIr, HAlignIr, VAlignIr, bool) {
        if let Some(tc) = elem.to_packed::<TableCell>() {
            let align = tc.align.get(StyleChain::default());
            let (ha, va) = Self::alignment_to_ir(align);
            let inset = Self::sides_rel_length_to_pt(&tc.inset.get(StyleChain::default()));
            let is_header = matches!(
                tc.kind.get(StyleChain::default()),
                Smart::Custom(TableCellKind::Header(..))
            ) || default_is_header;
            (inset, ha, va, is_header)
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            let align = gc.align.get(StyleChain::default());
            let (ha, va) = Self::alignment_to_ir(align);
            let inset = Self::sides_rel_length_to_pt(&gc.inset.get(StyleChain::default()));
            (inset, ha, va, default_is_header)
        } else {
            (
                SidesPtIr::default(),
                HAlignIr::Left,
                VAlignIr::Top,
                default_is_header,
            )
        }
    }
}
