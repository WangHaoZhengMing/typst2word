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
}
