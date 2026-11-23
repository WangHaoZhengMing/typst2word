//! Semantic + Visual Extractor (`CompiledTypstDocument` -> `DocumentIr`).
//!
//! Combines Typst's realized semantic store (`RealizedSemanticStore`), tag stream
//! (`Tag::Start(elem)`, `Tag::End(loc)`), pre-layout `CellGrid` semantics, and laid-out
//! `Frame` visual items (`TextItem`, `Shape`, `Image`, `GroupItem`, `Link`) to build a
//! target-independent `DocumentIr` with unified `NodeId`/`NodeOrigin` provenance and
//! 1-to-N `ReferenceLayout` measurements.

mod anchors;
pub mod color;
mod convert;
mod dispatch;
mod document;
mod elements;
mod flow;
mod font_embed;
mod font_metrics;
mod frame_tree;
mod inline;
mod math;
mod prescan;
mod sections;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use typst::foundations::{StyleChain, Styles};
use typst::introspection::Location;
use typst::layout::{GroupItem, Length, Point};
use typst::syntax::Span;
use typst::{World, WorldExt};

use compiler::compile::CompiledTypstDocument;
use compiler::world::TypstWorld;
use ir::{
    BlockLayoutSpanIr, BlockNode, ConversionReport, DocumentIr, FontEmbedMode, HAlignIr,
    LayoutFragmentIr, NodeId, NodeOrigin, NodeReferenceLayout, PageSetupIr, SemanticRoleIr,
    normalize_document_ir,
};

use self::sections::consolidate_page_sections;

/// Extract a complete, structurally normalized `DocumentIr` from a `CompiledTypstDocument` and `TypstWorld`
/// using the default font embedding mode ([`FontEmbedMode::Subset`]).
pub fn extract_document_ir(compiled: &CompiledTypstDocument, world: &TypstWorld) -> DocumentIr {
    extract_document_ir_with_embed_mode(compiled, world, FontEmbedMode::default())
}

/// Extract a complete, structurally normalized `DocumentIr` with an explicit [`FontEmbedMode`].
pub fn extract_document_ir_with_embed_mode(
    compiled: &CompiledTypstDocument,
    world: &TypstWorld,
    embed_mode: FontEmbedMode,
) -> DocumentIr {
    let mut ctx = ExtractorContext::new(compiled, world);
    let mut doc = ctx.extract(embed_mode);
    normalize_document_ir(&mut doc);
    doc
}

/// Scoped geometric and style-realization cursor updated while descending into nested frames,
/// containers, cells, and furniture regions. Mutate inside [`ExtractorContext::with_scope`] so
/// child scopes are atomically restored on exit (including early returns).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TraversalScope {
    /// Nearest enclosing realized `Location` for scoped `StyleChain` lookups.
    pub(crate) active_scope_loc: Option<Location>,
    /// Accumulated horizontal inset (in pt) when inside bordered/padded container blocks.
    pub(crate) container_inset_pt: f64,
    /// Horizontal bounds `(left_pt, right_pt)` of the currently active local `Group` or cell frame.
    pub(crate) active_group_bounds_pt: Option<(f64, f64)>,
    /// Active vertical container height override (`pt`), set while extracting header/footer furniture
    /// so `#block(height: 100%)` resolves against the header/footer margin rather than body height.
    pub(crate) active_region_height_pt: Option<f64>,
    /// Accumulated page-global origin offset (`Point`) when descending into unwrapped `TreeNode::Group`
    /// children, used strictly for `reference_layout` page fragments, line baselines, and flow spans.
    pub(crate) group_origin_offset: Point,
}

