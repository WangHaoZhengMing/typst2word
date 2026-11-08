//! Low-level OMML emitters (runs, bars, group characters, optional wrappers).

use typst::foundations::{Content, StyleChain};
use typst::math::EquationElem;
use typst::text::TextElem;
use typst::visualize::Paint;

use crate::math::MathStyleCtx;
use crate::math::node::convert_math_node;
use crate::xml::{escape_xml_attr, escape_xml_text};

pub(crate) fn emit_bar(
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
pub(crate) fn emit_group_chr(
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

pub(crate) fn emit_non_empty_element(
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

pub(crate) fn emit_math_run(
    text: &str,
    styles: StyleChain,
    default_upright: bool,
    xml: &mut String,
) {
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
