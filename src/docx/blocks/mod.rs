//! Block-level rendering: `BlockNode` -> WordprocessingML.
//!
//! `render_block` only dispatches; each Typst object family lives in its own file
//! (mirroring `crate::extract::elements`).

mod bibliography;
mod cetz;
mod code;
mod equation;
mod figure;
mod heading;
mod image;
mod list;
mod outline;
mod paragraph;
mod quote;
mod rule;
mod shape;
mod table;
mod unsupported;

use crate::docx::builder::DocxBuilder;
use crate::ir::BlockNode;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_blocks(&mut self, blocks: &[BlockNode], out: &mut String) {
        for block in blocks {
            self.render_block(block, out);
        }
    }

    pub(crate) fn render_block(&mut self, block: &BlockNode, out: &mut String) {
        match block {
            BlockNode::Title { content, align } => {
                self.render_title(content, *align, out);
            }
            BlockNode::Heading {
                level,
                numbering,
                content,
                bookmark,
                align,
            } => {
                self.render_heading(
                    *level,
                    numbering.as_deref(),
                    content,
                    bookmark.as_deref(),
                    *align,
                    out,
                );
            }
            BlockNode::Paragraph {
                content,
                style,
                bookmark,
            } => {
                self.render_paragraph(content, style, bookmark.as_deref(), out);
            }
            BlockNode::List {
                items,
                tight,
                level,
            } => {
                self.render_list(items, *tight, *level, out);
            }
            BlockNode::Enum {
                items,
                tight,
                level,
            } => {
                self.render_enum(items, *tight, *level, out);
            }
            BlockNode::Terms { items, tight } => {
                self.render_terms(items, *tight, out);
            }
            BlockNode::Table(table_ir) => {
                self.render_table(table_ir, out);
            }
            BlockNode::Quote { body, attribution } => {
                self.render_quote(body, attribution.as_deref(), out);
            }
            BlockNode::CodeBlock {
                lang,
                lines,
                fill,
                stroke,
            } => {
                self.render_code_block(
                    lang.as_deref(),
                    lines,
                    fill.as_deref(),
                    stroke.as_ref(),
                    out,
                );
            }
            BlockNode::Figure {
                body,
                caption,
                caption_top,
                kind: _,
                bookmark,
            } => {
                self.render_figure(
                    body,
                    caption.as_deref(),
                    *caption_top,
                    bookmark.as_deref(),
                    out,
                );
            }
            BlockNode::EquationBlock(eq) => {
                self.render_equation_block(eq, out);
            }
            BlockNode::ShapeBlock(shape) => {
                self.render_shape_block(shape, out);
            }
            BlockNode::CetzCanvas(canvas) => {
                self.render_cetz_block(canvas, out);
            }
            BlockNode::ImageBlock(img) => {
                self.render_image_block(img, out);
            }
            BlockNode::Outline { title, entries } => {
                self.render_outline(title.as_deref(), entries, out);
            }
            BlockNode::Bibliography { title, entries } => {
                self.render_bibliography(title.as_deref(), entries, out);
            }
            BlockNode::HorizontalRule {
                length_pt: _,
                stroke,
                align: _,
            } => {
                self.render_horizontal_rule(stroke, out);
            }
            BlockNode::PageBreak => {
                out.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
            }
            BlockNode::ColumnBreak => {
                out.push_str(r#"<w:p><w:r><w:br w:type="column"/></w:r></w:p>"#);
            }
            BlockNode::UnsupportedBlock(unsup) => {
                self.render_unsupported_block(unsup, out);
            }
        }
    }
}
