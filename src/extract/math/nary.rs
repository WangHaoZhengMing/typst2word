//! Big $n$-ary operators (`sum`, `product`, `integral`, ...) -> `MathNodeIr::Nary`.

use typst::foundations::{Content, StyleChain, StyledElem, SymbolElem};
use typst::math::{AlignPointElem, AttachElem, EquationElem, LimitsElem, ScriptsElem, StretchElem};
use typst::text::{LinebreakElem, TextElem};

use crate::extract::math::attach::merge_attach_base;
use crate::extract::math::{MathStyleCtx, extract_math_child, extract_sequence_as_node};
use crate::ir::MathNodeIr;

pub(crate) struct NaryInfo {
    pub(crate) chr: char,
    pub(crate) und_ovr: bool,
    pub(crate) sub: Option<Content>,
    pub(crate) sup: Option<Content>,
}

pub(crate) fn extract_nary_operator(
    item: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> Option<NaryInfo> {
    if let Some(styled) = item.to_packed::<StyledElem>() {
        let chained = styles.chain(&styled.styles);
        let mut info = extract_nary_operator(&styled.child, chained, ctx)?;
        info.sub = info.sub.map(|c| c.styled_with_map(styled.styles.clone()));
        info.sup = info.sup.map(|c| c.styled_with_map(styled.styles.clone()));
        return Some(info);
    }

    if let Some(limits) = item.to_packed::<LimitsElem>() {
        let mut sub_ctx = ctx;
        sub_ctx.force_limits = Some(true);
        return extract_nary_operator(&limits.body, styles, sub_ctx);
    }

    if let Some(scripts) = item.to_packed::<ScriptsElem>() {
        let mut sub_ctx = ctx;
        sub_ctx.force_limits = Some(false);
        return extract_nary_operator(&scripts.body, styles, sub_ctx);
    }

    if let Some(attach) = item.to_packed::<AttachElem>() {
        let merged = merge_attach_base(attach).unwrap_or_else(|| attach.clone());
        if merged.tl.is_set() || merged.bl.is_set() {
            return None;
        }
        let (base_chr, base_force_limits) = extract_nary_base_char(&merged.base, styles)?;
        let force = base_force_limits.or(ctx.force_limits);
        let und_ovr = match force {
            Some(v) => v,
            None => ctx.is_display && !is_integral_char(base_chr),
        };
        let sub = merged
            .b
            .get_cloned(styles)
            .or_else(|| merged.br.get_cloned(styles));
        let sup = merged
            .t
            .get_cloned(styles)
            .or_else(|| merged.tr.get_cloned(styles));
        return Some(NaryInfo {
            chr: base_chr,
            und_ovr,
            sub,
            sup,
        });
    }

    let (base_chr, base_force_limits) = extract_nary_base_char(item, styles)?;
    let force = base_force_limits.or(ctx.force_limits);
    let und_ovr = match force {
        Some(v) => v,
        None => ctx.is_display && !is_integral_char(base_chr),
    };
    Some(NaryInfo {
        chr: base_chr,
        und_ovr,
        sub: None,
        sup: None,
    })
}

pub(crate) fn extract_nary_base_char(
    content: &Content,
    styles: StyleChain,
) -> Option<(char, Option<bool>)> {
    if let Some(styled) = content.to_packed::<StyledElem>() {
        return extract_nary_base_char(&styled.child, styles.chain(&styled.styles));
    }
    if let Some(eq) = content.to_packed::<EquationElem>() {
        return extract_nary_base_char(&eq.body, styles);
    }
    if let Some(limits) = content.to_packed::<LimitsElem>() {
        let (c, _) = extract_nary_base_char(&limits.body, styles)?;
        return Some((c, Some(true)));
    }
    if let Some(scripts) = content.to_packed::<ScriptsElem>() {
        let (c, _) = extract_nary_base_char(&scripts.body, styles)?;
        return Some((c, Some(false)));
    }
    if let Some(stretch) = content.to_packed::<StretchElem>() {
        return extract_nary_base_char(&stretch.body, styles);
    }
    if let Some(sym) = content.to_packed::<SymbolElem>() {
        let mut chars = sym.text.chars();
        let ch = chars.next()?;
        if chars.next().is_none() && is_nary_char(ch) {
            return Some((ch, None));
        }
    }
    if let Some(txt) = content.to_packed::<TextElem>() {
        let mut chars = txt.text.chars();
        let ch = chars.next()?;
        if chars.next().is_none() && is_nary_char(ch) {
            return Some((ch, None));
        }
    }
    None
}

pub(crate) fn is_nary_char(c: char) -> bool {
    matches!(
        c,
        '∑' | '∏'
            | '∐'
            | '∫'
            | '∬'
            | '∭'
            | '∮'
            | '∯'
            | '∰'
            | '⨌'
            | '⋃'
            | '⋂'
            | '⋁'
            | '⋀'
            | '⨁'
            | '⨂'
            | '⨀'
            | '⨄'
            | '⨆'
    )
}

pub(crate) fn is_integral_char(c: char) -> bool {
    matches!(c, '∫' | '∬' | '∭' | '∮' | '∯' | '∰' | '⨌')
}

pub(crate) fn is_nary_operand_terminator(content: &Content) -> bool {
    if content.is::<AlignPointElem>() || content.is::<LinebreakElem>() {
        return true;
    }
    if let Some(sym) = content.to_packed::<SymbolElem>() {
        return matches!(
            sym.text.as_str(),
            "=" | "≠"
                | "<"
                | ">"
                | "≤"
                | "≥"
                | "⩽"
                | "⩾"
                | "≈"
                | "≡"
                | "∼"
                | "≃"
                | "≅"
                | "∝"
                | "∈"
                | "∉"
                | "⊂"
                | "⊆"
                | "⊃"
                | "⊇"
                | "→"
                | "←"
                | "↔"
                | "⇒"
                | "⇐"
                | "⇔"
                | "↦"
                | "≔"
                | "≕"
                | ","
                | ";"
        );
    }
    false
}

pub(crate) fn build_nary_node(
    info: &NaryInfo,
    base_styles: StyleChain,
    operand_slice: &[(Content, StyleChain)],
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let mut script_ctx = ctx;
    script_ctx.is_display = false;

    let sub = info
        .sub
        .as_ref()
        .map(|c| Box::new(extract_math_child(c, base_styles, script_ctx)));
    let sup = info
        .sup
        .as_ref()
        .map(|c| Box::new(extract_math_child(c, base_styles, script_ctx)));
    let operand = Box::new(extract_sequence_as_node(operand_slice, ctx));

    MathNodeIr::Nary {
        chr: info.chr,
        limits_under_over: info.und_ovr,
        sub,
        sup,
        operand,
    }
}
