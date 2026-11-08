//! Native Typst Math `Content` AST -> Microsoft Word Office Math (`<m:oMath>`) converter.
//!
//! Translates Typst's structured math elements (`EquationElem`, `FracElem`, `BinomElem`,
//! `AttachElem`, `PrimesElem`, `ScriptsElem`, `LimitsElem`, `StretchElem`, `RootElem`,
//! `LrElem`, `MidElem`, `VecElem`, `MatElem`, `CasesElem`, `AccentElem`, `UnderlineElem`,
//! `OverlineElem`, `UnderbraceElem`, `OverbraceElem`, `UnderbracketElem`, `OverbracketElem`,
//! `UnderparenElem`, `OverparenElem`, `UndershellElem`, `OvershellElem`, `CancelElem`,
//! `OpElem`, `ClassElem`, `AlignPointElem`, `SymbolElem`, `TextElem`, `StyledElem`)
//! directly into Word OMML (`<m:oMath>`).

use typst::foundations::{Content, Packed, SequenceElem, StyleChain, StyledElem, SymbolElem};
use typst::introspection::TagElem;
use typst::layout::{AlignElem, BoxElem, HElem, HideElem, PadElem, Spacing};
use typst::math::{
    AccentElem, AlignPointElem, AttachElem, BinomElem, CancelElem, CasesElem, ClassElem,
    EquationElem, FracElem, FracStyle, LimitsElem, LrElem, MatElem, MidElem, OpElem, OverbraceElem,
    OverbracketElem, OverlineElem as MathOverlineElem, OverparenElem, OvershellElem, PrimesElem,
    RootElem, ScriptsElem, StretchElem, UnderbraceElem, UnderbracketElem,
    UnderlineElem as MathUnderlineElem, UnderparenElem, UndershellElem, VecElem,
};
use typst::model::{EmphElem, LinkMarker, RefElem, StrongElem};
use typst::text::{
    HighlightElem, LinebreakElem, OverlineElem as TextOverlineElem, RawElem, SmartQuoteElem,
    SpaceElem, StrikeElem, SubElem, SuperElem, TextElem, UnderlineElem as TextUnderlineElem,
};
use typst::visualize::Paint;

/// Active math styling state propagated down `StyledElem` chains.
#[derive(Debug, Clone, Copy, Default)]
struct MathStyleCtx {
    is_display: bool,
    in_eq_arr: bool,
    force_limits: Option<bool>,
}

/// Convert a Typst `EquationElem` into `(omml_xml, plain_fallback_text)`.
///
/// The returned `omml_xml` is a complete `<m:oMath>...</m:oMath>` element ready
/// to be placed either directly in a paragraph (inline) or inside `<m:oMathPara>` (block).
pub fn convert_equation_to_omml(eq: &Packed<EquationElem>, styles: StyleChain) -> (String, String) {
    let is_block = eq.block.get(styles);
    let ctx = MathStyleCtx {
        is_display: is_block,
        in_eq_arr: false,
        force_limits: None,
    };

    let mut inner_xml = String::new();
    let mut plain_text = String::new();

    convert_math_root(&eq.body, styles, ctx, &mut inner_xml, &mut plain_text);

    if inner_xml.trim().is_empty() {
        inner_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
    }

    let omml = format!("<m:oMath>{inner_xml}</m:oMath>");
    (omml, plain_text.trim().to_string())
}

/// Convert any standalone math `Content` node into `<m:oMath>...</m:oMath>`.
pub fn convert_math_content_to_omml(
    content: &Content,
    is_display: bool,
    styles: StyleChain,
) -> (String, String) {
    if let Some(eq) = content.to_packed::<EquationElem>() {
        return convert_equation_to_omml(eq, styles);
    }
    let ctx = MathStyleCtx {
        is_display,
        in_eq_arr: false,
        force_limits: None,
    };
    let mut inner_xml = String::new();
    let mut plain_text = String::new();
    convert_math_root(content, styles, ctx, &mut inner_xml, &mut plain_text);
    if inner_xml.trim().is_empty() {
        inner_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
    }
    let omml = format!("<m:oMath>{inner_xml}</m:oMath>");
    (omml, plain_text.trim().to_string())
}

