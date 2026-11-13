//! Post-realization semantic store (`RealizedSemanticStore`), mapping each
//! converged Typst `Location` to its exact scoped `Styles`, pre-resolved
//! style snapshot, source `Span`, and semantic parent `Location`.

use std::collections::HashMap;
use std::sync::Arc;

use typst::foundations::{Smart, StyleChain, StyledElem, Styles, Target, TargetElem};
use typst::introspection::{Location, Tag};
use typst::layout::{BlockElem, Dir, Frame, FrameItem, Length};
use typst::model::{EnumElem, Linebreaks, ListElem, ParElem};
use typst::syntax::Span;
use typst::text::{FontStyle, FontWeight, TextElem};
use typst_layout::PagedDocument;

/// Internal label for document-root style markers.
pub const PAGE_STYLES_LABEL: &str = "__typst2word_page_styles";
/// Internal label for scoped container/paragraph style markers.
pub const SCOPE_STYLES_LABEL: &str = "__typst2word_scope";

/// Pre-resolved typographic and paragraph properties captured at a specific
/// realized scope (resolving `em` units against the local `TextElem::size`).
#[derive(Debug, Clone)]
pub struct RealizedStyleSnapshot {
    pub font_size_pt: f64,
    pub font_family: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub smallcaps: bool,
    pub tracking_pt: Option<f64>,
    pub baseline_shift_pt: Option<f64>,
    pub lang: String,
    pub region: Option<String>,
    pub dir_rtl: bool,
    pub par_leading_pt: f64,
    pub par_spacing_pt: f64,
    pub first_line_indent_pt: f64,
    pub first_line_indent_all: bool,
    pub hanging_indent_pt: f64,
    pub justify: bool,
    pub linebreaks_optimized: Option<bool>,
    pub breakable: bool,
    pub sticky: bool,
    pub list_indent_pt: f64,
    pub list_body_indent_pt: f64,
    pub enum_indent_pt: f64,
    pub enum_body_indent_pt: f64,
}

impl RealizedStyleSnapshot {
    /// Compute a snapshot directly from an active `StyleChain`.
    pub fn from_chain(chain: StyleChain<'_>) -> Self {
        let font_size_pt = chain.resolve(TextElem::size).to_pt();
        let font_family = chain
            .get_ref(TextElem::font)
            .into_iter()
            .next()
            .map(|f| f.as_str().to_string());
        let bold = chain.get(TextElem::weight) >= FontWeight::SEMIBOLD;
        let italic = chain.get(TextElem::style) == FontStyle::Italic;
        let smallcaps = chain.get(TextElem::smallcaps).is_some();

        let raw_tracking = chain.resolve(TextElem::tracking).to_pt();
        let tracking_pt = (raw_tracking.abs() > 1e-4).then_some(raw_tracking);

        // In Typst, positive `baseline` shifts downward (+Y); negate so positive = raised.
        let raw_baseline = -chain.resolve(TextElem::baseline).to_pt();
        let baseline_shift_pt = (raw_baseline.abs() > 1e-4).then_some(raw_baseline);

        let lang = chain.get(TextElem::lang).as_str().to_string();
        let region = chain.get(TextElem::region).map(|r| r.as_str().to_string());
        let dir_rtl = chain.resolve(TextElem::dir) == Dir::RTL;

        let par_leading_pt = resolve_len_pt(chain.get(ParElem::leading), font_size_pt);
        let par_spacing_pt = resolve_len_pt(chain.get(ParElem::spacing), font_size_pt);
        let fl_cfg = chain.get(ParElem::first_line_indent);
        let first_line_indent_pt = resolve_len_pt(fl_cfg.amount(), font_size_pt);
        let first_line_indent_all = fl_cfg.all();
        let hanging_indent_pt = resolve_len_pt(chain.get(ParElem::hanging_indent), font_size_pt);
        let justify = chain.get(ParElem::justify);
        let linebreaks_optimized = match chain.get(ParElem::linebreaks) {
            Smart::Custom(Linebreaks::Optimized) => Some(true),
            Smart::Custom(Linebreaks::Simple) => Some(false),
            Smart::Auto => None,
        };

        let breakable = chain.get(BlockElem::breakable);
        let sticky = chain.get(BlockElem::sticky);
        let list_indent_pt = resolve_len_pt(chain.get(ListElem::indent), font_size_pt);
        let list_body_indent_pt = resolve_len_pt(chain.get(ListElem::body_indent), font_size_pt);
        let enum_indent_pt = resolve_len_pt(chain.get(EnumElem::indent), font_size_pt);
        let enum_body_indent_pt = resolve_len_pt(chain.get(EnumElem::body_indent), font_size_pt);

        Self {
            font_size_pt,
            font_family,
            bold,
            italic,
            smallcaps,
            tracking_pt,
            baseline_shift_pt,
            lang,
            region,
            dir_rtl,
            par_leading_pt,
            par_spacing_pt,
            first_line_indent_pt,
            first_line_indent_all,
            hanging_indent_pt,
            justify,
            linebreaks_optimized,
            breakable,
            sticky,
            list_indent_pt,
            list_body_indent_pt,
            enum_indent_pt,
            enum_body_indent_pt,
        }
    }
}

fn resolve_len_pt(len: Length, em_pt: f64) -> f64 {
    len.abs.to_pt() + len.em.get() * em_pt
}

