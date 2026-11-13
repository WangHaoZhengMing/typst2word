//! Self-contained Typst 0.15 compilation & introspection/query engine.
//!
//! Provides:
//! 1. Custom `Routines` that preserve zero-interference (`introspectable: false, tagged: false`)
//!    `Tag::Start` / `Tag::End` markers for realized scoped `Styles`, explicit page/column/line
//!    breaks, non-weak horizontal spacing, containers (`BlockElem`, `BoxElem`), shapes,
//!    transforms, and HTML/PDF elements in `Target::Paged`.
//! 2. Post-convergence `RealizedSemanticStore` mapping every surviving `Location` to its
//!    exact scoped `Styles`, pre-resolved `RealizedStyleSnapshot`, and `semantic_parent`.
//! 3. Pre-layout `Content` tree evaluation + `Span -> Vec<Content>` index + on-demand
//!    `RealizationKind::Math` realization for equations containing `#show` or `context`.
//! 4. Full `PagedDocument` layout convergence with `typst::introspection::analyze` diagnostics.

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::LazyLock;

use comemo::Track;
use ecow::{EcoString, EcoVec};
use typst::diag::{SourceDiagnostic, SourceResult};
use typst::engine::{Engine, Route, Sink, Traced};
use typst::foundations::{
    Content, Label, NativeElement, NativeRuleMap, Packed, Selector, SequenceElem, StyleChain,
    StyledElem, Styles, Target, TargetElem,
};
use typst::introspection::{
    EmptyIntrospector, Introspector, Locatable, Location, MAX_ITERS, SplitLocator, Tag, TagElem,
    TagFlags, Tagged,
};
use typst::layout::{
    AlignElem, BlockBody, BlockElem, BoxElem, ColbreakElem, ColumnsElem, HElem, HideElem, MoveElem,
    PagebreakElem, RepeatElem, RotateElem, ScaleElem, SkewElem,
};
use typst::model::StrongElem;
use typst::routines::{Arenas, Pair, RealizationKind, Routines};
use typst::syntax::{Span, VirtualRoot};
use typst::text::{FontWeight, LinebreakElem, SmallcapsElem, SmartQuoteElem, SpaceElem, TextElem};
use typst::utils::Protected;
use typst::visualize::{
    CircleElem, CurveElem, EllipseElem, LineElem, PolygonElem, RectElem, SquareElem,
};
use typst::{World, WorldExt};
use typst_html::{FrameElem, HtmlElem};
use typst_layout::PagedDocument;

use crate::semantic::{PAGE_STYLES_LABEL, RealizedSemanticStore, SCOPE_STYLES_LABEL};
use crate::world::TypstWorld;

/// Zero-interference tag flags: preserved inside `FrameItem::Tag` in `PagedDocument`,
/// completely ignored by `Introspector` (`query(...)`) and `typst_pdf` tagged PDF trees.
const INTERNAL_TAG_FLAGS: TagFlags = TagFlags {
    introspectable: false,
    tagged: false,
};

/// Custom compiler routines that enrich `Target::Paged` realization with semantic tags
/// for scoped styles, explicit breaks, containers, shapes, transforms, and unsupported
/// elements (`html.elem`, `html.frame`).
///
/// Since Typst 0.15 the routines are attached to the `Library` (see
/// `GlobalTypstResources`) instead of being passed through the `Engine`.
pub static CUSTOM_ROUTINES: LazyLock<Routines> = LazyLock::new(|| Routines {
    rules: || {
        let mut rules = NativeRuleMap::new();
        typst_layout::register(&mut rules);
        typst_html::register(&mut rules);

        // Register `HtmlElem` on `Target::Paged` so it is not silently dropped with a
        // warning during paged layout, allowing our extractor to see its `Tag::Start` /
        // `Tag::End` and emit a visible unsupported-element placeholder.
        rules.register::<HtmlElem>(Target::Paged, |elem, _, styles| {
            let body = elem.body.get_cloned(styles).unwrap_or_default();
            Ok(BlockElem::new()
                .with_body(Some(BlockBody::Content(body)))
                .pack()
                .spanned(elem.span()))
        });

        rules
    },
    eval_string: typst_eval::eval_string,
    eval_closure: typst_eval::eval_closure,
    realize: custom_realize,
    layout_frame: typst_layout::layout_frame,
    html_module: typst_html::module,
    html_mathml_body: typst_html::html_mathml_body,
    html_span_filled: typst_html::html_span_filled,
});

