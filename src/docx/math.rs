//! Semantic `MathNodeIr` AST -> Microsoft Word Office Math (`<m:oMath>`) renderer.
//!
//! Pure IR-to-XML renderer with zero dependency on Typst crates.

use std::fmt::Write as _;

use crate::ir::{MathFracStyleIr, MathNodeIr, MathRunIr, MathScriptIr, MathStyleIr};
use crate::xml::{escape_xml_attr, escape_xml_text};

const EMPTY_MATH_RUN: &str = "<m:r><m:t>&#x200B;</m:t></m:r>";

/// Render a slice of `MathNodeIr` into a complete `<m:oMath>...</m:oMath>` XML element.
pub(crate) fn render_math_omml(nodes: &[MathNodeIr], out: &mut String) {
    out.push_str("<m:oMath>");
    let start_len = out.len();
    for node in nodes {
        render_math_node(node, out);
    }
    if out[start_len..].trim().is_empty() {
        out.push_str(EMPTY_MATH_RUN);
    }
    out.push_str("</m:oMath>");
}

fn render_non_empty_node(node: &MathNodeIr, out: &mut String) {
    let start_len = out.len();
    render_math_node(node, out);
    if out.len() == start_len {
        out.push_str(EMPTY_MATH_RUN);
    }
}