pub(crate) struct ExtractorContext<'a> {
    pub(crate) compiled: &'a CompiledTypstDocument,
    pub(crate) world: &'a TypstWorld,
    pub(crate) doc_styles: Styles,
    pub(crate) current_page_index: usize,
    pub(crate) report: ConversionReport,
    /// Word family name of the first realized equation font.
    math_font_family: Option<String>,
    /// Monotonic counter for allocating `NodeId`s (`1..`).
    pub(crate) next_node_id: u32,
    /// Semantic provenance table (`NodeId -> NodeOrigin`).
    pub(crate) origins: BTreeMap<NodeId, NodeOrigin>,
    /// Reference layout measurements (`NodeId -> NodeReferenceLayout`).
    pub(crate) reference_layout: BTreeMap<NodeId, NodeReferenceLayout>,
    /// Stack of active semantic parent `NodeId`s during recursive block extraction.
    pub(crate) parent_node_stack: Vec<NodeId>,
    /// Scoped cursor state restored across nested elements, groups, cells, and furniture regions.
    pub(crate) scope: TraversalScope,
    /// Active `(tracking_pt, baseline_shift_pt)` from `__typst2word_scope` during `frame_tree` build.
    pub(crate) active_text_style: (Option<f64>, Option<f64>),
    /// Active configured `TextElem::font` list from `__typst2word_scope` during `frame_tree` build.
    pub(crate) active_configured_fonts: Vec<String>,
    /// Deduplicated `(configured_fonts, realized_family)` font fallback pairs detected during extraction.
    pub(crate) seen_font_fallbacks: BTreeSet<(String, String)>,
    /// Map from element `Location` to bookmark anchor name (`_typst_loc_<hash>`).
    pub(crate) bookmarks: HashMap<Location, String>,
    /// Map from label string to bookmark anchor name.
    pub(crate) label_bookmarks: HashMap<String, String>,
    /// Pre-extracted footnote bodies keyed by `FootnoteElem` location.
    pub(crate) footnotes: HashMap<Location, Vec<BlockNode>>,
    /// Groups detached via `FrameParent { location, inherit: Inherit::Yes }` (e.g. multi-region cells).
    pub(crate) logical_child_groups: HashMap<Location, Vec<(Point, GroupItem)>>,
    /// Raw footnote entry groups detached via `FrameParent { location, inherit: Inherit::No }`.
    pub(crate) footnote_raw_groups: HashMap<Location, Vec<(Point, GroupItem)>>,
    /// Minimum local Y coordinate (in pt) of detached footnote groups on each 0-based page.
    pub(crate) footnote_top_y_by_page: HashMap<usize, f64>,
    /// Counter for assigning sequential 1-based Word footnote IDs.
    pub(crate) next_footnote_id: usize,
    /// Counter for assigning sequential 1-based numbered block equation numbers.
    pub(crate) next_equation_number: usize,
    /// Set of locations already emitted as unsupported so we don't duplicate badges.
    pub(crate) seen_unsupported_locs: HashSet<Location>,
    /// Left X coordinate (in pt) of the main page content area.
    pub(crate) content_left_pt: f64,
    /// Top Y coordinate (in pt) of the main page content area.
    pub(crate) content_top_pt: f64,
    /// Right X coordinate (in pt) of the main page content area.
    pub(crate) content_right_pt: f64,
    /// Bottom Y coordinate (in pt) of the main page content area.
    pub(crate) content_bottom_pt: f64,
    /// Active section page width (in pt) and whether two-sided mirror margins are active.
    pub(crate) page_width_pt: f64,
    pub(crate) mirror_margins: bool,
    /// Page-level column count (`#set page(columns: N)`).
    pub(crate) page_columns: usize,
    /// Active column layout `(column_count, gutter_pt)` during extraction.
    pub(crate) active_columns: (usize, f64),
    /// Pending list/enum marker text from an orphaned `PdfMarkerTag(ListItemLabel)`.
    pub(crate) pending_list_marker: Option<String>,
    /// Open `FrameItem::Group` that split across a page/column boundary with unclosed tags.
    pub(crate) pending_split_group: Option<frame_tree::PendingSplitGroup>,
    /// Recursion depth inside `FrameItem::Group` frames during `build_frame_tree`.
    pub(crate) group_tree_depth: usize,
}

