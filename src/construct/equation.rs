//! Word Equation Construct (`<m:oMath>` / `<m:oMathPara>` & 2-DoF Box Calibrator).
//!
//! Bridges [`EquationIr`] (pure Typst AST + PDF physical measurements) to
//! [`word_construct::WordEquationBox`] and solves the block equation container construct
//! ([`EquationBlockConstruct`]: centered display paragraph vs 3-column borderless numbered row).

use crate::ir::{EquationIr, EquationPdfMetricsIr, MathFracStyleIr, MathNodeIr};
use crate::units::pt_to_dxa;

/// Exhaustive block equation container construct in WordprocessingML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquationBlockConstruct<'a> {
    /// Standard unnumbered display equation (`<w:p>` containing `<m:oMathPara>`).
    CenteredDisplayParagraph {
        indent_left_dxa: usize,
        before_dxa: i64,
        after_dxa: i64,
    },
    /// Numbered display equation rendered as Word's standard 3-column borderless layout row (`[12% | 76% | 12%]`).
    NumberedThreeColumnRow {
        total_dxa: i64,
        side_dxa: i64,
        center_dxa: i64,
        num_half_pts: u32,
        number_text: &'a str,
    },
}

impl<'a> EquationBlockConstruct<'a> {
    pub fn solve(
        eq: &'a EquationIr,
        page_content_width_pt: f64,
        before_dxa: i64,
        after_dxa: i64,
        indent_left_dxa: usize,
    ) -> Self {
        if let Some(num) = eq.numbering_text.as_deref() {
            let content_w_pt = page_content_width_pt.max(360.0);
            let total_dxa = pt_to_dxa(content_w_pt);
            let side_dxa = (total_dxa * 12 / 100).max(720);
            let center_dxa = (total_dxa - 2 * side_dxa).max(2880);
            let num_half_pts =
                ((eq.font_size_pt.unwrap_or(11.0) * 2.0).round() as u32).clamp(8, 144);
            Self::NumberedThreeColumnRow {
                total_dxa,
                side_dxa,
                center_dxa,
                num_half_pts,
                number_text: num,
            }
        } else {
            Self::CenteredDisplayParagraph {
                indent_left_dxa,
                before_dxa,
                after_dxa,
            }
        }
    }
}

/// Structural metrics extracted from a `MathNodeIr` AST for [`word_construct::WordEquationBox`].
#[derive(Debug, Clone, Copy)]
pub struct MathAstStructure {
    pub row_count: usize,
    pub max_fraction_depth: usize,
    pub has_tall_construct: bool,
    pub all_glyphs_in_fraction_or_script: bool,
    pub expected_em_width: f64,
}

/// Solve [`word_construct::WordEquationBox`] for an [`EquationIr`] from its `EquationPdfMetricsIr` and AST structure.
pub fn solve_word_equation_box(eq: &EquationIr) -> word_construct::WordEquationBox {
    let ast_metrics = analyze_math_slice(&eq.body, eq.is_block);
    let (
        typst_width_pt,
        typst_height_pt,
        typst_max_glyph_pt,
        typst_min_glyph_pt,
        context_font_size_pt,
        max_available_width_pt,
    ) = match eq.pdf_metrics {
        EquationPdfMetricsIr::Unmeasured {
            context_font_size_pt,
            max_glyph_pt,
            min_glyph_pt,
            max_available_width_pt,
        } => (
            None,
            None,
            max_glyph_pt,
            min_glyph_pt,
            context_font_size_pt,
            max_available_width_pt,
        ),
        EquationPdfMetricsIr::Measured {
            context_font_size_pt,
            width_pt,
            height_pt,
            max_glyph_pt,
            min_glyph_pt,
            max_available_width_pt,
        } => (
            Some(width_pt),
            Some(height_pt),
            max_glyph_pt,
            min_glyph_pt,
            context_font_size_pt,
            max_available_width_pt,
        ),
    };
    word_construct::WordEquationBox::solve(word_construct::EquationBoxInput {
        is_block: eq.is_block,
        typst_width_pt,
        typst_height_pt,
        typst_max_glyph_pt,
        typst_min_glyph_pt,
        context_font_size_pt,
        max_available_width_pt,
        row_count: ast_metrics.row_count,
        max_fraction_depth: ast_metrics.max_fraction_depth,
        has_tall_construct: ast_metrics.has_tall_construct,
        all_glyphs_in_fraction_or_script: ast_metrics.all_glyphs_in_fraction_or_script,
        expected_em_width: ast_metrics.expected_em_width,
    })
}

