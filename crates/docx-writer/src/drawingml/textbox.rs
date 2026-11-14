//! Shape / label body blocks -> `<w:txbxContent>` paragraphs, tables, and runs.

use crate::inlines::text::render_text_run_xml;
use crate::math::render_equation_omml;
use crate::writer::DocxWriter;
use word_construct::model::{BlockNode, HAlignIr, InlineNode, TextRunIr};
use word_construct::units::pt_to_dxa;

pub(crate) fn render_textbox_blocks(writer: &mut DocxWriter<'_>, blocks: &[BlockNode]) -> String {
    let mut out = String::new();
    let len = blocks.len();
    for (idx, block) in blocks.iter().enumerate() {
        match block {
            BlockNode::Paragraph {
                id, content, style, ..
            } => {
                let after_dxa = if idx + 1 < len && style.space_after_pt > 0.5 {
                    pt_to_dxa(style.space_after_pt)
                } else {
                    0
                };
                let line_dxa = writer.doc.resolve_paragraph_line_dxa(
                    *id,
                    content,
                    style.line_spacing_pt,
                );
                out.push_str(&render_inline_nodes_as_txbx_para_with_line_dxa(
                    content,
                    style.align,
                    line_dxa,
                    after_dxa,
                ));
            }
            BlockNode::Heading { content, align, .. }
            | BlockNode::Title { content, align, .. } => {
                let after_dxa = if idx + 1 < len { 120 } else { 0 };
                let line_dxa = writer.doc.resolve_paragraph_line_dxa(
                    word_construct::model::NodeId::UNSET,
                    content,
                    None,
                );
                out.push_str(&render_inline_nodes_as_txbx_para_with_line_dxa(
                    content,
                    *align,
                    line_dxa,
                    after_dxa,
                ));
            }
            BlockNode::EquationBlock(eq) => {
                let after_dxa = if idx + 1 < len { 80 } else { 0 };
                out.push_str(&format!(
                    r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="110" w:after="{after_dxa}"/></w:pPr>"#,
                ));
                render_equation_omml(eq, &mut out);
                out.push_str("</w:p>");
            }
            BlockNode::Container(container) => {
                out.push_str(&render_textbox_blocks(writer, &container.body));
            }
            BlockNode::Table(tbl) => {
                writer.render_table(tbl, &mut out);
            }
            BlockNode::List { items, .. } => {
                for item in items {
                    out.push_str(&render_textbox_blocks(writer, &item.blocks));
                }
            }
            BlockNode::Enum { items, .. } => {
                for item in items {
                    out.push_str(&render_textbox_blocks(writer, &item.blocks));
                }
            }
            BlockNode::CodeBlock { lines, .. } => {
                for line_runs in lines {
                    let inlines: Vec<InlineNode> = line_runs
                        .iter()
                        .cloned()
                        .map(InlineNode::Text)
                        .collect();
                    out.push_str(&render_inline_nodes_as_txbx_para(
                        &inlines,
                        HAlignIr::Left,
                        None,
                    ));
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
                    None,
                ));
            }
            _ => {}
        }
    }
    if out.is_empty() {
        out.push_str(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr></w:p>"#);
    } else if !out.ends_with("</w:p>") {
        out.push_str(
            r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p>"#,
        );
    }
    out
}

pub(crate) fn render_inline_nodes_as_txbx_para(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_spacing_pt: Option<f64>,
) -> String {
    render_inline_nodes_as_txbx_para_with_after(inlines, align, line_spacing_pt, 0)
}

pub(crate) fn render_inline_nodes_as_txbx_para_with_after(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_spacing_pt: Option<f64>,
    after_dxa: i64,
) -> String {
    let line_dxa = word_construct::paragraph::compute_line_spacing_dxa(inlines, line_spacing_pt);
    render_inline_nodes_as_txbx_para_with_line_dxa(inlines, align, line_dxa, after_dxa)
}

pub(crate) fn render_inline_nodes_as_txbx_para_with_line_dxa(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_dxa: i64,
    after_dxa: i64,
) -> String {
    let jc = match align {
        HAlignIr::Left => "left",
        HAlignIr::Center => "center",
        HAlignIr::Right => "right",
        HAlignIr::Justify => "both",
    };
    let mut runs_xml = String::new();
    render_inline_nodes_to_wml(inlines, &mut runs_xml);
    format!(
        r#"<w:p><w:pPr><w:jc w:val="{jc}"/><w:spacing w:before="0" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/></w:pPr>{runs_xml}</w:p>"#
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
