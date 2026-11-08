//! `title` / `heading` -> `Title` / `HeadingN` paragraphs.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{HAlignIr, InlineNode, TextRunIr};

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::Title` -> `Title` styled paragraph.
    pub(crate) fn render_title(
        &mut self,
        content: &[InlineNode],
        align: HAlignIr,
        out: &mut String,
    ) {
        out.push_str("<w:p><w:pPr><w:pStyle w:val=\"Title\"/>");
        Self::render_jc(align, out);
        out.push_str("</w:pPr>");
        self.render_inlines(content, out);
        out.push_str("</w:p>");
    }

    /// `BlockNode::Heading` -> `HeadingN` styled paragraph with optional numbering prefix.
    pub(crate) fn render_heading(
        &mut self,
        level: usize,
        numbering: Option<&str>,
        content: &[InlineNode],
        bookmark: Option<&str>,
        align: HAlignIr,
        out: &mut String,
    ) {
        let style_id = format!("Heading{}", (level).clamp(1, 6));
        out.push_str("<w:p><w:pPr>");
        let _ = write!(out, r#"<w:pStyle w:val="{style_id}"/>"#);
        if align != HAlignIr::Left {
            Self::render_jc(align, out);
        }
        out.push_str("</w:pPr>");

        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        if let Some(num_prefix) = numbering {
            let default_sz = match level {
                1 => 15.4,
                2 => 13.2,
                _ => 11.0,
            };
            let sz_pt = content
                .iter()
                .find_map(|inl| {
                    if let InlineNode::Text(r) = inl {
                        r.font_size_pt
                    } else {
                        None
                    }
                })
                .unwrap_or(default_sz);
            let prefix_run = TextRunIr {
                text: format!("{num_prefix} "),
                font_family: Some("Libertinus Serif".to_string()),
                font_size_pt: Some(sz_pt),
                bold: true,
                ..Default::default()
            };
            self.render_text_run(&prefix_run, out);
        }
        self.render_inlines(content, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