/// Return a calibrated copy of `eq.body` with `run_font_size_pt`, `fraction_font_size_pt`,
/// and `width_scale_pct` solved by [`word_construct::WordEquationBox`].
pub fn calibrate_equation_body(eq: &EquationIr) -> Vec<MathNodeIr> {
    let eq_box = solve_word_equation_box(eq);
    let mut calibrated_body = eq.body.clone();
    apply_calibrated_box(
        &mut calibrated_body,
        eq_box.run_font_size_pt,
        eq_box.fraction_font_size_pt,
        eq_box.width_scale_pct,
    );
    calibrated_body
}

pub fn analyze_math_slice(nodes: &[MathNodeIr], is_block: bool) -> MathAstStructure {
    let mut row_count = 1usize;
    let mut max_fraction_depth = 0usize;
    let mut has_tall_construct = false;
    let mut has_root_unscaled_glyph = false;
    let mut has_any_glyph = false;
    let mut expected_em_width = 0.0_f64;

    for node in nodes {
        let (r, f, tall, root_g, any_g) = inspect_node_structure(node, false);
        row_count = row_count.max(r);
        max_fraction_depth = max_fraction_depth.max(f);
        has_tall_construct |= tall;
        has_root_unscaled_glyph |= root_g;
        has_any_glyph |= any_g;
        expected_em_width += estimate_node_em_width(node, is_block && row_count == 1);
    }

    MathAstStructure {
        row_count,
        max_fraction_depth,
        has_tall_construct,
        all_glyphs_in_fraction_or_script: has_any_glyph && !has_root_unscaled_glyph,
        expected_em_width: expected_em_width.max(0.5),
    }
}

pub fn apply_calibrated_box(
    nodes: &mut [MathNodeIr],
    run_font_size_pt: f64,
    fraction_font_size_pt: Option<f64>,
    width_scale_pct: Option<u32>,
) {
    for node in nodes {
        apply_calibrated_box_inner(
            node,
            run_font_size_pt,
            fraction_font_size_pt,
            width_scale_pct,
            false,
        );
    }
}

