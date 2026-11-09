//! Pass 1 (`PipelinePassKind::FontRecognition`):
//! Normalizes raw Typst font family names (`STSong` -> `Songti SC`, code `DejaVu Sans Mono` -> `Consolas`)
//! and classifies each `TextRunIr`'s [`FontFamilyClassIr`].

use crate::ir::{BlockNode, DocumentIr, FontFamilyClassIr, InlineNode, TextRunIr};
use crate::passes::{visit_document_block_lists_mut, visit_document_inline_lists_mut};

/// Run Pass 1 (`FontRecognition`) across all text runs and code blocks in `doc`.
pub fn run_font_recognition_pass(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut |inlines| {
        for inline in inlines.iter_mut() {
            if let InlineNode::Text(run) = inline {
                normalize_and_classify_text_run_font(run);
            }
        }
    });

    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            if let BlockNode::CodeBlock { lines, .. } = block {
                for line in lines.iter_mut() {
                    for run in line.iter_mut() {
                        normalize_and_classify_text_run_font(run);
                    }
                }
            }
        }
    });
}

/// Normalize a single `TextRunIr`'s `font_family` and classify its `font_class`.
pub fn normalize_and_classify_text_run_font(run: &mut TextRunIr) {
    match (&run.font_family, run.is_code) {
        (Some(raw_family), _) => {
            let normalized = normalize_font_family_name(raw_family, run.is_code);
            run.font_class = classify_font_family(Some(&normalized), run.is_code);
            run.font_family = Some(normalized);
        }
        (None, true) => {
            run.font_family = Some("Consolas".to_string());
            run.font_class = FontFamilyClassIr::Monospace;
        }
        (None, false) => {
            run.font_class = FontFamilyClassIr::LatinProportional;
        }
    }
}

/// Normalize Typst/OS font family aliases into canonical Word font names.
pub fn normalize_font_family_name(family: &str, is_code: bool) -> String {
    let trimmed = family.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("dejavu sans mono") {
        return "Consolas".to_string();
    }
    if is_code && trimmed.is_empty() {
        return "Consolas".to_string();
    }
    match trimmed {
        "Hei" | "STHeiti" => "Heiti SC".to_string(),
        "Kai" | "STKaiti" => "Kaiti SC".to_string(),
        "Song" | "STSong" => "Songti SC".to_string(),
        "Pingfang sc" | "pingfang sc" | "PingFang SC" => "PingFang SC".to_string(),
        other => other.to_string(),
    }
}

/// Exhaustively classify a font family name into [`FontFamilyClassIr`].
pub fn classify_font_family(font_family: Option<&str>, is_code: bool) -> FontFamilyClassIr {
    let Some(fam) = font_family else {
        return if is_code {
            FontFamilyClassIr::Monospace
        } else {
            FontFamilyClassIr::LatinProportional
        };
    };

    if let Some(paradigm) = word_construct::lookup_known_font(fam) {
        return match paradigm {
            word_construct::FontParadigm::AppleCjkQuirk { .. }
            | word_construct::FontParadigm::LegacyCjkSerif { .. } => FontFamilyClassIr::CjkSerif,
            word_construct::FontParadigm::StandardWin { is_cjk: true, .. } => {
                FontFamilyClassIr::CjkUiSans
            }
            word_construct::FontParadigm::Monospace { .. } => FontFamilyClassIr::Monospace,
            word_construct::FontParadigm::StandardWin { is_cjk: false, .. } => {
                if is_code {
                    FontFamilyClassIr::Monospace
                } else {
                    FontFamilyClassIr::LatinProportional
                }
            }
        };
    }

    let lower = fam.trim().to_ascii_lowercase();
    if lower.contains("song")
        || lower.contains("kai")
        || lower.contains("fang")
        || lower.contains("宋体")
        || lower.contains("楷体")
        || lower.contains("仿宋")
    {
        FontFamilyClassIr::CjkSerif
    } else if lower.contains("hei")
        || lower.contains("pingfang")
        || lower.contains("yahei")
        || lower.contains("dengxian")
        || lower.contains("sim")
        || lower.contains("黑体")
    {
        FontFamilyClassIr::CjkUiSans
    } else if is_code
        || lower.contains("mono")
        || lower.contains("consolas")
        || lower.contains("menlo")
        || lower.contains("courier")
    {
        FontFamilyClassIr::Monospace
    } else {
        FontFamilyClassIr::LatinProportional
    }
}
