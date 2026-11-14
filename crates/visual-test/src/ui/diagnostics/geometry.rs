//! 5-column geometric telemetry matrix comparing IR, Typst PDF, and Word PDF (`diagnostics/geometry.rs`).

use gpui_kit::{
    FontWeight, Hsla, IntoElement, ParentElement as _, Styled as _,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{model::CaseReport, ui::state::ParityStudioView};

struct GeoRow {
    metric: &'static str,
    ir_val: String,
    typst_val: String,
    word_val: String,
    delta_val: String,
    abs_delta_pt: Option<f64>,
}

impl ParityStudioView {
    pub(super) fn render_geometry_table(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_col = theme.border;
        let muted_fg = theme.muted_foreground;
        let even_bg = theme.list_even;
        let mono = theme.mono_font_family.clone();

        let ir = &case.ir_summary;
        let t_vis = case.typst_visual.as_ref();
        let w_vis = case.word_visual.as_ref();
        let t_box = t_vis.and_then(|v| v.ink_bbox);
        let w_box = w_vis.and_then(|v| v.ink_bbox);
        let cmp = case.comparison.as_ref();

        let m = ir.margins_pt;
        let ir_margins_str = format!(
            "[{:.1}, {:.1}, {:.1}, {:.1}]{}",
            m[0],
            m[1],
            m[2],
            m[3],
            if ir.mirror_margins { " mirror" } else { "" }
        );

        let fmt_opt = |v: Option<f64>| {
            v.map(|x| format!("{x:.2} pt"))
                .unwrap_or_else(|| "—".to_string())
        };
        let fmt_delta = |v: Option<f64>| {
            v.map(|d| format!("{d:+.2} pt"))
                .unwrap_or_else(|| "—".to_string())
        };

        let page_delta = match (t_vis, w_vis) {
            (Some(tv), Some(wv)) => {
                let dw = wv.page_width_pt - tv.page_width_pt;
                let dh = wv.page_height_pt - tv.page_height_pt;
                if tv.page_count != wv.page_count {
                    (
                        format!(
                            "Δp {:+} ({:.1}×{:.1})",
                            wv.page_count as isize - tv.page_count as isize,
                            dw,
                            dh
                        ),
                        Some(20.0),
                    )
                } else {
                    (
                        format!("ΔW {dw:+.1} · ΔH {dh:+.1} pt"),
                        Some(dw.abs().max(dh.abs())),
                    )
                }
            }
            _ => ("—".to_string(), None),
        };

        let rows = vec![
            GeoRow {
                metric: "Page Size (W × H)",
                ir_val: format!(
                    "{:.1} × {:.1} pt ({} sec)",
                    ir.page_width_pt,
                    ir.page_height_pt,
                    ir.sections_count.max(1)
                ),
                typst_val: t_vis
                    .map(|v| {
                        format!(
                            "{:.1} × {:.1} pt ({}p)",
                            v.page_width_pt, v.page_height_pt, v.page_count
                        )
                    })
                    .unwrap_or_else(|| "—".to_string()),
                word_val: w_vis
                    .map(|v| {
                        format!(
                            "{:.1} × {:.1} pt ({}p)",
                            v.page_width_pt, v.page_height_pt, v.page_count
                        )
                    })
                    .unwrap_or_else(|| "—".to_string()),
                delta_val: page_delta.0,
                abs_delta_pt: page_delta.1,
            },
            GeoRow {
                metric: "Margins [T, R, B, L]",
                ir_val: ir_margins_str,
                typst_val: format!(
                    "H/F [{:.1}, {:.1}] pt",
                    ir.header_footer_margin_pt[0], ir.header_footer_margin_pt[1]
                ),
                word_val: if ir.section_margins_pt.len() > 1 {
                    format!("{} section pgMar specs", ir.section_margins_pt.len())
                } else {
                    "w:pgMar (20 dxa/pt)".to_string()
                },
                delta_val: if ir.mirror_margins {
                    "w:mirrorMargins".to_string()
                } else {
                    "Direct mapping".to_string()
                },
                abs_delta_pt: None,
            },
            GeoRow {
                metric: "Ink Left",
                ir_val: format!("margin L {:.2} pt", m[3]),
                typst_val: fmt_opt(t_box.map(|b| b.left_pt)),
                word_val: fmt_opt(w_box.map(|b| b.left_pt)),
                delta_val: fmt_delta(cmp.map(|c| c.delta_left_pt)),
                abs_delta_pt: cmp.map(|c| c.delta_left_pt.abs()),
            },
            GeoRow {
                metric: "Ink Top",
                ir_val: format!("margin T {:.2} pt", m[0]),
                typst_val: fmt_opt(t_box.map(|b| b.top_pt)),
                word_val: fmt_opt(w_box.map(|b| b.top_pt)),
                delta_val: fmt_delta(cmp.map(|c| c.delta_top_pt)),
                abs_delta_pt: cmp.map(|c| c.delta_top_pt.abs()),
            },
            GeoRow {
                metric: "Ink Right",
                ir_val: format!("edge {:.2} pt", (ir.page_width_pt - m[1]).max(0.0)),
                typst_val: fmt_opt(t_box.map(|b| b.right_pt)),
                word_val: fmt_opt(w_box.map(|b| b.right_pt)),
                delta_val: fmt_delta(cmp.map(|c| c.delta_right_pt)),
                abs_delta_pt: cmp.map(|c| c.delta_right_pt.abs()),
            },
            GeoRow {
                metric: "Ink Bottom",
                ir_val: format!("edge {:.2} pt", (ir.page_height_pt - m[2]).max(0.0)),
                typst_val: fmt_opt(t_box.map(|b| b.bottom_pt)),
                word_val: fmt_opt(w_box.map(|b| b.bottom_pt)),
                delta_val: fmt_delta(cmp.map(|c| c.delta_bottom_pt)),
                abs_delta_pt: cmp.map(|c| c.delta_bottom_pt.abs()),
            },
            GeoRow {
                metric: "Ink Width",
                ir_val: format!(
                    "body {:.2} pt",
                    (ir.page_width_pt - m[1] - m[3]).max(0.0)
                ),
                typst_val: fmt_opt(t_box.map(|b| b.width_pt)),
                word_val: fmt_opt(w_box.map(|b| b.width_pt)),
                delta_val: fmt_delta(cmp.map(|c| c.delta_width_pt)),
                abs_delta_pt: cmp.map(|c| c.delta_width_pt.abs()),
            },
            GeoRow {
                metric: "Ink Height",
                ir_val: format!(
                    "body {:.2} pt",
                    (ir.page_height_pt - m[0] - m[2]).max(0.0)
                ),
                typst_val: fmt_opt(t_box.map(|b| b.height_pt)),
                word_val: fmt_opt(w_box.map(|b| b.height_pt)),
                delta_val: fmt_delta(cmp.map(|c| c.delta_height_pt)),
                abs_delta_pt: cmp.map(|c| c.delta_height_pt.abs()),
            },
            GeoRow {
                metric: "Raster Ink Pixels",
                ir_val: if ir.top_block_kinds.is_empty() {
                    "—".to_string()
                } else {
                    ir.top_block_kinds.join(", ")
                },
                typst_val: t_vis
                    .map(|v| format!("{} px", v.ink_pixels))
                    .unwrap_or_else(|| "—".to_string()),
                word_val: w_vis
                    .map(|v| format!("{} px", v.ink_pixels))
                    .unwrap_or_else(|| "—".to_string()),
                delta_val: cmp
                    .map(|c| format!("{} / {} overlap", c.overlap_pixels, c.union_pixels))
                    .unwrap_or_else(|| "—".to_string()),
                abs_delta_pt: None,
            },
        ];

        let delta_color = |abs_d: Option<f64>| -> Hsla {
            match abs_d {
                Some(d) if d <= 1.5 => theme.success,
                Some(d) if d <= 6.0 => theme.foreground,
                Some(_) => theme.warning,
                None => muted_fg,
            }
        };

        v_flex()
            .w_full()
            .text_xs()
            .child(
                h_flex()
                    .w_full()
                    .px_3p5()
                    .py_1p5()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(border_col)
                    .text_color(muted_fg)
                    .font_weight(FontWeight::MEDIUM)
                    .child(div().w(px(148.0)).flex_shrink_0().child("Coordinate / Metric"))
                    .child(div().flex_1().min_w_0().child("IR / Construct"))
                    .child(div().w(px(140.0)).flex_shrink_0().child("Typst PDF"))
                    .child(div().w(px(140.0)).flex_shrink_0().child("Word PDF"))
                    .child(div().w(px(148.0)).flex_shrink_0().child("Δ (Word − Typst)")),
            )
            .children(rows.into_iter().enumerate().map(|(idx, row)| {
                let d_col = delta_color(row.abs_delta_pt);
                h_flex()
                    .w_full()
                    .px_3p5()
                    .py_1()
                    .items_center()
                    .gap_2()
                    .when(idx % 2 == 1, |el| el.bg(even_bg))
                    .border_b_1()
                    .border_color(border_col.opacity(0.45))
                    .child(
                        div()
                            .w(px(148.0))
                            .flex_shrink_0()
                            .font_weight(FontWeight::MEDIUM)
                            .child(row.metric),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(mono.clone())
                            .text_color(muted_fg)
                            .child(row.ir_val),
                    )
                    .child(
                        div()
                            .w(px(140.0))
                            .flex_shrink_0()
                            .truncate()
                            .font_family(mono.clone())
                            .child(row.typst_val),
                    )
                    .child(
                        div()
                            .w(px(140.0))
                            .flex_shrink_0()
                            .truncate()
                            .font_family(mono.clone())
                            .child(row.word_val),
                    )
                    .child(
                        div()
                            .w(px(148.0))
                            .flex_shrink_0()
                            .truncate()
                            .font_family(mono.clone())
                            .text_color(d_col)
                            .child(row.delta_val),
                    )
            }))
            .when(!ir.pg_mar_xml.is_empty(), |el| {
                el.child(
                    h_flex()
                        .w_full()
                        .px_3p5()
                        .py_1p5()
                        .items_center()
                        .gap_2()
                        .font_family(mono)
                        .text_color(muted_fg)
                        .child(div().flex_shrink_0().child("OpenXML pgMar:"))
                        .child(div().flex_1().min_w_0().truncate().child(ir.pg_mar_xml.clone())),
                )
            })
    }
}
