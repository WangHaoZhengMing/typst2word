//! Center column: Calm single-row toolbar, spacious Visual Paper Stage,
//! and quiet 1-line telemetry summary bar (`center_stage.rs`).

use std::path::PathBuf;

use gpui_kit::{
    FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Styled as _,
    Window, div, img, prelude::FluentBuilder as _, px, rgb,
    component::{
        ActiveTheme as _, IconName, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        resizable::{resizable_panel, v_resizable},
        scroll::ScrollableElement as _,
        tab::{Tab, TabBar},
        v_flex,
    },
};

use crate::{
    model::{BBoxPt, CaseReport, CaseStatus},
    ui::{
        macos_theme::{status_dot_color, status_tag},
        state::{CanvasMode, ParityStudioView},
    },
};

/// Physical paper background for PDF raster comparison sheets (always white paper).
const PAPER_WHITE_HEX: u32 = 0xFFFFFF;
/// Subtle paper border around rasterized page bitmaps.
const PAPER_BORDER_HEX: u32 = 0xDDE3EA;

impl ParityStudioView {
    pub(crate) fn render_center_column(
        &self,
        case: &CaseReport,
        _window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .overflow_hidden()
            .child(self.render_case_header(case, cx))
            .child(if self.show_diagnostics {
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        v_resizable("center-vertical-split")
                            .child(
                                resizable_panel()
                                    .size(px(460.0))
                                    .size_range(px(220.0)..px(820.0))
                                    .child(self.render_stage_with_summary(case, cx)),
                            )
                            .child(
                                resizable_panel()
                                    .size_range(px(160.0)..Pixels::MAX)
                                    .child(self.render_diagnostics_pane(case, cx)),
                            ),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.render_stage_with_summary(case, cx))
                    .into_any_element()
            })
    }

    fn render_stage_with_summary(
        &self,
        case: &CaseReport,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.render_visual_stage(case, cx)),
            )
            .child(self.render_summary_strip(case, cx))
    }

    fn render_case_header(
        &self,
        case: &CaseReport,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let border_col = cx.theme().border;
        let muted_fg = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();
        let dot_col = status_dot_color(case.status, cx);
        let radius_full = cx.theme().radius_full();

        let mode_idx = match self.canvas_mode {
            CanvasMode::SideBySide => 0,
            CanvasMode::OverlayOnly => 1,
            CanvasMode::ThreeUp => 2,
        };

        let suite_crumb = case
            .suite_rel_path
            .split_once('/')
            .map(|(_, rest)| rest.strip_suffix(".typ").unwrap_or(rest))
            .unwrap_or(&case.target_func);

        h_flex()
            .w_full()
            .h(px(40.0))
            .px_4()
            .items_center()
            .justify_between()
            .gap_3()
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(border_col)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size_2()
                            .flex_shrink_0()
                            .rounded(radius_full)
                            .bg(dot_col),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_family(mono.clone())
                            .text_xs()
                            .text_color(muted_fg)
                            .child(format!("{suite_crumb} /")),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(case.title.clone()),
                    )
                    .when(
                        matches!(case.status, CaseStatus::Drift | CaseStatus::Fail),
                        |el| el.child(status_tag(case.status)),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(
                        TabBar::new("canvas-mode-tabs")
                            .segmented()
                            .xsmall()
                            .selected_index(mode_idx)
                            .children([
                                Tab::new().label("Side-by-Side"),
                                Tab::new().label("Overlay"),
                                Tab::new().label("3-Up"),
                            ])
                            .on_click(cx.listener(|this, idx, _, cx| {
                                this.canvas_mode = match *idx {
                                    1 => CanvasMode::OverlayOnly,
                                    2 => CanvasMode::ThreeUp,
                                    _ => CanvasMode::SideBySide,
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_0p5()
                            .child(
                                Button::new("btn-zoom-out")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Minus)
                                    .tooltip("Zoom Out")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_zoom(-1, cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-zoom-reset")
                                    .ghost()
                                    .xsmall()
                                    .label(format!("{:.0}%", self.zoom * 100.0))
                                    .tooltip("Reset Zoom to 100%")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.zoom = 1.0;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-zoom-in")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .tooltip("Zoom In")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_zoom(1, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_visual_stage(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let base_w = (case.ir_summary.page_width_pt as f32 * 0.78).clamp(180.0, 340.0) * self.zoom;
        let aspect = if case.ir_summary.page_width_pt > 1.0 {
            (case.ir_summary.page_height_pt / case.ir_summary.page_width_pt) as f32
        } else {
            0.6
        }
        .clamp(0.32, 1.5);

        let typst_png = case.typst_visual.as_ref().map(|v| v.png_path.clone());
        let word_png = case.word_visual.as_ref().map(|v| v.png_path.clone());
        let overlay_png = case.comparison.as_ref().map(|c| c.overlay_png_path.clone());

        let page_dim_meta = format!(
            "{:.0}×{:.0} pt",
            case.ir_summary.page_width_pt, case.ir_summary.page_height_pt
        );
        let diff_meta = case
            .comparison
            .as_ref()
            .map(|cmp| format!("IoU {:.1}%", cmp.aligned_iou.max(cmp.raw_iou) * 100.0))
            .unwrap_or_else(|| "Blue=Typst · Red=Word".to_string());

        let typst_sub = format_bbox_line(case.typst_visual.as_ref().and_then(|v| v.ink_bbox));
        let word_sub = format_bbox_line(case.word_visual.as_ref().and_then(|v| v.ink_bbox));
        let overlay_sub = case.comparison.as_ref().map(|cmp| {
            format!(
                "Raw {:.1}% · Shape {:.1}%",
                cmp.raw_iou * 100.0,
                cmp.aligned_iou * 100.0
            )
        });

        let mut cards = Vec::new();
        match self.canvas_mode {
            CanvasMode::SideBySide => {
                let w = base_w * 1.22;
                cards.push(render_paper_card(
                    "Typst Reference",
                    &page_dim_meta,
                    typst_png,
                    typst_sub.as_deref(),
                    "Click 'Verify IR' to render Typst preview",
                    w,
                    aspect,
                    cx,
                ));
                cards.push(render_paper_card(
                    "Microsoft Word (.docx)",
                    &page_dim_meta,
                    word_png,
                    word_sub.as_deref(),
                    "Word PDF skipped · Click 'Run Word' to compare",
                    w,
                    aspect,
                    cx,
                ));
            }
            CanvasMode::OverlayOnly => {
                let w = base_w * 1.55;
                cards.push(render_paper_card(
                    "Pixel Diff Overlay",
                    &diff_meta,
                    overlay_png,
                    overlay_sub.as_deref(),
                    "Click 'Run Word' to generate pixel overlay",
                    w,
                    aspect,
                    cx,
                ));
            }
            CanvasMode::ThreeUp => {
                let w = base_w * 0.95;
                cards.push(render_paper_card(
                    "Typst Reference",
                    "PDF",
                    typst_png,
                    typst_sub.as_deref(),
                    "Not rendered yet",
                    w,
                    aspect,
                    cx,
                ));
                cards.push(render_paper_card(
                    "Pixel Diff",
                    &diff_meta,
                    overlay_png,
                    overlay_sub.as_deref(),
                    "Word skipped",
                    w,
                    aspect,
                    cx,
                ));
                cards.push(render_paper_card(
                    "Microsoft Word",
                    ".docx",
                    word_png,
                    word_sub.as_deref(),
                    "Word skipped",
                    w,
                    aspect,
                    cx,
                ));
            }
        }

        div()
            .id("visual-stage-canvas")
            .size_full()
            .bg(cx.theme().muted)
            .p_6()
            .flex()
            .flex_wrap()
            .items_start()
            .justify_center()
            .gap_6()
            .overflow_scrollbar()
            .children(cards)
    }

    fn render_summary_strip(
        &self,
        case: &CaseReport,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let border_col = cx.theme().border;
        let bg = cx.theme().background;
        let muted_fg = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();

        let m = case.ir_summary.margins_pt;
        let margin_str = if (m[0] - m[1]).abs() < 0.05
            && (m[0] - m[2]).abs() < 0.05
            && (m[0] - m[3]).abs() < 0.05
        {
            format!("margin {:.1}pt", m[0])
        } else {
            format!(
                "margins [{:.1}, {:.1}, {:.1}, {:.1}]pt{}",
                m[0],
                m[1],
                m[2],
                m[3],
                if case.ir_summary.mirror_margins {
                    " · mirror"
                } else {
                    ""
                }
            )
        };

        let passed_checks = case.checks.iter().filter(|c| c.passed).count();
        let total_checks = case.checks.len();
        let checks_label = if total_checks > 0 {
            format!("{passed_checks}/{total_checks} checks")
        } else {
            "Not verified yet".to_string()
        };

        h_flex()
            .w_full()
            .px_4()
            .py_1p5()
            .items_center()
            .justify_between()
            .gap_3()
            .bg(bg)
            .border_t_1()
            .border_color(border_col)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .font_family(mono.clone())
                    .text_xs()
                    .child(
                        div()
                            .truncate()
                            .text_color(cx.theme().foreground)
                            .child(case.isolated_variable.clone()),
                    )
                    .child(div().text_color(muted_fg).child("·"))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(muted_fg)
                            .child(margin_str),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .when(!case.status_reasons.is_empty(), |el| {
                        el.child(
                            div()
                                .max_w(px(260.0))
                                .truncate()
                                .text_xs()
                                .text_color(if case.status == CaseStatus::Fail {
                                    cx.theme().danger
                                } else {
                                    cx.theme().warning
                                })
                                .child(case.status_reasons.join(" · ")),
                        )
                    })
                    .child(
                        div()
                            .font_family(mono)
                            .text_xs()
                            .text_color(if total_checks > 0 && passed_checks == total_checks {
                                cx.theme().success
                            } else {
                                muted_fg
                            })
                            .child(checks_label),
                    )
                    .child(
                        Button::new("btn-summary-toggle-details")
                            .ghost()
                            .xsmall()
                            .selected(self.show_diagnostics)
                            .icon(if self.show_diagnostics {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronUp
                            })
                            .label("Details")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_diagnostics = !this.show_diagnostics;
                                cx.notify();
                            })),
                    ),
            )
    }
}

#[allow(clippy::too_many_arguments)]
fn render_paper_card(
    title: &str,
    meta: &str,
    png_path: Option<String>,
    subtitle: Option<&str>,
    empty_hint: &str,
    width_px: f32,
    aspect: f32,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let has_file = png_path
        .as_ref()
        .is_some_and(|p| !p.is_empty() && PathBuf::from(p).exists());
    let border_col = cx.theme().border;
    let muted_fg = cx.theme().muted_foreground;
    let radius_sm = cx.theme().radius / 2.0;
    let mono = cx.theme().mono_font_family.clone();
    let placeholder_h = (width_px * aspect).clamp(108.0, 380.0);

    v_flex()
        .w(px(width_px))
        .gap_2()
        .child(
            h_flex()
                .w_full()
                .px_0p5()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_xs()
                        .text_color(muted_fg)
                        .child(meta.to_string()),
                ),
        )
        .child(if has_file {
            let p = PathBuf::from(png_path.unwrap());
            div()
                .w(px(width_px))
                .rounded(radius_sm)
                .overflow_hidden()
                .border_1()
                .border_color(rgb(PAPER_BORDER_HEX))
                .bg(rgb(PAPER_WHITE_HEX))
                .child(img(p).w(px(width_px)))
                .into_any_element()
        } else {
            div()
                .w(px(width_px))
                .h(px(placeholder_h))
                .px_4()
                .rounded(radius_sm)
                .bg(cx.theme().popover)
                .border_1()
                .border_color(border_col)
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .text_xs()
                .text_color(muted_fg)
                .child(empty_hint.to_string())
                .into_any_element()
        })
        .when_some(subtitle, |el, sub| {
            el.child(
                div()
                    .px_0p5()
                    .font_family(mono)
                    .text_xs()
                    .text_color(muted_fg)
                    .truncate()
                    .child(sub.to_string()),
            )
        })
}

fn format_bbox_line(bbox: Option<BBoxPt>) -> Option<String> {
    bbox.map(|b| {
        format!(
            "Ink L:{:.1} T:{:.1} W:{:.1} H:{:.1} pt",
            b.left_pt, b.top_pt, b.width_pt, b.height_pt
        )
    })
}
