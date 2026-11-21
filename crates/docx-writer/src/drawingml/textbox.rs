//! Shape / label body blocks -> `<w:txbxContent>` paragraphs, tables, and runs.

use crate::inlines::text::render_text_run_xml;
use crate::math::render_equation_omml_with_font;
use crate::writer::DocxWriter;
use word_construct::model::{BlockNode, HAlignIr, InlineNode, TextRunIr};
use word_construct::shape::FirstParaExactLayout;
use word_construct::units::pt_to_dxa;

pub(crate) fn render_textbox_blocks(writer: &mut DocxWriter<'_>, blocks: &[BlockNode]) -> String {
    render_textbox_blocks_with_first_override(writer, blocks, None)
}

pub(crate) fn render_textbox_blocks_with_first_override(
    writer: &mut DocxWriter<'_>,
    blocks: &[BlockNode],
    first_para_exact: Option<FirstParaExactLayout>,
) -> String {
    let mut out = String::new();
    let len = blocks.len();
    for (idx, block) in blocks.iter().enumerate() {
        match block {
            BlockNode::Paragraph {
                id, content, style, ..
            } => {
                if idx == 0
                    && let Some(exact) = first_para_exact
                {
                    let shifted_buf;
                    let eff_content: &[InlineNode] = match exact.run_position_half_pt {
                        Some(hp) if hp != 0 => {
                            shifted_buf = shift_inline_runs_half_pt(content, hp);
                            &shifted_buf
                        }
                        _ => content,
                    };
                    out.push_str(&render_inline_nodes_as_txbx_para_with_spacing(
                        eff_content,
                        style.align,
                        exact.line_dxa,
                        true,
                        exact.after_dxa,
                        writer.doc.math_font_family.as_deref(),
                    ));
                    continue;
                }
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
                    writer.doc.math_font_family.as_deref(),
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
                    writer.doc.math_font_family.as_deref(),
                ));
            }
            BlockNode::EquationBlock(eq) => {
                let after_dxa = if idx + 1 < len { 80 } else { 0 };
                out.push_str(&format!(
                    r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="110" w:after="{after_dxa}"/></w:pPr>"#,
                ));
                render_equation_omml_with_font(eq, writer.doc.math_font_family.as_deref(), &mut out);
                out.push_str("</w:p>");
            }
            BlockNode::Container(container) => {
                out.push_str(&render_textbox_blocks(writer, &container.body));
            }
            BlockNode::Table(tbl) => {
                writer.render_table(tbl, &mut out);
            }
            BlockNode::List {
                items,
                tight,
                level,
                spacing_pt,
                indent_pt,
                hanging_pt,
                ..
            } => {
                // Delegate `<w:txbxContent>` bullet lists to the native list serializer so
                // `<w:numPr>` markers and hanging indents are preserved inside rounded callout boxes.
                let trailing_after = (idx + 1 == len).then_some(0);
                writer.render_list_in_box(
                    items,
                    *tight,
                    *level,
                    *spacing_pt,
                    *indent_pt,
                    *hanging_pt,
                    idx == 0,
                    trailing_after,
                    &mut out,
                );
            }
            BlockNode::Enum {
                items,
                tight,
                level,
                spacing_pt,
                indent_pt,
                hanging_pt,
                ..
            } => {
                // Delegate `<w:txbxContent>` numbered lists to the native enum serializer so
                // decimal markers (`1.`, `2.`, ...) and nested indents are preserved inside shapes.
                let trailing_after = (idx + 1 == len).then_some(0);
                writer.render_enum_in_box(
                    items,
                    *tight,
                    *level,
                    *spacing_pt,
                    *indent_pt,
                    *hanging_pt,
                    idx == 0,
                    trailing_after,
                    &mut out,
                );
            }
            BlockNode::CodeBlock { id, lines, .. } => {
                if idx == 0
                    && lines.len() == 1
                    && let Some(exact) = first_para_exact
                {
                    let raw_inlines: Vec<InlineNode> =
                        lines[0].iter().cloned().map(InlineNode::Text).collect();
                    let shifted_buf;
                    let eff_content: &[InlineNode] = match exact.run_position_half_pt {
                        Some(hp) if hp != 0 => {
                            shifted_buf = shift_inline_runs_half_pt(&raw_inlines, hp);
                            &shifted_buf
                        }
                        _ => &raw_inlines,
                    };
                    out.push_str(&render_inline_nodes_as_txbx_para_with_spacing(
                        eff_content,
                        HAlignIr::Left,
                        exact.line_dxa,
                        true,
                        exact.after_dxa,
                        writer.doc.math_font_family.as_deref(),
                    ));
                    continue;
                }
                let (line_dxa, is_exact) = writer.doc.resolve_code_block_line_spacing(*id, lines);
                for line_runs in lines {
                    let inlines: Vec<InlineNode> = line_runs
                        .iter()
                        .cloned()
                        .map(InlineNode::Text)
                        .collect();
                    out.push_str(&render_inline_nodes_as_txbx_para_with_spacing(
                        &inlines,
                        HAlignIr::Left,
                        line_dxa,
                        is_exact,
                        0,
                        writer.doc.math_font_family.as_deref(),
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
                    writer.doc.math_font_family.as_deref(),
                ));
            }
            _ => {}
        }
    }
    if out.is_empty() {
        out.push_str(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr></w:p>"#);
    } else if !out.ends_with("</w:p>") {
        crate::xml_helpers::write_exact_spacer_p(0, 20, false, &mut out);
    }
    out
}

