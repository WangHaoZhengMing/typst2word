//! Semantic `EquationIr` & `MathNodeIr` -> `WordEquationBox` calibration + Office Math (`<m:oMath>`) renderer.
//!
//! Pure IR-to-Word renderer with zero dependency on Typst crates.

use std::fmt::Write as _;

use crate::ir::{
    EquationIr, MathFracStyleIr, MathNodeIr, MathRunIr, MathScriptIr, MathStyleIr,
};
use crate::xml::{escape_xml_attr, escape_xml_text};

const EMPTY_MATH_RUN: &str = "<m:r><m:t>&#x200B;</m:t></m:r>";

/// Structural metrics extracted from a `MathNodeIr` AST for `word_construct::WordEquationBox`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MathAstStructure {
    pub(crate) row_count: usize,
    pub(crate) max_fraction_depth: usize,
    pub(crate) has_tall_construct: bool,
    pub(crate) all_glyphs_in_fraction_or_script: bool,
    pub(crate) expected_em_width: f64,
}

/// Solve `word_construct::WordEquationBox` for an `EquationIr` from its `EquationPdfMetricsIr` and AST structure.
pub(crate) fn solve_word_equation_box(eq: &EquationIr) -> word_construct::WordEquationBox {
    let ast_metrics = analyze_math_slice(&eq.body, eq.is_block);
    let pdf = eq.pdf_metrics;
    word_construct::WordEquationBox::solve(word_construct::EquationBoxInput {
        is_block: eq.is_block,
        typst_width_pt: pdf.width_pt(),
        typst_height_pt: pdf.height_pt(),
        typst_max_glyph_pt: pdf.max_glyph_pt(),
        typst_min_glyph_pt: pdf.min_glyph_pt(),
        context_font_size_pt: pdf.context_font_size_pt(),
        max_available_width_pt: pdf.max_available_width_pt(),
        row_count: ast_metrics.row_count,
        max_fraction_depth: ast_metrics.max_fraction_depth,
        has_tall_construct: ast_metrics.has_tall_construct,
        all_glyphs_in_fraction_or_script: ast_metrics.all_glyphs_in_fraction_or_script,
        expected_em_width: ast_metrics.expected_em_width,
    })
}