/// Realized semantic record for a single surviving Typst `Location`.
#[derive(Debug, Clone)]
pub struct RealizedNodeRecord {
    pub location: Location,
    pub span: Span,
    pub semantic_parent: Option<Location>,
    pub styles: Arc<Styles>,
    pub style: RealizedStyleSnapshot,
}

/// Document-wide table of post-`show`, post-convergence semantic records.
#[derive(Debug, Clone)]
pub struct RealizedSemanticStore {
    records: HashMap<u128, RealizedNodeRecord>,
    root_styles: Arc<Styles>,
    root_style: RealizedStyleSnapshot,
}

impl RealizedSemanticStore {
    /// Build a `RealizedSemanticStore` from the final converged `PagedDocument`.
    pub fn from_paged_document(doc: &PagedDocument, library_styles: &Styles) -> Self {
        let target = TargetElem::target.set(Target::Paged).wrap();
        let full_root = StyleChain::new(library_styles).chain(&target).to_map();
        let root_styles = Arc::new(full_root);
        let root_style = RealizedStyleSnapshot::from_chain(StyleChain::new(root_styles.as_ref()));

        let mut builder = StoreBuilder {
            records: HashMap::new(),
            active_styles: Arc::clone(&root_styles),
            active_style: root_style.clone(),
            parent_stack: Vec::new(),
        };

        for page in doc.pages() {
            builder.visit_frame(&page.frame);
        }

        Self {
            records: builder.records,
            root_styles,
            root_style,
        }
    }

    /// Check whether an element label is an internal compiler style marker.
    pub fn is_internal_style_label(label: &str) -> bool {
        label == PAGE_STYLES_LABEL || label == SCOPE_STYLES_LABEL
    }

    /// Look up the realized record for `loc`.
    pub fn get(&self, loc: Location) -> Option<&RealizedNodeRecord> {
        self.records.get(&loc.hash())
    }

    /// Look up the pre-resolved style snapshot for `loc`, falling back to root defaults.
    pub fn style_of(&self, loc: Option<Location>) -> &RealizedStyleSnapshot {
        loc.and_then(|l| self.get(l))
            .map(|r| &r.style)
            .unwrap_or(&self.root_style)
    }

    /// Look up the full realized `Styles` map for `loc`.
    pub fn styles_of(&self, loc: Option<Location>) -> &Styles {
        loc.and_then(|l| self.get(l))
            .map(|r| r.styles.as_ref())
            .unwrap_or(self.root_styles.as_ref())
    }

    /// Construct a full `StyleChain<'a>` for the realized scope active at `loc`.
    pub fn style_chain(&self, loc: Option<Location>) -> StyleChain<'_> {
        StyleChain::new(self.styles_of(loc))
    }

    /// Look up the semantic parent `Location` of `loc`.
    pub fn semantic_parent_of(&self, loc: Location) -> Option<Location> {
        self.get(loc).and_then(|r| r.semantic_parent)
    }

    /// Total number of recorded realized semantic nodes.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

struct StoreBuilder {
    records: HashMap<u128, RealizedNodeRecord>,
    active_styles: Arc<Styles>,
    active_style: RealizedStyleSnapshot,
    parent_stack: Vec<Location>,
}

impl StoreBuilder {
    fn visit_frame(&mut self, frame: &Frame) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Tag(Tag::Start(elem, _)) => {
                    let is_style_marker = elem.label().is_some_and(|l| {
                        RealizedSemanticStore::is_internal_style_label(l.resolve().as_str())
                    });
                    if is_style_marker {
                        if let Some(styled) = elem.to_packed::<StyledElem>() {
                            let arc = Arc::new(styled.styles.clone());
                            self.active_style =
                                RealizedStyleSnapshot::from_chain(StyleChain::new(arc.as_ref()));
                            self.active_styles = arc;
                        }
                        continue;
                    }

                    if let Some(loc) = elem.location() {
                        let parent = self.parent_stack.last().copied().filter(|&p| p != loc);
                        self.records.entry(loc.hash()).or_insert_with(|| RealizedNodeRecord {
                            location: loc,
                            span: elem.span(),
                            semantic_parent: parent,
                            styles: Arc::clone(&self.active_styles),
                            style: self.active_style.clone(),
                        });
                        self.parent_stack.push(loc);
                    }
                }
                FrameItem::Tag(Tag::End(loc, _, _)) => {
                    if let Some(pos) = self.parent_stack.iter().rposition(|&open| open == *loc) {
                        // Only pop `loc` itself so overlapping inline tags remain properly tracked.
                        self.parent_stack.remove(pos);
                    }
                }
                FrameItem::Group(group) => {
                    let saved_styles = Arc::clone(&self.active_styles);
                    let saved_style = self.active_style.clone();
                    let pushed_parent = if let Some(fp) = group.parent {
                        if !self.parent_stack.contains(&fp.location) {
                            self.parent_stack.push(fp.location);
                            Some(fp.location)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    self.visit_frame(&group.frame);

                    if let Some(fp_loc) = pushed_parent
                        && let Some(pos) = self.parent_stack.iter().rposition(|&l| l == fp_loc)
                    {
                        self.parent_stack.remove(pos);
                    }
                    self.active_styles = saved_styles;
                    self.active_style = saved_style;
                }
                _ => {}
            }
        }
    }
}
