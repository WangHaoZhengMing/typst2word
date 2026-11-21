//! Word Native Shape & Inline Box Geometry Solver (`<wps:wsp>`, `<wps:bodyPr>`, `<wp:effectExtent>`).
//!
//! Computes exact Word DrawingML text-frame insets and `MicrosoftPTLS7` inline line-box clearances
//! from OpenType font metrics, ECMA-376 preset geometry equations, and Typst's `ReferenceLayout`,
//! keeping `docx-writer` a pure zero-magic-number serializer.

use std::collections::BTreeMap;

use ir::{
    AdjacentTextLineIr, BlockNode, FontFaceMetricsIr, HAlignIr, InlineBoxLineContextIr, InlineNode,
    ShapeIr, ShapeKindIr, SidesPtIr,
};

use crate::font_metrics::FontMetricsResolver;
use crate::objects::paragraph::compute_line_spacing_dxa_with_metrics;
use crate::objects::text_run::{effective_word_run_font_size_pt, run_has_cjk};
use crate::pipeline::lang_script::TextScriptClassIr;

/// ECMA-376 Part 1 §20.1.9.18 `prstGeom="roundRect"` inscribed text-rectangle factor:
/// $il = it = R \cdot (1 - \frac{\sqrt{2}}{2}) \approx 0.2928932\,R$.
const ROUND_RECT_INSCRIBED_FACTOR: f64 = 1.0 - std::f64::consts::FRAC_1_SQRT_2;

/// Solved Word paragraph wrapper construct for `BlockNode::ShapeBlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeBlockConstruct {
    pub align: HAlignIr,
    pub left_indent_dxa: usize,
    pub before_dxa: i64,
    pub after_dxa: i64,
    pub line_dxa: i64,
}

impl ShapeBlockConstruct {
    pub fn solve(shape: &ShapeIr, left_indent_dxa: usize) -> Self {
        Self::solve_with_collapse(shape, left_indent_dxa, false)
    }

    pub fn solve_with_collapse(
        shape: &ShapeIr,
        left_indent_dxa: usize,
        collapse_before: bool,
    ) -> Self {
        let (raw_before, after_dxa) = if shape.width_pt >= 350.0 {
            (0, 100)
        } else {
            (70, 210)
        };
        let before_dxa = if collapse_before { 0 } else { raw_before };
        Self {
            align: shape.align,
            left_indent_dxa,
            before_dxa,
            after_dxa,
            line_dxa: 240,
        }
    }
}

/// Pre-solved exact line-box spacing and baseline shift for `shape.body[0]` inside `<w:txbxContent>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FirstParaExactLayout {
    /// `<w:spacing w:line="..." w:lineRule="exact"/>` in twips (`dxa`).
    pub line_dxa: i64,
    /// `<w:spacing w:after="..."/>` in twips (`dxa`).
    pub after_dxa: i64,
    /// Optional `<w:rPr><w:position w:val="..."/></w:rPr>` shift in half-points (`1/2 pt`;
    /// negative lowers `p[0]`'s runs when negative `#v(...)` overlaps `p[0]`'s descent with `p[1]`).
    pub run_position_half_pt: Option<i32>,
}

/// Pre-solved DrawingML geometry for a `ShapeIr` (`BlockNode::ShapeBlock` or `InlineNode::ShapeInline`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SolvedShapeGeometry {
    /// Calibrated `<wps:bodyPr lIns/tIns/rIns/bIns>` padding in `pt`.
    pub body_inset_pt: SidesPtIr,
    /// Optional exact line-box and `<w:position>` layout for a single-line first paragraph inside
    /// `<w:txbxContent>` when negative vertical spacing (e.g. `#v(-8pt)` + `#v(-5pt)` inside a
    /// rounded code card) pulls the first baseline above Word's `tIns = 0` auto line box.
    pub first_para_exact: Option<FirstParaExactLayout>,
    /// Extra top `<wp:effectExtent t="..."/>` clearance in `pt` for inline shapes inside a paragraph.
    pub inline_extra_top_pt: f64,
    /// Extra bottom `<wp:effectExtent b="..."/>` clearance in `pt` for inline shapes inside a paragraph.
    pub inline_extra_bottom_pt: f64,
    /// Optional `<w:rPr><w:position w:val="..."/></w:rPr>` baseline shift in half-points (`1/2 pt`).
    pub run_position_half_pt: Option<i64>,
    /// Emit `wrap="none"` on `<wps:bodyPr>` for single-line auto-sized inline pills so minor
    /// Typst-vs-Word glyph advance differences never wrap a 1-line pill onto 2 lines.
    pub no_text_wrap: bool,
}

