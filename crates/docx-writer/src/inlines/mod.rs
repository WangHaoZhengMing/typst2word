//! Inline-level rendering: `InlineNode` -> runs.
//!
//! `render_inline` only dispatches; specific inline kinds live in sibling files.

mod bookmark;
mod footnote;
mod image;
mod link;
pub(crate) mod text;
mod unsupported;

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use crate::drawingml::{
    cetz_canvas_to_drawingml_run, shape_to_drawingml_run, shape_to_inline_drawingml_run,
};
use word_construct::model::InlineNode;

impl<'a> DocxWriter<'a> {
    // =========================================================================
    // Inline Rendering
    // =========================================================================
    pub(crate) fn render_inlines(&mut self, inlines: &[InlineNode], out: &mut String) {
        for inline in inlines {
            self.render_inline(inline, out);
        }
    }

    pub(crate) fn render_inline(&mut self, inline: &InlineNode, out: &mut String) {
        match inline {
            InlineNode::Field { kind, format, content } => {
                // In Word field syntax, general format switches (`\* ...`) must precede
                // field-specific switches such as `PAGEREF`'s `\h` hyperlink switch.
                let mut instruction = match kind {
                    word_construct::model::FieldKindIr::Page => "PAGE".to_string(),
                    word_construct::model::FieldKindIr::NumPages => "NUMPAGES".to_string(),
                    word_construct::model::FieldKindIr::SectionPages => "SECTIONPAGES".to_string(),
                    word_construct::model::FieldKindIr::PageRef { bookmark } => {
                        format!("PAGEREF {bookmark}")
                    }
                };
                if let Some(format) = format {
                    let _ = write!(instruction, " \\* {format}");
                }
                if matches!(kind, word_construct::model::FieldKindIr::PageRef { .. }) {
                    instruction.push_str(" \\h");
                }
                // Header/footer `PAGE` and `NUMPAGES` fields recalculate silently when marked
                // `w:dirty="true"`, whereas body `PAGEREF` fields marked `w:dirty="true"` trigger
                // Word's modal "This document contains fields that may refer to other files"
                // prompt on open. `PageRef` already carries Typst's evaluated page text in
                // `content` and remains live for hyperlink navigation (`\h`) and manual update.
                let dirty_attr = if matches!(kind, word_construct::model::FieldKindIr::PageRef { .. }) {
                    ""
                } else {
                    r#" w:dirty="true""#
                };
                // Emit complex field (`begin`/`instrText`/`separate`/`end`) with `\* CHARFORMAT`
                // and the cached run's `<w:rPr>` on `<w:instrText>` so Word preserves the exact
                // font family, size, weight, and color when recalculating `PAGE`/`NUMPAGES`
                // instead of falling back to the paragraph default style.
                let template_run = content.iter().find_map(|inl| match inl {
                    InlineNode::Text(r) => Some(r),
                    _ => None,
                });
                if let Some(tpl) = template_run {
                    instruction.push_str(" \\* CHARFORMAT");
                    out.push_str("<w:r>");
                    text::render_text_run_rpr_xml(tpl, None, out);
                    let _ = write!(out, r#"<w:fldChar w:fldCharType="begin"{dirty_attr}/></w:r>"#);
                    out.push_str("<w:r>");
                    text::render_text_run_rpr_xml(tpl, None, out);
                    let _ = write!(
                        out,
                        r#"<w:instrText xml:space="preserve"> {} </w:instrText></w:r>"#,
                        crate::xml::escape_xml(&instruction)
                    );
                    out.push_str("<w:r>");
                    text::render_text_run_rpr_xml(tpl, None, out);
                    out.push_str(r#"<w:fldChar w:fldCharType="separate"/></w:r>"#);
                    self.render_inlines(content, out);
                    out.push_str("<w:r>");
                    text::render_text_run_rpr_xml(tpl, None, out);
                    out.push_str(r#"<w:fldChar w:fldCharType="end"/></w:r>"#);
                } else {
                    let _ = write!(
                        out,
                        r#"<w:fldSimple w:instr="{}"{dirty_attr}>"#,
                        crate::xml::escape_xml(&instruction)
                    );
                    self.render_inlines(content, out);
                    out.push_str("</w:fldSimple>");
                }
            }
            InlineNode::Text(run) => {
                self.render_text_run(run, out);
            }
            InlineNode::EquationInline(eq) => {
                crate::math::render_equation_omml_with_font(
                    eq,
                    self.doc.math_font_family.as_deref(),
                    out,
                );
            }
            InlineNode::ShapeInline(shape) => {
                let run_xml = shape_to_inline_drawingml_run(self, shape);
                out.push_str(&run_xml);
            }
            InlineNode::CetzInline(canvas) => {
                let run_xml = cetz_canvas_to_drawingml_run(self, canvas);
                out.push_str(&run_xml);
            }
            InlineNode::ImageInline(img) => {
                self.render_image_run(img, out);
            }
            InlineNode::Link { target, content } => {
                self.render_link(target.as_ref(), content, out);
            }
            InlineNode::Ref { target, content } => {
                self.render_ref(target, content, out);
            }
            InlineNode::Citation { keys: _, content } => {
                self.render_inlines(content, out);
            }
            InlineNode::Footnote {
                id,
                number_text: _,
                content,
            } => {
                self.render_footnote_ref(*id, content, out);
            }
            InlineNode::LineBreak => {
                out.push_str("<w:r><w:br/></w:r>");
            }
            InlineNode::Tab => {
                out.push_str("<w:r><w:tab/></w:r>");
            }
            InlineNode::HorizontalSpace { width_pt, .. } => {
                if self.in_right_tab_paragraph && *width_pt >= 18.0 {
                    out.push_str("<w:r><w:tab/></w:r>");
                } else if *width_pt >= 4.0 {
                    let spacer = word_construct::model::ShapeIr {
                        id: Default::default(),
                        kind: word_construct::model::ShapeKindIr::Rect,
                        width_pt: *width_pt,
                        height_pt: 1.0,
                        fill: None,
                        fill_rule_even_odd: false,
                        stroke: None,
                        borders: Default::default(),
                        corner_radius_pt: None,
                        inset: Default::default(),
                        rotation_deg: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        body: Vec::new(),
                        align: word_construct::model::HAlignIr::Left,
                        placed_offset_pt: None,
                        baseline_shift_pt: None,
                    };
                    out.push_str(&shape_to_drawingml_run(self, &spacer));
                } else {
                    // Use non-stretchable `U+2005` (FOUR-PER-EM SPACE = `0.25em` in `Times New Roman`)
                    // rather than ASCII `' '` (`U+0020`), so Word's `<w:jc w:val="both"/>` justifier
                    // never stretches narrow math-padding spacers into wide gaps.
                    let sz_half_pt = ((*width_pt / 0.25) * 2.0).round().clamp(4.0, 48.0) as u32;
                    let _ = write!(
                        out,
                        r#"<w:r><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:eastAsia="Times New Roman" w:cs="Times New Roman"/><w:sz w:val="{sz_half_pt}"/><w:szCs w:val="{sz_half_pt}"/></w:rPr><w:t xml:space="preserve">&#x2005;</w:t></w:r>"#
                    );
                }
            }
            InlineNode::UnsupportedInline(unsup) => {
                self.render_unsupported_inline(unsup, out);
            }
        }
    }

    pub(crate) fn plain_text_of_inlines(inlines: &[InlineNode]) -> String {
        let mut s = String::new();
        for item in inlines {
            if let InlineNode::Text(r) = item {
                s.push_str(&r.text);
            }
        }
        s
    }
}
