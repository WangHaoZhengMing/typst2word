use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use compiler::{TypstWorld, compile_world};
use docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options};
use extract::extract_document_ir;
use image::{ImageBuffer, Rgba};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use word_construct::build_word_document;
use word_construct::page_calibration::PageMarginCalibration;
use zip::ZipArchive;

use crate::catalog::build_catalog;
use crate::model::{
    AggregateStats, CaseReport, CaseStatus, Category, LabReport, PairComparison, PdfVisualSummary,
    TestCaseSpec,
};
use crate::verify::{pretty_format_xml, summarize_and_verify};

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub out_dir: PathBuf,
    pub category_filter: Option<Category>,
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
            id_substring_filter: None,
            skip_word: false,
            fresh_word: false,
            raster_scale: 2.0,
        }
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

fn ensure_probe_png(fixtures: &Path) -> Result<(), String> {
    fs::create_dir_all(fixtures).map_err(|e| format!("create fixtures dir: {e}"))?;
    let png_path = fixtures.join("probe.png");
    if png_path.exists() {
        return Ok(());
    }
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(96, 48);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let border = !(3..93).contains(&x) || !(3..45).contains(&y);
        let diag = ((x as i32 - y as i32 * 2).abs()) <= 3;
        *px = if border {
            Rgba([29, 78, 216, 255])
        } else if diag {
            Rgba([220, 38, 38, 255])
        } else {
            Rgba([219, 234, 254, 255])
        };
    }
    img.save(&png_path)
        .map_err(|e| format!("failed to write probe.png: {e}"))
}

