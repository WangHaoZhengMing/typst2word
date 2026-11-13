//! Stage 2: Multi-Section Batch `.docx` ("Mega-Doc") Word PDF Export + `PDFKit` Slicing.
//!
//! Instead of opening, laying out, saving, and closing 176 separate `.docx` documents in
//! Microsoft Word, this layer:
//! 1. Filters unchanged `.docx` artifacts via per-case `word.pdf.sha256` cache digests.
//! 2. Partitions cache-miss single-page cases into two homogeneous `settings.xml` buckets
//!    (`batch_latin.docx` and `batch_cjk.docx`) assembled at the `WordDocument` section level,
//!    while routing multi-page / header-footer / mirror-margin / field / footnote cases to a
//!    compact standalone list.
//! 3. Exports the 2 Mega-Docs + standalone cases in **one silent AppleScript invocation**.
//! 4. Slices each Mega-Doc PDF back into individual `cases/<id>/word.pdf` files via `PDFKit`,
//!    using recursive binary bisection + standalone fallback if any case unexpectedly wraps
//!    onto 2+ pages in Word so neighbor pages can never shift or misalign.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use docx_writer::{DocxWriteOptions, write_docx_to_bytes_with_options};
use ir::{BlockNode, DocumentSectionIr, InlineNode, LinkTargetIr};
use word_construct::WordDocument;
use word_construct::page_calibration::PageMarginCalibration;

use super::compile::CompiledCaseArtifacts;
use super::inspector::{SplitJob, split_batch_pdfs};
use super::word_applescript::{SilentWordSession, StagedWordJob};

