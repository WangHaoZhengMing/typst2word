//! `raw(block: true)` -> `CodeBlockConstruct` -> syntax-colored code paragraphs (optionally boxed).

use std::fmt::Write as _;

use word_construct::code::CodeBlockConstruct;
use crate::writer::DocxWriter;
use ir::{StrokeIr, TextRunIr};

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_code_block(
        &self,
        lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        out: &mut String,
    ) {
        self.render_code_block_with_layout(lang, lines, fill, stroke, 0, false, out);
    }

    pub(crate) fn render_code_block_indented(
        &self,
        lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        left_indent_dxa: usize,
        out: &mut String,
    ) {
        self.render_code_block_with_layout(lang, lines, fill, stroke, left_indent_dxa, false, out);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_code_block_with_layout(
        &self,
        _lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        left_indent_dxa: usize,
        keep_last_with_next: bool,
        out: &mut String,
    ) {
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
                    let before = if idx == 0 { 40 } else { 0 };
                    let after = if is_last { 100 } else { 0 };
                    let keep_xml = if is_last && keep_last_with_next {
                        "<w:keepNext/>"
                    } else {
                        ""
                    };
                    let _ = write!(
                        out,
                        r#"<w:p><w:pPr>{keep_xml}<w:spacing w:before="{before}" w:after="{after}" w:line="276" w:lineRule="auto"/>{ind_xml}</w:pPr>"#,
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
            } => {
                // Render as a 1-cell full-width shaded code box when fill or stroke is set.
                let _ = write!(
                    out,
                    r#"<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblBorders><w:top w:val="single" w:sz="6" w:space="0" w:color="{border_color_hex}"/><w:left w:val="single" w:sz="6" w:space="0" w:color="{border_color_hex}"/><w:bottom w:val="single" w:sz="6" w:space="0" w:color="{border_color_hex}"/><w:right w:val="single" w:sz="6" w:space="0" w:color="{border_color_hex}"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="9000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="pct"/><w:shd w:val="clear" w:color="auto" w:fill="{bg_hex}"/><w:tcMar><w:top w:w="120" w:type="dxa"/><w:left w:w="160" w:type="dxa"/><w:bottom w:w="120" w:type="dxa"/><w:right w:w="160" w:type="dxa"/></w:tcMar></w:tcPr>"#
                );

                if lines.is_empty() {
                    out.push_str("<w:p/>");
                } else {
                    for line_runs in lines {
                        out.push_str(
                            r#"<w:p><w:pPr><w:spacing w:before="0" w:after="20" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                        );
                        for run in line_runs {
                            self.render_text_run(run, out);
                        }
                        out.push_str("</w:p>");
                    }
                }

                out.push_str("</w:tc></w:tr></w:tbl>");
                let keep_xml = if keep_last_with_next {
                    "<w:keepNext/>"
                } else {
                    ""
                };
                let _ = write!(
                    out,
                    r#"<w:p><w:pPr>{keep_xml}<w:spacing w:before="0" w:after="60"/></w:pPr></w:p>"#
                );
            }
        }
    }
}
