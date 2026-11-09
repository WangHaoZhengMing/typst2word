//! `title` / `heading` -> `HeadingConstruct` -> `Title` / `HeadingN` paragraphs.

use std::fmt::Write as _;

use crate::construct::heading::HeadingConstruct;
use crate::docx::builder::DocxBuilder;
use crate::ir::{HAlignIr, InlineNode};

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
        self.render_heading_inner(
            level, numbering, content, bookmark, align, false, false, out,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_heading_inner(
        &mut self,
        level: usize,
        numbering: Option<&str>,
        content: &[InlineNode],
        bookmark: Option<&str>,
        align: HAlignIr,
        collapse_before: bool,
        compact_after: bool,
        out: &mut String,
    ) {
        let construct = HeadingConstruct::solve(
            level,
            numbering,
            content,
            align,
            collapse_before,
            compact_after,
        );

        out.push_str("<w:p><w:pPr>");
        let _ = write!(
            out,
            r#"<w:pStyle w:val="{}"/><w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/>"#,
            construct.style_id, construct.before_dxa, construct.after_dxa, construct.line_dxa
        );
        if construct.align != HAlignIr::Left {
            Self::render_jc(construct.align, out);
        }
        out.push_str("</w:pPr>");

        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        if let Some(prefix_run) = &construct.prefix_run {
            self.render_text_run(prefix_run, out);
        }
        self.render_inlines(content, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