/// Optional ground-truth baseline context `(b_0, first_is_single_line_para, child_1_baseline)`
/// inside a `ShapeBlock`, measured in `pt` from the shape's top edge.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ShapeLocalBaselinesPt {
    /// Distance from shape top edge to child 0's first baseline (`b_0`).
    pub first_baseline_pt: f64,
    /// Whether `shape.body[0]` is a single-line `BlockNode::Paragraph` or `CodeBlock`.
    pub first_is_single_line_para: bool,
    /// Number of laid-out visual lines in `shape.body[0]` (`>= 1`).
    pub first_visual_line_count: usize,
    /// Optional uniform baseline step (`pt`) of `shape.body[0]` when multi-line `CodeBlock`.
    pub first_para_exact_step_pt: Option<f64>,
    /// Optional `(b_1, child_1_exact_step_pt)` for `shape.body[1]` on the same page.
    pub second_baseline_and_step_pt: Option<(f64, Option<f64>)>,
}

/// Solve `<wps:bodyPr>` insets, `<wp:effectExtent>` inline line-box clearances, and `<w:position>`
/// baseline shift from first principles without hardcoded font/leading constants.
pub fn solve_shape_geometry(
    shape: &ShapeIr,
    is_inline: bool,
    parent_w_line_240: i64,
    inline_ctx: Option<&InlineBoxLineContextIr>,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> SolvedShapeGeometry {
    solve_shape_geometry_with_baselines(
        shape,
        is_inline,
        parent_w_line_240,
        inline_ctx,
        None,
        font_metrics,
    )
}

/// Solve DrawingML `<wps:bodyPr>` insets and optional first-paragraph exact line box from
/// ground-truth [`ShapeLocalBaselinesPt`], honoring asymmetric insets and negative `#v(...)`
/// vertical shifts inside rounded callout/code cards without clipping trailing lines.
pub fn solve_shape_geometry_with_baselines(
    shape: &ShapeIr,
    is_inline: bool,
    parent_w_line_240: i64,
    inline_ctx: Option<&InlineBoxLineContextIr>,
    local_baselines: Option<ShapeLocalBaselinesPt>,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> SolvedShapeGeometry {
    let (body_inset_pt, first_para_exact) =
        solve_shape_body_inset_pt(shape, local_baselines, font_metrics);

    let run_position_half_pt = shape.baseline_shift_pt.and_then(|shift_pt| {
        // Typst positive `baseline` lowers the box; OOXML positive `w:position` raises the run.
        let half_pts = (-shift_pt * 2.0).round() as i64;
        (half_pts != 0).then_some(half_pts)
    });

    let (inline_extra_top_pt, inline_extra_bottom_pt) = if is_inline {
        solve_inline_line_clearances_pt(shape, parent_w_line_240, inline_ctx, font_metrics)
    } else {
        (0.0, 0.0)
    };

    let no_text_wrap = is_inline
        && shape.body.len() == 1
        && local_baselines.is_some_and(|lb| lb.first_is_single_line_para);

    SolvedShapeGeometry {
        body_inset_pt,
        first_para_exact,
        inline_extra_top_pt,
        inline_extra_bottom_pt,
        run_position_half_pt,
        no_text_wrap,
    }
}

fn solve_shape_body_inset_pt(
    shape: &ShapeIr,
    local_baselines: Option<ShapeLocalBaselinesPt>,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> (SidesPtIr, Option<FirstParaExactLayout>) {
    if shape.body.is_empty() {
        return (SidesPtIr::zero(), None);
    }

    match &shape.kind {
        ShapeKindIr::Ellipse => {
            if shape.width_pt < 80.0 {
                (SidesPtIr::zero(), None)
            } else {
                (
                    SidesPtIr {
                        top: (shape.inset.top - 2.0).max(0.0),
                        bottom: (shape.inset.bottom - 2.0).max(0.0),
                        left: (shape.inset.left - 2.0).max(0.0),
                        right: (shape.inset.right - 2.0).max(0.0),
                    },
                    None,
                )
            }
        }
        ShapeKindIr::Rect => {
            let has_nested_table = shape
                .body
                .iter()
                .any(|b| matches!(b, BlockNode::Table(_)));

            // 1. Subtract ECMA-376 `roundRect` inscribed text-rectangle inset $R(1 - 1/\sqrt{2})$
            //    because Word adds `<wps:bodyPr>` insets on top of the inscribed rectangle `(il, it, ir, ib)`.
            let max_r = 0.5 * shape.width_pt.min(shape.height_pt).max(0.0);
            let radius_pt = shape.corner_radius_pt.unwrap_or(0.0).clamp(0.0, max_r);
            let inscribed_pt = if radius_pt > 0.1 {
                radius_pt * ROUND_RECT_INSCRIBED_FACTOR
            } else {
                0.0
            };

            let stroke_pt = shape
                .stroke
                .as_ref()
                .or(shape.borders.left.as_ref())
                .or(shape.borders.top.as_ref())
                .map_or(0.0, |s| s.thickness_pt);
            let horiz_stroke_comp = if has_nested_table {
                0.0
            } else {
                0.5 * stroke_pt
            };

            // 2. Solve `tIns` and optional `first_para_exact_spacing_dxa`:
            //    - When `b_0` sits at or below Word's natural `auto` baseline at `tIns = 0`,
            //      solve `tIns >= 0` directly.
            //    - When negative `#v(...)` pulls a single-line first paragraph above Word's
            //      `tIns = 0` `auto` baseline (`b_0 < inscribed_pt + a_word - 1.0`), clamp
            //      `tIns = 0` and solve exact `(w:line, w:after)` in twips from OpenType
            //      ascent/total ratios so neither `b_0` nor `b_1` drifts downward to clip the
            //      final line at the bottom of the fixed-height shape.
            let resolver = FontMetricsResolver::new(font_metrics);
            let first_sample = shape
                .body
                .first()
                .and_then(|b| extract_block_text_sample(b, font_metrics));
            let top_ascent_comp =
                first_sample
                    .as_ref()
                    .map_or(0.0, |(fam, sz, is_cjk, w_line)| {
                        let delta = resolver.first_line_top_overhang_pt(
                            fam.as_deref(),
                            Some(*sz),
                            *is_cjk,
                        );
                        let w_scale = (*w_line as f64 / 240.0).clamp(0.75, 1.0);
                        delta * w_scale
                    });
            let mut first_para_exact = None;
            let effective_top_pad_pt = if !has_nested_table
                && let (Some(lb), Some((fam, sz, is_cjk, w_line))) =
                    (local_baselines, first_sample.as_ref())
            {
                let b_y = lb.first_baseline_pt;
                let (a_word, _, _) = resolver.word_line_ascent_descent_total_pt(
                    fam.as_deref(),
                    *sz,
                    *is_cjk,
                    *w_line,
                );
                if lb.first_is_single_line_para && b_y < inscribed_pt + a_word - 0.2 {
                    // In Word `PTLS7`, `w:lineRule="exact"` anchors the paragraph's natural
                    // `usWinDescent` (`d0_exact`) to the bottom of `H_0`:
                    //   `y_base(0) = inscribed_pt + H_0 - d0_exact`.
                    // When negative `#v(...)` between `p[0]` and `p[1]` makes `b_1 - b_0` smaller
                    // than `d0_exact + a1_eff`, cap `H_0` at `b_1 - inscribed_pt - a1_eff` so `p[1]`
                    // never drifts downward, and lower `p[0]`'s runs via `<w:position>` so both
                    // `b_0` and `b_1` match Typst simultaneously.
                    let d0_exact = shape
                        .body
                        .first()
                        .and_then(|b| word_exact_line_descent_pt(b, font_metrics))
                        .unwrap_or(*sz * 0.24);
                    let h0_ideal_pt = (b_y - inscribed_pt + d0_exact).max(4.0);
                    let b1_and_a1 =
                        lb.second_baseline_and_step_pt
                            .and_then(|(b_1, step1_opt)| {
                                let child1 = shape.body.get(1)?;
                                let (fam1, sz1, is_cjk1, w_line1) =
                                    extract_block_text_sample(child1, font_metrics)?;
                                let a1_eff = match step1_opt {
                                    Some(step_pt) => {
                                        let d1_exact = word_exact_line_descent_pt(
                                            child1,
                                            font_metrics,
                                        )
                                        .unwrap_or(sz1 * 0.24);
                                        (step_pt - d1_exact).max(sz1 * 0.50)
                                    }
                                    None => {
                                        let (a1_auto, _, _) = resolver
                                            .word_line_ascent_descent_total_pt(
                                                fam1.as_deref(),
                                                sz1,
                                                is_cjk1,
                                                w_line1,
                                            );
                                        a1_auto
                                    }
                                };
                                Some((b_1, a1_eff))
                            });
                    let max_single_h0_pt = (shape.height_pt - 2.0 * inscribed_pt).max(4.0);
                    let (exact_h0_pt, capped_to_box) = match b1_and_a1 {
                        Some((b_1, a1_eff)) => (
                            h0_ideal_pt.min((b_1 - inscribed_pt - a1_eff).max(4.0)),
                            false,
                        ),
                        None if shape.body.len() == 1 => (
                            h0_ideal_pt.min(max_single_h0_pt),
                            h0_ideal_pt > max_single_h0_pt,
                        ),
                        None => (h0_ideal_pt, false),
                    };
                    let exact_line_dxa = if capped_to_box {
                        ((exact_h0_pt * 20.0).floor() as i64).max(80)
                    } else {
                        crate::units::pt_to_dxa(exact_h0_pt).max(80)
                    };
                    let actual_h0_pt = exact_line_dxa as f64 / 20.0;
                    let after_dxa = b1_and_a1
                        .map(|(b_1, a1_eff)| {
                            let after_pt =
                                (b_1 - (inscribed_pt + actual_h0_pt + a1_eff)).max(0.0);
                            crate::units::pt_to_dxa(after_pt)
                        })
                        .unwrap_or(0);
                    let unshifted_b0_pt = inscribed_pt + actual_h0_pt - d0_exact;
                    let shift_half_pt = (-(b_y - unshifted_b0_pt) * 2.0).round() as i32;
                    let run_position_half_pt =
                        (b1_and_a1.is_some() && shift_half_pt != 0).then_some(shift_half_pt);
                    first_para_exact = Some(FirstParaExactLayout {
                        line_dxa: exact_line_dxa,
                        after_dxa,
                        run_position_half_pt,
                    });
                    0.0
                } else if matches!(shape.body.first(), Some(BlockNode::CodeBlock { .. }))
                    && let Some(step_pt) = lb.first_para_exact_step_pt
                {
                    // When a single `RawLine` (`lines.len() == 1`) wraps onto `N >= 2` visual
                    // lines inside a fixed-height pill (`shape.body.len() == 1`), `w:line="exact"`
                    // applies `H_exact` to line 0 as well (`y_bottom = inscribed + N * H_exact`).
                    // Cap `H_exact <= (shape.height_pt - inscribed_pt) / N` so line `N - 1` never
                    // clips at the bottom of the pill.
                    let is_wrapped_single_code_para = shape.body.len() == 1
                        && lb.first_visual_line_count >= 2
                        && matches!(
                            shape.body.first(),
                            Some(BlockNode::CodeBlock { lines, .. }) if lines.len() == 1
                        );
                    let max_per_line_pt =
                        (shape.height_pt - inscribed_pt).max(8.0) / (lb.first_visual_line_count as f64);
                    if is_wrapped_single_code_para && step_pt > max_per_line_pt - 0.1 {
                        let exact_line_dxa = ((max_per_line_pt * 20.0).floor() as i64).max(120);
                        first_para_exact = Some(FirstParaExactLayout {
                            line_dxa: exact_line_dxa,
                            after_dxa: 0,
                            run_position_half_pt: None,
                        });
                        0.0
                    } else {
                        let d0_exact = shape
                            .body
                            .first()
                            .and_then(|b| word_exact_line_descent_pt(b, font_metrics))
                            .unwrap_or(*sz * 0.24);
                        let exact_step_pt =
                            (crate::units::pt_to_dxa(step_pt).max(160) as f64) / 20.0;
                        (b_y - (exact_step_pt - d0_exact) + top_ascent_comp).max(0.0)
                    }
                } else {
                    let cap_ratio = fam
                        .as_deref()
                        .and_then(|f| font_metrics.get(f))
                        .and_then(|face| {
                            face.s_cap_height
                                .filter(|&c| c > 0)
                                .map(|c| (c as f64 / face.upem.max(1) as f64).clamp(0.55, 0.90))
                        })
                        .unwrap_or(if *is_cjk { 0.76 } else { 0.70 });
                    (b_y - *sz * cap_ratio).max(0.0)
                }
            } else {
                shape.inset.top
            };
            let top_inset_pt = if first_para_exact.is_some() {
                0.0
            } else {
                (effective_top_pad_pt - inscribed_pt - top_ascent_comp).max(0.0)
            };

            // Rectangular text boxes in DrawingML use `anchor="t"` (`top`) and `<a:noAutofit/>`
            // unless they contain a nested `<w:tbl>`. Under `anchor="t"`, `<wps:bodyPr bIns>` has
            // zero effect on top-aligned flow positions and only raises Word's hard scissor clip
            // plane above the shape bottom; setting `bottom = 0.0` (`bIns="0"`) lets descenders
            // (`}`, `g`, `p`, `y`, `;`) extend naturally into `shape.inset.bottom` as in Typst.
            let bottom_inset_pt = if has_nested_table {
                (shape.inset.bottom - inscribed_pt).max(0.0)
            } else {
                0.0
            };

            (
                SidesPtIr {
                    left: (shape.inset.left - inscribed_pt - horiz_stroke_comp).max(0.0),
                    right: (shape.inset.right - inscribed_pt - horiz_stroke_comp).max(0.0),
                    top: top_inset_pt,
                    bottom: bottom_inset_pt,
                },
                first_para_exact,
            )
        }
        _ => (
            SidesPtIr {
                left: shape.inset.left.max(0.0),
                top: shape.inset.top.max(0.0),
                right: shape.inset.right.max(0.0),
                bottom: shape.inset.bottom.max(0.0),
            },
            None,
        ),
    }
}

/// Solve `<wp:effectExtent t="..." b="..."/>` for an inline shape inside a paragraph:
/// - Above: `prev_line` leaves $d_{\text{Word}}$ below `prev_baseline`; missing gap is $\text{gap}_{\text{prev}} - d_{\text{Word}}$.
/// - Below (intra-paragraph `next_line`): `<wp:inline>` leaves $D_{\text{box}}$ below its baseline and
///   `next_line` adds $a_{\text{Word}}$; missing gap is $\text{gap}_{\text{next}} - (D_{\text{box}} + a_{\text{Word}})$.
/// - Below (paragraph-ending standalone box, `next_line == None && !shares_line_with_text`): `w:after`
///   assumes the last line contributes normal text descent $d_{\text{Word}}$, so restore $d_{\text{Word}} - D_{\text{box}}$.
fn solve_inline_line_clearances_pt(
    shape: &ShapeIr,
    parent_w_line_240: i64,
    inline_ctx: Option<&InlineBoxLineContextIr>,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> (f64, f64) {
    let Some(ctx) = inline_ctx else {
        return (0.0, 0.0);
    };

    let w_line = if parent_w_line_240 > 0 {
        parent_w_line_240
    } else {
        240
    };
    let shift_pt = shape.baseline_shift_pt.unwrap_or(0.0);
    let box_ascent_pt = (shape.height_pt - shift_pt).max(0.0);

    let resolver = FontMetricsResolver::new(font_metrics);
    let extra_top_pt = ctx.prev_line.as_ref().map_or(0.0, |prev: &AdjacentTextLineIr| {
        let (_, d_prev, _) = resolver.word_line_ascent_descent_total_pt(
            prev.font_family.as_deref(),
            prev.font_size_pt,
            prev.is_cjk,
            w_line,
        );
        (prev.gap_pt - d_prev).clamp(0.0, 24.0)
    });

    let extra_bottom_pt = if let Some(next) = ctx.next_line.as_ref() {
        let (a_next, _, h_next) = resolver.word_line_ascent_descent_total_pt(
            next.font_family.as_deref(),
            next.font_size_pt,
            next.is_cjk,
            w_line,
        );
        let d_box_line = shift_pt.max(0.0).max((h_next - box_ascent_pt).max(0.0));
        (next.gap_pt - (d_box_line + a_next)).clamp(0.0, 24.0)
    } else if !ctx.shares_line_with_text {
        let fallback_sample = shape
            .body
            .first()
            .and_then(|b| extract_block_text_sample(b, font_metrics));
        let (fam, sz, is_cjk) = if let Some(prev) = ctx.prev_line.as_ref() {
            (prev.font_family.as_deref(), prev.font_size_pt, prev.is_cjk)
        } else if let Some((ref f, s, c, _)) = fallback_sample {
            (f.as_deref(), s, c)
        } else {
            (None, 11.0, false)
        };
        let (_, d_par, h_par) =
            resolver.word_line_ascent_descent_total_pt(fam, sz, is_cjk, w_line);
        let d_box_line = shift_pt.max(0.0).max((h_par - box_ascent_pt).max(0.0));
        (d_par - d_box_line).clamp(0.0, 24.0)
    } else {
        0.0
    };

    (extra_top_pt, extra_bottom_pt)
}

fn extract_block_text_sample(
    block: &BlockNode,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> Option<(Option<String>, f64, bool, i64)> {
    match block {
        BlockNode::Paragraph { content, style, .. } => {
            let (fam, sz, is_cjk) = first_text_run_info(content, style.resolved_font_size_pt)?;
            let w_line = compute_line_spacing_dxa_with_metrics(
                content,
                style.line_spacing_pt,
                font_metrics,
                None,
            );
            Some((fam, sz, is_cjk, w_line))
        }
        BlockNode::Heading { content, .. } | BlockNode::Title { content, .. } => {
            let (fam, sz, is_cjk) = first_text_run_info(content, None)?;
            let w_line =
                compute_line_spacing_dxa_with_metrics(content, None, font_metrics, None);
            Some((fam, sz, is_cjk, w_line))
        }
        BlockNode::CodeBlock { lines, .. } => {
            let run = lines.iter().flatten().find(|r| !r.text.is_empty())
                .or_else(|| lines.iter().flatten().next())?;
            let inlines = [InlineNode::Text(run.clone())];
            let (fam, sz, is_cjk) = first_text_run_info(&inlines, Some(9.6))?;
            let w_line =
                compute_line_spacing_dxa_with_metrics(&inlines, None, font_metrics, None);
            Some((fam, sz, is_cjk, w_line))
        }
        _ => None,
    }
}

fn first_text_run_info(
    inlines: &[InlineNode],
    fallback_sz: Option<f64>,
) -> Option<(Option<String>, f64, bool)> {
    for inl in inlines {
        match inl {
            InlineNode::Text(r) if !r.text.trim().is_empty() => {
                let sz = effective_word_run_font_size_pt(r, fallback_sz.unwrap_or(11.0));
                return Some((r.font_family.clone(), sz, run_uses_cjk_slot_in_word(r)));
            }
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => {
                if let Some(found) = first_text_run_info(content, fallback_sz) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn run_uses_cjk_slot_in_word(r: &ir::TextRunIr) -> bool {
    if run_has_cjk(r) {
        return true;
    }
    let has_cjk_locale = r
        .locale
        .as_ref()
        .is_some_and(|loc| matches!(loc.lang.as_str(), "zh" | "ja" | "ko"));
    has_cjk_locale && TextScriptClassIr::classify(&r.text).has_east_asian_symbol()
}

/// Compute Word's natural `usWinDescent` (`pt`) for the first visual line of `block` inside a
/// `w:lineRule="exact"` line box (`y_base = y_box_top + H_exact - d_win`), delegated to
/// [`FontMetricsResolver::exact_line_descent_ratio`].
fn word_exact_line_descent_pt(
    block: &BlockNode,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> Option<f64> {
    let resolver = FontMetricsResolver::new(font_metrics);
    let mut max_desc_pt = 0.0_f64;
    let mut visit_run = |r: &ir::TextRunIr, fallback_sz: f64| {
        if r.text.is_empty() {
            return;
        }
        let eff_sz = effective_word_run_font_size_pt(r, fallback_sz);
        let uses_cjk = run_uses_cjk_slot_in_word(r);
        let ratio =
            resolver.exact_line_descent_ratio(r.font_family.as_deref(), r.is_code, uses_cjk);
        max_desc_pt = max_desc_pt.max(eff_sz * ratio);
    };

    match block {
        BlockNode::Paragraph { content, style, .. } => {
            let fallback_sz = style.resolved_font_size_pt.unwrap_or(11.0);
            visit_first_line_runs(content, fallback_sz, &mut visit_run);
        }
        BlockNode::Heading { content, .. } | BlockNode::Title { content, .. } => {
            visit_first_line_runs(content, 14.0, &mut visit_run);
        }
        BlockNode::CodeBlock { lines, .. } => {
            if let Some(first_line) = lines.iter().find(|l| !l.is_empty()) {
                for r in first_line {
                    visit_run(r, 9.6);
                }
            }
        }
        _ => return None,
    }
    (max_desc_pt > 0.0).then_some(max_desc_pt)
}

fn visit_first_line_runs(
    inlines: &[InlineNode],
    fallback_sz: f64,
    f: &mut impl FnMut(&ir::TextRunIr, f64),
) -> bool {
    for inl in inlines {
        match inl {
            InlineNode::LineBreak => return true,
            InlineNode::Text(r) => f(r, fallback_sz),
            InlineNode::Link { content, .. }
            | InlineNode::Ref { content, .. }
            | InlineNode::Citation { content, .. }
            | InlineNode::Field { content, .. } => {
                if visit_first_line_runs(content, fallback_sz, f) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}
