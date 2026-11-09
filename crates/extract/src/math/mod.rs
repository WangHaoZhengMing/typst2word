//! Typst Math `Content` AST -> Semantic `MathNodeIr` extractor.
//!
//! Translates Typst's structured math elements into our target-independent
//! `MathNodeIr` tree (`ir::equation`), keeping `extract` free of Word XML.
//!
//! Submodules by math construct:
//! - `node`: single-element router
//! - `frac`: fractions (`frac`), binomials (`binom`), radicals (`root`, `sqrt`)
//! - `attach`: subscripts, superscripts, pre-scripts (`attach`), limits (`limits`, `scripts`)
//! - `nary`: big operators (`sum`, `prod`, `int`, ...) and operand collection
//! - `delim`: fenced expressions (`lr`, `mid`)
//! - `matrix`: vectors (`vec`), matrices (`mat`), piecewise cases (`cases`)
//! - `accent`: accents (`hat`, ...), bars (`underline`, `overline`), braces (`underbrace`, ...), `cancel`
//! - `symbol`: symbols, operators (`op`), primes, text runs, and inline style wrappers

mod accent;
mod attach;
mod delim;
mod frac;
mod matrix;
mod nary;
mod node;
mod symbol;

use typst::foundations::{Content, Packed, SequenceElem, StyleChain, StyledElem};
use typst::introspection::TagElem;
use typst::math::{AlignPointElem, EquationElem};
use typst::text::{LinebreakElem, SpaceElem, TextElem};

use ir::{MathNodeIr, MathRunIr, MathStyleIr};
use nary::{build_nary_node, extract_nary_operator, is_nary_operand_terminator};
use node::extract_math_node;

/// Active math styling state propagated down `StyledElem` chains.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MathStyleCtx {
    pub(crate) is_display: bool,
    pub(crate) in_eq_arr: bool,
    pub(crate) force_limits: Option<bool>,
    pub(crate) base_font_size_pt: Option<f64>,
    pub(crate) math_font: Option<&'static str>,
    pub(crate) cjk_font: Option<&'static str>,
}

/// Extract a Typst `EquationElem` into a list of semantic `MathNodeIr` nodes.
pub(crate) fn extract_equation_math_nodes(
    eq: &Packed<EquationElem>,
    styles: StyleChain,
    base_font_size_pt: Option<f64>,
    math_font: Option<&'static str>,
    cjk_font: Option<&'static str>,
) -> Vec<MathNodeIr> {
    let is_block = eq.block.get(styles);
    let ctx = MathStyleCtx {
        is_display: is_block,
        in_eq_arr: false,
        force_limits: None,
        base_font_size_pt,
        math_font,
        cjk_font,
    };
    let mut nodes = Vec::new();
    extract_math_root(&eq.body, styles, ctx, &mut nodes);
    nodes
}

pub(crate) fn extract_math_root(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
    out: &mut Vec<MathNodeIr>,
) {
    // Unwrap outer `StyledElem` at root while keeping styles so we can detect multi-line equations.
    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chained = styles.chain(&styled.styles);
        extract_math_root(&styled.child, chained, ctx, out);
        return;
    }

    if let Some(seq) = content.to_packed::<SequenceElem>() {
        let flat = flatten_sequence_preserving_styles(&seq.children, styles);
        let has_linebreak = flat.iter().any(|(c, _)| c.is::<LinebreakElem>());
        if has_linebreak {
            let mut rows: Vec<Vec<(Content, StyleChain)>> = vec![Vec::new()];
            for (item, st) in &flat {
                if item.is::<LinebreakElem>() {
                    rows.push(Vec::new());
                } else {
                    rows.last_mut().unwrap().push((item.clone(), *st));
                }
            }
            // Filter trailing empty row if the equation ended with `\`
            if rows.len() > 1 && rows.last().is_some_and(|r| r.is_empty()) {
                rows.pop();
            }
            if rows.len() > 1 {
                let mut row_ctx = ctx;
                row_ctx.in_eq_arr = true;
                let row_nodes = rows
                    .iter()
                    .map(|row| extract_sequence_as_node(row, row_ctx))
                    .collect();
                out.push(MathNodeIr::EquationArray { rows: row_nodes });
                return;
            }
        }
        extract_sequence_slice(&flat, ctx, out);
        return;
    }

    extract_math_node(content, styles, ctx, out);
}

