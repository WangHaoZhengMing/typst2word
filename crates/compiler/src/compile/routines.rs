//! Custom Typst `Routines` VTable, pre-realization synthetic location annotation, and
//! zero-interference `INTERNAL_TAG_FLAGS` tag marker helpers.

use std::collections::HashSet;
use std::sync::LazyLock;

use typst::diag::SourceResult;
use typst::engine::Engine;
use typst::foundations::{
    Content, Label, NativeElement, NativeRuleMap, SequenceElem, StyleChain, StyledElem, Target,
};
use typst::introspection::{Locatable, Location, SplitLocator, Tag, TagElem, TagFlags, Tagged};
use typst::layout::{
    AlignElem, BlockBody, BlockElem, BoxElem, ColbreakElem, ColumnsElem, HideElem, MoveElem,
    PadElem, RepeatElem, RotateElem, ScaleElem, SkewElem, StackChild, StackElem,
};
use typst::routines::{Arenas, FragmentKind, Pair, RealizationKind, Routines};
use typst::syntax::{Span, VirtualRoot};
use typst::text::LinebreakElem;
use typst::visualize::{
    CircleElem, CurveElem, EllipseElem, LineElem, PolygonElem, RectElem, SquareElem,
};
use typst_html::{FrameElem, HtmlElem};

use super::layout::{realize_document_pairs, realize_fragment_pairs};
use super::par::realize_par_pairs;

/// Zero-interference tag flags: preserved inside `FrameItem::Tag` in `PagedDocument`,
/// completely ignored by `Introspector` (`query(...)`) and `typst_pdf` tagged PDF trees.
pub(super) const INTERNAL_TAG_FLAGS: TagFlags = TagFlags {
    introspectable: false,
    tagged: false,
};

/// Custom compiler routines that enrich `Target::Paged` realization with semantic tags
/// for scoped styles, explicit breaks, containers, shapes, transforms, and unsupported
/// elements (`html.elem`, `html.frame`).
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

    if let Some(pad) = content.to_packed::<PadElem>()
        && !is_cetz_span(content.span())
    {
        let mut cloned = pad.clone();
        if let Some(updated_body) = annotate_content_for_realization(&pad.body, engine, locator) {
            cloned.body = updated_body;
        }
        let mut packed = cloned.pack();
        if let Some(lbl) = content.label() {
            packed.set_label(lbl);
        }
        if let Some(loc) = content.location() {
            packed.set_location(loc);
        } else if !content.is_prepared() {
            // `PadElem` has a built-in `Target::Paged` native show rule, so assign a synthetic
            // location pre-realization to preserve its `Tag::Start(PadElem)` / `Tag::End(PadElem)`.
            let key = typst_utils::hash128(&packed);
            let loc = locator.next_location(engine, key, packed.span());
            packed.set_location(loc);
        }
        return Some(packed);
    }

    if let Some(st) = content.to_packed::<StackElem>()
        && !is_cetz_span(content.span())
    {
        let mut cloned = st.clone();
        let mut children_changed = false;
        let mut new_children = Vec::with_capacity(st.children.len());
        for child in &st.children {
            match child {
                StackChild::Block(blk_content) => {
                    if let Some(updated) =
                        annotate_content_for_realization(blk_content, engine, locator)
                    {
                        children_changed = true;
                        new_children.push(StackChild::Block(updated));
                    } else {
                        new_children.push(StackChild::Block(blk_content.clone()));
                    }
                }
                StackChild::Spacing(sp) => new_children.push(StackChild::Spacing(*sp)),
            }
        }
        if children_changed {
            cloned.children = new_children;
        }
        let mut packed = cloned.pack();
        if let Some(lbl) = content.label() {
            packed.set_label(lbl);
        }
        if let Some(loc) = content.location() {
            packed.set_location(loc);
        } else if !content.is_prepared() {
            let key = typst_utils::hash128(&packed);
            let loc = locator.next_location(engine, key, packed.span());
            packed.set_location(loc);
        }
        return Some(packed);
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

/// Emit a zero-size scope-style marker `(Tag::Start, Tag::End)` into `out`.
pub(super) fn push_scope_marker<'a>(
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
pub(super) fn wrap_item_with_internal_tag<'a>(
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

    let (raw_pairs, is_inline_fragment) = match kind {
        RealizationKind::Fragment { kind: frag_kind } => {
            let pairs = typst_realize::realize(
                RealizationKind::Fragment { kind: frag_kind },
                engine,
                locator,
                arenas,
                target_content,
                styles,
            )?;
            (pairs, matches!(*frag_kind, FragmentKind::Inline))
        }
        other => (
            typst_realize::realize(other, engine, locator, arenas, target_content, styles)?,
            false,
        ),
    };
    let pairs = sanitize_synthetic_tag_flags(raw_pairs, arenas);

    if is_par || is_inline_fragment {
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
