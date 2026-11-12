//! Core view state, editor buffer synchronization, async runner orchestration,
//! and top-level 3-column macOS layout (`state.rs`).

use std::{collections::HashSet, fs, path::PathBuf, time::Duration};

use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Pixels, Render,
    Styled as _, Subscription, Window, div, px,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _,
        button::Button,
        empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
        input::{EditorState, InputEvent, InputState},
        resizable::{h_resizable, resizable_panel},
        v_flex,
    },
};

use crate::{
    model::{CaseStatus, Category, LabReport},
    runner::{RunConfig, run_suite},
    ui::{
        load_or_placeholder_report,
        typst_highlight::{format_xml_for_display, typst_highlighter_factory},
    },
};

/// Zoom steps for the center visual comparison stage.
pub const ZOOM_STEPS: [f32; 6] = [0.60, 0.80, 1.0, 1.25, 1.50, 2.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Pass,
    Drift,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasMode {
    SideBySide,
    OverlayOnly,
    ThreeUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeTab {
    Typst,
    IrJson,
    Ooxml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsTab {
    Geometry,
    Checks,
    Lines,
}

/// Top-level macOS 3-column inspection studio view:
/// - **Left**: Collapsible stdlib tree navigator (`sidebar.rs`)
/// - **Center**: Generous visual paper canvas + optional diagnostics drawer (`center_stage.rs`, `diagnostics.rs`)
/// - **Right**: Syntax-highlighted GPUI Kit `Editor` inspector (`code_panel.rs`)
pub struct ParityStudioView {
    pub report_dir: PathBuf,
    pub report: LabReport,
    pub search_input: Entity<InputState>,
    pub category_filter: Option<Category>,
    pub status_filter: StatusFilter,
    pub collapsed_categories: HashSet<Category>,
    pub collapsed_suites: HashSet<String>,
    pub selected_case_id: String,
    pub canvas_mode: CanvasMode,
    pub zoom: f32,
    pub code_tab: CodeTab,
    pub diagnostics_tab: DiagnosticsTab,
    pub show_left_sidebar: bool,
    pub show_right_inspector: bool,
    pub show_diagnostics: bool,
    pub word_wrap: bool,
    pub pretty_xml: bool,
    pub is_running: bool,
    pub banner_message: Option<String>,
    typst_editor: Entity<EditorState>,
    ir_editor: Entity<EditorState>,
    xml_editor: Entity<EditorState>,
    raw_xml_cache: String,
    synced_case_id: String,
    _subscriptions: Vec<Subscription>,
}

impl ParityStudioView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, report_dir: PathBuf) -> Self {
        let report = load_or_placeholder_report(&report_dir);

        // Prefer starting on `margin_uniform_10pt` (Layout -> page/margins) if present,
        // otherwise fall back to the first case in the report.
        let initial_case = report
            .cases
            .iter()
            .find(|c| c.id == "margin_uniform_10pt")
            .or_else(|| report.cases.first());

        let selected_case_id = initial_case.map(|c| c.id.clone()).unwrap_or_default();
        let active_cat = initial_case.map(|c| c.category);

        // Collapse non-active categories on startup so all 12 stdlib sections fit cleanly
        // without scrolling.
        let mut collapsed_categories = HashSet::new();
        for cat in Category::ALL {
            if Some(cat) != active_cat {
                collapsed_categories.insert(cat);
            }
        }

        let initial_code_tab = CodeTab::Typst;

        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search function, param, case…")
                .clean_on_escape()
        });

        let typst_editor = cx.new(|cx| {
            let mut st = EditorState::new(window, cx)
                .language("typst")
                .line_number(true)
                .folding(true)
                .soft_wrap(true)
                .scroll_bounce(true);
            st.set_highlighter_factory(typst_highlighter_factory(), cx);
            st
        });

        let ir_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .folding(true)
                .soft_wrap(true)
                .scroll_bounce(true)
        });

        let xml_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("xml")
                .line_number(true)
                .folding(true)
                .soft_wrap(true)
                .scroll_bounce(true)
        });

        let search_sub =
            cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.ensure_filtered_selection(window, cx);
                    cx.notify();
                }
            });

        let mut view = Self {
            report_dir,
            report,
            search_input,
            category_filter: None,
            status_filter: StatusFilter::All,
            collapsed_categories,
            collapsed_suites: HashSet::new(),
            selected_case_id,
            canvas_mode: CanvasMode::SideBySide,
            zoom: 1.0,
            code_tab: initial_code_tab,
            diagnostics_tab: DiagnosticsTab::Geometry,
            show_left_sidebar: true,
            show_right_inspector: true,
            show_diagnostics: false,
            word_wrap: true,
            pretty_xml: true,
            is_running: false,
            banner_message: None,
            typst_editor,
            ir_editor,
            xml_editor,
            raw_xml_cache: String::new(),
            synced_case_id: String::new(),
            _subscriptions: vec![search_sub],
        };
        view.sync_editors_for_selected_case(window, cx);
        view
    }

    pub(crate) fn active_editor_entity(&self) -> &Entity<EditorState> {
        match self.code_tab {
            CodeTab::Typst => &self.typst_editor,
            CodeTab::IrJson => &self.ir_editor,
            CodeTab::Ooxml => &self.xml_editor,
        }
    }

    pub(crate) fn select_case(
        &mut self,
        case_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_case_id == case_id && self.synced_case_id == case_id {
            return;
        }
        self.selected_case_id = case_id.to_string();
        if let Some(cat) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == case_id)
            .map(|c| c.category)
        {
            self.collapsed_categories.remove(&cat);
        }
        self.sync_editors_for_selected_case(window, cx);
        cx.notify();
    }

    pub(crate) fn step_zoom(&mut self, delta: isize, cx: &mut Context<Self>) {
        let cur_idx = ZOOM_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((*a - self.zoom).abs())
                    .partial_cmp(&((*b - self.zoom).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(2);
        let next_idx = (cur_idx as isize + delta).clamp(0, ZOOM_STEPS.len() as isize - 1) as usize;
        self.zoom = ZOOM_STEPS[next_idx];
        cx.notify();
    }

    pub(crate) fn toggle_all_categories(&mut self, cx: &mut Context<Self>) {
        if self.collapsed_categories.len() >= Category::ALL.len() / 2 {
            self.collapsed_categories.clear();
            self.collapsed_suites.clear();
        } else {
            self.collapsed_categories = Category::ALL.into_iter().collect();
        }
        cx.notify();
    }

    pub(crate) fn toggle_word_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.word_wrap = !self.word_wrap;
        let wrap = self.word_wrap;
        for editor in [&self.typst_editor, &self.ir_editor, &self.xml_editor] {
            editor.update(cx, |st, cx| {
                st.set_soft_wrap(wrap, window, cx);
            });
        }
        cx.notify();
    }

    pub(crate) fn toggle_pretty_xml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pretty_xml = !self.pretty_xml;
        let xml_display = if self.pretty_xml {
            format_xml_for_display(&self.raw_xml_cache)
        } else {
            self.raw_xml_cache.clone()
        };
        self.xml_editor.update(cx, |st, cx| {
            st.set_value(xml_display, window, cx);
        });
        cx.notify();
    }

    pub(crate) fn reload_from_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.report = load_or_placeholder_report(&self.report_dir);
        if !self
            .report
            .cases
            .iter()
            .any(|c| c.id == self.selected_case_id)
            && let Some(first) = self.report.cases.first()
        {
            self.selected_case_id = first.id.clone();
        }
        self.synced_case_id.clear();
        self.sync_editors_for_selected_case(window, cx);
    }

    pub(crate) fn trigger_run(
        &mut self,
        only_selected: bool,
        skip_word: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_running {
            return;
        }
        let filter_id = if only_selected && !self.selected_case_id.is_empty() {
            Some(self.selected_case_id.clone())
        } else {
            None
        };
        let out_dir = self.report_dir.clone();
        self.is_running = true;
        let mode_label = if skip_word { "IR & OOXML" } else { "Word PDF" };
        self.banner_message = Some(match &filter_id {
            Some(id) => format!("Running `{id}` ({mode_label})…"),
            None => format!(
                "Running {} cases ({mode_label})…",
                self.report.cases.len()
            ),
        });
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let cfg = RunConfig {
                out_dir,
                category_filter: None,
                id_substring_filter: filter_id,
                skip_word,
                fresh_word: only_selected && !skip_word,
                raster_scale: 2.0,
            };
            let res = cx
                .background_executor()
                .spawn(async move { run_suite(&cfg) })
                .await;
            let _ = cx.background_executor().timer(Duration::from_millis(20)).await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.is_running = false;
                match res {
                    Ok(rep) => {
                        if rep.word_enabled {
                            view.banner_message = Some(format!(
                                "Finished {} case(s) · {} pass · {} drift · {} fail",
                                rep.cases.len(),
                                rep.stats.pass_count,
                                rep.stats.drift_count,
                                rep.stats.fail_count
                            ));
                        } else {
                            let ok = rep.cases.len().saturating_sub(rep.stats.fail_count);
                            view.banner_message = Some(format!(
                                "Verified {ok}/{} case(s) (IR & OOXML) · {} fail",
                                rep.cases.len(),
                                rep.stats.fail_count
                            ));
                        }
                        view.reload_from_disk(window, cx);
                    }
                    Err(err) => {
                        view.banner_message = Some(format!("Run error: {err}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn has_auto_expand_filter(&self, cx: &App) -> bool {
        self.status_filter != StatusFilter::All
            || !self.search_input.read(cx).value().trim().is_empty()
    }

    fn ensure_filtered_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filtered = self.filtered_indices(cx);
        if !filtered.is_empty()
            && !filtered
                .iter()
                .any(|&idx| self.report.cases[idx].id == self.selected_case_id)
        {
            self.selected_case_id = self.report.cases[filtered[0]].id.clone();
            self.sync_editors_for_selected_case(window, cx);
        }
    }

    fn sync_editors_for_selected_case(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(case) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
        else {
            return;
        };

        let typst_src = case.typst_source.clone();
        let ir_src = if case.ir_json_path.is_empty() {
            "// Click 'Verify IR' or 'Run Word' above to generate ir.json\n".to_string()
        } else {
            fs::read_to_string(&case.ir_json_path)
                .unwrap_or_else(|_| "// Click 'Verify IR' to generate ir.json\n".to_string())
        };
        let raw_xml = if case.document_xml_path.is_empty() {
            "<!-- Click 'Verify IR' or 'Run Word' above to generate word/document.xml -->\n"
                .to_string()
        } else {
            fs::read_to_string(&case.document_xml_path).unwrap_or_else(|_| {
                "<!-- Click 'Verify IR' to generate word/document.xml -->\n".to_string()
            })
        };

        let xml_display = if self.pretty_xml {
            format_xml_for_display(&raw_xml)
        } else {
            raw_xml.clone()
        };

        self.synced_case_id = case.id.clone();
        self.raw_xml_cache = raw_xml;

        self.typst_editor.update(cx, |st, cx| {
            st.set_value(typst_src, window, cx);
        });
        self.ir_editor.update(cx, |st, cx| {
            st.set_value(ir_src, window, cx);
        });
        self.xml_editor.update(cx, |st, cx| {
            st.set_value(xml_display, window, cx);
        });
    }

    fn filtered_indices(&self, cx: &App) -> Vec<usize> {
        let query = self.search_input.read(cx).value().trim().to_ascii_lowercase();
        self.report
            .cases
            .iter()
            .enumerate()
            .filter(|(_, c)| self.category_filter.is_none_or(|cat| c.category == cat))
            .filter(|(_, c)| match self.status_filter {
                StatusFilter::All => true,
                StatusFilter::Pass => c.status == CaseStatus::Pass,
                StatusFilter::Drift => c.status == CaseStatus::Drift,
                StatusFilter::Fail => c.status == CaseStatus::Fail,
            })
            .filter(|(_, c)| {
                if query.is_empty() {
                    true
                } else {
                    c.id.to_ascii_lowercase().contains(&query)
                        || c.title.to_ascii_lowercase().contains(&query)
                        || c.isolated_variable.to_ascii_lowercase().contains(&query)
                        || c.suite_rel_path.to_ascii_lowercase().contains(&query)
                        || c.target_func.to_ascii_lowercase().contains(&query)
                        || c.target_param.to_ascii_lowercase().contains(&query)
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn render_empty_filter_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_8()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                Empty::new()
                    .max_w(px(380.0))
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new()
                                    .with_variant(EmptyMediaVariant::Icon)
                                    .child(Icon::new(IconName::Search)),
                            )
                            .title(EmptyTitle::new().child("No Matching Cases"))
                            .description(
                                EmptyDescription::new()
                                    .child("Clear the search query or switch the status filter."),
                            ),
                    )
                    .child(
                        Button::new("empty-reset-filters")
                            .small()
                            .outline()
                            .label("Clear Filters")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.category_filter = None;
                                this.status_filter = StatusFilter::All;
                                this.search_input.update(cx, |st, cx| {
                                    st.set_value("", window, cx);
                                });
                                this.ensure_filtered_selection(window, cx);
                                cx.notify();
                            })),
                    ),
            )
    }
}

