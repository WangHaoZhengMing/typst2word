//! Per-element conversion of a single math node (`frac`, `root`, `mat`, `cases`,
//! `accent`, `cancel`, text / symbols, styling wrappers, ...) to OMML.

use typst::foundations::{Content, SequenceElem, StyleChain, StyledElem, SymbolElem};
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

use crate::math::attach::convert_attach;
use crate::math::delim::convert_lr;
use crate::math::emit::{emit_bar, emit_group_chr, emit_math_run, emit_non_empty_element};
use crate::math::nary::{emit_nary_xml, extract_nary_operator};
use crate::math::{
    MathStyleCtx, convert_sequence_slice, flatten_sequence_preserving_styles, split_by_align_point,
};
use crate::xml::escape_xml_attr;

pub(crate) fn convert_math_node(
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
