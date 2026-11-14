//! Collapsible lower Telemetry & Assertions Drawer (`diagnostics/mod.rs`).
//!
//! Uses a space-saving macOS Section Pop-Up Dropdown (`[ Geometry Telemetry ▾ ]`)
//! instead of a multi-tab bar, paired with inline IoU badges and a quick close button.

mod checks_lines;
mod geometry;

use gpui_kit::{
    InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    assets::IconName as Lucide,
    component::{
        ActiveTheme as _, IconName, Side, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        scroll::ScrollableElement as _,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
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
        let theme = cx.theme();
        let bg = theme.background;
        let header_bg = theme.title_bar;
        let border_col = theme.border;
        let muted_fg = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();

        let passed_checks = case.checks.iter().filter(|c| c.passed).count();
        let total_checks = case.checks.len();
        let t_lines = case.typst_visual.as_ref().map_or(0, |v| v.lines.len());
        let w_lines = case.word_visual.as_ref().map_or(0, |v| v.lines.len());

        let cur_tab = self.diagnostics_tab;
        let dropdown_label = match cur_tab {
            DiagnosticsTab::Geometry => "Geometry Telemetry".to_string(),
            DiagnosticsTab::Checks => format!("IR & OOXML Checks ({passed_checks}/{total_checks})"),
            DiagnosticsTab::Lines => format!("PDFKit Text Lines ({t_lines}/{w_lines})"),
        };

        let checks_menu_lbl = format!("IR & OOXML Checks ({passed_checks}/{total_checks})");
        let lines_menu_lbl = format!("PDFKit Text Lines ({t_lines} Typst / {w_lines} Word)");
        let view_for_tab = cx.entity();

        let body = match cur_tab {
            DiagnosticsTab::Geometry => self.render_geometry_table(case, cx).into_any_element(),
            DiagnosticsTab::Checks => self.render_checks_list(case, cx).into_any_element(),
            DiagnosticsTab::Lines => self.render_line_diffs(case, cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(bg)
            .border_t_1()
            .border_color(border_col)
            .overflow_hidden()
            .child(
                // Compact 36px macOS drawer header
                h_flex()
                    .w_full()
                    .h(px(36.0))
                    .px_3()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_col)
                    .child(
                        Button::new("diagnostics-tab-dropdown")
                            .outline()
                            .xsmall()
                            .icon(Lucide::Activity)
                            .dropdown_caret(true)
                            .label(dropdown_label)
                            .tooltip("Select Telemetry Section")
                            .dropdown_menu(move |menu, window, _cx| {
                                let v1 = view_for_tab.clone();
                                let v2 = view_for_tab.clone();
                                let v3 = view_for_tab.clone();
                                menu.min_w(px(260.0))
                                    .check_side(Side::Left)
                                    .label("Telemetry & Verification")
                                    .item(
                                        PopupMenuItem::new(DiagnosticsTab::Geometry.label())
                                            .checked(cur_tab == DiagnosticsTab::Geometry)
                                            .on_click(window.listener_for(&v1, |this, _, _, cx| {
                                                this.diagnostics_tab = DiagnosticsTab::Geometry;
                                                cx.notify();
                                            })),
                                    )
                                    .item(
                                        PopupMenuItem::new(checks_menu_lbl.clone())
                                            .checked(cur_tab == DiagnosticsTab::Checks)
                                            .on_click(window.listener_for(&v2, |this, _, _, cx| {
                                                this.diagnostics_tab = DiagnosticsTab::Checks;
                                                cx.notify();
                                            })),
                                    )
                                    .item(
                                        PopupMenuItem::new(lines_menu_lbl.clone())
                                            .checked(cur_tab == DiagnosticsTab::Lines)
                                            .on_click(window.listener_for(&v3, |this, _, _, cx| {
                                                this.diagnostics_tab = DiagnosticsTab::Lines;
                                                cx.notify();
                                            })),
                                    )
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .when_some(case.comparison.as_ref(), |el, cmp| {
                                let best_iou = cmp.aligned_iou.max(cmp.raw_iou);
                                let iou_col = if best_iou >= 0.72 {
                                    cx.theme().success
                                } else if best_iou >= 0.40 {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().warning
                                };
                                el.child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .font_family(mono.clone())
                                        .text_xs()
                                        .child(
                                            div()
                                                .text_color(muted_fg)
                                                .child(format!("Raw {:.1}%", cmp.raw_iou * 100.0)),
                                        )
                                        .child(div().text_color(muted_fg).child("·"))
                                        .child(
                                            div()
                                                .text_color(iou_col)
                                                .child(format!(
                                                    "Shape IoU {:.1}%",
                                                    cmp.aligned_iou * 100.0
                                                )),
                                        )
                                        .when(cmp.line_count_delta != 0, |row| {
                                            row.child(div().text_color(muted_fg).child("·")).child(
                                                div()
                                                    .text_color(cx.theme().warning)
                                                    .child(format!(
                                                        "ΔLines {:+}",
                                                        cmp.line_count_delta
                                                    )),
                                            )
                                        }),
                                )
                            })
                            .child(
                                Button::new("diagnostics-close-btn")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .tooltip("Close Telemetry Drawer (⌥⌘D)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_diagnostics = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("diagnostics-scroll-body")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_scrollbar()
                    .child(body),
            )
    }
}
