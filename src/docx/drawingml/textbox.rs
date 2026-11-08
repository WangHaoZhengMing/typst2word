//! Shape / label body blocks -> `<w:txbxContent>` paragraphs and runs.

use crate::ir::{BlockNode, HAlignIr, InlineNode, ScriptKindIr, TextRunIr};
use crate::xml::escape_xml_attr;

pub(crate) fn render_textbox_blocks(blocks: &[BlockNode]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, style, .. } => {
                out.push_str(&render_inline_nodes_as_txbx_para(content, style.align));
            }
            BlockNode::Heading { content, align, .. } | BlockNode::Title { content, align } => {
                out.push_str(&render_inline_nodes_as_txbx_para(content, *align));
            }
            BlockNode::EquationBlock(eq) => {
                out.push_str(&format!(
                    r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="0" w:after="0"/></w:pPr>{}</w:p>"#,
                    eq.omml_xml
                ));
            }
            BlockNode::List { items, .. } => {
                for item in items {
                    out.push_str(&render_textbox_blocks(&item.blocks));
                }
            }
            BlockNode::Enum { items, .. } => {
                for item in items {
                    out.push_str(&render_textbox_blocks(&item.blocks));
                }
            }
            BlockNode::UnsupportedBlock(u) => {
                let run = TextRunIr {
                    text: u.display_badge(),
                    color: Some("B91C1C".to_string()),
                    bold: true,
                    ..Default::default()
                };
                out.push_str(&render_inline_nodes_as_txbx_para(
                    &[InlineNode::Text(run)],
                    HAlignIr::Left,
                ));
            }
            _ => {}
        }
    }
    if out.is_empty() {
        out.push_str(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr></w:p>"#);
    }
    out
}

pub(crate) fn render_inline_nodes_as_txbx_para(inlines: &[InlineNode], align: HAlignIr) -> String {
    let jc = match align {
        HAlignIr::Left => "left",
        HAlignIr::Center => "center",
        HAlignIr::Right => "right",
        HAlignIr::Justify => "both",
    };
    let mut runs_xml = String::new();
    render_inline_nodes_to_wml(inlines, &mut runs_xml);
    format!(
        r#"<w:p><w:pPr><w:jc w:val="{jc}"/><w:spacing w:before="0" w:after="0"/></w:pPr>{runs_xml}</w:p>"#
    )
}

pub(crate) fn render_inline_nodes_to_wml(inlines: &[InlineNode], out: &mut String) {
    for node in inlines {
        match node {
            InlineNode::Text(run) => {
                out.push_str(&render_text_run_wml(run));
            }
            InlineNode::EquationInline(eq) => {
                out.push_str(&eq.omml_xml);
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                render_inline_nodes_to_wml(content, out);
            }
            InlineNode::LineBreak => {
                out.push_str("<w:r><w:br/></w:r>");
            }
            InlineNode::Tab => {
                out.push_str("<w:r><w:tab/></w:r>");
            }
            InlineNode::HorizontalSpace { width_pt } => {
                let spaces = ((width_pt / 3.5).round() as usize).clamp(1, 8);
                let s = "\u{00A0}".repeat(spaces);
                out.push_str(&format!(r#"<w:r><w:t xml:space="preserve">{s}</w:t></w:r>"#));
            }
            InlineNode::UnsupportedInline(u) => {
                let badge_run = TextRunIr {
                    text: u.display_badge(),
                    color: Some("B91C1C".to_string()),
                    bold: true,
                    ..Default::default()
                };
                out.push_str(&render_text_run_wml(&badge_run));
            }
            _ => {}
        }
    }
}

pub(crate) fn render_text_run_wml(run: &TextRunIr) -> String {
    if run.text.is_empty() {
        return String::new();
    }
    let mut rpr = String::new();
    if let Some(ref font) = run.font_family {
        let esc = escape_xml_attr(font);
        if font == "Consolas" {
            rpr.push_str(
                r#"<w:rFonts w:ascii="Menlo" w:hAnsi="Menlo" w:eastAsia="Heiti SC" w:cs="Consolas"/>"#,
            );
        } else {
            let ea = if font == "Libertinus Serif" || font == "New Computer Modern" {
                "Heiti SC".to_string()
            } else {
                esc.clone()
            };
            rpr.push_str(&format!(
                r#"<w:rFonts w:ascii="{esc}" w:hAnsi="{esc}" w:eastAsia="{ea}" w:cs="{esc}"/>"#
            ));
        }
    }
    if run.bold {
        rpr.push_str("<w:b/><w:bCs/>");
    }
    if run.italic {
        rpr.push_str("<w:i/><w:iCs/>");
    }
    if run.smallcaps {
        rpr.push_str("<w:smallCaps/>");
    }
    if run.underline.is_some() {
        rpr.push_str(r#"<w:u w:val="single"/>"#);
    }
    if run.strike.is_some() {
        rpr.push_str("<w:strike/>");
    }
    if let Some(ref color) = run.color {
        rpr.push_str(&format!(r#"<w:color w:val="{color}"/>"#));
    }
    if let Some(ref hl) = run.highlight {
        rpr.push_str(&format!(
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{hl}"/>"#
        ));
    }
    if let Some(sz_pt) = run.font_size_pt {
        let half_pts = (sz_pt * 2.0).round() as u32;
        if half_pts > 0 {
            rpr.push_str(&format!(
                r#"<w:sz w:val="{half_pts}"/><w:szCs w:val="{half_pts}"/>"#
            ));
        }
    }
    match run.script {
        ScriptKindIr::Normal => {}
        ScriptKindIr::Sub => rpr.push_str(r#"<w:vertAlign w:val="subscript"/>"#),
        ScriptKindIr::Super => rpr.push_str(r#"<w:vertAlign w:val="superscript"/>"#),
    }

    let rpr_block = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };

    format!(
        r#"<w:r>{rpr_block}<w:t xml:space="preserve">{}</w:t></w:r>"#,
        escape_xml_attr(&run.text)
    )
}