/// Check whether `span` originates inside the `@preview/cetz` package.
pub fn is_cetz_span(span: Span) -> bool {
    span.id().is_some_and(
        |id| matches!(id.get().root(), VirtualRoot::Package(pkg) if pkg.name.as_str() == "cetz"),
    )
}

/// Check whether an element transformed by a built-in `Target::Paged` native show rule
/// should receive a synthetic `Location` before `typst_realize::realize` so `prepare`
/// materializes its styles and preserves its original element identity across realization.
fn should_locate_pre_realization(elem: &Content) -> bool {
    if elem.is_prepared() || elem.location().is_some() {
        return false;
    }
    if is_cetz_span(elem.span()) {
        return false;
    }
    elem.is::<RectElem>()
        || elem.is::<SquareElem>()
        || elem.is::<CircleElem>()
        || elem.is::<EllipseElem>()
        || elem.is::<PolygonElem>()
        || elem.is::<CurveElem>()
        || elem.is::<LineElem>()
        || elem.is::<HtmlElem>()
        || elem.is::<FrameElem>()
        || elem.is::<RotateElem>()
        || elem.is::<ScaleElem>()
        || elem.is::<SkewElem>()
        || elem.is::<MoveElem>()
        || elem.is::<RepeatElem>()
        || elem.is::<HideElem>()
        || elem.is::<LinebreakElem>()
        || elem.is::<ColbreakElem>()
        || elem.is::<ColumnsElem>()
}

/// Recursively annotate pre-realization containers so shape, transform, and HTML
/// elements receive locations even when nested inside `#block[...]`, `#box[...]`,
/// `#align[...]`, or `#set`/`#show` wrappers.
fn annotate_content_for_realization(
    content: &Content,
    engine: &mut Engine,
    locator: &mut SplitLocator,
) -> Option<Content> {
    if should_locate_pre_realization(content) {
        let mut cloned = content.clone();
        let key = typst_utils::hash128(&cloned);
        let loc = locator.next_location(engine, key, cloned.span());
        cloned.set_location(loc);
        return Some(cloned);
    }

    if let Some(seq) = content.to_packed::<SequenceElem>() {
        let mut changed = false;
        let mut new_children = Vec::with_capacity(seq.children.len());
        for child in &seq.children {
            if let Some(updated) = annotate_content_for_realization(child, engine, locator) {
                changed = true;
                new_children.push(updated);
            } else {
                new_children.push(child.clone());
            }
        }
        if changed {
            let mut packed = SequenceElem::new(new_children)
                .pack()
                .spanned(content.span());
            if let Some(lbl) = content.label() {
                packed.set_label(lbl);
            }
            if let Some(loc) = content.location() {
                packed.set_location(loc);
            }
            return Some(packed);
        }
        return None;
    }

    if let Some(styled) = content.to_packed::<StyledElem>() {
        if let Some(updated_child) =
            annotate_content_for_realization(&styled.child, engine, locator)
        {
            let mut packed = StyledElem::new(updated_child, styled.styles.clone())
                .pack()
                .spanned(content.span());
            if let Some(lbl) = content.label() {
                packed.set_label(lbl);
            }
            if let Some(loc) = content.location() {
                packed.set_location(loc);
            }
            return Some(packed);
        }
        return None;
    }

    if let Some(align) = content.to_packed::<AlignElem>() {
        if let Some(updated_child) = annotate_content_for_realization(&align.body, engine, locator)
        {
            let mut cloned = align.clone();
            cloned.body = updated_child;
            return Some(cloned.pack());
        }
        return None;
    }

    if let Some(blk) = content.to_packed::<BlockElem>()
        && !is_cetz_span(content.span())
        && let Some(BlockBody::Content(body)) = blk.body.get_ref(StyleChain::default())
        && let Some(updated_body) = annotate_content_for_realization(body, engine, locator)
    {
        let mut cloned = blk.clone();
        cloned.body.set(Some(BlockBody::Content(updated_body)));
        return Some(cloned.pack());
    }

    if let Some(bx) = content.to_packed::<BoxElem>()
        && !is_cetz_span(content.span())
        && let Some(body) = bx.body.get_ref(StyleChain::default())
        && let Some(updated_body) = annotate_content_for_realization(body, engine, locator)
    {
        let mut cloned = bx.clone();
        cloned.body.set(Some(updated_body));
        return Some(cloned.pack());
    }

    None
}

