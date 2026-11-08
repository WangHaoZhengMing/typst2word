//! `figure` -> body blocks plus a `Caption` paragraph.

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, InlineNode};

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
        if caption_top {
            if let Some(cap) = caption {
                self.render_caption_paragraph(cap, bookmark, out);
            }
            self.render_blocks(body, out);
        } else {
            self.render_blocks(body, out);
            if let Some(cap) = caption {
                self.render_caption_paragraph(cap, bookmark, out);
            }
        }
    }

    pub(crate) fn render_caption_paragraph(
        &mut self,
        caption: &[InlineNode],
        bookmark: Option<&str>,
        out: &mut String,
    ) {
        out.push_str(
            r#"<w:p><w:pPr><w:pStyle w:val="Caption"/><w:jc w:val="center"/><w:spacing w:before="20" w:after="60"/></w:pPr>"#,
        );
        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        self.render_inlines(caption, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
