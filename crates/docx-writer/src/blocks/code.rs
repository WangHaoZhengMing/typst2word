//! `raw(block: true)` -> `CodeBlockConstruct` -> syntax-colored code paragraphs (optionally boxed).

use std::fmt::Write as _;

use word_construct::code::CodeBlockConstruct;
use crate::writer::DocxWriter;
use word_construct::model::{NodeId, StrokeIr, TextRunIr};

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_code_block(
        &self,
        id: NodeId,
        lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        out: &mut String,
    ) {
        self.render_code_block_with_layout(id, lang, lines, fill, stroke, 0, false, out);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_code_block_indented(
        &self,
        id: NodeId,
        lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        left_indent_dxa: usize,
        out: &mut String,
    ) {
        self.render_code_block_with_layout(
            id,
            lang,
            lines,
            fill,
            stroke,
            left_indent_dxa,
            false,
            out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_code_block_with_layout(
        &self,
        id: NodeId,
        _lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        left_indent_dxa: usize,
        keep_last_with_next: bool,
        out: &mut String,
    ) {
        let (line_dxa, is_exact) = self.doc.resolve_code_block_line_spacing(id, lines);
        let line_rule = if is_exact { "exact" } else { "auto" };
        match CodeBlockConstruct::solve(fill, stroke, left_indent_dxa) {
            CodeBlockConstruct::UnboxedLines { left_indent_dxa } => {
                // Standard unboxed Typst `raw(block: true)`: render lines directly with exact syntax colors.
                let num_lines = lines.len();
                let ind_xml = if left_indent_dxa > 0 {
                    format!(r#"<w:ind w:left="{left_indent_dxa}"/>"#)
                } else {
                    String::new()
                };
                for (idx, line_runs) in lines.iter().enumerate() {
                    let is_last = idx + 1 == num_lines;
                    let before = if idx == 0 { 20 } else { 0 };
                    let after = if is_last { 20 } else { 0 };
                    let keep_xml = if is_last && keep_last_with_next {
                        "<w:keepNext/>"
                    } else {
                        ""
                    };
                    let mark_rpr = line_runs
                        .first()
                        .and_then(|r| r.font_size_pt)
                        .map(|sz| {
                            let hp = ((sz * 2.0).round() as u32).clamp(8, 72);
                            crate::xml_helpers::format_mark_rpr_half_pt(hp)
                        })
                        .unwrap_or_default();
                    let _ = write!(
                        out,
                        r#"<w:p><w:pPr>{keep_xml}<w:spacing w:before="{before}" w:after="{after}" w:line="{line_dxa}" w:lineRule="{line_rule}"/>{ind_xml}{mark_rpr}</w:pPr>"#,
                    );
                    for run in line_runs {
                        self.render_text_run(run, out);
                    }
                    out.push_str("</w:p>");
                }
            }
            CodeBlockConstruct::ShadedBoxTable {
                bg_hex,
                border_color_hex,
                border_sz_eighths_pt,
                left_indent_dxa,
            } => {
                // Render as a 1-cell full-width shaded code box when fill or stroke is set.
                let ind_xml = if left_indent_dxa > 0 {
                    format!(r#"<w:tblInd w:w="{left_indent_dxa}" w:type="dxa"/>"#)
                } else {
                    String::new()
                };
                let borders_xml = if border_sz_eighths_pt > 0 {
                    format!(
                        r#"<w:tblBorders><w:top w:val="single" w:sz="{border_sz_eighths_pt}" w:space="0" w:color="{border_color_hex}"/><w:left w:val="single" w:sz="{border_sz_eighths_pt}" w:space="0" w:color="{border_color_hex}"/><w:bottom w:val="single" w:sz="{border_sz_eighths_pt}" w:space="0" w:color="{border_color_hex}"/><w:right w:val="single" w:sz="{border_sz_eighths_pt}" w:space="0" w:color="{border_color_hex}"/></w:tblBorders>"#
                    )
                } else {
                    r#"<w:tblBorders><w:top w:val="none" w:sz="0" w:space="0" w:color="auto"/><w:left w:val="none" w:sz="0" w:space="0" w:color="auto"/><w:bottom w:val="none" w:sz="0" w:space="0" w:color="auto"/><w:right w:val="none" w:sz="0" w:space="0" w:color="auto"/></w:tblBorders>"#.to_string()
                };
                let _ = write!(
                    out,
                    r#"<w:tbl><w:tblPr>{ind_xml}<w:tblW w:w="5000" w:type="pct"/>{borders_xml}</w:tblPr><w:tblGrid><w:gridCol w:w="9000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="pct"/><w:shd w:val="clear" w:color="auto" w:fill="{bg_hex}"/><w:tcMar><w:top w:w="100" w:type="dxa"/><w:left w:w="160" w:type="dxa"/><w:bottom w:w="100" w:type="dxa"/><w:right w:w="160" w:type="dxa"/></w:tcMar></w:tcPr>"#
                );

                let keep_xml = if keep_last_with_next {
                    "<w:keepNext/>"
                } else {
                    ""
                };
                if lines.is_empty() {
                    let _ = write!(out, r#"<w:p><w:pPr>{keep_xml}</w:pPr></w:p>"#);
                } else {
                    let after_in_box = if is_exact { 0 } else { 20 };
                    let num_lines = lines.len();
                    for (idx, line_runs) in lines.iter().enumerate() {
                        let line_keep = if idx + 1 == num_lines { keep_xml } else { "" };
                        let _ = write!(
                            out,
                            r#"<w:p><w:pPr>{line_keep}<w:spacing w:before="0" w:after="{after_in_box}" w:line="{line_dxa}" w:lineRule="{line_rule}"/></w:pPr>"#,
                        );
                        for run in line_runs {
                            self.render_text_run(run, out);
                        }
                        out.push_str("</w:p>");
                    }
                }

                out.push_str("</w:tc></w:tr></w:tbl>");
            }
        }
    }
}
