//! Semantic + Visual Extractor (`CompiledTypstDocument` -> `DocumentIr`).
//!
//! Combines Typst's introspection/tag stream (`Tag::Start(elem)`, `Tag::End(loc)`),
//! pre-layout `Content` / `CellGrid` semantics, and laid-out `Frame` visual items
//! (`TextItem`, `Shape`, `Image`, `GroupItem`, `Link`) to reconstruct a rich
//! `DocumentIr` with maximum semantic accuracy and visual fidelity.
//!
//! Layout:
//! - `frame_tree`: tag stream + frames -> `TreeNode` tree
//! - `document`: metadata, page setup, header / footer
//! - `dispatch`: routes each element to its extractor
//! - `elements`: one file per block-level Typst object
//! - `inline`: inline content and text runs
//! - `convert`: Typst value -> IR value helpers

mod convert;
mod dispatch;
mod document;
mod elements;
mod frame_tree;
mod inline;

use std::collections::{HashMap, HashSet};

use typst::foundations::{Content, SequenceElem, StyleChain, StyledElem, Styles};
use typst::introspection::Location;
use typst::layout::{GroupItem, Length, Point};

use crate::compiler::compile::CompiledTypstDocument;
use crate::compiler::world::TypstWorld;
use crate::extract::frame_tree::TreeNode;
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
    /// Left X coordinate (in pt) of the main page content area.
    pub(crate) content_left_pt: f64,
    /// Right X coordinate (in pt) of the main page content area.
    pub(crate) content_right_pt: f64,
}

impl<'a> ExtractorContext<'a> {
    pub(crate) fn new(compiled: &'a CompiledTypstDocument, world: &'a TypstWorld) -> Self {
        let mut doc_styles = Styles::new();
        Self::collect_document_styles(&compiled.evaluated_content, &mut doc_styles);
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
            content_left_pt: 70.866,
            content_right_pt: 595.276 - 70.866,
        }
    }

    pub(crate) fn collect_document_styles(content: &Content, out: &mut Styles) {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            out.apply(styled.styles.clone());
            Self::collect_document_styles(&styled.child, out);
        } else if let Some(seq) = content.to_packed::<SequenceElem>() {
            for child in &seq.children {
                if child.is::<StyledElem>() || child.is::<SequenceElem>() {
                    Self::collect_document_styles(child, out);
                }
            }
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

        // 4. Build frame trees once per page so `logical_child_groups` are attached cleanly.
        let mut page_trees: Vec<Vec<TreeNode>> =
            Vec::with_capacity(self.compiled.document.pages().len());
        for page in self.compiled.document.pages() {
            page_trees.push(self.build_frame_tree(&page.frame, Point::zero()));
        }

        // 5. Extract document metadata and page setup (including header/footer from page 1).
        let metadata = self.extract_metadata();
        let first_page_nodes = page_trees.first().cloned().unwrap_or_default();
        let page_setup = self.extract_page_setup(&first_page_nodes);

        // 6. Combine page trees across all pages. Only insert hard `PageBreak` nodes between
        //    pages if the source document explicitly used `#pagebreak()` (`PagebreakElem`).
        let has_explicit_pb = Self::has_explicit_pagebreak(&self.compiled.evaluated_content);
        let mut all_root_nodes: Vec<TreeNode> = Vec::new();
        for (page_idx, page_nodes) in page_trees.into_iter().enumerate() {
            if page_idx > 0 && has_explicit_pb {
                all_root_nodes.push(TreeNode::PageBreak);
            }
            all_root_nodes.extend(page_nodes);
        }

        let mut blocks = self.extract_blocks_from_nodes(&all_root_nodes, 0, HAlignIr::Left);

        // Also scan pre-layout `evaluated_content` for any elements that Typst's paged layout
        // completely drops without emitting tags (such as `PagebreakElem` or `ColbreakElem` or
        // `AttachElem` if not already captured).
        self.clean_redundant_pagebreaks(&mut blocks);

        DocumentIr {
            metadata,
            page_setup,
            blocks,
            report: self.report.clone(),
        }
    }
}
