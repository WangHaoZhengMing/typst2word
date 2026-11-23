//! Fenced expressions (`lr(...)`, `mid(...)`) -> `MathNodeIr::Delimited`.

use typst::foundations::{Content, Packed, SequenceElem, StyleChain, SymbolElem};
use typst::math::{LrElem, MidElem};
use typst::text::TextElem;

use crate::math::node::extract_math_node;
use crate::math::{
    MathStyleCtx, extract_sequence_as_node, extract_sequence_slice,
    flatten_sequence_preserving_styles,
};
use ir::{MathFracStyleIr, MathNodeIr};

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

    let elements: Vec<MathNodeIr> = segments
        .iter()
        .map(|seg| extract_sequence_as_node(seg, ctx))
        .collect();

    // Typst automatically wraps every matched `(...)`, `[...]`, `{...}` in `LrElem` (with default
    // `size: 100%`), only stretching glyphs when the inner content is tall enough to exceed the
    // base delimiter height. Word's OMML `<m:d>` aggressively stretches delimiters even around
    // simple superscripts (e.g. `c(Ca²⁺)`, `[Ca²⁺]`), inflating line height. Only emit stretchy
    // `MathNodeIr::Delimited` (`<m:d>`) when sizing is explicit, a `mid(...)` separator is used,
    // a fence is one-sided, or the inner expression contains a tall 2D construct.
    let explicit_size = !lr.size.get(styles).is_one();
    let needs_stretch = explicit_size
        || !separator.is_empty()
        || open.is_empty()
        || close.is_empty()
        || elements.iter().any(node_needs_stretchy_delimiter);

    if !needs_stretch {
        extract_math_node(&flat[0].0, flat[0].1, ctx, out);
        for el in elements {
            match el {
                MathNodeIr::Sequence(children) => out.extend(children),
                other => out.push(other),
            }
        }
        extract_math_node(&flat[end_idx].0, flat[end_idx].1, ctx, out);
        return;
    }

    out.push(MathNodeIr::Delimited {
        open,
        close,
        separator,
        elements,
    });
}

fn node_needs_stretchy_delimiter(node: &MathNodeIr) -> bool {
    match node {
        MathNodeIr::Sequence(children) => children.iter().any(node_needs_stretchy_delimiter),
        MathNodeIr::Fraction {
            num, denom, style, ..
        } => {
            matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar)
                || node_needs_stretchy_delimiter(num)
                || node_needs_stretchy_delimiter(denom)
        }
        MathNodeIr::Matrix { rows, .. } => {
            rows.len() > 1
                || rows
                    .iter()
                    .any(|row| row.iter().any(node_needs_stretchy_delimiter))
        }
        MathNodeIr::EquationArray { rows } => {
            rows.len() > 1 || rows.iter().any(node_needs_stretchy_delimiter)
        }
        MathNodeIr::Nary { .. } | MathNodeIr::GroupChar { .. } | MathNodeIr::Delimited { .. } => {
            true
        }
        MathNodeIr::Limit { base, lower, upper } => {
            lower.is_some() || upper.is_some() || node_needs_stretchy_delimiter(base)
        }
        MathNodeIr::Radical { degree, radicand } => {
            degree.as_deref().is_some_and(node_needs_stretchy_delimiter)
                || node_needs_stretchy_delimiter(radicand)
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            node_needs_stretchy_delimiter(base)
                || sub.as_deref().is_some_and(node_needs_stretchy_delimiter)
                || sup.as_deref().is_some_and(node_needs_stretchy_delimiter)
        }
        MathNodeIr::Func { name, arg } => {
            node_needs_stretchy_delimiter(name) || node_needs_stretchy_delimiter(arg)
        }
        MathNodeIr::Accent { base, .. }
        | MathNodeIr::Bar { body: base, .. }
        | MathNodeIr::Cancel { body: base, .. } => node_needs_stretchy_delimiter(base),
        MathNodeIr::AlignPoint | MathNodeIr::Run(_) => false,
    }
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
