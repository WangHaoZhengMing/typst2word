//! Four-stage `visual-test` execution pipeline:
//!
//! - [`compile`]: Stage 1 — Parallel 16 MiB-stack Typst compilation, IR extraction,
//!   `WordDocument` construction, `.docx` writing, and structural OOXML verification.
//! - [`word_batch`] + [`word_applescript`]: Stage 2 — Silent macOS sandbox Word session +
//!   Multi-Section "Mega-Doc" batch `.docx` PDF export + `PDFKit` page slicing with
//!   recursive overflow bisection fallback.
//! - [`inspector`]: Stage 3 — Parallel `PDFKit` / `CoreGraphics` rasterization, ink bounding
//!   boxes, and raw/aligned pixel IoU telemetry.
//! - [`report`]: Stage 4 — `Pass` / `Drift` / `Fail` / `IrOnly` classification, prose
//!   line-break parity diff, JSON report persistence, and Markdown summary generation.

mod compile;
mod inspector;
mod report;
mod word_applescript;
mod word_batch;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub use report::{recompute_aggregate_stats, render_markdown_summary};

use crate::catalog::build_catalog;
use crate::model::{Category, LabReport, TestCaseSpec};

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub out_dir: PathBuf,
    pub category_filter: Option<Category>,
    pub suite_filter: Option<String>,
    pub exact_case_id: Option<String>,
    pub id_substring_filter: Option<String>,
    pub skip_word: bool,
    pub fresh_word: bool,
    pub raster_scale: f64,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            out_dir: workspace_root().join("target/visual-test"),
            category_filter: None,
            suite_filter: None,
            exact_case_id: None,
            id_substring_filter: None,
            skip_word: false,
            fresh_word: false,
            raster_scale: 2.0,
        }
    }
}

impl RunConfig {
    /// Returns true if any case-subset filter is active.
    pub fn is_filtered(&self) -> bool {
        self.category_filter.is_some()
            || self.suite_filter.is_some()
            || self.exact_case_id.is_some()
            || self.id_substring_filter.is_some()
    }
}

pub fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest_dir)
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub fn swift_inspector_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/pdf_lab_inspector.swift")
}

pub fn run_suite(config: &RunConfig) -> Result<LabReport, String> {
    let fixtures = fixtures_dir();
    compile::ensure_probe_png(&fixtures)?;
    fs::create_dir_all(&config.out_dir).map_err(|e| format!("create out_dir: {e}"))?;
    let cases_root = config.out_dir.join("cases");
    fs::create_dir_all(&cases_root).map_err(|e| format!("create cases_root: {e}"))?;

    let inspector_bin = inspector::resolve_inspector_bin(&config.out_dir)?;
    let selected_specs = select_specs(config);

    // Stage 1: Parallel Typst compile + IR extract + WordDocument + DOCX + OOXML checks.
    let artifacts = compile::compile_all_cases(&selected_specs, &cases_root, &fixtures)?;

    // Stage 2: Silent Microsoft Word multi-section Mega-Doc PDF export + PDFKit page slicing.
    let (word_pdfs, word_degraded) = if config.skip_word {
        (HashMap::new(), false)
    } else {
        word_batch::export_word_pdfs(&artifacts, config.fresh_word, &inspector_bin, &config.out_dir)?
    };

    // Stage 3: Parallel PDFKit / CoreGraphics rasterization, ink bbox & pixel IoU diff.
    let insp_map = inspector::inspect_all_cases(
        &inspector_bin,
        &config.out_dir,
        config.raster_scale,
        &artifacts,
        &word_pdfs,
    )?;

    // Stage 4: Status classification, report.json merge/write & SUMMARY.md generation.
    Ok(report::build_and_persist_report(
        config,
        artifacts,
        &word_pdfs,
        word_degraded,
        insp_map,
    ))
}

fn select_specs(config: &RunConfig) -> Vec<TestCaseSpec> {
    build_catalog()
        .into_iter()
        .filter(|s| config.category_filter.is_none_or(|cat| s.category == cat))
        .filter(|s| {
            config
                .suite_filter
                .as_ref()
                .is_none_or(|suite| &s.suite_rel_path == suite)
        })
        .filter(|s| {
            config
                .exact_case_id
                .as_ref()
                .is_none_or(|exact| &s.id == exact)
        })
        .filter(|s| {
            config.id_substring_filter.as_ref().is_none_or(|sub| {
                let q = sub.to_ascii_lowercase();
                s.id.to_ascii_lowercase().contains(&q)
                    || s.title.to_ascii_lowercase().contains(&q)
                    || s.isolated_variable.to_ascii_lowercase().contains(&q)
                    || s.suite_rel_path.to_ascii_lowercase().contains(&q)
                    || s.target_func.to_ascii_lowercase().contains(&q)
                    || s.target_param.to_ascii_lowercase().contains(&q)
            })
        })
        .collect()
}
