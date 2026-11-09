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
        let fam_lower = fam.to_ascii_lowercase();
        let is_cjk_font = fam_lower.contains("song")
            || fam_lower.contains("hei")
            || fam_lower.contains("kai")
            || fam_lower.contains("fang")
            || fam_lower.contains("pingfang")
            || fam_lower.contains("sim")
            || fam_lower.contains("yahei")
            || fam_lower.contains("dengxian")
            || fam_lower.contains("宋体")
            || fam_lower.contains("黑体")
            || fam_lower.contains("楷体")
            || fam_lower.contains("仿宋");
        let has_ea_symbol = is_cjk_font
            && run
                .text
                .chars()
                .any(|c| ('\u{2000}'..='\u{2BFF}').contains(&c));
        let hint_attr = if has_ea_symbol {
            r#" w:hint="eastAsia""#
        } else {
            ""
        };
        // IMPORTANT: Always record and emit the exact `font_family` extracted from Typst
        // (`w:ascii`, `w:hAnsi`, `w:eastAsia`, `w:cs`) without substituting fallback fonts
        // (e.g., never replace `"PingFang SC"` with `"Arial"` / `"Heiti SC"`, or `"Songti SC"`
        // with `"Times New Roman"`). Even if Microsoft Word on macOS cannot index system-reserved
        // fonts like `PingFangUI.ttc`, that is a host application limitation; the generated `.docx`
        // must faithfully preserve the document's true font metadata.
        let _ = write!(
            out,
            r#"<w:rFonts w:ascii="{esc}" w:hAnsi="{esc}" w:eastAsia="{esc}" w:cs="{esc}"{hint_attr}/>"#
        );
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
        let is_consolas_fallback = run.is_code
            && matches!(
                run.font_family.as_deref(),
                Some("Consolas" | "DejaVu Sans Mono")
            )
            && (sz_pt - 8.8).abs() < 0.15;
        let half_pts = if is_consolas_fallback {
            // DejaVu Sans Mono (1233/2048 em at 8.8pt = 5.298pt/char) mapped to
            // Consolas (1126/2048 em): 19 half-points (9.5pt = 5.223pt/char) + 1 dxa
            // (+0.05pt/char) matches Typst's inline raw advance width to within 0.025pt/char.
            out.push_str(r#"<w:spacing w:val="1"/>"#);
            19
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
