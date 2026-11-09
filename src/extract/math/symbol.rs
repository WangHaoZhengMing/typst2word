//! Math symbols, text operators (`op`), primes, text runs, and inline formatting wrappers.

use typst::foundations::{Content, StyleChain, SymbolElem};
use typst::layout::{AlignElem, BoxElem, HElem, HideElem, PadElem, Spacing};
use typst::math::{EquationElem, OpElem, PrimesElem};
use typst::model::{EmphElem, LinkMarker, RefElem, StrongElem};
use typst::text::{
    HighlightElem, RawElem, SmartQuoteElem, SpaceElem, StrikeElem, SubElem, SuperElem, TextElem,
};
use typst::visualize::Paint;

use crate::extract::math::MathStyleCtx;
use crate::extract::math::node::extract_math_node;
use crate::ir::{MathNodeIr, MathRunIr, MathScriptIr, MathStyleIr};

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
            out.push(MathNodeIr::Run(run));
        }
        return;
    }

    // 2. Text Operator (`OpElem`, e.g. `sin`, `cos`, `lim`, `max`, `det`, etc.)
    if let Some(op) = content.to_packed::<OpElem>() {
        let op_str = op.text.plain_text();
        if let Some(run) = build_math_run(&op_str, styles, ctx, true) {
            out.push(MathNodeIr::Run(run));
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
            && let Some(run) = build_math_run("\u{2009}", styles, ctx, true)
        {
            out.push(MathNodeIr::Run(run));
        }
        return;
    }

    // 4. Symbols (`SymbolElem`) and Text (`TextElem`, `RawElem`, `SmartQuoteElem`)
    if let Some(sym) = content.to_packed::<SymbolElem>() {
        if let Some(run) = build_math_run(&sym.text, styles, ctx, false) {
            out.push(MathNodeIr::Run(run));
        }
        return;
    }
    if let Some(txt) = content.to_packed::<TextElem>() {
        let is_quoted_word = txt.text.chars().count() > 1;
        if let Some(run) = build_math_run(&txt.text, styles, ctx, is_quoted_word) {
            out.push(MathNodeIr::Run(run));
        }
        return;
    }
    if content.is::<RawElem>() {
        let t = content.plain_text();
        if let Some(run) = build_math_run(&t, styles, ctx, true) {
            out.push(MathNodeIr::Run(run));
        }
        return;
    }
    if content.is::<SmartQuoteElem>() {
        if let Some(run) = build_math_run("\"", styles, ctx, true) {
            out.push(MathNodeIr::Run(run));
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
            out.push(MathNodeIr::Run(run));
        }
        return;
    }

    // Fallback for any other content inside math: extract plain text.
    let fallback_txt = content.plain_text();
    if let Some(run) = build_math_run(&fallback_txt, styles, ctx, false) {
        out.push(MathNodeIr::Run(run));
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

    let style = match (bold, italic) {
        (true, Some(true)) => Some(MathStyleIr::BoldItalic),
        (true, Some(false)) => Some(MathStyleIr::Bold),
        (true, None) => {
            if default_upright {
                Some(MathStyleIr::Bold)
            } else {
                Some(MathStyleIr::BoldItalic)
            }
        }
        (false, Some(false)) => Some(MathStyleIr::Plain),
        (false, Some(true)) => Some(MathStyleIr::Italic),
        (false, None) => {
            if default_upright {
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

    let text_script = crate::passes::lang_script::classify_text_script(&text);
    let has_cjk = text_script.has_cjk();
    let font_family = if has_cjk {
        ctx.cjk_font.map(str::to_string).or_else(|| {
            styles
                .get_ref(TextElem::font)
                .into_iter()
                .find_map(|f| crate::extract::ExtractorContext::classify_cjk_font_static(f.as_str()))
                .map(str::to_string)
        })
    } else {
        ctx.math_font.map(str::to_string)
    };

    Some(MathRunIr {
        text: text.to_string(),
        script,
        style,
        color,
        font_family,
        font_size_pt: ctx.base_font_size_pt,
        width_scale_pct: None,
        text_script,
    })
}
