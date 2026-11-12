//! Native macOS unified `TitleBar` and quiet bottom `StatusBar` for `visual-test`.

use gpui_kit::{
    FontWeight, IntoElement, ParentElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, TitleBar,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        spinner::Spinner,
        status_bar::StatusBar,
    },
};

use crate::ui::{macos_theme::toggle_theme_mode, state::ParityStudioView};

impl ParityStudioView {
    pub(crate) fn render_titlebar(
        &self,
        _window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let s = &self.report.stats;
        let is_dark = cx.theme().is_dark();
        let muted_fg = cx.theme().muted_foreground;
        let success_col = cx.theme().success;
        let warning_col = cx.theme().warning;
        let danger_col = cx.theme().danger;
        let info_col = cx.theme().info;

        let (summary_dot, summary_label) = if s.fail_count > 0 {
            (
                danger_col,
                format!("{} fail / {}", s.fail_count, s.total_cases),
            )
        } else if s.drift_count > 0 {
            (
                warning_col,
                format!(
                    "{} pass · {} drift",
                    s.pass_count, s.drift_count
                ),
            )
        } else if s.pass_count > 0 {
            (success_col, format!("{}/{} pass", s.pass_count, s.total_cases))
        } else {
            (info_col, format!("{} IR verified", s.total_cases))
        };

        let view_entity = cx.entity();

        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .pr_3()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    // Left cluster (after macOS traffic lights)
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("toggle-left-sidebar")
                                .ghost()
                                .xsmall()
                                .selected(self.show_left_sidebar)
                                .icon(if self.show_left_sidebar {
                                    IconName::PanelLeftClose
                                } else {
                                    IconName::PanelLeftOpen
                                })
                                .tooltip("Toggle Stdlib Tree")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_left_sidebar = !this.show_left_sidebar;
                                    cx.notify();
                                })),
                        )
                        .child(div().h_3p5().w_px().bg(cx.theme().border))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Visual Test"),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .px_2()
                                .py_0p5()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().secondary)
                                .child(
                                    div()
                                        .size_1p5()
                                        .rounded(cx.theme().radius_full())
                                        .bg(summary_dot),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_fg)
                                        .child(summary_label),
                                ),
                        ),
                )
                .child(
                    // Right cluster: concise run actions + pane toggles + appearance
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            Button::new("btn-verify-ir-all")
                                .outline()
                                .xsmall()
                                .icon(IconName::CircleCheck)
                                .disabled(self.is_running)
                                .label("Verify IR")
                                .tooltip("Verify IR & Word XML across all cases without opening Word")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.trigger_run(false, true, window, cx);
                                })),
                        )
                        .child(
                            Button::new("btn-rerun-selected")
                                .primary()
                                .xsmall()
                                .icon(IconName::Play)
                                .loading(self.is_running)
                                .disabled(self.is_running)
                                .label(if self.is_running {
                                    "Running…"
                                } else {
                                    "Run Word"
                                })
                                .tooltip("Render & compare selected case with Microsoft Word")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.trigger_run(true, false, window, cx);
                                })),
                        )
                        .child(
                            Button::new("btn-more-run-actions")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Ellipsis)
                                .disabled(self.is_running)
                                .tooltip("More Run & Reload Actions")
                                .dropdown_menu(move |menu, window, _cx| {
                                    let v1 = view_entity.clone();
                                    let v2 = view_entity.clone();
                                    let v3 = view_entity.clone();
                                    menu.min_w(gpui_kit::px(220.0))
                                        .item(
                                            PopupMenuItem::new("Verify Selected Case (IR Only)")
                                                .on_click(window.listener_for(
                                                    &v1,
                                                    |this, _, window, cx| {
                                                        this.trigger_run(true, true, window, cx);
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new("Run Full Suite with Word")
                                                .on_click(window.listener_for(
                                                    &v2,
                                                    |this, _, window, cx| {
                                                        this.trigger_run(false, false, window, cx);
                                                    },
                                                )),
                                        )
                                        .separator()
                                        .item(
                                            PopupMenuItem::new("Reload report.json from Disk")
                                                .on_click(window.listener_for(
                                                    &v3,
                                                    |this, _, window, cx| {
                                                        this.reload_from_disk(window, cx);
                                                        this.banner_message = Some(
                                                            "Reloaded report.json from disk"
                                                                .to_string(),
                                                        );
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                }),
                        )
                        .child(div().h_3p5().w_px().bg(cx.theme().border).mx_0p5())
                        .child(
                            Button::new("toggle-diagnostics-pane")
                                .ghost()
                                .xsmall()
                                .selected(self.show_diagnostics)
                                .icon(if self.show_diagnostics {
                                    IconName::PanelBottomOpen
                                } else {
                                    IconName::PanelBottom
                                })
                                .tooltip("Toggle Details Drawer")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_diagnostics = !this.show_diagnostics;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("toggle-right-inspector")
                                .ghost()
                                .xsmall()
                                .selected(self.show_right_inspector)
                                .icon(if self.show_right_inspector {
                                    IconName::PanelRightClose
                                } else {
                                    IconName::PanelRightOpen
                                })
                                .tooltip("Toggle Code Inspector")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_right_inspector = !this.show_right_inspector;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("toggle-macos-appearance")
                                .ghost()
                                .xsmall()
                                .icon(if is_dark {
                                    IconName::Sun
                                } else {
                                    IconName::Moon
                                })
                                .tooltip(if is_dark {
                                    "Switch to Light Appearance"
                                } else {
                                    "Switch to Dark Appearance"
                                })
                                .on_click(cx.listener(|_, _, window, cx| {
                                    toggle_theme_mode(window, cx);
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }

    pub(crate) fn render_status_bar(
        &self,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let s = &self.report.stats;
        let mono = cx.theme().mono_font_family.clone();
        let muted_fg = cx.theme().muted_foreground;
        let pass_col = cx.theme().success;
        let fail_col = cx.theme().danger;
        let info_col = cx.theme().info;
        let full_radius = cx.theme().radius_full();

        let status_text = self
            .banner_message
            .clone()
            .unwrap_or_else(|| format!("Ready · {} standard-library cases", s.total_cases));

        let show_word_deltas = self.report.word_enabled && (s.pass_count + s.drift_count) > 0;

        let active_loc = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
            .map(|c| format!("{} · {}", c.suite_rel_path, c.id))
            .unwrap_or_else(|| "No selection".to_string());

        let mut bar = StatusBar::new().left(
            h_flex()
                .items_center()
                .gap_1p5()
                .when(self.is_running, |el| el.child(Spinner::new().xsmall()))
                .when(!self.is_running, |el| {
                    el.child(
                        div()
                            .size_1p5()
                            .rounded(full_radius)
                            .bg(if s.fail_count > 0 {
                                fail_col
                            } else if s.pass_count > 0 {
                                pass_col
                            } else {
                                info_col
                            }),
                    )
                })
                .child(div().text_xs().text_color(muted_fg).child(status_text)),
        );

        if show_word_deltas {
            bar = bar.child(
                div()
                    .font_family(mono.clone())
                    .text_xs()
                    .text_color(muted_fg)
                    .child(format!(
                        "Mean |Δ| L:{:.2} T:{:.2} W:{:.2}pt · IoU {:.1}%",
                        s.mean_abs_delta_left_pt,
                        s.mean_abs_delta_top_pt,
                        s.mean_abs_delta_width_pt,
                        s.mean_aligned_iou.max(s.mean_raw_iou) * 100.0,
                    )),
            );
        }

        bar.right(
            div()
                .font_family(mono)
                .text_xs()
                .text_color(muted_fg)
                .child(active_loc),
        )
    }
}
