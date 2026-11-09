//! Word Paragraph Construct (`<w:p>` / `<w:pPr>`).
//!
//! Solves `<w:spacing>`, `<w:ind>`, `<w:jc>`, `<w:tabs>`, paragraph-mark `<w:rPr>`,
//! `OS/2` line-box height (`w:line`), and inter-paragraph font-size transition spacing.

use ir::{BlockNode, HAlignIr, InlineNode, ParagraphStyleIr};
use crate::units::pt_to_dxa;

/// Exhaustive horizontal indentation construct for `<w:ind>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphIndentationConstruct {
    /// No `<w:ind>` element needed (`left == 0 && right == 0 && first == 0 && hanging == 0`).
    None,
    /// Hanging indent (`<w:ind w:left="..." w:right="..." w:hanging="..."/>`).
    Hanging {
        left_dxa: i64,
        right_dxa: i64,
        hanging_dxa: i64,
    },
    /// First-line indent (`<w:ind w:left="..." w:right="..." w:firstLine="..."/>`).
    FirstLine {
        left_dxa: i64,
        right_dxa: i64,
        first_line_dxa: i64,
    },
    /// Left/right indentation only (`<w:ind w:left="..." w:right="..."/>`).
    LeftRightOnly { left_dxa: i64, right_dxa: i64 },
}

impl ParagraphIndentationConstruct {
    pub fn solve(style: &ParagraphStyleIr) -> Self {
        let left_dxa = pt_to_dxa(style.left_indent_pt).max(0);
        let right_dxa = pt_to_dxa(style.right_indent_pt).max(0);
        let first_dxa = pt_to_dxa(style.first_line_indent_pt).max(0);
        let hang_dxa = pt_to_dxa(style.hanging_indent_pt).max(0);

        match (left_dxa > 0 || right_dxa > 0, hang_dxa > 0, first_dxa > 0) {
            (_, true, _) => Self::Hanging {
                left_dxa: left_dxa + hang_dxa,
                right_dxa,
                hanging_dxa: hang_dxa,
            },
            (_, false, true) => Self::FirstLine {
                left_dxa,
                right_dxa,
                first_line_dxa: first_dxa,
            },
            (true, false, false) => Self::LeftRightOnly {
                left_dxa,
                right_dxa,
            },
            (false, false, false) => Self::None,
        }
    }
}

/// Complete solved construct for a Word `<w:p>` paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphConstruct {
    pub align: HAlignIr,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
    pub indentation: ParagraphIndentationConstruct,
    pub right_tab_stop_dxa: Option<i64>,
    pub keep_with_next: bool,
    pub keep_lines: bool,
    pub mark_rpr_sz_half_pt: Option<u32>,
    pub needs_zwsp_for_inline_eq: bool,
}

impl ParagraphConstruct {
    /// Solve a [`ParagraphConstruct`] from `content`, `style`, and active column width.
    pub fn solve(
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        content_width_dxa: i64,
        keep_with_next_override: bool,
    ) -> Self {
        let has_right_tab = has_large_horizontal_space(content);
        let align = if has_right_tab && style.align == HAlignIr::Justify {
            HAlignIr::Left
        } else {
            style.align
        };
        let right_tab_stop_dxa = has_right_tab.then_some(content_width_dxa);
        let before_dxa = pt_to_dxa(style.space_before_pt).max(0);
        let after_dxa = pt_to_dxa(style.space_after_pt).max(0);
        let line_dxa = compute_line_spacing_dxa(content, style.line_spacing_pt);
        let indentation = ParagraphIndentationConstruct::solve(style);

        Self {
            align,
            before_dxa,
            after_dxa,
            line_dxa,
            indentation,
            right_tab_stop_dxa,
            keep_with_next: keep_with_next_override || style.keep_with_next,
            keep_lines: style.keep_lines,
            mark_rpr_sz_half_pt: paragraph_mark_rpr_sz_half_pt(content),
            needs_zwsp_for_inline_eq: is_only_inline_equation(content),
        }
    }

