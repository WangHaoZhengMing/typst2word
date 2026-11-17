//! `RealizationKind::Fragment` & `RealizationKind::Document` post-processing:
//! - Emits `__typst2word_scope` / `__typst2word_page_styles` markers when `StyleChain` changes.
//! - Wraps untagged `#show`-generated structural blocks (`BlockElem`, `PadElem`, `ColbreakElem`,
//!   `LinebreakElem`, `BoxElem`, non-weak `HElem`) in `INTERNAL_TAG_FLAGS` markers.
//! - Tracks explicit strong/weak `PagebreakElem` runs while keeping `migrate_unterminated_tags` intact.

use std::collections::HashSet;

use typst::engine::Engine;
use typst::foundations::{Content, Smart, StyleChain};
use typst::introspection::{Location, SplitLocator, Tag, TagElem};
use typst::layout::{BlockElem, BoxElem, ColbreakElem, HElem, PadElem, PagebreakElem, Sizing};
use typst::routines::{Arenas, Pair};
use typst::text::LinebreakElem;

use super::routines::{
    INTERNAL_TAG_FLAGS, is_cetz_span, push_scope_marker, wrap_item_with_internal_tag,
};
use crate::semantic::{PAGE_STYLES_LABEL, SCOPE_STYLES_LABEL};

pub(super) fn realize_fragment_pairs<'a>(
    pairs: Vec<Pair<'a>>,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
) -> Vec<Pair<'a>> {
    let mut out = Vec::with_capacity(pairs.len() + 4);
    let mut prev_chain: Option<StyleChain<'a>> = None;
    let mut prev_was_start_tag = false;

    for (idx, (item, chain)) in pairs.into_iter().enumerate() {
        if prev_chain != Some(chain) {
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

        let already_tagged = prev_was_start_tag;
        prev_was_start_tag = item
            .to_packed::<TagElem>()
            .is_some_and(|t| matches!(t.tag, Tag::Start(..)));

        if !already_tagged && should_wrap_layout_item(item, chain) {
            wrap_item_with_internal_tag(&mut out, item, chain, idx, engine, locator, arenas);
        } else {
            out.push((item, chain));
        }
    }

    out
}

pub(super) fn realize_document_pairs<'a>(
    pairs: Vec<Pair<'a>>,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
) -> Vec<Pair<'a>> {
    let mut out = Vec::with_capacity(pairs.len() + 8);
    let mut prev_chain: Option<StyleChain<'a>> = None;
    let mut prev_was_start_tag = false;
    let mut page_has_content = false;

    let n = pairs.len();
    let mut i = 0;
    while i < n {
        let (item, chain) = pairs[i];

        if item.is::<PagebreakElem>() {
            // Consume the entire contiguous run of `PagebreakElem`s together so
            // `typst_layout::pages::collect::migrate_unterminated_tags` sees an unbroken run.
            let run_start = i;
            while i < n && pairs[i].0.is::<PagebreakElem>() {
                i += 1;
            }
            let has_following_content = pairs[i..]
                .iter()
                .any(|(c, _)| !c.is::<TagElem>() && !c.is::<PagebreakElem>());

            // Simulate `pages::collect` on this contiguous run to count exact user pagebreaks.
            let mut explicit_breaks: Vec<(&'a Content, StyleChain<'a>, usize)> = Vec::new();
            for (offset, &(pb_item, pb_chain)) in pairs[run_start..i].iter().enumerate() {
                if let Some(pb) = pb_item.to_packed::<PagebreakElem>() {
                    let is_set_page_synthetic =
                        std::ptr::eq(pb_item, PagebreakElem::shared_weak())
                            || pb.boundary.get(pb_chain);
                    let strong = !pb.weak.get(pb_chain);

                    if is_set_page_synthetic {
                        if page_has_content && has_following_content && !pb.boundary.get(pb_chain) {
                            explicit_breaks.push((pb_item, pb_chain, run_start + offset));
                            page_has_content = false;
                        }
                    } else if strong {
                        explicit_breaks.push((pb_item, pb_chain, run_start + offset));
                        page_has_content = false;
                    } else if page_has_content && has_following_content {
                        explicit_breaks.push((pb_item, pb_chain, run_start + offset));
                        page_has_content = false;
                    }
                }
            }

            // Insert terminated `PagebreakElem` markers BEFORE any trailing unterminated
            // `Tag::Start`s in `out` so `migrate_unterminated_tags` keeps the pagebreak
            // marker at the end of the preceding page while migrating `Tag::Start(HeadingElem)`
            // onto the new page.
            if !explicit_breaks.is_empty() {
                let insert_pos = find_unterminated_tag_tail_start(&out);
                let mut marker_pairs = Vec::with_capacity(explicit_breaks.len() * 2);
                for (pb_item, pb_chain, salt) in explicit_breaks {
                    let mut marker = pb_item.clone();
                    marker.materialize(pb_chain);
                    let key = typst_utils::hash128(&("doc_pb", salt, pb_item.span()));
                    let loc = locator.next_location(engine, key, pb_item.span());
                    marker.set_location(loc);
                    marker_pairs.push((
                        &*arenas
                            .content
                            .alloc(TagElem::packed(Tag::Start(marker, INTERNAL_TAG_FLAGS))),
                        pb_chain,
                    ));
                    marker_pairs.push((
                        &*arenas.content.alloc(TagElem::packed(Tag::End(
                            loc,
                            key,
                            INTERNAL_TAG_FLAGS,
                        ))),
                        pb_chain,
                    ));
                }
                out.splice(insert_pos..insert_pos, marker_pairs);
            }

            for &pb_pair in &pairs[run_start..i] {
                out.push(pb_pair);
            }
            prev_chain = None;
            prev_was_start_tag = false;
            continue;
        }

        // Only emit page/scope markers right before non-`TagElem` items or when no
        // `PagebreakElem` immediately follows, keeping `migrate_unterminated_tags` intact.
        let followed_by_pagebreak = pairs[i + 1..]
            .iter()
            .take_while(|(c, _)| c.is::<TagElem>() || c.is::<PagebreakElem>())
            .any(|(c, _)| c.is::<PagebreakElem>());

        if prev_chain != Some(chain) && (!item.is::<TagElem>() || !followed_by_pagebreak) {
            push_scope_marker(
                &mut out,
                PAGE_STYLES_LABEL,
                chain,
                item.span(),
                i,
                engine,
                locator,
                arenas,
            );
            prev_chain = Some(chain);
        }

        if !item.is::<TagElem>() {
            page_has_content = true;
        }

        let already_tagged = prev_was_start_tag;
        prev_was_start_tag = item
            .to_packed::<TagElem>()
            .is_some_and(|t| matches!(t.tag, Tag::Start(..)));

        if !already_tagged && should_wrap_layout_item(item, chain) {
            wrap_item_with_internal_tag(&mut out, item, chain, i, engine, locator, arenas);
        } else {
            out.push((item, chain));
        }

        i += 1;
    }

    out
}

