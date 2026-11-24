//! Word Equation Construct (`<m:oMath>` / `<m:oMathPara>` & 2-DoF Box Calibrator).
//!
//! Bridges [`EquationIr`] (pure Typst AST + PDF physical measurements) to
//! [`crate::WordEquationBox`] and solves the block equation container construct
//! ([`EquationBlockConstruct`]: centered display paragraph vs 3-column borderless numbered row)
//! and calibrated Office Math nodes ([`WordMathNode`], [`WordMathRun`]).

use ir::{
    ColorHex, EquationIr, EquationLayoutMetricsIr, InlineNode, MathFracStyleIr, MathNodeIr,
    MathScriptIr, MathStyleIr,
};

use crate::pipeline::lang_script::TextScriptClassIr;
use crate::units::pt_to_dxa;

/// Calibrated Word Office Math (`<m:oMath>`) AST node solved by [`crate::WordEquationBox`].
#[derive(Debug, Clone)]
pub enum WordMathNode {
    Sequence(Vec<WordMathNode>),
    Fraction {
        num: Box<WordMathNode>,
        denom: Box<WordMathNode>,
        style: MathFracStyleIr,
        /// Optional `<m:fPr><m:ctrlPr><w:rPr><w:sz>` override so Word computes fraction bar
        /// shift/clearance using the compensated display size rather than cramped inline size.
        ctrl_font_size_pt: Option<f64>,
    },
    Radical {
        degree: Option<Box<WordMathNode>>,
        radicand: Box<WordMathNode>,
        /// Optional `<m:radPr><m:ctrlPr><w:rPr><w:sz>` override when inside a scaled fraction.
        ctrl_font_size_pt: Option<f64>,
    },
    SubSup {
        base: Box<WordMathNode>,
        sub: Option<Box<WordMathNode>>,
        sup: Option<Box<WordMathNode>>,
    },
    PreSubSup {
        base: Box<WordMathNode>,
        sub: Option<Box<WordMathNode>>,
        sup: Option<Box<WordMathNode>>,
    },
    Limit {
        base: Box<WordMathNode>,
        lower: Option<Box<WordMathNode>>,
        upper: Option<Box<WordMathNode>>,
    },
    Nary {
        chr: char,
        limits_under_over: bool,
        sub: Option<Box<WordMathNode>>,
        sup: Option<Box<WordMathNode>>,
        operand: Box<WordMathNode>,
    },
    Delimited {
        open: String,
        close: String,
        separator: String,
        elements: Vec<WordMathNode>,
    },
    Matrix {
        rows: Vec<Vec<WordMathNode>>,
        left_aligned: bool,
    },
    EquationArray {
        rows: Vec<WordMathNode>,
    },
    Accent {
        chr: char,
        base: Box<WordMathNode>,
    },
    Bar {
        pos_bottom: bool,
        body: Box<WordMathNode>,
    },
    GroupChar {
        chr: char,
        is_bottom: bool,
        body: Box<WordMathNode>,
        annotation: Option<Box<WordMathNode>>,
    },
    Func {
        name: Box<WordMathNode>,
        arg: Box<WordMathNode>,
    },
    Cancel {
        body: Box<WordMathNode>,
        cross: bool,
        inverted: bool,
    },
    AlignPoint,
    Run(WordMathRun),
}

/// Calibrated Word math leaf run (`<m:r>`) carrying solved font size, horizontal scaling (`<w:w>`),
/// and CJK normal-text flag (`<m:nor/>`).
#[derive(Debug, Clone, Default)]
pub struct WordMathRun {
    pub normal_text: bool,
    pub text: String,
    pub script: Option<MathScriptIr>,
    pub style: Option<MathStyleIr>,
    pub color: Option<ColorHex>,
    pub font_family: Option<String>,
    pub font_size_pt: Option<f64>,
    pub width_scale_pct: Option<u32>,
    /// Optional `<w:position w:val="..."/>` vertical baseline shift in half-points (used to tighten
    /// tall-`hhea` CJK runs inside OMML vertical fractions toward the fraction bar).
    pub position_half_pt: Option<i32>,
    pub has_cjk: bool,
}

