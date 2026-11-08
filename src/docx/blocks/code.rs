//! `raw(block: true)` -> syntax-colored code paragraphs (optionally boxed).

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{StrokeIr, TextRunIr};

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_code_block(
        &self,
        _lang: Option<&str>,
        lines: &[Vec<TextRunIr>],
        fill: Option<&str>,
        stroke: Option<&StrokeIr>,
        out: &mut String,
    ) {
        if fill.is_none() && stroke.is_none() {
            // Standard unboxed Typst `raw(block: true)`: render lines directly with exact syntax colors.
            let num_lines = lines.len();
            for (idx, line_runs) in lines.iter().enumerate() {
                let before = if idx == 0 { 80 } else { 0 };
                let after = if idx + 1 == num_lines { 100 } else { 15 };
                let _ = write!(
                    out,
                    r#"<w:p><w:pPr><w:spacing w:before="{before}" w:after="{after}" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                );
                for run in line_runs {
                    let mut code_run = run.clone();
                    code_run.is_code = true;
                    if code_run.font_family.is_none() {
                        code_run.font_family = Some("Consolas".to_string());
                    }
                    self.render_text_run(&code_run, out);
                }
                out.push_str("</w:p>");
            }
            return;
        }

        let bg = fill.unwrap_or("F6F8FA");
        let bdr_color = stroke.map(|s| s.color.as_str()).unwrap_or("D0D7DE");
        // Render as a 1-cell full-width shaded code box when fill or stroke is set.
        let _ = write!(
            out,
            r#"<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblBorders><w:top w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/><w:left w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/><w:bottom w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/><w:right w:val="single" w:sz="6" w:space="0" w:color="{bdr_color}"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="9000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="pct"/><w:shd w:val="clear" w:color="auto" w:fill="{bg}"/><w:tcMar><w:top w:w="120" w:type="dxa"/><w:left w:w="160" w:type="dxa"/><w:bottom w:w="120" w:type="dxa"/><w:right w:w="160" w:type="dxa"/></w:tcMar></w:tcPr>"#
        );

        if lines.is_empty() {
            out.push_str("<w:p/>");
        } else {
            for line_runs in lines {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="20" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                );
                for run in line_runs {
                    let mut code_run = run.clone();
                    code_run.is_code = true;
                    if code_run.font_family.is_none() {
                        code_run.font_family = Some("Consolas".to_string());
                    }
                    self.render_text_run(&code_run, out);
                }
                out.push_str("</w:p>");
            }
        }

        out.push_str("</w:tc></w:tr></w:tbl>");
        out.push_str("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"60\"/></w:pPr></w:p>");
    }
}
