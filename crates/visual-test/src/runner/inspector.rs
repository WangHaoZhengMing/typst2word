//! Bridge to the native Swift `pdf_lab_inspector` tool (precompiled by `build.rs`).
//!
//! Supports two subcommands:
//! - `split`: lossless `PDFKit` page slicing of multi-section batch Word PDFs with strict
//!   page-count verification (`actualPages == expectedPages`).
//! - `inspect`: parallel `CoreGraphics` rasterization, ink bounding-box telemetry, and
//!   raw/aligned pixel IoU comparison.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::compile::CompiledCaseArtifacts;
use super::swift_inspector_source;
use crate::model::{PairComparison, PdfVisualSummary};

#[derive(Debug, Clone, Serialize)]
pub(super) struct SplitJob {
    pub id: String,
    #[serde(rename = "batchPdf")]
    pub batch_pdf: String,
    pub outputs: Vec<String>,
}

#[derive(Serialize)]
struct SplitManifest {
    jobs: Vec<SplitJob>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SplitJobOutput {
    pub id: String,
    pub ok: bool,
    #[serde(rename = "actualPages")]
    pub actual_pages: usize,
    #[serde(rename = "expectedPages")]
    pub expected_pages: usize,
    pub error: Option<String>,
}

#[derive(Serialize)]
struct InspectorManifest {
    scale: f64,
    jobs: Vec<InspectorJob>,
}

#[derive(Serialize)]
struct InspectorJob {
    id: String,
    #[serde(rename = "typstPdf")]
    typst_pdf: String,
    #[serde(rename = "wordPdf")]
    word_pdf: Option<String>,
    #[serde(rename = "outDir")]
    out_dir: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct InspectorJobOutput {
    pub id: String,
    pub error: Option<String>,
    pub typst: Option<PdfVisualSummary>,
    pub word: Option<PdfVisualSummary>,
    pub comparison: Option<PairComparison>,
}

/// Resolve the `pdf_lab_inspector` binary built by `crates/visual-test/build.rs`,
/// falling back to on-demand `swiftc -O` compilation only if the `OUT_DIR` artifact was cleaned.
pub(super) fn resolve_inspector_bin(out_dir: &Path) -> Result<PathBuf, String> {
    let built_path = env!("VISUAL_TEST_INSPECTOR_BIN");
    if !built_path.is_empty() {
        let candidate = PathBuf::from(built_path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let bin_dir = out_dir.join("bin");
    fs::create_dir_all(&bin_dir).map_err(|e| format!("create bin dir: {e}"))?;
    let bin_path = bin_dir.join("pdf_lab_inspector");
    let src_path = swift_inspector_source();

    let need_build = match (fs::metadata(&src_path), fs::metadata(&bin_path)) {
        (Ok(sm), Ok(bm)) => sm.modified().ok() > bm.modified().ok(),
        (Ok(_), Err(_)) => true,
        (Err(e), _) => return Err(format!("missing swift source: {e}")),
    };

    if need_build {
        let output = Command::new("swiftc")
            .arg("-O")
            .arg(&src_path)
            .arg("-o")
            .arg(&bin_path)
            .output()
            .map_err(|e| format!("failed to spawn swiftc: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "swiftc failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(bin_path)
}

/// Slice one or more multi-section batch Word PDFs into individual 1-page `word.pdf` files.
/// Never writes misaligned slices if `actual_pages != expected_pages`.
pub(super) fn split_batch_pdfs(
    inspector_bin: &Path,
    manifest_path: &Path,
    jobs: Vec<SplitJob>,
) -> Result<HashMap<String, SplitJobOutput>, String> {
    if jobs.is_empty() {
        return Ok(HashMap::new());
    }

    let manifest_json = serde_json::to_string_pretty(&SplitManifest { jobs })
        .map_err(|e| format!("serialize split manifest: {e}"))?;
    fs::write(manifest_path, manifest_json).map_err(|e| format!("write split manifest: {e}"))?;

    let out = Command::new(inspector_bin)
        .arg("split")
        .arg(manifest_path)
        .output()
        .map_err(|e| format!("run pdf_lab_inspector split: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "pdf_lab_inspector split failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let items: Vec<SplitJobOutput> = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("parse split output JSON: {e}"))?;
    Ok(items.into_iter().map(|i| (i.id.clone(), i)).collect())
}

/// Run parallel `PDFKit` / `CoreGraphics` visual telemetry & pixel IoU diff across all compiled cases.
pub(super) fn inspect_all_cases(
    inspector_bin: &Path,
    out_dir: &Path,
    raster_scale: f64,
    artifacts: &[CompiledCaseArtifacts],
    word_pdfs: &HashMap<String, PathBuf>,
) -> Result<HashMap<String, InspectorJobOutput>, String> {
    let jobs: Vec<InspectorJob> = artifacts
        .iter()
        .filter(|a| a.compile_error.is_none())
        .map(|a| InspectorJob {
            id: a.spec.id.clone(),
            typst_pdf: a.typst_pdf_path.to_string_lossy().to_string(),
            word_pdf: word_pdfs
                .get(&a.spec.id)
                .map(|p| p.to_string_lossy().to_string()),
            out_dir: a.case_dir.to_string_lossy().to_string(),
        })
        .collect();

    if jobs.is_empty() {
        return Ok(HashMap::new());
    }

    let manifest_path = out_dir.join("inspector_manifest.json");
    let manifest_json = serde_json::to_string_pretty(&InspectorManifest {
        scale: raster_scale,
        jobs,
    })
    .map_err(|e| format!("serialize inspect manifest: {e}"))?;
    fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("write inspect manifest: {e}"))?;

    let insp_out = Command::new(inspector_bin)
        .arg("inspect")
        .arg(&manifest_path)
        .output()
        .map_err(|e| format!("run pdf_lab_inspector inspect: {e}"))?;
    if !insp_out.status.success() {
        return Err(format!(
            "pdf_lab_inspector inspect failed: {}",
            String::from_utf8_lossy(&insp_out.stderr)
        ));
    }

    let insp_items: Vec<InspectorJobOutput> = serde_json::from_slice(&insp_out.stdout)
        .map_err(|e| format!("parse inspector output JSON: {e}"))?;
    Ok(insp_items.into_iter().map(|i| (i.id.clone(), i)).collect())
}
