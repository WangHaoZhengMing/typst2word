//! `table` / `grid` -> `<w:tbl>` with spans, borders, fills and insets.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{DashPatternIr, StrokeIr, TableIr, VAlignIr, VMergeState};
use crate::shape_dml::pt_to_dxa;
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    // =========================================================================
    // Table & Grid Rendering
    // =========================================================================
    pub(crate) fn render_table(&mut self, table: &TableIr, out: &mut String) {
        out.push_str("<w:tbl><w:tblPr>");
        Self::render_table_jc(table.align, out);
        let total_w_dxa: i64 = table.col_widths_pt.iter().map(|&w| pt_to_dxa(w)).sum();
        if total_w_dxa > 0 {
            let _ = write!(out, r#"<w:tblW w:w="{total_w_dxa}" w:type="dxa"/>"#);
        } else {
            out.push_str(r#"<w:tblW w:w="5000" w:type="pct"/>"#);
        }
        if let Some(summary) = &table.summary {
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
        for row in &table.rows {
            out.push_str("<w:tr><w:trPr>");
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

                // Cell padding / margins
                let _ = write!(
                    out,
                    r#"<w:tcMar><w:top w:w="{}" w:type="dxa"/><w:left w:w="{}" w:type="dxa"/><w:bottom w:w="{}" w:type="dxa"/><w:right w:w="{}" w:type="dxa"/></w:tcMar>"#,
                    pt_to_dxa(cell.inset.top),
                    pt_to_dxa(cell.inset.left),
                    pt_to_dxa(cell.inset.bottom),
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
                if cell.blocks.is_empty() || cell.vmerge == Some(VMergeState::Continue) {
                    out.push_str("<w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr></w:p>");
                } else {
                    let mut cell_xml = String::new();
                    self.render_blocks(&cell.blocks, &mut cell_xml);
                    if !cell_xml.contains("<w:p") {
                        cell_xml.push_str("<w:p/>");
                    }
                    out.push_str(&cell_xml);
                }

                out.push_str("</w:tc>");
            }

            out.push_str("</w:tr>");
        }

        out.push_str("</w:tbl>");
        // Add a small spacing paragraph after a table so consecutive tables don't merge in Word.
        out.push_str("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"60\"/></w:pPr></w:p>");
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