/// Calibrate an `EquationIr` via `word_construct::WordEquationBox` and render `<m:oMath>...</m:oMath>`.
pub(crate) fn render_equation_omml(eq: &EquationIr, out: &mut String) {
    let eq_box = solve_word_equation_box(eq);
    let mut calibrated_body = eq.body.clone();
    apply_calibrated_box(
        &mut calibrated_body,
        eq_box.run_font_size_pt,
        eq_box.fraction_font_size_pt,
        eq_box.width_scale_pct,
    );
    render_math_omml(&calibrated_body, out);
}

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
    let has_cjk = word_construct::str_has_cjk_chars(&run.text);
    out.push_str("<m:r>");
    if has_cjk || run.script.is_some() || run.style.is_some() {
        out.push_str("<m:rPr>");
        if has_cjk {
            out.push_str("<m:nor/>");
        }
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
    let needs_font = run.font_family.as_deref().is_some_and(|fam| !fam.is_empty());
    let width_scale = run.width_scale_pct.filter(|&p| p != 100);
    if needs_font || run.font_size_pt.is_some() || run.color.is_some() || width_scale.is_some() {
        out.push_str("<w:rPr>");
        if needs_font
            && let Some(ref fam) = run.font_family
        {
            render_math_run_fonts(fam, has_cjk, out);
        }
        if let Some(ref hex) = run.color {
            let _ = write!(out, r#"<w:color w:val="{hex}"/>"#);
        }
        if let Some(sz_pt) = run.font_size_pt {
            let half_pts = ((sz_pt * 2.0).round() as u32).clamp(4, 288);
            let _ = write!(
                out,
                r#"<w:sz w:val="{half_pts}"/><w:szCs w:val="{half_pts}"/>"#
            );
        }
        if let Some(pct) = width_scale {
            let clamped = pct.clamp(50, 200);
            let _ = write!(out, r#"<w:w w:val="{clamped}"/>"#);
        }
        out.push_str("</w:rPr>");
    }
    out.push_str("<m:t>");
    out.push_str(&escape_xml_text(&run.text));
    out.push_str("</m:t></m:r>");
}

fn render_math_run_fonts(fam: &str, has_cjk: bool, out: &mut String) {
    if has_cjk {
        let ea_esc = escape_xml_attr(fam);
        let _ = write!(
            out,
            r#"<w:rFonts w:ascii="{ea_esc}" w:hAnsi="{ea_esc}" w:eastAsia="{ea_esc}" w:cs="{ea_esc}" w:hint="eastAsia"/>"#
        );
    } else {
        let math_esc = escape_xml_attr(fam);
        let _ = write!(
            out,
            r#"<w:rFonts w:ascii="{math_esc}" w:hAnsi="{math_esc}" w:cs="{math_esc}"/>"#
        );
    }
}

// ============================================================================
// Word OMML Structural Analyzer & 2-DoF Box Calibrator
// ============================================================================

fn analyze_math_slice(nodes: &[MathNodeIr], is_block: bool) -> MathAstStructure {
    let mut row_count = 1usize;
    let mut max_fraction_depth = 0usize;
    let mut has_tall_construct = false;
    let mut has_root_unscaled_glyph = false;
    let mut has_any_glyph = false;
    let mut expected_em_width = 0.0_f64;

    for node in nodes {
        let (r, f, tall, root_g, any_g) = inspect_node_structure(node, false);
        row_count = row_count.max(r);
        max_fraction_depth = max_fraction_depth.max(f);
        has_tall_construct |= tall;
        has_root_unscaled_glyph |= root_g;
        has_any_glyph |= any_g;
        expected_em_width += estimate_node_em_width(node, is_block && row_count == 1);
    }

    MathAstStructure {
        row_count,
        max_fraction_depth,
        has_tall_construct,
        all_glyphs_in_fraction_or_script: has_any_glyph && !has_root_unscaled_glyph,
        expected_em_width: expected_em_width.max(0.5),
    }
}

fn apply_calibrated_box(
    nodes: &mut [MathNodeIr],
    run_font_size_pt: f64,
    fraction_font_size_pt: Option<f64>,
    width_scale_pct: Option<u32>,
) {
    for node in nodes {
        apply_calibrated_box_inner(
            node,
            run_font_size_pt,
            fraction_font_size_pt,
            width_scale_pct,
            false,
        );
    }
}

fn apply_calibrated_box_inner(
    node: &mut MathNodeIr,
    run_font_size_pt: f64,
    fraction_font_size_pt: Option<f64>,
    width_scale_pct: Option<u32>,
    in_vertical_fraction: bool,
) {
    match node {
        MathNodeIr::Sequence(children) => {
            for ch in children {
                apply_calibrated_box_inner(
                    ch,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Fraction { num, denom, style } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let next_in_frac = in_vertical_fraction || is_vert;
            apply_calibrated_box_inner(
                num,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                next_in_frac,
            );
            apply_calibrated_box_inner(
                denom,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                next_in_frac,
            );
        }
        MathNodeIr::Radical { degree, radicand } => {
            if let Some(deg) = degree {
                apply_calibrated_box_inner(
                    deg,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            apply_calibrated_box_inner(
                radicand,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            apply_calibrated_box_inner(
                base,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
            if let Some(s) = sub {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            if let Some(s) = sup {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Limit { base, lower, upper } => {
            apply_calibrated_box_inner(
                base,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
            if let Some(l) = lower {
                apply_calibrated_box_inner(
                    l,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            if let Some(u) = upper {
                apply_calibrated_box_inner(
                    u,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            if let Some(s) = sub {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            if let Some(s) = sup {
                apply_calibrated_box_inner(
                    s,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            apply_calibrated_box_inner(
                operand,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::Delimited { elements, .. } => {
            for el in elements {
                apply_calibrated_box_inner(
                    el,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Matrix { rows } => {
            for row in rows {
                for cell in row {
                    apply_calibrated_box_inner(
                        cell,
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
            }
        }
        MathNodeIr::EquationArray { rows } => {
            for row in rows {
                apply_calibrated_box_inner(
                    row,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::Accent { base, .. } => {
            apply_calibrated_box_inner(
                base,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            apply_calibrated_box_inner(
                body,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            apply_calibrated_box_inner(
                body,
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                in_vertical_fraction,
            );
            if let Some(ann) = annotation {
                apply_calibrated_box_inner(
                    ann,
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
        }
        MathNodeIr::AlignPoint => {}
        MathNodeIr::Run(run) => {
            let target_sz = if in_vertical_fraction {
                fraction_font_size_pt.unwrap_or(run_font_size_pt)
            } else {
                run_font_size_pt
            };
            run.font_size_pt = Some(target_sz);
            run.width_scale_pct = width_scale_pct;
        }
    }
}

fn inspect_node_structure(
    node: &MathNodeIr,
    in_script_or_frac: bool,
) -> (usize, usize, bool, bool, bool) {
    match node {
        MathNodeIr::Sequence(children) => {
            let mut rows = 1usize;
            let mut frac_d = 0usize;
            let mut tall = false;
            let mut root_g = false;
            let mut any_g = false;
            for ch in children {
                let (r, f, t, rg, ag) = inspect_node_structure(ch, in_script_or_frac);
                rows = rows.max(r);
                frac_d = frac_d.max(f);
                tall |= t;
                root_g |= rg;
                any_g |= ag;
            }
            (rows, frac_d, tall, root_g, any_g)
        }
        MathNodeIr::Fraction { num, denom, style } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let (_, f1, t1, _, ag1) = inspect_node_structure(num, true);
            let (_, f2, t2, _, ag2) = inspect_node_structure(denom, true);
            let f = if is_vert { 1 + f1.max(f2) } else { f1.max(f2) };
            (1, f, is_vert || t1 || t2, false, ag1 || ag2)
        }
        MathNodeIr::Radical { degree, radicand } => {
            let (_, f1, _, _, ag1) = degree.as_ref().map_or((1, 0, false, false, false), |d| {
                inspect_node_structure(d, true)
            });
            let (_, f2, _, rg2, ag2) = inspect_node_structure(radicand, in_script_or_frac);
            (1, f1.max(f2), true, rg2, ag1 || ag2)
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            let (_, f0, t0, rg0, ag0) = inspect_node_structure(base, in_script_or_frac);
            let (_, f1, _, _, ag1) = sub.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            let (_, f2, _, _, ag2) = sup.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            (
                1,
                f0.max(f1).max(f2),
                t0 || sub.is_some() || sup.is_some(),
                rg0,
                ag0 || ag1 || ag2,
            )
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let (_, f0, _, rg0, ag0) = inspect_node_structure(base, in_script_or_frac);
            let (_, f1, _, _, ag1) = lower.as_ref().map_or((1, 0, false, false, false), |l| {
                inspect_node_structure(l, true)
            });
            let (_, f2, _, _, ag2) = upper.as_ref().map_or((1, 0, false, false, false), |u| {
                inspect_node_structure(u, true)
            });
            (1, f0.max(f1).max(f2), true, rg0, ag0 || ag1 || ag2)
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            let (_, f1, _, _, _) = sub.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            let (_, f2, _, _, _) = sup.as_ref().map_or((1, 0, false, false, false), |s| {
                inspect_node_structure(s, true)
            });
            let (_, f3, _, rg3, _) = inspect_node_structure(operand, in_script_or_frac);
            (1, f1.max(f2).max(f3), true, rg3, true)
        }
        MathNodeIr::Delimited { elements, .. } => {
            let mut rows = 1usize;
            let mut frac_d = 0usize;
            let mut tall = false;
            let mut root_g = false;
            let mut any_g = false;
            for el in elements {
                let (r, f, t, rg, ag) = inspect_node_structure(el, in_script_or_frac);
                rows = rows.max(r);
                frac_d = frac_d.max(f);
                tall |= t;
                root_g |= rg;
                any_g |= ag;
            }
            (rows, frac_d, tall, root_g, any_g)
        }
        MathNodeIr::Matrix { rows } => {
            let r_cnt = rows.len().max(1);
            let mut frac_d = 0usize;
            let mut root_g = false;
            let mut any_g = false;
            for row in rows {
                for cell in row {
                    let (_, f, _, rg, ag) = inspect_node_structure(cell, in_script_or_frac);
                    frac_d = frac_d.max(f);
                    root_g |= rg;
                    any_g |= ag;
                }
            }
            (r_cnt, frac_d, r_cnt > 1, root_g, any_g)
        }
        MathNodeIr::EquationArray { rows } => {
            let r_cnt = rows.len().max(1);
            let mut frac_d = 0usize;
            let mut root_g = false;
            let mut any_g = false;
            for row in rows {
                let (_, f, _, rg, ag) = inspect_node_structure(row, in_script_or_frac);
                frac_d = frac_d.max(f);
                root_g |= rg;
                any_g |= ag;
            }
            (r_cnt, frac_d, r_cnt > 1, root_g, any_g)
        }
        MathNodeIr::Accent { base, .. } => {
            let (r, f, _, rg, ag) = inspect_node_structure(base, in_script_or_frac);
            (r, f, true, rg, ag)
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            inspect_node_structure(body, in_script_or_frac)
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            let (_, f0, _, rg0, ag0) = inspect_node_structure(body, in_script_or_frac);
            let (_, f1, _, _, ag1) = annotation
                .as_ref()
                .map_or((1, 0, false, false, false), |a| {
                    inspect_node_structure(a, true)
                });
            (1, f0.max(f1), true, rg0, ag0 || ag1)
        }
        MathNodeIr::AlignPoint => (1, 0, false, false, false),
        MathNodeIr::Run(run) => {
            let non_ws = !run.text.trim().is_empty();
            (1, 0, false, non_ws && !in_script_or_frac, non_ws)
        }
    }
}

fn estimate_node_em_width(node: &MathNodeIr, display_top_level: bool) -> f64 {
    match node {
        MathNodeIr::Sequence(children) => children
            .iter()
            .map(|c| estimate_node_em_width(c, display_top_level))
            .sum(),
        MathNodeIr::Fraction { num, denom, style } => match style {
            MathFracStyleIr::Vertical | MathFracStyleIr::NoBar => {
                let scale = if display_top_level { 1.0 } else { 0.73 };
                let w = estimate_node_em_width(num, false)
                    .max(estimate_node_em_width(denom, false));
                w * scale + 0.28
            }
            MathFracStyleIr::Skewed | MathFracStyleIr::Horizontal => {
                estimate_node_em_width(num, false) + 0.45 + estimate_node_em_width(denom, false)
            }
        },
        MathNodeIr::Radical { degree, radicand } => {
            let deg_w = degree
                .as_ref()
                .map_or(0.0, |d| estimate_node_em_width(d, false) * 0.6);
            0.65 + deg_w + estimate_node_em_width(radicand, display_top_level)
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            let bw = estimate_node_em_width(base, display_top_level);
            let sub_w = sub.as_ref().map_or(0.0, |s| estimate_node_em_width(s, false));
            let sup_w = sup.as_ref().map_or(0.0, |s| estimate_node_em_width(s, false));
            bw + 0.73 * sub_w.max(sup_w) + 0.04
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let bw = estimate_node_em_width(base, display_top_level);
            let lw = lower
                .as_ref()
                .map_or(0.0, |l| estimate_node_em_width(l, false) * 0.73);
            let uw = upper
                .as_ref()
                .map_or(0.0, |u| estimate_node_em_width(u, false) * 0.73);
            bw.max(lw).max(uw) + 0.10
        }
        MathNodeIr::Nary {
            limits_under_over,
            sub,
            sup,
            operand,
            ..
        } => {
            let sub_w = sub
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.73);
            let sup_w = sup
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.73);
            let op_w = if *limits_under_over {
                0.90_f64.max(sub_w).max(sup_w)
            } else {
                0.85 + sub_w.max(sup_w)
            };
            op_w + 0.15 + estimate_node_em_width(operand, display_top_level)
        }
        MathNodeIr::Delimited {
            open,
            close,
            separator,
            elements,
        } => {
            let open_w = if open.is_empty() { 0.0 } else { 0.40 };
            let close_w = if close.is_empty() { 0.0 } else { 0.40 };
            let sep_w = if separator.is_empty() { 0.0 } else { 0.35 };
            let inner_w: f64 = elements
                .iter()
                .map(|e| estimate_node_em_width(e, display_top_level))
                .sum();
            let seps = elements.len().saturating_sub(1) as f64 * sep_w;
            open_w + inner_w + seps + close_w
        }
        MathNodeIr::Matrix { rows } => {
            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut total_w = 0.0;
            for c in 0..cols {
                let col_max = rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .map(|cell| estimate_node_em_width(cell, false))
                    .fold(0.0_f64, f64::max);
                total_w += col_max;
            }
            total_w + cols.saturating_sub(1) as f64 * 0.75
        }
        MathNodeIr::EquationArray { rows } => rows
            .iter()
            .map(|r| estimate_node_em_width(r, false))
            .fold(0.0_f64, f64::max),
        MathNodeIr::Accent { base, .. } => estimate_node_em_width(base, display_top_level),
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            estimate_node_em_width(body, display_top_level)
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            let bw = estimate_node_em_width(body, display_top_level);
            let aw = annotation
                .as_ref()
                .map_or(0.0, |a| estimate_node_em_width(a, false) * 0.73);
            bw.max(aw)
        }
        MathNodeIr::AlignPoint => 0.0,
        MathNodeIr::Run(run) => run.text.chars().map(char_math_em_width).sum(),
    }
}

fn char_math_em_width(c: char) -> f64 {
    if word_construct::is_cjk_or_fullwidth_char(c) {
        1.0
    } else if c.is_ascii_digit() {
        0.55
    } else if c.is_ascii_uppercase() {
        0.66
    } else if c.is_ascii_lowercase() {
        0.52
    } else if matches!(
        c,
        '=' | '+'
            | '-'
            | '−'
            | '×'
            | '÷'
            | '<'
            | '>'
            | '≤'
            | '≥'
            | '⩽'
            | '⩾'
            | '≠'
            | '≈'
            | '≡'
            | '→'
            | '←'
            | '⇒'
            | '⇔'
    ) {
        0.95
    } else if c.is_whitespace() {
        0.25
    } else {
        0.48
    }
}
