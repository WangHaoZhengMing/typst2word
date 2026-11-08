//! Native Typst Math `Content` AST -> Microsoft Word Office Math (`<m:oMath>`) converter.
//!
//! Translates Typst's structured math elements (`EquationElem`, `FracElem`, `BinomElem`,
//! `AttachElem`, `PrimesElem`, `ScriptsElem`, `LimitsElem`, `StretchElem`, `RootElem`,
//! `LrElem`, `MidElem`, `VecElem`, `MatElem`, `CasesElem`, `AccentElem`, `UnderlineElem`,
//! `OverlineElem`, `UnderbraceElem`, `OverbraceElem`, `UnderbracketElem`, `OverbracketElem`,
//! `UnderparenElem`, `OverparenElem`, `UndershellElem`, `OvershellElem`, `CancelElem`,
//! `OpElem`, `ClassElem`, `AlignPointElem`, `SymbolElem`, `TextElem`, `StyledElem`)
//! directly into Word OMML (`<m:oMath>`).
//!
//! The entry points and sequence handling live here; element-specific conversion is in
//! the submodules.

mod attach;
mod delim;
mod emit;
mod nary;
mod node;

use typst::foundations::{Content, Packed, SequenceElem, StyleChain, StyledElem};
use typst::introspection::TagElem;
use typst::math::{AlignPointElem, EquationElem};
use typst::text::{LinebreakElem, SpaceElem};

use crate::math::nary::{emit_nary_xml, extract_nary_operator, is_nary_operand_terminator};
use crate::math::node::convert_math_node;

/// Active math styling state propagated down `StyledElem` chains.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MathStyleCtx {
    pub(crate) is_display: bool,
    pub(crate) in_eq_arr: bool,
    pub(crate) force_limits: Option<bool>,
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

pub(crate) fn convert_math_root(
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

pub(crate) fn convert_sequence_slice(
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
