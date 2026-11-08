//! Attachments (sub/superscripts, limits, primes) -> `<m:sSub>` / `<m:sSup>` /
//! `<m:sSubSup>` / `<m:limLow>` / `<m:limUpp>`.

use typst::foundations::{Content, Packed, StyleChain, StyledElem};
use typst::math::{AttachElem, EquationElem, LimitsElem, OpElem, ScriptsElem};

use crate::math::MathStyleCtx;
use crate::math::emit::emit_non_empty_element;

pub(crate) fn convert_attach(
    attach: &Packed<AttachElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    let merged = attach.merge_base().unwrap_or_else(|| attach.clone());
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

    // Build core base (possibly wrapped in `<m:sPre>` if `tl` or `bl` is present)
    let mut base_xml = String::new();
    if tl.is_some() || bl.is_some() {
        base_xml.push_str("<m:sPre><m:sub>");
        if let Some(ref bl_c) = bl {
            emit_non_empty_element(bl_c, styles, script_ctx, &mut base_xml, plain);
        }
        base_xml.push_str("</m:sub><m:sup>");
        if let Some(ref tl_c) = tl {
            emit_non_empty_element(tl_c, styles, script_ctx, &mut base_xml, plain);
        }
        base_xml.push_str("</m:sup><m:e>");
        emit_non_empty_element(&merged.base, styles, ctx, &mut base_xml, plain);
        base_xml.push_str("</m:e></m:sPre>");
    } else {
        emit_non_empty_element(&merged.base, styles, ctx, &mut base_xml, plain);
    }

    if base_limits {
        // Emit `<m:limLow>` and/or `<m:limUpp>`
        let mut current = base_xml;
        if let Some(ref sub_c) = sub {
            plain.push_str("_(");
            let mut lim_xml = String::new();
            emit_non_empty_element(sub_c, styles, script_ctx, &mut lim_xml, plain);
            plain.push(')');
            current = format!("<m:limLow><m:e>{current}</m:e><m:lim>{lim_xml}</m:lim></m:limLow>");
        }
        if let Some(ref sup_c) = sup {
            plain.push_str("^(");
            let mut lim_xml = String::new();
            emit_non_empty_element(sup_c, styles, script_ctx, &mut lim_xml, plain);
            plain.push(')');
            current = format!("<m:limUpp><m:e>{current}</m:e><m:lim>{lim_xml}</m:lim></m:limUpp>");
        }
        xml.push_str(&current);
        return;
    }

    match (sub, sup) {
        (Some(sub_c), Some(sup_c)) => {
            xml.push_str("<m:sSubSup><m:e>");
            xml.push_str(&base_xml);
            xml.push_str("</m:e><m:sub>");
            plain.push('_');
            emit_non_empty_element(&sub_c, styles, script_ctx, xml, plain);
            xml.push_str("</m:sub><m:sup>");
            plain.push('^');
            emit_non_empty_element(&sup_c, styles, script_ctx, xml, plain);
            xml.push_str("</m:sup></m:sSubSup>");
        }
        (Some(sub_c), None) => {
            xml.push_str("<m:sSub><m:e>");
            xml.push_str(&base_xml);
            xml.push_str("</m:e><m:sub>");
            plain.push('_');
            emit_non_empty_element(&sub_c, styles, script_ctx, xml, plain);
            xml.push_str("</m:sub></m:sSub>");
        }
        (None, Some(sup_c)) => {
            xml.push_str("<m:sSup><m:e>");
            xml.push_str(&base_xml);
            xml.push_str("</m:e><m:sup>");
            plain.push('^');
            emit_non_empty_element(&sup_c, styles, script_ctx, xml, plain);
            xml.push_str("</m:sup></m:sSup>");
        }
        (None, None) => {
            xml.push_str(&base_xml);
        }
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
