//! Math symbols, text operators (`op`), primes, text runs, and inline formatting wrappers.

use typst::foundations::{Content, StyleChain, SymbolElem};
use typst::layout::{AlignElem, BoxElem, HElem, HideElem, PadElem, Spacing};
use typst::math::{EquationElem, OpElem, PrimesElem};
use typst::model::{EmphElem, LinkMarker, RefElem, StrongElem};
use typst::text::{
    FontStyle, HighlightElem, RawElem, SmartQuoteElem, SpaceElem, StrikeElem, SubElem, SuperElem,
    TextElem,
};
use typst::visualize::Paint;

use crate::math::MathStyleCtx;
use crate::math::node::extract_math_node;
use ir::{MathNodeIr, MathRunIr, MathScriptIr, MathStyleIr};

pub(crate) fn extract_symbol_or_wrapper(
    content: &Content,
    styles: StyleChain,
    ctx: MathStyleCtx,
    out: &mut Vec<MathNodeIr>,
) {
    // 1. Primes (`PrimesElem`)
    if let Some(primes) = content.to_packed::<PrimesElem>() {
        let s = match primes.count {
            0 => String::new(),
            1 => "′".to_string(),
            2 => "″".to_string(),
            3 => "‴".to_string(),
            4 => "⁗".to_string(),
            n => "′".repeat(n),
        };
        if let Some(run) = build_math_run(&s, styles, ctx, false) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }

    // 2. Text Operator (`OpElem`, e.g. `sin`, `cos`, `lim`, `max`, `det`, etc.)
    if let Some(op) = content.to_packed::<OpElem>() {
        let op_str = op.text.plain_text();
        if let Some(run) = build_math_run(&op_str, styles, ctx, true) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }

    // 3. Spaces (`SpaceElem`, `HElem`)
    if content.is::<SpaceElem>() {
        return;
    }
    if let Some(h) = content.to_packed::<HElem>() {
        if let Spacing::Rel(rel) = h.amount
            && !rel.is_zero()
        {
            let base_pt = ctx.base_font_size_pt.unwrap_or(11.0).max(1.0);
            let em_val = rel.abs.em.get() + rel.abs.abs.to_pt() / base_pt;
            let space_str = if em_val <= 0.20 {
                "\u{2009}".to_string()
            } else if em_val <= 0.35 {
                "\u{2005}".to_string()
            } else if em_val <= 0.70 {
                "\u{2002}".to_string()
            } else if em_val <= 1.40 {
                "\u{2001}".to_string()
            } else {
                "\u{2001}".repeat((em_val.round() as usize).clamp(2, 6))
            };
            if let Some(run) = build_math_run(&space_str, styles, ctx, true) {
                ctx.fonts.emit(content.span(), run, out);
            }
        }
        return;
    }

    // 4. Symbols (`SymbolElem`) and Text (`TextElem`, `RawElem`, `SmartQuoteElem`)
    if let Some(sym) = content.to_packed::<SymbolElem>() {
        if let Some(run) = build_math_run(&sym.text, styles, ctx, false) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }
    if let Some(txt) = content.to_packed::<TextElem>() {
        // In Typst math layout (`typst-layout/src/math/text.rs`), every `TextElem`
        // (quoted strings including single-letter upright symbols like `"H"`, `"O"`, `"S"`,
        // numbers, and text runs) disables math auto-italic unless explicitly italicized.
        // Bare single-letter math variables (`$x$`, `$H$`) are `SymbolElem`, not `TextElem`.
        let is_explicit_italic_text = matches!(
            styles.get(TextElem::style),
            FontStyle::Italic | FontStyle::Oblique
        );
        if let Some(run) = build_math_run(&txt.text, styles, ctx, !is_explicit_italic_text) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }
    if content.is::<RawElem>() {
        let t = content.plain_text();
        if let Some(run) = build_math_run(&t, styles, ctx, true) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }
    if content.is::<SmartQuoteElem>() {
        if let Some(run) = build_math_run("\"", styles, ctx, true) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }

    // 5. Inline formatting wrappers (`StrongElem`, `EmphElem`, `SubElem`, `SuperElem`, `StrikeElem`,
    //    `HighlightElem`, `BoxElem`, `PadElem`, `AlignElem`, `HideElem`, `LinkMarker`, `RefElem`)
    if let Some(strong) = content.to_packed::<StrongElem>() {
        let styled = strong.body.clone().set(EquationElem::bold, true);
        extract_math_node(&styled, styles, ctx, out);
        return;
    }
    if let Some(emph) = content.to_packed::<EmphElem>() {
        let styled = emph.body.clone().set(EquationElem::italic, Some(true));
        extract_math_node(&styled, styles, ctx, out);
        return;
    }
    if let Some(sub) = content.to_packed::<SubElem>() {
        extract_math_node(&sub.body, styles, ctx, out);
        return;
    }
    if let Some(sup) = content.to_packed::<SuperElem>() {
        extract_math_node(&sup.body, styles, ctx, out);
        return;
    }
    if let Some(strike) = content.to_packed::<StrikeElem>() {
        extract_math_node(&strike.body, styles, ctx, out);
        return;
    }
    if let Some(hl) = content.to_packed::<HighlightElem>() {
        extract_math_node(&hl.body, styles, ctx, out);
        return;
    }
    if let Some(bx) = content.to_packed::<BoxElem>() {
        if let Some(body) = bx.body.get_ref(styles) {
            extract_math_node(body, styles, ctx, out);
        }
        return;
    }
    if let Some(pad) = content.to_packed::<PadElem>() {
        extract_math_node(&pad.body, styles, ctx, out);
        return;
    }
    if let Some(align) = content.to_packed::<AlignElem>() {
        extract_math_node(&align.body, styles, ctx, out);
        return;
    }
    if content.is::<HideElem>() {
        return;
    }
    if let Some(link) = content.to_packed::<LinkMarker>() {
        extract_math_node(&link.body, styles, ctx, out);
        return;
    }
    if let Some(r) = content.to_packed::<RefElem>() {
        let t = format!("@{}", r.target.resolve());
        if let Some(run) = build_math_run(&t, styles, ctx, true) {
            ctx.fonts.emit(content.span(), run, out);
        }
        return;
    }

    // Fallback for any other content inside math: extract plain text.
    let fallback_txt = content.plain_text();
    if let Some(run) = build_math_run(&fallback_txt, styles, ctx, false) {
        ctx.fonts.emit(content.span(), run, out);
    }
}

