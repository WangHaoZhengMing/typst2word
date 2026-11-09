//! Semantic Intermediate Representation for Mathematical Equations (`EquationIr` & `MathNodeIr`).
//!
//! Decouples Typst's `typst::math` AST from Microsoft Word's Office Math (`<m:oMath>`) XML:
//! - `crate::extract::math` converts Typst `Content` -> `Vec<MathNodeIr>`
//! - `crate::docx::math` renders `&[MathNodeIr]` -> `<m:oMath>...</m:oMath>`

use serde::{Deserialize, Serialize};

use crate::ir::ColorHex;

/// Mathematical equation IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationIr {
    pub is_block: bool,
    /// Structured semantic math AST nodes.
    pub body: Vec<MathNodeIr>,
    /// Plain-text/TeX-like fallback representation for accessibility or debugging.
    pub source_text: String,
    /// Optional equation number text (e.g., `"(1)"` or `"(2.1)"`).
    pub numbering_text: Option<String>,
    /// Optional bookmark name for `@ref` cross-referencing.
    pub bookmark: Option<String>,
    /// Calibrated base font size in points resolved from the laid-out equation frame.
    #[serde(default)]
    pub font_size_pt: Option<f64>,
    /// Tight rendered width in points measured from Typst's laid-out equation frame.
    #[serde(default)]
    pub width_pt: Option<f64>,
    /// Tight rendered height in points measured from Typst's laid-out equation frame.
    #[serde(default)]
    pub height_pt: Option<f64>,
    /// Optional horizontal character width scale percentage (`<w:w w:val="..."/>`, `100` = 100%).
    #[serde(default)]
    pub width_scale_pct: Option<u32>,
}

/// Semantic math AST node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MathNodeIr {
    /// A horizontal sequence of math nodes.
    Sequence(Vec<MathNodeIr>),
    /// Fraction or binomial stack (`<m:f>`).
    Fraction {
        num: Box<MathNodeIr>,
        denom: Box<MathNodeIr>,
        style: MathFracStyleIr,
    },
    /// Square root or $n$-th root (`<m:rad>`).
    Radical {
        degree: Option<Box<MathNodeIr>>,
        radicand: Box<MathNodeIr>,
    },
    /// Right subscript and/or superscript (`<m:sSub>`, `<m:sSup>`, `<m:sSubSup>`).
    SubSup {
        base: Box<MathNodeIr>,
        sub: Option<Box<MathNodeIr>>,
        sup: Option<Box<MathNodeIr>>,
    },
    /// Pre-subscript and/or pre-superscript (`<m:sPre>`).
    PreSubSup {
        base: Box<MathNodeIr>,
        sub: Option<Box<MathNodeIr>>,
        sup: Option<Box<MathNodeIr>>,
    },
    /// Under-limit and/or over-limit on an operator (`<m:limLow>`, `<m:limUpp>`).
    Limit {
        base: Box<MathNodeIr>,
        lower: Option<Box<MathNodeIr>>,
        upper: Option<Box<MathNodeIr>>,
    },
    /// Big $n$-ary operator such as $\sum$, $\prod$, $\int$ (`<m:nary>`).
    Nary {
        chr: char,
        limits_under_over: bool,
        sub: Option<Box<MathNodeIr>>,
        sup: Option<Box<MathNodeIr>>,
        operand: Box<MathNodeIr>,
    },
    /// Left/right fenced expression (`lr(...)`, `binom`, `vec`, `mat`, `cases`) (`<m:d>`).
    Delimited {
        open: String,
        close: String,
        separator: String,
        elements: Vec<MathNodeIr>,
    },
    /// 2D matrix grid (`<m:m>`).
    Matrix { rows: Vec<Vec<MathNodeIr>> },
    /// Multi-line aligned equation array (`<m:eqArr>`).
    EquationArray { rows: Vec<MathNodeIr> },
    /// Diacritical accent above a base (`hat`, `tilde`, `vec`, `dot`, ...) (`<m:acc>`).
    Accent { chr: char, base: Box<MathNodeIr> },
    /// Overbar or underbar (`overline`, `underline`) (`<m:bar>`).
    Bar {
        pos_bottom: bool,
        body: Box<MathNodeIr>,
    },
    /// Horizontal grouping brace/bracket/paren/shell with optional annotation (`<m:groupChr>`).
    GroupChar {
        chr: char,
        is_bottom: bool,
        body: Box<MathNodeIr>,
        annotation: Option<Box<MathNodeIr>>,
    },
    /// Diagonal strike/cancellation box (`cancel`) (`<m:borderBox>`).
    Cancel {
        body: Box<MathNodeIr>,
        cross: bool,
        inverted: bool,
    },
    /// Alignment point (`&`) inside an `EquationArray`.
    AlignPoint,
    /// Styled math text/symbol run (`<m:r>`).
    Run(MathRunIr),
}

