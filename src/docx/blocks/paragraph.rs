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
        out.push_str("<w:p><w:pPr>");
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
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }

    pub(crate) fn compute_line_spacing_dxa(
        content: &[InlineNode],
        line_spacing_pt: Option<f64>,
    ) -> i64 {
        let base_font_pt = content
            .iter()
            .find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_size_pt,
                _ => None,
            })
            .unwrap_or(11.0)
            .max(6.0);
        let is_cjk = Self::content_has_cjk(content);
        let has_inline_eq = content
            .iter()
            .any(|inl| matches!(inl, InlineNode::EquationInline(_)));
        if has_inline_eq && !content.iter().any(|inl| matches!(inl, InlineNode::Text(_))) {
            return 276;
        }
        if let Some(ls_pt) = line_spacing_pt {
            let ratio = (ls_pt / base_font_pt).clamp(0.1, 2.0);
            if is_cjk && ls_pt > 3.5 {
                if has_inline_eq && base_font_pt >= 14.0 {
                    return 240;
                }
                // In Word, CJK fonts (`PingFang SC`, `Songti SC`, `Heiti SC`) have a 1.0x (`w:line="240"`)
                // line height of ~`1.38 * font_size_pt`, whereas pure-ASCII runs mapped to `Arial`
                // have a 1.0x line height of ~`1.15 * font_size_pt`.
                let metric_div = if Self::content_has_cjk_chars(content) {
                    1.38
                } else {
                    1.15
                };
                ((240.0 * (0.95 + ratio) / metric_div).round() as i64).clamp(240, 720)
            } else if ratio <= 1.05 {
                (212.0 + ratio * 24.0).round() as i64
            } else {
                ((236.0 * (1.0 + ratio) / 2.0).round() as i64).clamp(180, 720)
            }
        } else if is_cjk {
            276
        } else {
            242
        }
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
                r.text.chars().any(|c| {
                    ('\u{4E00}'..='\u{9FFF}').contains(&c)
                        || ('\u{3000}'..='\u{303F}').contains(&c)
                        || ('\u{FF00}'..='\u{FFEF}').contains(&c)
                }) || r.font_family.as_deref().is_some_and(|f| {
                    let fl = f.to_ascii_lowercase();
                    fl.contains("pingfang")
                        || fl.contains("songti")
                        || fl.contains("heiti")
                        || fl.contains("kaiti")
                        || fl.contains("fangsong")
                        || fl.contains("simsun")
                        || fl.contains("simhei")
                        || fl.contains("yahei")
                })
            }
            _ => false,
        })
    }
}