pub(super) fn export_word_pdfs(
    artifacts: &[CompiledCaseArtifacts],
    fresh_word: bool,
    inspector_bin: &Path,
    out_dir: &Path,
) -> Result<(HashMap<String, PathBuf>, bool), String> {
    let mut exported = HashMap::new();
    let mut pending: Vec<&CompiledCaseArtifacts> = Vec::new();

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
            exported.insert(art.spec.id.clone(), word_pdf);
            continue;
        }
        pending.push(art);
    }

    if pending.is_empty() {
        return Ok((exported, false));
    }

    let session = match SilentWordSession::prepare() {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "[visual-test] Microsoft Word unavailable ({err}); degrading remaining cases to IR-only."
            );
            return Ok((exported, true));
        }
    };

    let mut latin_bucket: Vec<&CompiledCaseArtifacts> = Vec::new();
    let mut cjk_bucket: Vec<&CompiledCaseArtifacts> = Vec::new();
    let mut standalone_bucket: Vec<&CompiledCaseArtifacts> = Vec::new();

    if pending.len() == 1 {
        standalone_bucket.extend(pending);
    } else {
        for art in pending {
            if is_batchable_single_page(art)
                && let Some(ref doc) = art.word_doc
            {
                if doc.has_cjk {
                    cjk_bucket.push(art);
                } else {
                    latin_bucket.push(art);
                }
            } else {
                standalone_bucket.push(art);
            }
        }
        if latin_bucket.len() == 1 {
            standalone_bucket.append(&mut latin_bucket);
        }
        if cjk_bucket.len() == 1 {
            standalone_bucket.append(&mut cjk_bucket);
        }
    }

    const MAX_SECTIONS_PER_BATCH: usize = 24;
    const MAX_JOBS_PER_OSASCRIPT: usize = 8;

    let pid = std::process::id();
    let mut staged_jobs: Vec<StagedWordJob> = Vec::new();
    let mut active_batches: Vec<(String, Vec<&CompiledCaseArtifacts>, PathBuf)> = Vec::new();

    for (prefix, bucket) in [("batch_latin", latin_bucket), ("batch_cjk", cjk_bucket)] {
        for (chunk_idx, chunk) in bucket.chunks(MAX_SECTIONS_PER_BATCH).enumerate() {
            if chunk.len() == 1 {
                standalone_bucket.push(chunk[0]);
                continue;
            }
            let tag = format!("{prefix}_{chunk_idx}");
            let chunk_vec: Vec<&CompiledCaseArtifacts> = chunk.to_vec();
            let docx_bytes = assemble_batch_docx_bytes(&chunk_vec)?;
            let st_docx = session
                .stage_dir
                .join(format!("typst2word_stage_{pid}_{tag}.docx"));
            let st_pdf = session
                .stage_dir
                .join(format!("typst2word_stage_{pid}_{tag}.pdf"));
            fs::write(&st_docx, docx_bytes)
                .map_err(|e| format!("write {tag}.docx to sandbox stage: {e}"))?;
            staged_jobs.push(StagedWordJob {
                staged_docx: st_docx,
                staged_pdf: st_pdf.clone(),
            });
            active_batches.push((tag, chunk_vec, st_pdf));
        }
    }

    let mut standalone_targets: Vec<(&CompiledCaseArtifacts, PathBuf)> =
        Vec::with_capacity(standalone_bucket.len());
    for (idx, art) in standalone_bucket.iter().enumerate() {
        let st_docx = session
            .stage_dir
            .join(format!("typst2word_stage_{pid}_solo_{idx}.docx"));
        let st_pdf = session
            .stage_dir
            .join(format!("typst2word_stage_{pid}_solo_{idx}.pdf"));
        fs::copy(&art.docx_path, &st_docx)
            .map_err(|e| format!("stage standalone docx {}: {e}", art.spec.id))?;
        staged_jobs.push(StagedWordJob {
            staged_docx: st_docx,
            staged_pdf: st_pdf.clone(),
        });
        standalone_targets.push((*art, st_pdf));
    }

    for job_chunk in staged_jobs.chunks(MAX_JOBS_PER_OSASCRIPT) {
        let chunk_timeout = Duration::from_secs(60 + job_chunk.len() as u64 * 10);
        if let Err(err) = session.export_jobs(job_chunk, chunk_timeout) {
            eprintln!(
                "[visual-test] Microsoft Word batch export failed ({err}); degrading remaining cases to IR-only."
            );
            return Ok((exported, true));
        }
    }

    // 1. Commit standalone case PDFs.
    for (art, st_pdf) in standalone_targets {
        commit_case_pdf(art, &st_pdf, &mut exported)?;
    }

    // 2. Slice Mega-Doc PDFs via PDFKit, bisecting/falling back if any case overflowed 1 page.
    if !active_batches.is_empty() {
        let split_jobs: Vec<SplitJob> = active_batches
            .iter()
            .map(|(tag, bucket, st_pdf)| SplitJob {
                id: tag.clone(),
                batch_pdf: st_pdf.to_string_lossy().to_string(),
                outputs: bucket
                    .iter()
                    .map(|a| a.case_dir.join("word.pdf").to_string_lossy().to_string())
                    .collect(),
            })
            .collect();

        let split_manifest_path = out_dir.join("split_manifest.json");
        let split_results = split_batch_pdfs(inspector_bin, &split_manifest_path, split_jobs)?;

        let mut bisect_seq = 0usize;
        for (tag, bucket, _) in active_batches {
            match split_results.get(tag.as_str()) {
                Some(res) if res.ok => {
                    mark_bucket_cached(&bucket, &mut exported);
                }
                Some(res) => {
                    let detail = res.error.as_deref().unwrap_or("page count mismatch");
                    eprintln!(
                        "[visual-test] {tag} split check failed ({detail}: expected {}, got {}); isolating overflowing case(s)...",
                        res.expected_pages, res.actual_pages
                    );
                    if !recover_overflow_bucket(
                        &session,
                        inspector_bin,
                        out_dir,
                        &bucket,
                        &mut bisect_seq,
                        &mut exported,
                    )? {
                        return Ok((exported, true));
                    }
                }
                None => {
                    return Err(format!("missing split output for batch {tag}"));
                }
            }
        }
    }

    Ok((exported, false))
}

