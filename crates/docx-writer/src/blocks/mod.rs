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

use word_construct::BlockSequenceNeighborContext;
use word_construct::model::BlockNode;
use word_construct::paragraph::ParagraphConstruct;

use crate::writer::DocxWriter;
use crate::xml_helpers::{open_tiny_sect_carrier_p, write_exact_spacer_p, write_tiny_break_p};

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_blocks(&mut self, blocks: &[BlockNode], out: &mut String) {
        for (idx, block) in blocks.iter().enumerate() {
            let nb = BlockSequenceNeighborContext::for_index(blocks, idx, self.doc);
            if let BlockNode::Heading {
                id,
                level,
                numbering,
                content,
                bookmark,
                align,
                left_indent_pt,
                box_style,
            } = block
            {
                self.render_heading_inner(
                    *id,
                    *level,
                    numbering.as_deref(),
                    content,
                    bookmark.as_deref(),
                    *align,
                    *left_indent_pt,
                    box_style.as_ref(),
                    nb.collapse_before,
                    nb.heading_compact_after,
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
                if let Some(spacer_after) = nb.figure_pre_spacer_after_dxa {
                    write_exact_spacer_p(spacer_after, 40, false, out);
                }
                self.render_figure_with_layout_and_keep(
                    body,
                    caption.as_deref(),
                    *caption_pos,
                    bookmark.as_deref(),
                    0,
                    nb.figure_after_dxa,
                    nb.figure_keep_with_next,
                    out,
                );
                continue;
            }
            match block {
                BlockNode::List {
                    id,
                    items,
                    tight,
                    level,
                    spacing_pt,
                    indent_pt,
                    hanging_pt,
                } => {
                    self.render_list_collapsed_with_id(
                        *id,
                        items,
                        *tight,
                        *level,
                        *spacing_pt,
                        *indent_pt,
                        *hanging_pt,
                        nb.collapse_before,
                        out,
                    );
                }
                BlockNode::Enum {
                    id,
                    items,
                    tight,
                    level,
                    spacing_pt,
                    indent_pt,
                    hanging_pt,
                } => {
                    self.render_enum_collapsed_with_id(
                        *id,
                        items,
                        *tight,
                        *level,
                        *spacing_pt,
                        *indent_pt,
                        *hanging_pt,
                        nb.collapse_before,
                        nb.enum_after_table_block,
                        out,
                    );
                }
                BlockNode::Quote {
                    body, attribution, ..
                } => {
                    self.render_quote_collapsed(
                        body,
                        attribution.as_deref(),
                        nb.collapse_before,
                        out,
                    );
                }
                BlockNode::EquationBlock(eq) => {
                    self.render_equation_block_with_layout(
                        eq,
                        nb.equation_before_dxa,
                        nb.equation_after_dxa,
                        0,
                        out,
                    );
                }
                BlockNode::ShapeBlock(shape) => {
                    self.render_shape_block_collapsed(shape, nb.collapse_before, out);
                }
                BlockNode::ImageBlock(img) => {
                    self.render_image_block_collapsed(img, nb.collapse_before, out);
                }
                BlockNode::Outline { title, entries } => {
                    self.render_outline(title.as_deref(), entries, nb.collapse_before, out);
                }
                BlockNode::Bibliography { title, entries } => {
                    self.render_bibliography(title.as_deref(), entries, nb.collapse_before, out);
                }
                BlockNode::HorizontalRule {
                    id,
                    length_pt,
                    stroke,
                    align,
                    left_indent_pt,
                } => {
                    self.render_horizontal_rule(
                        *id,
                        *length_pt,
                        stroke,
                        *align,
                        *left_indent_pt,
                        nb.collapse_before,
                        out,
                    );
                }
                _ => self.render_block(block, out),
            }
            if let Some(sep_after) = nb.table_to_table_separator_after_dxa {
                write_exact_spacer_p(sep_after, 40, false, out);
            }
        }
    }

    pub(crate) fn render_block(&mut self, block: &BlockNode, out: &mut String) {
        match block {
            BlockNode::Title { content, align, .. } => {
                self.render_title(content, *align, out);
            }
            BlockNode::Heading {
                id,
                level,
                numbering,
                content,
                bookmark,
                align,
                left_indent_pt,
                box_style,
            } => {
                self.render_heading(
                    *id,
                    *level,
                    numbering.as_deref(),
                    content,
                    bookmark.as_deref(),
                    *align,
                    *left_indent_pt,
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
                id,
                lang,
                lines,
                fill,
                stroke,
                ..
            } => {
                self.render_code_block(
                    *id,
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
                id,
                length_pt,
                stroke,
                align,
                left_indent_pt,
            } => {
                self.render_horizontal_rule(
                    *id,
                    *length_pt,
                    stroke,
                    *align,
                    *left_indent_pt,
                    false,
                    out,
                );
            }
            BlockNode::PageBreak => {
                write_tiny_break_p("page", out);
            }
            BlockNode::ColumnBreak => {
                write_tiny_break_p("column", out);
            }
            BlockNode::Columns {
                count,
                gutter_pt,
                body,
                ..
            } => {
                // 1. End the preceding 1-column section with a section-break paragraph.
                open_tiny_sect_carrier_p(out);
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
                self.render_blocks(body, out);
                self.active_columns = prev_cols;

                // 3. End the multi-column section with a continuous section break.
                self.has_scoped_columns = true;
                open_tiny_sect_carrier_p(out);
                self.render_sect_pr_inner(*count, *gutter_pt, true, out);
                out.push_str("</w:pPr></w:p>");
            }
            BlockNode::UnsupportedBlock(unsup) => {
                self.render_unsupported_block(unsup, out);
            }
        }
    }
}