impl<'a> ExtractorContext<'a> {
    pub(crate) fn new(compiled: &'a CompiledTypstDocument, world: &'a TypstWorld) -> Self {
        let mut doc_styles = Styles::new();
        Self::collect_document_styles(&compiled.evaluated_content, &mut doc_styles);
        let initial_fonts: Vec<String> = StyleChain::new(&doc_styles)
            .get_ref(typst::text::TextElem::font)
            .into_iter()
            .map(|f| f.as_str().to_string())
            .collect();
        Self {
            compiled,
            world,
            doc_styles,
            current_page_index: 0,
            report: ConversionReport::default(),
            math_font_family: None,
            next_node_id: 1,
            origins: BTreeMap::new(),
            reference_layout: BTreeMap::new(),
            parent_node_stack: Vec::new(),
            scope: TraversalScope::default(),
            active_text_style: (None, None),
            active_configured_fonts: initial_fonts,
            seen_font_fallbacks: BTreeSet::new(),
            bookmarks: HashMap::new(),
            label_bookmarks: HashMap::new(),
            footnotes: HashMap::new(),
            logical_child_groups: HashMap::new(),
            footnote_raw_groups: HashMap::new(),
            footnote_top_y_by_page: HashMap::new(),
            next_footnote_id: 1,
            next_equation_number: 1,
            seen_unsupported_locs: HashSet::new(),
            content_left_pt: 70.866,
            content_top_pt: 70.866,
            content_right_pt: 595.276 - 70.866,
            content_bottom_pt: 841.89 - 70.866,
            page_width_pt: 595.276,
            mirror_margins: false,
            page_columns: 1,
            active_columns: (1, 0.0),
            pending_list_marker: None,
            pending_split_group: None,
            group_tree_depth: 0,
        }
    }

