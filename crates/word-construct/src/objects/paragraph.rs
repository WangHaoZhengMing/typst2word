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
    pub rtl: bool,
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
        let has_right_tab = has_flush_right_horizontal_space(content, content_width_dxa);
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
            keep_with_next: keep_with_next_override || style.sticky,
            keep_lines: !style.breakable,
            rtl: style.dir == Some(ir::TextDirIr::Rtl),
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
            } else if cur_sz <= 14.5
                && next_sz <= 14.5
                && (next_sz - cur_sz).abs() >= 0.5
                && !matches!(
                    (style.resolved_par_spacing_pt, style.resolved_par_leading_pt),
                    (Some(par_spacing_pt), Some(par_leading_pt))
                        if (par_spacing_pt - par_leading_pt).abs() < 0.1
                )
            {
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

/// Estimate inline width (`pt`) of `content` for distinguishing `#h(1fr)` flush-right tabs from fixed `#h(36pt)`.
fn estimate_inlines_width_pt(content: &[InlineNode]) -> f64 {
    content
        .iter()
        .map(|inl| match inl {
            InlineNode::Text(r) => {
                let sz = r.font_size_pt.unwrap_or(11.0);
                r.text
                    .chars()
                    .map(|ch| if ch as u32 > 0x2E80 { sz } else { sz * 0.52 })
                    .sum::<f64>()
            }
            InlineNode::HorizontalSpace { width_pt } => *width_pt,
            InlineNode::ShapeInline(s) => s.width_pt,
            InlineNode::ImageInline(i) => i.width_pt,
            InlineNode::CetzInline(c) => c.width_pt,
            InlineNode::Field { content: fc, .. }
            | InlineNode::Link { content: fc, .. }
            | InlineNode::Ref { content: fc, .. }
            | InlineNode::Citation { content: fc, .. } => estimate_inlines_width_pt(fc),
            _ => 0.0,
        })
        .sum()
}

/// Whether `content` contains a single `#h(1fr)`-style fill space reaching the right margin
/// (as opposed to a fixed `#h(36pt)` horizontal spacer mid-line).
pub fn has_flush_right_horizontal_space(content: &[InlineNode], content_width_dxa: i64) -> bool {
    let mut large_count = 0usize;
    let mut max_space_pt = 0.0_f64;
    for inl in content {
        if let InlineNode::HorizontalSpace { width_pt } = inl
            && *width_pt >= 18.0
        {
            large_count += 1;
            max_space_pt = max_space_pt.max(*width_pt);
        }
    }
    if large_count != 1 {
        return false;
    }
    let content_w_pt = (content_width_dxa as f64 / 20.0).max(72.0);
    let est_line_w_pt = estimate_inlines_width_pt(content);
    max_space_pt >= content_w_pt * 0.35 || est_line_w_pt >= content_w_pt * 0.85
}

/// Whether `content` contains a large `#h(...)` (`>= 18pt`).
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
        InlineNode::Text(r) => crate::objects::text_run::run_has_cjk_chars(r),
        _ => false,
    })
}

pub fn content_has_cjk(content: &[InlineNode]) -> bool {
    content.iter().any(|inl| match inl {
        InlineNode::Text(r) => crate::objects::text_run::run_has_cjk(r),
        _ => false,
    })
}

/// Solved inline item inside a justified Knuth-Plass paragraph when glue compensation is required
/// so Microsoft Word's greedy `PTLS7` line-breaker matches Typst's optimal breaks without `<w:br/>`.
#[derive(Debug, Clone)]
pub enum ParagraphInlineConstruct<'a> {
    /// Unmodified inline node borrowed directly from IR.
    Passthrough(&'a InlineNode),
    /// Sliced `TextRunIr` segment carrying an explicit Word `<w:spacing w:val="..."/>` (`dxa`) adjustment.
    AdjustedText {
        run: ir::TextRunIr,
        char_spacing_dxa: Option<i32>,
    },
}

