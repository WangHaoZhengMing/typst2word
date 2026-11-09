//! Attachments (`attach`: sub/superscripts, pre-scripts, limits) -> `MathNodeIr`.

use typst::foundations::{Content, Packed, StyleChain, StyledElem};
use typst::math::{AttachElem, EquationElem, LimitsElem, OpElem, ScriptsElem};

use crate::math::{MathStyleCtx, extract_math_child};
use ir::MathNodeIr;

pub(crate) fn extract_attach(
    attach: &Packed<AttachElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
    out: &mut Vec<MathNodeIr>,
) {
    let merged = merge_attach_base(attach).unwrap_or_else(|| attach.clone());
    let base_limits = should_attach_as_limits(&merged.base, styles, ctx);

    let sub =
        combine_optional_attachments(merged.b.get_cloned(styles), merged.br.get_cloned(styles));
    let sup =
        combine_optional_attachments(merged.t.get_cloned(styles), merged.tr.get_cloned(styles));
    let tl = merged.tl.get_cloned(styles);
    let bl = merged.bl.get_cloned(styles);

    let mut script_ctx = ctx;
    script_ctx.is_display = false;
    script_ctx.force_limits = None;

    let core_base = extract_math_child(&merged.base, styles, ctx);
    let base_node = if tl.is_some() || bl.is_some() {
        MathNodeIr::PreSubSup {
            base: Box::new(core_base),
            sub: bl
                .as_ref()
                .map(|c| Box::new(extract_math_child(c, styles, script_ctx))),
            sup: tl
                .as_ref()
                .map(|c| Box::new(extract_math_child(c, styles, script_ctx))),
        }
    } else {
        core_base
    };

    if base_limits {
        let lower = sub
            .as_ref()
            .map(|c| Box::new(extract_math_child(c, styles, script_ctx)));
        let upper = sup
            .as_ref()
            .map(|c| Box::new(extract_math_child(c, styles, script_ctx)));
        if lower.is_some() || upper.is_some() {
            out.push(MathNodeIr::Limit {
                base: Box::new(base_node),
                lower,
                upper,
            });
        } else {
            out.push(base_node);
        }
        return;
    }

    let sub_ir = sub
        .as_ref()
        .map(|c| Box::new(extract_math_child(c, styles, script_ctx)));
    let sup_ir = sup
        .as_ref()
        .map(|c| Box::new(extract_math_child(c, styles, script_ctx)));

    if sub_ir.is_some() || sup_ir.is_some() {
        out.push(MathNodeIr::SubSup {
            base: Box::new(base_node),
            sub: sub_ir,
            sup: sup_ir,
        });
    } else {
        out.push(base_node);
    }
}

pub(crate) fn combine_optional_attachments(
    a: Option<Content>,
    b: Option<Content>,
) -> Option<Content> {
    match (a, b) {
        (Some(x), Some(y)) => Some(Content::sequence([x, y])),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

pub(crate) fn should_attach_as_limits(
    base: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> bool {
    if let Some(force) = ctx.force_limits {
        return force;
    }
    if let Some(styled) = base.to_packed::<StyledElem>() {
        return should_attach_as_limits(&styled.child, styles.chain(&styled.styles), ctx);
    }
    if let Some(eq) = base.to_packed::<EquationElem>() {
        return should_attach_as_limits(&eq.body, styles, ctx);
    }
    if let Some(limits) = base.to_packed::<LimitsElem>() {
        return ctx.is_display || limits.inline.get(styles);
    }
    if base.is::<ScriptsElem>() {
        return false;
    }
    if let Some(op) = base.to_packed::<OpElem>() {
        return ctx.is_display && op.limits.get(styles);
    }
    false
}

/// If an `AttachElem`'s base is also an `AttachElem`, merge attachments into the
/// base where possible (e.g. `a_b^c` written as nested attachments).
pub(crate) fn merge_attach_base(attach: &Packed<AttachElem>) -> Option<Packed<AttachElem>> {
    let mut base = &attach.base;
    while let Some(equation) = base.to_packed::<EquationElem>() {
        base = &equation.body;
    }

    let base = base.to_packed::<AttachElem>()?;
    let mut elem = attach.clone();
    let mut base = base.clone();

    macro_rules! merge {
        ($field:ident) => {
            if !base.$field.is_set() && elem.$field.is_set() {
                base.$field = elem.$field.clone();
                elem.$field.unset();
            }
        };
    }

    merge!(t);
    merge!(b);
    merge!(tl);
    merge!(tr);
    merge!(bl);
    merge!(br);

    elem.base = base.pack();
    Some(elem)
}
