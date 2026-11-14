//! Core view state, enums, keyboard action handlers, and top-level 3-column macOS layout.

mod navigation;
mod runner_bridge;

use std::{collections::HashSet, path::PathBuf};

use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, Styled as _, Subscription, Window, div, px,
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
    model::{Category, LabReport},
    ui::{
        actions::{
            FindInEditor, FocusCaseSearch, NextCase, OpenDocxInWord, PrevCase, ReloadReport,
            RevealCaseInFinder, RunActiveTarget, RunCurrentSuiteWithWord, RunSelectedWithWord,
            ShowDocBodyXml, ShowDocIr, ShowDocSettingsXml, ShowDocTypst, ToggleAppearance,
            ToggleBottomDrawer, ToggleLeftSidebar, ToggleRightInspector, VerifyAllIr, ZoomIn,
            ZoomOut, ZoomReset,
        },
        docx_parts::InspectorDoc,
        load_or_placeholder_report,
        macos_theme::toggle_theme_mode,
        typst_highlight::typst_highlighter_factory,
    },
};

/// Zoom steps for the center visual comparison stage.
pub const ZOOM_STEPS: [f32; 6] = [0.60, 0.80, 1.0, 1.25, 1.50, 2.0];

/// Target scope for the unified Xcode-style Run Scheme selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunScope {
    SelectedCase,
    CurrentSuite,
    CurrentCategory,
    FullSuite,
}

/// Execution engine for the unified Run Scheme selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEngine {
    /// Silent macOS sandbox Word + Multi-Section Mega-Doc + PDFKit slicing + pixel IoU diff.
    SilentWordBatch,
    /// Fast in-process Typst + IR + OpenXML verification (skips Microsoft Word).
    IrOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Pass,
    Drift,
    Fail,
    IrOnly,
}

impl StatusFilter {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Pass => "Pass",
            Self::Drift => "Drift",
            Self::Fail => "Fail",
            Self::IrOnly => "IR Only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasMode {
    SideBySide,
    OverlayOnly,
    ThreeUp,
    TypstOnly,
    WordOnly,
}

impl CanvasMode {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::SideBySide => "Side-by-Side",
            Self::OverlayOnly => "Pixel Overlay",
            Self::ThreeUp => "3-Up Sheet",
            Self::TypstOnly => "Typst Only",
            Self::WordOnly => "Word Only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsTab {
    Geometry,
    Checks,
    Lines,
}

impl DiagnosticsTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Geometry => "Geometry Telemetry",
            Self::Checks => "IR & OOXML Checks",
            Self::Lines => "PDFKit Text Lines",
        }
    }
}

/// Top-level macOS 3-column inspection studio view:
/// - **Left**: Finder/Xcode-style stdlib navigator (`sidebar/`)
/// - **Center**: Preview-style visual paper stage + collapsible telemetry drawer (`stage/`, `diagnostics/`)
/// - **Right**: Syntax-highlighted `.typ` / `ir.json` / `.docx` XML inspector (`inspector/`)
pub struct ParityStudioView {
    pub(crate) focus_handle: FocusHandle,
    pub report_dir: PathBuf,
    pub report: LabReport,
    pub search_input: Entity<InputState>,
    pub category_filter: Option<Category>,
    pub status_filter: StatusFilter,
    pub collapsed_categories: HashSet<Category>,
    pub collapsed_suites: HashSet<String>,
    pub selected_case_id: String,
    pub run_scope: RunScope,
    pub run_engine: RunEngine,
    pub force_fresh_word: bool,
    pub canvas_mode: CanvasMode,
    pub zoom: f32,
    pub inspector_doc: InspectorDoc,
    pub diagnostics_tab: DiagnosticsTab,
    pub show_left_sidebar: bool,
    pub show_right_inspector: bool,
    pub show_diagnostics: bool,
    pub word_wrap: bool,
    pub pretty_xml: bool,
    pub is_running: bool,
    pub banner_message: Option<String>,
    pub(crate) typst_editor: Entity<EditorState>,
    pub(crate) ir_editor: Entity<EditorState>,
    pub(crate) xml_editor: Entity<EditorState>,
    pub(crate) available_docx_entries: HashSet<String>,
    pub(crate) raw_xml_cache: String,
    pub(crate) synced_case_id: String,
    pub(crate) synced_xml_doc: InspectorDoc,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for ParityStudioView {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ParityStudioView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, report_dir: PathBuf) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        let report = load_or_placeholder_report(&report_dir);

        let initial_case = report
            .cases
            .iter()
            .find(|c| c.id == "margin_uniform_10pt")
            .or_else(|| report.cases.first());

        let selected_case_id = initial_case.map(|c| c.id.clone()).unwrap_or_default();
        let active_cat = initial_case.map(|c| c.category);