/// Recursively bisect a batch bucket whose Word PDF page count exceeded `bucket.len()`,
/// isolating the overflowing case(s) into standalone exports so no neighbor case is ever shifted.
fn recover_overflow_bucket(
    session: &SilentWordSession,
    inspector_bin: &Path,
    out_dir: &Path,
    bucket: &[&CompiledCaseArtifacts],
    seq: &mut usize,
    exported: &mut HashMap<String, PathBuf>,
) -> Result<bool, String> {
    const STANDALONE_THRESHOLD: usize = 6;
    if bucket.len() <= STANDALONE_THRESHOLD {
        return export_standalone_slice(session, bucket, seq, exported);
    }

    let mid = bucket.len() / 2;
    let halves = [&bucket[..mid], &bucket[mid..]];
    let mut staged_jobs = Vec::with_capacity(2);
    let mut sub_batches: Vec<(String, &[&CompiledCaseArtifacts], PathBuf)> = Vec::with_capacity(2);

    let pid = std::process::id();
    for half in halves {
        if half.is_empty() {
            continue;
        }
        *seq += 1;
        let tag = format!("bisect_{}", *seq);
        let docx_bytes = assemble_batch_docx_bytes(half)?;
        let st_docx = session
            .stage_dir
            .join(format!("typst2word_stage_{pid}_{tag}.docx"));
        let st_pdf = session
            .stage_dir
            .join(format!("typst2word_stage_{pid}_{tag}.pdf"));
        fs::write(&st_docx, docx_bytes)
            .map_err(|e| format!("write {tag}.docx to sandbox stage: {e}"))?;
        staged_jobs.push(StagedWordJob {
            staged_docx: st_docx,
            staged_pdf: st_pdf.clone(),
        });
        sub_batches.push((tag, half, st_pdf));
    }

    let timeout = Duration::from_secs(35 + (bucket.len() as u64 / 2).min(60));
    if let Err(err) = session.export_jobs(&staged_jobs, timeout) {
        eprintln!("[visual-test] Word bisection export failed ({err}).");
        return Ok(false);
    }

    let split_jobs: Vec<SplitJob> = sub_batches
        .iter()
        .map(|(tag, half, st_pdf)| SplitJob {
            id: tag.clone(),
            batch_pdf: st_pdf.to_string_lossy().to_string(),
            outputs: half
                .iter()
                .map(|a| a.case_dir.join("word.pdf").to_string_lossy().to_string())
                .collect(),
        })
        .collect();

    let manifest_path = out_dir.join("split_bisect_manifest.json");
    let split_results = split_batch_pdfs(inspector_bin, &manifest_path, split_jobs)?;

    for (tag, half, _) in sub_batches {
        if split_results.get(&tag).is_some_and(|r| r.ok) {
            mark_bucket_cached(half, exported);
        } else if !recover_overflow_bucket(session, inspector_bin, out_dir, half, seq, exported)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn export_standalone_slice(
    session: &SilentWordSession,
    cases: &[&CompiledCaseArtifacts],
    seq: &mut usize,
    exported: &mut HashMap<String, PathBuf>,
) -> Result<bool, String> {
    let mut staged_jobs = Vec::with_capacity(cases.len());
    let mut targets = Vec::with_capacity(cases.len());

    let pid = std::process::id();
    for art in cases {
        *seq += 1;
        let tag = format!("fallback_{}", *seq);
        let st_docx = session
            .stage_dir
            .join(format!("typst2word_stage_{pid}_{tag}.docx"));
        let st_pdf = session
            .stage_dir
            .join(format!("typst2word_stage_{pid}_{tag}.pdf"));
        fs::copy(&art.docx_path, &st_docx)
            .map_err(|e| format!("copy fallback docx {}: {e}", art.spec.id))?;
        staged_jobs.push(StagedWordJob {
            staged_docx: st_docx,
            staged_pdf: st_pdf.clone(),
        });
        targets.push((*art, st_pdf));
    }

    let timeout = Duration::from_secs(30 + cases.len() as u64 * 5);
    if let Err(err) = session.export_jobs(&staged_jobs, timeout) {
        eprintln!("[visual-test] Word standalone fallback failed ({err}).");
        return Ok(false);
    }

    for (art, st_pdf) in targets {
        commit_case_pdf(art, &st_pdf, exported)?;
    }
    Ok(true)
}

fn commit_case_pdf(
    art: &CompiledCaseArtifacts,
    staged_pdf: &Path,
    exported: &mut HashMap<String, PathBuf>,
) -> Result<(), String> {
    if staged_pdf.exists() {
        let final_pdf = art.case_dir.join("word.pdf");
        let hash_file = art.case_dir.join("word.pdf.sha256");
        fs::copy(staged_pdf, &final_pdf)
            .map_err(|e| format!("copy word.pdf back for {}: {e}", art.spec.id))?;
        let _ = fs::write(hash_file, &art.docx_sha256);
        exported.insert(art.spec.id.clone(), final_pdf);
    }
    Ok(())
}

fn mark_bucket_cached(
    bucket: &[&CompiledCaseArtifacts],
    exported: &mut HashMap<String, PathBuf>,
) {
    for art in bucket {
        let final_pdf = art.case_dir.join("word.pdf");
        if final_pdf.exists() {
            let hash_file = art.case_dir.join("word.pdf.sha256");
            let _ = fs::write(hash_file, &art.docx_sha256);
            exported.insert(art.spec.id.clone(), final_pdf);
        }
    }
}

fn assemble_batch_docx_bytes(bucket: &[&CompiledCaseArtifacts]) -> Result<Vec<u8>, String> {
    let first_doc = bucket
        .first()
        .and_then(|a| a.word_doc.as_ref())
        .ok_or_else(|| "cannot assemble empty batch bucket".to_string())?;

    let mut sections = Vec::with_capacity(bucket.len().saturating_sub(1));
    for art in &bucket[1..] {
        let doc = art
            .word_doc
            .as_ref()
            .ok_or_else(|| format!("missing WordDocument for {}", art.spec.id))?;
        sections.push(DocumentSectionIr {
            page_setup: doc.page_setup.clone(),
            blocks: doc.blocks.clone(),
        });
    }

    let has_cetz_canvas = bucket
        .iter()
        .filter_map(|a| a.word_doc.as_ref())
        .any(|d| d.has_cetz_canvas);

    let batch_doc = WordDocument {
        sections,
        metadata: Default::default(),
        page_setup: first_doc.page_setup.clone(),
        blocks: first_doc.blocks.clone(),
        has_cjk: first_doc.has_cjk,
        has_cetz_canvas,
    };

    write_docx_to_bytes_with_options(
        &batch_doc,
        DocxWriteOptions {
            page_margin_calibration: PageMarginCalibration::Preserve,
        },
    )
    .map_err(|e| format!("serialize multi-section batch docx: {e}"))
}

/// Determine whether a compiled case can safely join a 1-page-per-section Mega-Doc without
/// leaking document-global OOXML settings (`mirrorMargins`, `<w:background>`, headers/footers,
/// footnotes, dynamic page fields, or bookmarks) or wrapping the 1pt section-break carrier paragraph.
fn is_batchable_single_page(art: &CompiledCaseArtifacts) -> bool {
    if art.compile_error.is_some() || art.typst_page_count != 1 {
        return false;
    }
    let Some(ref doc) = art.word_doc else {
        return false;
    };
    if !doc.sections.is_empty() {
        return false;
    }

    let ps = &doc.page_setup;
    if ps.mirror_margins
        || ps.different_first_page
        || ps.different_odd_even
        || !ps.header.is_empty()
        || !ps.footer.is_empty()
        || ps.fill.is_some()
        || ps.margin_bottom_pt < 2.0
        || ps.width_pt < 80.0
        || ps.height_pt < 80.0
    {
        return false;
    }

    // Auto-sized, full-height, or bottom-anchored layouts can wrap the 1pt section-break carrier <w:p>.
    let src = art.spec.typst_source.as_str();
    if src.contains("height: 100%")
        || src.contains("width: auto")
        || src.contains("height: auto")
        || src.contains("align(bottom")
        || src.contains("place(bottom")
    {
        return false;
    }

    !blocks_have_isolated_constructs(&doc.blocks)
}

fn blocks_have_isolated_constructs(blocks: &[BlockNode]) -> bool {
    blocks.iter().any(block_has_isolated_construct)
}

fn block_has_isolated_construct(block: &BlockNode) -> bool {
    match block {
        BlockNode::PageBreak
        | BlockNode::Columns { .. }
        | BlockNode::Outline { .. }
        | BlockNode::Bibliography { .. } => true,
        BlockNode::Title { content, .. } => inlines_have_isolated_constructs(content),
        BlockNode::Heading {
            content, bookmark, ..
        }
        | BlockNode::Paragraph {
            content, bookmark, ..
        } => bookmark.is_some() || inlines_have_isolated_constructs(content),
        BlockNode::List { items, .. } => items
            .iter()
            .any(|item| blocks_have_isolated_constructs(&item.blocks)),
        BlockNode::Enum { items, .. } => items
            .iter()
            .any(|item| blocks_have_isolated_constructs(&item.blocks)),
        BlockNode::Terms { items, .. } => items.iter().any(|item| {
            inlines_have_isolated_constructs(&item.term)
                || blocks_have_isolated_constructs(&item.description)
        }),
        BlockNode::Container(c) => blocks_have_isolated_constructs(&c.body),
        BlockNode::Table(tbl) => tbl.rows.iter().any(|row| {
            row.cells
                .iter()
                .any(|cell| blocks_have_isolated_constructs(&cell.blocks))
        }),
        BlockNode::Quote {
            body, attribution, ..
        } => {
            blocks_have_isolated_constructs(body)
                || attribution
                    .as_deref()
                    .is_some_and(inlines_have_isolated_constructs)
        }
        BlockNode::Figure {
            body,
            caption,
            bookmark,
            ..
        } => {
            bookmark.is_some()
                || blocks_have_isolated_constructs(body)
                || caption
                    .as_deref()
                    .is_some_and(inlines_have_isolated_constructs)
        }
        BlockNode::CodeBlock { .. }
        | BlockNode::EquationBlock(_)
        | BlockNode::ShapeBlock(_)
        | BlockNode::CetzCanvas(_)
        | BlockNode::ImageBlock(_)
        | BlockNode::HorizontalRule { .. }
        | BlockNode::ColumnBreak
        | BlockNode::UnsupportedBlock(_) => false,
    }
}

fn inlines_have_isolated_constructs(inlines: &[InlineNode]) -> bool {
    inlines.iter().any(|inl| match inl {
        InlineNode::Field { .. } | InlineNode::Footnote { .. } | InlineNode::Ref { .. } => true,
        InlineNode::Link { target, content } => {
            matches!(target, Some(LinkTargetIr::Anchor(_)))
                || inlines_have_isolated_constructs(content)
        }
        InlineNode::Citation { content, .. } => inlines_have_isolated_constructs(content),
        InlineNode::Text(_)
        | InlineNode::EquationInline(_)
        | InlineNode::ShapeInline(_)
        | InlineNode::CetzInline(_)
        | InlineNode::ImageInline(_)
        | InlineNode::LineBreak
        | InlineNode::Tab
        | InlineNode::HorizontalSpace { .. }
        | InlineNode::UnsupportedInline(_) => false,
    })
}
