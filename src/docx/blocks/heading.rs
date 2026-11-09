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
        let style_id = format!("Heading{}", (level).clamp(1, 6));
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

        out.push_str("<w:p><w:pPr>");
        let _ = write!(out, r#"<w:pStyle w:val="{style_id}"/>"#);
        if level >= 2 && sz_pt <= 13.0 {
            let line_dxa = Self::compute_line_spacing_dxa(content, Some(0.65 * sz_pt));
            let before_dxa = if collapse_before {
                0
            } else {
                crate::units::pt_to_dxa(sz_pt * 0.567)
            };
            let after_dxa = if compact_after {
                20
            } else {
                crate::units::pt_to_dxa(sz_pt * 0.483)
            };
            let _ = write!(
                out,
                r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/>"#
            );
        } else {
            let line_dxa = Self::compute_line_spacing_dxa(content, None);
            let before_dxa = if collapse_before {
                0
            } else if level == 1 {
                if line_dxa < 200 { 224 } else { 240 }
            } else {
                160
            };
            let after_dxa = if compact_after {
                if line_dxa < 200 { 0 } else { 16 }
            } else {
                80
            };
            let _ = write!(
                out,
                r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/>"#
            );
        }
        if align != HAlignIr::Left {
            Self::render_jc(align, out);
        }
        out.push_str("</w:pPr>");

        let bm_id = bookmark.map(|bm| self.emit_bookmark_start(bm, out));
        if let Some(num_prefix) = numbering {
            let sep = if num_prefix.ends_with('、')
                || num_prefix.ends_with('．')
                || num_prefix.ends_with('）')
            {
                ""
            } else {
                " "
            };
            let first_font = content.iter().find_map(|inl| {
                if let InlineNode::Text(r) = inl {
                    r.font_family.clone()
                } else {
                    None
                }
            });
            let prefix_run = TextRunIr {
                text: format!("{num_prefix}{sep}"),
                font_family: first_font.or_else(|| Some("Libertinus Serif".to_string())),
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
