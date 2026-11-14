//! Stage 1: Parallel in-process Typst compilation, IR extraction, `WordDocument`
//! construction, `.docx` serialization, and structural OOXML verification.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use compiler::{TypstWorld, compile_world};
use docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options};
use extract::extract_document_ir;
use image::{ImageBuffer, Rgba};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use word_construct::page_calibration::PageMarginCalibration;
use word_construct::{WordDocument, build_word_document};
use zip::ZipArchive;

use crate::model::{CheckItem, IrSummary, TestCaseSpec};
use crate::verify::{pretty_format_xml, summarize_and_verify};

/// Per-case artifacts produced by Stage 1 compilation before Word PDF export.
pub(super) struct CompiledCaseArtifacts {
    pub spec: TestCaseSpec,
    pub case_dir: PathBuf,
    pub typst_pdf_path: PathBuf,
    pub typst_page_count: usize,
    pub docx_path: PathBuf,
    pub docx_sha256: String,
    pub word_doc: Option<WordDocument>,
    pub ir_json_path: PathBuf,
    pub document_xml_path: PathBuf,
    pub compile_error: Option<String>,
    pub ir_summary: IrSummary,
    pub checks: Vec<CheckItem>,
}

/// Deterministically generate `fixtures/probe.png` if missing so `#image("probe.png")`
/// suites stay self-contained without binary blob churn.
pub(super) fn ensure_probe_png(fixtures: &Path) -> Result<(), String> {
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

/// Compile all selected `TestCaseSpec` items in parallel on a dedicated 16 MiB stack
/// Rayon pool so deeply nested packages (such as CeTZ) never overflow macOS's default
/// 2 MiB worker thread stack in debug/test builds.
pub(super) fn compile_all_cases(
    specs: &[TestCaseSpec],
    cases_root: &Path,
    fixtures: &Path,
) -> Result<Vec<CompiledCaseArtifacts>, String> {
    let compile_pool = rayon::ThreadPoolBuilder::new()
        .stack_size(16 * 1024 * 1024)
        .build()
        .map_err(|e| format!("failed to build rayon compile pool: {e}"))?;

    Ok(compile_pool.install(|| {
        specs
            .par_iter()
            .map(|spec| compile_single_case(spec, cases_root, fixtures))
            .collect()
    }))
}

fn failed_artifact(
    spec: &TestCaseSpec,
    case_dir: PathBuf,
    typst_pdf_path: PathBuf,
    docx_path: PathBuf,
    ir_json_path: PathBuf,
    document_xml_path: PathBuf,
    err: String,
) -> CompiledCaseArtifacts {
    CompiledCaseArtifacts {
        spec: spec.clone(),
        case_dir,
        typst_pdf_path,
        typst_page_count: 0,
        docx_path,
        docx_sha256: String::new(),
        word_doc: None,
        ir_json_path,
        document_xml_path,
        compile_error: Some(err),
        ir_summary: Default::default(),
        checks: Vec::new(),
    }
}

fn compile_single_case(
    spec: &TestCaseSpec,
    cases_root: &Path,
    fixtures: &Path,
) -> CompiledCaseArtifacts {
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
            return failed_artifact(
                spec,
                case_dir,
                typst_pdf_path,
                docx_path,
                ir_json_path,
                document_xml_path,
                format!("Typst compile error: {err}"),
            );
        }
    };

    // Export Typst ground-truth PDF in-process (disable tagged PDF so internal realization markers never interfere).
    let pdf_opts = typst_pdf::PdfOptions {
        tagged: false,
        ..Default::default()
    };
    match typst_pdf::pdf(&compiled.document, &pdf_opts) {
        Ok(pdf_bytes) => {
            if let Err(e) = fs::write(&typst_pdf_path, pdf_bytes) {
                return failed_artifact(
                    spec,
                    case_dir,
                    typst_pdf_path,
                    docx_path,
                    ir_json_path,
                    document_xml_path,
                    format!("Write typst.pdf error: {e}"),
                );
            }
        }
        Err(_) => {
            let cli_out = Command::new("typst")
                .arg("compile")
                .arg("--root")
                .arg(fixtures)
                .arg(&typ_file)
                .arg(&typst_pdf_path)
                .output();
            if let Err(e) = cli_out {
                return failed_artifact(
                    spec,
                    case_dir,
                    typst_pdf_path,
                    docx_path,
                    ir_json_path,
                    document_xml_path,
                    format!("Typst PDF fallback failed: {e}"),
                );
            }
        }
    }

    let ir = extract_document_ir(&compiled, &world);
    let typst_page_count = ir.report.pages_count;
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
            return failed_artifact(
                spec,
                case_dir,
                typst_pdf_path,
                docx_path,
                ir_json_path,
                document_xml_path,
                format!("DOCX writer error: {err}"),
            );
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
        typst_page_count,
        docx_path,
        docx_sha256,
        word_doc: Some(word_doc),
        ir_json_path,
        document_xml_path,
        compile_error: None,
        ir_summary,
        checks,
    }
}
