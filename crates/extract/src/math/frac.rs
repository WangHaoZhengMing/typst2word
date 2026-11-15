//! Fractions (`frac`), Binomials (`binom`), and Radicals (`root`, `sqrt`) -> `MathNodeIr`.

use typst::foundations::{Packed, StyleChain};
use typst::math::{BinomElem, EquationElem, FracElem, FracStyle, MathSize, RootElem};

use crate::math::node::extract_math_node;
use crate::math::{MathStyleCtx, extract_math_child, wrap_nodes};
use ir::{MathFracStyleIr, MathNodeIr, MathRunIr};

pub(crate) fn extract_frac(
    frac: &Packed<FracElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let style = match frac.style.get(styles) {
        FracStyle::Vertical => MathFracStyleIr::Vertical,
        FracStyle::Skewed => MathFracStyleIr::Skewed,
        FracStyle::Horizontal => MathFracStyleIr::Horizontal,
    };
    let display_style = match (styles.get(EquationElem::size), ctx.is_display) {
        (MathSize::Display, false) => Some(true),
        (MathSize::Text, true) => Some(false),
        _ => None,
    };
    MathNodeIr::Fraction {
        num: Box::new(extract_math_child(&frac.num, styles, ctx)),
        denom: Box::new(extract_math_child(&frac.denom, styles, ctx)),
        style,
        display_style,
    }
}

pub(crate) fn extract_binom(
    binom: &Packed<BinomElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let mut denom_nodes = Vec::new();
    for (idx, item) in binom.lower.iter().enumerate() {
        if idx > 0 {
            denom_nodes.push(MathNodeIr::Run(MathRunIr {
                text: ", ".to_string(),
                ..Default::default()
            }));
        }
        extract_math_node(item, styles, ctx, &mut denom_nodes);
    }
    let display_style = match (styles.get(EquationElem::size), ctx.is_display) {
        (MathSize::Display, false) => Some(true),
        (MathSize::Text, true) => Some(false),
        _ => None,
    };
    let stack = MathNodeIr::Fraction {
        num: Box::new(extract_math_child(&binom.upper, styles, ctx)),
        denom: Box::new(wrap_nodes(denom_nodes)),
        style: MathFracStyleIr::NoBar,
        display_style,
    };
    MathNodeIr::Delimited {
        open: "(".to_string(),
        close: ")".to_string(),
        separator: String::new(),
        elements: vec![stack],
    }
}

pub(crate) fn extract_root(
    root: &Packed<RootElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let degree = root
        .index
        .get_ref(styles)
        .as_ref()
        .map(|idx| Box::new(extract_math_child(idx, styles, ctx)));
    let radicand = Box::new(extract_math_child(&root.radicand, styles, ctx));
    MathNodeIr::Radical { degree, radicand }
}
