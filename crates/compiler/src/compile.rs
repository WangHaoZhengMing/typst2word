//! Self-contained Typst 0.15 compilation & introspection/query engine.
//!
//! Provides:
//! 1. Custom `Routines` that preserve `Tag::Start` / `Tag::End` with materialized
//!    styles for shapes (`rect`, `square`, `circle`, `ellipse`, `polygon`, `curve`,
//!    `path`, `line`), transforms (`rotate`, `scale`, `skew`, `move`), and HTML/PDF
//!    elements (`html.elem`, `html.frame`, `pdf.attach`) in `Target::Paged`.
//! 2. Pre-layout `Content` tree evaluation + `Span -> Vec<Content>` index.
//! 3. Full `PagedDocument` layout convergence + `Introspector` query helpers.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::LazyLock;

use comemo::Track;
use ecow::{EcoString, EcoVec};
use typst::diag::{SourceDiagnostic, SourceResult, warning};
use typst::engine::{Engine, Route, Sink, Traced};
use typst::foundations::{
    Content, Label, NativeElement, NativeRuleMap, Packed, Selector, SequenceElem, StyleChain,
    StyledElem, Target, TargetElem,
};
use typst::introspection::{EmptyIntrospector, Introspector, MAX_ITERS, SplitLocator};
use typst::layout::{
    BlockBody, BlockElem, ColbreakElem, ColumnsElem, HideElem, MoveElem, RepeatElem, RotateElem,
    ScaleElem, SkewElem,
};
use typst::routines::{Arenas, Pair, RealizationKind, Routines};
use typst::syntax::{Span, VirtualRoot};
use typst::utils::Protected;
use typst::visualize::{
    CircleElem, CurveElem, EllipseElem, LineElem, PolygonElem, RectElem, SquareElem,
};
use typst::{World, WorldExt};
use typst_html::{FrameElem, HtmlElem};
use typst_layout::PagedDocument;

use crate::world::TypstWorld;

/// Custom compiler routines that enrich `Target::Paged` realization with semantic tags
/// for shapes, transforms, and unsupported elements (`html.elem`, `html.frame`).
///
/// Since Typst 0.15 the routines are attached to the `Library` (see
/// `GlobalTypstResources`) instead of being passed through the `Engine`.
pub static CUSTOM_ROUTINES: LazyLock<Routines> = LazyLock::new(|| Routines {
    rules: || {
        let mut rules = NativeRuleMap::new();
        typst_layout::register(&mut rules);
        typst_html::register(&mut rules);

        // Register `HtmlElem` and `FrameElem` on `Target::Paged` so they are not
        // silently dropped with a warning during paged layout, allowing our
        // extractor to see their `Tag::Start` / `Tag::End` and emit a visible
        // unsupported-element placeholder.
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

/// Check whether an element should receive a synthetic `Location` during realization
/// so that `typst_realize::prepare` materializes its styles and wraps its laid-out
/// frame items in `Tag::Start(elem)` / `Tag::End(loc)`.
fn should_locate_for_extraction(elem: &Content) -> bool {
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
        || elem.is::<typst::text::LinebreakElem>()
        || elem.is::<ColbreakElem>()
        || elem.is::<ColumnsElem>()
}

/// Recursively annotate top-level `SequenceElem` and `StyledElem` nodes so that
/// shape, transform, and HTML elements get locations before `typst_realize::realize`.
fn annotate_content_for_realization(
    content: &Content,
    engine: &mut Engine,
    locator: &mut SplitLocator,
) -> Option<Content> {
    if should_locate_for_extraction(content) {
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

    if let Some(align) = content.to_packed::<typst_library::layout::AlignElem>() {
        if let Some(updated_child) = annotate_content_for_realization(&align.body, engine, locator)
        {
            let mut cloned = align.clone();
            cloned.body = updated_child;
            return Some(cloned.pack());
        }
        return None;
    }

    None
}

fn custom_realize<'a>(
    kind: RealizationKind,
    engine: &mut Engine,
    locator: &mut SplitLocator,
    arenas: &'a Arenas,
    content: &'a Content,
    styles: StyleChain<'a>,
) -> SourceResult<Vec<Pair<'a>>> {
    let target_content = if matches!(
        kind,
        RealizationKind::Document { .. } | RealizationKind::Fragment { .. } | RealizationKind::Par
    ) {
        if let Some(annotated) = annotate_content_for_realization(content, engine, locator) {
            arenas.content.alloc(annotated)
        } else {
            content
        }
    } else {
        content
    };

    typst_realize::realize(kind, engine, locator, arenas, target_content, styles)
}

/// Complete compilation result from the Typst backend, holding:
/// - Laid-out `PagedDocument` (pages, frames, metadata, `Introspector`).
/// - Evaluated pre-layout `Content` IR tree.
/// - `Span -> Vec<Content>` lookup table.
/// - Compiler warnings and diagnostics.
pub struct CompiledTypstDocument {
    pub document: PagedDocument,
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
        .map_err(|diags| format_diagnostics(world, &diags))?;

        let evaluated_content = module.content();
        let span_index = build_span_index(&evaluated_content);

        let empty_introspector = EmptyIntrospector;
        let mut previous: Option<PagedDocument> = None;
        let mut iter = 0;
        let mut subsink;
        let mut document: PagedDocument;

        loop {
            subsink = Sink::new();
            let introspector: &dyn Introspector = match &previous {
                Some(doc) => doc.introspector().as_ref(),
                None => &empty_introspector,
            };
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
                .map_err(|diags| format_diagnostics(world, &diags))?;
            iter += 1;

            if constraint.validate(document.introspector().as_ref() as &dyn Introspector) {
                break;
            }

            if iter >= MAX_ITERS {
                subsink.warn(warning!(
                    Span::detached(),
                    "layout did not converge within {MAX_ITERS} attempts"
                ));
                break;
            }
            previous = Some(document);
        }

        sink.extend_from_sink(subsink);
        let delayed = sink.delayed();
        if !delayed.is_empty() {
            return Err(format_diagnostics(world, &delayed));
        }

        Ok(Self {
            document,
            evaluated_content,
            span_index,
            warnings: sink.warnings(),
        })
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