impl ParagraphConstruct {
    /// Solve per-glue `<w:spacing>` adjustments (`dxa`) when a justified Knuth-Plass paragraph
    /// shrinks inter-word spaces or fullwidth CJK punctuation (or breaks early to avoid a runt)
    /// relative to Word's greedy `FirstFit` engine. Returns `None` when Word's native `FirstFit`
    /// already produces identical line breaks.
    pub fn solve_inlines<'a>(
        content: &'a [InlineNode],
        style: &ParagraphStyleIr,
        content_width_dxa: i64,
    ) -> Option<Vec<ParagraphInlineConstruct<'a>>> {
        if style.align != HAlignIr::Justify
            || has_flush_right_horizontal_space(content, content_width_dxa)
        {
            return None;
        }
        let policy =
            crate::LineBreakPolicy::from_typst_linebreaks(style.linebreaks, true);
        let crate::LineBreakPolicy::KnuthPlass {
            hyph_cost,
            runt_cost,
            consecutive_dash_cost,
            max_cost,
            min_ratio,
        } = policy
        else {
            return None;
        };

        if content.is_empty()
            || !content.iter().all(|inl| match inl {
                InlineNode::Text(r) => r.script == ir::ScriptKindIr::Normal && !r.smallcaps,
                InlineNode::LineBreak => true,
                _ => false,
            })
        {
            return None;
        }

        let default_fam = if content_has_cjk_chars(content) {
            "Heiti SC"
        } else {
            "Libertinus Serif"
        };

        let mut spans = Vec::with_capacity(content.len());
        let mut total_chars = 0usize;
        for inl in content {
            match inl {
                InlineNode::Text(r) => {
                    if r.text.is_empty() {
                        continue;
                    }
                    let sz = r.font_size_pt.unwrap_or(11.0);
                    let fam = r
                        .font_family
                        .as_deref()
                        .unwrap_or(if r.is_code { "Courier New" } else { default_fam });
                    spans.push(crate::InlineSpan::new(r.text.clone(), sz).with_font(fam));
                    total_chars += r.text.chars().count();
                }
                InlineNode::LineBreak => {
                    spans.push(crate::InlineSpan::new("\n", 11.0));
                    total_chars += 1;
                }
                _ => return None,
            }
        }
        if total_chars == 0 {
            return None;
        }

        let metrics = crate::WordFontMetricsProvider::new();
        let orig_tokens = crate::tokenize_spans(&spans, &metrics);
        let mut tok_char_starts = Vec::with_capacity(orig_tokens.len());
        let mut char_cursor = 0usize;
        for tok in &orig_tokens {
            tok_char_starts.push(char_cursor);
            char_cursor += tok.text.chars().count();
        }
        if char_cursor != total_chars {
            return None;
        }

        let left_dxa = pt_to_dxa(style.left_indent_pt).max(0);
        let right_dxa = pt_to_dxa(style.right_indent_pt).max(0);
        let first_dxa = pt_to_dxa(style.first_line_indent_pt).max(0);
        let hang_dxa = pt_to_dxa(style.hanging_indent_pt).max(0);

        let base_avail_pt = ((content_width_dxa - left_dxa - right_dxa).max(200) as f64) / 20.0;
        let first_indent_pt = (first_dxa as f64) / 20.0;
        let hang_indent_pt = (hang_dxa as f64) / 20.0;

        let kinsoku = crate::KinsokuSet::default();
        let ff_lines = crate::FirstFitBreaker.break_tokens_with_indents(
            &orig_tokens,
            base_avail_pt,
            first_indent_pt,
            hang_indent_pt,
            true,
            &kinsoku,
        );
        let kp_breaker = crate::KnuthPlassBreaker {
            hyph_cost,
            runt_cost,
            consecutive_dash_cost,
            max_cost,
            min_ratio,
        };
        let kp_lines = kp_breaker.break_tokens_with_indents(
            &orig_tokens,
            base_avail_pt,
            first_indent_pt,
            hang_indent_pt,
            true,
            &kinsoku,
        );

        if ff_lines.len() == kp_lines.len()
            && ff_lines
                .iter()
                .zip(&kp_lines)
                .all(|(a, b)| a.visible_text() == b.visible_text())
        {
            return None;
        }

        // Map each broken line's tokens back to `orig_tokens` indices in O(N).
        let mut line_orig_indices: Vec<Vec<usize>> = Vec::with_capacity(kp_lines.len());
        let mut orig_cursor = 0usize;
        for line in &kp_lines {
            let mut idxs = Vec::with_capacity(line.tokens.len());
            for ltok in &line.tokens {
                while orig_cursor < orig_tokens.len() && orig_tokens[orig_cursor].text != ltok.text {
                    orig_cursor += 1;
                }
                if orig_cursor >= orig_tokens.len() {
                    return None;
                }
                idxs.push(orig_cursor);
                orig_cursor += 1;
            }
            line_orig_indices.push(idxs);
        }

        let mut char_dxa_adj = vec![0_i32; total_chars];

        for (line_idx, line) in kp_lines.iter().enumerate() {
            let idxs = &line_orig_indices[line_idx];
            if idxs.is_empty() {
                continue;
            }
            let word_avail_pt = if line_idx == 0 {
                base_avail_pt - first_indent_pt
            } else {
                base_avail_pt - hang_indent_pt
            };
            let word_line_w_pt: f64 = line.tokens.iter().map(|t| t.width_pt).sum();

            if word_line_w_pt > word_avail_pt - 0.05 {
                // Case 1: Overfull shrunk line — attach negative `<w:spacing>` to shrinkable glues.
                let mut targets: Vec<(usize, f64)> = Vec::new();
                for (pos_in_line, &orig_i) in idxs.iter().enumerate() {
                    let tok = &orig_tokens[orig_i];
                    if tok.shrink_pt <= 1e-6 {
                        continue;
                    }
                    let c_start = tok_char_starts[orig_i];
                    if tok.is_whitespace()
                        || tok
                            .text
                            .chars()
                            .next()
                            .is_some_and(crate::linebreak::is_cjk_right_half_space_punct)
                    {
                        targets.push((c_start, tok.shrink_pt));
                    } else if pos_in_line > 0
                        && tok
                            .text
                            .chars()
                            .next()
                            .is_some_and(crate::linebreak::is_cjk_left_half_space_punct)
                    {
                        let prev_orig_i = idxs[pos_in_line - 1];
                        let prev_last_char = tok_char_starts[prev_orig_i]
                            + orig_tokens[prev_orig_i].text.chars().count()
                            - 1;
                        targets.push((prev_last_char, tok.shrink_pt));
                    }
                }
                let weight_sum: f64 = targets.iter().map(|(_, w)| *w).sum();
                if weight_sum > 1e-6 {
                    let total_shrink_dxa =
                        (((word_line_w_pt - word_avail_pt) + 0.25) * 20.0).ceil().max(1.0) as i32;
                    distribute_weighted_dxa(&targets, -total_shrink_dxa, &mut char_dxa_adj);
                }
            } else if line_idx + 1 < kp_lines.len() && !line.ended_by_mandatory_break {
                // Case 2: Underfull early break — check if Word's FirstFit would pull next line's head.
                let next_idxs = &line_orig_indices[line_idx + 1];
                if let (Some(&last_i), Some(&next_first_i)) = (idxs.last(), next_idxs.first()) {
                    let gap_space_w: f64 = orig_tokens[last_i + 1..next_first_i]
                        .iter()
                        .filter(|t| t.is_whitespace())
                        .map(|t| t.width_pt)
                        .sum();
                    let min_pull_w = min_kinsoku_pull_width_pt(&kp_lines[line_idx + 1].tokens, &kinsoku);
                    if word_line_w_pt + gap_space_w + min_pull_w <= word_avail_pt + 0.35 {
                        let ws_targets: Vec<(usize, f64)> = idxs
                            .iter()
                            .copied()
                            .filter(|&oi| orig_tokens[oi].is_whitespace())
                            .map(|oi| (tok_char_starts[oi], orig_tokens[oi].stretch_pt.max(0.5)))
                            .collect();
                        let max_safe_expand = (word_avail_pt - word_line_w_pt - 0.25).max(0.0);
                        let needed_expand_pt = ((word_avail_pt
                            - (word_line_w_pt + gap_space_w + min_pull_w))
                            + 0.40)
                            .min(max_safe_expand);
                        if needed_expand_pt > 0.05 && !ws_targets.is_empty() {
                            let expand_dxa = (needed_expand_pt * 20.0).ceil() as i32;
                            distribute_weighted_dxa(&ws_targets, expand_dxa, &mut char_dxa_adj);
                        }
                    }
                }
            }
        }

        if char_dxa_adj.iter().all(|&d| d == 0) {
            return None;
        }

        let mut out = Vec::with_capacity(content.len() + 8);
        let mut cursor = 0usize;
        for inl in content {
            match inl {
                InlineNode::LineBreak => {
                    cursor += 1;
                    out.push(ParagraphInlineConstruct::Passthrough(inl));
                }
                InlineNode::Text(r) => {
                    if r.text.is_empty() {
                        continue;
                    }
                    let chars: Vec<char> = r.text.chars().collect();
                    let adjs = &char_dxa_adj[cursor..cursor + chars.len()];
                    cursor += chars.len();
                    if adjs.iter().all(|&d| d == 0) {
                        out.push(ParagraphInlineConstruct::Passthrough(inl));
                    } else {
                        let mut seg_start = 0usize;
                        while seg_start < chars.len() {
                            let d = adjs[seg_start];
                            let mut seg_end = seg_start + 1;
                            while seg_end < chars.len() && adjs[seg_end] == d {
                                seg_end += 1;
                            }
                            let seg_text: String = chars[seg_start..seg_end].iter().collect();
                            let mut seg_run = r.clone();
                            seg_run.text = seg_text;
                            out.push(ParagraphInlineConstruct::AdjustedText {
                                run: seg_run,
                                char_spacing_dxa: (d != 0).then_some(d),
                            });
                            seg_start = seg_end;
                        }
                    }
                }
                _ => out.push(ParagraphInlineConstruct::Passthrough(inl)),
            }
        }
        Some(out)
    }
}

