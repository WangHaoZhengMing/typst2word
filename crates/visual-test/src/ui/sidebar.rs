//! Left column: Finder/Xcode-style Collapsible Standard-Library Navigator (`sidebar.rs`).
//!
//! Organizes cases into a clean 3-level hierarchy (`Category -> Suite -> Case`)
//! with single-line leaf items and automatic expansion during search or filtering.

use gpui_kit::{
    AnyElement, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::Input,
        scroll::ScrollableElement as _,
        tab::{Tab, TabBar},
        v_flex,
    },
};

use crate::{
    model::{CaseStatus, Category},
    ui::{
        macos_theme::status_dot_color,
        state::{ParityStudioView, StatusFilter},
    },
};

impl ParityStudioView {
    pub(crate) fn render_sidebar(
        &self,
        filtered: &[usize],
        _window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let sidebar_bg = theme.sidebar;
        let sidebar_fg = theme.sidebar_foreground;
        let border_col = theme.sidebar_border;
        let muted_fg = theme.muted_foreground;
        let list_active_bg = theme.list_active;
        let list_hover_bg = theme.list_hover;
        let radius_md = theme.radius;
        let radius_full = theme.radius_full();
        let mono = theme.mono_font_family.clone();

        let status_idx = match self.status_filter {
            StatusFilter::All => 0,
            StatusFilter::Pass => 1,
            StatusFilter::Drift => 2,
            StatusFilter::Fail => 3,
        };

        let auto_expand = self.has_auto_expand_filter(cx);
        let has_active_filter = self.category_filter.is_some() || auto_expand;

        let mut tree_rows: Vec<AnyElement> = Vec::new();

        for cat in Category::ALL {
            let cat_indices: Vec<usize> = filtered
                .iter()
                .copied()
                .filter(|&idx| self.report.cases[idx].category == cat)
                .collect();
            if cat_indices.is_empty() {
                continue;
            }

            let cat_expanded = auto_expand || !self.collapsed_categories.contains(&cat);
            let cat_fail = cat_indices
                .iter()
                .filter(|&&idx| self.report.cases[idx].status == CaseStatus::Fail)
                .count();
            let cat_pass = cat_indices
                .iter()
                .filter(|&&idx| self.report.cases[idx].status == CaseStatus::Pass)
                .count();
            let cat_count_label = if self.report.word_enabled && cat_pass > 0 {
                format!("{cat_pass}/{}", cat_indices.len())
            } else {
                format!("{}", cat_indices.len())
            };

            // Level 1: Category Folder Header
            tree_rows.push(
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
                                Icon::new(if cat_expanded {
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
                                el.child(
                                    div()
                                        .size_1p5()
                                        .rounded(radius_full)
                                        .bg(cx.theme().danger),
                                )
                            })
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_xs()
                                    .text_color(muted_fg)
                                    .child(cat_count_label),
                            ),
                    )
                    .into_any_element(),
            );

            if !cat_expanded {
                continue;
            }

            // Group cases inside this Category by suite subpath (preserving catalog order)
            let mut suite_groups: Vec<(String, String, Vec<usize>)> = Vec::new();
            for idx in cat_indices {
                let rel = &self.report.cases[idx].suite_rel_path;
                let suite_label = suite_display_name(rel);
                if let Some(last) = suite_groups.last_mut()
                    && last.0 == *rel
                {
                    last.2.push(idx);
                } else {
                    suite_groups.push((rel.clone(), suite_label, vec![idx]));
                }
            }

            for (suite_key, suite_label, group_indices) in suite_groups {
                if group_indices.len() > 1 {
                    let suite_expanded =
                        auto_expand || !self.collapsed_suites.contains(&suite_key);
                    let toggle_key = suite_key.clone();

                    // Level 2: Multi-case Suite / Function Sub-folder
                    tree_rows.push(
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
                                        Icon::new(if suite_expanded {
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
                                            .child(suite_label),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .font_family(mono.clone())
                                    .text_xs()
                                    .text_color(muted_fg)
                                    .child(format!("{}", group_indices.len())),
                            )
                            .into_any_element(),
                    );

                    if suite_expanded {
                        for idx in group_indices {
                            tree_rows.push(self.render_case_leaf_row(
                                idx,
                                true,
                                list_active_bg,
                                list_hover_bg,
                                muted_fg,
                                radius_md,
                                radius_full,
                                &mono,
                                cx,
                            ));
                        }
                    }
                } else if let Some(&idx) = group_indices.first() {
                    tree_rows.push(self.render_case_leaf_row(
                        idx,
                        false,
                        list_active_bg,
                        list_hover_bg,
                        muted_fg,
                        radius_md,
                        radius_full,
                        &mono,
                        cx,
                    ));
                }
            }
        }

        v_flex()
            .size_full()
            .bg(sidebar_bg)
            .text_color(sidebar_fg)
            .overflow_hidden()
            .child(
                // Clean 2-row filter header
                v_flex()
                    .p_2()
                    .gap_1p5()
                    .border_b_1()
                    .border_color(border_col)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_1()
                            .child(
                                div().flex_1().min_w_0().child(
                                    Input::new(&self.search_input)
                                        .id("sidebar-case-search")
                                        .small()
                                        .cleanable(true)
                                        .prefix(
                                            Icon::new(IconName::Search)
                                                .xsmall()
                                                .text_color(muted_fg),
                                        ),
                                ),
                            )
                            .child(
                                Button::new("sidebar-toggle-expand-all")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ChevronsUpDown)
                                    .tooltip("Expand / Collapse All Categories")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_all_categories(cx);
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_1p5()
                            .child(
                                div().flex_1().min_w_0().child(
                                    TabBar::new("sidebar-status-segmented")
                                        .segmented()
                                        .xsmall()
                                        .selected_index(status_idx)
                                        .children([
                                            Tab::new().label("All"),
                                            Tab::new().label("Pass"),
                                            Tab::new().label("Drift"),
                                            Tab::new().label("Fail"),
                                        ])
                                        .on_click(cx.listener(|this, idx, _, cx| {
                                            this.status_filter = match *idx {
                                                1 => StatusFilter::Pass,
                                                2 => StatusFilter::Drift,
                                                3 => StatusFilter::Fail,
                                                _ => StatusFilter::All,
                                            };
                                            cx.notify();
                                        })),
                                ),
                            )
                            .when(has_active_filter, |el| {
                                el.child(
                                    Button::new("sidebar-reset-filters")
                                        .ghost()
                                        .xsmall()
                                        .label("Clear")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.category_filter = None;
                                            this.status_filter = StatusFilter::All;
                                            this.search_input.update(cx, |st, cx| {
                                                st.set_value("", window, cx);
                                            });
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .id("sidebar-case-tree")
                    .flex_1()
                    .min_h_0()
                    .px_1p5()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .overflow_y_scrollbar()
                    .children(tree_rows),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_case_leaf_row(
        &self,
        idx: usize,
        nested_in_suite: bool,
        list_active_bg: gpui_kit::Hsla,
        list_hover_bg: gpui_kit::Hsla,
        muted_fg: gpui_kit::Hsla,
        radius_md: gpui_kit::Pixels,
        radius_full: gpui_kit::Pixels,
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

fn suite_display_name(suite_rel_path: &str) -> String {
    let without_cat = suite_rel_path
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(suite_rel_path);
    without_cat
        .strip_suffix(".typ")
        .unwrap_or(without_cat)
        .to_string()
}
