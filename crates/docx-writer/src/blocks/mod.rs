//! Block-level rendering: `BlockNode` -> Word Construct (`crate::construct`) -> WordprocessingML.
//!
//! `render_block` dispatches each [`BlockNode`] to its dedicated object renderer, which
//! constructs a solved Word object via `word_construct::*` before serializing XML.

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

use word_construct::WordBlockObjectKind;
use word_construct::paragraph::ParagraphConstruct;
use crate::writer::DocxWriter;
use word_construct::model::BlockNode;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_blocks(&mut self, blocks: &[BlockNode], out: &mut String) {
        for (idx, block) in blocks.iter().enumerate() {
            let collapse_before = idx == 0
                || matches!(
                    blocks.get(idx.wrapping_sub(1)),
                    Some(BlockNode::PageBreak | BlockNode::ColumnBreak)
                );
            if let BlockNode::Heading {
                level,
                numbering,
                content,
                bookmark,
                align,
                box_style,
                ..
            } = block
            {
                let compact_after = matches!(blocks.get(idx + 1), Some(BlockNode::Paragraph { .. }));
                self.render_heading_inner(
                    *level,
                    numbering.as_deref(),
                    content,
                    bookmark.as_deref(),
                    *align,
                    box_style.as_ref(),
                    collapse_before,
                    compact_after,
                    out,
                );
                continue;
            }
            if let BlockNode::Paragraph {
                id,
                content,
                style,
                bookmark,
            } = block
            {
                let adj_style = ParagraphConstruct::solve_adjacent_paragraph_style(
                    content,
                    style,
                    blocks.get(idx + 1),
                );
                self.render_paragraph(*id, content, &adj_style, bookmark.as_deref(), out);
                continue;
            }
            if let BlockNode::Figure {
                body,
                caption,
                caption_pos,
                bookmark,
                ..
            } = block
            {
                let next_keep = matches!(
                    blocks.get(idx + 1),
                    Some(BlockNode::Paragraph { style, .. }) if style.sticky
                );
                self.render_figure_with_layout_and_keep(
                    body,
                    caption.as_deref(),
                    *caption_pos,
                    bookmark.as_deref(),
                    0,
                    60,
                    next_keep,
                    out,
                );
                continue;
            }
            match block {
                BlockNode::List {
                    items,
                    tight,
                    level,
                    spacing_pt,
                    indent_pt,
                    hanging_pt,
                    ..
                } => {
                    self.render_list_collapsed(
                        items,
                        *tight,
                        *level,
                        *spacing_pt,
                        *indent_pt,
                        *hanging_pt,
                        collapse_before,
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
                    ..
                } => {
                    self.render_enum_collapsed(
                        items,
                        *tight,
                        *level,
                        *spacing_pt,
                        *indent_pt,
                        *hanging_pt,
                        collapse_before,
                        out,
                    );
                }
                BlockNode::Quote {
                    body, attribution, ..
                } => {
                    self.render_quote_collapsed(
                        body,
                        attribution.as_deref(),
                        collapse_before,
                        out,
                    );
                }
                BlockNode::EquationBlock(eq) => {
                    self.render_equation_block_collapsed(eq, collapse_before, out);
                }
                BlockNode::ShapeBlock(shape) => {
                    self.render_shape_block_collapsed(shape, collapse_before, out);
                }
                BlockNode::ImageBlock(img) => {
                    self.render_image_block_collapsed(img, collapse_before, out);
                }
                BlockNode::Outline { title, entries } => {
                    self.render_outline(title.as_deref(), entries, collapse_before, out);
                }
                BlockNode::Bibliography { title, entries } => {
                    self.render_bibliography(title.as_deref(), entries, collapse_before, out);
                }
                BlockNode::HorizontalRule {
                    length_pt,
                    stroke,
                    align,
                } => {
                    self.render_horizontal_rule(*length_pt, stroke, *align, collapse_before, out);
                }
                _ => self.render_block(block, out),
            }
            // If two Word `<w:tbl>` blocks (e.g. two tables or numbered equations) are directly
            // adjacent without an intervening `<w:p>`, Word merges them into a single table!
            // Emit a minimal 2pt separator paragraph only between adjacent table-backed blocks.
            if Self::is_table_backed_block(block)
                && blocks.get(idx + 1).is_some_and(Self::is_table_backed_block)
            {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="40" w:line="40" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p>"#,
                );
            }
        }
    }

    pub(crate) fn is_table_backed_block(block: &BlockNode) -> bool {
        WordBlockObjectKind::classify(block).is_table_backed()
    }

    pub(crate) fn render_block(&mut self, block: &BlockNode, out: &mut String) {
        match block {
            BlockNode::Title { content, align, .. } => {
                self.render_title(content, *align, out);
            }
            BlockNode::Heading {
                level,
                numbering,
                content,
                bookmark,
                align,
                box_style,
                ..
            } => {
                self.render_heading(
                    *level,
                    numbering.as_deref(),
                    content,
                    bookmark.as_deref(),
                    *align,
                    box_style.as_ref(),
                    out,
                );
            }
            BlockNode::Paragraph {
                id,
                content,
                style,
                bookmark,
            } => {
                self.render_paragraph(*id, content, style, bookmark.as_deref(), out);
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
                ..
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
            BlockNode::Terms { items, tight, .. } => {
                self.render_terms(items, *tight, out);
            }
            BlockNode::Container(container) => {
                self.render_blocks(&container.body, out);
            }
            BlockNode::Table(table_ir) => {
                self.render_table(table_ir, out);
            }
            BlockNode::Quote {
                body, attribution, ..
            } => {
                self.render_quote(body, attribution.as_deref(), out);
            }
            BlockNode::CodeBlock {
                lang,
                lines,
                fill,
                stroke,
                ..
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
                caption_pos,
                bookmark,
                ..
            } => {
                self.render_figure(
                    body,
                    caption.as_deref(),
                    *caption_pos,
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
                self.render_outline(title.as_deref(), entries, false, out);
            }
            BlockNode::Bibliography { title, entries } => {
                self.render_bibliography(title.as_deref(), entries, false, out);
            }
            BlockNode::HorizontalRule {
                length_pt,
                stroke,
                align,
            } => {
                self.render_horizontal_rule(*length_pt, stroke, *align, false, out);
            }
            BlockNode::PageBreak => {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr><w:r><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr><w:br w:type="page"/></w:r></w:p>"#,
                );
            }
            BlockNode::ColumnBreak => {
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr><w:r><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr><w:br w:type="column"/></w:r></w:p>"#,
                );
            }
            BlockNode::Columns {
                count,
                gutter_pt,
                body,
                ..
            } => {
                // 1. End the preceding 1-column section with a section-break paragraph.
                out.push_str(
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr>"#,
                );
                self.render_sect_pr_inner(
                    self.current_setup.columns.max(1),
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
                    box_style,
                    ..
                }) = body.first()
                {
                    let compact_after = matches!(body.get(1), Some(BlockNode::Paragraph { .. }));
                    self.render_heading_inner(
                        *level,
                        numbering.as_deref(),
                        content,
                        bookmark.as_deref(),
                        *align,
                        box_style.as_ref(),
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