/// Exhaustive block equation container construct in WordprocessingML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquationBlockConstruct<'a> {
    /// Standard unnumbered display equation (`<w:p>` containing `<m:oMathPara>`).
    CenteredDisplayParagraph {
        indent_left_dxa: usize,
        before_dxa: i64,
        after_dxa: i64,
        mark_sz_half_pt: u32,
    },
    /// Numbered display equation rendered as Word's standard 3-column borderless layout row (`[12% | 76% | 12%]`).
    NumberedThreeColumnRow {
        total_dxa: i64,
        side_dxa: i64,
        center_dxa: i64,
        before_dxa: i64,
        after_dxa: i64,
        num_half_pts: u32,
        number_text: &'a str,
    },
}

impl<'a> EquationBlockConstruct<'a> {
    /// Fallback `(before_dxa, after_dxa)` for a standalone block equation when reference flow spans are unavailable.
    pub fn default_flow_spacing_dxa(eq: &EquationIr) -> (i64, i64) {
        if eq.numbering_text.is_some() {
            return (0, 0);
        }
        let eq_box = solve_word_equation_box(eq);
        (eq_box.block_before_dxa, eq_box.block_after_dxa)
    }

    pub fn solve(
        eq: &'a EquationIr,
        page_content_width_pt: f64,
        before_dxa: i64,
        after_dxa: i64,
        indent_left_dxa: usize,
    ) -> Self {
        let mark_sz_half_pt =
            ((eq.font_size_pt.unwrap_or(11.0) * 2.0).round() as u32).clamp(8, 144);
        if let Some(num) = eq.numbering_text.as_deref() {
            let content_w_pt = page_content_width_pt.max(144.0);
            let total_dxa = pt_to_dxa(content_w_pt);
            let side_dxa = (total_dxa * 12 / 100).max(480);
            let center_dxa = (total_dxa - 2 * side_dxa).max(960);
            Self::NumberedThreeColumnRow {
                total_dxa,
                side_dxa,
                center_dxa,
                before_dxa: before_dxa.max(0),
                after_dxa: after_dxa.max(0),
                num_half_pts: mark_sz_half_pt,
                number_text: num,
            }
        } else {
            Self::CenteredDisplayParagraph {
                indent_left_dxa,
                before_dxa: before_dxa.max(0),
                after_dxa: after_dxa.max(0),
                mark_sz_half_pt,
            }
        }
    }
}

/// Structural metrics extracted from a `MathNodeIr` AST for [`crate::WordEquationBox`].
#[derive(Debug, Clone, Copy)]
pub struct MathAstStructure {
    pub row_count: usize,
    pub max_fraction_depth: usize,
    /// Whether any vertical/noBar fraction explicitly sets `display_style: Some(true)`.
    pub has_display_fraction: bool,
    pub has_tall_construct: bool,
    /// True when the AST contains 2D vertical math structures (vertical fractions, radicals,
    /// n-ary operators, limits, matrices, or equation arrays) whose OMML bounding box is clipped
    /// by Word/WPS whenever the enclosing paragraph uses sub-single `w:line < 240`.
    pub requires_full_line_box: bool,
    pub all_glyphs_in_fraction_or_script: bool,
    pub expected_em_width: f64,
}

/// Whether `content` contains any inline equation whose OMML box requires at least single-line
/// spacing (`w:line >= 240`) so Word/WPS never clips vertical fractions, radicals, or n-ary operators.
pub fn inlines_require_full_line_box(content: &[InlineNode]) -> bool {
    content.iter().any(|inl| match inl {
        InlineNode::EquationInline(eq) => {
            analyze_math_slice(&eq.body, eq.is_block).requires_full_line_box
        }
        InlineNode::Link { content: inner, .. }
        | InlineNode::Ref { content: inner, .. }
        | InlineNode::Citation { content: inner, .. }
        | InlineNode::Field { content: inner, .. } => inlines_require_full_line_box(inner),
        _ => false,
    })
}

