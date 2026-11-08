//! Delimiters (`lr`, fences) -> `<m:d>`.

use typst::foundations::{Content, Packed, SequenceElem, StyleChain, SymbolElem};
use typst::math::{LrElem, MidElem};
use typst::text::TextElem;

use crate::math::{MathStyleCtx, convert_sequence_slice, flatten_sequence_preserving_styles};
use crate::xml::escape_xml_attr;

pub(crate) fn convert_lr(
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

pub(crate) fn extract_delimiter_char(content: &Content, is_open: bool) -> Option<char> {
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

pub(crate) fn is_fence_char(c: char, _is_open: bool) -> bool {
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