#[inline]
fn should_wrap_layout_item(item: &Content, chain: StyleChain<'_>) -> bool {
    !is_cetz_span(item.span())
        && ((item.is::<BlockElem>() && is_styled_or_structural_block(item, chain))
            || (item.is::<PadElem>() && item.location().is_none())
            || item.is::<ColbreakElem>()
            || item.is::<LinebreakElem>()
            || (item.is::<BoxElem>() && item.location().is_none())
            || item
                .to_packed::<HElem>()
                .is_some_and(|h| !h.weak.get(chain) && !h.amount.is_zero()))
}

/// Find the index in `out` where trailing unterminated `TagElem`s start.
fn find_unterminated_tag_tail_start(out: &[Pair<'_>]) -> usize {
    let trailing_tags = out
        .iter()
        .rev()
        .take_while(|(c, _)| c.is::<TagElem>())
        .count();
    let tail_start = out.len() - trailing_tags;
    if trailing_tags == 0 {
        return out.len();
    }
    let closed: HashSet<Location> = out[tail_start..]
        .iter()
        .filter_map(|(c, _)| match c.to_packed::<TagElem>()?.tag {
            Tag::End(loc, ..) => Some(loc),
            Tag::Start(..) => None,
        })
        .collect();
    let mut idx = tail_start;
    while idx < out.len() {
        if let Some(te) = out[idx].0.to_packed::<TagElem>()
            && closed.contains(&te.tag.location())
        {
            idx += 1;
        } else {
            break;
        }
    }
    idx
}

/// Only wrap `BlockElem`s that carry visual container styling (fill, stroke, inset, outset,
/// radius), explicit sizing, sticky/unbreakable constraints, or explicit `BlockBody::Content`.
fn is_styled_or_structural_block(item: &Content, chain: StyleChain<'_>) -> bool {
    let Some(blk) = item.to_packed::<BlockElem>() else {
        return false;
    };
    if item.location().is_some() {
        return false;
    }
    blk.fill.get_ref(chain).is_some()
        || blk.stroke.resolve(chain).iter().any(|s| s.is_some())
        || blk
            .inset
            .resolve(chain)
            .iter()
            .any(|s| s.is_some_and(|v| !v.is_zero()))
        || blk
            .radius
            .resolve(chain)
            .iter()
            .any(|c| c.is_some_and(|v| !v.is_zero()))
        || blk.sticky.get(chain)
        || !blk.breakable.get(chain)
        || !matches!(blk.width.get(chain), Smart::Auto)
        || !matches!(blk.height.get(chain), Sizing::Auto)
}
