//! Bottom 1-line isolated-variable & check summary strip (`stage/summary_bar.rs`).

use gpui_kit::{
    IntoElement, ParentElement as _, Styled as _,
    component::{
        ActiveTheme as _, IconName, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    model::{CaseReport, CaseStatus},
    ui::state::ParityStudioView,
};

impl ParityStudioView {
    pub(super) fn render_summary_strip(
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
            "Unverified".to_string()
        };

        h_flex()
            .w_full()
            .px_3p5()
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
                    .child(div().flex_shrink_0().text_color(muted_fg).child(margin_str)),
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
                            .label("Telemetry")
                            .tooltip("Toggle Telemetry & Assertions Drawer (⌥⌘D)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_diagnostics = !this.show_diagnostics;
                                cx.notify();
                            })),
                    ),
            )
    }
}