/// Fraction bar rendering style (`<m:fPr><m:type>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathFracStyleIr {
    /// Standard horizontal fraction bar (`bar`).
    Vertical,
    /// Skewed / diagonal fraction (`skw`).
    Skewed,
    /// Inline linear fraction (`lin`).
    Horizontal,
    /// Barless stack (used inside `binom`) (`noBar`).
    NoBar,
}

/// Math script/font family variant (`<m:scr>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathScriptIr {
    DoubleStruck,
    Script,
    Fraktur,
    Monospace,
    SansSerif,
}

/// Math character style (`<m:sty>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathStyleIr {
    /// Plain / upright (`p`).
    Plain,
    /// Bold upright (`b`).
    Bold,
    /// Italic (`i`).
    Italic,
    /// Bold italic (`bi`).
    BoldItalic,
}

/// A leaf text or symbol run inside a math equation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MathRunIr {
    pub text: String,
    pub script: Option<MathScriptIr>,
    pub style: Option<MathStyleIr>,
    pub color: Option<ColorHex>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size_pt: Option<f64>,
    #[serde(default)]
    pub width_scale_pct: Option<u32>,
}

/// Structural metrics extracted from a `MathNodeIr` AST for `word_construct::WordEquationBox`.
#[derive(Debug, Clone, Copy)]
pub struct MathAstStructure {
    pub row_count: usize,
    pub max_fraction_depth: usize,
    pub has_tall_construct: bool,
    pub all_glyphs_in_fraction_or_script: bool,
    pub expected_em_width: f64,
}

impl MathNodeIr {
    /// Analyze a slice of root `MathNodeIr` nodes to extract structural and `em`-width metrics.
    pub fn analyze_slice(nodes: &[MathNodeIr], is_block: bool) -> MathAstStructure {
        let mut row_count = 1usize;
        let mut max_fraction_depth = 0usize;
        let mut has_tall_construct = false;
        let mut has_root_unscaled_glyph = false;
        let mut has_any_glyph = false;
        let mut expected_em_width = 0.0_f64;

        for node in nodes {
            let (r, f, tall, root_g, any_g) = node.inspect_structure(false);
            row_count = row_count.max(r);
            max_fraction_depth = max_fraction_depth.max(f);
            has_tall_construct |= tall;
            has_root_unscaled_glyph |= root_g;
            has_any_glyph |= any_g;
            expected_em_width += node.estimate_em_width(is_block && row_count == 1);
        }

        MathAstStructure {
            row_count,
            max_fraction_depth,
            has_tall_construct,
            all_glyphs_in_fraction_or_script: has_any_glyph && !has_root_unscaled_glyph,
            expected_em_width: expected_em_width.max(0.5),
        }
    }

    /// Propagate solved `run_font_size_pt`, `fraction_font_size_pt`, and `width_scale_pct`
    /// down a slice of `MathNodeIr` nodes.
    pub fn apply_calibrated_box(
        nodes: &mut [MathNodeIr],
        run_font_size_pt: f64,
        fraction_font_size_pt: Option<f64>,
        width_scale_pct: Option<u32>,
    ) {
        for node in nodes {
            node.apply_calibrated_box_inner(
                run_font_size_pt,
                fraction_font_size_pt,
                width_scale_pct,
                false,
            );
        }
    }