/// Solve [`crate::WordEquationBox`] for an [`EquationIr`] from its `EquationLayoutMetricsIr` and AST structure.
pub fn solve_word_equation_box(eq: &EquationIr) -> crate::WordEquationBox {
    let ast_metrics = analyze_math_slice(&eq.body, eq.is_block);
    let (
        typst_width_pt,
        typst_height_pt,
        typst_max_glyph_pt,
        typst_min_glyph_pt,
        context_font_size_pt,
        max_available_width_pt,
    ) = match eq.layout_metrics {
        EquationLayoutMetricsIr::Unmeasured {
            context_font_size_pt,
            max_glyph_pt,
            min_glyph_pt,
            max_available_width_pt,
        } => (
            None,
            None,
            max_glyph_pt,
            min_glyph_pt,
            context_font_size_pt,
            max_available_width_pt,
        ),
        EquationLayoutMetricsIr::Measured {
            context_font_size_pt,
            width_pt,
            height_pt,
            max_glyph_pt,
            min_glyph_pt,
            max_available_width_pt,
        } => (
            Some(width_pt),
            Some(height_pt),
            max_glyph_pt,
            min_glyph_pt,
            context_font_size_pt,
            max_available_width_pt,
        ),
    };
    crate::WordEquationBox::solve(crate::EquationBoxInput {
        is_block: eq.is_block,
        typst_width_pt,
        typst_height_pt,
        typst_max_glyph_pt,
        typst_min_glyph_pt,
        context_font_size_pt,
        max_available_width_pt,
        row_count: ast_metrics.row_count,
        max_fraction_depth: ast_metrics.max_fraction_depth,
        has_display_fraction: ast_metrics.has_display_fraction,
        has_tall_construct: ast_metrics.has_tall_construct,
        all_glyphs_in_fraction_or_script: ast_metrics.all_glyphs_in_fraction_or_script,
        expected_em_width: ast_metrics.expected_em_width,
    })
}

/// Return a calibrated [`WordMathNode`] tree for `eq.body` with `run_font_size_pt`,
/// `fraction_font_size_pt`, and `width_scale_pct` solved by [`crate::WordEquationBox`].
pub fn calibrate_equation_body(eq: &EquationIr) -> Vec<WordMathNode> {
    let eq_box = solve_word_equation_box(eq);
    let ctx = MathCalibCtx {
        is_block: eq.is_block,
        run_font_size_pt: eq_box.run_font_size_pt,
        fraction_font_size_pt: eq_box.fraction_font_size_pt,
        width_scale_pct: eq_box.width_scale_pct,
        fraction_slot: FractionSlot::None,
    };
    ctx.calibrate_slice(&eq.body)
}

pub fn analyze_math_slice(nodes: &[MathNodeIr], is_block: bool) -> MathAstStructure {
    let mut summary = NodeStructSummary::identity();
    let mut expected_em_width = 0.0_f64;

    for node in nodes {
        summary.merge(inspect_node_structure(node, false));
        expected_em_width += estimate_node_em_width(node, is_block && summary.rows == 1);
    }

    MathAstStructure {
        row_count: summary.rows,
        max_fraction_depth: summary.max_fraction_depth,
        has_display_fraction: summary.has_display_fraction,
        has_tall_construct: summary.has_tall_construct,
        requires_full_line_box: summary.requires_full_line_box,
        all_glyphs_in_fraction_or_script: summary.has_any_glyph && !summary.has_root_unscaled_glyph,
        expected_em_width: expected_em_width.max(0.5),
    }
}