use typst::foundations::Smart;

/// Emit a zero-size scope-style marker `(Tag::Start, Tag::End)` into `out`.
fn push_scope_marker<'a>(
    out: &mut Vec<Pair<'a>>,
    label_str: &'static str,
    chain: StyleChain<'a>,
    span: Span,
    salt: usize,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
) {
    let mut marker = StyledElem::new(Content::empty(), chain.to_map()).pack();
    marker.set_label(
        Label::new(typst_utils::PicoStr::intern(label_str)).expect("nonempty internal label"),
    );
    let key = typst_utils::hash128(&(label_str, salt, span));
    let loc = locator.next_location(engine, key, span);
    marker.set_location(loc);
    out.push((
        &*arenas
            .content
            .alloc(TagElem::packed(Tag::Start(marker, INTERNAL_TAG_FLAGS))),
        chain,
    ));
    out.push((
        &*arenas.content.alloc(TagElem::packed(Tag::End(
            loc,
            key,
            INTERNAL_TAG_FLAGS,
        ))),
        chain,
    ));
}

/// Downgrade pre-realization synthetic locations (for unlabelled non-locatable elements
/// such as shapes/transforms) to `INTERNAL_TAG_FLAGS` so `Introspector` stays 100% clean.
fn sanitize_synthetic_tag_flags<'a>(pairs: Vec<Pair<'a>>, arenas: &'a Arenas) -> Vec<Pair<'a>> {
    let mut synthetic_locs: HashSet<Location> = HashSet::new();
    for &(item, _) in &pairs {
        if let Some(tag_elem) = item.to_packed::<TagElem>()
            && let Tag::Start(inner, flags) = &tag_elem.tag
            && flags.any()
            && !inner.can::<dyn Locatable>()
            && !inner.can::<dyn Tagged>()
            && inner.label().is_none()
            && let Some(loc) = inner.location()
        {
            synthetic_locs.insert(loc);
        }
    }
    if synthetic_locs.is_empty() {
        return pairs;
    }

    pairs
        .into_iter()
        .map(|(item, chain)| {
            if let Some(tag_elem) = item.to_packed::<TagElem>() {
                match &tag_elem.tag {
                    Tag::Start(inner, _)
                        if inner
                            .location()
                            .is_some_and(|loc| synthetic_locs.contains(&loc)) =>
                    {
                        let updated =
                            TagElem::packed(Tag::Start(inner.clone(), INTERNAL_TAG_FLAGS));
                        return (&*arenas.content.alloc(updated), chain);
                    }
                    Tag::End(loc, key, _) if synthetic_locs.contains(loc) => {
                        let updated = TagElem::packed(Tag::End(*loc, *key, INTERNAL_TAG_FLAGS));
                        return (&*arenas.content.alloc(updated), chain);
                    }
                    _ => {}
                }
            }
            (item, chain)
        })
        .collect()
}

/// Wrap post-realize container / break / spacing items (`BlockElem`, `BoxElem`,
/// `ColbreakElem`, `LinebreakElem`, non-weak `HElem`) that were generated inside
/// `#show` rules or closures (and thus missed pre-realize annotation) with
/// `INTERNAL_TAG_FLAGS` `Tag::Start` / `Tag::End` markers.
fn wrap_item_with_internal_tag<'a>(
    out: &mut Vec<Pair<'a>>,
    item: &'a Content,
    chain: StyleChain<'a>,
    salt: usize,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
) {
    let mut marker = item.clone();
    marker.materialize(chain);
    let key = typst_utils::hash128(&(salt, &marker));
    let loc = locator.next_location(engine, key, item.span());
    marker.set_location(loc);
    out.push((
        &*arenas
            .content
            .alloc(TagElem::packed(Tag::Start(marker, INTERNAL_TAG_FLAGS))),
        chain,
    ));
    out.push((item, chain));
    out.push((
        &*arenas.content.alloc(TagElem::packed(Tag::End(
            loc,
            key,
            INTERNAL_TAG_FLAGS,
        ))),
        chain,
    ));
}

