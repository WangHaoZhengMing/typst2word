//! `table` / `grid` -> `<w:tbl>` with spans, borders, fills and insets.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, DashPatternIr, StrokeIr, TableIr, VAlignIr, VMergeState};
use crate::units::pt_to_dxa;
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    // =========================================================================
    // Table & Grid Rendering
    // =========================================================================
    pub(crate) fn render_table(&mut self, table: &TableIr, out: &mut String) {
        let is_styled_container = table.summary.as_deref() == Some("__styled_container__");
        out.push_str("<w:tbl><w:tblPr>");
        Self::render_table_jc(table.align, out);
        if table.left_indent_pt > 0.5 && table.align == crate::ir::HAlignIr::Left {
            let ind_dxa = pt_to_dxa(table.left_indent_pt);
            let _ = write!(out, r#"<w:tblInd w:w="{ind_dxa}" w:type="dxa"/>"#);
        }
        let total_w_dxa: i64 = table.col_widths_pt.iter().map(|&w| pt_to_dxa(w)).sum();
        if total_w_dxa > 0 {
            let _ = write!(out, r#"<w:tblW w:w="{total_w_dxa}" w:type="dxa"/>"#);
        } else {
            out.push_str(r#"<w:tblW w:w="5000" w:type="pct"/>"#);
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
        for &col_w in &table.col_widths_pt {
            let dxa = pt_to_dxa(col_w).max(144);
            let _ = write!(out, r#"<w:gridCol w:w="{dxa}"/>"#);
        }
        out.push_str("</w:tblGrid>");

        // Rows
        let keep_rows_together = !is_styled_container && table.rows.len() <= 15;
        for row in &table.rows {
            if is_styled_container {
                out.push_str("<w:tr><w:trPr>");
            } else {
                out.push_str("<w:tr><w:trPr><w:cantSplit/>");
            }
            if row.is_header {
                out.push_str("<w:tblHeader/>");
            }
            if let Some(rh) = row.height_pt {
                let h_dxa = pt_to_dxa(rh);
                if h_dxa > 40 {
                    let _ = write!(out, r#"<w:trHeight w:val="{h_dxa}" w:hRule="atLeast"/>"#);
                }
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
                let top_mar_dxa = pt_to_dxa((cell.inset.top - 2.2).max(0.0));
                let bottom_mar_dxa = pt_to_dxa((cell.inset.bottom - 2.2).max(0.0));
                let _ = write!(
                    out,
                    r#"<w:tcMar><w:top w:w="{top_mar_dxa}" w:type="dxa"/><w:left w:w="{}" w:type="dxa"/><w:bottom w:w="{bottom_mar_dxa}" w:type="dxa"/><w:right w:w="{}" w:type="dxa"/></w:tcMar>"#,
                    pt_to_dxa(cell.inset.left),
                    pt_to_dxa(cell.inset.right),
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
                let mut cell_style = style.clone();
                if idx == 0 {
                    cell_style.space_before_pt = 0.0;
                }
                if idx + 1 == len {
                    cell_style.space_after_pt = 0.0;
                } else if !is_styled_container {
                    cell_style.space_after_pt = cell_style.space_after_pt.min(2.0);
                }
                if !is_styled_container {
                    // Compact line spacing inside table/grid cells (~0.35em leading in Typst).
                    cell_style.line_spacing_pt = Some(2.0);
                }
                let mut para_xml = String::new();
                self.render_paragraph(content, &cell_style, bookmark.as_deref(), &mut para_xml);
                if keep_next {
                    para_xml = para_xml.replacen("<w:pPr>", "<w:pPr><w:keepNext/>", 1);
                }
                out.push_str(&para_xml);
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