fn convert_math_root(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    // Unwrap outer StyledElem at root while keeping styles so we can detect multi-line equations.
    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chained = styles.chain(&styled.styles);
        convert_math_root(&styled.child, chained, ctx, xml, plain);
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
                xml.push_str("<m:eqArr><m:eqArrPr><m:maxDist m:val=\"1\"/></m:eqArrPr>");
                let mut row_ctx = ctx;
                row_ctx.in_eq_arr = true;
                for (i, row) in rows.iter().enumerate() {
                    if i > 0 {
                        plain.push_str(" \\ ");
                    }
                    xml.push_str("<m:e>");
                    let mut row_xml = String::new();
                    convert_sequence_slice(row, row_ctx, &mut row_xml, plain);
                    if row_xml.is_empty() {
                        row_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
                    }
                    xml.push_str(&row_xml);
                    xml.push_str("</m:e>");
                }
                xml.push_str("</m:eqArr>");
                return;
            }
        }
        convert_sequence_slice(&flat, ctx, xml, plain);
        return;
    }

    convert_math_node(content, styles, ctx, xml, plain);
}

fn flatten_sequence_preserving_styles<'a>(
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

fn convert_sequence_slice(
    items: &[(Content, StyleChain)],
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    let mut i = 0;
    while i < items.len() {
        let (ref item, st) = items[i];

        // Check if this item is an n-ary operator (either `AttachElem` with n-ary base,
        // `LimitsElem`/`ScriptsElem` wrapping an n-ary symbol, or a standalone n-ary `SymbolElem`).
        if let Some(nary_info) = extract_nary_operator(item, st, ctx) {
            // Collect the operand `<m:e>` following the n-ary operator in this sequence.
            let operand_start = i + 1;
            // Skip leading space after n-ary operator
            let mut cursor = operand_start;
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
            emit_nary_xml(&nary_info, st, operand_slice, ctx, xml, plain);
            i = cursor;
            continue;
        }

        convert_math_node(item, st, ctx, xml, plain);
        i += 1;
    }
}

struct NaryInfo {
    chr: char,
    und_ovr: bool,
    sub: Option<Content>,
    sup: Option<Content>,
}

