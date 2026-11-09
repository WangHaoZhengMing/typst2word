//! Vectors (`vec`), Matrices (`mat`), and Piecewise Cases (`cases`) -> `MathNodeIr`.

use typst::foundations::{Packed, StyleChain};
use typst::math::{CasesElem, MatElem, VecElem};

use crate::math::{
    MathStyleCtx, extract_math_child, extract_sequence_as_node, split_by_align_point,
};
use ir::MathNodeIr;

pub(crate) fn extract_vec(
    vec_elem: &Packed<VecElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let delim = vec_elem.delim.get(styles);
    let open = delim.open().map(|c| c.to_string()).unwrap_or_default();
    let close = delim.close().map(|c| c.to_string()).unwrap_or_default();
    let rows = vec_elem
        .children
        .iter()
        .map(|item| vec![extract_math_child(item, styles, ctx)])
        .collect();
    MathNodeIr::Delimited {
        open,
        close,
        separator: String::new(),
        elements: vec![MathNodeIr::Matrix { rows }],
    }
}

pub(crate) fn extract_mat(
    mat_elem: &Packed<MatElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let delim = mat_elem.delim.get(styles);
    let open = delim.open().map(|c| c.to_string()).unwrap_or_default();
    let close = delim.close().map(|c| c.to_string()).unwrap_or_default();
    let rows = mat_elem
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| extract_math_child(cell, styles, ctx))
                .collect()
        })
        .collect();
    MathNodeIr::Delimited {
        open,
        close,
        separator: String::new(),
        elements: vec![MathNodeIr::Matrix { rows }],
    }
}

pub(crate) fn extract_cases(
    cases_elem: &Packed<CasesElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    let delim = cases_elem.delim.get(styles);
    let reverse = cases_elem.reverse.get(styles);
    let (open, close) = if reverse {
        (
            String::new(),
            delim
                .close()
                .or_else(|| delim.open())
                .unwrap_or('}')
                .to_string(),
        )
    } else {
        (delim.open().unwrap_or('{').to_string(), String::new())
    };
    let rows = cases_elem
        .children
        .iter()
        .map(|branch| {
            split_by_align_point(branch, styles)
                .iter()
                .map(|col_items| extract_sequence_as_node(col_items, ctx))
                .collect()
        })
        .collect();
    MathNodeIr::Delimited {
        open,
        close,
        separator: String::new(),
        elements: vec![MathNodeIr::Matrix { rows }],
    }
}