    fn apply_calibrated_box_inner(
        &mut self,
        run_font_size_pt: f64,
        fraction_font_size_pt: Option<f64>,
        width_scale_pct: Option<u32>,
        in_vertical_fraction: bool,
    ) {
        match self {
            MathNodeIr::Sequence(children) => {
                for ch in children {
                    ch.apply_calibrated_box_inner(
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
                num.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    next_in_frac,
                );
                denom.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    next_in_frac,
                );
            }
            MathNodeIr::Radical { degree, radicand } => {
                if let Some(deg) = degree {
                    deg.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
                radicand.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
                base.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
                if let Some(s) = sub {
                    s.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
                if let Some(s) = sup {
                    s.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
            }
            MathNodeIr::Limit { base, lower, upper } => {
                base.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
                if let Some(l) = lower {
                    l.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
                if let Some(u) = upper {
                    u.apply_calibrated_box_inner(
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
                    s.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
                if let Some(s) = sup {
                    s.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
                operand.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            MathNodeIr::Delimited { elements, .. } => {
                for el in elements {
                    el.apply_calibrated_box_inner(
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
                        cell.apply_calibrated_box_inner(
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
                    row.apply_calibrated_box_inner(
                        run_font_size_pt,
                        fraction_font_size_pt,
                        width_scale_pct,
                        in_vertical_fraction,
                    );
                }
            }
            MathNodeIr::Accent { base, .. } => {
                base.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
                body.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
            }
            MathNodeIr::GroupChar {
                body, annotation, ..
            } => {
                body.apply_calibrated_box_inner(
                    run_font_size_pt,
                    fraction_font_size_pt,
                    width_scale_pct,
                    in_vertical_fraction,
                );
                if let Some(ann) = annotation {
                    ann.apply_calibrated_box_inner(
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

    /// Returns `(row_count, max_fraction_depth, has_tall_construct, has_root_unscaled_glyph, has_any_glyph)`.
    fn inspect_structure(&self, in_script_or_frac: bool) -> (usize, usize, bool, bool, bool) {
        match self {
            MathNodeIr::Sequence(children) => {
                let mut rows = 1usize;
                let mut frac_d = 0usize;
                let mut tall = false;
                let mut root_g = false;
                let mut any_g = false;
                for ch in children {
                    let (r, f, t, rg, ag) = ch.inspect_structure(in_script_or_frac);
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
                let (_, f1, t1, _, ag1) = num.inspect_structure(true);
                let (_, f2, t2, _, ag2) = denom.inspect_structure(true);
                let f = if is_vert { 1 + f1.max(f2) } else { f1.max(f2) };
                (1, f, is_vert || t1 || t2, false, ag1 || ag2)
            }
            MathNodeIr::Radical { degree, radicand } => {
                let (_, f1, _, _, ag1) = degree
                    .as_ref()
                    .map_or((1, 0, false, false, false), |d| d.inspect_structure(true));
                let (_, f2, _, rg2, ag2) = radicand.inspect_structure(in_script_or_frac);
                (1, f1.max(f2), true, rg2, ag1 || ag2)
            }
            MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
                let (_, f0, t0, rg0, ag0) = base.inspect_structure(in_script_or_frac);
                let (_, f1, _, _, ag1) = sub
                    .as_ref()
                    .map_or((1, 0, false, false, false), |s| s.inspect_structure(true));
                let (_, f2, _, _, ag2) = sup
                    .as_ref()
                    .map_or((1, 0, false, false, false), |s| s.inspect_structure(true));
                (
                    1,
                    f0.max(f1).max(f2),
                    t0 || sub.is_some() || sup.is_some(),
                    rg0,
                    ag0 || ag1 || ag2,
                )
            }
            MathNodeIr::Limit { base, lower, upper } => {
                let (_, f0, _, rg0, ag0) = base.inspect_structure(in_script_or_frac);
                let (_, f1, _, _, ag1) = lower
                    .as_ref()
                    .map_or((1, 0, false, false, false), |l| l.inspect_structure(true));
                let (_, f2, _, _, ag2) = upper
                    .as_ref()
                    .map_or((1, 0, false, false, false), |u| u.inspect_structure(true));
                (1, f0.max(f1).max(f2), true, rg0, ag0 || ag1 || ag2)
            }
            MathNodeIr::Nary {
                sub, sup, operand, ..
            } => {
                let (_, f1, _, _, ag1) = sub
                    .as_ref()
                    .map_or((1, 0, false, false, false), |s| s.inspect_structure(true));
                let (_, f2, _, _, ag2) = sup
                    .as_ref()
                    .map_or((1, 0, false, false, false), |s| s.inspect_structure(true));
                let (_, f3, _, rg3, ag3) = operand.inspect_structure(in_script_or_frac);
                (1, f1.max(f2).max(f3), true, rg3, true || ag1 || ag2 || ag3)
            }
            MathNodeIr::Delimited { elements, .. } => {
                let mut rows = 1usize;
                let mut frac_d = 0usize;
                let mut tall = false;
                let mut root_g = false;
                let mut any_g = false;
                for el in elements {
                    let (r, f, t, rg, ag) = el.inspect_structure(in_script_or_frac);
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
                        let (_, f, _, rg, ag) = cell.inspect_structure(in_script_or_frac);
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
                    let (_, f, _, rg, ag) = row.inspect_structure(in_script_or_frac);
                    frac_d = frac_d.max(f);
                    root_g |= rg;
                    any_g |= ag;
                }
                (r_cnt, frac_d, r_cnt > 1, root_g, any_g)
            }
            MathNodeIr::Accent { base, .. } => {
                let (r, f, _, rg, ag) = base.inspect_structure(in_script_or_frac);
                (r, f, true, rg, ag)
            }
            MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
                body.inspect_structure(in_script_or_frac)
            }
            MathNodeIr::GroupChar {
                body, annotation, ..
            } => {
                let (_, f0, _, rg0, ag0) = body.inspect_structure(in_script_or_frac);
                let (_, f1, _, _, ag1) = annotation
                    .as_ref()
                    .map_or((1, 0, false, false, false), |a| a.inspect_structure(true));
                (1, f0.max(f1), true, rg0, ag0 || ag1)
            }
            MathNodeIr::AlignPoint => (1, 0, false, false, false),
            MathNodeIr::Run(run) => {
                let non_ws = !run.text.trim().is_empty();
                (1, 0, false, non_ws && !in_script_or_frac, non_ws)
            }
        }
    }

    /// Estimate horizontal width in `em` units under Word OMML metrics.
    pub fn estimate_em_width(&self, display_top_level: bool) -> f64 {
        match self {
            MathNodeIr::Sequence(children) => children
                .iter()
                .map(|c| c.estimate_em_width(display_top_level))
                .sum(),
            MathNodeIr::Fraction { num, denom, style } => match style {
                MathFracStyleIr::Vertical | MathFracStyleIr::NoBar => {
                    let scale = if display_top_level { 1.0 } else { 0.73 };
                    let w = num
                        .estimate_em_width(false)
                        .max(denom.estimate_em_width(false));
                    w * scale + 0.28
                }
                MathFracStyleIr::Skewed | MathFracStyleIr::Horizontal => {
                    num.estimate_em_width(false) + 0.45 + denom.estimate_em_width(false)
                }
            },
            MathNodeIr::Radical { degree, radicand } => {
                let deg_w = degree
                    .as_ref()
                    .map_or(0.0, |d| d.estimate_em_width(false) * 0.6);
                0.65 + deg_w + radicand.estimate_em_width(display_top_level)
            }
            MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
                let bw = base.estimate_em_width(display_top_level);
                let sub_w = sub.as_ref().map_or(0.0, |s| s.estimate_em_width(false));
                let sup_w = sup.as_ref().map_or(0.0, |s| s.estimate_em_width(false));
                bw + 0.73 * sub_w.max(sup_w) + 0.04
            }
            MathNodeIr::Limit { base, lower, upper } => {
                let bw = base.estimate_em_width(display_top_level);
                let lw = lower
                    .as_ref()
                    .map_or(0.0, |l| l.estimate_em_width(false) * 0.73);
                let uw = upper
                    .as_ref()
                    .map_or(0.0, |u| u.estimate_em_width(false) * 0.73);
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
                    .map_or(0.0, |s| s.estimate_em_width(false) * 0.73);
                let sup_w = sup
                    .as_ref()
                    .map_or(0.0, |s| s.estimate_em_width(false) * 0.73);
                let op_w = if *limits_under_over {
                    0.90_f64.max(sub_w).max(sup_w)
                } else {
                    0.85 + sub_w.max(sup_w)
                };
                op_w + 0.15 + operand.estimate_em_width(display_top_level)
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
                    .map(|e| e.estimate_em_width(display_top_level))
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
                        .map(|cell| cell.estimate_em_width(false))
                        .fold(0.0_f64, f64::max);
                    total_w += col_max;
                }
                total_w + cols.saturating_sub(1) as f64 * 0.75
            }
            MathNodeIr::EquationArray { rows } => rows
                .iter()
                .map(|r| r.estimate_em_width(false))
                .fold(0.0_f64, f64::max),
            MathNodeIr::Accent { base, .. } => base.estimate_em_width(display_top_level),
            MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
                body.estimate_em_width(display_top_level)
            }
            MathNodeIr::GroupChar {
                body, annotation, ..
            } => {
                let bw = body.estimate_em_width(display_top_level);
                let aw = annotation
                    .as_ref()
                    .map_or(0.0, |a| a.estimate_em_width(false) * 0.73);
                bw.max(aw)
            }
            MathNodeIr::AlignPoint => 0.0,
            MathNodeIr::Run(run) => run. text.chars().map(char_math_em_width).sum(),
        }
    }
}

fn char_math_em_width(c: char) -> f64 {
    if ('\u{4E00}'..='\u{9FFF}').contains(&c)
        || ('\u{3000}'..='\u{303F}').contains(&c)
        || ('\u{FF00}'..='\u{FFEF}').contains(&c)
    {
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
