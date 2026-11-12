//! Center column lower split: Geometry comparison table, Structural IR/OOXML
//! assertions, and PDFKit per-line text diff inspector (`diagnostics.rs`).

use gpui_kit::{
    FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, div, prelude::FluentBuilder as _, px,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        scroll::ScrollableElement as _,
        tab::{Tab, TabBar},
        v_flex,
    },
};

use crate::{
    model::CaseReport,
    ui::state::{DiagnosticsTab, ParityStudioView},
};

impl ParityStudioView {
    pub(crate) fn render_diagnostics_pane(
        &self,
        case: &CaseReport,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let border_col = cx.theme().border;
        let bg = cx.theme().background;
        let passed_checks = case.checks.iter().filter(|c| c.passed).count();
        let total_checks = case.checks.len();
        let t_lines_len = case.typst_visual.as_ref().map_or(0, |v| v.lines.len());
        let w_lines_len = case.word_visual.as_ref().map_or(0, |v| v.lines.len());

        let tab_idx = match self.diagnostics_tab {
            DiagnosticsTab::Geometry => 0,
            DiagnosticsTab::Checks => 1,
            DiagnosticsTab::Lines => 2,
        };

        let body = match self.diagnostics_tab {
            DiagnosticsTab::Geometry => self.render_geometry_table(case, cx).into_any_element(),
            DiagnosticsTab::Checks => self.render_checks_list(case, cx).into_any_element(),
            DiagnosticsTab::Lines => self.render_line_diffs(case, cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(bg)
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_1()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border_col)
                    .child(
                        TabBar::new("diagnostics-subtabs")
                            .underline()
                            .small()
                            .selected_index(tab_idx)
                            .children([
                                Tab::new().label("Geometry"),
                                Tab::new().label(format!(
                                    "Checks ({passed_checks}/{total_checks})"
                                )),
                                Tab::new().label(format!(
                                    "PDF Lines ({t_lines_len}/{w_lines_len})"
                                )),
                            ])
                            .on_click(cx.listener(|this, idx, _, cx| {
                                this.diagnostics_tab = match *idx {
                                    1 => DiagnosticsTab::Checks,
                                    2 => DiagnosticsTab::Lines,
                                    _ => DiagnosticsTab::Geometry,
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-close-diagnostics")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Hide Details Drawer")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_diagnostics = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(body)
    }

    fn render_geometry_table(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let tb = case.typst_visual.as_ref().and_then(|v| v.ink_bbox);
        let wb = case.word_visual.as_ref().and_then(|v| v.ink_bbox);
        let cmp = case.comparison.as_ref();

        let fmt_opt =
            |v: Option<f64>| v.map(|x| format!("{x:.2} pt")).unwrap_or_else(|| "—".into());
        let fmt_delta =
            |v: Option<f64>| v.map(|x| format!("{x:+.2} pt")).unwrap_or_else(|| "—".into());

        type MetricRow<'a> = (&'a str, String, String, String, String, Option<f64>);
        let rows: Vec<MetricRow<'_>> = vec![
            (
                "Page Size (W × H)",
                format!(
                    "{:.1} × {:.1} pt",
                    case.ir_summary.page_width_pt, case.ir_summary.page_height_pt
                ),
                case.typst_visual
                    .as_ref()
                    .map(|v| {
                        format!(
                            "{:.1} × {:.1} pt (p={})",
                            v.page_width_pt, v.page_height_pt, v.page_count
                        )
                    })
                    .unwrap_or_else(|| "—".into()),
                case.word_visual
                    .as_ref()
                    .map(|v| {
                        format!(
                            "{:.1} × {:.1} pt (p={})",
                            v.page_width_pt, v.page_height_pt, v.page_count
                        )
                    })
                    .unwrap_or_else(|| "—".into()),
                "—".into(),
                None,
            ),
            (
                "Page Margins [T,R,B,L]",
                format!(
                    "[{:.2}, {:.2}, {:.2}, {:.2}] pt{}",
                    case.ir_summary.margins_pt[0],
                    case.ir_summary.margins_pt[1],
                    case.ir_summary.margins_pt[2],
                    case.ir_summary.margins_pt[3],
                    if case.ir_summary.mirror_margins {
                        " (mirror)"
                    } else {
                        ""
                    }
                ),
                format!(
                    "T:{:.1} R:{:.1} B:{:.1} L:{:.1} pt",
                    case.ir_summary.margins_pt[0],
                    case.ir_summary.margins_pt[1],
                    case.ir_summary.margins_pt[2],
                    case.ir_summary.margins_pt[3]
                ),
                if case.ir_summary.pg_mar_xml.is_empty() {
                    "—".into()
                } else {
                    case.ir_summary.pg_mar_xml.clone()
                },
                if case.ir_summary.mirror_margins {
                    "mirrorMargins".into()
                } else {
                    "1:1 Preserve".into()
                },
                None,
            ),
            (
                "Ink Left (x_min)",
                case.ir_summary
                    .anchor_bbox
                    .map(|b| format!("{:.2} pt", b.left_pt))
                    .unwrap_or_else(|| "—".into()),
                fmt_opt(tb.map(|b| b.left_pt)),
                fmt_opt(wb.map(|b| b.left_pt)),
                fmt_delta(cmp.map(|c| c.delta_left_pt)),
                cmp.map(|c| c.delta_left_pt),
            ),
            (
                "Ink Top (y_min)",
                case.ir_summary
                    .anchor_bbox
                    .map(|b| format!("{:.2} pt", b.top_pt))
                    .unwrap_or_else(|| "—".into()),
                fmt_opt(tb.map(|b| b.top_pt)),
                fmt_opt(wb.map(|b| b.top_pt)),
                fmt_delta(cmp.map(|c| c.delta_top_pt)),
                cmp.map(|c| c.delta_top_pt),
            ),
            (
                "Ink Right (x_max)",
                case.ir_summary
                    .anchor_bbox
                    .map(|b| format!("{:.2} pt", b.right_pt))
                    .unwrap_or_else(|| "—".into()),
                fmt_opt(tb.map(|b| b.right_pt)),
                fmt_opt(wb.map(|b| b.right_pt)),
                fmt_delta(cmp.map(|c| c.delta_right_pt)),
                cmp.map(|c| c.delta_right_pt),
            ),
            (
                "Ink Bottom (y_max)",
                case.ir_summary
                    .anchor_bbox
                    .map(|b| format!("{:.2} pt", b.bottom_pt))
                    .unwrap_or_else(|| "—".into()),
                fmt_opt(tb.map(|b| b.bottom_pt)),
                fmt_opt(wb.map(|b| b.bottom_pt)),
                fmt_delta(cmp.map(|c| c.delta_bottom_pt)),
                cmp.map(|c| c.delta_bottom_pt),
            ),
            (
                "Ink Width",
                case.ir_summary
                    .anchor_bbox
                    .map(|b| format!("{:.2} pt", b.width_pt))
                    .unwrap_or_else(|| "—".into()),
                fmt_opt(tb.map(|b| b.width_pt)),
                fmt_opt(wb.map(|b| b.width_pt)),
                fmt_delta(cmp.map(|c| c.delta_width_pt)),
                cmp.map(|c| c.delta_width_pt),
            ),
            (
                "Ink Height",
                "—".into(),
                fmt_opt(tb.map(|b| b.height_pt)),
                fmt_opt(wb.map(|b| b.height_pt)),
                fmt_delta(cmp.map(|c| c.delta_height_pt)),
                cmp.map(|c| c.delta_height_pt),
            ),
        ];

        let border_col = cx.theme().border;
        let radius_md = cx.theme().radius;

        div()
            .id("diagnostics-geometry-scroll")
            .flex_1()
            .min_h_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scrollbar()
            .when_some(cmp, |el, cmp_metrics| {
                el.child(self.render_case_kpi_strip(cmp_metrics.raw_iou, cmp_metrics.aligned_iou, cx))
            })
            .child(
                v_flex()
                    .w_full()
                    .rounded(radius_md)
                    .border_1()
                    .border_color(border_col)
                    .overflow_hidden()
                    .child(geometry_row(
                        "Coordinate / Metric",
                        "IR / Construct",
                        "Typst PDF",
                        "Word PDF",
                        "Δ (Word − Typst)",
                        true,
                        false,
                        None,
                        cx,
                    ))
                    .children(rows.into_iter().enumerate().map(
                        |(idx, (label, ir_v, typ_v, wrd_v, d_v, raw_d))| {
                            geometry_row(
                                label,
                                &ir_v,
                                &typ_v,
                                &wrd_v,
                                &d_v,
                                false,
                                idx % 2 == 1,
                                raw_d,
                                cx,
                            )
                        },
                    )),
            )
    }

    fn render_case_kpi_strip(
        &self,
        raw_iou: f64,
        aligned_iou: f64,
        cx: &gpui_kit::App,
    ) -> impl IntoElement {
        let mono = cx.theme().mono_font_family.clone();
        let muted_bg = cx.theme().muted;
        let muted_fg = cx.theme().muted_foreground;
        let radius = cx.theme().radius;

        let metric_cell = |title: &str, value: String| {
            h_flex()
                .px_2p5()
                .py_1()
                .gap_2()
                .items_center()
                .rounded(radius)
                .bg(muted_bg)
                .child(div().text_xs().text_color(muted_fg).child(title.to_string()))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                )
        };

        h_flex()
            .flex_wrap()
            .gap_2()
            .child(metric_cell("Raw Pixel IoU", format!("{:.1}%", raw_iou * 100.0)))
            .child(metric_cell(
                "Shape-Aligned IoU",
                format!("{:.1}%", aligned_iou * 100.0),
            ))
    }

    fn render_checks_list(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let success_col = cx.theme().success;
        let danger_col = cx.theme().danger;
        let muted_fg = cx.theme().muted_foreground;
        let border_col = cx.theme().border;
        let radius = cx.theme().radius;
        let mono = cx.theme().mono_font_family.clone();

        div()
            .id("diagnostics-checks-scroll")
            .flex_1()
            .min_h_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_1p5()
            .overflow_y_scrollbar()
            .when(case.checks.is_empty(), |el| {
                el.child(
                    div()
                        .p_4()
                        .text_xs()
                        .text_color(muted_fg)
                        .child("No assertions recorded yet. Run the case or suite to verify IR & OpenXML."),
                )
            })
            .children(case.checks.iter().enumerate().map(|(i, chk)| {
                let col = if chk.passed { success_col } else { danger_col };
                h_flex()
                    .id(SharedString::from(format!("chk-row-{i}")))
                    .w_full()
                    .px_3()
                    .py_1p5()
                    .gap_2p5()
                    .items_center()
                    .rounded(radius)
                    .border_1()
                    .border_color(border_col)
                    .bg(cx.theme().popover)
                    .child(
                        Icon::new(if chk.passed {
                            IconName::CircleCheck
                        } else {
                            IconName::CircleX
                        })
                        .small()
                        .text_color(col),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(chk.name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(muted_fg)
                            .child(chk.detail.clone()),
                    )
            }))
    }

    fn render_line_diffs(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let t_lines = case
            .typst_visual
            .as_ref()
            .map(|v| v.lines.as_slice())
            .unwrap_or(&[]);
        let w_lines = case
            .word_visual
            .as_ref()
            .map(|v| v.lines.as_slice())
            .unwrap_or(&[]);
        let max_lines = t_lines.len().max(w_lines.len());

        let border_col = cx.theme().border;
        let muted_fg = cx.theme().muted_foreground;
        let info_col = cx.theme().info;
        let danger_col = cx.theme().danger;
        let mono = cx.theme().mono_font_family.clone();

        div()
            .id("diagnostics-lines-scroll")
            .flex_1()
            .min_h_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_1()
            .overflow_y_scrollbar()
            .when(max_lines == 0, |el| {
                el.child(
                    div()
                        .p_4()
                        .text_xs()
                        .text_color(muted_fg)
                        .child("No PDFKit text lines extracted for this case yet."),
                )
            })
            .children((0..max_lines).map(|i| {
                let tl = t_lines
                    .get(i)
                    .map(|l| {
                        format!(
                            "[p{}] ({:.1}, {:.1}, {:.1}×{:.1}) \"{}\"",
                            l.page, l.x_pt, l.y_top_pt, l.width_pt, l.height_pt, l.text
                        )
                    })
                    .unwrap_or_else(|| "—".to_string());
                let wl = w_lines
                    .get(i)
                    .map(|l| {
                        format!(
                            "[p{}] ({:.1}, {:.1}, {:.1}×{:.1}) \"{}\"",
                            l.page, l.x_pt, l.y_top_pt, l.width_pt, l.height_pt, l.text
                        )
                    })
                    .unwrap_or_else(|| "—".to_string());

                h_flex()
                    .w_full()
                    .px_2p5()
                    .py_1p5()
                    .gap_3()
                    .items_start()
                    .border_b_1()
                    .border_color(border_col)
                    .font_family(mono.clone())
                    .text_xs()
                    .child(
                        div()
                            .w_8()
                            .flex_shrink_0()
                            .text_color(muted_fg)
                            .child(format!("#{}", i + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(info_col)
                            .child(format!("Typst: {tl}")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(danger_col)
                            .child(format!("Word:  {wl}")),
                    )
            }))
    }
}

#[allow(clippy::too_many_arguments)]
fn geometry_row(
    c0: &str,
    c1: &str,
    c2: &str,
    c3: &str,
    c4: &str,
    is_header: bool,
    is_even: bool,
    raw_delta: Option<f64>,
    cx: &gpui_kit::App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let row_bg = if is_header {
        theme.muted
    } else if is_even {
        theme.list_even
    } else {
        theme.background
    };
    let delta_color = match raw_delta {
        Some(d) if d.abs() > 4.0 => theme.danger,
        Some(d) if d.abs() > 1.5 => theme.warning,
        Some(_) => theme.success,
        None => theme.foreground,
    };
    let mono = theme.mono_font_family.clone();

    h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(row_bg)
        .border_b_1()
        .border_color(theme.border)
        .text_xs()
        .font_weight(if is_header {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .child(div().w(px(144.0)).flex_shrink_0().truncate().child(c0.to_string()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .when(!is_header, |el| el.font_family(mono.clone()))
                .child(c1.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .when(!is_header, |el| el.font_family(mono.clone()))
                .child(c2.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .when(!is_header, |el| el.font_family(mono.clone()))
                .child(c3.to_string()),
        )
        .child(
            div()
                .w(px(116.0))
                .flex_shrink_0()
                .text_right()
                .when(!is_header, |el| {
                    el.font_family(mono)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(delta_color)
                })
                .child(c4.to_string()),
        )
}