    /// Solve effective [`ParagraphStyleIr`] when a paragraph is immediately followed by another
    /// paragraph of a different font size (`cur_sz != next_sz`).
    pub fn solve_adjacent_paragraph_style(
        content: &[InlineNode],
        style: &ParagraphStyleIr,
        next_block: Option<&BlockNode>,
    ) -> ParagraphStyleIr {
        let mut adj_style = style.clone();
        if style.space_after_pt <= 8.0
            && let Some(BlockNode::Paragraph {
                content: next_content,
                ..
            }) = next_block
        {
            let cur_sz = primary_font_size_pt(content);
            let next_sz = primary_font_size_pt(next_content);
            if cur_sz <= 13.5 && next_sz >= 15.0 && next_sz >= cur_sz + 2.0 {
                adj_style.space_after_pt =
                    (adj_style.space_after_pt - (next_sz - cur_sz) * 1.2).max(0.0);
            } else if cur_sz <= 14.5 && next_sz <= 14.5 && (next_sz - cur_sz).abs() >= 0.5 {
                let max_sz = cur_sz.max(next_sz);
                adj_style.space_after_pt =
                    (1.20 * max_sz - 0.65 * cur_sz - 0.229 * (next_sz - cur_sz)).max(1.5);
            }
        }
        adj_style
    }
}

/// Primary font size (`pt`) of the first text or inline equation in `content`.
pub fn primary_font_size_pt(content: &[InlineNode]) -> f64 {
    content
        .iter()
        .find_map(|inl| match inl {
            InlineNode::Text(r) => r.font_size_pt,
            InlineNode::EquationInline(eq) => eq.font_size_pt,
            _ => None,
        })
        .unwrap_or(11.0)
}

/// Whether `content` contains a large `#h(...)` (`>= 18pt`) that maps to a right-aligned tab stop.
pub fn has_large_horizontal_space(content: &[InlineNode]) -> bool {
    content
        .iter()
        .any(|inl| matches!(inl, InlineNode::HorizontalSpace { width_pt } if *width_pt >= 18.0))
}

/// Whether `content` contains at least one inline equation and zero non-empty text runs,
/// requiring a trailing zero-width space (`&#8203;`) so Word does not promote `<m:oMath>`
/// to centered Display Math (`<m:oMathPara>`).
pub fn is_only_inline_equation(content: &[InlineNode]) -> bool {
    let has_inline_eq = content
        .iter()
        .any(|inl| matches!(inl, InlineNode::EquationInline(_)));
    has_inline_eq
        && !content
            .iter()
            .any(|inl| matches!(inl, InlineNode::Text(r) if !r.text.is_empty()))
}

/// Optional `<w:rPr><w:sz/></w:rPr>` on `<w:pPr>` when the paragraph's text size differs from 11pt (`22` half-pts).
pub fn paragraph_mark_rpr_sz_half_pt(content: &[InlineNode]) -> Option<u32> {
    let sz_pt = content.iter().find_map(|inl| match inl {
        InlineNode::Text(r) => r.font_size_pt,
        _ => None,
    })?;
    let sz_val = (sz_pt.clamp(2.0, 200.0) * 2.0).round() as u32;
    (sz_val != 22).then_some(sz_val)
}

/// Solve Word `w:line` (`1/240` line units) for `content` via [`crate::WordLineBox`].
pub fn compute_line_spacing_dxa(content: &[InlineNode], line_spacing_pt: Option<f64>) -> i64 {
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
            .filter(|f| crate::WordFontMetricsProvider::is_cjk_codepage_font(Some(f))),
        _ => None,
    });
    let first_font = cjk_font.or_else(|| {
        content.iter().find_map(|inl| match inl {
            InlineNode::Text(r) => r.font_family.as_deref(),
            _ => None,
        })
    });
    let has_cjk_chars = content_has_cjk_chars(content);
    let is_cjk = content_has_cjk(content);
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
    crate::WordLineBox::from_typst_metrics(
        eff_font,
        has_cjk_chars,
        has_inline_eq,
        only_inline_eq,
        base_font_pt,
        line_spacing_pt,
    )
    .w_line_240
}

pub fn content_has_cjk_chars(content: &[InlineNode]) -> bool {
    content.iter().any(|inl| match inl {
        InlineNode::Text(r) => r.text_script.has_cjk(),
        _ => false,
    })
}

pub fn content_has_cjk(content: &[InlineNode]) -> bool {
    content.iter().any(|inl| match inl {
        InlineNode::Text(r) => r.text_script.has_cjk() || r.font_class.is_cjk_font(),
        _ => false,
    })
}
