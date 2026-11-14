//! Quiet macOS bottom `StatusBar` with live runner status, aggregate parity metrics,
//! and Prev / Next case stepper controls (`status_bar.rs`).

use gpui_kit::{
    IntoElement, ParentElement as _, Styled as _,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        spinner::Spinner,
        status_bar::StatusBar,
    },
    div,
    prelude::FluentBuilder as _,
};

use crate::ui::state::ParityStudioView;

impl ParityStudioView {
    pub(crate) fn render_status_bar(
        &self,
        filtered: &[usize],
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
            .unwrap_or_else(|| format!("Ready · {} typesetting cases", s.total_cases));

        let show_word_deltas = self.report.word_enabled && (s.pass_count + s.drift_count) > 0;

        let cur_pos = filtered
            .iter()
            .position(|&idx| self.report.cases[idx].id == self.selected_case_id)
            .map(|i| i + 1)
            .unwrap_or(0);
        let pos_badge = if filtered.is_empty() {
            "0/0".to_string()
        } else {
            format!("{cur_pos}/{}", filtered.len())
        };

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
            h_flex()
                .items_center()
                .gap_1p5()
                .child(
                    div()
                        .font_family(mono)
                        .text_xs()
                        .text_color(muted_fg)
                        .child(format!("{active_loc}  ({pos_badge})")),
                )
                .child(
                    Button::new("status-prev-case")
                        .ghost()
                        .xsmall()
                        .disabled(filtered.is_empty())
                        .icon(IconName::ChevronLeft)
                        .tooltip("Previous Case (⌘[)")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.select_adjacent_case(-1, window, cx);
                        })),
                )
                .child(
                    Button::new("status-next-case")
                        .ghost()
                        .xsmall()
                        .disabled(filtered.is_empty())
                        .icon(IconName::ChevronRight)
                        .tooltip("Next Case (⌘])")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.select_adjacent_case(1, window, cx);
                        })),
                ),
        )
    }
}
