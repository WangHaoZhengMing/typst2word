//! Inline-level rendering: `InlineNode` -> runs.
//!
//! `render_inline` only dispatches; specific inline kinds live in sibling files.

mod bookmark;
mod footnote;
mod image;
mod link;
mod text;
mod unsupported;

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::docx::drawingml::{cetz_canvas_to_drawingml_run, shape_to_drawingml_run};
use crate::ir::InlineNode;

impl<'a> DocxBuilder<'a> {
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
            InlineNode::Text(run) => {
                self.render_text_run(run, out);
            }
            InlineNode::EquationInline(eq) => {
                out.push_str(&eq.omml_xml);
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
            InlineNode::Link {
                url,
                anchor,
                content,
            } => {
                self.render_link(url.as_deref(), anchor.as_deref(), content, out);
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
