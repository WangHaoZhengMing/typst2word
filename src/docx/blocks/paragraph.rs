//! `par` -> `<w:p>` with spacing and indentation.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{InlineNode, ParagraphStyleIr};
use crate::units::pt_to_dxa;

impl<'a> DocxBuilder<'a> {
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
        out.push_str("<w:p><w:pPr>");
        if keep_with_next || style.keep_with_next {
            out.push_str("<w:keepNext/>");
        }
        if style.keep_lines {
            out.push_str("<w:keepLines/>");
        }
        let has_right_tab = Self::has_large_horizontal_space(content);
        let eff_align = if has_right_tab && style.align == crate::ir::HAlignIr::Justify {
            crate::ir::HAlignIr::Left
        } else {
            style.align
        };
        Self::render_jc(eff_align, out);
        if has_right_tab {
            let _ = write!(
                out,
                r#"<w:tabs><w:tab w:val="right" w:pos="{}"/></w:tabs>"#,
                self.content_width_dxa()
            );
        }
        let before_dxa = pt_to_dxa(style.space_before_pt).max(0);
        let after_dxa = pt_to_dxa(style.space_after_pt).max(0);
        let line_dxa = Self::compute_line_spacing_dxa(content, style.line_spacing_pt);
        let _ = write!(
            out,
            r#"<w:spacing w:before="{before_dxa}" w:after="{after_dxa}" w:line="{line_dxa}" w:lineRule="auto"/>"#
        );
        let left_dxa = pt_to_dxa(style.left_indent_pt).max(0);
        let right_dxa = pt_to_dxa(style.right_indent_pt).max(0);
        let first_dxa = pt_to_dxa(style.first_line_indent_pt).max(0);
        let hang_dxa = pt_to_dxa(style.hanging_indent_pt).max(0);
        if left_dxa > 0 || right_dxa > 0 || first_dxa > 0 || hang_dxa > 0 {
            let eff_left_dxa = if hang_dxa > 0 {
                left_dxa + hang_dxa
            } else {
                left_dxa
            };
            let mut ind = format!(r#"<w:ind w:left="{eff_left_dxa}" w:right="{right_dxa}""#);
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
        if Self::is_only_inline_equation(content) {
            // Prevent Microsoft Word from promoting a `<w:p>` containing only `<m:oMath>`
            // into centered Display Math Mode (`oMathPara` with +8pt vertical margin).
            out.push_str("<w:r><w:t>&#8203;</w:t></w:r>");
        }
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }

    pub(crate) fn is_only_inline_equation(content: &[InlineNode]) -> bool {
        let has_inline_eq = content
            .iter()
            .any(|inl| matches!(inl, InlineNode::EquationInline(_)));
        has_inline_eq
            && !content
                .iter()
                .any(|inl| matches!(inl, InlineNode::Text(r) if !r.text.is_empty()))
    }

    pub(crate) fn render_paragraph_mark_rpr(content: &[InlineNode], out: &mut String) {
        if let Some(sz_pt) = content.iter().find_map(|inl| match inl {
            InlineNode::Text(r) => r.font_size_pt,
            _ => None,
        }) {
            let sz_val = (sz_pt.clamp(2.0, 200.0) * 2.0).round() as u32;
            if sz_val != 22 {
                let _ = write!(
                    out,
                    r#"<w:rPr><w:sz w:val="{sz_val}"/><w:szCs w:val="{sz_val}"/></w:rPr>"#
                );
            }
        }
    }

    pub(crate) fn compute_line_spacing_dxa(
        content: &[InlineNode],
        line_spacing_pt: Option<f64>,
    ) -> i64 {
        let base_font_pt = content
            .iter()
            .find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_size_pt,
                InlineNode::EquationInline(eq) => eq.font_size_pt,
                _ => None,
            })
            .unwrap_or(11.0)
            .max(6.0);
        let cjk_font = content.iter().find_map(|inl| match inl {
            InlineNode::Text(r) => r
                .font_family
                .as_deref()
                .filter(|f| word_construct::WordFontMetricsProvider::is_cjk_codepage_font(Some(f))),
            _ => None,
        });
        let first_font = cjk_font.or_else(|| {
            content.iter().find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_family.as_deref(),
                _ => None,
            })
        });
        let has_cjk_chars = Self::content_has_cjk_chars(content);
        let is_cjk = Self::content_has_cjk(content);
        let has_inline_eq = content
            .iter()
            .any(|inl| matches!(inl, InlineNode::EquationInline(_)));
        let only_inline_eq =
            has_inline_eq && !content.iter().any(|inl| matches!(inl, InlineNode::Text(_)));
        let eff_font = if is_cjk && first_font.is_none() {
            Some("PingFang SC")
        } else {
            first_font
        };
        word_construct::WordLineBox::from_typst_metrics(
            eff_font,
            has_cjk_chars,
            has_inline_eq,
            only_inline_eq,
            base_font_pt,
            line_spacing_pt,
        )
        .w_line_240
    }

    pub(crate) fn content_has_cjk_chars(content: &[InlineNode]) -> bool {
        content.iter().any(|inl| match inl {
            InlineNode::Text(r) => r.text.chars().any(|c| {
                ('\u{4E00}'..='\u{9FFF}').contains(&c)
                    || ('\u{3000}'..='\u{303F}').contains(&c)
                    || ('\u{FF00}'..='\u{FFEF}').contains(&c)
            }),
            _ => false,
        })
    }

    pub(crate) fn content_has_cjk(content: &[InlineNode]) -> bool {
        content.iter().any(|inl| match inl {
            InlineNode::Text(r) => {
                word_construct::str_has_cjk_chars(&r.text)
                    || word_construct::WordFontMetricsProvider::is_cjk_codepage_font(
                        r.font_family.as_deref(),
                    )
            }
            _ => false,
        })
    }
}