pub(crate) fn render_inline_nodes_as_txbx_para(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_spacing_pt: Option<f64>,
    math_font: Option<&str>,
) -> String {
    render_inline_nodes_as_txbx_para_with_after(inlines, align, line_spacing_pt, 0, math_font)
}

pub(crate) fn render_inline_nodes_as_txbx_para_with_after(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_spacing_pt: Option<f64>,
    after_dxa: i64,
    math_font: Option<&str>,
) -> String {
    let line_dxa = word_construct::paragraph::compute_line_spacing_dxa(inlines, line_spacing_pt);
    render_inline_nodes_as_txbx_para_with_line_dxa(inlines, align, line_dxa, after_dxa, math_font)
}

pub(crate) fn render_inline_nodes_as_txbx_para_with_line_dxa(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_dxa: i64,
    after_dxa: i64,
    math_font: Option<&str>,
) -> String {
    render_inline_nodes_as_txbx_para_with_spacing(
        inlines, align, line_dxa, false, after_dxa, math_font,
    )
}

pub(crate) fn render_inline_nodes_as_txbx_para_with_spacing(
    inlines: &[InlineNode],
    align: HAlignIr,
    line_dxa: i64,
    line_rule_exact: bool,
    after_dxa: i64,
    math_font: Option<&str>,
) -> String {
    let jc = match align {
        HAlignIr::Left => "left",
        HAlignIr::Center => "center",
        HAlignIr::Right => "right",
        HAlignIr::Justify => "both",
    };
    let line_rule = if line_rule_exact { "exact" } else { "auto" };
    // Size the `<w:pPr><w:rPr>` paragraph mark to match the line's font size so Word never
    // inflates sub-11pt code/label lines inside `<w:txbxContent>` to the 11pt Normal mark.
    let mark_rpr = word_construct::paragraph::paragraph_mark_rpr_sz_half_pt(inlines)
        .map(crate::xml_helpers::format_mark_rpr_half_pt)
        .unwrap_or_default();
    let mut runs_xml = String::new();
    render_inline_nodes_to_wml(inlines, math_font, &mut runs_xml);
    format!(
        r#"<w:p><w:pPr><w:jc w:val="{jc}"/><w:spacing w:before="0" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="{line_rule}"/>{mark_rpr}</w:pPr>{runs_xml}</w:p>"#
    )
}

pub(crate) fn render_inline_nodes_to_wml(
    inlines: &[InlineNode],
    math_font: Option<&str>,
    out: &mut String,
) {
    for node in inlines {
        match node {
            InlineNode::Text(run) => {
                render_text_run_xml(run, out);
            }
            InlineNode::EquationInline(eq) => {
                render_equation_omml_with_font(eq, math_font, out);
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. } => {
                render_inline_nodes_to_wml(content, math_font, out);
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

fn shift_inline_runs_half_pt(inlines: &[InlineNode], delta_half_pt: i32) -> Vec<InlineNode> {
    let delta_pt = delta_half_pt as f64 * 0.5;
    inlines
        .iter()
        .map(|inl| match inl {
            InlineNode::Text(r) => {
                let mut shifted = r.clone();
                shifted.baseline_shift_pt = Some(shifted.baseline_shift_pt.unwrap_or(0.0) + delta_pt);
                InlineNode::Text(shifted)
            }
            InlineNode::Link { target, content } => InlineNode::Link {
                target: target.clone(),
                content: shift_inline_runs_half_pt(content, delta_half_pt),
            },
            InlineNode::Ref { target, content } => InlineNode::Ref {
                target: target.clone(),
                content: shift_inline_runs_half_pt(content, delta_half_pt),
            },
            InlineNode::Citation { keys, content } => InlineNode::Citation {
                keys: keys.clone(),
                content: shift_inline_runs_half_pt(content, delta_half_pt),
            },
            other => other.clone(),
        })
        .collect()
}
