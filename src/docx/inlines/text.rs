//! Styled text runs -> `<w:r>` with `<w:rPr>`.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{DashPatternIr, ScriptKindIr, TextRunIr};
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_text_run(&self, run: &TextRunIr, out: &mut String) {
        if run.text.is_empty() {
            return;
        }
        out.push_str("<w:r><w:rPr>");
        if let Some(fam) = &run.font_family {
            let esc = escape_xml(fam);
            if fam == "Consolas" {
                out.push_str(
                    r#"<w:rFonts w:ascii="Menlo" w:hAnsi="Menlo" w:eastAsia="Heiti SC" w:cs="Consolas"/>"#,
                );
            } else if fam == "New Computer Modern" {
                out.push_str(
                    r#"<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:eastAsia="Songti SC" w:cs="Times New Roman"/>"#,
                );
            } else if fam == "Songti SC" || fam == "Kaiti SC" || fam == "Heiti SC" {
                let _ = write!(
                    out,
                    r#"<w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:eastAsia="{esc}" w:cs="Times New Roman"/>"#
                );
            } else {
                let ea_esc = if fam == "Libertinus Serif" {
                    "Heiti SC".to_string()
                } else {
                    esc.clone()
                };
                let _ = write!(
                    out,
                    r#"<w:rFonts w:ascii="{esc}" w:hAnsi="{esc}" w:eastAsia="{ea_esc}" w:cs="{esc}"/>"#
                );
            }
        }
        if run.bold {
            out.push_str("<w:b/><w:bCs/>");
        }
        if run.italic {
            out.push_str("<w:i/><w:iCs/>");
        }
        if run.smallcaps {
            out.push_str("<w:smallCaps/>");
        }
        if let Some(ul) = &run.underline {
            let u_val = match ul.dash {
                DashPatternIr::Dotted => "dotted",
                DashPatternIr::Dashed | DashPatternIr::Custom => "dash",
                DashPatternIr::DashDot => "dotDash",
                DashPatternIr::DashDotDot => "dotDotDash",
                DashPatternIr::Solid => "single",
            };
            let _ = write!(out, r#"<w:u w:val="{u_val}" w:color="{}"/>"#, ul.color);
        }
        if run.strike.is_some() {
            out.push_str("<w:strike/>");
        }
        if let Some(color) = &run.color {
            let _ = write!(out, r#"<w:color w:val="{color}"/>"#);
        }
        if let Some(hl) = &run.highlight {
            let _ = write!(
                out,
                r#"<w:shd w:val="clear" w:color="auto" w:fill="{hl}"/>"#
            );
        }
        if let Some(sz_pt) = run.font_size_pt {
            let half_pts = if run.is_code && (sz_pt - 8.8).abs() < 0.15 {
                17
            } else {
                ((sz_pt * 2.0).round() as u32).clamp(4, 288)
            };
            let _ = write!(
                out,
                r#"<w:sz w:val="{half_pts}"/><w:szCs w:val="{half_pts}"/>"#
            );
        }
        match run.script {
            ScriptKindIr::Sub => out.push_str(r#"<w:vertAlign w:val="subscript"/>"#),
            ScriptKindIr::Super => out.push_str(r#"<w:vertAlign w:val="superscript"/>"#),
            ScriptKindIr::Normal => {}
        }
        out.push_str("</w:rPr><w:t xml:space=\"preserve\">");
        out.push_str(&escape_xml(&run.text));
        out.push_str("</w:t></w:r>");
    }
}
