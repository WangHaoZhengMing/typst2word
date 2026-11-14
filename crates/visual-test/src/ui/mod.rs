//! Modular native macOS desktop inspection studio for `visual-test` (`src/ui/`).
//!
//! Layered UI architecture:
//! - [`actions`]: macOS System Menu Bar (`cx.set_menus`) + global `⌘` keyboard shortcuts
//! - [`toolbar`]: Unified Xcode-style macOS `TitleBar` with Run Scheme Pop-Up & Quick Open menu
//! - [`sidebar`]: Finder/Xcode Source-List Navigator with single-row Search + Filter Pop-Up menu
//! - [`stage`]: Preview-style Paper Comparison Canvas with compact View & Zoom Pop-Up menus
//! - [`diagnostics`]: Collapsible lower drawer with Geometry table, OOXML checks & PDF lines
//! - [`inspector`]: Right Xcode-style Code Inspector with `.docx` Package Part Pop-Up menu
//! - [`docx_parts`]: On-demand `.docx` ZIP XML reader & macOS Word/Preview/Finder openers
//! - [`status_bar`]: Calm bottom telemetry & case stepper bar
//! - [`state`]: `ParityStudioView` state, silent batch `run_suite` bridge, and navigation
//! - [`macos_theme`]: macOS Sequoia/Sonoma Light & Dark studio palettes
//! - [`typst_highlight`]: Custom Typst syntax highlighter + OpenXML pretty-formatter

pub mod actions;
mod diagnostics;
pub mod docx_parts;
mod inspector;
pub mod macos_theme;
mod sidebar;
mod stage;
pub mod state;
mod status_bar;
mod toolbar;
pub mod typst_highlight;

use std::{
    fs,
    path::{Path, PathBuf},
};

use gpui_kit::{
    AppContext as _, Bounds, WindowBounds, WindowOptions, assets::Assets, component::TitleBar, px,
    size,
};

pub use docx_parts::InspectorDoc;
pub use state::{
    CanvasMode, DiagnosticsTab, ParityStudioView, RunEngine, RunScope, StatusFilter,
};

use crate::{
    catalog::build_catalog,
    model::{AggregateStats, CaseReport, CaseStatus, IrSummary, LabReport},
    runner::workspace_root,
};

/// Load `report.json` from `report_dir` if present, otherwise construct a live
/// catalog preview so all atomic cases and Typst sources can be browsed immediately.
pub fn load_or_placeholder_report(report_dir: &Path) -> LabReport {
    let report_path = report_dir.join("report.json");
    if let Ok(raw) = fs::read_to_string(&report_path)
        && let Ok(rep) = serde_json::from_str::<LabReport>(&raw)
    {
        return rep;
    }

    let specs = build_catalog();
    let cases: Vec<CaseReport> = specs
        .into_iter()
        .map(|s| CaseReport {
            id: s.id,
            category: s.category,
            suite_rel_path: s.suite_rel_path,
            target_func: s.target_func,
            target_param: s.target_param,
            title: s.title,
            isolated_variable: s.isolated_variable,
            status: CaseStatus::IrOnly,
            status_reasons: Vec::new(),
            typst_source: s.typst_source,
            case_dir: String::new(),
            ir_json_path: String::new(),
            document_xml_path: String::new(),
            docx_path: String::new(),
            typst_pdf_path: String::new(),
            word_pdf_path: None,
            ir_summary: IrSummary {
                page_width_pt: s.page_width_pt,
                page_height_pt: s.page_height_pt,
                margins_pt: s.expectation.expected_margins_pt,
                mirror_margins: s.expectation.expected_mirror_margins.unwrap_or(false),
                ..Default::default()
            },
            checks: Vec::new(),
            typst_visual: None,
            word_visual: None,
            comparison: None,
        })
        .collect();

    LabReport {
        generated_at_unix: 0,
        out_dir: report_dir.to_string_lossy().to_string(),
        word_enabled: false,
        stats: AggregateStats {
            total_cases: cases.len(),
            ir_only_count: cases.len(),
            ..Default::default()
        },
        cases,
    }
}

/// Launch the native macOS 3-column `gpui-kit` desktop studio window.
pub fn run_desktop_app(report_dir: Option<PathBuf>) {
    let dir = report_dir.unwrap_or_else(|| workspace_root().join("target/visual-test"));
    gpui_kit::application()
        .with_assets(Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            actions::install_macos_menus_and_keys(cx);

            let bounds = Bounds::centered(None, size(px(1540.0), px(940.0)), cx);
            let opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1020.0), px(640.0))),
                ..TitleBar::window_options()
            };

            let dir_clone = dir.clone();
            gpui_kit::open_window(opts, cx, move |window, cx| {
                macos_theme::install_macos_theme(Some(window), cx);
                cx.new(|cx| ParityStudioView::new(window, cx, dir_clone))
            })
            .expect("failed to open visual-test gpui-kit window");
        });
}
