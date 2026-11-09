//! Single math `Content` node router -> delegates to specialized construct extractors.

use typst::foundations::{Content, SequenceElem, StyleChain, StyledElem};
use typst::introspection::TagElem;
use typst::math::{
    AccentElem, AlignPointElem, AttachElem, BinomElem, CancelElem, CasesElem, ClassElem,
    EquationElem, FracElem, LimitsElem, LrElem, MatElem, MidElem, RootElem, ScriptsElem,
    StretchElem, VecElem,
};
use typst::text::LinebreakElem;

use crate::math::accent::{
    extract_accent, extract_cancel, try_extract_bar, try_extract_group_char,
};
use crate::math::attach::extract_attach;
use crate::math::delim::extract_lr;
use crate::math::frac::{extract_binom, extract_frac, extract_root};
use crate::math::matrix::{extract_cases, extract_mat, extract_vec};
use crate::math::nary::{build_nary_node, extract_nary_operator};
use crate::math::symbol::extract_symbol_or_wrapper;
use crate::math::{
    MathStyleCtx, extract_sequence_slice, flatten_sequence_preserving_styles,
};
use ir::MathNodeIr;

pub(crate) fn extract_math_node(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
    out: &mut Vec<MathNodeIr>,
) {
    if content.is::<TagElem>() || content.is::<LinebreakElem>() {
        return;
    }

    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chained = styles.chain(&styled.styles);
        extract_math_node(&styled.child, chained, ctx, out);
        return;
    }

    if let Some(seq) = content.to_packed::<SequenceElem>() {
        let flat = flatten_sequence_preserving_styles(&seq.children, styles);
        extract_sequence_slice(&flat, ctx, out);
        return;
    }

    if let Some(eq) = content.to_packed::<EquationElem>() {
        extract_math_node(&eq.body, styles, ctx, out);
        return;
    }

    // 1. Fractions, Binomials, and Radicals (`frac.rs`)
    if let Some(frac) = content.to_packed::<FracElem>() {
        out.push(extract_frac(frac, styles, ctx));
        return;
    }
    if let Some(binom) = content.to_packed::<BinomElem>() {
        out.push(extract_binom(binom, styles, ctx));
        return;
    }
    if let Some(root) = content.to_packed::<RootElem>() {
        out.push(extract_root(root, styles, ctx));
        return;
    }

    // 2. Attachments, Scripts, Limits (`attach.rs` & `nary.rs`)
    if let Some(attach) = content.to_packed::<AttachElem>() {
        if let Some(nary) = extract_nary_operator(content, styles, ctx) {
            out.push(build_nary_node(&nary, styles, &[], ctx));
            return;
        }
        extract_attach(attach, styles, ctx, out);
        return;
    }
    if let Some(scripts) = content.to_packed::<ScriptsElem>() {
        let mut sub_ctx = ctx;
        sub_ctx.force_limits = Some(false);
        extract_math_node(&scripts.body, styles, sub_ctx, out);
        return;
    }
    if let Some(limits) = content.to_packed::<LimitsElem>() {
        let mut sub_ctx = ctx;
        sub_ctx.force_limits = Some(true);
        extract_math_node(&limits.body, styles, sub_ctx, out);
        return;
    }
    if let Some(stretch) = content.to_packed::<StretchElem>() {
        extract_math_node(&stretch.body, styles, ctx, out);
        return;
    }
    if let Some(cls) = content.to_packed::<ClassElem>() {
        extract_math_node(&cls.body, styles, ctx, out);
        return;
    }

    // 3. Delimiters (`delim.rs`)
    if let Some(lr) = content.to_packed::<LrElem>() {
        extract_lr(lr, styles, ctx, out);
        return;
    }
    if let Some(mid) = content.to_packed::<MidElem>() {
        extract_math_node(&mid.body, styles, ctx, out);
        return;
    }

    // 4. Vectors, Matrices, Cases (`matrix.rs`)
    if let Some(vec_elem) = content.to_packed::<VecElem>() {
        out.push(extract_vec(vec_elem, styles, ctx));
        return;
    }
    if let Some(mat_elem) = content.to_packed::<MatElem>() {
        out.push(extract_mat(mat_elem, styles, ctx));
        return;
    }
    if let Some(cases_elem) = content.to_packed::<CasesElem>() {
        out.push(extract_cases(cases_elem, styles, ctx));
        return;
    }

    // 5. Accents, Over/Underlines, Group Braces, Cancel (`accent.rs`)
    if let Some(acc) = content.to_packed::<AccentElem>() {
        out.push(extract_accent(acc, styles, ctx));
        return;
    }
    if let Some(bar_node) = try_extract_bar(content, styles, ctx) {
        out.push(bar_node);
        return;
    }
    if let Some(group_node) = try_extract_group_char(content, styles, ctx) {
        out.push(group_node);
        return;
    }
    if let Some(cancel) = content.to_packed::<CancelElem>() {
        out.push(extract_cancel(cancel, styles, ctx));
        return;
    }

    // 6. Alignment Point (`&`)
    if content.is::<AlignPointElem>() {
        if ctx.in_eq_arr {
            out.push(MathNodeIr::AlignPoint);
        }
        return;
    }

    // 7. Symbols, Operators, Primes, Text, and Formatting Wrappers (`symbol.rs`)
    extract_symbol_or_wrapper(content, styles, ctx, out);
}
