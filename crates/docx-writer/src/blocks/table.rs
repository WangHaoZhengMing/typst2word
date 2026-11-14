//! `table` / `grid` -> `TableConstruct` -> `<w:tbl>` with spans, borders, fills and insets.

use std::fmt::Write as _;

use word_construct::table::{
    RowSplitPolicy, TableCellSlotConstruct, TableConstruct, TableWidthConstruct, VMergeState,
    resolve_table_cell_role, solve_table_cell_paragraph_style,
};
use crate::writer::DocxWriter;
use word_construct::model::{
    BlockNode, DashPatternIr, StrokeIr, TableIr, TableKindIr, VAlignIr,
};
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_table(&mut self, table: &TableIr, out: &mut String) {
        self.render_table_with_keep_all_rows(table, false, out);
    }

    pub(crate) fn render_table_with_keep_all_rows(
        &mut self,
        table: &TableIr,
        keep_all_rows_with_next: bool,
        out: &mut String,
    ) {
        let construct = TableConstruct::solve(table);
        let table_kind = construct.kind;

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
            && table_kind != TableKindIr::StyledContainer
        {
            let _ = write!(
                out,
                r#"<w:tblCaption w:val="{}"/><w:tblDescription w:val="{}"/>"#,
                escape_xml(summary),
                escape_xml(summary)
            );
        }
        let last_row_attr = if table.footer_rows > 0 { "1" } else { "0" };
        let _ = write!(
            out,
            r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="{last_row_attr}" w:firstColumn="0" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#
        );
        out.push_str("</w:tblPr>");

        // `<w:tblGrid>`
        out.push_str("<w:tblGrid>");
        for &dxa in &construct.grid_cols_dxa {
            let _ = write!(out, r#"<w:gridCol w:w="{dxa}"/>"#);
        }
        out.push_str("</w:tblGrid>");

        // Rows
        for (row_idx, row_construct) in construct.rows.iter().enumerate() {
            let keep_rows_together = keep_all_rows_with_next || row_construct.keep_rows_together;
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

            for slot in &row_construct.cells {
                let (
                    colspan,
                    cell_w_dxa,
                    vmerge,
                    borders,
                    fill,
                    inset,
                    valign,
                    blocks,
                ) = match slot {
                    TableCellSlotConstruct::Primary {
                        cell,
                        vmerge,
                        width_dxa,
                    } => (
                        cell.colspan,
                        *width_dxa,
                        *vmerge,
                        &cell.borders,
                        cell.fill.as_deref(),
                        cell.inset,
                        cell.valign,
                        cell.blocks.as_slice(),
                    ),
                    TableCellSlotConstruct::VMergeContinue {
                        colspan,
                        width_dxa,
                        fill,
                        borders,
                        inset,
                        valign,
                        ..
                    } => (
                        *colspan,
                        *width_dxa,
                        Some(VMergeState::Continue),
                        borders,
                        fill.as_deref(),
                        *inset,
                        *valign,
                        &[][..],
                    ),
                };

                out.push_str("<w:tc><w:tcPr>");
                let _ = write!(out, r#"<w:tcW w:w="{cell_w_dxa}" w:type="dxa"/>"#);

                if colspan > 1 {
                    let _ = write!(out, r#"<w:gridSpan w:val="{colspan}"/>"#);
                }
                match vmerge {
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
                Self::render_tc_border("top", borders.top.as_ref(), out);
                Self::render_tc_border("left", borders.left.as_ref(), out);
                Self::render_tc_border("bottom", borders.bottom.as_ref(), out);
                Self::render_tc_border("right", borders.right.as_ref(), out);
                out.push_str("</w:tcBorders>");

                // Cell shading / background fill
                if let Some(fill_hex) = fill {
                    let _ = write!(
                        out,
                        r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill_hex}"/>"#
                    );
                }

                // Cell padding / margins
                let cell_role = resolve_table_cell_role(table_kind, blocks);
                let mut margins = word_construct::WordCellMarginsDxa::solve(
                    inset.top,
                    inset.bottom,
                    inset.left,
                    inset.right,
                    cell_role,
                );
                if table_kind == TableKindIr::SemanticTable
                    && row_idx + 1 == table.rows.len()
                    && inset.bottom >= 2.5
                {
                    margins.bottom_dxa += 70;
                }
                let _ = write!(
                    out,
                    r#"<w:tcMar><w:top w:w="{}" w:type="dxa"/><w:left w:w="{}" w:type="dxa"/><w:bottom w:w="{}" w:type="dxa"/><w:right w:w="{}" w:type="dxa"/></w:tcMar>"#,
                    margins.top_dxa, margins.left_dxa, margins.bottom_dxa, margins.right_dxa,
                );

                // Vertical alignment
                let valign_str = match valign {
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
                let zero_vertical_inset = inset.top <= 0.5 && inset.bottom <= 0.5;
                if blocks.is_empty() || vmerge == Some(VMergeState::Continue) {
                    let _ = write!(
                        out,
                        r#"<w:p><w:pPr>{keep_next_xml}<w:spacing w:before="0" w:after="0" w:line="220" w:lineRule="auto"/></w:pPr></w:p>"#
                    );
                } else {
                    let mut cell_xml = String::new();
                    self.render_table_cell_blocks(
                        blocks,
                        keep_rows_together,
                        table_kind,
                        zero_vertical_inset,
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
        table_kind: TableKindIr,
        zero_vertical_inset: bool,
        out: &mut String,
    ) {
        let len = blocks.len();
        for (idx, block) in blocks.iter().enumerate() {
            if let BlockNode::Paragraph {
                id,
                content,
                style,
                bookmark,
            } = block
            {
                let prev_block = (idx > 0).then(|| &blocks[idx - 1]);
                let next_block = blocks.get(idx + 1);
                let mut cell_style = solve_table_cell_paragraph_style(
                    content,
                    style,
                    idx,
                    len,
                    prev_block,
                    next_block,
                    table_kind,
                );
                if table_kind == TableKindIr::SemanticTable
                    && zero_vertical_inset
                    && style.line_spacing_pt.is_none()
                {
                    cell_style.line_spacing_pt = Some(0.0);
                }
                self.render_paragraph_with_keep_next(
                    *id,
                    content,
                    &cell_style,
                    bookmark.as_deref(),
                    keep_next,
                    out,
                );
            } else if table_kind != TableKindIr::StyledContainer
                && let BlockNode::ShapeBlock(shape) = block
            {
                let run_xml = crate::drawingml::shape_to_drawingml_run(self, shape);
                out.push_str("<w:p><w:pPr>");
                if keep_next {
                    out.push_str("<w:keepNext/>");
                }
                Self::render_jc(shape.align, out);
                let after_dxa = if idx + 1 < len { 20 } else { 0 };
                let _ = write!(
                    out,
                    r#"<w:spacing w:before="0" w:after="{after_dxa}" w:line="240" w:lineRule="auto"/></w:pPr>"#
                );
                out.push_str(&run_xml);
                out.push_str("</w:p>");
            } else if table_kind != TableKindIr::StyledContainer && idx + 1 == len {
                match block {
                    BlockNode::List {
                        items,
                        tight,
                        level,
                        spacing_pt,
                        indent_pt,
                        hanging_pt,
                        ..
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
                        ..
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