fn ensure_swift_inspector(out_dir: &Path) -> Result<PathBuf, String> {
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

struct CompiledCaseArtifacts {
    spec: TestCaseSpec,
    case_dir: PathBuf,
    typst_pdf_path: PathBuf,
    docx_path: PathBuf,
    docx_sha256: String,
    ir_json_path: PathBuf,
    document_xml_path: PathBuf,
    compile_error: Option<String>,
    ir_summary: crate::model::IrSummary,
    checks: Vec<crate::model::CheckItem>,
}

fn compile_single_case(spec: &TestCaseSpec, cases_root: &Path, fixtures: &Path) -> CompiledCaseArtifacts {
    let case_dir = cases_root.join(&spec.id);
    let _ = fs::create_dir_all(&case_dir);
    let typ_file = case_dir.join("source.typ");
    let _ = fs::write(&typ_file, &spec.typst_source);

    let typst_pdf_path = case_dir.join("typst.pdf");
    let docx_path = case_dir.join("output.docx");
    let ir_json_path = case_dir.join("ir.json");
    let document_xml_path = case_dir.join("document.xml");

    let world = TypstWorld::from_str(&spec.typst_source, Some(fixtures.to_path_buf()));
    let compiled = match compile_world(&world) {
        Ok(c) => c,
        Err(err) => {
            return CompiledCaseArtifacts {
                spec: spec.clone(),
                case_dir,
                typst_pdf_path,
                docx_path,
                docx_sha256: String::new(),
                ir_json_path,
                document_xml_path,
                compile_error: Some(format!("Typst compile error: {err}")),
                ir_summary: Default::default(),
                checks: Vec::new(),
            };
        }
    };

    // Export Typst ground-truth PDF in-process (disable tagged PDF so internal realization markers never interfere).
    let pdf_opts = typst_pdf::PdfOptions {
        tagged: false,
        ..Default::default()
    };
    let pdf_res = typst_pdf::pdf(&compiled.document, &pdf_opts);
    match pdf_res {
        Ok(pdf_bytes) => {
            if let Err(e) = fs::write(&typst_pdf_path, pdf_bytes) {
                return CompiledCaseArtifacts {
                    spec: spec.clone(),
                    case_dir,
                    typst_pdf_path,
                    docx_path,
                    docx_sha256: String::new(),
                    ir_json_path,
                    document_xml_path,
                    compile_error: Some(format!("Write typst.pdf error: {e}")),
                    ir_summary: Default::default(),
                    checks: Vec::new(),
                };
            }
        }
        Err(_) => {
            // Fallback to `typst compile` CLI if needed.
            let cli_out = Command::new("typst")
                .arg("compile")
                .arg("--root")
                .arg(fixtures)
                .arg(&typ_file)
                .arg(&typst_pdf_path)
                .output();
            if let Err(e) = cli_out {
                return CompiledCaseArtifacts {
                    spec: spec.clone(),
                    case_dir,
                    typst_pdf_path,
                    docx_path,
                    docx_sha256: String::new(),
                    ir_json_path,
                    document_xml_path,
                    compile_error: Some(format!("Typst PDF fallback failed: {e}")),
                    ir_summary: Default::default(),
                    checks: Vec::new(),
                };
            }
        }
    }

    let ir = extract_document_ir(&compiled, &world);
    if let Ok(ir_json) = serde_json::to_string_pretty(&ir) {
        let _ = fs::write(&ir_json_path, ir_json);
    }

    let word_doc = build_word_document(&ir);
    let docx_bytes = match write_docx_to_bytes_with_options(
        &word_doc,
        DocxWriteOptions {
            page_margin_calibration: PageMarginCalibration::Preserve,
        },
    ) {
        Ok(b) => b,
        Err(err) => {
            return CompiledCaseArtifacts {
                spec: spec.clone(),
                case_dir,
                typst_pdf_path,
                docx_path,
                docx_sha256: String::new(),
                ir_json_path,
                document_xml_path,
                compile_error: Some(format!("DOCX writer error: {err}")),
                ir_summary: Default::default(),
                checks: Vec::new(),
            };
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&docx_bytes);
    let docx_sha256: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let _ = fs::write(&docx_path, &docx_bytes);

    // Extract word/document.xml + furniture XML parts from the DOCX zip in memory.
    let mut doc_xml = String::new();
    let mut all_xml = String::new();
    if let Ok(mut zip) = ZipArchive::new(Cursor::new(&docx_bytes)) {
        for idx in 0..zip.len() {
            if let Ok(mut entry) = zip.by_index(idx) {
                let name = entry.name().map(|s| s.to_string()).unwrap_or_default();
                if name.starts_with("word/") && name.ends_with(".xml") {
                    let mut buf = String::new();
                    if entry.read_to_string(&mut buf).is_ok() {
                        if name == "word/document.xml" {
                            doc_xml = buf.clone();
                        }
                        all_xml.push_str(&buf);
                        all_xml.push('\n');
                    }
                }
            }
        }
    }

    let pretty_xml = pretty_format_xml(&doc_xml);
    let _ = fs::write(&document_xml_path, &pretty_xml);

    let (ir_summary, checks) = summarize_and_verify(spec, &ir, &word_doc, &doc_xml, &all_xml);

    CompiledCaseArtifacts {
        spec: spec.clone(),
        case_dir,
        typst_pdf_path,
        docx_path,
        docx_sha256,
        ir_json_path,
        document_xml_path,
        compile_error: None,
        ir_summary,
        checks,
    }
}

fn run_osascript_with_timeout(script: &str, timeout: Duration) -> Result<(), String> {
    let mut child = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn osascript: {e}"))?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let mut stderr = String::new();
                if let Some(mut err_pipe) = child.stderr.take() {
                    let _ = err_pipe.read_to_string(&mut stderr);
                }
                return Err(format!("osascript exited with {status}: {}", stderr.trim()));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "osascript timed out after {}s (Word may be blocked by a modal or security prompt)",
                        timeout.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("osascript wait error: {e}"));
            }
        }
    }
}

