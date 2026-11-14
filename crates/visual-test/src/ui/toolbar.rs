//! Unified macOS Xcode-style `TitleBar` (`toolbar.rs`).
//!
//! Consolidates scattered action buttons into compact, native macOS pop-up menus:
//! - **Left**: Navigator toggle + clickable Suite Health Pill (quick filter dropdown)
//! - **Center**: Xcode-style Run Scheme Bar (`[ ▶ Run ]` + `[ Target · Engine ▾ ]` dropdown)
//! - **Right**: `[ Open ▾ ]` desktop integration menu + Pane toggles + Light/Dark toggle

use std::path::Path;

use gpui_kit::{
    Anchor, FontWeight, IntoElement, ParentElement as _, Styled as _, Window,
    assets::IconName as Lucide,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Selectable as _, Side, Sizable as _,
        TitleBar,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    div, px,
};

use crate::{
    model::CaseReport,
    ui::{
        macos_theme::toggle_theme_mode,
        state::{ParityStudioView, RunEngine, RunScope, StatusFilter},
    },
};

impl ParityStudioView {
    pub(crate) fn render_toolbar(
        &self,
        active_case: Option<&CaseReport>,
        _window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let s = &self.report.stats;
        let is_dark = cx.theme().is_dark();
        let border_col = cx.theme().border;
        let secondary_bg = cx.theme().secondary;
        let success_col = cx.theme().success;
        let warning_col = cx.theme().warning;
        let danger_col = cx.theme().danger;
        let info_col = cx.theme().info;

        let (summary_dot, summary_label) = if s.fail_count > 0 {
            (
                danger_col,
                format!("{} fail · {} drift", s.fail_count, s.drift_count),
            )
        } else if s.drift_count > 0 {
            (
                warning_col,
                format!("{} pass · {} drift", s.pass_count, s.drift_count),
            )
        } else if s.pass_count > 0 {
            (success_col, format!("{}/{} pass", s.pass_count, s.total_cases))
        } else {
            (info_col, format!("{} cases", s.total_cases))
        };

        // Compose Xcode-style Scheme label for the Run Pop-Up trigger.
        let scope_short = match self.run_scope {
            RunScope::SelectedCase => active_case
                .map(|c| c.id.clone())
                .unwrap_or_else(|| "Selected Case".to_string()),
            RunScope::CurrentSuite => active_case
                .map(|c| {
                    c.suite_rel_path
                        .split_once('/')
                        .map(|(_, rest)| rest.strip_suffix(".typ").unwrap_or(rest).to_string())
                        .unwrap_or_else(|| c.suite_rel_path.clone())
                })
                .unwrap_or_else(|| "Current Suite".to_string()),
            RunScope::CurrentCategory => active_case
                .map(|c| c.category.title().to_string())
                .unwrap_or_else(|| "Category".to_string()),
            RunScope::FullSuite => format!("All {} Cases", s.total_cases),
        };
        let engine_short = match self.run_engine {
            RunEngine::SilentWordBatch => "Word PDF",
            RunEngine::IrOnly => "IR Only",
        };
        let scheme_trigger_label = format!("{scope_short}  ·  {engine_short}");

        let suite_name_for_menu = active_case
            .map(|c| {
                let n = self
                    .report
                    .cases
                    .iter()
                    .filter(|x| x.suite_rel_path == c.suite_rel_path)
                    .count();
                format!("Current Suite ({} · {n} cases)", c.suite_rel_path)
            })
            .unwrap_or_else(|| "Current Suite".to_string());

        let cat_name_for_menu = active_case
            .map(|c| {
                let n = self
                    .report
                    .cases
                    .iter()
                    .filter(|x| x.category == c.category)
                    .count();
                format!("Current Category ({} · {n} cases)", c.category.title())
            })
            .unwrap_or_else(|| "Current Category".to_string());

        let case_id_for_menu = active_case
            .map(|c| format!("Selected Case ({})", c.id))
            .unwrap_or_else(|| "Selected Case".to_string());

        let total_cases = s.total_cases;
        let cur_scope = self.run_scope;
        let cur_engine = self.run_engine;
        let cur_fresh = self.force_fresh_word;
        let cur_status_filter = self.status_filter;
        let pass_n = s.pass_count;
        let drift_n = s.drift_count;
        let fail_n = s.fail_count;
        let ir_n = s.ir_only_count;

        let has_docx = active_case.is_some_and(|c| !c.docx_path.is_empty() && Path::new(&c.docx_path).exists());
        let has_typst_pdf = active_case.is_some_and(|c| !c.typst_pdf_path.is_empty() && Path::new(&c.typst_pdf_path).exists());
        let has_word_pdf = active_case
            .and_then(|c| c.word_pdf_path.as_deref())
            .is_some_and(|p| !p.is_empty() && Path::new(p).exists());
        let has_case_dir = active_case.is_some_and(|c| !c.case_dir.is_empty() && Path::new(&c.case_dir).exists());

        let view_for_health = cx.entity();
        let view_for_scheme = cx.entity();
        let view_for_open = cx.entity();

        TitleBar::new().child(
            h_flex()
                .w_full()
                .h_full()
                .pr_3()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    // Left cluster: Sidebar toggle + Title + Quick Status Filter Pill
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            Button::new("toolbar-toggle-sidebar")
                                .ghost()
                                .xsmall()
                                .selected(self.show_left_sidebar)
                                .icon(if self.show_left_sidebar {
                                    IconName::PanelLeftClose
                                } else {
                                    IconName::PanelLeftOpen
                                })
                                .tooltip("Toggle Stdlib Navigator (⌥⌘S)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_left_sidebar = !this.show_left_sidebar;
                                    cx.notify();
                                })),
                        )
                        .child(div().h_3p5().w_px().bg(border_col))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Visual Test"),
                        )
                        .child(
                            Button::new("toolbar-health-dropdown")
                                .ghost()
                                .xsmall()
                                .dropdown_caret(true)
                                .label(summary_label)
                                .icon(
                                    gpui_kit::component::Icon::new(Lucide::Activity)
                                        .xsmall()
                                        .text_color(summary_dot),
                                )
                                .tooltip("Quick Filter by Suite Status")
                                .dropdown_menu(move |menu, window, _cx| {
                                    let v1 = view_for_health.clone();
                                    let v2 = view_for_health.clone();
                                    let v3 = view_for_health.clone();
                                    let v4 = view_for_health.clone();
                                    let v5 = view_for_health.clone();
                                    menu.min_w(px(210.0))
                                        .check_side(Side::Left)
                                        .label("Filter Navigator by Status")
                                        .item(
                                            PopupMenuItem::new(format!("All Cases ({total_cases})"))
                                                .checked(cur_status_filter == StatusFilter::All)
                                                .on_click(window.listener_for(
                                                    &v1,
                                                    |this, _, window, cx| {
                                                        this.status_filter = StatusFilter::All;
                                                        this.ensure_filtered_selection(window, cx);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(format!("Pass ({pass_n})"))
                                                .checked(cur_status_filter == StatusFilter::Pass)
                                                .on_click(window.listener_for(
                                                    &v2,
                                                    |this, _, window, cx| {
                                                        this.status_filter = StatusFilter::Pass;
                                                        this.ensure_filtered_selection(window, cx);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(format!("Drift ({drift_n})"))
                                                .checked(cur_status_filter == StatusFilter::Drift)
                                                .on_click(window.listener_for(
                                                    &v3,
                                                    |this, _, window, cx| {
                                                        this.status_filter = StatusFilter::Drift;
                                                        this.ensure_filtered_selection(window, cx);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(format!("Fail ({fail_n})"))
                                                .checked(cur_status_filter == StatusFilter::Fail)
                                                .on_click(window.listener_for(
                                                    &v4,
                                                    |this, _, window, cx| {
                                                        this.status_filter = StatusFilter::Fail;
                                                        this.ensure_filtered_selection(window, cx);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(format!("IR Only ({ir_n})"))
                                                .checked(cur_status_filter == StatusFilter::IrOnly)
                                                .on_click(window.listener_for(
                                                    &v5,
                                                    |this, _, window, cx| {
                                                        this.status_filter = StatusFilter::IrOnly;
                                                        this.ensure_filtered_selection(window, cx);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                }),
                        ),
                )
                .child(
                    // Center cluster: Native Xcode-style Run Split Control
                    h_flex()
                        .items_center()
                        .gap_1()
                        .px_1()
                        .py_0p5()
                        .rounded(cx.theme().radius)
                        .bg(secondary_bg)
                        .border_1()
                        .border_color(border_col)
                        .child(
                            Button::new("toolbar-btn-run-scheme")
                                .primary()
                                .xsmall()
                                .icon(IconName::Play)
                                .loading(self.is_running)
                                .disabled(self.is_running)
                                .label(if self.is_running { "Running…" } else { "Run" })
                                .tooltip("Run Active Scheme Silently (⌘R)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.trigger_active_scheme(window, cx);
                                })),
                        )
                        .child(
                            Button::new("toolbar-scheme-dropdown")
                                .ghost()
                                .xsmall()
                                .disabled(self.is_running)
                                .dropdown_caret(true)
                                .label(scheme_trigger_label)
                                .tooltip("Choose Run Target Scope & Engine")
                                .dropdown_menu(move |menu, window, _cx| {
                                    let v_s1 = view_for_scheme.clone();
                                    let v_s2 = view_for_scheme.clone();
                                    let v_s3 = view_for_scheme.clone();
                                    let v_s4 = view_for_scheme.clone();
                                    let v_e1 = view_for_scheme.clone();
                                    let v_e2 = view_for_scheme.clone();
                                    let v_fr = view_for_scheme.clone();
                                    let v_q1 = view_for_scheme.clone();
                                    let v_q2 = view_for_scheme.clone();
                                    let v_q3 = view_for_scheme.clone();

                                    menu.min_w(px(300.0))
                                        .check_side(Side::Left)
                                        .label("Run Target Scope")
                                        .item(
                                            PopupMenuItem::new(case_id_for_menu.clone())
                                                .checked(cur_scope == RunScope::SelectedCase)
                                                .on_click(window.listener_for(
                                                    &v_s1,
                                                    |this, _, _, cx| {
                                                        this.run_scope = RunScope::SelectedCase;
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(suite_name_for_menu.clone())
                                                .checked(cur_scope == RunScope::CurrentSuite)
                                                .on_click(window.listener_for(
                                                    &v_s2,
                                                    |this, _, _, cx| {
                                                        this.run_scope = RunScope::CurrentSuite;
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(cat_name_for_menu.clone())
                                                .checked(cur_scope == RunScope::CurrentCategory)
                                                .on_click(window.listener_for(
                                                    &v_s3,
                                                    |this, _, _, cx| {
                                                        this.run_scope = RunScope::CurrentCategory;
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new(format!(
                                                "Full Catalog ({total_cases} cases)"
                                            ))
                                            .checked(cur_scope == RunScope::FullSuite)
                                            .on_click(window.listener_for(
                                                &v_s4,
                                                |this, _, _, cx| {
                                                    this.run_scope = RunScope::FullSuite;
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                        .separator()
                                        .label("Pipeline Engine")
                                        .item(
                                            PopupMenuItem::new(
                                                "Silent Word Batch + Pixel Diff",
                                            )
                                            .checked(cur_engine == RunEngine::SilentWordBatch)
                                            .on_click(window.listener_for(
                                                &v_e1,
                                                |this, _, _, cx| {
                                                    this.run_engine = RunEngine::SilentWordBatch;
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                        .item(
                                            PopupMenuItem::new(
                                                "Fast IR & OpenXML Only (Skip Word)",
                                            )
                                            .checked(cur_engine == RunEngine::IrOnly)
                                            .on_click(window.listener_for(
                                                &v_e2,
                                                |this, _, _, cx| {
                                                    this.run_engine = RunEngine::IrOnly;
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                        .separator()
                                        .item(
                                            PopupMenuItem::new(
                                                "Force Fresh Word Export (Bypass Cache)",
                                            )
                                            .checked(cur_fresh)
                                            .on_click(window.listener_for(
                                                &v_fr,
                                                |this, _, _, cx| {
                                                    this.force_fresh_word = !this.force_fresh_word;
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                        .separator()
                                        .label("Quick One-Shot Actions")
                                        .item(
                                            PopupMenuItem::new(
                                                "Run Current .typ Suite in Word Now   ⌥⌘R",
                                            )
                                            .on_click(window.listener_for(
                                                &v_q1,
                                                |this, _, window, cx| {
                                                    this.trigger_run_with(
                                                        RunScope::CurrentSuite,
                                                        RunEngine::SilentWordBatch,
                                                        this.force_fresh_word,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )),
                                        )
                                        .item(
                                            PopupMenuItem::new(
                                                "Run Full Catalog in Word Now (Silent Batch)",
                                            )
                                            .on_click(window.listener_for(
                                                &v_q2,
                                                |this, _, window, cx| {
                                                    this.trigger_run_with(
                                                        RunScope::FullSuite,
                                                        RunEngine::SilentWordBatch,
                                                        this.force_fresh_word,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )),
                                        )
                                        .item(
                                            PopupMenuItem::new(
                                                "Verify All Cases (IR & XML Only)   ⇧⌘I",
                                            )
                                            .on_click(window.listener_for(
                                                &v_q3,
                                                |this, _, window, cx| {
                                                    this.trigger_run_with(
                                                        RunScope::FullSuite,
                                                        RunEngine::IrOnly,
                                                        false,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )),
                                        )
                                }),
                        ),
                )
                .child(
                    // Right cluster: Open/Inspect Menu + Pane Toggles + Appearance
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("toolbar-open-dropdown")
                                .ghost()
                                .xsmall()
                                .icon(Lucide::ExternalLink)
                                .dropdown_caret(true)
                                .label("Open")
                                .tooltip("Open Case Artifacts in Word, Preview, or Finder")
                                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, window, _cx| {
                                    let v1 = view_for_open.clone();
                                    let v2 = view_for_open.clone();
                                    let v3 = view_for_open.clone();
                                    let v4 = view_for_open.clone();
                                    let v5 = view_for_open.clone();
                                    menu.min_w(px(245.0))
                                        .label("Case Artifacts")
                                        .item(
                                            PopupMenuItem::new("Open output.docx in Microsoft Word")
                                                .disabled(!has_docx)
                                                .on_click(window.listener_for(
                                                    &v1,
                                                    |this, _, _, cx| {
                                                        this.open_selected_docx_in_word(cx);
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new("Open typst.pdf in Preview")
                                                .disabled(!has_typst_pdf)
                                                .on_click(window.listener_for(
                                                    &v2,
                                                    |this, _, _, cx| {
                                                        this.open_selected_pdf_in_preview(false, cx);
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new("Open word.pdf in Preview")
                                                .disabled(!has_word_pdf)
                                                .on_click(window.listener_for(
                                                    &v3,
                                                    |this, _, _, cx| {
                                                        this.open_selected_pdf_in_preview(true, cx);
                                                    },
                                                )),
                                        )
                                        .item(
                                            PopupMenuItem::new("Reveal Case Folder in Finder")
                                                .disabled(!has_case_dir)
                                                .on_click(window.listener_for(
                                                    &v4,
                                                    |this, _, _, cx| {
                                                        this.reveal_selected_case_in_finder(cx);
                                                    },
                                                )),
                                        )
                                        .separator()
                                        .item(
                                            PopupMenuItem::new("Reload report.json from Disk   ⇧⌘L")
                                                .on_click(window.listener_for(
                                                    &v5,
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
                        .child(div().h_3p5().w_px().bg(border_col).mx_0p5())
                        .child(
                            Button::new("toolbar-toggle-drawer")
                                .ghost()
                                .xsmall()
                                .selected(self.show_diagnostics)
                                .icon(if self.show_diagnostics {
                                    IconName::PanelBottomOpen
                                } else {
                                    IconName::PanelBottom
                                })
                                .tooltip("Toggle Telemetry Drawer (⌥⌘D)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_diagnostics = !this.show_diagnostics;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("toolbar-toggle-inspector")
                                .ghost()
                                .xsmall()
                                .selected(self.show_right_inspector)
                                .icon(if self.show_right_inspector {
                                    IconName::PanelRightClose
                                } else {
                                    IconName::PanelRightOpen
                                })
                                .tooltip("Toggle Code Inspector (⌥⌘I)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_right_inspector = !this.show_right_inspector;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("toolbar-toggle-appearance")
                                .ghost()
                                .xsmall()
                                .icon(if is_dark {
                                    IconName::Sun
                                } else {
                                    IconName::Moon
                                })
                                .tooltip("Toggle Light / Dark Appearance (⇧⌘A)")
                                .on_click(cx.listener(|_, _, window, cx| {
                                    toggle_theme_mode(window, cx);
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }
}
