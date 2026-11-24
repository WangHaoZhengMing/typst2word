//! Styled text runs -> `TextRunConstruct` -> `<w:r>` with `<w:rPr>`.

use std::fmt::Write as _;

use word_construct::text_run::{RunFontSizeConstruct, TextRunConstruct};
use crate::writer::DocxWriter;
use word_construct::model::{DashPatternIr, ScriptKindIr, TextRunIr};
use crate::xml::escape_xml;

pub(crate) fn render_text_run_xml(run: &TextRunIr, out: &mut String) {
    render_text_run_xml_with_spacing(run, None, out);
}

pub(crate) fn render_text_run_rpr_xml(
    run: &TextRunIr,
    extra_char_spacing_dxa: Option<i32>,
    out: &mut String,
) {
    render_text_run_rpr_xml_in_style(run, extra_char_spacing_dxa, false, out);
}

pub(crate) fn render_text_run_rpr_xml_in_style(
    run: &TextRunIr,
    extra_char_spacing_dxa: Option<i32>,
    in_bold_style: bool,
    out: &mut String,
) {
    let construct = TextRunConstruct::solve_with_spacing(run, extra_char_spacing_dxa);
    out.push_str("<w:rPr>");
    if let Some(fam) = construct.font_family {
        let esc = escape_xml(fam);
        let hint_attr = construct.east_asia_hint.as_xml_attr();
        let _ = write!(
            out,
            r#"<w:rFonts w:ascii="{esc}" w:hAnsi="{esc}" w:eastAsia="{esc}" w:cs="{esc}"{hint_attr}/>"#
        );
    }
    if run.bold {
        out.push_str("<w:b/><w:bCs/>");
    } else if in_bold_style {
        // Suppress bold inheritance from `HeadingN` / `Title` paragraph styles when the run uses
        // a non-bold weight or an explicit weight-qualified face (e.g. `"Heiti SC Medium"`).
        out.push_str(r#"<w:b w:val="0"/><w:bCs w:val="0"/>"#);
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
    if let (Some(bdr), Some(sz)) = (&run.border, construct.border_sz_eighths_pt) {
        let val = DocxWriter::dash_to_w_border_val(&bdr.dash);
        // `w:space="0"` places `<w:bdr>` on the font's OS/2/hhea cell (which already provides
        // internal clearance above cap-height and below baseline, matching Word's native character
        // border) without inflating `w:lineRule="auto"` line height.
        let _ = write!(
            out,
            r#"<w:bdr w:val="{val}" w:sz="{sz}" w:space="0" w:color="{}"/>"#,
            bdr.color
        );
    }
    if let Some(hl) = &run.highlight {
        let _ = write!(
            out,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{hl}"/>"#
        );
    }
    if let Some(char_spacing_dxa) = construct.char_spacing_dxa {
        let _ = write!(out, r#"<w:spacing w:val="{char_spacing_dxa}"/>"#);
    }
    if let Some(pos_half_pts) = construct.position_half_pts {
        let _ = write!(out, r#"<w:position w:val="{pos_half_pts}"/>"#);
    }
    match construct.font_size {
        RunFontSizeConstruct::Inherited => {}
        RunFontSizeConstruct::StandardHalfPts(half_pts)
        | RunFontSizeConstruct::ConsolasInlineCodeCompensated { half_pts, .. } => {
            let _ = write!(
                out,
                r#"<w:sz w:val="{half_pts}"/><w:szCs w:val="{half_pts}"/>"#
            );
        }
    }
    match run.script {
        ScriptKindIr::Sub => out.push_str(r#"<w:vertAlign w:val="subscript"/>"#),
        ScriptKindIr::Super => out.push_str(r#"<w:vertAlign w:val="superscript"/>"#),
        ScriptKindIr::Normal => {}
    }
    if let Some(loc) = construct.locale {
        let bcp = escape_xml(&loc.to_word_lang_tag());
        let _ = write!(out, r#"<w:lang w:val="{bcp}" w:eastAsia="{bcp}"/>"#);
    }
    if let Some(outline) = construct.text_outline {
        let emu = ((outline.thickness_pt * 12700.0).round() as i64).max(3175);
        let _ = write!(
            out,
            r#"<w14:textOutline w14:w="{emu}"><w14:solidFill><w14:srgbClr w14:val="{}"/></w14:solidFill></w14:textOutline>"#,
            outline.color
        );
    }
    out.push_str("</w:rPr>");
}

pub(crate) fn render_text_run_xml_with_spacing(
    run: &TextRunIr,
    extra_char_spacing_dxa: Option<i32>,
    out: &mut String,
) {
    if run.text.is_empty() {
        return;
    }
    out.push_str("<w:r>");
    render_text_run_rpr_xml(run, extra_char_spacing_dxa, out);
    out.push_str("<w:t xml:space=\"preserve\">");
    out.push_str(&escape_xml(&run.text));
    out.push_str("</w:t></w:r>");
}

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_text_run(&self, run: &TextRunIr, out: &mut String) {
        if run.text.is_empty() {
            return;
        }
        out.push_str("<w:r>");
        render_text_run_rpr_xml_in_style(run, None, self.in_bold_style, out);
        out.push_str("<w:t xml:space=\"preserve\">");
        out.push_str(&escape_xml(&run.text));
        out.push_str("</w:t></w:r>");
    }
}
