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
        for (idx, block) in blocks.iter().enumerate() {
            if let BlockNode::Heading {
                level,
                numbering,
                content,
                bookmark,
                align,
            } = block
                && matches!(blocks.get(idx + 1), Some(BlockNode::Paragraph { .. }))
            {
                self.render_heading_inner(
                    *level,
                    numbering.as_deref(),
                    content,
                    bookmark.as_deref(),
                    *align,
                    false,
                    true,
                    out,
                );
                continue;
            }
            self.render_block(block, out);
            // If two Word `<w:tbl>` blocks (e.g. two tables or numbered equations) are directly
            // adjacent without an intervening `<w:p>`, Word merges them into a single table!
            // Emit a minimal 2pt separator paragraph only between adjacent table-backed blocks.
            if Self::is_table_backed_block(block)
                && blocks
                    .get(idx + 1)
                    .is_some_and(Self::is_table_backed_block)
            {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="40" w:line="40" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p>"#,
                );
            }
        }
    }

    pub(crate) fn is_table_backed_block(block: &BlockNode) -> bool {
        matches!(block, BlockNode::Table(_) | BlockNode::UnsupportedBlock(_))
            || matches!(
                block,
                BlockNode::EquationBlock(eq) if eq.numbering_text.is_some()
            )
            || matches!(
                block,
                BlockNode::CodeBlock { fill, stroke, .. } if fill.is_some() || stroke.is_some()
            )
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
                spacing_pt,
                indent_pt,
                hanging_pt,
            } => {
                self.render_list(
                    items,
                    *tight,
                    *level,
                    *spacing_pt,
                    *indent_pt,
                    *hanging_pt,
                    out,
                );
            }
            BlockNode::Enum {
                items,
                tight,
                level,
                spacing_pt,
                indent_pt,
                hanging_pt,
            } => {
                self.render_enum(
                    items,
                    *tight,
                    *level,
                    *spacing_pt,
                    *indent_pt,
                    *hanging_pt,
                    out,
                );
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
            BlockNode::Columns {
                count,
                gutter_pt,
                body,
            } => {
                // 1. End the preceding 1-column section with a section-break paragraph.
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr>"#,
                );
                self.render_sect_pr_inner(
                    self.ir.page_setup.columns.max(1),
                    36.0,
                    self.has_scoped_columns,
                    out,
                );
                out.push_str("</w:pPr></w:p>");

                // 2. Render the multi-column body with active column width.
                let prev_cols = self.active_columns;
                self.active_columns = (*count, *gutter_pt);
                if let Some(BlockNode::Heading {
                    level,
                    numbering,
                    content,
                    bookmark,
                    align,
                }) = body.first()
                {
                    let compact_after =
                        matches!(body.get(1), Some(BlockNode::Paragraph { .. }));
                    self.render_heading_inner(
                        *level,
                        numbering.as_deref(),
                        content,
                        bookmark.as_deref(),
                        *align,
                        true,
                        compact_after,
                        out,
                    );
                    self.render_blocks(&body[1..], out);
                } else {
                    self.render_blocks(body, out);
                }
                self.active_columns = prev_cols;

                // 3. End the multi-column section with a continuous section break.
                self.has_scoped_columns = true;
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr>"#,
                );
                self.render_sect_pr_inner(*count, *gutter_pt, true, out);
                out.push_str("</w:pPr></w:p>");
            }
            BlockNode::UnsupportedBlock(unsup) => {
                self.render_unsupported_block(unsup, out);
            }
        }
    }
}
