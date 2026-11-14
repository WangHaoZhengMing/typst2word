//! IR/OOXML structural assertion list & PDFKit extracted text-line comparator (`diagnostics/checks_lines.rs`).

use gpui_kit::{
    FontWeight, IntoElement, ParentElement as _, Styled as _,
    component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    model::{CaseReport, CaseStatus, LineMetric},
    ui::state::ParityStudioView,
};

impl ParityStudioView {
    pub(super) fn render_checks_list(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_col = theme.border;
        let muted_fg = theme.muted_foreground;
        let even_bg = theme.list_even;
        let success_col = theme.success;
        let danger_col = theme.danger;
        let warning_col = theme.warning;
        let mono = theme.mono_font_family.clone();

        if case.checks.is_empty() {
            return v_flex()
                .w_full()
                .p_6()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(muted_fg)
                .child("No structural checks recorded yet — press ⌘R to run verification.")
                .into_any_element();
        }

        v_flex()
            .w_full()
            .text_xs()
            .when(!case.status_reasons.is_empty(), |el| {
                let col = if case.status == CaseStatus::Fail {
                    danger_col
                } else {
                    warning_col
                };
                el.child(
                    h_flex()
                        .w_full()
                        .px_3p5()
                        .py_1p5()
                        .gap_2()
                        .items_center()
                        .bg(col.opacity(0.09))
                        .border_b_1()
                        .border_color(border_col)
                        .font_family(mono.clone())
                        .text_color(col)
                        .child(case.status_reasons.join("  ·  ")),
                )
            })
            .children(case.checks.iter().enumerate().map(|(idx, item)| {
                let icon_col = if item.passed { success_col } else { danger_col };
                let icon_name = if item.passed {
                    IconName::Check
                } else {
                    IconName::Close
                };

                h_flex()
                    .w_full()
                    .px_3p5()
                    .py_1p5()
                    .items_center()
                    .gap_2p5()
                    .when(idx % 2 == 1, |el| el.bg(even_bg))
                    .border_b_1()
                    .border_color(border_col.opacity(0.45))
                    .child(
                        Icon::new(icon_name)
                            .xsmall()
                            .text_color(icon_col)
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .w(px(190.0))
                            .flex_shrink_0()
                            .truncate()
                            .font_family(mono.clone())
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if item.passed {
                                theme.foreground
                            } else {
                                danger_col
                            })
                            .child(item.name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(mono.clone())
                            .text_color(muted_fg)
                            .child(item.detail.clone()),
                    )
            }))
            .into_any_element()
    }

    pub(super) fn render_line_diffs(
        &self,
        case: &CaseReport,
        cx: &gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_col = theme.border;
        let muted_fg = theme.muted_foreground;
        let even_bg = theme.list_even;
        let mono = theme.mono_font_family.clone();

        let t_lines: &[LineMetric] = case
            .typst_visual
            .as_ref()
            .map_or(&[], |v| v.lines.as_slice());
        let w_lines: &[LineMetric] = case
            .word_visual
            .as_ref()
            .map_or(&[], |v| v.lines.as_slice());

        let max_len = t_lines.len().max(w_lines.len());
        if max_len == 0 {
            return v_flex()
                .w_full()
                .p_6()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(muted_fg)
                .child("No PDFKit text lines extracted (shape-only case or not yet rendered).")
                .into_any_element();
        }

        let fmt_line = |opt: Option<&LineMetric>| -> String {
            match opt {
                Some(l) => format!(
                    "p{} ({:.1}, {:.1})  {}",
                    l.page,
                    l.x_pt,
                    l.y_top_pt,
                    l.text.trim()
                ),
                None => "—".to_string(),
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
                    .child(div().w(px(36.0)).flex_shrink_0().child("#"))
                    .child(div().flex_1().min_w_0().child("Typst PDF Line (x, yTop · text)"))
                    .child(div().flex_1().min_w_0().child("Word PDF Line (x, yTop · text)"))
                    .child(div().w(px(110.0)).flex_shrink_0().child("ΔyTop")),
            )
            .children((0..max_len).map(|idx| {
                let tl = t_lines.get(idx);
                let wl = w_lines.get(idx);
                let dy_str = match (tl, wl) {
                    (Some(t), Some(w)) if t.page == w.page => {
                        format!("{:+.2} pt", w.y_top_pt - t.y_top_pt)
                    }
                    (Some(_), Some(_)) => "diff page".to_string(),
                    _ => "unmatched".to_string(),
                };
                let dy_col = match (tl, wl) {
                    (Some(t), Some(w)) if t.page == w.page && (w.y_top_pt - t.y_top_pt).abs() <= 2.5 => {
                        theme.success
                    }
                    (Some(_), Some(_)) => theme.foreground,
                    _ => theme.warning,
                };

                h_flex()
                    .w_full()
                    .px_3p5()
                    .py_1()
                    .items_center()
                    .gap_2()
                    .when(idx % 2 == 1, |el| el.bg(even_bg))
                    .border_b_1()
                    .border_color(border_col.opacity(0.45))
                    .font_family(mono.clone())
                    .child(
                        div()
                            .w(px(36.0))
                            .flex_shrink_0()
                            .text_color(muted_fg)
                            .child(format!("{}", idx + 1)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(fmt_line(tl)))
                    .child(div().flex_1().min_w_0().truncate().child(fmt_line(wl)))
                    .child(
                        div()
                            .w(px(110.0))
                            .flex_shrink_0()
                            .truncate()
                            .text_color(dy_col)
                            .child(dy_str),
                    )
            }))
            .into_any_element()
    }
}
