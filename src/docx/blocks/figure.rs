//! `figure` -> `FigureConstruct` -> body blocks plus a `Caption` paragraph.

use std::fmt::Write as _;

use crate::construct::figure::{FigureConstruct, solve_indented_figure_table};
use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, CaptionPositionIr, InlineNode};

impl<'a> DocxBuilder<'a> {
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
        if construct.caption_pos.is_top() {
            if let Some(cap) = caption {
                self.render_caption_paragraph(cap, bookmark, &construct, out);
            }
            self.render_figure_body_indented(body, eff_indent, out);
        } else {
            let mut body_xml = String::new();
            self.render_figure_body_indented(body, eff_indent, &mut body_xml);
            if caption.is_some() {
                if body_xml.ends_with("</w:tbl>") {
                    // Keep all rows of a figure table together with the figure caption.
                    let patched = body_xml.replace("<w:p><w:pPr>", "<w:p><w:pPr><w:keepNext/>");
                    out.push_str(&patched);
                } else if body_xml.contains("<w:drawing>") {
                    out.push_str(&body_xml.replacen(
                        "<w:p><w:pPr>",
                        "<w:p><w:pPr><w:keepNext/>",
                        1,
                    ));
                } else if let Some(last_p_pos) = body_xml.rfind("<w:p><w:pPr>") {
                    let (head, tail) = body_xml.split_at(last_p_pos);
                    let patched_tail = if tail.contains("<w:keepNext/>") {
                        tail.to_string()
                    } else {
                        tail.replacen("<w:p><w:pPr>", "<w:p><w:pPr><w:keepNext/>", 1)
                    };
                    out.push_str(head);
                    out.push_str(&patched_tail);
                } else {
                    out.push_str(&body_xml);
                }
            } else {
                out.push_str(&body_xml);
            }
            if let Some(cap) = caption {
                self.render_caption_paragraph(cap, bookmark, &construct, out);
            }
        }
    }

    fn render_figure_body_indented(
        &mut self,
        body: &[BlockNode],
        indent_left: usize,
        out: &mut String,
    ) {
        if indent_left == 0 {
            self.render_blocks(body, out);
            return;
        }
        for b in body {
            if let BlockNode::Table(tbl) = b {
                let adjusted =
                    solve_indented_figure_table(tbl, indent_left, self.content_width_dxa());
                self.render_table(&adjusted, out);
            } else {
                let mut blk_xml = String::new();
                self.render_block(b, &mut blk_xml);
                let ind_tag = format!(r#"<w:pPr><w:ind w:left="{indent_left}"/>"#);
                blk_xml = blk_xml.replacen("<w:pPr>", &ind_tag, 1);
                out.push_str(&blk_xml);
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
