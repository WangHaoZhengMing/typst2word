//! `figure` -> body blocks plus a `Caption` paragraph.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, HAlignIr, InlineNode};

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::Figure` -> body blocks plus a `Caption` paragraph.
    pub(crate) fn render_figure(
        &mut self,
        body: &[BlockNode],
        caption: Option<&[InlineNode]>,
        caption_top: bool,
        bookmark: Option<&str>,
        out: &mut String,
    ) {
        self.render_figure_indented(body, caption, caption_top, bookmark, 0, out);
    }

    pub(crate) fn render_figure_indented(
        &mut self,
        body: &[BlockNode],
        caption: Option<&[InlineNode]>,
        caption_top: bool,
        bookmark: Option<&str>,
        indent_left: usize,
        out: &mut String,
    ) {
        let body_indent_dxa = body
            .iter()
            .find_map(|b| match b {
                BlockNode::Table(t) if t.left_indent_pt > 0.5 => {
                    Some(crate::units::pt_to_dxa(t.left_indent_pt).max(0) as usize)
                }
                _ => None,
            })
            .unwrap_or(0);
        let eff_indent = indent_left.max(body_indent_dxa);
        if caption_top {
            if let Some(cap) = caption {
                self.render_caption_paragraph_indented(cap, bookmark, eff_indent, out);
            }
            self.render_figure_body_indented(body, eff_indent, out);
        } else {
            let mut body_xml = String::new();
            self.render_figure_body_indented(body, eff_indent, &mut body_xml);
            if caption.is_some()
                && body_xml.ends_with("</w:tbl>")
                && let Some(last_tr_pos) = body_xml.rfind("<w:tr>")
            {
                let (head, tail) = body_xml.split_at(last_tr_pos);
                let patched_tail = if tail.contains("<w:keepNext/>") {
                    tail.to_string()
                } else {
                    tail.replace("<w:p><w:pPr>", "<w:p><w:pPr><w:keepNext/>")
                };
                out.push_str(head);
                out.push_str(&patched_tail);
            } else {
                out.push_str(&body_xml);
            }
            if let Some(cap) = caption {
                self.render_caption_paragraph_indented(cap, bookmark, eff_indent, out);
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
        let target_left_pt = indent_left as f64 / 20.0;
        let avail_w_pt = ((self.content_width_dxa() as f64 / 20.0) - target_left_pt).max(80.0);
        for b in body {
            if let BlockNode::Table(tbl) = b {
                let mut adjusted = tbl.clone();
                adjusted.align = HAlignIr::Left;
                let mut total_col_w: f64 = adjusted.col_widths_pt.iter().sum();
                if total_col_w > avail_w_pt + 2.0 {
                    let excess = total_col_w - avail_w_pt;
                    if let Some(last_w) = adjusted.col_widths_pt.last_mut()
                        && *last_w > excess + 40.0
                    {
                        *last_w -= excess;
                        total_col_w = avail_w_pt;
                    }
                }
                if adjusted.left_indent_pt <= 0.5 {
                    adjusted.left_indent_pt =
                        target_left_pt + ((avail_w_pt - total_col_w).max(0.0) * 0.5);
                }
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

    pub(crate) fn render_caption_paragraph_indented(
        &mut self,
        caption: &[InlineNode],
        bookmark: Option<&str>,
        indent_left: usize,
        out: &mut String,
    ) {
        out.push_str(
            r#"<w:p><w:pPr><w:pStyle w:val="Caption"/><w:jc w:val="center"/><w:spacing w:before="20" w:after="60"/>"#,
        );
        if indent_left > 0 {
            let _ = write!(out, r#"<w:ind w:left="{indent_left}"/>"#);
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
