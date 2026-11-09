//! Shape / label body blocks -> `<w:txbxContent>` paragraphs and runs.

use crate::inlines::text::render_text_run_xml;
use crate::math::render_equation_omml;
use ir::{BlockNode, HAlignIr, InlineNode, TextRunIr};

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
                out.push_str(
                    r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="0" w:after="0"/></w:pPr>"#,
                );
                render_equation_omml(eq, &mut out);
                out.push_str("</w:p>");
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
    let line_dxa = word_construct::paragraph::compute_line_spacing_dxa(inlines, None);
    let mut runs_xml = String::new();
    render_inline_nodes_to_wml(inlines, &mut runs_xml);
    format!(
        r#"<w:p><w:pPr><w:jc w:val="{jc}"/><w:spacing w:before="0" w:after="0" w:line="{line_dxa}" w:lineRule="auto"/></w:pPr>{runs_xml}</w:p>"#
    )
}

pub(crate) fn render_inline_nodes_to_wml(inlines: &[InlineNode], out: &mut String) {
    for node in inlines {
        match node {
            InlineNode::Text(run) => {
                render_text_run_xml(run, out);
            }
            InlineNode::EquationInline(eq) => {
                render_equation_omml(eq, out);
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
                out.push_str(&format!(
                    r#"<w:r><w:t xml:space="preserve">{s}</w:t></w:r>"#
                ));
            }
            InlineNode::UnsupportedInline(u) => {
                let badge_run = TextRunIr {
                    text: u.display_badge(),
                    color: Some("B91C1C".to_string()),
                    bold: true,
                    ..Default::default()
                };
                render_text_run_xml(&badge_run, out);
            }
            _ => {}
        }
    }
}
