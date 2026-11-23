//! Semantic Intermediate Representation for Mathematical Equations (`EquationIr` & `MathNodeIr`).
//!
//! Pure data definitions capturing Typst's `typst::math` AST and laid-out PDF frame bounds
//! (`EquationPdfMetricsIr`).

use serde::{Deserialize, Serialize};

use crate::{ColorHex, NodeId, TextRunIr};
use crate::reference_layout::EquationLayoutMetricsIr;

/// Mathematical equation IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationIr {
    #[serde(default, skip_serializing_if = "NodeId::is_unset")]
    pub id: NodeId,
    pub is_block: bool,
    /// Structured semantic math AST nodes.
    pub body: Vec<MathNodeIr>,
    /// Plain-text/TeX-like fallback representation for accessibility or debugging.
    pub source_text: String,
    /// Optional equation number text (e.g., `"(1)"` or `"(2.1)"`).
    pub numbering_text: Option<String>,
    /// Laid-out equation label, retaining font changes within the number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub numbering_runs: Vec<TextRunIr>,
    /// Optional bookmark name for `@ref` cross-referencing.
    pub bookmark: Option<String>,
    /// Realized math face (OpenType family name), including local equation font overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math_font_family: Option<String>,
    /// Base Typst font size in points.
    #[serde(default)]
    pub font_size_pt: Option<f64>,
    /// Reference layout measurements from Typst's laid-out frame.
    #[serde(default, alias = "pdf_metrics")]
    pub layout_metrics: EquationLayoutMetricsIr,
}

/// Semantic math AST node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MathNodeIr {
    /// A horizontal sequence of math nodes.
    Sequence(Vec<MathNodeIr>),
    /// Fraction or binomial stack (`frac`, `skewed`, `binom`).
    Fraction {
        num: Box<MathNodeIr>,
        denom: Box<MathNodeIr>,
        style: MathFracStyleIr,
        /// Explicit math size override (`Some(true)` for `display(...)` inside inline math,
        /// `Some(false)` for `inline(...)` inside display math).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display_style: Option<bool>,
    },
    /// Square root or $n$-th root (`sqrt`, `root`).
    Radical {
        degree: Option<Box<MathNodeIr>>,
        radicand: Box<MathNodeIr>,
    },
    /// Right subscript and/or superscript (`attach`).
    SubSup {
        base: Box<MathNodeIr>,
        sub: Option<Box<MathNodeIr>>,
        sup: Option<Box<MathNodeIr>>,
    },
    /// Pre-subscript and/or pre-superscript (`attach(base, tl: ..., bl: ...)`).
    PreSubSup {
        base: Box<MathNodeIr>,
        sub: Option<Box<MathNodeIr>>,
        sup: Option<Box<MathNodeIr>>,
    },
    /// Under-limit and/or over-limit on an operator (`limits(...)`).
    Limit {
        base: Box<MathNodeIr>,
        lower: Option<Box<MathNodeIr>>,
        upper: Option<Box<MathNodeIr>>,
    },
    /// Big $n$-ary operator such as $\sum$, $\prod$, $\int$.
    Nary {
        chr: char,
        limits_under_over: bool,
        sub: Option<Box<MathNodeIr>>,
        sup: Option<Box<MathNodeIr>>,
        operand: Box<MathNodeIr>,
    },
    /// Left/right fenced expression (`lr(...)`, `binom`, `vec`, `mat`, `cases`).
    Delimited {
        open: String,
        close: String,
        separator: String,
        elements: Vec<MathNodeIr>,
    },
    /// 2D matrix grid (`mat`, `vec`, `cases`).
    Matrix {
        rows: Vec<Vec<MathNodeIr>>,
        /// Whether columns are left-aligned with compact piecewise spacing (`cases(...)`).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        left_aligned: bool,
    },
    /// Multi-line aligned equation array.
    EquationArray { rows: Vec<MathNodeIr> },
    /// Diacritical accent above a base (`hat`, `tilde`, `vec`, `dot`, ...).
    Accent { chr: char, base: Box<MathNodeIr> },
    /// Overbar or underbar (`overline`, `underline`).
    Bar {
        pos_bottom: bool,
        body: Box<MathNodeIr>,
    },
    /// Horizontal grouping brace/bracket/paren/shell with optional annotation (`overbrace`, `underbrace`, ...).
    GroupChar {
        chr: char,
        is_bottom: bool,
        body: Box<MathNodeIr>,
        annotation: Option<Box<MathNodeIr>>,
    },
    /// Mathematical function application (`sin x`, `cos 2α`, `ln x`), rendered as OMML `<m:func>`.
    Func {
        name: Box<MathNodeIr>,
        arg: Box<MathNodeIr>,
    },
    /// Diagonal strike/cancellation box (`cancel`).
    Cancel {
        body: Box<MathNodeIr>,
        cross: bool,
        inverted: bool,
    },
    /// Alignment point (`&`) inside an `EquationArray`.
    AlignPoint,
    /// Styled math text/symbol run.
    Run(MathRunIr),
}

/// Fraction bar rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathFracStyleIr {
    /// Standard horizontal fraction bar.
    Vertical,
    /// Skewed / diagonal fraction.
    Skewed,
    /// Inline linear fraction.
    Horizontal,
    /// Barless stack (used inside `binom`).
    NoBar,
}

/// Math script/font family variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathScriptIr {
    DoubleStruck,
    Script,
    Fraktur,
    Monospace,
    SansSerif,
}

/// Math character style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathStyleIr {
    /// Plain / upright (`upright`).
    Plain,
    /// Bold upright (`bold(upright(...))`).
    Bold,
    /// Italic (`italic`).
    Italic,
    /// Bold italic (`bold(italic(...))`).
    BoldItalic,
}

/// A leaf text or symbol run inside a math equation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MathRunIr {
    /// Typst shaped this run with a text face inside the equation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub normal_text: bool,
    pub text: String,
    pub script: Option<MathScriptIr>,
    pub style: Option<MathStyleIr>,
    pub color: Option<ColorHex>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size_pt: Option<f64>,
}
