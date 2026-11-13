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

use compiler::{CompiledTypstDocument, TypstWorld};
use typst::foundations::{Content, NativeElement, Packed, SequenceElem, StyleChain, StyledElem};
use typst::introspection::TagElem;
use typst::math::{
    AccentElem, AlignPointElem, AttachElem, BinomElem, CancelElem, CasesElem, ClassElem,
    EquationElem, FracElem, LimitsElem, LrElem, MatElem, MidElem, RootElem, ScriptsElem,
    StretchElem, VecElem,
};
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

/// Extract a Typst `EquationElem` into a list of semantic `MathNodeIr` nodes,
/// first running `RealizationKind::Math` so `#show` rules, `context`, and nested
/// `$...$` equation references are evaluated faithfully.
pub(crate) fn extract_equation_math_nodes(
    compiled: &CompiledTypstDocument,
    world: &TypstWorld,
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
    let realized_body = realize_math_content_deep(compiled, world, &eq.body, styles, 0);
    let mut nodes = Vec::new();
    extract_math_root(&realized_body, styles, ctx, &mut nodes);
    nodes
}

fn realize_math_content_deep(
    compiled: &CompiledTypstDocument,
    world: &TypstWorld,
    content: &Content,
    styles: StyleChain,
    depth: usize,
) -> Content {
    if depth > 16 {
        return content.clone();
    }
    let parent_map = styles.to_map();
    let Some(pairs) = compiled.realize_math_owned(world, content, Some(&parent_map)) else {
        return content.clone();
    };
    let mut out_items = Vec::with_capacity(pairs.len());
    for (item, item_map) in pairs {
        if item.is::<TagElem>() {
            continue;
        }
        let item_chain = StyleChain::new(&item_map);
        let transformed =
            realize_math_compound_children(compiled, world, &item, item_chain, depth + 1);
        let is_structural = transformed.is::<LinebreakElem>()
            || transformed.is::<AlignPointElem>()
            || transformed.is::<SpaceElem>();
        if !is_structural && item_map != parent_map {
            out_items.push(transformed.styled_with_map(item_map));
            continue;
        }
        out_items.push(transformed);
    }
    match out_items.len() {
        0 => Content::empty(),
        1 => out_items.pop().unwrap(),
        _ => Content::sequence(out_items),
    }
}

fn realize_math_compound_children(
    compiled: &CompiledTypstDocument,
    world: &TypstWorld,
    item: &Content,
    styles: StyleChain,
    depth: usize,
) -> Content {
    let rec = |sub: &Content| realize_math_content_deep(compiled, world, sub, styles, depth);
    if let Some(frac) = item.to_packed::<FracElem>() {
        let mut c = (**frac).clone();
        c.num = rec(&frac.num);
        c.denom = rec(&frac.denom);
        return c.pack().spanned(frac.span());
    }
    if let Some(binom) = item.to_packed::<BinomElem>() {
        let mut c = (**binom).clone();
        c.upper = rec(&binom.upper);
        c.lower = binom.lower.iter().map(rec).collect();
        return c.pack().spanned(binom.span());
    }
    if let Some(root) = item.to_packed::<RootElem>() {
        let mut c = (**root).clone();
        c.radicand = rec(&root.radicand);
        if let Some(idx) = root.index.get_ref(styles) {
            c.index.set(Some(rec(idx)));
        }
        return c.pack().spanned(root.span());
    }
    if let Some(attach) = item.to_packed::<AttachElem>() {
        let mut c = (**attach).clone();
        c.base = rec(&attach.base);
        if let Some(v) = attach.t.get_ref(styles) {
            c.t.set(Some(rec(v)));
        }
        if let Some(v) = attach.b.get_ref(styles) {
            c.b.set(Some(rec(v)));
        }
        if let Some(v) = attach.tl.get_ref(styles) {
            c.tl.set(Some(rec(v)));
        }
        if let Some(v) = attach.bl.get_ref(styles) {
            c.bl.set(Some(rec(v)));
        }
        if let Some(v) = attach.tr.get_ref(styles) {
            c.tr.set(Some(rec(v)));
        }
        if let Some(v) = attach.br.get_ref(styles) {
            c.br.set(Some(rec(v)));
        }
        return c.pack().spanned(attach.span());
    }
    if let Some(scripts) = item.to_packed::<ScriptsElem>() {
        let mut c = (**scripts).clone();
        c.body = rec(&scripts.body);
        return c.pack().spanned(scripts.span());
    }
    if let Some(limits) = item.to_packed::<LimitsElem>() {
        let mut c = (**limits).clone();
        c.body = rec(&limits.body);
        return c.pack().spanned(limits.span());
    }
    if let Some(stretch) = item.to_packed::<StretchElem>() {
        let mut c = (**stretch).clone();
        c.body = rec(&stretch.body);
        return c.pack().spanned(stretch.span());
    }
    if let Some(cls) = item.to_packed::<ClassElem>() {
        let mut c = (**cls).clone();
        c.body = rec(&cls.body);
        return c.pack().spanned(cls.span());
    }
    if let Some(lr) = item.to_packed::<LrElem>() {
        let mut c = (**lr).clone();
        c.body = rec(&lr.body);
        return c.pack().spanned(lr.span());
    }
    if let Some(mid) = item.to_packed::<MidElem>() {
        let mut c = (**mid).clone();
        c.body = rec(&mid.body);
        return c.pack().spanned(mid.span());
    }
    if let Some(acc) = item.to_packed::<AccentElem>() {
        let mut c = (**acc).clone();
        c.base = rec(&acc.base);
        return c.pack().spanned(acc.span());
    }
    if let Some(can) = item.to_packed::<CancelElem>() {
        let mut c = (**can).clone();
        c.body = rec(&can.body);
        return c.pack().spanned(can.span());
    }
    if let Some(vec_el) = item.to_packed::<VecElem>() {
        let mut c = (**vec_el).clone();
        c.children = vec_el.children.iter().map(rec).collect();
        return c.pack().spanned(vec_el.span());
    }
    if let Some(mat_el) = item.to_packed::<MatElem>() {
        let mut c = (**mat_el).clone();
        c.rows = mat_el
            .rows
            .iter()
            .map(|row| row.iter().map(rec).collect())
            .collect();
        return c.pack().spanned(mat_el.span());
    }
    if let Some(cases_el) = item.to_packed::<CasesElem>() {
        let mut c = (**cases_el).clone();
        c.children = cases_el.children.iter().map(rec).collect();
        return c.pack().spanned(cases_el.span());
    }
    item.clone()
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
        } else if let Some(styled) = item.to_packed::<StyledElem>()
            && let Some(seq) = styled.child.to_packed::<SequenceElem>()
        {
            for sub in flatten_sequence_preserving_styles(&seq.children, styles) {
                let is_structural = sub.0.is::<LinebreakElem>()
                    || sub.0.is::<AlignPointElem>()
                    || sub.0.is::<SpaceElem>();
                if is_structural {
                    out.push(sub);
                } else {
                    out.push((sub.0.styled_with_map(styled.styles.clone()), styles));
                }
            }
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
