//! Finder / Xcode source-list folder and case row elements (`sidebar/tree_row.rs`).

use gpui_kit::{
    AnyElement, FontWeight, Hsla, InteractiveElement as _, IntoElement as _, ParentElement as _,
    Pixels, SharedString, StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Lucide,
    component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex},
    div,
    prelude::FluentBuilder as _,
};

use crate::{
    model::{CaseStatus, Category},
    ui::{macos_theme::status_dot_color, state::ParityStudioView},
};

impl ParityStudioView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_category_header_row(
        &self,
        cat: Category,
        cat_indices: &[usize],
        expanded: bool,
        list_hover_bg: Hsla,
        muted_fg: Hsla,
        radius_md: Pixels,
        radius_full: Pixels,
        mono: &SharedString,
        cx: &mut gpui_kit::Context<Self>,
    ) -> AnyElement {
        let cat_fail = cat_indices
            .iter()
            .filter(|&&idx| self.report.cases[idx].status == CaseStatus::Fail)
            .count();
        let cat_pass = cat_indices
            .iter()
            .filter(|&&idx| self.report.cases[idx].status == CaseStatus::Pass)
            .count();
        let count_label = if self.report.word_enabled && cat_pass > 0 {
            format!("{cat_pass}/{}", cat_indices.len())
        } else {
            format!("{}", cat_indices.len())
        };

        h_flex()
            .id(SharedString::from(format!("cat-hdr-{}", cat.slug())))
            .w_full()
            .px_2()
            .py_1()
            .mt_0p5()
            .rounded(radius_md)
            .items_center()
            .justify_between()
            .gap_1p5()
            .cursor_pointer()
            .hover(move |style| style.bg(list_hover_bg))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.collapsed_categories.remove(&cat) {
                    this.collapsed_categories.insert(cat);
                }
                cx.notify();
            }))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(if expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall()
                        .text_color(muted_fg),
                    )
                    .child(
                        Icon::new(if expanded {
                            Lucide::FolderOpen
                        } else {
                            Lucide::Folder
                        })
                        .xsmall()
                        .text_color(cx.theme().accent_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(cat.title()),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1p5()
                    .when(cat_fail > 0, |el| {
                        el.child(div().size_1p5().rounded(radius_full).bg(cx.theme().danger))
                    })
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_xs()
                            .text_color(muted_fg)
                            .child(count_label),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_suite_folder_row(
        &self,
        suite_key: &str,
        suite_label: &str,
        group_len: usize,
        expanded: bool,
        list_hover_bg: Hsla,
        muted_fg: Hsla,
        radius_md: Pixels,
        mono: &SharedString,
        cx: &mut gpui_kit::Context<Self>,
    ) -> AnyElement {
        let toggle_key = suite_key.to_string();

        h_flex()
            .id(SharedString::from(format!("suite-hdr-{suite_key}")))
            .w_full()
            .pl_4()
            .pr_2()
            .py_1()
            .rounded(radius_md)
            .items_center()
            .justify_between()
            .gap_1p5()
            .cursor_pointer()
            .hover(move |style| style.bg(list_hover_bg))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.collapsed_suites.remove(&toggle_key) {
                    this.collapsed_suites.insert(toggle_key.clone());
                }
                cx.notify();
            }))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(if expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall()
                        .text_color(muted_fg),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(mono.clone())
                            .text_xs()
                            .text_color(muted_fg)
                            .child(suite_label.to_string()),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .font_family(mono.clone())
                    .text_xs()
                    .text_color(muted_fg)
                    .child(format!("{group_len}")),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_case_leaf_row(
        &self,
        idx: usize,
        nested_in_suite: bool,
        list_active_bg: Hsla,
        list_hover_bg: Hsla,
        muted_fg: Hsla,
        radius_md: Pixels,
        radius_full: Pixels,
        mono: &SharedString,
        cx: &mut gpui_kit::Context<Self>,
    ) -> AnyElement {
        let c = &self.report.cases[idx];
        let case_id = c.id.clone();
        let is_selected = case_id == self.selected_case_id;
        let dot_col = status_dot_color(c.status, cx);
        let iou_badge = c
            .comparison
            .as_ref()
            .map(|cmp| format!("{:.0}%", cmp.aligned_iou.max(cmp.raw_iou) * 100.0));

        h_flex()
            .id(SharedString::from(format!("case-item-{}", c.id)))
            .w_full()
            .when(nested_in_suite, |el| el.pl_7())
            .when(!nested_in_suite, |el| el.pl_5())
            .pr_2()
            .py_1()
            .rounded(radius_md)
            .items_center()
            .justify_between()
            .gap_2()
            .cursor_pointer()
            .when(is_selected, |el| el.bg(list_active_bg))
            .when(!is_selected, |el| {
                el.hover(move |style| style.bg(list_hover_bg))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_case(&case_id, window, cx);
            }))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size_1p5()
                            .flex_shrink_0()
                            .rounded(radius_full)
                            .bg(dot_col),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .font_weight(if is_selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(c.title.clone()),
                    ),
            )
            .when_some(iou_badge, |el, metric| {
                el.child(
                    div()
                        .flex_shrink_0()
                        .font_family(mono.clone())
                        .text_xs()
                        .text_color(muted_fg)
                        .child(metric),
                )
            })
            .into_any_element()
    }
}

pub(super) fn suite_display_name(suite_rel_path: &str) -> String {
    let without_cat = suite_rel_path
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(suite_rel_path);
    without_cat
        .strip_suffix(".typ")
        .unwrap_or(without_cat)
        .to_string()
}
