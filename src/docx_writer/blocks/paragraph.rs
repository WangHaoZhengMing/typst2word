//! `par` -> `ParagraphConstruct` -> `<w:p>` with spacing and indentation.

use std::fmt::Write as _;

use crate::construct::paragraph::{ParagraphConstruct, ParagraphIndentationConstruct};
use crate::docx_writer::writer::DocxWriter;
use crate::ir::{InlineNode, ParagraphStyleIr};

impl<'a> DocxWriter<'a> {
    /// `BlockNode::Paragraph` -> `<w:p>` with spacing / indentation.
    pub(crate) fn render_paragraph(
        &mut self,
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        bookmark: Option<&str>,
        out: &mut String,
    ) {
        self.render_paragraph_with_keep_next(content, style, bookmark, false, out);
    }

    pub(crate) fn render_paragraph_with_keep_next(
        &mut self,
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        bookmark: Option<&str>,
        keep_with_next: bool,
        out: &mut String,
    ) {
        let construct = ParagraphConstruct::solve(
            content,
            style,
            self.content_width_dxa(),
            keep_with_next,
        );
        out.push_str("<w:p><w:pPr>");
        if construct.keep_with_next {
            out.push_str("<w:keepNext/>");
        }
        if construct.keep_lines {
            out.push_str("<w:keepLines/>");
        }
        Self::render_jc(construct.align, out);
        if let Some(tab_pos) = construct.right_tab_stop_dxa {
            let _ = write!(
                out,
                r#"<w:tabs><w:tab w:val="right" w:pos="{tab_pos}"/></w:tabs>"#
            );
        }
        let _ = write!(
            out,
            r#"<w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/>"#,
            construct.before_dxa, construct.after_dxa, construct.line_dxa
        );
        match construct.indentation {
            ParagraphIndentationConstruct::None => {}
            ParagraphIndentationConstruct::Hanging {
                left_dxa,
                right_dxa,
                hanging_dxa,
            } => {
                let _ = write!(
                    out,
                    r#"<w:ind w:left="{left_dxa}" w:right="{right_dxa}" w:hanging="{hanging_dxa}"/>"#
                );
            }
            ParagraphIndentationConstruct::FirstLine {
                left_dxa,
                right_dxa,
                first_line_dxa,
            } => {
                let _ = write!(
                    out,
                    r#"<w:ind w:left="{left_dxa}" w:right="{right_dxa}" w:firstLine="{first_line_dxa}"/>"#
                );
            }
            ParagraphIndentationConstruct::LeftRightOnly {
                left_dxa,
                right_dxa,
            } => {
                let _ = write!(out, r#"<w:ind w:left="{left_dxa}" w:right="{right_dxa}"/>"#);
            }
        }
        if let Some(sz_val) = construct.mark_rpr_sz_half_pt {
            let _ = write!(
                out,
                r#"<w:rPr><w:sz w:val="{sz_val}"/><w:szCs w:val="{sz_val}"/></w:rPr>"#
            );
        }
        out.push_str("</w:pPr>");
        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        self.render_inlines(content, out);
        if construct.needs_zwsp_for_inline_eq {
            // Prevent Microsoft Word from promoting a `<w:p>` containing only `<m:oMath>`
            // into centered Display Math Mode (`oMathPara` with +8pt vertical margin).
            out.push_str("<w:r><w:t>&#8203;</w:t></w:r>");
        }
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
