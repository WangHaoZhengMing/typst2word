//! Left column: Finder / Xcode Source-List Navigator (`sidebar/mod.rs`).
//!
//! Uses a compact **single-row** search + pop-up filter bar (`[ 🔍 Search… ] [ Filter ▾ ]`)
//! instead of stacked segmented rows so the entire vertical height belongs to the stdlib tree.

mod tree_row;

use gpui_kit::{
    Anchor, AnyElement, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    Window,
    assets::IconName as Lucide,
    component::{
        ActiveTheme as _, Icon, IconName, Selectable as _, Side, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
        scroll::ScrollableElement as _,
        v_flex,
    },
    div, px,
};

use crate::{
    model::Category,
    ui::state::{ParityStudioView, StatusFilter},
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

        let auto_expand = self.has_auto_expand_filter(cx);
        let has_non_search_filter =
            self.status_filter != StatusFilter::All || self.category_filter.is_some();

        let filter_btn_label = match (self.status_filter, self.category_filter) {
            (StatusFilter::All, None) => None,
            (st, None) => Some(st.short_label().to_string()),
            (StatusFilter::All, Some(cat)) => Some(cat.slug().to_string()),
            (st, Some(_)) => Some(format!("{}+1", st.short_label())),
        };

        let cur_status = self.status_filter;
        let cur_cat = self.category_filter;
        let s = &self.report.stats;
        let total_n = s.total_cases;
        let pass_n = s.pass_count;
        let drift_n = s.drift_count;
        let fail_n = s.fail_count;
        let ir_n = s.ir_only_count;
        let view_for_filter = cx.entity();

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
            tree_rows.push(self.render_category_header_row(
                cat,
                &cat_indices,
                cat_expanded,
                list_hover_bg,
                muted_fg,
                radius_md,
                radius_full,
                &mono,
                cx,
            ));

            if !cat_expanded {
                continue;
            }

            let mut suite_groups: Vec<(String, String, Vec<usize>)> = Vec::new();
            for idx in cat_indices {
                let rel = &self.report.cases[idx].suite_rel_path;
                let suite_label = tree_row::suite_display_name(rel);
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
                    tree_rows.push(self.render_suite_folder_row(
                        &suite_key,
                        &suite_label,
                        group_indices.len(),
                        suite_expanded,
                        list_hover_bg,
                        muted_fg,
                        radius_md,
                        &mono,
                        cx,
                    ));

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

        let mut filter_btn = Button::new("sidebar-filter-dropdown")
            .ghost()
            .xsmall()
            .selected(has_non_search_filter)
            .icon(Lucide::ListFilter)
            .dropdown_caret(true)
            .tooltip("Filter by Status, Category, or Expand/Collapse Folders");
        if let Some(lbl) = filter_btn_label {
            filter_btn = filter_btn.label(lbl);
        }

        v_flex()
            .size_full()
            .bg(sidebar_bg)
            .text_color(sidebar_fg)
            .overflow_hidden()
            .child(
                // Single-row native macOS Source List search + filter bar (38px aligned with Center & Right headers)
                h_flex()
                    .w_full()
                    .h(px(38.0))
                    .px_2()
                    .gap_1()
                    .items_center()
                    .border_b_1()
                    .border_color(border_col)
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.search_input)
                                .id("sidebar-case-search")
                                .xsmall()
                                .cleanable(true)
                                .prefix(
                                    Icon::new(IconName::Search)
                                        .xsmall()
                                        .text_color(muted_fg),
                                ),
                        ),
                    )
                    .child(filter_btn.dropdown_menu_with_anchor(
                        Anchor::TopRight,
                        move |mut menu, window, _cx| {
                            let vs0 = view_for_filter.clone();
                            let vs1 = view_for_filter.clone();
                            let vs2 = view_for_filter.clone();
                            let vs3 = view_for_filter.clone();
                            let vs4 = view_for_filter.clone();
                            let vc_all = view_for_filter.clone();
                            let v_exp = view_for_filter.clone();
                            let v_col = view_for_filter.clone();
                            let v_rst = view_for_filter.clone();

                            menu = menu
                                .min_w(px(225.0))
                                .max_h(px(460.0))
                                .scrollable(true)
                                .check_side(Side::Left)
                                .label("Status")
                                .item(
                                    PopupMenuItem::new(format!("All Statuses ({total_n})"))
                                        .checked(cur_status == StatusFilter::All)
                                        .on_click(window.listener_for(
                                            &vs0,
                                            |this, _, window, cx| {
                                                this.status_filter = StatusFilter::All;
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                )
                                .item(
                                    PopupMenuItem::new(format!("Pass ({pass_n})"))
                                        .checked(cur_status == StatusFilter::Pass)
                                        .on_click(window.listener_for(
                                            &vs1,
                                            |this, _, window, cx| {
                                                this.status_filter = StatusFilter::Pass;
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                )
                                .item(
                                    PopupMenuItem::new(format!("Drift ({drift_n})"))
                                        .checked(cur_status == StatusFilter::Drift)
                                        .on_click(window.listener_for(
                                            &vs2,
                                            |this, _, window, cx| {
                                                this.status_filter = StatusFilter::Drift;
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                )
                                .item(
                                    PopupMenuItem::new(format!("Fail ({fail_n})"))
                                        .checked(cur_status == StatusFilter::Fail)
                                        .on_click(window.listener_for(
                                            &vs3,
                                            |this, _, window, cx| {
                                                this.status_filter = StatusFilter::Fail;
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                )
                                .item(
                                    PopupMenuItem::new(format!("IR Only ({ir_n})"))
                                        .checked(cur_status == StatusFilter::IrOnly)
                                        .on_click(window.listener_for(
                                            &vs4,
                                            |this, _, window, cx| {
                                                this.status_filter = StatusFilter::IrOnly;
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                )
                                .separator()
                                .label("Stdlib Category")
                                .item(
                                    PopupMenuItem::new("All Categories")
                                        .checked(cur_cat.is_none())
                                        .on_click(window.listener_for(
                                            &vc_all,
                                            |this, _, window, cx| {
                                                this.category_filter = None;
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                );

                            for cat in Category::ALL {
                                let v_cat = view_for_filter.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(cat.title())
                                        .checked(cur_cat == Some(cat))
                                        .on_click(window.listener_for(
                                            &v_cat,
                                            move |this, _, window, cx| {
                                                this.category_filter = Some(cat);
                                                this.collapsed_categories.remove(&cat);
                                                this.ensure_filtered_selection(window, cx);
                                                cx.notify();
                                            },
                                        )),
                                );
                            }

                            menu.separator()
                                .label("Tree Layout")
                                .item(PopupMenuItem::new("Expand All Folders").on_click(
                                    window.listener_for(&v_exp, |this, _, _, cx| {
                                        this.expand_all_folders(cx);
                                    }),
                                ))
                                .item(PopupMenuItem::new("Collapse All Folders").on_click(
                                    window.listener_for(&v_col, |this, _, _, cx| {
                                        this.collapse_all_folders(cx);
                                    }),
                                ))
                                .separator()
                                .item(
                                    PopupMenuItem::new("Reset All Filters")
                                        .disabled(!has_non_search_filter)
                                        .on_click(window.listener_for(
                                            &v_rst,
                                            |this, _, window, cx| {
                                                this.reset_all_filters(window, cx);
                                            },
                                        )),
                                )
                        },
                    )),
            )
            .child(
                div()
                    .id("sidebar-case-tree")
                    .flex_1()
                    .min_h_0()
                    .px_1p5()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .overflow_y_scrollbar()
                    .children(tree_rows),
            )
    }
}
