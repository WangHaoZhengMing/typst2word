//! Pass 1 (`WordPipelinePassKind::FontRecognition`):
//! Classifies each `TextRunIr`'s [`FontFamilyClassIr`] without altering or substituting
//! the original `font_family` string extracted from Typst.

use ir::{
    BlockNode, DocumentIr, FontFamilyClassIr, InlineNode, TextRunIr,
    visit_document_block_lists_mut, visit_document_inline_lists_mut,
};

use crate::{FontParadigm, lookup_known_font};

/// Run Pass 1 (`FontRecognition`) across all text runs and code blocks in `doc`.
pub fn run_font_recognition_pass(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut |inlines| {
        for inline in inlines.iter_mut() {
            if let InlineNode::Text(run) = inline {
                classify_text_run_font(run);
            }
        }
    });

    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            if let BlockNode::CodeBlock { lines, .. } = block {
                for line in lines.iter_mut() {
                    for run in line.iter_mut() {
                        classify_text_run_font(run);
                    }
                }
            }
        }
    });
}

/// Classify a single `TextRunIr`'s `font_class` without modifying `run.font_family`.
pub fn classify_text_run_font(run: &mut TextRunIr) {
    // 不改变原字体：原文档使用什么字体（包括 "Hei"、"STHeiti"、"Song"、"STSong"、"DejaVu Sans Mono" 等），
    // 转换器均严格保留 `run.font_family` 原始字体名称，绝不在代码中篡改或替换原字体。
    // 但是：在识别 Word 字体分类（`FontFamilyClassIr`）与行盒度量模型（`FontParadigm`）时，
    // 需要将传统中文字体简称（如 `"Hei"` -> `"Heiti SC"`、`"Song"` -> `"Songti SC"`）仅用于内部度量与分类查询。
    run.font_class = classify_font_family(run.font_family.as_deref(), run.is_code);
}

/// Resolve a font family alias solely for internal metric/classification lookup
/// (never written back to `TextRunIr::font_family`).
pub fn resolve_font_alias_for_metrics(family: &str) -> &str {
    match family.trim() {
        "Hei" | "STHeiti" => "Heiti SC",
        "Kai" | "STKaiti" => "Kaiti SC",
        "Song" | "STSong" => "Songti SC",
        "Pingfang sc" | "pingfang sc" | "PingFang SC" => "PingFang SC",
        other => other,
    }
}

/// Exhaustively classify a font family name into [`FontFamilyClassIr`].
pub fn classify_font_family(font_family: Option<&str>, is_code: bool) -> FontFamilyClassIr {
    let Some(raw_fam) = font_family else {
        return if is_code {
            FontFamilyClassIr::Monospace
        } else {
            FontFamilyClassIr::LatinProportional
        };
    };

    let fam = resolve_font_alias_for_metrics(raw_fam);

    if let Some(paradigm) = lookup_known_font(fam) {
        return match paradigm {
            FontParadigm::AppleCjkQuirk { .. } | FontParadigm::LegacyCjkSerif { .. } => {
                FontFamilyClassIr::CjkSerif
            }
            FontParadigm::StandardWin { is_cjk: true, .. } => FontFamilyClassIr::CjkUiSans,
            FontParadigm::Monospace { .. } => FontFamilyClassIr::Monospace,
            FontParadigm::StandardWin { is_cjk: false, .. } => {
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
