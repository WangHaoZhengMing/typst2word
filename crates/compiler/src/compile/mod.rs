//! Self-contained Typst 0.15 compilation & introspection/query engine.
//!
//! Submodules:
//! - [`routines`]: Custom `Routines` VTable, pre-realization synthetic location annotation, and internal tag helpers.
//! - [`par`]: `RealizationKind::Par` inline run coalescing (`StrongElem` / `SmallcapsElem`) and inline scope markers.
//! - [`layout`]: `RealizationKind::Fragment` & `RealizationKind::Document` container/pagebreak realization hooks.

mod layout;
mod par;
mod routines;

pub use routines::{CUSTOM_ROUTINES, is_cetz_span};

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

use comemo::Track;
use ecow::{EcoString, EcoVec};
use typst::diag::{SourceDiagnostic, SourceResult};
use typst::engine::{Engine, Route, Sink, Traced};
use typst::foundations::{
    Content, Label, NativeElement, Packed, Selector, SequenceElem, StyleChain, StyledElem, Styles,
    Target, TargetElem,
};
use typst::introspection::{EmptyIntrospector, Introspector, MAX_ITERS};
use typst::routines::{Arenas, Pair, RealizationKind};
use typst::syntax::Span;
use typst::utils::Protected;
use typst::{World, WorldExt};
use typst_layout::PagedDocument;

use crate::semantic::RealizedSemanticStore;
use crate::world::TypstWorld;

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
    /// with `RealizationKind::Math` using the converged `Introspector` and local `Styles`.
    ///
    /// Resolves `#show` rules (including regex rules like `#show "RR": math.bb("R")`),
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
