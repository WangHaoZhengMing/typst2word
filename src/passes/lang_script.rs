//! Pass 3 (`PipelinePassKind::LanguageAndScriptRecognition`):
//! Single source of truth for CJK ideograph / East Asian symbol / Latin script classification
//! across `TextRunIr` (`TextScriptClassIr`) and `MathNodeIr` (`MathRunIr::text_script`).

use crate::ir::{BlockNode, DocumentIr, InlineNode, MathNodeIr, TextScriptClassIr};
use crate::passes::{visit_document_block_lists_mut, visit_document_inline_lists_mut};

/// Run Pass 3 (`LanguageAndScriptRecognition`) across all text runs, code blocks, and equations in `doc`.
pub fn run_language_and_script_pass(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut |inlines| {
        for inline in inlines.iter_mut() {
            match inline {
                InlineNode::Text(run) => {
                    run.text_script = classify_text_script(&run.text);
                }
                InlineNode::EquationInline(eq) => {
                    classify_math_nodes_script(&mut eq.body);
                }
                _ => {}
            }
        }
    });

    visit_document_block_lists_mut(doc, &mut |blocks| {
        for block in blocks.iter_mut() {
            match block {
                BlockNode::CodeBlock { lines, .. } => {
                    for line in lines.iter_mut() {
                        for run in line.iter_mut() {
                            run.text_script = classify_text_script(&run.text);
                        }
                    }
                }
                BlockNode::EquationBlock(eq) => {
                    classify_math_nodes_script(&mut eq.body);
                }
                _ => {}
            }
        }
    });
}

/// Exhaustively classify a text string into one of the four [`TextScriptClassIr`] variants.
#[inline]
pub fn classify_text_script(text: &str) -> TextScriptClassIr {
    let mut has_cjk = false;
    let mut has_ea_symbol = false;
    for ch in text.chars() {
        if is_cjk_char(ch) {
            has_cjk = true;
        } else if is_east_asian_symbol_char(ch) {
            has_ea_symbol = true;
        }
        if has_cjk && has_ea_symbol {
            return TextScriptClassIr::CjkWithEastAsianSymbols;
        }
    }
    match (has_cjk, has_ea_symbol) {
        (true, true) => TextScriptClassIr::CjkWithEastAsianSymbols,
        (true, false) => TextScriptClassIr::CjkIdeographs,
        (false, true) => TextScriptClassIr::LatinWithEastAsianSymbols,
        (false, false) => TextScriptClassIr::LatinOnly,
    }
}

/// Check if a character is a CJK ideograph, CJK punctuation, Hiragana/Katakana, or fullwidth glyph.
#[inline]
pub fn is_cjk_char(c: char) -> bool {
    word_construct::is_cjk_or_fullwidth_char(c)
        || matches!(c as u32, 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

/// Check if a character is a general punctuation / arrow / mathematical symbol (`U+2000..=U+2BFF`)
/// that requires `w:hint="eastAsia"` when rendered with a CJK font in Word.
#[inline]
pub fn is_east_asian_symbol_char(c: char) -> bool {
    ('\u{2000}'..='\u{2BFF}').contains(&c)
}

fn classify_math_nodes_script(nodes: &mut [MathNodeIr]) {
    for node in nodes {
        classify_math_node_script(node);
    }
}

fn classify_math_node_script(node: &mut MathNodeIr) {
    match node {
        MathNodeIr::Sequence(children) => {
            classify_math_nodes_script(children);
        }
        MathNodeIr::Fraction { num, denom, .. } => {
            classify_math_node_script(num);
            classify_math_node_script(denom);
        }
        MathNodeIr::Radical { degree, radicand } => {
            if let Some(d) = degree {
                classify_math_node_script(d);
            }
            classify_math_node_script(radicand);
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            classify_math_node_script(base);
            if let Some(s) = sub {
                classify_math_node_script(s);
            }
            if let Some(s) = sup {
                classify_math_node_script(s);
            }
        }
        MathNodeIr::Limit { base, lower, upper } => {
            classify_math_node_script(base);
            if let Some(l) = lower {
                classify_math_node_script(l);
            }
            if let Some(u) = upper {
                classify_math_node_script(u);
            }
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            if let Some(s) = sub {
                classify_math_node_script(s);
            }
            if let Some(s) = sup {
                classify_math_node_script(s);
            }
            classify_math_node_script(operand);
        }
        MathNodeIr::Delimited { elements, .. } => {
            classify_math_nodes_script(elements);
        }
        MathNodeIr::Matrix { rows } => {
            for row in rows {
                classify_math_nodes_script(row);
            }
        }
        MathNodeIr::EquationArray { rows } => {
            classify_math_nodes_script(rows);
        }
        MathNodeIr::Accent { base, .. } => {
            classify_math_node_script(base);
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            classify_math_node_script(body);
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            classify_math_node_script(body);
            if let Some(ann) = annotation {
                classify_math_node_script(ann);
            }
        }
        MathNodeIr::AlignPoint => {}
        MathNodeIr::Run(r) => {
            r.text_script = classify_text_script(&r.text);
        }
    }
}
