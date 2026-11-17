//! `RealizationKind::Par` post-processing:
//! - Emits `__typst2word_scope` markers when `StyleChain` changes.
//! - Coalesces contiguous runs sharing `#set text(weight: "bold")` or `smallcaps` into single
//!   `StrongElem` / `SmallcapsElem` tag runs so HarfBuzz shaping and kerning stay intact.
//! - Wraps `#show`-generated `LinebreakElem`, `BoxElem`, and non-weak `HElem` in internal tags.

use typst::diag::SourceResult;
use typst::engine::Engine;
use typst::foundations::{Content, NativeElement, StyleChain};
use typst::introspection::{Location, SplitLocator, Tag, TagElem};
use typst::layout::{BoxElem, HElem};
use typst::model::StrongElem;
use typst::routines::{Arenas, Pair};
use typst::text::{FontWeight, LinebreakElem, SmallcapsElem, SmartQuoteElem, SpaceElem, TextElem};

use super::routines::{
    INTERNAL_TAG_FLAGS, is_cetz_span, push_scope_marker, wrap_item_with_internal_tag,
};
use crate::semantic::SCOPE_STYLES_LABEL;

pub(super) fn realize_par_pairs<'a>(
    pairs: Vec<Pair<'a>>,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
) -> SourceResult<Vec<Pair<'a>>> {
    let mut out = Vec::with_capacity(pairs.len() + 4);
    let mut prev_chain: Option<StyleChain<'a>> = None;
    let mut active_bold: Option<(Location, u128, StyleChain<'a>)> = None;
    let mut active_sc: Option<(Location, u128, StyleChain<'a>)> = None;

    let close_inline_runs = |out: &mut Vec<Pair<'a>>,
                             active_bold: &mut Option<(Location, u128, StyleChain<'a>)>,
                             active_sc: &mut Option<(Location, u128, StyleChain<'a>)>,
                             arenas: &'a Arenas| {
        if let Some((loc, key, ch)) = active_sc.take() {
            out.push((
                &*arenas.content.alloc(TagElem::packed(Tag::End(
                    loc,
                    key,
                    INTERNAL_TAG_FLAGS,
                ))),
                ch,
            ));
        }
        if let Some((loc, key, ch)) = active_bold.take() {
            out.push((
                &*arenas.content.alloc(TagElem::packed(Tag::End(
                    loc,
                    key,
                    INTERNAL_TAG_FLAGS,
                ))),
                ch,
            ));
        }
    };

    let mut prev_was_tag_for_item = false;
    for (idx, (item, chain)) in pairs.into_iter().enumerate() {
        let is_textlike =
            item.is::<TextElem>() || item.is::<SpaceElem>() || item.is::<SmartQuoteElem>();
        let want_bold = if item.is::<TextElem>() {
            chain.get(TextElem::weight) >= FontWeight::SEMIBOLD
        } else if is_textlike {
            active_bold.is_some() && chain.get(TextElem::weight) >= FontWeight::SEMIBOLD
        } else {
            false
        };
        let want_sc = if item.is::<TextElem>() {
            chain.get(TextElem::smallcaps).is_some()
        } else if is_textlike {
            active_sc.is_some() && chain.get(TextElem::smallcaps).is_some()
        } else {
            false
        };

        if (active_bold.is_some() != want_bold) || (active_sc.is_some() != want_sc) {
            close_inline_runs(&mut out, &mut active_bold, &mut active_sc, arenas);
        }

        if prev_chain != Some(chain) {
            close_inline_runs(&mut out, &mut active_bold, &mut active_sc, arenas);
            push_scope_marker(
                &mut out,
                SCOPE_STYLES_LABEL,
                chain,
                item.span(),
                idx,
                engine,
                locator,
                arenas,
            );
            prev_chain = Some(chain);
        }

        if want_bold && active_bold.is_none() {
            let mut marker = StrongElem::new(Content::empty()).pack();
            let key = typst_utils::hash128(&("par_bold", idx, item.span()));
            let loc = locator.next_location(engine, key, item.span());
            marker.set_location(loc);
            out.push((
                &*arenas
                    .content
                    .alloc(TagElem::packed(Tag::Start(marker, INTERNAL_TAG_FLAGS))),
                chain,
            ));
            active_bold = Some((loc, key, chain));
        }
        if want_sc && active_sc.is_none() {
            let mut marker = SmallcapsElem::new(Content::empty()).pack();
            let key = typst_utils::hash128(&("par_sc", idx, item.span()));
            let loc = locator.next_location(engine, key, item.span());
            marker.set_location(loc);
            out.push((
                &*arenas
                    .content
                    .alloc(TagElem::packed(Tag::Start(marker, INTERNAL_TAG_FLAGS))),
                chain,
            ));
            active_sc = Some((loc, key, chain));
        }

        // Wrap `#show`-generated `LinebreakElem`, `BoxElem`, or non-weak `HElem` when not
        // already preceded by their own `Tag::Start`.
        let already_tagged = prev_was_tag_for_item;
        prev_was_tag_for_item = item
            .to_packed::<TagElem>()
            .is_some_and(|t| matches!(t.tag, Tag::Start(..)));

        let wrap_inline = !already_tagged
            && !is_cetz_span(item.span())
            && (item.is::<LinebreakElem>()
                || (item.is::<BoxElem>() && item.location().is_none())
                || item
                    .to_packed::<HElem>()
                    .is_some_and(|h| !h.weak.get(chain) && !h.amount.is_zero()));

        if wrap_inline {
            wrap_item_with_internal_tag(&mut out, item, chain, idx, engine, locator, arenas);
        } else {
            out.push((item, chain));
        }
    }

    close_inline_runs(&mut out, &mut active_bold, &mut active_sc, arenas);
    Ok(out)
}
