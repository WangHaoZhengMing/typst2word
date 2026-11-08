//! Fenced expressions (`lr(...)`, `mid(...)`) -> `MathNodeIr::Delimited`.

use typst::foundations::{Content, Packed, SequenceElem, StyleChain, SymbolElem};
use typst::math::{LrElem, MidElem};
use typst::text::TextElem;

use crate::extract::math::{
    MathStyleCtx, extract_sequence_as_node, extract_sequence_slice,
    flatten_sequence_preserving_styles,
};
use crate::ir::MathNodeIr;

pub(crate) fn extract_lr(
    lr: &Packed<LrElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
    out: &mut Vec<MathNodeIr>,
) {
    let flat = if let Some(seq) = lr.body.to_packed::<SequenceElem>() {
        flatten_sequence_preserving_styles(&seq.children, styles)
    } else {
        vec![(lr.body.clone(), styles)]
    };

    if flat.is_empty() {
        return;
    }

    let mut start_idx = 0;
    let mut end_idx = flat.len();
    let mut open = String::new();
    let mut close = String::new();

    if let Some(open_c) = extract_delimiter_char(&flat[0].0, true) {
        open = open_c.to_string();
        start_idx = 1;
    }
    if end_idx > start_idx {
        if let Some(close_c) = extract_delimiter_char(&flat[end_idx - 1].0, false) {
            close = close_c.to_string();
            end_idx -= 1;
        }
    }

    if open.is_empty() && close.is_empty() {
        extract_sequence_slice(&flat, ctx, out);
        return;
    }

    let inner = &flat[start_idx..end_idx];

    // Check if there are `MidElem` separators (e.g. `{ x mid(|) x > 0 }`).
    let mut separator = String::new();
    let mut segments: Vec<Vec<(Content, StyleChain)>> = vec![Vec::new()];
    for (item, st) in inner {
        if let Some(mid) = item.to_packed::<MidElem>() {
            let mid_t = mid.body.plain_text().trim().to_string();
            if separator.is_empty() && !mid_t.is_empty() {
                separator = mid_t;
            }
            segments.push(Vec::new());
        } else {
            segments.last_mut().unwrap().push((item.clone(), *st));
        }
    }

    let elements = segments
        .iter()
        .map(|seg| extract_sequence_as_node(seg, ctx))
        .collect();

    out.push(MathNodeIr::Delimited {
        open,
        close,
        separator,
        elements,
    });
}

pub(crate) fn extract_delimiter_char(content: &Content, is_open: bool) -> Option<char> {
    if let Some(sym) = content.to_packed::<SymbolElem>() {
        let mut chars = sym.text.chars();
        let c = chars.next()?;
        if chars.next().is_none() && is_fence_char(c, is_open) {
            return Some(c);
        }
    }
    if let Some(txt) = content.to_packed::<TextElem>() {
        let mut chars = txt.text.chars();
        let c = chars.next()?;
        if chars.next().is_none() && is_fence_char(c, is_open) {
            return Some(c);
        }
    }
    None
}

pub(crate) fn is_fence_char(c: char, _is_open: bool) -> bool {
    matches!(
        c,
        '(' | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '|'
            | '‖'
            | '⌊'
            | '⌋'
            | '⌈'
            | '⌉'
            | '⟨'
            | '⟩'
            | '⟪'
            | '⟫'
            | '⟦'
            | '⟧'
            | '⟮'
            | '⟯'
    )
}
