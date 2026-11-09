//! `figure` -> `FigureConstruct` -> body blocks plus a `Caption` paragraph.

use std::fmt::Write as _;

use word_construct::figure::{FigureConstruct, solve_indented_figure_table};
use crate::writer::DocxWriter;
use ir::{BlockNode, CaptionPositionIr, InlineNode};

impl<'a> DocxWriter<'a> {
    /// `BlockNode::Figure` -> body blocks plus a `Caption` paragraph.
    pub(crate) fn render_figure(
        &mut self,
        body: &[BlockNode],
        caption: Option<&[InlineNode]>,
        caption_pos: CaptionPositionIr,
        bookmark: Option<&str>,
        out: &mut String,
    ) {
        self.render_figure_with_layout(body, caption, caption_pos, bookmark, 0, 60, out);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_figure_with_layout(
        &mut self,
        body: &[BlockNode],
        caption: Option<&[InlineNode]>,
        caption_pos: CaptionPositionIr,
        bookmark: Option<&str>,
        indent_left: usize,
        caption_after_dxa: i64,
        out: &mut String,
    ) {
        self.render_figure_with_layout_and_keep(
            body,
            caption,
            caption_pos,
            bookmark,
            indent_left,
            caption_after_dxa,
            false,
            out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_figure_with_layout_and_keep(
        &mut self,
        body: &[BlockNode],
        caption: Option<&[InlineNode]>,
        caption_pos: CaptionPositionIr,
        bookmark: Option<&str>,
        indent_left: usize,
        caption_after_dxa: i64,
        caption_keep_with_next: bool,
        out: &mut String,
    ) {
        let construct = FigureConstruct::solve(
            body,
            caption_pos,
            indent_left,
            caption_after_dxa,
            caption_keep_with_next,
        );
        let eff_indent = construct.eff_indent_dxa;
        match construct.caption_pos {
            CaptionPositionIr::Top => {
                if let Some(cap) = caption {
                    self.render_caption_paragraph(cap, bookmark, &construct, out);
                }
                self.render_figure_body(body, eff_indent, false, out);
            }
            CaptionPositionIr::Bottom => {
                self.render_figure_body(body, eff_indent, caption.is_some(), out);
                if let Some(cap) = caption {
                    self.render_caption_paragraph(cap, bookmark, &construct, out);
                }
            }
        }
    }

    fn render_figure_body(
        &mut self,
        body: &[BlockNode],
        indent_left: usize,
        keep_with_caption: bool,
        out: &mut String,
    ) {
        let len = body.len();
        for (idx, b) in body.iter().enumerate() {
            let is_last = idx + 1 == len;
            let keep_block_with_next = is_last && keep_with_caption;
            match b {
                BlockNode::Table(tbl) => {
                    if indent_left > 0 {
                        let adjusted =
                            solve_indented_figure_table(tbl, indent_left, self.content_width_dxa());
                        self.render_table_with_keep_all_rows(&adjusted, keep_block_with_next, out);
                    } else {
                        self.render_table_with_keep_all_rows(tbl, keep_block_with_next, out);
                    }
                }
                BlockNode::ImageBlock(img) => {
                    self.render_image_block_with_layout(img, indent_left, keep_block_with_next, out);
                }
                BlockNode::ShapeBlock(shape) => {
                    self.render_shape_block_with_layout(
                        shape,
                        indent_left,
                        keep_block_with_next,
                        out,
                    );
                }
                BlockNode::CetzCanvas(canvas) => {
                    self.render_cetz_block_with_layout(
                        canvas,
                        indent_left,
                        keep_block_with_next,
                        out,
                    );
                }
                BlockNode::Paragraph {
                    content,
                    style,
                    bookmark,
                } => {
                    let mut adj_style = style.clone();
                    if indent_left > 0 {
                        adj_style.left_indent_pt =
                            adj_style.left_indent_pt.max(indent_left as f64 / 20.0);
                    }
                    self.render_paragraph_with_keep_next(
                        content,
                        &adj_style,
                        bookmark.as_deref(),
                        keep_block_with_next,
                        out,
                    );
                }
                BlockNode::CodeBlock {
                    lang,
                    lines,
                    fill,
                    stroke,
                } => {
                    self.render_code_block_with_layout(
                        lang.as_deref(),
                        lines,
                        fill.as_deref(),
                        stroke.as_ref(),
                        indent_left,
                        keep_block_with_next,
                        out,
                    );
                }
                other => {
                    self.render_block(other, out);
                }
            }
        }
    }

    pub(crate) fn render_caption_paragraph(
        &mut self,
        caption: &[InlineNode],
        bookmark: Option<&str>,
        construct: &FigureConstruct,
        out: &mut String,
    ) {
        out.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Caption"/>"#);
        if construct.caption_keep_with_next {
            out.push_str("<w:keepNext/>");
        }
        let _ = write!(
            out,
            r#"<w:jc w:val="center"/><w:spacing w:before="{}" w:after="{}"/>"#,
            construct.caption_before_dxa, construct.caption_after_dxa,
        );
        if construct.eff_indent_dxa > 0 {
            let _ = write!(out, r#"<w:ind w:left="{}"/>"#, construct.eff_indent_dxa);
        }
        out.push_str("</w:pPr>");
        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        self.render_inlines(caption, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
