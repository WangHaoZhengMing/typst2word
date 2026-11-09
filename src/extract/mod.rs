//! Semantic + Visual Extractor (`CompiledTypstDocument` -> `DocumentIr`).
//!
//! Combines Typst's introspection/tag stream (`Tag::Start(elem)`, `Tag::End(loc)`),
//! pre-layout `Content` / `CellGrid` semantics, and laid-out `Frame` visual items
//! (`TextItem`, `Shape`, `Image`, `GroupItem`, `Link`) to reconstruct a rich
//! `DocumentIr` with maximum semantic accuracy and visual fidelity.
//!
//! Layout:
//! - `prescan`: document-level styles and explicit linebreak pre-scanning
//! - `frame_tree`: tag stream + frames -> `TreeNode` tree
//! - `document`: metadata, page setup, header / footer
//! - `flow`: block sequence walking, stray inline coalescing, cross-page container merging
//! - `dispatch`: routes each block element to its extractor in `elements/`
//! - `elements`: one file per block-level Typst object
//! - `inline`: inline content, formatting, links, footnotes, boxes, and text runs
//! - `math`: Typst math AST -> semantic `MathNodeIr`
//! - `convert`: Typst value -> IR value helpers

mod anchors;
mod convert;
mod dispatch;
mod document;
mod elements;
mod flow;
mod frame_tree;
mod inline;
mod math;
mod prescan;

use std::collections::{HashMap, HashSet};

use typst::foundations::{StyleChain, Styles};
use typst::introspection::Location;
use typst::layout::{GroupItem, Length, Point};

use crate::compiler::compile::CompiledTypstDocument;
use crate::compiler::world::TypstWorld;
use crate::ir::{BlockNode, ConversionReport, DocumentIr, HAlignIr};

/// Extract a complete `DocumentIr` from a `CompiledTypstDocument` and `TypstWorld`.
pub fn extract_document_ir(compiled: &CompiledTypstDocument, world: &TypstWorld) -> DocumentIr {
    let mut ctx = ExtractorContext::new(compiled, world);
    ctx.extract()
}

pub(crate) struct ExtractorContext<'a> {
    pub(crate) compiled: &'a CompiledTypstDocument,
    pub(crate) world: &'a TypstWorld,
    pub(crate) doc_styles: Styles,
    pub(crate) report: ConversionReport,
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
    /// Counter for assigning sequential 1-based Word footnote IDs.
    pub(crate) next_footnote_id: usize,
    /// Counter for assigning sequential 1-based numbered block equation numbers.
    pub(crate) next_equation_number: usize,
    /// Set of locations already emitted as unsupported so we don't duplicate badges.
    pub(crate) seen_unsupported_locs: HashSet<Location>,
    /// Trailing text snippets immediately preceding an explicit `LinebreakElem` (`\`) in source.
    pub(crate) linebreak_preceding_texts: HashSet<String>,
    /// Left X coordinate (in pt) of the main page content area.
    pub(crate) content_left_pt: f64,
    /// Right X coordinate (in pt) of the main page content area.
    pub(crate) content_right_pt: f64,
    /// Page-level column count (`#set page(columns: N)`).
    pub(crate) page_columns: usize,
    /// Active column layout `(column_count, gutter_pt)` during extraction.
    pub(crate) active_columns: (usize, f64),
    /// Accumulated horizontal inset (in pt) when inside bordered/padded container blocks.
    pub(crate) container_inset_pt: f64,
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
        let mut linebreak_preceding_texts = HashSet::new();
        Self::collect_linebreak_preceding_texts(
            &compiled.evaluated_content,
            &mut linebreak_preceding_texts,
        );
        Self {
            compiled,
            world,
            doc_styles,
            report: ConversionReport::default(),
            bookmarks: HashMap::new(),
            label_bookmarks: HashMap::new(),
            footnotes: HashMap::new(),
            logical_child_groups: HashMap::new(),
            footnote_raw_groups: HashMap::new(),
            next_footnote_id: 1,
            next_equation_number: 1,
            seen_unsupported_locs: HashSet::new(),
            linebreak_preceding_texts,
            content_left_pt: 70.866,
            content_right_pt: 595.276 - 70.866,
            page_columns: 1,
            active_columns: (1, 0.0),
            container_inset_pt: 0.0,
            pending_list_marker: None,
            pending_split_group: None,
            group_tree_depth: 0,
        }
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

    pub(crate) fn style_chain(&self) -> StyleChain<'_> {
        StyleChain::new(&self.doc_styles)
    }

    /// Resolve a Typst `Length` (which has both an absolute `Abs` component and a font-relative
    /// `Em` component) to points (`pt`) given the active font size in points.
    pub(crate) fn resolve_length_pt(len: Length, font_size_pt: f64) -> f64 {
        len.abs.to_pt() + len.em.get() * font_size_pt
    }

    pub(crate) fn extract(&mut self) -> DocumentIr {
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
        for page in self.compiled.document.pages() {
            self.collect_detached_parent_groups(&page.frame, Point::zero());
        }

        // 3. Extract footnote bodies from `footnote_raw_groups` and `FootnoteEntry` elements.
        let footnote_locs: Vec<Location> = self.footnote_raw_groups.keys().copied().collect();
        for fn_loc in footnote_locs {
            if let Some(groups) = self.footnote_raw_groups.remove(&fn_loc) {
                let mut fn_tree = Vec::new();
                for (pos, grp) in groups {
                    let nodes = self.build_frame_tree(&grp.frame, pos);
                    fn_tree.extend(nodes);
                }
                let blocks = self.extract_footnote_entry_blocks(&fn_tree);
                self.footnotes.insert(fn_loc, blocks);
            }
        }

        // 4. Extract document metadata and page setup (including header/footer from page 1).
        let metadata = self.extract_metadata();
        let first_page_nodes = self
            .compiled
            .document
            .pages()
            .first()
            .map(|p| {
                let saved_groups = self.logical_child_groups.clone();
                let nodes = self.build_frame_tree(&p.frame, Point::zero());
                self.logical_child_groups = saved_groups;
                nodes
            })
            .unwrap_or_default();
        let page_setup = self.extract_page_setup(&first_page_nodes);

        // 5. Build a continuous frame tree across all pages, keeping cross-page open tags
        //    (`EnumElem`, `ListElem`, `ParElem`, etc.) intact and only inserting `PageBreak`
        //    at explicit `#pagebreak()` (`PagebreakElem`) boundaries.
        let mut pagebreak_targets = Vec::new();
        Self::collect_pagebreak_following_texts(
            &self.compiled.evaluated_content,
            &mut pagebreak_targets,
        );
        let all_root_nodes = self.build_document_frame_tree(pagebreak_targets);

        let mut blocks = self.extract_blocks_from_nodes(&all_root_nodes, 0, HAlignIr::Left);
        self.clean_redundant_pagebreaks(&mut blocks);
        let anchors = self.extract_page_line_anchors();

        DocumentIr {
            metadata,
            page_setup,
            blocks,
            report: self.report.clone(),
            anchors,
        }
    }
}