    /// Execute `f` with a snapshotted copy of [`Self::scope`], restoring the outer scope on exit.
    #[inline]
    pub(crate) fn with_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.scope;
        let out = f(self);
        self.scope = saved;
        out
    }

    pub(crate) fn current_column_width_pt(&self) -> f64 {
        let full_w = (self.content_right_pt - self.content_left_pt).max(100.0);
        let (count, gutter_pt) = self.active_columns;
        if count > 1 {
            ((full_w - (count - 1) as f64 * gutter_pt) / count as f64).max(72.0)
        } else {
            full_w
        }
    }

    pub(crate) fn current_page_content_height_pt(&self) -> f64 {
        if let Some(region_h) = self.scope.active_region_height_pt {
            return region_h.max(1.0);
        }
        (self.content_bottom_pt - self.content_top_pt).max(120.0)
    }

    /// Document/page-level `StyleChain` (used for page geometry & margins).
    pub(crate) fn style_chain(&self) -> StyleChain<'_> {
        StyleChain::new(&self.doc_styles)
    }

    /// Realized scoped `StyleChain` active at `loc` (or the nearest enclosing element scope).
    pub(crate) fn scoped_style_chain(&self, loc: Option<Location>) -> StyleChain<'_> {
        let target = loc.or(self.scope.active_scope_loc);
        if target.is_some_and(|l| self.compiled.semantics.get(l).is_some()) {
            self.compiled.semantics.style_chain(target)
        } else if !self.doc_styles.is_empty() {
            StyleChain::new(&self.doc_styles)
        } else {
            self.compiled.semantics.style_chain(None)
        }
    }

    /// Allocate a fresh `NodeId` and record its `NodeOrigin` (source span, line/col,
    /// `Location` hash, semantic parent, and role).
    pub(crate) fn alloc_node_id(
        &mut self,
        span: Span,
        loc: Option<Location>,
        role: Option<SemanticRoleIr>,
    ) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id = self.next_node_id.saturating_add(1);

        let (source_file, source_range, source_line_col) = self.resolve_span_origin(span);
        let semantic_parent = self.parent_node_stack.last().copied();
        self.origins.insert(
            id,
            NodeOrigin {
                source_file,
                source_range,
                source_line_col,
                typst_location: loc.map(|l| l.hash()),
                semantic_parent,
                role,
            },
        );
        id
    }

    /// Allocate a `NodeId`, record its `NodeOrigin`, and populate its 1-to-N per-page
    /// `LayoutFragmentIr`s from `nodes`.
    pub(crate) fn alloc_node_with_fragments(
        &mut self,
        span: Span,
        loc: Option<Location>,
        role: Option<SemanticRoleIr>,
        nodes: &[frame_tree::TreeNode],
    ) -> NodeId {
        let id = self.alloc_node_id(span, loc, role);
        let fragments = self.collect_page_fragments(nodes);
        if !fragments.is_empty() {
            let span_ir = BlockLayoutSpanIr::from_fragments(&fragments);
            let entry = self.reference_layout.entry(id).or_default();
            entry.fragments = fragments;
            entry.span = span_ir;
        }
        id
    }

    fn resolve_span_origin(
        &self,
        span: Span,
    ) -> (Option<String>, Option<(usize, usize)>, Option<(usize, usize)>) {
        let Some(file_id) = span.id() else {
            return (None, None, None);
        };
        let path = file_id.vpath().get_without_slash().to_string();
        let Some(range) = self.world.range(span) else {
            return (Some(path), None, None);
        };
        let line_col = self.world.source(file_id).ok().and_then(|src| {
            let line = src.lines().byte_to_line(range.start)?;
            let col = src.lines().byte_to_column(range.start)?;
            Some((line + 1, col + 1))
        });
        (Some(path), Some((range.start, range.end)), line_col)
    }

    /// Group laid-out leaf items in `nodes` by 0-based `page_idx` into 1-to-N `LayoutFragmentIr`s.
    pub(crate) fn collect_page_fragments(
        &self,
        nodes: &[frame_tree::TreeNode],
    ) -> Vec<LayoutFragmentIr> {
        let mut by_page: BTreeMap<usize, (f64, f64, f64, f64)> = BTreeMap::new();
        Self::accumulate_page_fragments(nodes, self.scope.group_origin_offset, &mut by_page);
        by_page
            .into_iter()
            .filter_map(|(page_idx, (min_x, min_y, max_x, max_y))| {
                if !min_x.is_finite() || !min_y.is_finite() {
                    return None;
                }
                Some(LayoutFragmentIr {
                    page_idx,
                    x_pt: min_x,
                    top_y_pt: min_y,
                    width_pt: (max_x - min_x).max(0.0),
                    height_pt: (max_y - min_y).max(0.0),
                })
            })
            .collect()
    }

    fn record_page_rect(
        by_page: &mut BTreeMap<usize, (f64, f64, f64, f64)>,
        page_idx: usize,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        let entry = by_page.entry(page_idx).or_insert((
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ));
        entry.0 = entry.0.min(x1.min(x2));
        entry.1 = entry.1.min(y1.min(y2));
        entry.2 = entry.2.max(x1.max(x2));
        entry.3 = entry.3.max(y1.max(y2));
    }

    fn accumulate_page_fragments(
        nodes: &[frame_tree::TreeNode],
        offset: Point,
        by_page: &mut BTreeMap<usize, (f64, f64, f64, f64)>,
    ) {
        for node in nodes {
            match node {
                frame_tree::TreeNode::Text { text, pos, .. } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let abs_x = (offset.x + pos.x).to_pt();
                    let raw_top = (offset.y + pos.y - text.size * 0.8).to_pt();
                    let (page_idx, top_y, _) = frame_tree::decode_page_and_local_y(raw_top, abs_x);
                    let bot_y = top_y + text.size.to_pt();
                    Self::record_page_rect(
                        by_page,
                        page_idx,
                        abs_x,
                        top_y,
                        abs_x + text.width().to_pt(),
                        bot_y,
                    );
                }
                frame_tree::TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let x1 = (offset.x + pos.x + b.min.x).to_pt();
                    let x2 = (offset.x + pos.x + b.max.x).to_pt();
                    let raw_y1 = (offset.y + pos.y + b.min.y).to_pt();
                    let (page_idx, y1, _) = frame_tree::decode_page_and_local_y(raw_y1, x1);
                    let y2 = y1 + b.size().y.to_pt().abs();
                    Self::record_page_rect(by_page, page_idx, x1, y1, x2, y2);
                }
                frame_tree::TreeNode::Image { size, pos, .. } => {
                    let x1 = (offset.x + pos.x).to_pt();
                    let raw_y1 = (offset.y + pos.y).to_pt();
                    let (page_idx, y1, _) = frame_tree::decode_page_and_local_y(raw_y1, x1);
                    Self::record_page_rect(
                        by_page,
                        page_idx,
                        x1,
                        y1,
                        x1 + size.x.to_pt(),
                        y1 + size.y.to_pt(),
                    );
                }
                frame_tree::TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let next_off = offset + *pos;
                    if !children.is_empty() {
                        Self::accumulate_page_fragments(children, next_off, by_page);
                    } else if group.frame.size().x.to_pt() > 0.0
                        || group.frame.size().y.to_pt() > 0.0
                    {
                        let x1 = next_off.x.to_pt();
                        let (page_idx, y1, _) =
                            frame_tree::decode_page_and_local_y(next_off.y.to_pt(), x1);
                        Self::record_page_rect(
                            by_page,
                            page_idx,
                            x1,
                            y1,
                            x1 + group.frame.size().x.to_pt(),
                            y1 + group.frame.size().y.to_pt(),
                        );
                    }
                }
                frame_tree::TreeNode::Element { elem, children, .. } => {
                    if Self::is_page_furniture_elem(elem) {
                        continue;
                    }
                    Self::accumulate_page_fragments(children, offset, by_page);
                }
                _ => {}
            }
        }
    }

    /// Resolve a Typst `Length` (which has both an absolute `Abs` component and a font-relative
    /// `Em` component) to points (`pt`) given the active font size in points.
    pub(crate) fn resolve_length_pt(len: Length, font_size_pt: f64) -> f64 {
        len.abs.to_pt() + len.em.get() * font_size_pt
    }

    pub(crate) fn extract(&mut self, embed_mode: FontEmbedMode) -> DocumentIr {
        self.report.pages_count = self.compiled.document.pages().len();
        for warn in &self.compiled.warnings {
            let msg = warn.message.to_string();
            self.report.compiler_warnings.push(msg);
        }

        // 1. Index bookmarks for all introspectable elements (`HeadingElem`, `FigureElem`,
        //    `EquationElem`, labelled elements, etc.).
        for elem in self.compiled.all_introspected() {
            if let Some(loc) = elem.location() {
                let bm = format!("_typst_loc_{:x}", loc.hash());
                self.bookmarks.insert(loc, bm.clone());
                if let Some(lbl) = elem.label() {
                    self.label_bookmarks
                        .insert(lbl.resolve().as_str().to_string(), bm);
                }
            }
        }

        // 2. Pre-scan all page frames for detached `FrameParent` groups (footnotes & multi-frame continuations).
        for (page_idx, page) in self.compiled.document.pages().iter().enumerate() {
            self.collect_detached_parent_groups(page_idx, &page.frame, Point::zero());
        }

        // 3. Extract footnote bodies from `footnote_raw_groups` (sharing open stack across split pages!).
        let footnote_locs: Vec<Location> = self.footnote_raw_groups.keys().copied().collect();
        for fn_loc in footnote_locs {
            if let Some(groups) = self.footnote_raw_groups.remove(&fn_loc) {
                let mut fn_tree = Vec::new();
                let mut fn_stack = Vec::new();
                for (pos, grp) in groups {
                    self.append_frame_items_to_tree(&grp.frame, pos, &mut fn_tree, &mut fn_stack);
                }
                self.flush_open_stack(&mut fn_tree, &mut fn_stack);
                let blocks = self.extract_footnote_entry_blocks(&fn_tree);
                self.footnotes.insert(fn_loc, blocks);
            }
        }

        // 4. Extract document metadata and per-page setup/styles.
        let metadata = self.extract_metadata();
        let mut page_setups = Vec::new();
        let mut page_styles = Vec::new();
        let saved_groups = self.logical_child_groups.clone();
        for index in 0..self.compiled.document.pages().len() {
            self.logical_child_groups = saved_groups.clone();
            let frame = self.compiled.document.pages()[index].frame.clone();
            if let Some(styles) = Self::page_styles_from_frame(&frame) {
                self.doc_styles = styles;
            }
            page_styles.push(self.doc_styles.clone());
            let nodes = self.build_frame_tree(&frame, Point::zero());
            page_setups.push(self.extract_page_setup(index, &nodes));
        }
        self.logical_child_groups = saved_groups;
        self.pending_split_group = None;

        // Consolidate consecutive pages into Word sections.
        let section_plans = consolidate_page_sections(&page_setups, &page_styles);

        let mut sections = Vec::new();
        for (range, setup) in section_plans {
            self.current_page_index = range.start;
            if let Some(styles) = page_styles.get(range.start) {
                self.doc_styles = styles.clone();
            }
            self.content_left_pt = setup.margin_left_pt;
            self.content_top_pt = setup.margin_top_pt;
            self.content_right_pt =
                (setup.width_pt - setup.margin_right_pt).max(setup.margin_left_pt + 36.0);
            self.content_bottom_pt =
                (setup.height_pt - setup.margin_bottom_pt).max(setup.margin_top_pt + 36.0);
            self.page_width_pt = setup.width_pt;
            self.mirror_margins = setup.mirror_margins;
            self.page_columns = setup.columns;
            self.active_columns = (setup.columns, 18.0);
            let nodes = self.build_document_frame_tree(range);
            let blocks = self.extract_blocks_from_nodes(&nodes, 0, HAlignIr::Left);
            sections.push(ir::DocumentSectionIr {
                page_setup: setup,
                blocks,
            });
        }

        let (page_setup, blocks) = if !sections.is_empty() {
            let first = sections.remove(0);
            (first.page_setup, first.blocks)
        } else {
            (PageSetupIr::default(), Vec::new())
        };
        let anchors = self.extract_page_line_anchors();
        let font_metrics = self.collect_document_font_metrics();
        let embedded = self.collect_embedded_fonts(embed_mode);
        // Warn when realized fonts in the document are embedded in Typst rather than installed
        // on the host OS AND were not embedded into the `.docx` package.
        for (family, metrics) in &font_metrics {
            if !metrics.is_system_installed
                && !embedded.embedded_typst_families.contains(family)
                && self
                    .seen_font_fallbacks
                    .insert(("<host>".to_string(), family.clone()))
            {
                self.report.font_fallback_warnings.push(format!(
                    "系统未安装字体 \"{family}\"（Typst 内置字体），Word 打开时将由系统回退渲染（未更改原字体名称）"
                ));
            }
        }
        self.report.font_embed_notes = embedded.notes;

        let cjk_latin_spacing = page_styles
            .first()
            .map(|styles| {
                StyleChain::new(styles)
                    .get(typst::text::TextElem::cjk_latin_spacing)
                    .is_auto()
            })
            .unwrap_or(true);

        let mut doc = DocumentIr::default();
        doc.sections = sections;
        doc.metadata = metadata;
        doc.page_setup = page_setup;
        doc.blocks = blocks;
        doc.cjk_latin_spacing = cjk_latin_spacing;
        doc.font_metrics = font_metrics;
        doc.math_font_family = self.math_font_family.take();
        doc.embedded_fonts = embedded.faces;
        doc.origins = std::mem::take(&mut self.origins);
        doc.reference_layout = std::mem::take(&mut self.reference_layout);
        doc.report = self.report.clone();
        doc.anchors = anchors;
        doc
    }
}