fn custom_realize<'a>(
    kind: RealizationKind,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
    content: &'a Content,
    styles: StyleChain<'a>,
) -> SourceResult<Vec<Pair<'a>>> {
    let is_document = matches!(kind, RealizationKind::Document { .. });
    let is_fragment = matches!(kind, RealizationKind::Fragment { .. });
    let is_par = matches!(kind, RealizationKind::Par);

    let target_content = if is_document || is_fragment || is_par {
        if let Some(annotated) = annotate_content_for_realization(content, engine, locator) {
            arenas.content.alloc(annotated)
        } else {
            content
        }
    } else {
        content
    };

    let raw_pairs = typst_realize::realize(kind, engine, locator, arenas, target_content, styles)?;
    let pairs = sanitize_synthetic_tag_flags(raw_pairs, arenas);

    if is_par {
        return realize_par_pairs(pairs, engine, locator, arenas);
    }
    if is_fragment {
        return Ok(realize_fragment_pairs(pairs, engine, locator, arenas));
    }
    if is_document {
        return Ok(realize_document_pairs(pairs, engine, locator, arenas));
    }

    Ok(pairs)
}

/// Post-process `RealizationKind::Par` pairs:
/// - Emit `__typst2word_scope` markers when `StyleChain` changes.
/// - Group contiguous runs of text/spaces sharing `#set text(weight: "bold")` or `smallcaps`
///   into a single `StrongElem` / `SmallcapsElem` marker run so HarfBuzz shaping, ligatures,
///   and kerning across spaces remain intact.
/// - Wrap `#show`-generated `LinebreakElem`, `BoxElem`, and non-weak `HElem` in internal tags.
fn realize_par_pairs<'a>(
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

/// Post-process `RealizationKind::Fragment` pairs (containers, cells, list bodies, headers/footers):
/// - Emit `__typst2word_scope` markers when `StyleChain` changes.
/// - Wrap untagged `BlockElem` and `ColbreakElem` in `INTERNAL_TAG_FLAGS` markers.
fn realize_fragment_pairs<'a>(
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

        let should_wrap = !already_tagged
            && !is_cetz_span(item.span())
            && ((item.is::<BlockElem>() && is_styled_or_structural_block(item, chain))
                || item.is::<ColbreakElem>());

        if should_wrap {
            wrap_item_with_internal_tag(&mut out, item, chain, idx, engine, locator, arenas);
        } else {
            out.push((item, chain));
        }
    }

    out
}

/// Post-process `RealizationKind::Document` pairs:
/// - Keep `__typst2word_page_styles` markers for page-root style extraction AND
///   `RealizedSemanticStore`.
/// - Wrap untagged `BlockElem` and `ColbreakElem` in `INTERNAL_TAG_FLAGS` markers.
/// - Emit terminated `PagebreakElem` internal markers ONLY for genuine user/show-rule
///   pagebreaks (`!ptr::eq(shared_weak) && !boundary`) that actually break a page, without
///   splitting contiguous `PagebreakElem` runs (`pages::collect::migrate_unterminated_tags`).
fn realize_document_pairs<'a>(
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
                        if page_has_content && !pb.boundary.get(pb_chain) {
                            page_has_content = false;
                        }
                    } else if strong {
                        explicit_breaks.push((pb_item, pb_chain, run_start + offset));
                        page_has_content = false;
                    } else if page_has_content && has_following_content {
                        // Weak explicit pagebreak (`#pagebreak(weak: true)` or
                        // `#show heading: it => pagebreak(weak: true) + it`) after real content.
                        explicit_breaks.push((pb_item, pb_chain, run_start + offset));
                        page_has_content = false;
                    }
                }
            }

            // Insert terminated `PagebreakElem` markers BEFORE any trailing unterminated
            // `Tag::Start`s in `out` so `migrate_unterminated_tags` keeps the pagebreak
            // marker at the end of the preceding page while migrating `Tag::Start(HeadingElem)`
            // onto the new page!
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

        let should_wrap = !already_tagged
            && !is_cetz_span(item.span())
            && ((item.is::<BlockElem>() && is_styled_or_structural_block(item, chain))
                || item.is::<ColbreakElem>());

        if should_wrap {
            wrap_item_with_internal_tag(&mut out, item, chain, i, engine, locator, arenas);
        } else {
            out.push((item, chain));
        }

        i += 1;
    }

    out
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
    // Advance past terminated tags at the start of the tail so the pagebreak marker sits
    // right after terminated tags on the current page and right before unterminated `Tag::Start`s.
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
/// radius), explicit sizing, sticky/unbreakable constraints, or explicit `BlockBody::Content`
/// (avoiding wrapping bare internal single-layouter blocks unnecessarily).
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
}