pub(crate) fn build_math_run(
    text: &str,
    styles: StyleChain,
    ctx: MathStyleCtx,
    default_upright: bool,
) -> Option<MathRunIr> {
    if text.is_empty() {
        return None;
    }

    let bold = styles.get(EquationElem::bold);
    let italic = styles.get(EquationElem::italic);
    let variant = styles.get(EquationElem::variant);
    let variant_dbg = variant.map(|v| format!("{v:?}")).unwrap_or_default();

    let script = match variant_dbg.as_str() {
        "DoubleStruck" => Some(MathScriptIr::DoubleStruck),
        "Chancery" | "Roundhand" => Some(MathScriptIr::Script),
        "Fraktur" => Some(MathScriptIr::Fraktur),
        "Monospace" => Some(MathScriptIr::Monospace),
        "SansSerif" => Some(MathScriptIr::SansSerif),
        _ => None,
    };

    let is_typst_default_upright_symbol = text
        .chars()
        .all(|c| ('\u{0391}'..='\u{03A9}').contains(&c) || c == '\u{2207}');
    let effective_upright = default_upright || is_typst_default_upright_symbol;

    let style = match (bold, italic) {
        (true, Some(true)) => Some(MathStyleIr::BoldItalic),
        (true, Some(false)) => Some(MathStyleIr::Bold),
        (true, None) => {
            if effective_upright {
                Some(MathStyleIr::Bold)
            } else {
                Some(MathStyleIr::BoldItalic)
            }
        }
        (false, Some(false)) => Some(MathStyleIr::Plain),
        (false, Some(true)) => Some(MathStyleIr::Italic),
        (false, None) => {
            if effective_upright {
                Some(MathStyleIr::Plain)
            } else {
                None
            }
        }
    };

    let color = match styles.get_ref(TextElem::fill) {
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

    let has_cjk = text.chars().any(ir::is_cjk_char);
    let font_family = if has_cjk {
        ctx.cjk_font.map(str::to_string).or_else(|| {
            styles
                .get_ref(TextElem::font)
                .into_iter()
                .find(|f| crate::ExtractorContext::is_cjk_font_family(f.as_str()))
                .map(|f| f.as_str().to_string())
        })
    } else {
        ctx.math_font.map(str::to_string)
    };

    Some(MathRunIr {
        normal_text: false,
        text: text.to_string(),
        script,
        style,
        color,
        font_family,
        font_size_pt: ctx.base_font_size_pt,
    })
}
