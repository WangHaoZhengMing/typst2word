//! Accents (`hat`, `tilde`, ...), Over/Underbars, Group Braces (`underbrace`, ...),
//! and Diagonal Cancellation (`cancel`) -> `MathNodeIr`.

use typst::foundations::{Content, Packed, StyleChain};
use typst::math::{
    AccentElem, CancelElem, OverbraceElem, OverbracketElem, OverlineElem as MathOverlineElem,
    OverparenElem, OvershellElem, UnderbraceElem, UnderbracketElem,
    UnderlineElem as MathUnderlineElem, UnderparenElem, UndershellElem,
};
use typst::text::{OverlineElem as TextOverlineElem, UnderlineElem as TextUnderlineElem};

use crate::extract::math::{MathStyleCtx, extract_math_child};
use crate::ir::MathNodeIr;

pub(crate) fn extract_accent(
    acc: &Packed<AccentElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    MathNodeIr::Accent {
        chr: acc.accent.0,
        base: Box::new(extract_math_child(&acc.base, styles, ctx)),
    }
}

pub(crate) fn try_extract_bar(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> Option<MathNodeIr> {
    if let Some(ul) = content.to_packed::<MathUnderlineElem>() {
        return Some(MathNodeIr::Bar {
            pos_bottom: true,
            body: Box::new(extract_math_child(&ul.body, styles, ctx)),
        });
    }
    if let Some(ol) = content.to_packed::<MathOverlineElem>() {
        return Some(MathNodeIr::Bar {
            pos_bottom: false,
            body: Box::new(extract_math_child(&ol.body, styles, ctx)),
        });
    }
    if let Some(ul) = content.to_packed::<TextUnderlineElem>() {
        return Some(MathNodeIr::Bar {
            pos_bottom: true,
            body: Box::new(extract_math_child(&ul.body, styles, ctx)),
        });
    }
    if let Some(ol) = content.to_packed::<TextOverlineElem>() {
        return Some(MathNodeIr::Bar {
            pos_bottom: false,
            body: Box::new(extract_math_child(&ol.body, styles, ctx)),
        });
    }
    None
}

pub(crate) fn try_extract_group_char(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> Option<MathNodeIr> {
    let (body, ann_opt, chr, is_bottom) = if let Some(el) = content.to_packed::<UnderbraceElem>() {
        (&el.body, el.annotation.get_ref(styles), '⏟', true)
    } else if let Some(el) = content.to_packed::<OverbraceElem>() {
        (&el.body, el.annotation.get_ref(styles), '⏞', false)
    } else if let Some(el) = content.to_packed::<UnderbracketElem>() {
        (&el.body, el.annotation.get_ref(styles), '⎵', true)
    } else if let Some(el) = content.to_packed::<OverbracketElem>() {
        (&el.body, el.annotation.get_ref(styles), '⎴', false)
    } else if let Some(el) = content.to_packed::<UnderparenElem>() {
        (&el.body, el.annotation.get_ref(styles), '⏝', true)
    } else if let Some(el) = content.to_packed::<OverparenElem>() {
        (&el.body, el.annotation.get_ref(styles), '⏜', false)
    } else if let Some(el) = content.to_packed::<UndershellElem>() {
        (&el.body, el.annotation.get_ref(styles), '⏡', true)
    } else if let Some(el) = content.to_packed::<OvershellElem>() {
        (&el.body, el.annotation.get_ref(styles), '⏠', false)
    } else {
        return None;
    };

    let mut ann_ctx = ctx;
    ann_ctx.is_display = false;
    let annotation = ann_opt
        .as_ref()
        .map(|ann| Box::new(extract_math_child(ann, styles, ann_ctx)));

    Some(MathNodeIr::GroupChar {
        chr,
        is_bottom,
        body: Box::new(extract_math_child(body, styles, ctx)),
        annotation,
    })
}

pub(crate) fn extract_cancel(
    cancel: &Packed<CancelElem>,
    styles: StyleChain,
    ctx: MathStyleCtx,
) -> MathNodeIr {
    MathNodeIr::Cancel {
        body: Box::new(extract_math_child(&cancel.body, styles, ctx)),
        cross: cancel.cross.get(styles),
        inverted: cancel.inverted.get(styles),
    }
}