/// Complete compilation result from the Typst backend, holding:
/// - Laid-out `PagedDocument` (pages, frames, metadata, `Introspector`).
/// - Post-convergence `RealizedSemanticStore` (`Location` -> scoped `Styles` & `semantic_parent`).
/// - Evaluated pre-layout `Content` IR tree.
/// - `Span -> Vec<Content>` lookup table.
/// - Compiler warnings and diagnostics.
pub struct CompiledTypstDocument {
    pub document: PagedDocument,
    pub semantics: RealizedSemanticStore,
    pub evaluated_content: Content,
    pub span_index: HashMap<Span, Vec<Content>>,
    pub warnings: EcoVec<SourceDiagnostic>,
}

impl CompiledTypstDocument {
    /// Compile a `TypstWorld` into a `CompiledTypstDocument`.
    pub fn compile(world: &TypstWorld) -> Result<Self, String> {
        let mut sink = Sink::new();
        let tracked_world = (world as &dyn World).track();
        let traced = Traced::default();

        let library = world.library();
        let base = StyleChain::new(&library.styles);
        let target = TargetElem::target.set(Target::Paged).wrap();
        let styles = base.chain(&target);

        let main_id = world.main();
        let main_source = world
            .source(main_id)
            .map_err(|e| format!("failed to load main Typst source: {e:?}"))?;

        let module = typst_eval::eval(
            tracked_world,
            library,
            traced.track(),
            sink.track_mut(),
            Route::default().track(),
            &main_source,
        )
        .map_err(|diags| format_diagnostics(world, &deduplicate(diags)))?;

        let evaluated_content = module.content();
        let span_index = build_span_index(&evaluated_content);

        let empty_introspector = EmptyIntrospector;
        let mut history: Vec<PagedDocument> = Vec::with_capacity(MAX_ITERS - 1);
        let mut document: PagedDocument;

        loop {
            let mut subsink = Sink::new();
            let introspector: &dyn Introspector = history
                .last()
                .map(|doc| doc.introspector().as_ref() as &dyn Introspector)
                .unwrap_or(&empty_introspector);
            let constraint = comemo::Constraint::new();
            let mut engine = Engine {
                library,
                world: tracked_world,
                introspector: Protected::new(introspector.track_with(&constraint)),
                traced: traced.track(),
                sink: subsink.track_mut(),
                route: Route::default(),
            };

            document = typst_layout::layout_document(&mut engine, &evaluated_content, styles)
                .map_err(|diags| format_diagnostics(world, &deduplicate(diags)))?;

            if constraint.validate(document.introspector().as_ref() as &dyn Introspector) {
                sink.extend_from_sink(subsink);
                break;
            }

            if history.len() >= MAX_ITERS - 1 {
                let mut introspectors = [&empty_introspector as &dyn Introspector; MAX_ITERS + 1];
                for (idx, prev_doc) in history.iter().enumerate() {
                    introspectors[idx + 1] = prev_doc.introspector().as_ref();
                }
                introspectors[MAX_ITERS] = document.introspector().as_ref();

                let warnings = typst::introspection::analyze(
                    tracked_world,
                    introspectors,
                    subsink.introspections(),
                );
                sink.extend_from_sink(subsink);
                for w in warnings {
                    sink.warn(w);
                }
                break;
            }

            history.push(document);
        }

        let delayed = sink.delayed();
        if !delayed.is_empty() {
            return Err(format_diagnostics(world, &deduplicate(delayed)));
        }

        let semantics = RealizedSemanticStore::from_paged_document(&document, &library.styles);
        let warnings = deduplicate(sink.warnings());

        Ok(Self {
            document,
            semantics,
            evaluated_content,
            span_index,
            warnings,
        })
    }