pub(crate) fn extract_math_child(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let mut nodes = Vec::new();
    extract_math_node(content, styles, ctx, &mut nodes);
    wrap_nodes(nodes)
}

pub(crate) fn extract_sequence_as_node(
    items: &[(Content, StyleChain)],
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let mut nodes = Vec::new();
    extract_sequence_slice(items, ctx, &mut nodes);
    wrap_nodes(nodes)
}

pub(crate) fn wrap_nodes(mut nodes: Vec<MathNodeIr>) -> MathNodeIr {
    if nodes.len() == 1 {
        nodes.pop().unwrap()
    } else {
        MathNodeIr::Sequence(nodes)
    }
}

pub(crate) fn flatten_sequence_preserving_styles<'a>(
    items: &'a [Content],
    styles: StyleChain<'a>,
) -> Vec<(Content, StyleChain<'a>)> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if item.is::<TagElem>() {
            continue;
        }
        if let Some(seq) = item.to_packed::<SequenceElem>() {
            out.extend(flatten_sequence_preserving_styles(&seq.children, styles));
        } else {
            out.push((item.clone(), styles));
        }
    }
    out
}

pub(crate) fn extract_sequence_slice(
    items: &[(Content, StyleChain)],
    ctx: MathStyleCtx,
    out: &mut Vec<MathNodeIr>,
) {
    let mut i = 0;
    while i < items.len() {
        let (ref item, st) = items[i];

        // Check if this item is an n-ary operator (`AttachElem` with n-ary base,
        // `LimitsElem`/`ScriptsElem` wrapping an n-ary symbol, or a standalone n-ary `SymbolElem`).
        if let Some(nary_info) = extract_nary_operator(item, st, ctx) {
            let mut cursor = i + 1;
            while cursor < items.len() && items[cursor].0.is::<SpaceElem>() {
                cursor += 1;
            }
            let operand_begin = cursor;
            while cursor < items.len() {
                if is_nary_operand_terminator(&items[cursor].0) && cursor > operand_begin {
                    break;
                }
                cursor += 1;
            }
            let operand_slice = &items[operand_begin..cursor];
            out.push(build_nary_node(&nary_info, st, operand_slice, ctx));
            i = cursor;
            continue;
        }

        if item.is::<SpaceElem>() {
            let is_multi_char_text = |c: &Content| {
                c.to_packed::<TextElem>()
                    .is_some_and(|t| t.text.chars().count() > 1)
            };
            let prev_is_word = i > 0 && is_multi_char_text(&items[i - 1].0);
            let next_is_word = i + 1 < items.len() && is_multi_char_text(&items[i + 1].0);
            if prev_is_word || next_is_word {
                out.push(MathNodeIr::Run(MathRunIr {
                    text: "\u{2005}".to_string(),
                    style: Some(MathStyleIr::Plain),
                    font_size_pt: ctx.base_font_size_pt,
                    ..Default::default()
                }));
            }
            i += 1;
            continue;
        }

        extract_math_node(item, st, ctx, out);
        i += 1;
    }
}

pub(crate) fn split_by_align_point<'a>(
    content: &'a Content,
    styles: StyleChain<'a>,
) -> Vec<Vec<(Content, StyleChain<'a>)>> {
    let flat = if let Some(seq) = content.to_packed::<SequenceElem>() {
        flatten_sequence_preserving_styles(&seq.children, styles)
    } else {
        vec![(content.clone(), styles)]
    };

    let mut cols: Vec<Vec<(Content, StyleChain<'a>)>> = vec![Vec::new()];
    for (item, st) in flat {
        if item.is::<AlignPointElem>() {
            cols.push(Vec::new());
        } else {
            cols.last_mut().unwrap().push((item, st));
        }
    }
    cols
}
