//! Column width and row height solver for `TableElem` and `GridElem`.

use typst::layout::Sizing;
use typst::layout::grid::resolve::CellGrid;

use super::CellNodeMap;
use crate::ExtractorContext;

pub(super) struct ResolvedTableDimensions {
    pub(super) col_widths_pt: Vec<f64>,
    pub(super) row_heights_pt: Vec<f64>,
    pub(super) row_has_explicit_height: Vec<bool>,
    pub(super) inferred_center_align: bool,
}

impl<'a> ExtractorContext<'a> {
    pub(super) fn resolve_table_dimensions(
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
                let extra = if has_fr_col {
                    5.0 + list_pad
                } else if bbox_w > 45.0 {
                    6.0
                } else if bbox_w > 11.0 {
                    3.5
                } else {
                    0.5
                };
                ins.left + ins.right + extra
            } else if num_cols == 1 {
                28.0
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
                    let min_auto_w = if num_cols >= 8 { 14.0 } else { 20.0 };
                    col_widths_pt[x] = natural_col_w[x].max(min_auto_w);
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
            if total_fr > 0.0 {
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
                col_widths_pt[num_cols - 1] =
                    (right_bound - col_min_x[num_cols - 1] + 4.0).max(24.0);
            }
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

        let total_table_w: f64 = col_widths_pt.iter().sum();
        let inferred_center_align = !is_semantic_table
            && total_fr == 0.0
            && col_min_x.first().is_some_and(|&x0| {
                if x0 >= self.content_left_pt - 4.0 {
                    let right_gap = self.content_right_pt - (x0 + total_table_w);
                    let left_gap_page = x0 - self.content_left_pt;
                    let left_gap_avail = x0 - (self.content_right_pt - avail_w_pt);
                    right_gap > 20.0
                        && total_table_w < avail_w_pt - 40.0
                        && ((left_gap_page > 20.0 && (left_gap_page - right_gap).abs() <= 18.0)
                            || (left_gap_avail > 20.0
                                && (left_gap_avail - right_gap).abs() <= 18.0))
                } else {
                    x0 > 20.0
                        && total_table_w < avail_w_pt - 40.0
                        && ((avail_w_pt - (x0 + total_table_w)) - x0).abs() <= 18.0
                }
            });

        ResolvedTableDimensions {
            col_widths_pt,
            row_heights_pt,
            row_has_explicit_height,
            inferred_center_align,
        }
    }
}