    /// Realize a math `Content` expression (`EquationElem::body`) through `typst_realize::realize`
    /// with `RealizationKind::Math` using the converged `Introspector` and local `Styles`!
    ///
    /// This resolves `#show` rules (including regex rules like `#show "RR": math.bb("R")`),
    /// `context` blocks, and nested `$...$` equations inside math bodies before OMML conversion.
    pub fn realize_math<'a>(
        &'a self,
        world: &'a TypstWorld,
        content: &'a Content,
        local_styles: Option<&'a Styles>,
        arenas: &'a Arenas,
    ) -> SourceResult<Vec<Pair<'a>>> {
        let tracked_world = (world as &dyn World).track();
        let traced = Traced::default();
        let mut sink = Sink::new();
        let library = world.library();
        let introspector: &dyn Introspector = self.document.introspector().as_ref();

        let mut engine = Engine {
            library,
            world: tracked_world,
            introspector: Protected::new(introspector.track()),
            traced: traced.track(),
            sink: sink.track_mut(),
            route: Route::default(),
        };

        let styles = StyleChain::new(local_styles.unwrap_or(&library.styles));
        let root_loc = typst::introspection::Locator::root();
        let mut locator = root_loc.split();
        typst_realize::realize(
            RealizationKind::Math,
            &mut engine,
            &mut locator,
            arenas,
            content,
            styles,
        )
    }

    /// Owned convenience wrapper around [`Self::realize_math`] that allocates an internal
    /// [`Arenas`] and returns owned `(Content, Styles)` pairs.
    pub fn realize_math_owned(
        &self,
        world: &TypstWorld,
        content: &Content,
        local_styles: Option<&Styles>,
    ) -> Option<Vec<(Content, Styles)>> {
        let arenas = Arenas::default();
        let pairs = self
            .realize_math(world, content, local_styles, &arenas)
            .ok()?;
        Some(
            pairs
                .into_iter()
                .map(|(item, chain)| (item.clone(), chain.to_map()))
                .collect(),
        )
    }

    /// Access the `Introspector` of the compiled document.
    pub fn introspector(&self) -> &dyn Introspector {
        self.document.introspector().as_ref()
    }

    /// Run a Typst `Selector` query against the document's `Introspector`.
    pub fn query(&self, selector: &Selector) -> EcoVec<Content> {
        self.introspector().query(selector)
    }

    /// Query all materialized elements of type `T` from the document's `Introspector`.
    pub fn query_elem<T: NativeElement>(&self) -> Vec<Packed<T>> {
        let selector = Selector::Elem(T::ELEM, None);
        self.introspector()
            .query(&selector)
            .into_iter()
            .filter_map(|c| c.into_packed::<T>().ok())
            .collect()
    }

    /// Query an element by label string (e.g. `"my-fig"`).
    pub fn query_label(&self, label_name: &str) -> Option<&Content> {
        let label = Label::construct(EcoString::from(label_name).into()).ok()?;
        self.introspector().query_label(label).ok()
    }

    /// Iterate over all introspectable elements in document order.
    pub fn all_introspected(&self) -> impl Iterator<Item = &Content> {
        self.document.introspector().elements().all()
    }

    /// Look up pre-layout `Content` elements associated with a `Span`.
    pub fn lookup_span(&self, span: Span) -> &[Content] {
        self.span_index.get(&span).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Extract the source snippet for a `Span` if available.
    pub fn source_snippet(&self, world: &TypstWorld, span: Span) -> Option<String> {
        let id = span.id()?;
        let source = world.source(id).ok()?;
        let range = world.range(span)?;
        source.text().get(range).map(|s| s.to_owned())
    }
}

fn deduplicate(mut diags: EcoVec<SourceDiagnostic>) -> EcoVec<SourceDiagnostic> {
    let mut seen = HashSet::new();
    diags.retain(|d| seen.insert(typst_utils::hash128(&(&d.span, &d.message))));
    diags
}

fn build_span_index(root: &Content) -> HashMap<Span, Vec<Content>> {
    let mut index: HashMap<Span, Vec<Content>> = HashMap::new();
    let _ = root.traverse(&mut |elem: Content| -> ControlFlow<()> {
        let span = elem.span();
        if !span.is_detached() && !elem.is::<SequenceElem>() && !elem.is::<StyledElem>() {
            index.entry(span).or_default().push(elem);
        }
        ControlFlow::Continue(())
    });
    index
}

fn format_diagnostics(world: &TypstWorld, diags: &[SourceDiagnostic]) -> String {
    let mut out = String::new();
    for (i, diag) in diags.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let loc_info = diag
            .span
            .id()
            .and_then(|id| {
                let source = world.source(id).ok()?;
                let range = world.range(diag.span)?;
                let line = source.lines().byte_to_line(range.start)?;
                let col = source.lines().byte_to_column(range.start)?;
                let path = id.vpath().get_without_slash().to_string();
                Some(format!("{path}:{}:{}", line + 1, col + 1))
            })
            .unwrap_or_else(|| "<detached>".to_string());
        out.push_str(&format!(
            "{:?} at {}: {}",
            diag.severity, loc_info, diag.message
        ));
        for hint in &diag.hints {
            out.push_str(&format!("\n  hint: {}", hint.v));
        }
    }
    out
}
