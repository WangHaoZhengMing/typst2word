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
use crate::drawingml::{cetz_canvas_to_drawingml_run, shape_to_drawingml_run};
use ir::InlineNode;

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
                let mut instruction = match kind {
                    ir::FieldKindIr::Page => "PAGE".to_string(),
                    ir::FieldKindIr::NumPages => "NUMPAGES".to_string(),
                    ir::FieldKindIr::PageRef { bookmark } => format!("PAGEREF {bookmark} \\h"),
                };
                if let Some(format) = format { instruction.push_str(&format!(" \\* {format}")); }
                let _ = write!(out, r#"<w:fldSimple w:instr="{}" w:dirty="true">"#, crate::xml::escape_xml(&instruction));
                self.render_inlines(content, out);
                out.push_str("</w:fldSimple>");
            }
            InlineNode::Text(run) => {
                self.render_text_run(run, out);
            }
            InlineNode::EquationInline(eq) => {
                crate::math::render_equation_omml(eq, out);
            }
            InlineNode::ShapeInline(shape) => {
                let run_xml = shape_to_drawingml_run(shape);
                out.push_str(&run_xml);
            }
            InlineNode::CetzInline(canvas) => {
                let run_xml = cetz_canvas_to_drawingml_run(canvas);
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
            InlineNode::HorizontalSpace { width_pt } => {
                if *width_pt >= 36.0 {
                    out.push_str("<w:r><w:tab/></w:r>");
                } else {
                    let spaces = ((width_pt / 3.5).round() as usize).clamp(1, 8);
                    let s = " ".repeat(spaces);
                    let _ = write!(out, r#"<w:r><w:t xml:space="preserve">{s}</w:t></w:r>"#);
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