fn render_math_node(node: &MathNodeIr, out: &mut String) {
    match node {
        MathNodeIr::Sequence(children) => {
            for child in children {
                render_math_node(child, out);
            }
        }
        MathNodeIr::Fraction { num, denom, style } => {
            let frac_type = match style {
                MathFracStyleIr::Vertical => "bar",
                MathFracStyleIr::Skewed => "skw",
                MathFracStyleIr::Horizontal => "lin",
                MathFracStyleIr::NoBar => "noBar",
            };
            let _ = write!(
                out,
                r#"<m:f><m:fPr><m:type m:val="{frac_type}"/></m:fPr><m:num>"#
            );
            render_non_empty_node(num, out);
            out.push_str("</m:num><m:den>");
            render_non_empty_node(denom, out);
            out.push_str("</m:den></m:f>");
        }
        MathNodeIr::Radical { degree, radicand } => {
            if let Some(deg) = degree {
                out.push_str(r#"<m:rad><m:radPr><m:degHide m:val="0"/></m:radPr><m:deg>"#);
                render_non_empty_node(deg, out);
                out.push_str("</m:deg><m:e>");
                render_non_empty_node(radicand, out);
                out.push_str("</m:e></m:rad>");
            } else {
                out.push_str(r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>"#);
                render_non_empty_node(radicand, out);
                out.push_str("</m:e></m:rad>");
            }
        }
        MathNodeIr::SubSup { base, sub, sup } => match (sub, sup) {
            (Some(sub_n), Some(sup_n)) => {
                out.push_str("<m:sSubSup><m:e>");
                render_non_empty_node(base, out);
                out.push_str("</m:e><m:sub>");
                render_non_empty_node(sub_n, out);
                out.push_str("</m:sub><m:sup>");
                render_non_empty_node(sup_n, out);
                out.push_str("</m:sup></m:sSubSup>");
            }
            (Some(sub_n), None) => {
                out.push_str("<m:sSub><m:e>");
                render_non_empty_node(base, out);
                out.push_str("</m:e><m:sub>");
                render_non_empty_node(sub_n, out);
                out.push_str("</m:sub></m:sSub>");
            }
            (None, Some(sup_n)) => {
                out.push_str("<m:sSup><m:e>");
                render_non_empty_node(base, out);
                out.push_str("</m:e><m:sup>");
                render_non_empty_node(sup_n, out);
                out.push_str("</m:sup></m:sSup>");
            }
            (None, None) => {
                render_non_empty_node(base, out);
            }
        },
        MathNodeIr::PreSubSup { base, sub, sup } => {
            out.push_str("<m:sPre><m:sub>");
            if let Some(sub_n) = sub {
                render_non_empty_node(sub_n, out);
            }
            out.push_str("</m:sub><m:sup>");
            if let Some(sup_n) = sup {
                render_non_empty_node(sup_n, out);
            }
            out.push_str("</m:sup><m:e>");
            render_non_empty_node(base, out);
            out.push_str("</m:e></m:sPre>");
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let mut current = String::new();
            render_non_empty_node(base, &mut current);
            if let Some(low_n) = lower {
                let mut lim_xml = String::new();
                render_non_empty_node(low_n, &mut lim_xml);
                current =
                    format!("<m:limLow><m:e>{current}</m:e><m:lim>{lim_xml}</m:lim></m:limLow>");
            }
            if let Some(upp_n) = upper {
                let mut lim_xml = String::new();
                render_non_empty_node(upp_n, &mut lim_xml);
                current =
                    format!("<m:limUpp><m:e>{current}</m:e><m:lim>{lim_xml}</m:lim></m:limUpp>");
            }
            out.push_str(&current);
        }
        MathNodeIr::Nary {
            chr,
            limits_under_over,
            sub,
            sup,
            operand,
        } => {
            let lim_loc = if *limits_under_over {
                "undOvr"
            } else {
                "subSup"
            };
            let sub_hide = if sub.is_none() { "1" } else { "0" };
            let sup_hide = if sup.is_none() { "1" } else { "0" };
            let mut chr_buf = [0u8; 4];
            let chr_str = escape_xml_attr(chr.encode_utf8(&mut chr_buf));

            let _ = write!(
                out,
                r#"<m:nary><m:naryPr><m:chr m:val="{chr_str}"/><m:limLoc m:val="{lim_loc}"/><m:subHide m:val="{sub_hide}"/><m:supHide m:val="{sup_hide}"/></m:naryPr><m:sub>"#
            );
            if let Some(sub_n) = sub {
                render_math_node(sub_n, out);
            }
            out.push_str("</m:sub><m:sup>");
            if let Some(sup_n) = sup {
                render_math_node(sup_n, out);
            }
            out.push_str("</m:sup><m:e>");
            render_non_empty_node(operand, out);
            out.push_str("</m:e></m:nary>");
        }
        MathNodeIr::Delimited {
            open,
            close,
            separator,
            elements,
        } => {
            let _ = write!(
                out,
                r#"<m:d><m:dPr><m:begChr m:val="{}"/>"#,
                escape_xml_attr(open)
            );
            if !separator.is_empty() {
                let _ = write!(out, r#"<m:sepChr m:val="{}"/>"#, escape_xml_attr(separator));
            }
            let _ = write!(
                out,
                r#"<m:endChr m:val="{}"/></m:dPr>"#,
                escape_xml_attr(close)
            );
            for elem in elements {
                out.push_str("<m:e>");
                render_non_empty_node(elem, out);
                out.push_str("</m:e>");
            }
            out.push_str("</m:d>");
        }
        MathNodeIr::Matrix { rows } => {
            out.push_str("<m:m>");
            for row in rows {
                out.push_str("<m:mr>");
                for cell in row {
                    out.push_str("<m:e>");
                    render_non_empty_node(cell, out);
                    out.push_str("</m:e>");
                }
                out.push_str("</m:mr>");
            }
            out.push_str("</m:m>");
        }
        MathNodeIr::EquationArray { rows } => {
            out.push_str(r#"<m:eqArr><m:eqArrPr><m:maxDist m:val="1"/></m:eqArrPr>"#);
            for row in rows {
                out.push_str("<m:e>");
                render_non_empty_node(row, out);
                out.push_str("</m:e>");
            }
            out.push_str("</m:eqArr>");
        }
        MathNodeIr::Accent { chr, base } => {
            let mut buf = [0u8; 4];
            let chr_str = escape_xml_attr(chr.encode_utf8(&mut buf));
            let _ = write!(
                out,
                r#"<m:acc><m:accPr><m:chr m:val="{chr_str}"/></m:accPr><m:e>"#
            );
            render_non_empty_node(base, out);
            out.push_str("</m:e></m:acc>");
        }
        MathNodeIr::Bar { pos_bottom, body } => {
            let pos = if *pos_bottom { "bot" } else { "top" };
            let _ = write!(
                out,
                r#"<m:bar><m:barPr><m:pos m:val="{pos}"/></m:barPr><m:e>"#
            );
            render_non_empty_node(body, out);
            out.push_str("</m:e></m:bar>");
        }
        MathNodeIr::GroupChar {
            chr,
            is_bottom,
            body,
            annotation,
        } => {
            let mut buf = [0u8; 4];
            let chr_str = escape_xml_attr(chr.encode_utf8(&mut buf));
            let pos = if *is_bottom { "bot" } else { "top" };
            let vert_jc = if *is_bottom { "top" } else { "bot" };

            let mut group_xml = format!(
                r#"<m:groupChr><m:groupChrPr><m:chr m:val="{chr_str}"/><m:pos m:val="{pos}"/><m:vertJc m:val="{vert_jc}"/></m:groupChrPr><m:e>"#
            );
            render_non_empty_node(body, &mut group_xml);
            group_xml.push_str("</m:e></m:groupChr>");

            if let Some(ann) = annotation {
                let mut ann_xml = String::new();
                render_non_empty_node(ann, &mut ann_xml);
                if *is_bottom {
                    let _ = write!(
                        out,
                        "<m:limLow><m:e>{group_xml}</m:e><m:lim>{ann_xml}</m:lim></m:limLow>"
                    );
                } else {
                    let _ = write!(
                        out,
                        "<m:limUpp><m:e>{group_xml}</m:e><m:lim>{ann_xml}</m:lim></m:limUpp>"
                    );
                }
            } else {
                out.push_str(&group_xml);
            }
        }
        MathNodeIr::Cancel {
            body,
            cross,
            inverted,
        } => {
            out.push_str("<m:borderBox><m:borderBoxPr>");
            out.push_str(
                r#"<m:hideTop m:val="1"/><m:hideBot m:val="1"/><m:hideLeft m:val="1"/><m:hideRight m:val="1"/>"#,
            );
            if *cross {
                out.push_str(r#"<m:strikeBLTR m:val="1"/><m:strikeTLBR m:val="1"/>"#);
            } else if *inverted {
                out.push_str(r#"<m:strikeTLBR m:val="1"/>"#);
            } else {
                out.push_str(r#"<m:strikeBLTR m:val="1"/>"#);
            }
            out.push_str("</m:borderBoxPr><m:e>");
            render_non_empty_node(body, out);
            out.push_str("</m:e></m:borderBox>");
        }
        MathNodeIr::AlignPoint => {
            out.push_str("<m:r><m:t>&amp;</m:t></m:r>");
        }
        MathNodeIr::Run(run) => {
            render_math_run(run, out);
        }
    }
}

fn render_math_run(run: &MathRunIr, out: &mut String) {
    if run.text.is_empty() {
        return;
    }
    out.push_str("<m:r>");
    if run.script.is_some() || run.style.is_some() {
        out.push_str("<m:rPr>");
        if let Some(scr) = run.script {
            let scr_val = match scr {
                MathScriptIr::DoubleStruck => "double-struck",
                MathScriptIr::Script => "script",
                MathScriptIr::Fraktur => "fraktur",
                MathScriptIr::Monospace => "monospace",
                MathScriptIr::SansSerif => "sans-serif",
            };
            let _ = write!(out, r#"<m:scr m:val="{scr_val}"/>"#);
        }
        if let Some(sty) = run.style {
            let sty_val = match sty {
                MathStyleIr::Plain => "p",
                MathStyleIr::Bold => "b",
                MathStyleIr::Italic => "i",
                MathStyleIr::BoldItalic => "bi",
            };
            let _ = write!(out, r#"<m:sty m:val="{sty_val}"/>"#);
        }
        out.push_str("</m:rPr>");
    }
    if let Some(ref hex) = run.color {
        let _ = write!(out, r#"<w:rPr><w:color w:val="{hex}"/></w:rPr>"#);
    }
    out.push_str("<m:t>");
    out.push_str(&escape_xml_text(&run.text));
    out.push_str("</m:t></m:r>");
}