fn apply_calibrated_box_inner(
    node: &mut MathNodeIr,
    run_font_size_pt: f64,
    fraction_font_size_pt: Option<f64>,
    width_scale_pct: Option<u32>,
    in_vertical_fraction: bool,
) {
    match node {
        MathNodeIr::Sequence(children) => {
            for ch in children {
                apply_calibrated_box_inner(
                    ch,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Fraction { num, denom, style } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let next_in_frac = in_vertical_fraction || is_vert;
            apply_calibrated_box_inner(
                num,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                next_in_frac,
            );
            apply_calibrated_box_inner(
                denom,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                next_in_frac,
            );
        }
        MathNodeIr::Radical { degree, radicand } => {
            if let Some(deg) = degree {
                apply_calibrated_box_inner(
                    deg,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            apply_calibrated_box_inner(
                radicand,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            apply_calibrated_box_inner(
                base,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
            if let Some(s) = sub {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            if let Some(s) = sup {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Limit { base, lower, upper } => {
            apply_calibrated_box_inner(
                base,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
            if let Some(l) = lower {
                apply_calibrated_box_inner(
                    l,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            if let Some(u) = upper {
                apply_calibrated_box_inner(
                    u,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            if let Some(s) = sub {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            if let Some(s) = sup {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            apply_calibrated_box_inner(
                operand,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::Delimited { elements, .. } => {
            for el in elements {
                apply_calibrated_box_inner(
                    el,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Matrix { rows } => {
            for row in rows {
                for cell in row {
                    apply_calibrated_box_inner(
                        cell,
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
            }
        }
        MathNodeIr::EquationArray { rows } => {
            for row in rows {
                apply_calibrated_box_inner(
                    row,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Accent { base, .. } => {
            apply_calibrated_box_inner(
                base,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            apply_calibrated_box_inner(
                body,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            apply_calibrated_box_inner(
                body,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
            if let Some(ann) = annotation {
                apply_calibrated_box_inner(
                    ann,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::AlignPoint => {}
        MathNodeIr::Run(run) => {
            let target_sz = if in_vertical_fraction {
                fraction_font_size_pt.unwrap_or(run_font_size_pt)
            } else {
                run_font_size_pt
            };
            run.font_size_pt = Some(target_sz);
            run.width_scale_pct = width_scale_pct;
        }
    }
}

fn inspect_node_structure(
    node: &MathNodeIr,
    in_script_or_frac: bool,
) -> (usize, usize, bool, bool, bool) {
    match node {
        MathNodeIr::Sequence(children) => {
            let mut rows = 1usize;
            let mut frac_d = 0usize;
            let mut tall = false;
            let mut root_g = false;
            let mut any_g = false;
            for ch in children {
                let (r, f, t, rg, ag) = inspect_node_structure(ch, in_script_or_frac);
                rows = rows.max(r);
                frac_d = frac_d.max(f);
                tall |= t;
                root_g |= rg;
                any_g |= ag;
            }
            (rows, frac_d, tall, root_g, any_g)
        }
        MathNodeIr::Fraction { num, denom, style } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let (_, f1, t1, _, ag1) = inspect_node_structure(num, true);
            let (_, f2, t2, _, ag2) = inspect_node_structure(denom, true);
            let f = if is_vert { 1 + f1.max(f2) } else { f1.max(f2) };
            (1, f, is_vert || t1 || t2, false, ag1 || ag2)
        }
        MathNodeIr::Radical { degree, radicand } => {
            let (_, f1, _, _, ag1) = degree.as_ref().map_or((1, 0, false, false, false), |d| {
                inspect_node_structure(d, true)
            });
            let (_, f2, _, rg2, ag2) = inspect_node_structure(radicand, in_script_or_frac);
            (1, f1.max(f2), true, rg2, ag1 || ag2)
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            let (_, f0, t0, rg0, ag0) = inspect_node_structure(base, in_script_or_frac);
            let (_, f1, _, _, ag1) = sub.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            let (_, f2, _, _, ag2) = sup.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            (
                1,
                f0.max(f1).max(f2),
                t0 || sub.is_some() || sup.is_some(),
                rg0,
                ag0 || ag1 || ag2,
            )
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let (_, f0, _, rg0, ag0) = inspect_node_structure(base, in_script_or_frac);
            let (_, f1, _, _, ag1) = lower.as_ref().map_or((1, 0, false, false, false), |l| {
                inspect_node_structure(l, true)
            });
            let (_, f2, _, _, ag2) = upper.as_ref().map_or((1, 0, false, false, false), |u| {
                inspect_node_structure(u, true)
            });
            (1, f0.max(f1).max(f2), true, rg0, ag0 || ag1 || ag2)
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            let (_, f1, _, _, _) = sub.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            let (_, f2, _, _, _) = sup.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            let (_, f3, _, rg3, _) = inspect_node_structure(operand, in_script_or_frac);
            (1, f1.max(f2).max(f3), true, rg3, true)
        }
        MathNodeIr::Delimited { elements, .. } => {
            let mut rows = 1usize;
            let mut frac_d = 0usize;
            let mut tall = false;
            let mut root_g = false;
            let mut any_g = false;
            for el in elements {
                let (r, f, t, rg, ag) = inspect_node_structure(el, in_script_or_frac);
                rows = rows.max(r);
                frac_d = frac_d.max(f);
                tall |= t;
                root_g |= rg;
                any_g |= ag;
            }
            (rows, frac_d, tall, root_g, any_g)
        }
        MathNodeIr::Matrix { rows } => {
            let r_cnt = rows.len().max(1);
            let mut frac_d = 0usize;
            let mut root_g = false;
            let mut any_g = false;
            for row in rows {
                for cell in row {
                    let (_, f, _, rg, ag) = inspect_node_structure(cell, in_script_or_frac);
                    frac_d = frac_d.max(f);
                    root_g |= rg;
                    any_g |= ag;
                }
            }
            (r_cnt, frac_d, r_cnt > 1, root_g, any_g)
        }
        MathNodeIr::EquationArray { rows } => {
            let r_cnt = rows.len().max(1);
            let mut frac_d = 0usize;
            let mut root_g = false;
            let mut any_g = false;
            for row in rows {
                let (_, f, _, rg, ag) = inspect_node_structure(row, in_script_or_frac);
                frac_d = frac_d.max(f);
                root_g |= rg;
                any_g |= ag;
            }
            (r_cnt, frac_d, r_cnt > 1, root_g, any_g)
        }
        MathNodeIr::Accent { base, .. } => {
            let (_, f, _, rg, ag) = inspect_node_structure(base, in_script_or_frac);
            (1, f, true, rg, ag)
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            inspect_node_structure(body, in_script_or_frac)
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            let (_, f1, _, rg1, ag1) = inspect_node_structure(body, in_script_or_frac);
            let (_, f2, _, _, ag2) = annotation
                .as_ref()
                .map_or((1, 0, false, false, false), |a| {
                    inspect_node_structure(a, true)
                });
            (1, f1.max(f2), true, rg1, ag1 || ag2)
        }
        MathNodeIr::AlignPoint => (1, 0, false, false, false),
        MathNodeIr::Run(run) => {
            let non_empty = !run.text.trim().is_empty();
            (1, 0, false, non_empty && !in_script_or_frac, non_empty)
        }
    }
}

fn estimate_node_em_width(node: &MathNodeIr, in_display_context: bool) -> f64 {
    match node {
        MathNodeIr::Sequence(children) => children
            .iter()
            .map(|c| estimate_node_em_width(c, in_display_context))
            .sum(),
        MathNodeIr::Fraction { num, denom, style } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let child_scale = if is_vert && !in_display_context {
                0.73
            } else {
                1.0
            };
            let nw = estimate_node_em_width(num, false) * child_scale;
            let dw = estimate_node_em_width(denom, false) * child_scale;
            if is_vert {
                nw.max(dw) + 0.35
            } else {
                nw + dw + 0.45
            }
        }
        MathNodeIr::Radical { degree, radicand } => {
            let deg_w = degree
                .as_ref()
                .map_or(0.0, |d| estimate_node_em_width(d, false) * 0.65);
            deg_w + 0.85 + estimate_node_em_width(radicand, in_display_context)
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            let bw = estimate_node_em_width(base, in_display_context);
            let sub_w = sub
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            let sup_w = sup
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            bw + sub_w.max(sup_w)
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let bw = estimate_node_em_width(base, in_display_context);
            let lw = lower
                .as_ref()
                .map_or(0.0, |l| estimate_node_em_width(l, false) * 0.70);
            let uw = upper
                .as_ref()
                .map_or(0.0, |u| estimate_node_em_width(u, false) * 0.70);
            bw.max(lw).max(uw) + 0.15
        }
        MathNodeIr::Nary {
            limits_under_over,
            sub,
            sup,
            operand,
            ..
        } => {
            let sub_w = sub
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            let sup_w = sup
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            let op_w = estimate_node_em_width(operand, in_display_context);
            if *limits_under_over {
                1.15_f64.max(sub_w).max(sup_w) + 0.20 + op_w
            } else {
                1.05 + sub_w.max(sup_w) + 0.15 + op_w
            }
        }
        MathNodeIr::Delimited {
            open,
            close,
            separator,
            elements,
        } => {
            let open_w = if open.is_empty() { 0.0 } else { 0.42 };
            let close_w = if close.is_empty() { 0.0 } else { 0.42 };
            let sep_w = if separator.is_empty() { 0.0 } else { 0.35 };
            let elems_w: f64 = elements
                .iter()
                .map(|e| estimate_node_em_width(e, in_display_context))
                .sum();
            let seps_total = sep_w * elements.len().saturating_sub(1) as f64;
            open_w + elems_w + seps_total + close_w
        }
        MathNodeIr::Matrix { rows } => {
            let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut total_w = 0.0;
            for c in 0..num_cols {
                let col_max = rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .map(|cell| estimate_node_em_width(cell, false))
                    .fold(0.0_f64, f64::max);
                total_w += col_max;
            }
            total_w + num_cols.saturating_sub(1) as f64 * 0.85
        }
        MathNodeIr::EquationArray { rows } => rows
            .iter()
            .map(|r| estimate_node_em_width(r, true))
            .fold(0.0_f64, f64::max),
        MathNodeIr::Accent { base, .. }
        | MathNodeIr::Bar { body: base, .. }
        | MathNodeIr::Cancel { body: base, .. } => estimate_node_em_width(base, in_display_context),
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            let bw = estimate_node_em_width(body, in_display_context);
            let aw = annotation
                .as_ref()
                .map_or(0.0, |a| estimate_node_em_width(a, false) * 0.75);
            bw.max(aw)
        }
        MathNodeIr::AlignPoint => 0.0,
        MathNodeIr::Run(run) => run
            .text
            .chars()
            .map(|ch| {
                if word_construct::is_cjk_or_fullwidth_char(ch) {
                    1.0
                } else if ch.is_whitespace() {
                    0.28
                } else if matches!(
                    ch,
                    '+' | '-' | '=' | '−' | '×' | '÷' | '±' | '<' | '>' | '≤' | '≥' | '≠' | '≈'
                        | '→' | '←' | '⇒' | '⇔'
                ) {
                    0.78
                } else if ch.is_ascii_digit() {
                    0.55
                } else if ch.is_ascii_uppercase() {
                    0.66
                } else {
                    0.52
                }
            })
            .sum(),
    }
}
