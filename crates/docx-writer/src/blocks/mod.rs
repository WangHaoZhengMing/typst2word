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

use crate::writer::DocxWriter;
use crate::xml_helpers::{open_tiny_sect_carrier_p, write_exact_spacer_p, write_tiny_break_p};

/// Inject `<w:pageBreakBefore/>` into the first paragraph inside `out[start_len..]` in strict
/// ECMA-376 `<w:pPr>` element order (immediately after `<w:pStyle .../>` when present), falling
/// back to a tiny explicit page-break paragraph only if no `<w:p>` was emitted.
fn inject_page_break_before(out: &mut String, start_len: usize) {
    let slice = &out[start_len..];
    if let Some(ppr_rel) = slice.find("<w:pPr>") {
        let ppr_content_start = start_len + ppr_rel + "<w:pPr>".len();
        let after_ppr = &out[ppr_content_start..];
        let insert_at = if after_ppr.starts_with("<w:pStyle ")
            && let Some(end_tag) = after_ppr.find("/>")
        {
            ppr_content_start + end_tag + 2
        } else {
            ppr_content_start
        };
        out.insert_str(insert_at, "<w:pageBreakBefore/>");
    } else if let Some(p_rel) = slice.find("<w:p>") {
        let insert_at = start_len + p_rel + "<w:p>".len();
        out.insert_str(insert_at, "<w:pPr><w:pageBreakBefore/></w:pPr>");
    } else {
        let block_xml = out.split_off(start_len);
        write_tiny_break_p("page", out);
        out.push_str(&block_xml);
    }
}

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_blocks(&mut self, blocks: &[BlockNode], out: &mut String) {
        let mut pending_page_break = false;
        for (idx, block) in blocks.iter().enumerate() {
            if matches!(block, BlockNode::PageBreak) {
                if pending_page_break {
                    write_tiny_break_p("page", out);
                } else {
                    pending_page_break = true;
                }
                continue;
            }
            let start_len = out.len();
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
            } else if let BlockNode::Paragraph {
                id,
                content,
                style,
                bookmark,
            } = block
            {
                self.render_paragraph(*id, content, style, bookmark.as_deref(), out);
            } else if let BlockNode::Figure {
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
            } else {
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

            if pending_page_break && out.len() > start_len {
                // Prefer `<w:pageBreakBefore/>` on the following block's first `<w:pPr>`
                // over a standalone `<w:p><w:r><w:br w:type="page"/></w:r></w:p>`: when the
                // preceding block ends right near the bottom of a page, a standalone break
                // paragraph wraps onto the next page and then breaks a second time, creating
                // a spurious blank page, whereas `<w:pageBreakBefore/>` is idempotent.
                inject_page_break_before(out, start_len);
                pending_page_break = false;
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