fn extract_nary_operator(
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

fn extract_nary_base_char(content: &Content, styles: StyleChain) -> Option<(char, Option<bool>)> {
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

fn is_nary_char(c: char) -> bool {
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

fn is_integral_char(c: char) -> bool {
    matches!(c, '∫' | '∬' | '∭' | '∮' | '∯' | '∰' | '⨌')
}

fn is_nary_operand_terminator(content: &Content) -> bool {
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

fn emit_nary_xml(
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

fn convert_math_node(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    if content.is::<TagElem>() {
        return;
    }

    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chained = styles.chain(&styled.styles);
        convert_math_node(&styled.child, chained, ctx, xml, plain);
        return;
    }

    if let Some(seq) = content.to_packed::<SequenceElem>() {
        let flat = flatten_sequence_preserving_styles(&seq.children, styles);
        convert_sequence_slice(&flat, ctx, xml, plain);
        return;
    }

    if let Some(eq) = content.to_packed::<EquationElem>() {
        convert_math_node(&eq.body, styles, ctx, xml, plain);
        return;
    }

    // 1. Fraction (`FracElem`)
    if let Some(frac) = content.to_packed::<FracElem>() {
        let style = frac.style.get(styles);
        let frac_type = match style {
            FracStyle::Vertical => "bar",
            FracStyle::Skewed => "skw",
            FracStyle::Horizontal => "lin",
        };
        xml.push_str(&format!(
            r#"<m:f><m:fPr><m:type m:val="{frac_type}"/></m:fPr><m:num>"#
        ));
        plain.push('(');
        emit_non_empty_element(&frac.num, styles, ctx, xml, plain);
        plain.push_str(")/(");
        xml.push_str("</m:num><m:den>");
        emit_non_empty_element(&frac.denom, styles, ctx, xml, plain);
        plain.push(')');
        xml.push_str("</m:den></m:f>");
        return;
    }

    // 2. Binomial (`BinomElem`)
    if let Some(binom) = content.to_packed::<BinomElem>() {
        xml.push_str(
            r#"<m:d><m:dPr><m:begChr m:val="("/><m:endChr m:val=")"/></m:dPr><m:e><m:f><m:fPr><m:type m:val="noBar"/></m:fPr><m:num>"#,
        );
        plain.push_str("binom(");
        emit_non_empty_element(&binom.upper, styles, ctx, xml, plain);
        plain.push_str(", ");
        xml.push_str("</m:num><m:den>");
        let mut den_xml = String::new();
        for (idx, item) in binom.lower.iter().enumerate() {
            if idx > 0 {
                den_xml.push_str("<m:r><m:t>, </m:t></m:r>");
                plain.push_str(", ");
            }
            convert_math_node(item, styles, ctx, &mut den_xml, plain);
        }
        if den_xml.is_empty() {
            den_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
        }
        xml.push_str(&den_xml);
        plain.push(')');
        xml.push_str("</m:den></m:f></m:e></m:d>");
        return;
    }

    // 3. Root / Square Root (`RootElem`)
    if let Some(root) = content.to_packed::<RootElem>() {
        if let Some(index) = root.index.get_ref(styles) {
            xml.push_str(r#"<m:rad><m:radPr><m:degHide m:val="0"/></m:radPr><m:deg>"#);
            plain.push_str("root(");
            emit_non_empty_element(index, styles, ctx, xml, plain);
            plain.push_str(", ");
            xml.push_str("</m:deg><m:e>");
            emit_non_empty_element(&root.radicand, styles, ctx, xml, plain);
            plain.push(')');
            xml.push_str("</m:e></m:rad>");
        } else {
            xml.push_str(r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>"#);
            plain.push_str("sqrt(");
            emit_non_empty_element(&root.radicand, styles, ctx, xml, plain);
            plain.push(')');
            xml.push_str("</m:e></m:rad>");
        }
        return;
    }

    // 4. Attachments (`AttachElem`: scripts, limits, pre-scripts)
    if let Some(attach) = content.to_packed::<AttachElem>() {
        if let Some(nary) = extract_nary_operator(content, styles, ctx) {
            emit_nary_xml(&nary, styles, &[], ctx, xml, plain);
            return;
        }
        convert_attach(attach, styles, ctx, xml, plain);
        return;
    }

    // 5. Primes (`PrimesElem`)
    if let Some(primes) = content.to_packed::<PrimesElem>() {
        let s = match primes.count {
            0 => String::new(),
            1 => "′".to_string(),
            2 => "″".to_string(),
            3 => "‴".to_string(),
            4 => "⁗".to_string(),
            n => "′".repeat(n),
        };
        if !s.is_empty() {
            plain.push_str(&s);
            emit_math_run(&s, styles, false, xml);
        }
        return;
    }

    // 6. Scripts / Limits / Stretch / Class wrappers
    if let Some(scripts) = content.to_packed::<ScriptsElem>() {
        let mut sub_ctx = ctx;
        sub_ctx.force_limits = Some(false);
        convert_math_node(&scripts.body, styles, sub_ctx, xml, plain);
        return;
    }
    if let Some(limits) = content.to_packed::<LimitsElem>() {
        let mut sub_ctx = ctx;
        sub_ctx.force_limits = Some(true);
        convert_math_node(&limits.body, styles, sub_ctx, xml, plain);
        return;
    }
    if let Some(stretch) = content.to_packed::<StretchElem>() {
        convert_math_node(&stretch.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(cls) = content.to_packed::<ClassElem>() {
        convert_math_node(&cls.body, styles, ctx, xml, plain);
        return;
    }

    // 7. Left-Right Delimiters (`LrElem`) & Mid (`MidElem`)
    if let Some(lr) = content.to_packed::<LrElem>() {
        convert_lr(lr, styles, ctx, xml, plain);
        return;
    }
    if let Some(mid) = content.to_packed::<MidElem>() {
        convert_math_node(&mid.body, styles, ctx, xml, plain);
        return;
    }

    // 8. Column Vector (`VecElem`)
    if let Some(vec_elem) = content.to_packed::<VecElem>() {
        let delim = vec_elem.delim.get(styles);
        let beg = delim.open().map(|c| c.to_string()).unwrap_or_default();
        let end = delim.close().map(|c| c.to_string()).unwrap_or_default();
        xml.push_str(&format!(
            r#"<m:d><m:dPr><m:begChr m:val="{}"/><m:endChr m:val="{}"/></m:dPr><m:e><m:m>"#,
            escape_xml_attr(&beg),
            escape_xml_attr(&end)
        ));
        plain.push_str(&beg);
        for (idx, item) in vec_elem.children.iter().enumerate() {
            if idx > 0 {
                plain.push_str("; ");
            }
            xml.push_str("<m:mr><m:e>");
            emit_non_empty_element(item, styles, ctx, xml, plain);
            xml.push_str("</m:e></m:mr>");
        }
        plain.push_str(&end);
        xml.push_str("</m:m></m:e></m:d>");
        return;
    }

    // 9. Matrix (`MatElem`)
    if let Some(mat_elem) = content.to_packed::<MatElem>() {
        let delim = mat_elem.delim.get(styles);
        let beg = delim.open().map(|c| c.to_string()).unwrap_or_default();
        let end = delim.close().map(|c| c.to_string()).unwrap_or_default();
        xml.push_str(&format!(
            r#"<m:d><m:dPr><m:begChr m:val="{}"/><m:endChr m:val="{}"/></m:dPr><m:e><m:m>"#,
            escape_xml_attr(&beg),
            escape_xml_attr(&end)
        ));
        plain.push_str(&beg);
        for (r_idx, row) in mat_elem.rows.iter().enumerate() {
            if r_idx > 0 {
                plain.push_str("; ");
            }
            xml.push_str("<m:mr>");
            for (c_idx, cell) in row.iter().enumerate() {
                if c_idx > 0 {
                    plain.push_str(", ");
                }
                xml.push_str("<m:e>");
                emit_non_empty_element(cell, styles, ctx, xml, plain);
                xml.push_str("</m:e>");
            }
            xml.push_str("</m:mr>");
        }
        plain.push_str(&end);
        xml.push_str("</m:m></m:e></m:d>");
        return;
    }

    // 10. Cases (`CasesElem`)
    if let Some(cases_elem) = content.to_packed::<CasesElem>() {
        let delim = cases_elem.delim.get(styles);
        let reverse = cases_elem.reverse.get(styles);
        let (beg, end) = if reverse {
            (
                String::new(),
                delim
                    .close()
                    .or_else(|| delim.open())
                    .unwrap_or('}')
                    .to_string(),
            )
        } else {
            (delim.open().unwrap_or('{').to_string(), String::new())
        };
        xml.push_str(&format!(
            r#"<m:d><m:dPr><m:begChr m:val="{}"/><m:endChr m:val="{}"/></m:dPr><m:e><m:m>"#,
            escape_xml_attr(&beg),
            escape_xml_attr(&end)
        ));
        plain.push_str("cases(");
        for (idx, branch) in cases_elem.children.iter().enumerate() {
            if idx > 0 {
                plain.push_str(", ");
            }
            let cols = split_by_align_point(branch, styles);
            xml.push_str("<m:mr>");
            for (c_idx, col_items) in cols.iter().enumerate() {
                if c_idx > 0 {
                    plain.push(' ');
                }
                xml.push_str("<m:e>");
                let mut col_xml = String::new();
                convert_sequence_slice(col_items, ctx, &mut col_xml, plain);
                if col_xml.is_empty() {
                    col_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
                }
                xml.push_str(&col_xml);
                xml.push_str("</m:e>");
            }
            xml.push_str("</m:mr>");
        }
        plain.push(')');
        xml.push_str("</m:m></m:e></m:d>");
        return;
    }

    // 11. Accent (`AccentElem`)
    if let Some(acc) = content.to_packed::<AccentElem>() {
        let mut buf = [0u8; 4];
        let chr_str = escape_xml_attr(acc.accent.0.encode_utf8(&mut buf));
        xml.push_str(&format!(
            r#"<m:acc><m:accPr><m:chr m:val="{chr_str}"/></m:accPr><m:e>"#
        ));
        emit_non_empty_element(&acc.base, styles, ctx, xml, plain);
        plain.push(acc.accent.0);
        xml.push_str("</m:e></m:acc>");
        return;
    }

    // 12. Overline / Underline (Math & Text variants)
    if let Some(ul) = content.to_packed::<MathUnderlineElem>() {
        emit_bar(&ul.body, "bot", styles, ctx, xml, plain);
        return;
    }
    if let Some(ol) = content.to_packed::<MathOverlineElem>() {
        emit_bar(&ol.body, "top", styles, ctx, xml, plain);
        return;
    }
    if let Some(ul) = content.to_packed::<TextUnderlineElem>() {
        emit_bar(&ul.body, "bot", styles, ctx, xml, plain);
        return;
    }
    if let Some(ol) = content.to_packed::<TextOverlineElem>() {
        emit_bar(&ol.body, "top", styles, ctx, xml, plain);
        return;
    }

    // 13. Under/Over Braces, Brackets, Parens, Shells
    if let Some(el) = content.to_packed::<UnderbraceElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⏟',
            true,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<OverbraceElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⏞',
            false,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<UnderbracketElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⎵',
            true,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<OverbracketElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⎴',
            false,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<UnderparenElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⏝',
            true,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<OverparenElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⏜',
            false,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<UndershellElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⏡',
            true,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }
    if let Some(el) = content.to_packed::<OvershellElem>() {
        emit_group_chr(
            &el.body,
            el.annotation.get_ref(styles),
            '⏠',
            false,
            styles,
            ctx,
            xml,
            plain,
        );
        return;
    }

    // 14. Cancel (`CancelElem`)
    if let Some(cancel) = content.to_packed::<CancelElem>() {
        let cross = cancel.cross.get(styles);
        let inverted = cancel.inverted.get(styles);
        xml.push_str("<m:borderBox><m:borderBoxPr>");
        xml.push_str(
            r#"<m:hideTop m:val="1"/><m:hideBot m:val="1"/><m:hideLeft m:val="1"/><m:hideRight m:val="1"/>"#,
        );
        if cross {
            xml.push_str(r#"<m:strikeBLTR m:val="1"/><m:strikeTLBR m:val="1"/>"#);
        } else if inverted {
            xml.push_str(r#"<m:strikeTLBR m:val="1"/>"#);
        } else {
            xml.push_str(r#"<m:strikeBLTR m:val="1"/>"#);
        }
        xml.push_str("</m:borderBoxPr><m:e>");
        emit_non_empty_element(&cancel.body, styles, ctx, xml, plain);
        xml.push_str("</m:e></m:borderBox>");
        return;
    }

    // 15. Text Operator (`OpElem`, e.g. `sin`, `cos`, `lim`, `max`, `det`, etc.)
    if let Some(op) = content.to_packed::<OpElem>() {
        let op_str = op.text.plain_text();
        plain.push_str(&op_str);
        emit_math_run(&op_str, styles, true, xml);
        return;
    }

    // 16. Alignment Point (`AlignPointElem`: `&`)
    if content.is::<AlignPointElem>() {
        if ctx.in_eq_arr {
            xml.push_str("<m:r><m:t>&amp;</m:t></m:r>");
        }
        return;
    }

    // 17. Linebreak (`LinebreakElem`: `\`)
    if content.is::<LinebreakElem>() {
        plain.push('\n');
        return;
    }

    // 18. Spaces (`SpaceElem`, `HElem`)
    if content.is::<SpaceElem>() {
        plain.push(' ');
        return;
    }
    if let Some(h) = content.to_packed::<HElem>() {
        if let Spacing::Rel(rel) = h.amount {
            if !rel.is_zero() {
                plain.push(' ');
                emit_math_run("\u{2009}", styles, true, xml);
            }
        }
        return;
    }

    // 19. Symbols (`SymbolElem`) and Text (`TextElem`, `RawElem`, `SmartQuoteElem`)
    if let Some(sym) = content.to_packed::<SymbolElem>() {
        plain.push_str(&sym.text);
        emit_math_run(&sym.text, styles, false, xml);
        return;
    }
    if let Some(txt) = content.to_packed::<TextElem>() {
        plain.push_str(&txt.text);
        // Multi-character text strings in math (e.g. `"if"`) default to upright unless italic is forced
        let is_quoted_word = txt.text.chars().count() > 1;
        emit_math_run(&txt.text, styles, is_quoted_word, xml);
        return;
    }
    if content.is::<RawElem>() {
        let t = content.plain_text();
        plain.push_str(&t);
        emit_math_run(&t, styles, true, xml);
        return;
    }
    if content.is::<SmartQuoteElem>() {
        plain.push('"');
        emit_math_run("\"", styles, true, xml);
        return;
    }

    // 20. Inline formatting wrappers (`StrongElem`, `EmphElem`, `SubElem`, `SuperElem`, `StrikeElem`, `HighlightElem`, `BoxElem`, `PadElem`, `AlignElem`, `HideElem`, `LinkMarker`, `RefElem`)
    if let Some(strong) = content.to_packed::<StrongElem>() {
        let styled = strong.body.clone().set(EquationElem::bold, true);
        convert_math_node(&styled, styles, ctx, xml, plain);
        return;
    }
    if let Some(emph) = content.to_packed::<EmphElem>() {
        let styled = emph.body.clone().set(EquationElem::italic, Some(true));
        convert_math_node(&styled, styles, ctx, xml, plain);
        return;
    }
    if let Some(sub) = content.to_packed::<SubElem>() {
        convert_math_node(&sub.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(sup) = content.to_packed::<SuperElem>() {
        convert_math_node(&sup.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(strike) = content.to_packed::<StrikeElem>() {
        convert_math_node(&strike.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(hl) = content.to_packed::<HighlightElem>() {
        convert_math_node(&hl.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(bx) = content.to_packed::<BoxElem>() {
        if let Some(body) = bx.body.get_ref(styles) {
            convert_math_node(body, styles, ctx, xml, plain);
        }
        return;
    }
    if let Some(pad) = content.to_packed::<PadElem>() {
        convert_math_node(&pad.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(align) = content.to_packed::<AlignElem>() {
        convert_math_node(&align.body, styles, ctx, xml, plain);
        return;
    }
    if content.is::<HideElem>() {
        return;
    }
    if let Some(link) = content.to_packed::<LinkMarker>() {
        convert_math_node(&link.body, styles, ctx, xml, plain);
        return;
    }
    if let Some(r) = content.to_packed::<RefElem>() {
        let t = format!("@{}", r.target.resolve());
        plain.push_str(&t);
        emit_math_run(&t, styles, true, xml);
        return;
    }

    // Fallback for any other content inside math: extract plain text
    let fallback_txt = content.plain_text();
    if !fallback_txt.is_empty() {
        plain.push_str(&fallback_txt);
        emit_math_run(&fallback_txt, styles, false, xml);
    }
}

fn convert_attach(
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

fn combine_optional_attachments(a: Option<Content>, b: Option<Content>) -> Option<Content> {
    match (a, b) {
        (Some(x), Some(y)) => Some(Content::sequence([x, y])),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

fn should_attach_as_limits(base: &Content, styles: StyleChain, ctx: MathStyleCtx) -> bool {
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

fn convert_lr(
    lr: &Packed<LrElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
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
    let mut beg_chr = String::new();
    let mut end_chr = String::new();

    if let Some(open_c) = extract_delimiter_char(&flat[0].0, true) {
        beg_chr = open_c.to_string();
        start_idx = 1;
    }
    if end_idx > start_idx {
        if let Some(close_c) = extract_delimiter_char(&flat[end_idx - 1].0, false) {
            end_chr = close_c.to_string();
            end_idx -= 1;
        }
    }

    if beg_chr.is_empty() && end_chr.is_empty() {
        convert_sequence_slice(&flat, ctx, xml, plain);
        return;
    }

    let inner = &flat[start_idx..end_idx];

    // Check if there are `MidElem` separators (e.g. `{ x mid(|) x > 0 }`)
    let mut sep_chr = String::new();
    let mut segments: Vec<Vec<(Content, StyleChain)>> = vec![Vec::new()];
    for (item, st) in inner {
        if let Some(mid) = item.to_packed::<MidElem>() {
            let mid_t = mid.body.plain_text().trim().to_string();
            if sep_chr.is_empty() && !mid_t.is_empty() {
                sep_chr = mid_t;
            }
            segments.push(Vec::new());
        } else {
            segments.last_mut().unwrap().push((item.clone(), *st));
        }
    }

    xml.push_str("<m:d><m:dPr>");
    xml.push_str(&format!(
        r#"<m:begChr m:val="{}"/>"#,
        escape_xml_attr(&beg_chr)
    ));
    if !sep_chr.is_empty() {
        xml.push_str(&format!(
            r#"<m:sepChr m:val="{}"/>"#,
            escape_xml_attr(&sep_chr)
        ));
    }
    xml.push_str(&format!(
        r#"<m:endChr m:val="{}"/>"#,
        escape_xml_attr(&end_chr)
    ));
    xml.push_str("</m:dPr>");

    plain.push_str(&beg_chr);
    for (idx, seg) in segments.iter().enumerate() {
        if idx > 0 {
            plain.push_str(" | ");
        }
        xml.push_str("<m:e>");
        let mut seg_xml = String::new();
        convert_sequence_slice(seg, ctx, &mut seg_xml, plain);
        if seg_xml.is_empty() {
            seg_xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
        }
        xml.push_str(&seg_xml);
        xml.push_str("</m:e>");
    }
    plain.push_str(&end_chr);
    xml.push_str("</m:d>");
}

fn extract_delimiter_char(content: &Content, is_open: bool) -> Option<char> {
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

fn is_fence_char(c: char, _is_open: bool) -> bool {
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

fn split_by_align_point<'a>(
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

fn emit_bar(
    body: &Content,
    pos: &str,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    xml.push_str(&format!(
        r#"<m:bar><m:barPr><m:pos m:val="{pos}"/></m:barPr><m:e>"#
    ));
    emit_non_empty_element(body, styles, ctx, xml, plain);
    xml.push_str("</m:e></m:bar>");
}

#[allow(clippy::too_many_arguments)]
fn emit_group_chr(
    body: &Content,
    annotation: &Option<Content>,
    chr: char,
    is_bottom: bool,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    let mut buf = [0u8; 4];
    let chr_str = escape_xml_attr(chr.encode_utf8(&mut buf));
    let pos = if is_bottom { "bot" } else { "top" };
    let vert_jc = if is_bottom { "top" } else { "bot" };

    let mut group_xml = format!(
        r#"<m:groupChr><m:groupChrPr><m:chr m:val="{chr_str}"/><m:pos m:val="{pos}"/><m:vertJc m:val="{vert_jc}"/></m:groupChrPr><m:e>"#
    );
    emit_non_empty_element(body, styles, ctx, &mut group_xml, plain);
    group_xml.push_str("</m:e></m:groupChr>");

    if let Some(ann) = annotation {
        let mut ann_ctx = ctx;
        ann_ctx.is_display = false;
        let mut ann_xml = String::new();
        plain.push_str(" (");
        emit_non_empty_element(ann, styles, ann_ctx, &mut ann_xml, plain);
        plain.push(')');
        if is_bottom {
            xml.push_str(&format!(
                "<m:limLow><m:e>{group_xml}</m:e><m:lim>{ann_xml}</m:lim></m:limLow>"
            ));
        } else {
            xml.push_str(&format!(
                "<m:limUpp><m:e>{group_xml}</m:e><m:lim>{ann_xml}</m:lim></m:limUpp>"
            ));
        }
    } else {
        xml.push_str(&group_xml);
    }
}

fn emit_non_empty_element(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
    xml: &mut String,
    plain: &mut String,
) {
    let mut buf = String::new();
    convert_math_node(content, styles, ctx, &mut buf, plain);
    if buf.is_empty() {
        xml.push_str("<m:r><m:t>&#x200B;</m:t></m:r>");
    } else {
        xml.push_str(&buf);
    }
}

fn emit_math_run(text: &str, styles: StyleChain, default_upright: bool, xml: &mut String) {
    if text.is_empty() {
        return;
    }

    let bold = styles.get(EquationElem::bold);
    let italic = styles.get(EquationElem::italic);
    let variant = styles.get(EquationElem::variant);
    let variant_dbg = variant.map(|v| format!("{v:?}")).unwrap_or_default();

    let scr = match variant_dbg.as_str() {
        "DoubleStruck" => Some("double-struck"),
        "Chancery" | "Roundhand" => Some("script"),
        "Fraktur" => Some("fraktur"),
        "Monospace" => Some("monospace"),
        "SansSerif" => Some("sans-serif"),
        _ => None,
    };

    let sty = match (bold, italic) {
        (true, Some(true)) => Some("bi"),
        (true, Some(false)) => Some("b"),
        (true, None) => {
            if default_upright {
                Some("b")
            } else {
                Some("bi")
            }
        }
        (false, Some(false)) => Some("p"),
        (false, Some(true)) => Some("i"),
        (false, None) => {
            if default_upright {
                Some("p")
            } else {
                None
            }
        }
    };

    let color_hex = match styles.get_ref(TextElem::fill) {
        Paint::Solid(c) => {
            let [r, g, b, _] = c.to_vec4_u8();
            if r == 0 && g == 0 && b == 0 {
                None
            } else {
                Some(format!("{r:02X}{g:02X}{b:02X}"))
            }
        }
        _ => None,
    };

    xml.push_str("<m:r>");
    if scr.is_some() || sty.is_some() {
        xml.push_str("<m:rPr>");
        if let Some(scr_val) = scr {
            xml.push_str(&format!(r#"<m:scr m:val="{scr_val}"/>"#));
        }
        if let Some(sty_val) = sty {
            xml.push_str(&format!(r#"<m:sty m:val="{sty_val}"/>"#));
        }
        xml.push_str("</m:rPr>");
    }
    if let Some(hex) = color_hex {
        xml.push_str(&format!(r#"<w:rPr><w:color w:val="{hex}"/></w:rPr>"#));
    }
    xml.push_str("<m:t>");
    xml.push_str(&escape_xml_text(text));
    xml.push_str("</m:t></m:r>");
}

/// Convert a Typst `Paint` to a 6-character uppercase hex color string (`RRGGBB`).
pub fn paint_to_hex(paint: &Paint) -> Option<String> {
    match paint {
        Paint::Solid(c) => {
            let [r, g, b, _] = c.to_vec4_u8();
            Some(format!("{r:02X}{g:02X}{b:02X}"))
        }
        Paint::Gradient(grad) => {
            let stops = grad.stops_ref();
            if let Some((c, _)) = stops.first() {
                let [r, g, b, _] = c.to_vec4_u8();
                Some(format!("{r:02X}{g:02X}{b:02X}"))
            } else {
                None
            }
        }
        Paint::Tiling(_) => None,
    }
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
