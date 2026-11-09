//! `table` / `grid` -> `TableConstruct` -> `<w:tbl>` with spans, borders, fills and insets.

use std::fmt::Write as _;

use crate::construct::table::{
    RowSplitPolicy, TableConstruct, TableWidthConstruct, solve_table_cell_paragraph_style,
};
use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, DashPatternIr, StrokeIr, TableIr, VAlignIr, VMergeState};
use crate::units::pt_to_dxa;
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    // =========================================================================
    // Table & Grid Rendering
    // =========================================================================
    pub(crate) fn render_table(&mut self, table: &TableIr, out: &mut String) {
        let construct = TableConstruct::solve(table);
        let is_styled_container = construct.is_styled_container;
        let is_semantic_table = construct.is_semantic_table;

        out.push_str("<w:tbl><w:tblPr>");
        Self::render_table_jc(construct.align, out);
        if let Some(ind_dxa) = construct.indent_dxa {
            let _ = write!(out, r#"<w:tblInd w:w="{ind_dxa}" w:type="dxa"/>"#);
        }
        match construct.width {
            TableWidthConstruct::ExactDxa(total_w_dxa) => {
                let _ = write!(out, r#"<w:tblW w:w="{total_w_dxa}" w:type="dxa"/>"#);
            }
            TableWidthConstruct::FullWidthPct5000 => {
                out.push_str(r#"<w:tblW w:w="5000" w:type="pct"/>"#);
            }
        }
        if let Some(summary) = &table.summary
            && !is_styled_container
        {
            let _ = write!(
                out,
                r#"<w:tblCaption w:val="{}"/><w:tblDescription w:val="{}"/>"#,
                escape_xml(summary),
                escape_xml(summary)
            );
        }
        out.push_str(r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#);
        out.push_str("</w:tblPr>");

        // `<w:tblGrid>`
        out.push_str("<w:tblGrid>");
        for &dxa in &construct.grid_cols_dxa {
            let _ = write!(out, r#"<w:gridCol w:w="{dxa}"/>"#);
        }
        out.push_str("</w:tblGrid>");

        // Rows
        for (row_idx, row) in table.rows.iter().enumerate() {
            let row_construct = construct.rows[row_idx];
            let keep_rows_together = row_construct.keep_rows_together;
            match row_construct.split_policy {
                RowSplitPolicy::AllowSplit => out.push_str("<w:tr><w:trPr>"),
                RowSplitPolicy::CantSplit => out.push_str("<w:tr><w:trPr><w:cantSplit/>"),
            }
            if row_construct.is_header {
                out.push_str("<w:tblHeader/>");
            }
            if let Some(h_dxa) = row_construct.min_height_dxa {
                let _ = write!(out, r#"<w:trHeight w:val="{h_dxa}" w:hRule="atLeast"/>"#);
            }
            out.push_str("</w:trPr>");

            for cell in &row.cells {
                if cell.hmerged_covered {
                    continue;
                }
                out.push_str("<w:tc><w:tcPr>");
                let span_cols = cell.colspan.max(1);
                let cell_w_pt: f64 = table
                    .col_widths_pt
                    .iter()
                    .skip(cell.x)
                    .take(span_cols)
                    .sum();
                let cell_w_dxa = pt_to_dxa(cell_w_pt).max(144);
                let _ = write!(out, r#"<w:tcW w:w="{cell_w_dxa}" w:type="dxa"/>"#);

                if cell.colspan > 1 {
                    let _ = write!(out, r#"<w:gridSpan w:val="{}"/>"#, cell.colspan);
                }
                match cell.vmerge {
                    Some(VMergeState::Restart) => {
                        out.push_str(r#"<w:vMerge w:val="restart"/>"#);
                    }
                    Some(VMergeState::Continue) => {
                        out.push_str("<w:vMerge/>");
                    }
                    None => {}
                }

                // Cell borders
                out.push_str("<w:tcBorders>");
                Self::render_tc_border("top", cell.borders.top.as_ref(), out);
                Self::render_tc_border("left", cell.borders.left.as_ref(), out);
                Self::render_tc_border("bottom", cell.borders.bottom.as_ref(), out);
                Self::render_tc_border("right", cell.borders.right.as_ref(), out);
                out.push_str("</w:tcBorders>");

                // Cell shading / background fill
                if let Some(fill_hex) = &cell.fill {
                    let _ = write!(
                        out,
                        r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill_hex}"/>"#
                    );
                }

                // Cell padding / margins (subtract Word's built-in ~2.2pt font box vertical leading
                // from top/bottom cell padding so row heights match Typst's `inset` visually).
                let cell_has_list = !is_styled_container
                    && cell
                        .blocks
                        .iter()
                        .any(|b| matches!(b, BlockNode::List { .. } | BlockNode::Enum { .. }));
                let cell_role = word_construct::TableCellRole::from_flags(
                    is_styled_container,
                    is_semantic_table,
                    cell_has_list,
                );
                let margins = word_construct::WordCellMarginsDxa::solve_for_role(
                    cell.inset.top,
                    cell.inset.bottom,
                    cell.inset.left,
                    cell.inset.right,
                    cell_role,
                );
                let _ = write!(
                    out,
                    r#"<w:tcMar><w:top w:w="{}" w:type="dxa"/><w:left w:w="{}" w:type="dxa"/><w:bottom w:w="{}" w:type="dxa"/><w:right w:w="{}" w:type="dxa"/></w:tcMar>"#,
                    margins.top_dxa, margins.left_dxa, margins.bottom_dxa, margins.right_dxa,
                );

                // Vertical alignment
                let valign_str = match cell.valign {
                    VAlignIr::Top => "top",
                    VAlignIr::Center => "center",
                    VAlignIr::Bottom => "bottom",
                };
                let _ = write!(out, r#"<w:vAlign w:val="{valign_str}"/>"#);
                out.push_str("</w:tcPr>");

                // Cell body blocks (Word requires at least one `<w:p>` inside every `<w:tc>`!)
                let keep_next_xml = if keep_rows_together {
                    "<w:keepNext/>"
                } else {
                    ""
                };
                if cell.blocks.is_empty() || cell.vmerge == Some(VMergeState::Continue) {
                    let _ = write!(
                        out,
                        r#"<w:p><w:pPr>{keep_next_xml}<w:spacing w:before="0" w:after="0" w:line="220" w:lineRule="auto"/></w:pPr></w:p>"#
                    );
                } else {
                    let mut cell_xml = String::new();
                    self.render_table_cell_blocks(
                        &cell.blocks,
                        keep_rows_together,
                        is_styled_container,
                        is_semantic_table,
                        &mut cell_xml,
                    );
                    if !cell_xml.contains("<w:p") || !cell_xml.ends_with("</w:p>") {
                        let _ = write!(
                            cell_xml,
                            r#"<w:p><w:pPr>{keep_next_xml}<w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p>"#
                        );
                    }
                    out.push_str(&cell_xml);
                }

                out.push_str("</w:tc>");
            }

            out.push_str("</w:tr>");
        }

        out.push_str("</w:tbl>");
    }

    pub(crate) fn render_table_cell_blocks(
        &mut self,
        blocks: &[BlockNode],
        keep_next: bool,
        is_styled_container: bool,
        is_semantic_table: bool,
        out: &mut String,
    ) {
        let len = blocks.len();
        for (idx, block) in blocks.iter().enumerate() {
            if let BlockNode::Paragraph {
                content,
                style,
                bookmark,
            } = block
            {
                let prev_block = (idx > 0).then(|| &blocks[idx - 1]);
                let next_block = blocks.get(idx + 1);
                let cell_style = solve_table_cell_paragraph_style(
                    content,
                    style,
                    idx,
                    len,
                    prev_block,
                    next_block,
                    is_styled_container,
                    is_semantic_table,
                );
                self.render_paragraph_with_keep_next(
                    content,
                    &cell_style,
                    bookmark.as_deref(),
                    keep_next,
                    out,
                );
            } else if !is_styled_container && idx + 1 == len {
                match block {
                    BlockNode::List {
                        items,
                        tight,
                        level,
                        spacing_pt,
                        indent_pt,
                        hanging_pt,
                    } => {
                        self.render_list_with_trailing_gap(
                            items,
                            *tight,
                            *level,
                            *spacing_pt,
                            *indent_pt,
                            *hanging_pt,
                            Some(0),
                            Some(254),
                            out,
                        );
                    }
                    BlockNode::Enum {
                        items,
                        tight,
                        level,
                        spacing_pt,
                        indent_pt,
                        hanging_pt,
                    } => {
                        self.render_enum_with_trailing_gap(
                            items,
                            *tight,
                            *level,
                            *spacing_pt,
                            *indent_pt,
                            *hanging_pt,
                            Some(0),
                            Some(254),
                            out,
                        );
                    }
                    _ => self.render_block(block, out),
                }
            } else {
                self.render_block(block, out);
            }
        }
    }

    pub(crate) fn render_tc_border(side: &str, stroke: Option<&StrokeIr>, out: &mut String) {
        if let Some(s) = stroke {
            let sz = ((s.thickness_pt * 8.0).round() as u32).clamp(2, 96);
            let val = Self::dash_to_w_border_val(&s.dash);
            let _ = write!(
                out,
                r#"<w:{side} w:val="{val}" w:sz="{sz}" w:space="0" w:color="{}"/>"#,
                s.color
            );
        } else {
            let _ = write!(
                out,
                r#"<w:{side} w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#
            );
        }
    }

    pub(crate) fn dash_to_w_border_val(dash: &DashPatternIr) -> &'static str {
        match dash {
            DashPatternIr::Solid => "single",
            DashPatternIr::Dotted => "dotted",
            DashPatternIr::Dashed | DashPatternIr::Custom => "dashed",
            DashPatternIr::DashDot => "dotDash",
            DashPatternIr::DashDotDot => "dotDotDash",
        }
    }
}
