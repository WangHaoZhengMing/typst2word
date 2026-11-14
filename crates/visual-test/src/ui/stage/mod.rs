//! Center column: Spacious Case Breadcrumb Bar + macOS Preview Paper Canvas (`stage/mod.rs`).
//!
//! Replaces wide segmented tabs and 3-button zoom clusters with two compact macOS
//! pop-up dropdown menus (`[ Side-by-Side ▾ ]` and `[ 100% ▾ ]`) so the active case
//! breadcrumb and title get maximum breathing room.

mod paper_card;
mod summary_bar;

use gpui_kit::{
    Anchor, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Styled as _, Window,
    assets::IconName as Lucide,
    component::{
        ActiveTheme as _, Side, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        resizable::{resizable_panel, v_resizable},
        scroll::ScrollableElement as _,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    model::{CaseReport, CaseStatus},
    ui::{
        macos_theme::{status_dot_color, status_tag},
        state::{CanvasMode, ParityStudioView, ZOOM_STEPS},
    },
};

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
            .child(self.render_stage_header(case, cx))
            .child(if self.show_diagnostics {
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        v_resizable("center-vertical-split")
                            .child(
                                resizable_panel()
                                    .size(px(470.0))
                                    .size_range(px(220.0)..px(840.0))
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

    fn render_stage_header(
        &self,
        case: &CaseReport,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let border_col = cx.theme().border;
        let muted_fg = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();
        let dot_col = status_dot_color(case.status, cx);
        let radius_full = cx.theme().radius_full();

        let suite_crumb = case
            .suite_rel_path
            .split_once('/')
            .map(|(_, rest)| rest.strip_suffix(".typ").unwrap_or(rest))
            .unwrap_or(&case.target_func);

        let cur_mode = self.canvas_mode;
        let cur_zoom = self.zoom;
        let view_for_mode = cx.entity();
        let view_for_zoom = cx.entity();

        h_flex()
            .w_full()
            .h(px(38.0))
            .px_3p5()
            .items_center()
            .justify_between()
            .gap_3()
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(border_col)
            .child(
                // Left: Status dot + suite breadcrumb + Case Title + Drift/Fail badge
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
                            .font_family(mono)
                            .text_xs()
                            .text_color(muted_fg)
                            .child(format!("{suite_crumb}  ›")),
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
                // Right: Compact Canvas Mode Pop-Up + Zoom Pop-Up
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Button::new("stage-mode-dropdown")
                            .outline()
                            .xsmall()
                            .icon(Lucide::Columns2)
                            .dropdown_caret(true)
                            .label(cur_mode.short_label())
                            .tooltip("Choose Comparison Layout")
                            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, window, _cx| {
                                let v1 = view_for_mode.clone();
                                let v2 = view_for_mode.clone();
                                let v3 = view_for_mode.clone();
                                let v4 = view_for_mode.clone();
                                let v5 = view_for_mode.clone();
                                menu.min_w(px(220.0))
                                    .check_side(Side::Left)
                                    .label("Comparison View")
                                    .item(
                                        PopupMenuItem::new("Side-by-Side (Typst vs Word)")
                                            .checked(cur_mode == CanvasMode::SideBySide)
                                            .on_click(window.listener_for(
                                                &v1,
                                                |this, _, _, cx| {
                                                    this.canvas_mode = CanvasMode::SideBySide;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new("Pixel Diff Overlay")
                                            .checked(cur_mode == CanvasMode::OverlayOnly)
                                            .on_click(window.listener_for(
                                                &v2,
                                                |this, _, _, cx| {
                                                    this.canvas_mode = CanvasMode::OverlayOnly;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new("3-Up Comparison Sheet")
                                            .checked(cur_mode == CanvasMode::ThreeUp)
                                            .on_click(window.listener_for(
                                                &v3,
                                                |this, _, _, cx| {
                                                    this.canvas_mode = CanvasMode::ThreeUp;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .separator()
                                    .label("Single Document")
                                    .item(
                                        PopupMenuItem::new("Typst Reference Only")
                                            .checked(cur_mode == CanvasMode::TypstOnly)
                                            .on_click(window.listener_for(
                                                &v4,
                                                |this, _, _, cx| {
                                                    this.canvas_mode = CanvasMode::TypstOnly;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new("Microsoft Word (.docx) Only")
                                            .checked(cur_mode == CanvasMode::WordOnly)
                                            .on_click(window.listener_for(
                                                &v5,
                                                |this, _, _, cx| {
                                                    this.canvas_mode = CanvasMode::WordOnly;
                                                    cx.notify();
                                                },
                                            )),
                                    )
                            }),
                    )
                    .child(
                        Button::new("stage-zoom-dropdown")
                            .ghost()
                            .xsmall()
                            .dropdown_caret(true)
                            .label(format!("{:.0}%", cur_zoom * 100.0))
                            .tooltip("Paper Zoom Level (⌘+ / ⌘- / ⌘0)")
                            .dropdown_menu_with_anchor(
                                Anchor::TopRight,
                                move |mut menu, window, _cx| {
                                    menu = menu
                                        .min_w(px(160.0))
                                        .check_side(Side::Left)
                                        .label("Paper Scale");

                                    for step in ZOOM_STEPS {
                                        let v_z = view_for_zoom.clone();
                                        let pct = (step * 100.0).round() as usize;
                                        let lbl = if (step - 1.0).abs() < 0.01 {
                                            format!("{pct}% (Actual)")
                                        } else {
                                            format!("{pct}%")
                                        };
                                        menu = menu.item(
                                            PopupMenuItem::new(lbl)
                                                .checked((cur_zoom - step).abs() < 0.02)
                                                .on_click(window.listener_for(
                                                    &v_z,
                                                    move |this, _, _, cx| {
                                                        this.zoom = step;
                                                        cx.notify();
                                                    },
                                                )),
                                        );
                                    }

                                    let vz_in = view_for_zoom.clone();
                                    let vz_out = view_for_zoom.clone();
                                    let vz_rst = view_for_zoom.clone();
                                    menu.separator()
                                        .item(PopupMenuItem::new("Zoom In   ⌘+").on_click(
                                            window.listener_for(&vz_in, |this, _, _, cx| {
                                                this.step_zoom(1, cx);
                                            }),
                                        ))
                                        .item(PopupMenuItem::new("Zoom Out   ⌘-").on_click(
                                            window.listener_for(&vz_out, |this, _, _, cx| {
                                                this.step_zoom(-1, cx);
                                            }),
                                        ))
                                        .item(PopupMenuItem::new("Reset 100%   ⌘0").on_click(
                                            window.listener_for(&vz_rst, |this, _, _, cx| {
                                                this.zoom = 1.0;
                                                cx.notify();
                                            }),
                                        ))
                                },
                            ),
                    ),
            )
    }

    fn render_visual_stage(
        &self,
        case: &CaseReport,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let base_w = (case.ir_summary.page_width_pt as f32 * 0.80).clamp(190.0, 350.0) * self.zoom;
        let aspect = if case.ir_summary.page_width_pt > 1.0 {
            (case.ir_summary.page_height_pt / case.ir_summary.page_width_pt) as f32
        } else {
            0.6
        }
        .clamp(0.32, 1.5);

        let typst_png = case.typst_visual.as_ref().map(|v| v.png_path.clone());
        let word_png = case.word_visual.as_ref().map(|v| v.png_path.clone());
        let overlay_png = case.comparison.as_ref().map(|c| c.overlay_png_path.clone());

        let t_pages = case.typst_visual.as_ref().map_or(1, |v| v.page_count);
        let w_pages = case.word_visual.as_ref().map_or(1, |v| v.page_count);

        let typst_dim_meta = if t_pages > 1 {
            format!(
                "{t_pages}p · {:.0}×{:.0} pt",
                case.ir_summary.page_width_pt, case.ir_summary.page_height_pt
            )
        } else {
            format!(
                "{:.0}×{:.0} pt",
                case.ir_summary.page_width_pt, case.ir_summary.page_height_pt
            )
        };
        let word_dim_meta = if w_pages > 1 {
            format!(
                "{w_pages}p · {:.0}×{:.0} pt",
                case.ir_summary.page_width_pt, case.ir_summary.page_height_pt
            )
        } else {
            format!(
                "{:.0}×{:.0} pt",
                case.ir_summary.page_width_pt, case.ir_summary.page_height_pt
            )
        };
        let diff_meta = case
            .comparison
            .as_ref()
            .map(|cmp| format!("IoU {:.1}%", cmp.aligned_iou.max(cmp.raw_iou) * 100.0))
            .unwrap_or_else(|| "Blue=Typst · Red=Word".to_string());

        let typst_sub = paper_card::format_bbox_line(
            case.typst_visual.as_ref().and_then(|v| v.ink_bbox),
        );
        let word_sub = case
            .comparison
            .as_ref()
            .map(|cmp| {
                format!(
                    "ΔL:{:+.1}  ΔT:{:+.1}  ΔW:{:+.1}  ΔH:{:+.1} pt · IoU {:.1}%",
                    cmp.delta_left_pt,
                    cmp.delta_top_pt,
                    cmp.delta_width_pt,
                    cmp.delta_height_pt,
                    cmp.aligned_iou.max(cmp.raw_iou) * 100.0
                )
            })
            .or_else(|| {
                paper_card::format_bbox_line(case.word_visual.as_ref().and_then(|v| v.ink_bbox))
            });
        let overlay_sub = case.comparison.as_ref().map(|cmp| {
            format!(
                "Raw IoU {:.1}% · Shape-Aligned IoU {:.1}%",
                cmp.raw_iou * 100.0,
                cmp.aligned_iou * 100.0
            )
        });

        let mut cards = Vec::new();
        match self.canvas_mode {
            CanvasMode::SideBySide => {
                let w = base_w * 1.22;
                cards.push(self.render_paper_card(
                    "typst",
                    "Typst Reference",
                    &typst_dim_meta,
                    typst_png,
                    typst_sub.as_deref(),
                    "Not compiled yet",
                    false,
                    w,
                    aspect,
                    cx,
                ));
                cards.push(self.render_paper_card(
                    "word",
                    "Microsoft Word (.docx)",
                    &word_dim_meta,
                    word_png,
                    word_sub.as_deref(),
                    "Word PDF not rendered yet",
                    true,
                    w,
                    aspect,
                    cx,
                ));
            }
            CanvasMode::OverlayOnly => {
                let w = base_w * 1.58;
                cards.push(self.render_paper_card(
                    "overlay",
                    "Pixel Diff Overlay",
                    &diff_meta,
                    overlay_png,
                    overlay_sub.as_deref(),
                    "Render with Word to generate pixel diff overlay",
                    true,
                    w,
                    aspect,
                    cx,
                ));
            }
            CanvasMode::ThreeUp => {
                let w = base_w * 0.94;
                cards.push(self.render_paper_card(
                    "typst-3",
                    "Typst Reference",
                    &typst_dim_meta,
                    typst_png,
                    typst_sub.as_deref(),
                    "Not compiled yet",
                    false,
                    w,
                    aspect,
                    cx,
                ));
                cards.push(self.render_paper_card(
                    "overlay-3",
                    "Pixel Diff",
                    &diff_meta,
                    overlay_png,
                    overlay_sub.as_deref(),
                    "Word not rendered",
                    true,
                    w,
                    aspect,
                    cx,
                ));
                cards.push(self.render_paper_card(
                    "word-3",
                    "Microsoft Word",
                    &word_dim_meta,
                    word_png,
                    word_sub.as_deref(),
                    "Word not rendered",
                    true,
                    w,
                    aspect,
                    cx,
                ));
            }
            CanvasMode::TypstOnly => {
                let w = base_w * 1.58;
                cards.push(self.render_paper_card(
                    "typst-1",
                    "Typst Reference",
                    &typst_dim_meta,
                    typst_png,
                    typst_sub.as_deref(),
                    "Not compiled yet",
                    false,
                    w,
                    aspect,
                    cx,
                ));
            }
            CanvasMode::WordOnly => {
                let w = base_w * 1.58;
                cards.push(self.render_paper_card(
                    "word-1",
                    "Microsoft Word (.docx)",
                    &word_dim_meta,
                    word_png,
                    word_sub.as_deref(),
                    "Word PDF not rendered yet",
                    true,
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
            .gap_7()
            .overflow_scrollbar()
            .children(cards)
    }
}
