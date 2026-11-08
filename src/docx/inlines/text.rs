//! Styled text runs -> `<w:r>` with `<w:rPr>`.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{DashPatternIr, ScriptKindIr, TextRunIr};
use crate::xml::escape_xml;

pub(crate) fn render_text_run_xml(run: &TextRunIr, out: &mut String) {
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
        } else if fam == "PingFang SC" {
            out.push_str(
                r#"<w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="Heiti SC" w:cs="Arial"/>"#,
            );
            let ascii_count = run
                .text
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .count();
            if ascii_count > 0 {
                let total_chars = run.text.chars().count().max(1);
                let twips = ((ascii_count * 9 + total_chars / 2) / total_chars).clamp(1, 9);
                let _ = write!(out, r#"<w:spacing w:val="{twips}"/>"#);
            }
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
    if let Some(bdr) = &run.border {
        let sz = ((bdr.thickness_pt * 8.0).round() as u32).clamp(2, 96);
        let val = DocxBuilder::dash_to_w_border_val(&bdr.dash);
        let _ = write!(
            out,
            r#"<w:bdr w:val="{val}" w:sz="{sz}" w:space="2" w:color="{}"/>"#,
            bdr.color
        );
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

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_text_run(&self, run: &TextRunIr, out: &mut String) {
        render_text_run_xml(run, out);
    }
}