        let mut collapsed_categories = HashSet::new();
        for cat in Category::ALL {
            if Some(cat) != active_cat {
                collapsed_categories.insert(cat);
            }
        }

        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Filter functions, params, cases…")
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
            focus_handle,
            report_dir,
            report,
            search_input,
            category_filter: None,
            status_filter: StatusFilter::All,
            collapsed_categories,
            collapsed_suites: HashSet::new(),
            selected_case_id,
            run_scope: RunScope::SelectedCase,
            run_engine: RunEngine::SilentWordBatch,
            force_fresh_word: false,
            canvas_mode: CanvasMode::SideBySide,
            zoom: 1.0,
            inspector_doc: InspectorDoc::TypstSource,
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
            available_docx_entries: HashSet::new(),
            raw_xml_cache: String::new(),
            synced_case_id: String::new(),
            synced_xml_doc: InspectorDoc::DocumentXml,
            _subscriptions: vec![search_sub],
        };
        view.sync_editors_for_selected_case(window, cx);
        view
    }

    fn render_empty_filter_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_8()
            .items_center()
            .justify_center()
            .bg(cx.theme().muted)
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
                                    .child("Clear the search query or reset the Filter menu."),
                            ),
                    )
                    .child(
                        Button::new("empty-reset-filters")
                            .small()
                            .outline()
                            .label("Reset Filters")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_all_filters(window, cx);
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
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &RunActiveTarget, window, cx| {
                this.trigger_active_scheme(window, cx);
            }))
            .on_action(cx.listener(|this, _: &RunSelectedWithWord, window, cx| {
                this.trigger_run_with(RunScope::SelectedCase, RunEngine::SilentWordBatch, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &RunCurrentSuiteWithWord, window, cx| {
                this.trigger_run_with(
                    RunScope::CurrentSuite,
                    RunEngine::SilentWordBatch,
                    this.force_fresh_word,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &VerifyAllIr, window, cx| {
                this.trigger_run_with(RunScope::FullSuite, RunEngine::IrOnly, false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReloadReport, window, cx| {
                this.reload_from_disk(window, cx);
                this.banner_message = Some("Reloaded report.json from disk".to_string());
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusCaseSearch, window, cx| {
                this.show_left_sidebar = true;
                this.search_input.update(cx, |st, cx| st.focus(window, cx));
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FindInEditor, window, cx| {
                this.show_right_inspector = true;
                let ed = this.active_editor_entity().clone();
                ed.update(cx, |st, cx| {
                    st.focus(window, cx);
                    st.open_search(false, cx);
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PrevCase, window, cx| {
                this.select_adjacent_case(-1, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextCase, window, cx| {
                this.select_adjacent_case(1, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| {
                this.step_zoom(1, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| {
                this.step_zoom(-1, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| {
                this.zoom = 1.0;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleLeftSidebar, _, cx| {
                this.show_left_sidebar = !this.show_left_sidebar;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleRightInspector, _, cx| {
                this.show_right_inspector = !this.show_right_inspector;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleBottomDrawer, _, cx| {
                this.show_diagnostics = !this.show_diagnostics;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ToggleAppearance, window, cx| {
                toggle_theme_mode(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowDocTypst, window, cx| {
                this.select_inspector_doc(InspectorDoc::TypstSource, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowDocIr, window, cx| {
                this.select_inspector_doc(InspectorDoc::IrJson, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowDocBodyXml, window, cx| {
                this.select_inspector_doc(InspectorDoc::DocumentXml, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowDocSettingsXml, window, cx| {
                this.select_inspector_doc(InspectorDoc::SettingsXml, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenDocxInWord, _, cx| {
                this.open_selected_docx_in_word(cx);
            }))
            .on_action(cx.listener(|this, _: &RevealCaseInFinder, _, cx| {
                this.reveal_selected_case_in_finder(cx);
            }))
            .size_full()
            .bg(bg)
            .text_color(fg)
            .overflow_hidden()
            .child(self.render_toolbar(active_case.as_ref(), window, cx))
            .child(
                div().flex_1().min_h_0().w_full().child(
                    h_resizable("parity-studio-three-columns")
                        .child(
                            resizable_panel()
                                .visible(self.show_left_sidebar)
                                .size(px(256.0))
                                .size_range(px(208.0)..px(380.0))
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
                                .size_range(px(290.0)..px(820.0))
                                .fixed()
                                .child(if let Some(ref case) = active_case {
                                    self.render_inspector_panel(case, window, cx)
                                        .into_any_element()
                                } else {
                                    v_flex()
                                        .size_full()
                                        .items_center()
                                        .justify_center()
                                        .bg(bg)
                                        .text_xs()
                                        .text_color(muted_fg)
                                        .child("Select a case to inspect source & OpenXML")
                                        .into_any_element()
                                }),
                        ),
                ),
            )
            .child(self.render_status_bar(&filtered, cx))
    }
}
