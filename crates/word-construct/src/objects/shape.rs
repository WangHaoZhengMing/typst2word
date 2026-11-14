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

use crate::font_metrics::{FontParadigm, RawFontMetrics, lookup_known_font};
use crate::objects::paragraph::compute_line_spacing_dxa_with_metrics;
use crate::objects::text_run::run_has_cjk;
use crate::page_calibration::measured_first_line_top_delta_pt;

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

/// Pre-solved DrawingML geometry for a `ShapeIr` (`BlockNode::ShapeBlock` or `InlineNode::ShapeInline`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SolvedShapeGeometry {
    /// Calibrated `<wps:bodyPr lIns/tIns/rIns/bIns>` padding in `pt`.
    pub body_inset_pt: SidesPtIr,
    /// Extra top `<wp:effectExtent t="..."/>` clearance in `pt` for inline shapes inside a paragraph.
    pub inline_extra_top_pt: f64,
    /// Extra bottom `<wp:effectExtent b="..."/>` clearance in `pt` for inline shapes inside a paragraph.
    pub inline_extra_bottom_pt: f64,
    /// Optional `<w:rPr><w:position w:val="..."/></w:rPr>` baseline shift in half-points (`1/2 pt`).
    pub run_position_half_pt: Option<i64>,
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
    let body_inset_pt = solve_shape_body_inset_pt(shape, font_metrics);

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

    SolvedShapeGeometry {
        body_inset_pt,
        inline_extra_top_pt,
        inline_extra_bottom_pt,
        run_position_half_pt,
    }
}

fn solve_shape_body_inset_pt(
    shape: &ShapeIr,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> SidesPtIr {
    if shape.body.is_empty() {
        return SidesPtIr::zero();
    }

    match &shape.kind {
        ShapeKindIr::Ellipse => {
            if shape.width_pt < 80.0 {
                SidesPtIr::zero()
            } else {
                SidesPtIr {
                    top: (shape.inset.top - 2.0).max(0.0),
                    bottom: (shape.inset.bottom - 2.0).max(0.0),
                    left: (shape.inset.left - 2.0).max(0.0),
                    right: (shape.inset.right - 2.0).max(0.0),
                }
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

            // 2. Compensate top inset for Word's first-line ascent overhang (`a_Word - sCapHeight`),
            //    and bottom inset for Word's last-line descent below baseline so `<a:noAutofit/>`
            //    never clips the final text line of a tight box.
            let top_ascent_comp = shape
                .body
                .first()
                .and_then(|b| extract_block_text_sample(b, font_metrics))
                .map_or(0.0, |(fam, sz, is_cjk, w_line)| {
                    let delta =
                        measured_first_line_top_delta_pt(fam.as_deref(), Some(sz), is_cjk);
                    let w_scale = (w_line as f64 / 240.0).clamp(0.75, 1.0);
                    delta * w_scale
                });

            let bot_descent_comp = if has_nested_table {
                0.0
            } else {
                shape
                    .body
                    .last()
                    .and_then(|b| extract_block_text_sample(b, font_metrics))
                    .map_or(0.0, |(fam, sz, is_cjk, w_line)| {
                        let (_, d_word, _) = word_line_ascent_descent_total_pt(
                            fam.as_deref(),
                            sz,
                            is_cjk,
                            w_line,
                            font_metrics,
                        );
                        d_word
                    })
            };

            SidesPtIr {
                left: (shape.inset.left - inscribed_pt - horiz_stroke_comp).max(0.0),
                right: (shape.inset.right - inscribed_pt - horiz_stroke_comp).max(0.0),
                top: (shape.inset.top - inscribed_pt - top_ascent_comp).max(0.0),
                bottom: (shape.inset.bottom - inscribed_pt - bot_descent_comp).max(0.0),
            }
        }
        _ => SidesPtIr {
            left: shape.inset.left.max(0.0),
            top: shape.inset.top.max(0.0),
            right: shape.inset.right.max(0.0),
            bottom: shape.inset.bottom.max(0.0),
        },
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

    let extra_top_pt = ctx.prev_line.as_ref().map_or(0.0, |prev: &AdjacentTextLineIr| {
        let (_, d_prev, _) = word_line_ascent_descent_total_pt(
            prev.font_family.as_deref(),
            prev.font_size_pt,
            prev.is_cjk,
            w_line,
            font_metrics,
        );
        (prev.gap_pt - d_prev).clamp(0.0, 24.0)
    });

    let extra_bottom_pt = if let Some(next) = ctx.next_line.as_ref() {
        let (a_next, _, h_next) = word_line_ascent_descent_total_pt(
            next.font_family.as_deref(),
            next.font_size_pt,
            next.is_cjk,
            w_line,
            font_metrics,
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
        let (_, d_par, h_par) = word_line_ascent_descent_total_pt(fam, sz, is_cjk, w_line, font_metrics);
        let d_box_line = shift_pt.max(0.0).max((h_par - box_ascent_pt).max(0.0));
        (d_par - d_box_line).clamp(0.0, 24.0)
    } else {
        0.0
    };

    (extra_top_pt, extra_bottom_pt)
}

fn word_line_ascent_descent_total_pt(
    font_family: Option<&str>,
    font_size_pt: f64,
    is_cjk: bool,
    w_line_240: i64,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> (f64, f64, f64) {
    let sz = font_size_pt.clamp(6.0, 72.0);
    let paradigm = resolve_paradigm_for_family(font_family, is_cjk, font_metrics);
    let a_word = sz * paradigm.m_ink()
        + measured_first_line_top_delta_pt(font_family, Some(sz), is_cjk);
    let h_word = sz * paradigm.m_win() * (w_line_240 as f64 / 240.0);
    let d_word = (h_word - a_word).max(0.0);
    (a_word, d_word, h_word)
}

fn resolve_paradigm_for_family(
    font_family: Option<&str>,
    is_cjk: bool,
    font_metrics: &BTreeMap<String, FontFaceMetricsIr>,
) -> FontParadigm {
    if let Some(fam) = font_family {
        let effective_fam = match font_metrics.get(fam) {
            Some(face) if !face.is_system_installed => {
                if is_cjk || face.has_cjk_codepage {
                    "SimSun"
                } else {
                    "Times New Roman"
                }
            }
            Some(face) => {
                if let Some(known) = lookup_known_font(fam) {
                    return known;
                }
                let raw = RawFontMetrics {
                    family_name: fam,
                    upm: face.upem,
                    win_ascent: face
                        .us_win_ascent
                        .unwrap_or(face.hhea_ascender.max(0) as u16),
                    win_descent: face
                        .us_win_descent
                        .unwrap_or((-face.hhea_descender).max(0) as u16),
                    hhea_ascent: face.hhea_ascender,
                    hhea_descent: face.hhea_descender,
                    has_cjk_codepage: face.has_cjk_codepage,
                    is_serif: fam.to_ascii_lowercase().contains("song")
                        || fam.to_ascii_lowercase().contains("serif")
                        || fam.to_ascii_lowercase().contains("times"),
                    is_monospace: face.is_monospaced,
                    has_valid_gsub_hani: face.has_gsub_hani,
                };
                return FontParadigm::from(&raw);
            }
            None => fam,
        };
        if let Some(known) = lookup_known_font(effective_fam) {
            return known;
        }
    }
    if is_cjk {
        lookup_known_font("PingFang SC").unwrap()
    } else {
        lookup_known_font("Times New Roman").unwrap()
    }
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
                let sz = r.font_size_pt.or(fallback_sz).unwrap_or(11.0);
                return Some((r.font_family.clone(), sz, run_has_cjk(r)));
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
