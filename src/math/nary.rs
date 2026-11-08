//! N-ary operators (`sum`, `product`, `integral`, ...) -> `<m:nary>`.

use typst::foundations::{Content, StyleChain, StyledElem, SymbolElem};
use typst::math::{AlignPointElem, AttachElem, EquationElem, LimitsElem, ScriptsElem, StretchElem};
use typst::text::{LinebreakElem, TextElem};

use crate::math::node::convert_math_node;
use crate::math::{MathStyleCtx, convert_sequence_slice};
use crate::xml::escape_xml_attr;

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
        let merged = attach.merge_base().unwrap_or_else(|| attach.clone());
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

pub(crate) fn emit_nary_xml(
    info: &NaryInfo,
    base_styles: StyleChain,
    operand_slice: &[(Content, StyleChain)],
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    let lim_loc = if info.und_ovr { "undOvr" } else { "subSup" };
    let sub_hide = if info.sub.is_none() { "1" } else { "0" };
    let sup_hide = if info.sup.is_none() { "1" } else { "0" };
    let mut chr_buf = [0u8; 4];
    let chr_str = escape_xml_attr(info.chr.encode_utf8(&mut chr_buf));

    plain.push(info.chr);

    xml.push_str("<m:nary><m:naryPr>");
    xml.push_str(&format!(r#"<m:chr m:val="{chr_str}"/>"#));
    xml.push_str(&format!(r#"<m:limLoc m:val="{lim_loc}"/>"#));
    xml.push_str(&format!(r#"<m:subHide m:val="{sub_hide}"/>"#));
    xml.push_str(&format!(r#"<m:supHide m:val="{sup_hide}"/>"#));
    xml.push_str("</m:naryPr><m:sub>");
    if let Some(ref sub_c) = info.sub {
        plain.push_str("_(");
        let mut sub_ctx = ctx;
        sub_ctx.is_display = false;
        convert_math_node(sub_c, base_styles, sub_ctx, xml, plain);
        plain.push(')');
    }
    xml.push_str("</m:sub><m:sup>");
    if let Some(ref sup_c) = info.sup {
        plain.push_str("^(");
        let mut sup_ctx = ctx;
        sup_ctx.is_display = false;
        convert_math_node(sup_c, base_styles, sup_ctx, xml, plain);
        plain.push(')');
    }
    xml.push_str("</m:sup><m:e>");
    if operand_slice.is_empty() {
        xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
    } else {
        plain.push(' ');
        let mut op_xml = String::new();
        convert_sequence_slice(operand_slice, ctx, &mut op_xml, plain);
        if op_xml.is_empty() {
            op_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
        }
        xml.push_str(&op_xml);
    }
    xml.push_str("</m:e></m:nary>");
}