/// Batch convert `.docx -> .pdf` via Microsoft Word AppleScript staged in `/private/tmp/...`,
/// skipping cases whose `.docx` SHA-256 already matches the cached `.word.pdf.sha256`.
/// Returns `(exported_pdfs, word_degraded_to_ir_only)`.
fn batch_export_word_pdfs(
    artifacts: &[CompiledCaseArtifacts],
    fresh_word: bool,
) -> Result<(HashMap<String, PathBuf>, bool), String> {
    let mut result = HashMap::new();
    let mut pending: Vec<(&CompiledCaseArtifacts, PathBuf, PathBuf)> = Vec::new();

    for art in artifacts {
        if art.compile_error.is_some() {
            continue;
        }
        let word_pdf = art.case_dir.join("word.pdf");
        let hash_file = art.case_dir.join("word.pdf.sha256");
        if !fresh_word
            && word_pdf.exists()
            && hash_file.exists()
            && let Ok(saved_hash) = fs::read_to_string(&hash_file)
            && saved_hash.trim() == art.docx_sha256
        {
            result.insert(art.spec.id.clone(), word_pdf);
            continue;
        }
        pending.push((art, word_pdf, hash_file));
    }

    if pending.is_empty() {
        return Ok((result, false));
    }

    let stage_dir = PathBuf::from("/private/tmp/typst2word_visual_test_stage");
    let _ = fs::remove_dir_all(&stage_dir);
    fs::create_dir_all(&stage_dir).map_err(|e| format!("create /private/tmp stage dir: {e}"))?;

    // Process in chunks of 12 with a strict 25s host-side timeout so an unattended Mac
    // never hangs if Microsoft Word shows a security or recovery prompt.
    let mut word_degraded = false;
    for (chunk_idx, chunk) in pending.chunks(12).enumerate() {
        let mut script_lines = vec![
            "with timeout of 20 seconds".to_string(),
            "tell application \"Microsoft Word\"".to_string(),
            "  launch".to_string(),
        ];
        let mut staged_targets: Vec<(usize, PathBuf)> = Vec::new();

        for (local_i, (art, _, _)) in chunk.iter().enumerate() {
            let tag = format!("c{}_{}", chunk_idx, local_i);
            let st_docx = stage_dir.join(format!("{tag}.docx"));
            let st_pdf = stage_dir.join(format!("{tag}.pdf"));
            fs::copy(&art.docx_path, &st_docx)
                .map_err(|e| format!("copy staged docx {}: {e}", art.spec.id))?;
            script_lines.push(format!(
                "  open POSIX file \"{}\" with read only",
                st_docx.display()
            ));
            script_lines.push(format!(
                "  save as active document file name \"{}\" file format format PDF",
                st_pdf.display()
            ));
            script_lines.push("  close active document saving no".to_string());
            staged_targets.push((local_i, st_pdf));
        }
        script_lines.push("end tell".to_string());
        script_lines.push("end timeout".to_string());
        let script = script_lines.join("\n");

        if let Err(err) = run_osascript_with_timeout(&script, Duration::from_secs(25)) {
            eprintln!(
                "[visual-test] Microsoft Word unavailable ({err}); skipping remaining Word PDF exports."
            );
            word_degraded = true;
            break;
        }

        for (local_i, st_pdf) in staged_targets {
            let (art, final_pdf, hash_file) = &chunk[local_i];
            if st_pdf.exists() {
                fs::copy(&st_pdf, final_pdf)
                    .map_err(|e| format!("copy word.pdf back for {}: {e}", art.spec.id))?;
                let _ = fs::write(hash_file, &art.docx_sha256);
                result.insert(art.spec.id.clone(), final_pdf.clone());
            }
        }
    }

    let _ = fs::remove_dir_all(&stage_dir);
    Ok((result, word_degraded))
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

#[derive(Deserialize)]
struct InspectorJobOutput {
    id: String,
    error: Option<String>,
    typst: Option<PdfVisualSummary>,
    word: Option<PdfVisualSummary>,
    comparison: Option<PairComparison>,
}

pub fn run_suite(config: &RunConfig) -> Result<LabReport, String> {
    let fixtures = fixtures_dir();
    ensure_probe_png(&fixtures)?;
    fs::create_dir_all(&config.out_dir).map_err(|e| format!("create out_dir: {e}"))?;
    let cases_root = config.out_dir.join("cases");
    fs::create_dir_all(&cases_root).map_err(|e| format!("create cases_root: {e}"))?;

    let inspector_bin = ensure_swift_inspector(&config.out_dir)?;

    let all_specs: Vec<TestCaseSpec> = build_catalog()
        .into_iter()
        .filter(|s| {
            config
                .category_filter
                .is_none_or(|cat| s.category == cat)
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
        .collect();

    // 1. Parallel Typst compile + IR extract + WordDocument construct + DOCX write + XML check.
    // Use a 16 MiB stack per worker thread so deeply nested packages (e.g. CeTZ) never overflow
    // macOS's default 2 MiB worker stack in unoptimized debug/test builds or GUI background runs.
    let compile_pool = rayon::ThreadPoolBuilder::new()
        .stack_size(16 * 1024 * 1024)
        .build()
        .map_err(|e| format!("failed to build rayon compile pool: {e}"))?;
    let artifacts: Vec<CompiledCaseArtifacts> = compile_pool.install(|| {
        all_specs
            .par_iter()
            .map(|spec| compile_single_case(spec, &cases_root, &fixtures))
            .collect()
    });

    // 2. Optional Microsoft Word batch PDF export (with SHA-256 caching & host timeout).
    let (word_pdfs, word_degraded) = if config.skip_word {
        (HashMap::new(), false)
    } else {
        batch_export_word_pdfs(&artifacts, config.fresh_word)?
    };

    // 3. Batch Swift PDFKit/CoreGraphics raster + ink-bbox + IoU + overlay PNG inspection.
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

    let manifest_path = config.out_dir.join("inspector_manifest.json");
    let manifest_json = serde_json::to_string_pretty(&InspectorManifest {
        scale: config.raster_scale,
        jobs,
    })
    .map_err(|e| format!("serialize manifest: {e}"))?;
    fs::write(&manifest_path, manifest_json).map_err(|e| format!("write manifest: {e}"))?;

    let insp_out = Command::new(&inspector_bin)
        .arg(&manifest_path)
        .output()
        .map_err(|e| format!("run pdf_lab_inspector: {e}"))?;
    if !insp_out.status.success() {
        return Err(format!(
            "pdf_lab_inspector failed: {}",
            String::from_utf8_lossy(&insp_out.stderr)
        ));
    }

    let insp_items: Vec<InspectorJobOutput> = serde_json::from_slice(&insp_out.stdout)
        .map_err(|e| format!("parse inspector output JSON: {e}"))?;
    let mut insp_map: HashMap<String, InspectorJobOutput> =
        insp_items.into_iter().map(|i| (i.id.clone(), i)).collect();

    // 4. Assemble final per-case reports & status classification.
    let mut case_reports = Vec::with_capacity(artifacts.len());
    let mut pass_count = 0usize;
    let mut drift_count = 0usize;
    let mut fail_count = 0usize;
    let mut ir_only_count = 0usize;

    let mut sum_dl = 0.0;
    let mut sum_dt = 0.0;
    let mut sum_dw = 0.0;
    let mut sum_dh = 0.0;
    let mut sum_raw_iou = 0.0;
    let mut sum_aligned_iou = 0.0;
    let mut cmp_n = 0usize;

    for art in artifacts {
        let mut reasons = Vec::new();
        if let Some(err) = &art.compile_error {
            reasons.push(err.clone());
        }
        for chk in &art.checks {
            if !chk.passed {
                reasons.push(format!("{}: {}", chk.name, chk.detail));
            }
        }

        let insp = insp_map.remove(&art.spec.id);
        if let Some(ref io) = insp
            && let Some(ref e) = io.error
        {
            reasons.push(e.clone());
        }

        let typst_visual = insp.as_ref().and_then(|i| i.typst.clone());
        let word_visual = insp.as_ref().and_then(|i| i.word.clone());
        let comparison = insp.as_ref().and_then(|i| i.comparison.clone());

        let status = if !reasons.is_empty() {
            fail_count += 1;
            CaseStatus::Fail
        } else if config.skip_word || (word_degraded && comparison.is_none()) {
            ir_only_count += 1;
            CaseStatus::IrOnly
        } else if let Some(ref cmp) = comparison {
            cmp_n += 1;
            sum_dl += cmp.delta_left_pt.abs();
            sum_dt += cmp.delta_top_pt.abs();
            sum_dw += cmp.delta_width_pt.abs();
            sum_dh += cmp.delta_height_pt.abs();
            sum_raw_iou += cmp.raw_iou;
            sum_aligned_iou += cmp.aligned_iou;

            let exp = &art.spec.expectation;
            let mut drift_reasons = Vec::new();
            if cmp.delta_left_pt.abs() > exp.max_delta_left_pt {
                drift_reasons.push(format!(
                    "|Δleft| = {:.2}pt > {:.1}pt",
                    cmp.delta_left_pt, exp.max_delta_left_pt
                ));
            }
            if cmp.delta_top_pt.abs() > exp.max_delta_top_pt {
                drift_reasons.push(format!(
                    "|Δtop| = {:.2}pt > {:.1}pt",
                    cmp.delta_top_pt, exp.max_delta_top_pt
                ));
            }
            if cmp.delta_width_pt.abs() > exp.max_delta_width_pt {
                drift_reasons.push(format!(
                    "|Δwidth| = {:.2}pt > {:.1}pt",
                    cmp.delta_width_pt, exp.max_delta_width_pt
                ));
            }
            if cmp.delta_height_pt.abs() > exp.max_delta_height_pt {
                drift_reasons.push(format!(
                    "|Δheight| = {:.2}pt > {:.1}pt",
                    cmp.delta_height_pt, exp.max_delta_height_pt
                ));
            }
            if let (Some(tv), Some(wv)) = (&typst_visual, &word_visual) {
                if tv.page_count != wv.page_count {
                    drift_reasons.push(format!(
                        "page_count Typst={} vs Word={}",
                        tv.page_count, wv.page_count
                    ));
                }
                if matches!(
                    art.spec.target_func.as_str(),
                    "par" | "text" | "heading" | "title"
                ) && let Some(line_diff) = detect_prose_linebreak_mismatch(&tv.lines, &wv.lines)
                {
                    drift_reasons.push(line_diff);
                }
            }

            if drift_reasons.is_empty() {
                pass_count += 1;
                CaseStatus::Pass
            } else {
                drift_count += 1;
                reasons = drift_reasons;
                CaseStatus::Drift
            }
        } else {
            fail_count += 1;
            reasons.push("Missing Word PDF comparison output".to_string());
            CaseStatus::Fail
        };

        let word_pdf_path = word_pdfs
            .get(&art.spec.id)
            .map(|p| p.to_string_lossy().to_string());

        case_reports.push(CaseReport {
            id: art.spec.id,
            category: art.spec.category,
            suite_rel_path: art.spec.suite_rel_path,
            target_func: art.spec.target_func,
            target_param: art.spec.target_param,
            title: art.spec.title,
            isolated_variable: art.spec.isolated_variable,
            status,
            status_reasons: reasons,
            typst_source: art.spec.typst_source,
            case_dir: art.case_dir.to_string_lossy().to_string(),
            ir_json_path: art.ir_json_path.to_string_lossy().to_string(),
            document_xml_path: art.document_xml_path.to_string_lossy().to_string(),
            docx_path: art.docx_path.to_string_lossy().to_string(),
            typst_pdf_path: art.typst_pdf_path.to_string_lossy().to_string(),
            word_pdf_path,
            ir_summary: art.ir_summary,
            checks: art.checks,
            typst_visual,
            word_visual,
            comparison,
        });
    }

    let denom = (cmp_n.max(1)) as f64;
    let stats = AggregateStats {
        total_cases: case_reports.len(),
        pass_count,
        drift_count,
        fail_count,
        ir_only_count,
        mean_abs_delta_left_pt: round2(sum_dl / denom),
        mean_abs_delta_top_pt: round2(sum_dt / denom),
        mean_abs_delta_width_pt: round2(sum_dw / denom),
        mean_abs_delta_height_pt: round2(sum_dh / denom),
        mean_raw_iou: round4(sum_raw_iou / denom),
        mean_aligned_iou: round4(sum_aligned_iou / denom),
    };

    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let report = LabReport {
        generated_at_unix: now_unix,
        out_dir: config.out_dir.to_string_lossy().to_string(),
        word_enabled: !config.skip_word && !word_degraded,
        stats,
        cases: case_reports,
    };

    // Only overwrite global report.json / SUMMARY.md when running unfiltered suite
    // (or if report.json does not exist yet) so single-case runs don't clobber the full report.
    let report_json_path = config.out_dir.join("report.json");
    if (config.category_filter.is_none() && config.id_substring_filter.is_none())
        || !report_json_path.exists()
    {
        if let Ok(js) = serde_json::to_string_pretty(&report) {
            let _ = fs::write(&report_json_path, js);
        }
        let summary_md = render_markdown_summary(&report);
        let _ = fs::write(config.out_dir.join("SUMMARY.md"), summary_md);
    } else if let Ok(existing_str) = fs::read_to_string(&report_json_path)
        && let Ok(mut existing_rep) = serde_json::from_str::<LabReport>(&existing_str)
    {
        // Merge updated cases back into existing report.json so GUI single-case re-runs update cleanly!
        for updated in &report.cases {
            if let Some(slot) = existing_rep.cases.iter_mut().find(|c| c.id == updated.id) {
                *slot = updated.clone();
            } else {
                existing_rep.cases.push(updated.clone());
            }
        }
        recompute_aggregate_stats(&mut existing_rep);
        if let Ok(js) = serde_json::to_string_pretty(&existing_rep) {
            let _ = fs::write(&report_json_path, js);
        }
        let summary_md = render_markdown_summary(&existing_rep);
        let _ = fs::write(config.out_dir.join("SUMMARY.md"), summary_md);
    }

    Ok(report)
}

pub fn recompute_aggregate_stats(rep: &mut LabReport) {
    let mut pass = 0usize;
    let mut drift = 0usize;
    let mut fail = 0usize;
    let mut ir_only = 0usize;
    let mut dl = 0.0;
    let mut dt = 0.0;
    let mut dw = 0.0;
    let mut dh = 0.0;
    let mut riou = 0.0;
    let mut aiou = 0.0;
    let mut n = 0usize;

    for c in &rep.cases {
        match c.status {
            CaseStatus::Pass => pass += 1,
            CaseStatus::Drift => drift += 1,
            CaseStatus::Fail => fail += 1,
            CaseStatus::IrOnly => ir_only += 1,
        }
        if let Some(ref cmp) = c.comparison {
            n += 1;
            dl += cmp.delta_left_pt.abs();
            dt += cmp.delta_top_pt.abs();
            dw += cmp.delta_width_pt.abs();
            dh += cmp.delta_height_pt.abs();
            riou += cmp.raw_iou;
            aiou += cmp.aligned_iou;
        }
    }
    let denom = (n.max(1)) as f64;
    rep.stats = AggregateStats {
        total_cases: rep.cases.len(),
        pass_count: pass,
        drift_count: drift,
        fail_count: fail,
        ir_only_count: ir_only,
        mean_abs_delta_left_pt: round2(dl / denom),
        mean_abs_delta_top_pt: round2(dt / denom),
        mean_abs_delta_width_pt: round2(dw / denom),
        mean_abs_delta_height_pt: round2(dh / denom),
        mean_raw_iou: round4(riou / denom),
        mean_aligned_iou: round4(aiou / denom),
    };
}

pub fn render_markdown_summary(report: &LabReport) -> String {
    let mut md = String::with_capacity(16384);
    let s = &report.stats;
    let _ = writeln!(md, "# Typst-to-Word Single-Variable (`margin: 10pt`) Visual Test Report\n");
    let _ = writeln!(
        md,
        "- **Total Atomic Cases**: `{}`  |  **PASS**: `{}`  |  **DRIFT**: `{}`  |  **FAIL**: `{}`  |  **IR-Only**: `{}`",
        s.total_cases, s.pass_count, s.drift_count, s.fail_count, s.ir_only_count
    );
    let _ = writeln!(
        md,
        "- **Mean Absolute BBox Deltas**: `|Δleft| = {:.2}pt`, `|Δtop| = {:.2}pt`, `|Δwidth| = {:.2}pt`, `|Δheight| = {:.2}pt`",
        s.mean_abs_delta_left_pt,
        s.mean_abs_delta_top_pt,
        s.mean_abs_delta_width_pt,
        s.mean_abs_delta_height_pt
    );
    let _ = writeln!(
        md,
        "- **Mean Ink Overlap**: `Raw IoU = {:.1}%`, `Aligned Shape IoU = {:.1}%`\n",
        s.mean_raw_iou * 100.0,
        s.mean_aligned_iou * 100.0
    );

    for cat in Category::ALL {
        let cat_cases: Vec<&CaseReport> = report.cases.iter().filter(|c| c.category == cat).collect();
        if cat_cases.is_empty() {
            continue;
        }
        let _ = writeln!(md, "## {}\n", cat.title());
        let _ = writeln!(
            md,
            "| Status | Case ID | Isolated Variable | Typst Ink (L,T,W,H) | Word Ink (L,T,W,H) | ΔL / ΔT / ΔW / ΔH (pt) | Raw / Aligned IoU | Notes |"
        );
        let _ = writeln!(
            md,
            "|---|---|---|---|---|---|---|---|"
        );
        for c in cat_cases {
            let tb = c
                .typst_visual
                .as_ref()
                .and_then(|v| v.ink_bbox)
                .map(|b| format!("({:.1}, {:.1}, {:.1}, {:.1})", b.left_pt, b.top_pt, b.width_pt, b.height_pt))
                .unwrap_or_else(|| "-".to_string());
            let wb = c
                .word_visual
                .as_ref()
                .and_then(|v| v.ink_bbox)
                .map(|b| format!("({:.1}, {:.1}, {:.1}, {:.1})", b.left_pt, b.top_pt, b.width_pt, b.height_pt))
                .unwrap_or_else(|| "-".to_string());
            let deltas = c
                .comparison
                .as_ref()
                .map(|cmp| {
                    format!(
                        "{:+.1} / {:+.1} / {:+.1} / {:+.1}",
                        cmp.delta_left_pt, cmp.delta_top_pt, cmp.delta_width_pt, cmp.delta_height_pt
                    )
                })
                .unwrap_or_else(|| "-".to_string());
            let ious = c
                .comparison
                .as_ref()
                .map(|cmp| format!("{:.0}% / {:.0}%", cmp.raw_iou * 100.0, cmp.aligned_iou * 100.0))
                .unwrap_or_else(|| "-".to_string());
            let note = if c.status_reasons.is_empty() {
                "OK".to_string()
            } else {
                c.status_reasons.join("; ")
            };
            let _ = writeln!(
                md,
                "| **{}** | `{}` | `{}` | {} | {} | {} | {} | {} |",
                c.status.label(),
                c.id,
                c.isolated_variable.replace('|', "\\|"),
                tb,
                wb,
                deltas,
                ious,
                note.replace('|', "\\|")
            );
        }
        let _ = writeln!(md);
    }

    md
}

const KANGXI_RADICALS_TO_UNIFIED: [char; 214] = [
    '一', '丨', '丶', '丿', '乙', '亅', '二', '亠', '人', '儿', '入', '八', '冂', '冖', '冫', '几',
    '凵', '刀', '力', '勹', '匕', '匚', '匸', '十', '卜', '卩', '厂', '厶', '又', '口', '囗', '土',
    '士', '夂', '夊', '夕', '大', '女', '子', '宀', '寸', '小', '尢', '尸', '屮', '山', '巛', '工',
    '己', '巾', '干', '幺', '广', '廴', '廾', '弋', '弓', '彐', '彡', '彳', '心', '戈', '戶', '手',
    '支', '攴', '文', '斗', '斤', '方', '无', '日', '曰', '月', '木', '欠', '止', '歹', '殳', '毋',
    '比', '毛', '氏', '气', '水', '火', '爪', '父', '爻', '爿', '片', '牙', '牛', '犬', '玄', '玉',
    '瓜', '瓦', '甘', '生', '用', '田', '疋', '疒', '癶', '白', '皮', '皿', '目', '矛', '矢', '石',
    '示', '禸', '禾', '穴', '立', '竹', '米', '糸', '缶', '网', '羊', '羽', '老', '而', '耒', '耳',
    '聿', '肉', '臣', '自', '至', '臼', '舌', '舛', '舟', '艮', '色', '艸', '虍', '虫', '血', '行',
    '衣', '襾', '見', '角', '言', '谷', '豆', '豕', '豸', '貝', '赤', '走', '足', '身', '車', '辛',
    '辰', '辵', '邑', '酉', '釆', '里', '金', '長', '門', '阜', '隶', '隹', '雨', '靑', '非', '面',
    '革', '韋', '韭', '音', '頁', '風', '飛', '食', '首', '香', '馬', '骨', '高', '髟', '鬥', '鬯',
    '鬲', '鬼', '魚', '鳥', '鹵', '鹿', '麥', '麻', '黃', '黍', '黑', '黹', '黽', '鼎', '鼓', '鼠',
    '鼻', '齊', '齒', '龍', '龜', '龠',
];

fn coalesce_visual_rows(lines: &[crate::model::LineMetric]) -> Vec<String> {
    let mut sorted: Vec<&crate::model::LineMetric> =
        lines.iter().filter(|l| !l.text.trim().is_empty()).collect();
    sorted.sort_by(|a, b| {
        a.page
            .cmp(&b.page)
            .then_with(|| a.y_top_pt.total_cmp(&b.y_top_pt))
            .then_with(|| a.x_pt.total_cmp(&b.x_pt))
    });

    type LineRow<'a> = (usize, f64, Vec<(f64, &'a str)>);
    let mut rows: Vec<LineRow<'_>> = Vec::new();
    for lm in sorted {
        if let Some(last) = rows.last_mut()
            && last.0 == lm.page
            && (lm.y_top_pt - last.1).abs() <= 3.0
        {
            last.2.push((lm.x_pt, lm.text.as_str()));
        } else {
            rows.push((lm.page, lm.y_top_pt, vec![(lm.x_pt, lm.text.as_str())]));
        }
    }

    rows.into_iter()
        .map(|(_, _, mut segs)| {
            segs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut joined = String::new();
            for (_, txt) in segs {
                joined.push_str(txt);
            }
            joined
        })
        .collect()
}

fn detect_prose_linebreak_mismatch(
    typst_lines: &[crate::model::LineMetric],
    word_lines: &[crate::model::LineMetric],
) -> Option<String> {
    let normalize = |s: &str| -> String {
        let mut out = String::with_capacity(s.len());
        for raw_c in s.chars() {
            if raw_c.is_whitespace() {
                continue;
            }
            match raw_c {
                'ﬀ' => {
                    out.push_str("ff");
                    continue;
                }
                'ﬁ' => {
                    out.push_str("fi");
                    continue;
                }
                'ﬂ' => {
                    out.push_str("fl");
                    continue;
                }
                'ﬃ' => {
                    out.push_str("ffi");
                    continue;
                }
                'ﬄ' => {
                    out.push_str("ffl");
                    continue;
                }
                _ => {}
            }
            let cp = raw_c as u32;
            let c = if (0x2F00..=0x2FD5).contains(&cp) {
                KANGXI_RADICALS_TO_UNIFIED[(cp - 0x2F00) as usize]
            } else {
                raw_c
            };
            let mapped = match c {
                '⺜' => '日',
                '⺝' | '⺼' => '月',
                '⺟' => '母',
                '⺠' => '民',
                '⺩' => '王',
                '⺫' => '目',
                '⻄' => '西',
                '⻅' => '见',
                '⻆' => '角',
                '⻉' => '贝',
                '⻋' => '车',
                '⻓' => '长',
                '⻔' => '门',
                '⻗' => '雨',
                '⻘' | '靑' => '青',
                '⻙' => '韦',
                '⻚' => '页',
                '⻛' => '风',
                '⻜' => '飞',
                '⻝' => '食',
                '⻢' => '马',
                '⻣' => '骨',
                '⻤' => '鬼',
                '⻥' => '鱼',
                '⻦' => '鸟',
                '⻧' => '卤',
                '⻨' => '麦',
                '⻩' | '黃' => '黄',
                '⻪' => '黾',
                '⻬' => '齐',
                '⻮' => '齿',
                '⻰' => '龙',
                '⻳' => '龟',
                '戶' => '户',
                '，' => ',',
                '（' => '(',
                '）' => ')',
                '：' => ':',
                '；' => ';',
                '！' => '!',
                '？' => '?',
                other => other.to_ascii_lowercase(),
            };
            out.push(mapped);
        }
        out
    };

    let t_norm: Vec<String> = coalesce_visual_rows(typst_lines)
        .into_iter()
        .map(|s| normalize(&s))
        .filter(|s| !s.is_empty())
        .collect();
    let w_norm: Vec<String> = coalesce_visual_rows(word_lines)
        .into_iter()
        .map(|s| normalize(&s))
        .filter(|s| !s.is_empty())
        .collect();

    if t_norm.is_empty() || w_norm.is_empty() || t_norm == w_norm {
        return None;
    }

    let max_len = t_norm.len().max(w_norm.len());
    for i in 0..max_len {
        let ts = t_norm.get(i).map(String::as_str).unwrap_or("<EOF>");
        let ws = w_norm.get(i).map(String::as_str).unwrap_or("<EOF>");
        if ts != ws {
            return Some(format!(
                "line_break[{}] mismatch: Typst=\"{}\" vs Word=\"{}\"",
                i + 1,
                ts,
                ws
            ));
        }
    }
    None
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}
