//! Modular native macOS desktop inspection studio for `visual-test` (`src/ui/`).
//!
//! Architecture:
//! - [`titlebar`]: Native macOS `TitleBar` (traffic lights inset) + bottom `StatusBar`
//! - [`sidebar`]: Left column — Finder/Xcode-style source-list navigator
//! - [`center_stage`]: Center upper split — 3-Up / Side-by-Side / Overlay visual PDF stage
//! - [`diagnostics`]: Center lower split — Geometry table, IR/OOXML checks, PDFKit lines
//! - [`code_panel`]: Right column — Syntax-highlighted GPUI Kit `Editor` (`.typ`, `ir.json`, `.xml`)
//! - [`typst_highlight`]: Custom Typst `InputHighlighter` + lossless OpenXML pretty-formatter
//! - [`macos_theme`]: Light/Dark macOS Studio palettes synced with system appearance
//! - [`state`]: `ParityStudioView` state, editor synchronization, and 3-column layout

mod center_stage;
mod code_panel;
mod diagnostics;
pub mod macos_theme;
mod sidebar;
pub mod state;
mod titlebar;
pub mod typst_highlight;

use std::{
    fs,
    path::{Path, PathBuf},
};

use gpui_kit::{
    AppContext as _, Bounds, WindowBounds, WindowOptions, assets::Assets,
    component::TitleBar, px, size,
};

pub use state::{CanvasMode, CodeTab, DiagnosticsTab, ParityStudioView, StatusFilter};

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