impl Render for ParityStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_indices(cx);

        if !filtered.is_empty()
            && !filtered
                .iter()
                .any(|&idx| self.report.cases[idx].id == self.selected_case_id)
        {
            self.selected_case_id = self.report.cases[filtered[0]].id.clone();
        }
        if !filtered.is_empty() && self.synced_case_id != self.selected_case_id {
            self.sync_editors_for_selected_case(window, cx);
        }

        let active_case = if filtered.is_empty() {
            None
        } else {
            self.report
                .cases
                .iter()
                .find(|c| c.id == self.selected_case_id)
                .cloned()
        };

        let bg = cx.theme().background;
        let fg = cx.theme().foreground;
        let muted_fg = cx.theme().muted_foreground;

        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .overflow_hidden()
            .child(self.render_titlebar(window, cx))
            .child(
                // Three-column macOS layout: Left Tree Navigator · Center Paper Stage · Right Code Inspector
                div().flex_1().min_h_0().w_full().child(
                    h_resizable("parity-studio-three-columns")
                        .child(
                            resizable_panel()
                                .visible(self.show_left_sidebar)
                                .size(px(264.0))
                                .size_range(px(210.0)..px(380.0))
                                .fixed()
                                .child(self.render_sidebar(&filtered, window, cx)),
                        )
                        .child(
                            resizable_panel()
                                .size_range(px(420.0)..Pixels::MAX)
                                .child(if let Some(ref case) = active_case {
                                    self.render_center_column(case, window, cx)
                                        .into_any_element()
                                } else {
                                    self.render_empty_filter_state(cx).into_any_element()
                                }),
                        )
                        .child(
                            resizable_panel()
                                .visible(self.show_right_inspector)
                                .size(px(440.0))
                                .size_range(px(300.0)..px(820.0))
                                .fixed()
                                .child(if let Some(ref case) = active_case {
                                    self.render_code_panel(case, window, cx)
                                        .into_any_element()
                                } else {
                                    v_flex()
                                        .size_full()
                                        .items_center()
                                        .justify_center()
                                        .bg(bg)
                                        .text_xs()
                                        .text_color(muted_fg)
                                        .child("Select a case to inspect source & XML")
                                        .into_any_element()
                                }),
                        ),
                ),
            )
            .child(self.render_status_bar(cx))
    }
}