pub fn calibrate_math_nodes(
    nodes: &[MathNodeIr],
    run_font_size_pt: f64,
    fraction_font_size_pt: Option<f64>,
    width_scale_pct: Option<u32>,
) -> Vec<WordMathNode> {
    let ctx = MathCalibCtx {
        is_block: false,
        run_font_size_pt,
        fraction_font_size_pt,
        width_scale_pct,
        fraction_slot: FractionSlot::None,
    };
    ctx.calibrate_slice(nodes)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FractionSlot {
    None,
    Numerator,
    Denominator,
}

#[derive(Clone, Copy)]
struct MathCalibCtx {
    is_block: bool,
    run_font_size_pt: f64,
    fraction_font_size_pt: Option<f64>,
    width_scale_pct: Option<u32>,
    fraction_slot: FractionSlot,
}

/// Whether `node` starts with a closing fence/delimiter or whitespace (so a preceding `,` should
/// NOT append a math punctuation thin space).
fn starts_with_closing_or_space(node: &MathNodeIr) -> bool {
    match node {
        MathNodeIr::Run(r) => r.text.chars().next().is_none_or(|ch| {
            ch.is_whitespace() || matches!(ch, ')' | ']' | '}' | '⟩' | '⟫' | '⌋' | '⌉' | '|')
        }),
        MathNodeIr::Sequence(children) => children
            .first()
            .is_none_or(starts_with_closing_or_space),
        _ => false,
    }
}

/// Normalize Typst math symbols to the Unicode codepoints that Microsoft Word's OMML engine
/// recognizes with matching mathematical spacing classes:
/// - `U+27C2` (`⟂`, Typst `perp`) -> `U+22A5` (`⊥`, relation spacing `AE ⊥ CE`).
/// - Bare `|` (`U+007C`, e.g. set-builder `{x | √x = x}`) -> `U+2223` (`∣`, relation spacing).
///   Paired absolute-value bars (`|a + b|`) are extracted as `MathNodeIr::Delimited` and untouched.
fn normalize_omml_math_text(text: &str, normal_text: bool) -> String {
    if normal_text {
        return text.to_string();
    }
    if text == "|" {
        return "∣".to_string();
    }
    if text.contains('\u{27C2}') {
        return text.replace('\u{27C2}', "\u{22A5}");
    }
    text.to_string()
}

impl MathCalibCtx {
    #[inline]
    fn map_box(self, node: &MathNodeIr) -> Box<WordMathNode> {
        Box::new(self.calibrate(node))
    }

    #[inline]
    fn map_opt(self, opt: Option<&Box<MathNodeIr>>) -> Option<Box<WordMathNode>> {
        opt.map(|n| self.map_box(n))
    }

    fn calibrate_slice(self, nodes: &[MathNodeIr]) -> Vec<WordMathNode> {
        let mut out = Vec::with_capacity(nodes.len());
        for (i, node) in nodes.iter().enumerate() {
            let mut calibrated = self.calibrate(node);
            // Word's OMML engine treats `digit,digit` (e.g. `{0,1,3,6,9}`, `(1,0)`) as a European
            // decimal separator with zero space after `,`. Append a six-per-em math punctuation
            // space (`U+2006`) whenever a comma run is followed by another math atom.
            if let WordMathNode::Run(ref mut r) = calibrated
                && !r.normal_text
                && r.text.ends_with(',')
                && nodes
                    .get(i + 1)
                    .is_some_and(|next| !starts_with_closing_or_space(next))
            {
                r.text.push('\u{2006}');
            }
            out.push(calibrated);
        }
        out
    }

    fn calibrate(self, node: &MathNodeIr) -> WordMathNode {
        match node {
            MathNodeIr::Sequence(children) => {
                WordMathNode::Sequence(self.calibrate_slice(children))
            }
            MathNodeIr::Fraction {
                num,
                denom,
                style,
                display_style,
            } => {
                let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
                // Explicit `display(...)` / `inline(...)` math size overrides on vertical fractions
                // take precedence even when nested subscripts/radicals lower `typst_min_glyph_pt`.
                let local_frac_sz = match (is_vert, display_style) {
                    (true, Some(true)) => crate::FractionScaleMode::InlineDisplayCompensated
                        .resolve_font_size_pt(self.run_font_size_pt),
                    (true, Some(false)) => crate::FractionScaleMode::BlockInlineCompensated
                        .resolve_font_size_pt(self.run_font_size_pt),
                    _ => self.fraction_font_size_pt,
                };
                let num_ctx = Self {
                    fraction_font_size_pt: local_frac_sz,
                    fraction_slot: if is_vert {
                        FractionSlot::Numerator
                    } else {
                        self.fraction_slot
                    },
                    ..self
                };
                let denom_ctx = Self {
                    fraction_font_size_pt: local_frac_sz,
                    fraction_slot: if is_vert {
                        FractionSlot::Denominator
                    } else {
                        self.fraction_slot
                    },
                    ..self
                };
                WordMathNode::Fraction {
                    num: num_ctx.map_box(num),
                    denom: denom_ctx.map_box(denom),
                    style: *style,
                    ctrl_font_size_pt: local_frac_sz,
                }
            }
            MathNodeIr::Radical { degree, radicand } => WordMathNode::Radical {
                degree: self.map_opt(degree.as_ref()),
                radicand: self.map_box(radicand),
                ctrl_font_size_pt: (self.fraction_slot != FractionSlot::None)
                    .then_some(self.fraction_font_size_pt)
                    .flatten(),
            },
            MathNodeIr::SubSup { base, sub, sup } => WordMathNode::SubSup {
                base: self.map_box(base),
                sub: self.map_opt(sub.as_ref()),
                sup: self.map_opt(sup.as_ref()),
            },
            MathNodeIr::PreSubSup { base, sub, sup } => WordMathNode::PreSubSup {
                base: self.map_box(base),
                sub: self.map_opt(sub.as_ref()),
                sup: self.map_opt(sup.as_ref()),
            },
            MathNodeIr::Limit { base, lower, upper } => WordMathNode::Limit {
                base: self.map_box(base),
                lower: self.map_opt(lower.as_ref()),
                upper: self.map_opt(upper.as_ref()),
            },
            MathNodeIr::Nary {
                chr,
                limits_under_over,
                sub,
                sup,
                operand,
            } => WordMathNode::Nary {
                chr: *chr,
                limits_under_over: *limits_under_over,
                sub: self.map_opt(sub.as_ref()),
                sup: self.map_opt(sup.as_ref()),
                operand: self.map_box(operand),
            },
            MathNodeIr::Delimited {
                open,
                close,
                separator,
                elements,
            } => WordMathNode::Delimited {
                open: open.clone(),
                close: close.clone(),
                separator: separator.clone(),
                elements: elements.iter().map(|el| self.calibrate(el)).collect(),
            },
            MathNodeIr::Matrix { rows, left_aligned } => WordMathNode::Matrix {
                rows: rows
                    .iter()
                    .map(|row| row.iter().map(|cell| self.calibrate(cell)).collect())
                    .collect(),
                left_aligned: *left_aligned,
            },
            MathNodeIr::EquationArray { rows } => WordMathNode::EquationArray {
                rows: rows.iter().map(|row| self.calibrate(row)).collect(),
            },
            MathNodeIr::Accent { chr, base } => WordMathNode::Accent {
                chr: *chr,
                base: self.map_box(base),
            },
            MathNodeIr::Bar { pos_bottom, body } => WordMathNode::Bar {
                pos_bottom: *pos_bottom,
                body: self.map_box(body),
            },
            MathNodeIr::GroupChar {
                chr,
                is_bottom,
                body,
                annotation,
            } => WordMathNode::GroupChar {
                chr: *chr,
                is_bottom: *is_bottom,
                body: self.map_box(body),
                annotation: self.map_opt(annotation.as_ref()),
            },
            MathNodeIr::Func { name, arg } => WordMathNode::Func {
                name: self.map_box(name),
                arg: self.map_box(arg),
            },
            MathNodeIr::Cancel {
                body,
                cross,
                inverted,
            } => WordMathNode::Cancel {
                body: self.map_box(body),
                cross: *cross,
                inverted: *inverted,
            },
            MathNodeIr::AlignPoint => WordMathNode::AlignPoint,
            MathNodeIr::Run(run) => {
                let text = normalize_omml_math_text(&run.text, run.normal_text);
                let has_cjk = TextScriptClassIr::classify(&text).has_cjk();
                let in_vert_frac = self.fraction_slot != FractionSlot::None;
                let target_sz = if in_vert_frac {
                    if has_cjk {
                        // Word's OMML engine leaves `<m:nor/>` CJK runs unscaled inside inline
                        // `<m:f>`: preserve `1.0 * run_font_size_pt` for display/block CJK
                        // fractions (e.g. `频率/组距`), and explicitly scale inline CJK fractions
                        // by `0.73` to match Typst.
                        if self.is_block || self.fraction_font_size_pt.is_some() {
                            self.run_font_size_pt
                        } else {
                            ((self.run_font_size_pt * 0.73 * 2.0).round() / 2.0).clamp(5.0, 72.0)
                        }
                    } else {
                        self.fraction_font_size_pt.unwrap_or(self.run_font_size_pt)
                    }
                } else {
                    self.run_font_size_pt
                };
                // CJK fonts with large `hhea` internal leading (`Songti SC`, `PingFang SC`) leave
                // excessive vertical air around the OMML `<m:f>` bar; pull `<m:nor/>` CJK runs
                // toward the fraction bar via `<w:position>`.
                let position_half_pt = if has_cjk && in_vert_frac {
                    let shift = ((target_sz * 0.70).round() as i32).clamp(2, 9);
                    match self.fraction_slot {
                        FractionSlot::Numerator => Some(-shift),
                        FractionSlot::Denominator => Some(shift),
                        FractionSlot::None => None,
                    }
                } else {
                    None
                };
                WordMathNode::Run(WordMathRun {
                    normal_text: run.normal_text,
                    text,
                    script: run.script,
                    style: run.style,
                    color: run.color.clone(),
                    font_family: run.font_family.clone(),
                    font_size_pt: Some(target_sz),
                    width_scale_pct: self.width_scale_pct,
                    position_half_pt,
                    has_cjk,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct NodeStructSummary {
    rows: usize,
    max_fraction_depth: usize,
    has_display_fraction: bool,
    has_tall_construct: bool,
    requires_full_line_box: bool,
    has_root_unscaled_glyph: bool,
    has_any_glyph: bool,
}

impl NodeStructSummary {
    #[inline]
    const fn identity() -> Self {
        Self {
            rows: 1,
            max_fraction_depth: 0,
            has_display_fraction: false,
            has_tall_construct: false,
            requires_full_line_box: false,
            has_root_unscaled_glyph: false,
            has_any_glyph: false,
        }
    }

    #[inline]
    fn merge(&mut self, other: Self) {
        self.rows = self.rows.max(other.rows);
        self.max_fraction_depth = self.max_fraction_depth.max(other.max_fraction_depth);
        self.has_display_fraction |= other.has_display_fraction;
        self.has_tall_construct |= other.has_tall_construct;
        self.requires_full_line_box |= other.requires_full_line_box;
        self.has_root_unscaled_glyph |= other.has_root_unscaled_glyph;
        self.has_any_glyph |= other.has_any_glyph;
    }

    fn merge_slice(nodes: &[MathNodeIr], in_script_or_frac: bool) -> Self {
        let mut acc = Self::identity();
        for node in nodes {
            acc.merge(inspect_node_structure(node, in_script_or_frac));
        }
        acc
    }

    #[inline]
    fn merge_opt(&mut self, opt: Option<&Box<MathNodeIr>>, in_script_or_frac: bool) {
        if let Some(node) = opt {
            self.merge(inspect_node_structure(node, in_script_or_frac));
        }
    }
}

fn inspect_node_structure(node: &MathNodeIr, in_script_or_frac: bool) -> NodeStructSummary {
    match node {
        MathNodeIr::Sequence(children) | MathNodeIr::Delimited { elements: children, .. } => {
            NodeStructSummary::merge_slice(children, in_script_or_frac)
        }
        MathNodeIr::Fraction {
            num,
            denom,
            style,
            display_style,
        } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let mut acc = inspect_node_structure(num, true);
            acc.merge(inspect_node_structure(denom, true));
            NodeStructSummary {
                rows: 1,
                max_fraction_depth: if is_vert {
                    1 + acc.max_fraction_depth
                } else {
                    acc.max_fraction_depth
                },
                has_display_fraction: acc.has_display_fraction
                    || (is_vert && *display_style == Some(true)),
                has_tall_construct: is_vert || acc.has_tall_construct,
                requires_full_line_box: is_vert || acc.requires_full_line_box,
                has_root_unscaled_glyph: false,
                has_any_glyph: acc.has_any_glyph,
            }
        }
        MathNodeIr::Radical { degree, radicand } => {
            let mut acc = inspect_node_structure(radicand, in_script_or_frac);
            let root_g = acc.has_root_unscaled_glyph;
            acc.merge_opt(degree.as_ref(), true);
            acc.rows = 1;
            acc.has_tall_construct = true;
            acc.requires_full_line_box = true;
            acc.has_root_unscaled_glyph = root_g;
            acc
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            let mut acc = inspect_node_structure(base, in_script_or_frac);
            let root_g = acc.has_root_unscaled_glyph;
            acc.merge_opt(sub.as_ref(), true);
            acc.merge_opt(sup.as_ref(), true);
            acc.rows = 1;
            acc.has_tall_construct |= sub.is_some() || sup.is_some();
            acc.has_root_unscaled_glyph = root_g;
            acc
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let mut acc = inspect_node_structure(base, in_script_or_frac);
            let root_g = acc.has_root_unscaled_glyph;
            acc.merge_opt(lower.as_ref(), true);
            acc.merge_opt(upper.as_ref(), true);
            acc.rows = 1;
            acc.has_tall_construct = true;
            acc.requires_full_line_box = true;
            acc.has_root_unscaled_glyph = root_g;
            acc
        }
        MathNodeIr::Nary {
            sub, sup, operand, ..
        } => {
            let mut acc = inspect_node_structure(operand, in_script_or_frac);
            let root_g = acc.has_root_unscaled_glyph;
            acc.merge_opt(sub.as_ref(), true);
            acc.merge_opt(sup.as_ref(), true);
            acc.rows = 1;
            acc.has_tall_construct = true;
            acc.requires_full_line_box = true;
            acc.has_root_unscaled_glyph = root_g;
            acc.has_any_glyph = true;
            acc
        }
        MathNodeIr::Matrix { rows, .. } => {
            let r_cnt = rows.len().max(1);
            let mut acc = NodeStructSummary::identity();
            for row in rows {
                for cell in row {
                    acc.merge(inspect_node_structure(cell, in_script_or_frac));
                }
            }
            acc.rows = r_cnt;
            acc.has_tall_construct = r_cnt > 1;
            acc.requires_full_line_box |= r_cnt > 1;
            acc
        }
        MathNodeIr::EquationArray { rows } => {
            let r_cnt = rows.len().max(1);
            let mut acc = NodeStructSummary::merge_slice(rows, in_script_or_frac);
            acc.rows = r_cnt;
            acc.has_tall_construct = r_cnt > 1;
            acc.requires_full_line_box |= r_cnt > 1;
            acc
        }
        MathNodeIr::Accent { base, .. } => {
            let mut acc = inspect_node_structure(base, in_script_or_frac);
            acc.rows = 1;
            acc.has_tall_construct = true;
            acc
        }
        MathNodeIr::Func { name, arg } => {
            let mut acc = inspect_node_structure(name, in_script_or_frac);
            acc.merge(inspect_node_structure(arg, in_script_or_frac));
            acc
        }
        MathNodeIr::Bar { body, .. } | MathNodeIr::Cancel { body, .. } => {
            inspect_node_structure(body, in_script_or_frac)
        }
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            let mut acc = inspect_node_structure(body, in_script_or_frac);
            let root_g = acc.has_root_unscaled_glyph;
            acc.merge_opt(annotation.as_ref(), true);
            acc.rows = 1;
            acc.has_tall_construct = true;
            acc.has_root_unscaled_glyph = root_g;
            acc
        }
        MathNodeIr::AlignPoint => NodeStructSummary::identity(),
        MathNodeIr::Run(run) => {
            let non_empty = !run.text.trim().is_empty();
            NodeStructSummary {
                has_root_unscaled_glyph: non_empty && !in_script_or_frac,
                has_any_glyph: non_empty,
                ..NodeStructSummary::identity()
            }
        }
    }
}

fn estimate_node_em_width(node: &MathNodeIr, in_display_context: bool) -> f64 {
    match node {
        MathNodeIr::Sequence(children) => children
            .iter()
            .map(|c| estimate_node_em_width(c, in_display_context))
            .sum(),
        MathNodeIr::Fraction {
            num,
            denom,
            style,
            display_style,
        } => {
            let is_vert = matches!(style, MathFracStyleIr::Vertical | MathFracStyleIr::NoBar);
            let is_disp = display_style.unwrap_or(in_display_context);
            let child_scale = if is_vert && !is_disp { 0.73 } else { 1.0 };
            let nw = estimate_node_em_width(num, false) * child_scale;
            let dw = estimate_node_em_width(denom, false) * child_scale;
            if is_vert {
                nw.max(dw) + 0.35
            } else {
                nw + dw + 0.45
            }
        }
        MathNodeIr::Radical { degree, radicand } => {
            let deg_w = degree
                .as_ref()
                .map_or(0.0, |d| estimate_node_em_width(d, false) * 0.65);
            deg_w + 0.85 + estimate_node_em_width(radicand, in_display_context)
        }
        MathNodeIr::SubSup { base, sub, sup } | MathNodeIr::PreSubSup { base, sub, sup } => {
            let bw = estimate_node_em_width(base, in_display_context);
            let sub_w = sub
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            let sup_w = sup
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            bw + sub_w.max(sup_w)
        }
        MathNodeIr::Limit { base, lower, upper } => {
            let bw = estimate_node_em_width(base, in_display_context);
            let lw = lower
                .as_ref()
                .map_or(0.0, |l| estimate_node_em_width(l, false) * 0.70);
            let uw = upper
                .as_ref()
                .map_or(0.0, |u| estimate_node_em_width(u, false) * 0.70);
            bw.max(lw).max(uw) + 0.15
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
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            let sup_w = sup
                .as_ref()
                .map_or(0.0, |s| estimate_node_em_width(s, false) * 0.70);
            let op_w = estimate_node_em_width(operand, in_display_context);
            if *limits_under_over {
                1.15_f64.max(sub_w).max(sup_w) + 0.20 + op_w
            } else {
                1.05 + sub_w.max(sup_w) + 0.15 + op_w
            }
        }
        MathNodeIr::Delimited {
            open,
            close,
            separator,
            elements,
        } => {
            let open_w = if open.is_empty() { 0.0 } else { 0.42 };
            let close_w = if close.is_empty() { 0.0 } else { 0.42 };
            let sep_w = if separator.is_empty() { 0.0 } else { 0.35 };
            let elems_w: f64 = elements
                .iter()
                .map(|e| estimate_node_em_width(e, in_display_context))
                .sum();
            let seps_total = sep_w * elements.len().saturating_sub(1) as f64;
            open_w + elems_w + seps_total + close_w
        }
        MathNodeIr::Matrix { rows, left_aligned } => {
            let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut total_w = 0.0;
            for c in 0..num_cols {
                let col_max = rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .map(|cell| estimate_node_em_width(cell, false))
                    .fold(0.0_f64, f64::max);
                total_w += col_max;
            }
            let col_gap_em = if *left_aligned { 0.50 } else { 0.85 };
            total_w + num_cols.saturating_sub(1) as f64 * col_gap_em
        }
        MathNodeIr::EquationArray { rows } => rows
            .iter()
            .map(|r| estimate_node_em_width(r, true))
            .fold(0.0_f64, f64::max),
        MathNodeIr::Func { name, arg } => {
            estimate_node_em_width(name, in_display_context)
                + 0.18
                + estimate_node_em_width(arg, in_display_context)
        }
        MathNodeIr::Accent { base, .. }
        | MathNodeIr::Bar { body: base, .. }
        | MathNodeIr::Cancel { body: base, .. } => estimate_node_em_width(base, in_display_context),
        MathNodeIr::GroupChar {
            body, annotation, ..
        } => {
            let bw = estimate_node_em_width(body, in_display_context);
            let aw = annotation
                .as_ref()
                .map_or(0.0, |a| estimate_node_em_width(a, false) * 0.75);
            bw.max(aw)
        }
        MathNodeIr::AlignPoint => 0.0,
        MathNodeIr::Run(run) => run
            .text
            .chars()
            .map(|ch| {
                if crate::is_cjk_or_fullwidth_char(ch) {
                    1.0
                } else if ch.is_whitespace() {
                    0.28
                } else if matches!(
                    ch,
                    '+' | '-' | '=' | '−' | '×' | '÷' | '±' | '<' | '>' | '≤' | '≥' | '≠' | '≈'
                        | '→' | '←' | '⇒' | '⇔'
                ) {
                    0.78
                } else if ch.is_ascii_digit() {
                    0.55
                } else if ch.is_ascii_uppercase() {
                    0.66
                } else {
                    0.52
                }
            })
            .sum(),
    }
}
