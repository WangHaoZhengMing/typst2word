//! `par` -> `<w:p>` with spacing and indentation.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{InlineNode, ParagraphStyleIr};
use crate::shape_dml::pt_to_dxa;

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::Paragraph` -> `<w:p>` with spacing / indentation.
    pub(crate) fn render_paragraph(
        &mut self,
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        bookmark: Option<&str>,
        out: &mut String,
    ) {
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(style.align, out);
        let before_dxa = pt_to_dxa(style.space_before_pt).max(0);
        let after_dxa = pt_to_dxa(style.space_after_pt).max(0);
        if let Some(ls_pt) = style.line_spacing_pt {
            let line_dxa = (pt_to_dxa(ls_pt) + 240).max(240);
            let _ = write!(
                out,
                r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/>"#
            );
        } else if before_dxa > 0 || after_dxa > 0 {
            let _ = write!(
                out,
                r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}"/>"#
            );
        }
        let left_dxa = pt_to_dxa(style.left_indent_pt).max(0);
        let right_dxa = pt_to_dxa(style.right_indent_pt).max(0);
        let first_dxa = pt_to_dxa(style.first_line_indent_pt).max(0);
        let hang_dxa = pt_to_dxa(style.hanging_indent_pt).max(0);
        if left_dxa > 0 || right_dxa > 0 || first_dxa > 0 || hang_dxa > 0 {
            let mut ind = format!(r#"<w:ind w:left="{left_dxa}" w:right="{right_dxa}""#);
            if hang_dxa > 0 {
                let _ = write!(ind, r#" w:hanging="{hang_dxa}""#);
            } else if first_dxa > 0 {
                let _ = write!(ind, r#" w:firstLine="{first_dxa}""#);
            }
            ind.push_str("/>");
            out.push_str(&ind);
        }
        out.push_str("</w:pPr>");
        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        self.render_inlines(content, out);
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