fn distribute_weighted_dxa(targets: &[(usize, f64)], total_dxa: i32, out_adj: &mut [i32]) {
    let weight_sum: f64 = targets.iter().map(|(_, w)| *w).sum();
    if targets.is_empty() || weight_sum <= 1e-9 || total_dxa == 0 {
        return;
    }
    let mut acc_weight = 0.0_f64;
    let mut prev_assigned = 0_i32;
    for &(char_idx, w) in targets {
        acc_weight += w;
        let target_cum = ((total_dxa as f64) * (acc_weight / weight_sum)).round() as i32;
        let delta = target_cum - prev_assigned;
        prev_assigned = target_cum;
        out_adj[char_idx] += delta;
    }
}

fn min_kinsoku_pull_width_pt(tokens: &[crate::InlineToken], kinsoku: &crate::KinsokuSet) -> f64 {
    let mut w = 0.0_f64;
    for (i, tok) in tokens.iter().enumerate() {
        w += tok.width_pt;
        if tok.is_whitespace() {
            continue;
        }
        let mut next_vis = i + 1;
        while next_vis < tokens.len() && tokens[next_vis].is_whitespace() {
            next_vis += 1;
        }
        let end_ok = !tok.is_kinsoku_line_end_forbidden(kinsoku);
        let start_ok = next_vis >= tokens.len()
            || !tokens[next_vis].is_kinsoku_line_start_forbidden(kinsoku);
        if end_ok && start_ok {
            break;
        }
    }
    w
}
