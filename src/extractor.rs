//! Semantic + Visual Extractor (`CompiledTypstDocument` -> `DocumentIr`).
//!
//! Combines Typst's introspection/tag stream (`Tag::Start(elem)`, `Tag::End(loc)`),
//! pre-layout `Content` / `CellGrid` semantics, and laid-out `Frame` visual items
//! (`TextItem`, `Shape`, `Image`, `GroupItem`, `Link`) to reconstruct a rich
//! `DocumentIr` with maximum semantic accuracy and visual fidelity.

use std::collections::{HashMap, HashSet};

use typst::foundations::{Content, Packed, SequenceElem, Smart, StyleChain, StyledElem};
use typst::introspection::{Location, Tag};
use typst::layout::{
    Abs, AlignElem, Alignment, ColbreakElem, ColumnsElem, Corners, Frame, FrameItem, FrameKind,
    GridCell, GridElem, GroupItem, HAlignment, HideElem, Inherit, LayoutElem, Length, MoveElem,
    PageElem, PagebreakElem, Point, Rel, RepeatElem, RotateElem, ScaleAmount, ScaleElem, Sides,
    Size, SkewElem, Transform, VAlignment,
};
use typst::math::EquationElem;
use typst::model::{
    BibliographyElem, CiteGroup, Destination, DirectLinkElem, EmphElem, EnumElem, FigureCaption,
    FigureElem, FootnoteElem, FootnoteEntry, HeadingElem, LinkElem, LinkMarker, ListElem,
    OutlineElem, OutlineEntry, ParElem, QuoteElem, RefElem, StrongElem, TableCell, TableElem,
    TermsElem, TitleElem,
};
use typst::pdf::{
    ArtifactElem, ArtifactKind, AttachElem, PdfMarkerTag, PdfMarkerTagKind, TableCellKind,
};
use typst::syntax::Span;
use typst::text::{
    FontWeight, HighlightElem, LinebreakElem, OverlineElem, RawElem, RawLine, SmallcapsElem,
    StrikeElem, SubElem, SuperElem, TextItem, UnderlineElem,
};
use typst::visualize::{
    CircleElem, CurveElem, CurveItem, EllipseElem, ExchangeFormat, FillRule, FixedStroke, Geometry,
    Image, ImageKind, LineCap, LineElem, LineJoin, Paint, PathElem, PolygonElem, RasterFormat,
    RectElem, Shape, SquareElem, Stroke,
};
use typst_html::{FrameElem, HtmlElem};

use crate::compiler::compile::{CompiledTypstDocument, is_cetz_span};
use crate::compiler::world::TypstWorld;
use crate::ir::*;
use crate::math_omml::{convert_equation_to_omml, paint_to_hex};

/// Extract a complete `DocumentIr` from a `CompiledTypstDocument` and `TypstWorld`.
pub fn extract_document_ir(compiled: &CompiledTypstDocument, world: &TypstWorld) -> DocumentIr {
    let mut ctx = ExtractorContext::new(compiled, world);
    ctx.extract()
}

/// Tree representation unifying `Tag::Start` / `Tag::End` semantic nodes with
/// laid-out `FrameItem` visual nodes.
#[derive(Debug, Clone)]
enum TreeNode {
    Element {
        elem: Content,
        loc: Location,
        pos: Point,
        children: Vec<TreeNode>,
    },
    Group {
        group: GroupItem,
        pos: Point,
        children: Vec<TreeNode>,
    },
    Text {
        text: TextItem,
        pos: Point,
    },
    Shape {
        shape: Shape,
        span: Span,
        pos: Point,
    },
    Image {
        image: Image,
        size: Size,
        span: Span,
        pos: Point,
    },
    Link {
        dest: Destination,
        #[allow(dead_code)]
        size: Size,
        #[allow(dead_code)]
        pos: Point,
    },
    PageBreak,
}

#[derive(Debug, Clone)]
struct OpenElement {
    elem: Content,
    loc: Location,
    pos: Point,
    children: Vec<TreeNode>,
}

/// Active inline formatting context accumulated while walking inside a paragraph.
#[derive(Debug, Clone, Default)]
struct ActiveInlineState {
    bold: bool,
    italic: bool,
    underline: Option<StrokeIr>,
    strike: Option<StrokeIr>,
    overline: Option<StrokeIr>,
    highlight: Option<ColorHex>,
    script: ScriptKindIr,
    smallcaps: bool,
    is_code: bool,
    hidden: bool,
}

struct ExtractorContext<'a> {
    compiled: &'a CompiledTypstDocument,
    world: &'a TypstWorld,
    report: ConversionReport,
    /// Map from element `Location` to bookmark anchor name (`_typst_loc_<hash>`).
    bookmarks: HashMap<Location, String>,
    /// Map from label string to bookmark anchor name.
    label_bookmarks: HashMap<String, String>,
    /// Pre-extracted footnote bodies keyed by `FootnoteElem` location.
    footnotes: HashMap<Location, Vec<BlockNode>>,
    /// Groups detached via `FrameParent { location, inherit: Inherit::Yes }` (e.g. multi-region cells).
    logical_child_groups: HashMap<Location, Vec<(Point, GroupItem)>>,
    /// Raw footnote entry groups detached via `FrameParent { location, inherit: Inherit::No }`.
    footnote_raw_groups: HashMap<Location, Vec<(Point, GroupItem)>>,
    /// Counter for assigning sequential 1-based Word footnote IDs.
    next_footnote_id: usize,
    /// Set of locations already emitted as unsupported so we don't duplicate badges.
    seen_unsupported_locs: HashSet<Location>,
    /// Left X coordinate (in pt) of the main page content area.
    content_left_pt: f64,
    /// Right X coordinate (in pt) of the main page content area.
    content_right_pt: f64,
}

impl<'a> ExtractorContext<'a> {
    fn new(compiled: &'a CompiledTypstDocument, world: &'a TypstWorld) -> Self {
        Self {
            compiled,
            world,
            report: ConversionReport::default(),
            bookmarks: HashMap::new(),
            label_bookmarks: HashMap::new(),
            footnotes: HashMap::new(),
            logical_child_groups: HashMap::new(),
            footnote_raw_groups: HashMap::new(),
            next_footnote_id: 1,
            seen_unsupported_locs: HashSet::new(),
            content_left_pt: 70.866,
            content_right_pt: 595.276 - 70.866,
        }
    }

    fn extract(&mut self) -> DocumentIr {
        self.report.pages_count = self.compiled.document.pages.len();
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
        for page in &self.compiled.document.pages {
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
            Vec::with_capacity(self.compiled.document.pages.len());
        for page in &self.compiled.document.pages {
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

    fn has_explicit_pagebreak(content: &Content) -> bool {
        if content.is::<PagebreakElem>() {
            return true;
        }
        if let Some(styled) = content.to_packed::<StyledElem>() {
            return Self::has_explicit_pagebreak(&styled.child);
        }
        if let Some(seq) = content.to_packed::<SequenceElem>() {
            return seq.children.iter().any(Self::has_explicit_pagebreak);
        }
        false
    }

    fn clean_redundant_pagebreaks(&self, blocks: &mut Vec<BlockNode>) {
        // Trim leading or trailing PageBreaks and collapse consecutive PageBreaks.
        while matches!(blocks.first(), Some(BlockNode::PageBreak)) {
            blocks.remove(0);
        }
        while matches!(blocks.last(), Some(BlockNode::PageBreak)) {
            blocks.pop();
        }
        let mut i = 1;
        while i < blocks.len() {
            if matches!(blocks[i], BlockNode::PageBreak)
                && matches!(blocks[i - 1], BlockNode::PageBreak)
            {
                blocks.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn extract_metadata(&self) -> MetadataIr {
        let info = &self.compiled.document.info;
        MetadataIr {
            title: info.title.as_ref().map(|s| s.to_string()),
            authors: info.author.iter().map(|s| s.to_string()).collect(),
            description: info.description.as_ref().map(|s| s.to_string()),
            keywords: info.keywords.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn extract_page_setup(&mut self, page_nodes: &[TreeNode]) -> PageSetupIr {
        let mut setup = PageSetupIr::default();
        if let Some(first_page) = self.compiled.document.pages.first() {
            let size = first_page.frame.size();
            setup.width_pt = size.x.to_pt();
            setup.height_pt = size.y.to_pt();
            setup.fill = first_page
                .fill
                .clone()
                .custom()
                .and_then(|p| p.as_ref().and_then(paint_to_hex));

            // Check if the document uses columns (`#set page(columns: N)` or `#columns(N)`).
            if let Some(cols) = Self::find_columns_in_content(
                &self.compiled.evaluated_content,
                StyleChain::default(),
            ) {
                setup.columns = cols;
            } else {
                let cols = self.compiled.query_elem::<ColumnsElem>();
                if let Some(col_elem) = cols.first() {
                    setup.columns = col_elem.count.get(StyleChain::default()).get();
                }
            }

            // Estimate margins from the first page content position if available BEFORE extracting
            // headers/footers so `self.content_left_pt` and `self.content_right_pt` are accurate!
            if let Some((min_x, min_y)) = Self::find_first_content_origin(page_nodes) {
                if min_x > 10.0 && min_x < setup.width_pt * 0.4 {
                    setup.margin_left_pt = min_x;
                    setup.margin_right_pt = min_x;
                }
                if min_y > 10.0 && min_y < setup.height_pt * 0.4 {
                    setup.margin_top_pt = min_y;
                    setup.margin_bottom_pt = min_y;
                }
            }
            self.content_left_pt = setup.margin_left_pt;
            self.content_right_pt =
                (setup.width_pt - setup.margin_right_pt).max(setup.margin_left_pt + 100.0);

            // Extract header & footer from ArtifactElem(Header / Footer) on the first page.
            for node in page_nodes {
                self.collect_header_footer_artifacts(node, &mut setup.header, &mut setup.footer);
            }
        }
        setup
    }

    fn find_columns_in_content(content: &Content, styles: StyleChain) -> Option<usize> {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            let chained = styles.chain(&styled.styles);
            let page_cols = chained.get(PageElem::columns).get();
            if page_cols > 1 {
                return Some(page_cols);
            }
            return Self::find_columns_in_content(&styled.child, chained);
        }
        if let Some(col_elem) = content.to_packed::<ColumnsElem>() {
            let c = col_elem.count.get(styles).get();
            if c > 1 {
                return Some(c);
            }
        }
        if let Some(seq) = content.to_packed::<SequenceElem>() {
            for child in &seq.children {
                if let Some(c) = Self::find_columns_in_content(child, styles) {
                    return Some(c);
                }
            }
        }
        None
    }

    fn collect_header_footer_artifacts(
        &mut self,
        node: &TreeNode,
        header: &mut Vec<BlockNode>,
        footer: &mut Vec<BlockNode>,
    ) {
        match node {
            TreeNode::Element { elem, children, .. } => {
                if let Some(artifact) = elem.to_packed::<ArtifactElem>() {
                    match artifact.kind.get(StyleChain::default()) {
                        ArtifactKind::Header => {
                            let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
                            let blocks = self.extract_blocks_from_nodes(children, 0, align);
                            header.extend(blocks);
                            return;
                        }
                        ArtifactKind::Footer => {
                            let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
                            let blocks = self.extract_blocks_from_nodes(children, 0, align);
                            footer.extend(blocks);
                            return;
                        }
                        _ => {}
                    }
                }
                for child in children {
                    self.collect_header_footer_artifacts(child, header, footer);
                }
            }
            TreeNode::Group { children, .. } => {
                for child in children {
                    self.collect_header_footer_artifacts(child, header, footer);
                }
            }
            _ => {}
        }
    }

    fn find_first_content_origin(nodes: &[TreeNode]) -> Option<(f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    pos,
                    children,
                    ..
                } => {
                    if elem.is::<ArtifactElem>() || elem.is::<FootnoteEntry>() {
                        continue;
                    }
                    if pos.x.to_pt() > 0.0 && pos.y.to_pt() > 0.0 {
                        return Some((pos.x.to_pt(), pos.y.to_pt()));
                    }
                    if let Some(found) = Self::find_first_content_origin(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    if let Some((cx, cy)) = Self::find_first_content_origin(children) {
                        return Some((pos.x.to_pt() + cx, pos.y.to_pt() + cy));
                    }
                }
                TreeNode::Text { pos, .. } => {
                    if pos.x.to_pt() > 0.0 && pos.y.to_pt() > 0.0 {
                        return Some((pos.x.to_pt(), pos.y.to_pt()));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Pre-scan a frame to collect `GroupItem`s that have `group.parent.is_some()`.
    fn collect_detached_parent_groups(&mut self, frame: &Frame, offset: Point) {
        for (pos, item) in frame.items() {
            let abs_pos = offset + *pos;
            if let FrameItem::Group(group) = item {
                if let Some(parent) = group.parent {
                    match parent.inherit {
                        Inherit::No => {
                            self.footnote_raw_groups
                                .entry(parent.location)
                                .or_default()
                                .push((abs_pos, group.clone()));
                        }
                        Inherit::Yes => {
                            self.logical_child_groups
                                .entry(parent.location)
                                .or_default()
                                .push((abs_pos, group.clone()));
                        }
                    }
                } else {
                    self.collect_detached_parent_groups(&group.frame, abs_pos);
                }
            }
        }
    }

    /// Build a hierarchical `Vec<TreeNode>` from `frame`, flattening soft untransformed
    /// line frames while keeping hard/transformed groups and pairing `Tag::Start` / `Tag::End`.
    fn build_frame_tree(&mut self, frame: &Frame, base_offset: Point) -> Vec<TreeNode> {
        let mut flat_items: Vec<(Point, FrameItem)> = Vec::new();
        Self::flatten_soft_groups(frame, base_offset, &mut flat_items);

        let mut root_nodes: Vec<TreeNode> = Vec::new();
        let mut stack: Vec<OpenElement> = Vec::new();

        for (pos, item) in flat_items {
            match item {
                FrameItem::Tag(Tag::Start(elem, _flags)) => {
                    let loc = elem
                        .location()
                        .unwrap_or_else(|| Location::new(typst_utils::hash128(&elem)));
                    stack.push(OpenElement {
                        elem,
                        loc,
                        pos,
                        children: Vec::new(),
                    });
                }
                FrameItem::Tag(Tag::End(loc, _key, _flags)) => {
                    if let Some(match_idx) = stack.iter().rposition(|open| open.loc == loc) {
                        while stack.len() > match_idx {
                            let mut closed = stack.pop().unwrap();
                            // Attach any logical child groups registered for `closed.loc`.
                            if let Some(detached) = self.logical_child_groups.remove(&closed.loc) {
                                for (gpos, grp) in detached {
                                    let gchildren = self.build_frame_tree(&grp.frame, gpos);
                                    closed.children.extend(gchildren);
                                }
                            }
                            let node = TreeNode::Element {
                                elem: closed.elem,
                                loc: closed.loc,
                                pos: closed.pos,
                                children: closed.children,
                            };
                            Self::push_node(&mut root_nodes, &mut stack, node);
                        }
                    }
                }
                FrameItem::Group(group) => {
                    if group.parent.is_some() {
                        // Handled via `footnote_raw_groups` / `logical_child_groups`.
                        continue;
                    }
                    let children = self.build_frame_tree(&group.frame, Point::zero());
                    let node = TreeNode::Group {
                        group,
                        pos,
                        children,
                    };
                    Self::push_node(&mut root_nodes, &mut stack, node);
                }
                FrameItem::Text(text) => {
                    Self::push_node(&mut root_nodes, &mut stack, TreeNode::Text { text, pos });
                }
                FrameItem::Shape(shape, span) => {
                    Self::push_node(
                        &mut root_nodes,
                        &mut stack,
                        TreeNode::Shape { shape, span, pos },
                    );
                }
                FrameItem::Image(image, size, span) => {
                    Self::push_node(
                        &mut root_nodes,
                        &mut stack,
                        TreeNode::Image {
                            image,
                            size,
                            span,
                            pos,
                        },
                    );
                }
                FrameItem::Link(dest, size) => {
                    Self::push_node(
                        &mut root_nodes,
                        &mut stack,
                        TreeNode::Link { dest, size, pos },
                    );
                }
            }
        }

        // Close any remaining open tags at the end of the frame.
        while let Some(mut closed) = stack.pop() {
            if let Some(detached) = self.logical_child_groups.remove(&closed.loc) {
                for (gpos, grp) in detached {
                    let gchildren = self.build_frame_tree(&grp.frame, gpos);
                    closed.children.extend(gchildren);
                }
            }
            let node = TreeNode::Element {
                elem: closed.elem,
                loc: closed.loc,
                pos: closed.pos,
                children: closed.children,
            };
            Self::push_node(&mut root_nodes, &mut stack, node);
        }

        root_nodes
    }

    fn flatten_soft_groups(frame: &Frame, offset: Point, out: &mut Vec<(Point, FrameItem)>) {
        for (pos, item) in frame.items() {
            let abs_pos = offset + *pos;
            if let FrameItem::Group(group) = item {
                if group.parent.is_none()
                    && group.frame.kind() == FrameKind::Soft
                    && group.transform == Transform::identity()
                    && group.clip.is_none()
                    && group.label.is_none()
                {
                    Self::flatten_soft_groups(&group.frame, abs_pos, out);
                    continue;
                }
            }
            out.push((abs_pos, item.clone()));
        }
    }

    fn push_node(root: &mut Vec<TreeNode>, stack: &mut [OpenElement], node: TreeNode) {
        if let Some(top) = stack.last_mut() {
            top.children.push(node);
        } else {
            root.push(node);
        }
    }

    fn extract_footnote_entry_blocks(&mut self, nodes: &[TreeNode]) -> Vec<BlockNode> {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if elem.is::<FootnoteEntry>() {
                    // Skip the `PdfMarkerTag::Label` prefix child so Word doesn't duplicate
                    // the footnote superscript number inside the footnote text!
                    let filtered: Vec<TreeNode> = children
                        .iter()
                        .filter(|c| {
                            !Self::is_pdf_marker(c, |k| matches!(k, PdfMarkerTagKind::Label))
                        })
                        .cloned()
                        .collect();
                    return self.extract_blocks_from_nodes(&filtered, 0, HAlignIr::Left);
                }
            }
        }
        self.extract_blocks_from_nodes(nodes, 0, HAlignIr::Left)
    }

    fn is_pdf_marker(node: &TreeNode, pred: impl Fn(&PdfMarkerTagKind) -> bool) -> bool {
        if let TreeNode::Element { elem, .. } = node {
            if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                return pred(&marker.kind);
            }
        }
        false
    }

    // =========================================================================
    // Block-level Extraction
    // =========================================================================

    fn extract_blocks_from_nodes(
        &mut self,
        nodes: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Vec<BlockNode> {
        let mut blocks = Vec::new();
        let mut stray_inlines: Vec<TreeNode> = Vec::new();

        for node in nodes {
            if Self::is_stray_inline_node(node) {
                stray_inlines.push(node.clone());
                continue;
            }
            if !stray_inlines.is_empty() {
                self.flush_stray_inlines(&mut stray_inlines, &mut blocks, inherited_align);
            }
            self.extract_block_from_node(node, &mut blocks, list_level, inherited_align);
        }

        if !stray_inlines.is_empty() {
            self.flush_stray_inlines(&mut stray_inlines, &mut blocks, inherited_align);
        }

        blocks
    }

    fn is_stray_inline_node(node: &TreeNode) -> bool {
        match node {
            TreeNode::Text { .. } | TreeNode::Link { .. } => true,
            TreeNode::Element { elem, .. } => {
                elem.is::<StrongElem>()
                    || elem.is::<EmphElem>()
                    || elem.is::<UnderlineElem>()
                    || elem.is::<StrikeElem>()
                    || elem.is::<OverlineElem>()
                    || elem.is::<HighlightElem>()
                    || elem.is::<SubElem>()
                    || elem.is::<SuperElem>()
                    || elem.is::<SmallcapsElem>()
                    || elem.is::<LinkMarker>()
                    || elem.is::<LinkElem>()
                    || elem.is::<DirectLinkElem>()
                    || elem.is::<RefElem>()
                    || elem.is::<CiteGroup>()
                    || elem.is::<FootnoteElem>()
                    || (elem.is::<EquationElem>()
                        && !elem
                            .to_packed::<EquationElem>()
                            .unwrap()
                            .block
                            .get(StyleChain::default()))
                    || (elem.is::<RawElem>()
                        && !elem
                            .to_packed::<RawElem>()
                            .unwrap()
                            .block
                            .get(StyleChain::default()))
            }
            _ => false,
        }
    }

    fn flush_stray_inlines(
        &mut self,
        stray: &mut Vec<TreeNode>,
        blocks: &mut Vec<BlockNode>,
        align: HAlignIr,
    ) {
        let mut inlines = Vec::new();
        let mut state = ActiveInlineState::default();
        let mut last_y: Option<f64> = None;
        let mut last_x_end: Option<f64> = None;
        self.extract_inlines_from_nodes(
            stray,
            &mut inlines,
            &mut state,
            &mut last_y,
            &mut last_x_end,
        );
        stray.clear();
        if !inlines.is_empty() {
            self.report.paragraphs_count += 1;
            blocks.push(BlockNode::Paragraph {
                content: inlines,
                style: ParagraphStyleIr {
                    align,
                    ..Default::default()
                },
                bookmark: None,
            });
        }
    }

    fn extract_block_from_node(
        &mut self,
        node: &TreeNode,
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        match node {
            TreeNode::PageBreak => {
                blocks.push(BlockNode::PageBreak);
            }
            TreeNode::Group {
                group,
                pos,
                children,
            } => {
                // Check if this group is a standalone CeTZ canvas not wrapped in a LayoutElem.
                if Self::is_cetz_group_nodes(children) {
                    if let Some(canvas) = self.extract_cetz_canvas(
                        children,
                        Some(group.frame.size()),
                        inherited_align,
                    ) {
                        self.report.cetz_canvases_count += 1;
                        blocks.push(BlockNode::CetzCanvas(canvas));
                        return;
                    }
                }

                // Check if this hard group is a styled `block(...)` or `box(...)` with its own
                // background fill or border stroke (e.g. `#block(fill: luma(240), inset: 8pt)[...]`)!
                if let Some(styled_box) = self.try_extract_styled_container_group(
                    group,
                    *pos,
                    children,
                    list_level,
                    inherited_align,
                ) {
                    blocks.push(styled_box);
                    return;
                }

                let inner = self.extract_blocks_from_nodes(children, list_level, inherited_align);
                blocks.extend(inner);
            }
            TreeNode::Shape { shape, span, pos } => {
                if let Some(shape_ir) =
                    self.extract_raw_shape_item(shape, *span, *pos, Vec::new(), inherited_align)
                {
                    // Check if it's a horizontal divider line (`#line(length: 100%)`).
                    if let ShapeKindIr::Line {
                        x1_pt,
                        y1_pt,
                        x2_pt,
                        y2_pt,
                    } = shape_ir.kind
                    {
                        if (y2_pt - y1_pt).abs() < 0.5 && (x2_pt - x1_pt).abs() > 20.0 {
                            self.report.shapes_count += 1;
                            blocks.push(BlockNode::HorizontalRule {
                                length_pt: (x2_pt - x1_pt).abs(),
                                stroke: shape_ir.stroke.unwrap_or_default(),
                                align: inherited_align,
                            });
                            return;
                        }
                    }
                    self.report.shapes_count += 1;
                    blocks.push(BlockNode::ShapeBlock(shape_ir));
                }
            }
            TreeNode::Image {
                image,
                size,
                span,
                pos: _,
            } => {
                if let Some(img_ir) = self.extract_image_item(image, *size, *span, inherited_align)
                {
                    self.report.images_count += 1;
                    blocks.push(BlockNode::ImageBlock(img_ir));
                }
            }
            TreeNode::Element {
                elem,
                loc,
                pos,
                children,
            } => {
                self.extract_element_block(
                    elem,
                    *loc,
                    *pos,
                    children,
                    blocks,
                    list_level,
                    inherited_align,
                );
            }
            TreeNode::Text { .. } | TreeNode::Link { .. } => {}
        }
    }

    fn extract_element_block(
        &mut self,
        elem: &Content,
        loc: Location,
        pos: Point,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        // 1. Skip artifacts (headers, footers, page decorations) and footnote entries.
        if let Some(artifact) = elem.to_packed::<ArtifactElem>() {
            match artifact.kind.get(StyleChain::default()) {
                ArtifactKind::Header | ArtifactKind::Footer | ArtifactKind::Page => return,
                ArtifactKind::Other => {
                    // Other artifacts (like decorative shapes) can still be processed if they are shapes.
                }
            }
        }
        if elem.is::<FootnoteEntry>() {
            return;
        }

        // 2. Check for Unsupported Elements (`AttachElem`, `HtmlElem`, `FrameElem`, `SkewElem`, `RepeatElem`, `HideElem`).
        if let Some(unsupported) = self.check_unsupported_element(elem, loc) {
            blocks.push(BlockNode::UnsupportedBlock(unsupported));
            return;
        }
        if elem.is::<HideElem>() {
            // Hidden elements are intentionally invisible in Word output.
            return;
        }

        // 3. Check for CeTZ canvas (`LayoutElem` or `ContextElem` whose span is inside `@preview/cetz`).
        if (elem.is::<LayoutElem>() || elem.elem().name() == "context")
            && (is_cetz_span(elem.span()) || Self::contains_cetz_shapes_recursive(children))
        {
            if let Some(canvas) = self.extract_cetz_canvas(children, None, inherited_align) {
                self.report.cetz_canvases_count += 1;
                blocks.push(BlockNode::CetzCanvas(canvas));
                return;
            }
        }

        // 4. Title (`#title[...]`).
        if let Some(_title) = elem.to_packed::<TitleElem>() {
            let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
            let content = self.extract_inlines_flat_with_bold(children, true);
            self.report.headings_count += 1;
            blocks.push(BlockNode::Title { content, align });
            return;
        }

        // 5. Heading (`= Heading`, `== Subheading`).
        if let Some(heading) = elem.to_packed::<HeadingElem>() {
            let level = heading.resolve_level(StyleChain::default()).get();
            let align = self.infer_node_halign(children).unwrap_or(inherited_align);
            let bookmark = self.bookmarks.get(&loc).cloned();
            let (numbering, content) = self.extract_heading_numbering_and_body(heading, children);
            self.report.headings_count += 1;
            blocks.push(BlockNode::Heading {
                level,
                numbering,
                content,
                bookmark,
                align,
            });
            return;
        }

        // 6. Paragraph (`ParElem`).
        if let Some(par) = elem.to_packed::<ParElem>() {
            // Check if this paragraph is actually just a wrapper around a single CeTZ canvas or block shape!
            if children.len() == 1 {
                if let TreeNode::Group {
                    group,
                    children: gchildren,
                    ..
                } = &children[0]
                {
                    if Self::is_cetz_group_nodes(gchildren) {
                        if let Some(canvas) = self.extract_cetz_canvas(
                            gchildren,
                            Some(group.frame.size()),
                            inherited_align,
                        ) {
                            self.report.cetz_canvases_count += 1;
                            blocks.push(BlockNode::CetzCanvas(canvas));
                            return;
                        }
                    }
                }
            }

            let mut style = ParagraphStyleIr {
                align: if par.justify.get(StyleChain::default()) {
                    HAlignIr::Justify
                } else {
                    self.infer_node_halign(children).unwrap_or(inherited_align)
                },
                first_line_indent_pt: par
                    .first_line_indent
                    .get(StyleChain::default())
                    .amount
                    .abs
                    .to_pt(),
                hanging_indent_pt: par.hanging_indent.get(StyleChain::default()).abs.to_pt(),
                space_after_pt: par.spacing.get(StyleChain::default()).abs.to_pt().max(4.0),
                line_spacing_pt: Some(par.leading.get(StyleChain::default()).abs.to_pt()),
                ..Default::default()
            };
            if style.align == HAlignIr::Left && inherited_align != HAlignIr::Left {
                style.align = inherited_align;
            }

            let content = self.extract_inlines_flat(children);
            if !content.is_empty() {
                self.report.paragraphs_count += 1;
                blocks.push(BlockNode::Paragraph {
                    content,
                    style,
                    bookmark: self.bookmarks.get(&loc).cloned(),
                });
            }
            return;
        }

        // 7. Bullet List (`ListElem`).
        if let Some(list) = elem.to_packed::<ListElem>() {
            let tight = list.tight.get(StyleChain::default());
            let items = self.extract_list_items(children, list_level);
            if !items.is_empty() {
                self.report.lists_count += 1;
                blocks.push(BlockNode::List {
                    items,
                    tight,
                    level: list_level,
                });
            }
            return;
        }

        // 8. Numbered List (`EnumElem`).
        if let Some(en) = elem.to_packed::<EnumElem>() {
            let tight = en.tight.get(StyleChain::default());
            let items = self.extract_enum_items(en, children, list_level);
            if !items.is_empty() {
                self.report.lists_count += 1;
                blocks.push(BlockNode::Enum {
                    items,
                    tight,
                    level: list_level,
                });
            }
            return;
        }

        // 9. Term / Definition List (`TermsElem`).
        if let Some(terms) = elem.to_packed::<TermsElem>() {
            let tight = terms.tight.get(StyleChain::default());
            let items = self.extract_term_items(children, list_level);
            if !items.is_empty() {
                self.report.lists_count += 1;
                blocks.push(BlockNode::Terms { items, tight });
            }
            return;
        }

        // 10. Table (`TableElem`) and Grid (`GridElem`).
        if let Some(table) = elem.to_packed::<TableElem>() {
            if let Some(table_ir) = self.extract_table_or_grid(
                table.grid.as_deref(),
                true,
                table
                    .summary
                    .get_cloned(StyleChain::default())
                    .map(|s| s.to_string()),
                children,
                inherited_align,
            ) {
                self.report.tables_count += 1;
                blocks.push(BlockNode::Table(table_ir));
                return;
            }
        }
        if let Some(grid) = elem.to_packed::<GridElem>() {
            if let Some(table_ir) = self.extract_table_or_grid(
                grid.grid.as_deref(),
                false,
                None,
                children,
                inherited_align,
            ) {
                self.report.tables_count += 1;
                blocks.push(BlockNode::Table(table_ir));
                return;
            }
        }

        // 11. Block Quote (`QuoteElem`).
        if let Some(quote) = elem.to_packed::<QuoteElem>() {
            let is_block = quote.block.get(StyleChain::default());
            if is_block {
                let has_attr = quote.attribution.get_ref(StyleChain::default()).is_some();
                let mut body =
                    self.extract_blocks_from_nodes(children, list_level, inherited_align);
                let attribution = if has_attr && body.len() >= 2 {
                    match body.pop() {
                        Some(BlockNode::Paragraph { content, .. }) => Some(content),
                        Some(other) => {
                            body.push(other);
                            None
                        }
                        None => None,
                    }
                } else {
                    None
                };
                self.report.quotes_count += 1;
                blocks.push(BlockNode::Quote { body, attribution });
                return;
            }
        }

        // 12. Fenced / Block Code (`RawElem`).
        if let Some(raw) = elem.to_packed::<RawElem>() {
            if raw.block.get(StyleChain::default()) {
                let code_block = self.extract_code_block(raw, children);
                self.report.code_blocks_count += 1;
                blocks.push(code_block);
                return;
            }
        }

        // 13. Figure (`FigureElem`).
        if let Some(fig) = elem.to_packed::<FigureElem>() {
            let fig_block = self.extract_figure(fig, loc, children, list_level);
            self.report.figures_count += 1;
            blocks.push(fig_block);
            return;
        }

        // 14. Equation (`EquationElem`).
        if let Some(eq) = elem.to_packed::<EquationElem>() {
            let eq_ir = self.extract_equation(eq, loc, children);
            self.report.equations_count += 1;
            blocks.push(BlockNode::EquationBlock(eq_ir));
            return;
        }

        // 15. Native Shapes (`RectElem`, `SquareElem`, `CircleElem`, `EllipseElem`,
        //     `PolygonElem`, `CurveElem`, `PathElem`, `LineElem`, and rotated/scaled shapes).
        if elem.is::<RotateElem>() || elem.is::<ScaleElem>() || elem.is::<MoveElem>() {
            if let Some(shape_ir) =
                self.try_extract_transformed_shape(elem, pos, children, list_level, inherited_align)
            {
                self.report.shapes_count += 1;
                blocks.push(BlockNode::ShapeBlock(shape_ir));
                return;
            }
        }
        if Self::is_native_shape_elem(elem) {
            if let Some(shape_ir) = self.extract_native_shape_elem(
                elem,
                pos,
                children,
                list_level,
                inherited_align,
                0.0,
                (1.0, 1.0),
            ) {
                if let ShapeKindIr::Line {
                    x1_pt,
                    y1_pt,
                    x2_pt,
                    y2_pt,
                } = shape_ir.kind
                {
                    if (y2_pt - y1_pt).abs() < 0.5 && (x2_pt - x1_pt).abs() > 80.0 {
                        self.report.shapes_count += 1;
                        blocks.push(BlockNode::HorizontalRule {
                            length_pt: (x2_pt - x1_pt).abs(),
                            stroke: shape_ir.stroke.unwrap_or_default(),
                            align: inherited_align,
                        });
                        return;
                    }
                }
                self.report.shapes_count += 1;
                blocks.push(BlockNode::ShapeBlock(shape_ir));
                return;
            }
        }

        // 16. Image (`ImageElem`).
        if let Some(img_elem) = elem.to_packed::<typst::visualize::ImageElem>() {
            if let Some(img_ir) = self.extract_image_elem(img_elem, children, inherited_align) {
                self.report.images_count += 1;
                blocks.push(BlockNode::ImageBlock(img_ir));
                return;
            }
        }

        // 17. Outline / Table of Contents (`OutlineElem`).
        if elem.is::<OutlineElem>() {
            let outline_block = self.extract_outline(children);
            blocks.push(outline_block);
            return;
        }

        // 18. Bibliography (`BibliographyElem`).
        if elem.is::<BibliographyElem>() {
            let bib_block = self.extract_bibliography(children);
            blocks.push(bib_block);
            return;
        }

        // 19. Explicit Pagebreak / Colbreak.
        if elem.is::<PagebreakElem>() {
            blocks.push(BlockNode::PageBreak);
            return;
        }
        if elem.is::<ColbreakElem>() {
            blocks.push(BlockNode::ColumnBreak);
            return;
        }

        // 20. Generic container fallback (recurse into children).
        let child_blocks = self.extract_blocks_from_nodes(children, list_level, inherited_align);
        blocks.extend(child_blocks);
    }

    // =========================================================================
    // Unsupported Element Detection
    // =========================================================================

    fn check_unsupported_element(
        &mut self,
        elem: &Content,
        loc: Location,
    ) -> Option<UnsupportedIr> {
        let source_loc = self.compiled.source_snippet(self.world, elem.span());

        if let Some(attach) = elem.to_packed::<AttachElem>() {
            let path_str = format!("{:?}", attach.path.source);
            let item = UnsupportedIr {
                element_name: "pdf.attach".to_string(),
                reason: format!("PDF 附件嵌入 ({}) 在 Word 文档中不受支持", path_str),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        if let Some(html_elem) = elem.to_packed::<HtmlElem>() {
            let item = UnsupportedIr {
                element_name: format!("html.elem(<{}>)", html_elem.tag),
                reason: "HTML 专属标签元素无法直接转换为 Word 原生语义结构".to_string(),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        if elem.is::<FrameElem>() {
            let item = UnsupportedIr {
                element_name: "html.frame".to_string(),
                reason: "HTML frame 容器为 HTML 导出专属元素".to_string(),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        if let Some(skew) = elem.to_packed::<SkewElem>() {
            let ax = skew.ax.get(StyleChain::default()).to_deg();
            let ay = skew.ay.get(StyleChain::default()).to_deg();
            if ax.abs() > 0.01 || ay.abs() > 0.01 {
                let item = UnsupportedIr {
                    element_name: format!("skew(ax: {:.1}deg, ay: {:.1}deg)", ax, ay),
                    reason: "Word 文本与图形对象不支持 2D 错切(skew)仿射变换".to_string(),
                    source_location: source_loc,
                };
                if self.seen_unsupported_locs.insert(loc) {
                    self.report.unsupported_elements.push(item.clone());
                }
                return Some(item);
            }
        }

        if elem.is::<RepeatElem>() {
            let item = UnsupportedIr {
                element_name: "repeat".to_string(),
                reason: "动态宽度重复填充元素(repeat)在 Word 流式布局中不支持".to_string(),
                source_location: source_loc,
            };
            if self.seen_unsupported_locs.insert(loc) {
                self.report.unsupported_elements.push(item.clone());
            }
            return Some(item);
        }

        None
    }

    fn record_unsupported_paint(&mut self, context_name: &str, span: Span) -> UnsupportedIr {
        let item = UnsupportedIr {
            element_name: format!("tiling/pattern ({})", context_name),
            reason: "Word 原生图形不支持 Typst 自定义平铺图案(tiling/pattern)填充".to_string(),
            source_location: self.compiled.source_snippet(self.world, span),
        };
        self.report.unsupported_elements.push(item.clone());
        item
    }

    // =========================================================================
    // Headings, Lists, Enums, Terms
    // =========================================================================

    fn extract_heading_numbering_and_body(
        &mut self,
        heading: &Packed<HeadingElem>,
        children: &[TreeNode],
    ) -> (Option<String>, Vec<InlineNode>) {
        let all_inlines = self.extract_inlines_flat_with_bold(children, true);
        if heading.numbering.get_ref(StyleChain::default()).is_none() {
            return (None, all_inlines);
        }

        // First check synthesized `heading.numbers` if populated by Typst synthesis.
        if let Some(ref synth_eco) = heading.numbers {
            let synth_num = synth_eco.trim();
            if !synth_num.is_empty() {
                if let Some(stripped) =
                    Self::strip_leading_prefix_from_inlines(&all_inlines, synth_num)
                {
                    if !stripped.is_empty() {
                        return (Some(synth_num.to_string()), stripped);
                    }
                }
            }
        }

        // Fallback: look for the first `HorizontalSpace` (or space run after numbering text).
        for (idx, inline) in all_inlines.iter().enumerate() {
            if matches!(inline, InlineNode::HorizontalSpace { .. }) && idx > 0 {
                let prefix_str = Self::inlines_to_plain_text(&all_inlines[..idx])
                    .trim()
                    .to_string();
                let body_inlines = all_inlines[idx + 1..].to_vec();
                if !prefix_str.is_empty() && !body_inlines.is_empty() {
                    return (Some(prefix_str), body_inlines);
                }
            }
        }

        (None, all_inlines)
    }

    fn strip_leading_prefix_from_inlines(
        inlines: &[InlineNode],
        prefix: &str,
    ) -> Option<Vec<InlineNode>> {
        let mut rem_prefix = prefix;
        let mut out = Vec::new();
        let mut prefix_done = false;

        for inline in inlines {
            if prefix_done {
                if out.is_empty() && matches!(inline, InlineNode::HorizontalSpace { .. }) {
                    continue;
                }
                if out.is_empty() {
                    if let InlineNode::Text(r) = inline {
                        let trimmed = r.text.trim_start();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if trimmed.len() != r.text.len() {
                            let mut nr = r.clone();
                            nr.text = trimmed.to_string();
                            out.push(InlineNode::Text(nr));
                            continue;
                        }
                    }
                }
                out.push(inline.clone());
                continue;
            }

            match inline {
                InlineNode::Text(r) => {
                    let t = r.text.as_str();
                    if let Some(rest_run) = t.strip_prefix(rem_prefix) {
                        rem_prefix = "";
                        prefix_done = true;
                        let trimmed_rest = rest_run.trim_start();
                        if !trimmed_rest.is_empty() {
                            let mut nr = r.clone();
                            nr.text = trimmed_rest.to_string();
                            out.push(InlineNode::Text(nr));
                        }
                    } else if let Some(rest_pref) = rem_prefix.strip_prefix(t) {
                        rem_prefix = rest_pref;
                        if rem_prefix.is_empty() {
                            prefix_done = true;
                        }
                    } else {
                        return None;
                    }
                }
                InlineNode::HorizontalSpace { .. } if rem_prefix.is_empty() => {
                    prefix_done = true;
                }
                _ => return None,
            }
        }

        if prefix_done { Some(out) } else { None }
    }

    fn extract_list_items(&mut self, children: &[TreeNode], list_level: usize) -> Vec<ListItemIr> {
        let mut items = Vec::new();
        let mut current_marker = "•".to_string();

        let mut flat_markers = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut flat_markers);

        for (kind, marker_children) in flat_markers {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let txt = Self::nodes_to_plain_text(&marker_children)
                        .trim()
                        .to_string();
                    if !txt.is_empty() {
                        current_marker = txt;
                    }
                }
                PdfMarkerTagKind::ListItemBody => {
                    let blocks = self.extract_blocks_from_nodes(
                        &marker_children,
                        list_level + 1,
                        HAlignIr::Left,
                    );
                    items.push(ListItemIr {
                        marker: current_marker.clone(),
                        blocks,
                    });
                }
                _ => {}
            }
        }

        items
    }

    fn extract_enum_items(
        &mut self,
        en: &Packed<EnumElem>,
        children: &[TreeNode],
        list_level: usize,
    ) -> Vec<EnumItemIr> {
        let mut items = Vec::new();
        let mut current_marker = "1.".to_string();
        let start_num = en.start.get(StyleChain::default()).unwrap_or(1);
        let mut counter = start_num;

        let mut flat_markers = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut flat_markers);

        for (kind, marker_children) in flat_markers {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let txt = Self::nodes_to_plain_text(&marker_children)
                        .trim()
                        .to_string();
                    if !txt.is_empty() {
                        current_marker = txt;
                    }
                }
                PdfMarkerTagKind::ListItemBody => {
                    let blocks = self.extract_blocks_from_nodes(
                        &marker_children,
                        list_level + 1,
                        HAlignIr::Left,
                    );
                    let parsed_num = current_marker
                        .trim_matches(|c: char| !c.is_ascii_digit())
                        .parse::<u64>()
                        .ok()
                        .or(Some(counter));
                    items.push(EnumItemIr {
                        number: parsed_num,
                        marker_text: current_marker.clone(),
                        blocks,
                    });
                    counter += 1;
                }
                _ => {}
            }
        }

        items
    }

    fn extract_term_items(&mut self, children: &[TreeNode], list_level: usize) -> Vec<TermItemIr> {
        let mut items = Vec::new();
        let mut term_bodies = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut term_bodies);

        for (kind, body_children) in term_bodies {
            if matches!(kind, PdfMarkerTagKind::TermsItemBody) {
                let mut term_inlines = Vec::new();
                let mut desc_nodes = Vec::new();
                for child in body_children {
                    if Self::is_pdf_marker(&child, |k| {
                        matches!(k, PdfMarkerTagKind::TermsItemLabel)
                    }) {
                        if let TreeNode::Element {
                            children: lbl_children,
                            ..
                        } = &child
                        {
                            term_inlines = self.extract_inlines_flat(lbl_children);
                        }
                    } else if let TreeNode::Element { elem, .. } = &child
                        && elem.is::<ArtifactElem>()
                    {
                        // Skip the separator artifact between term and description.
                        continue;
                    } else {
                        desc_nodes.push(child);
                    }
                }
                let description =
                    self.extract_blocks_from_nodes(&desc_nodes, list_level + 1, HAlignIr::Left);
                items.push(TermItemIr {
                    term: term_inlines,
                    description,
                });
            }
        }

        items
    }

    fn collect_pdf_marker_elements(
        nodes: &[TreeNode],
        out: &mut Vec<(PdfMarkerTagKind, Vec<TreeNode>)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        out.push((marker.kind.clone(), children.clone()));
                    } else {
                        // Do not recurse into nested ListElem/EnumElem/TermsElem when collecting
                        // top-level list items!
                        if elem.is::<ListElem>() || elem.is::<EnumElem>() || elem.is::<TermsElem>()
                        {
                            continue;
                        }
                        Self::collect_pdf_marker_elements(children, out);
                    }
                }
                TreeNode::Group { children, .. } => {
                    Self::collect_pdf_marker_elements(children, out);
                }
                _ => {}
            }
        }
    }

    // =========================================================================
    // Tables & Grids (`TableElem`, `GridElem`)
    // =========================================================================

    fn extract_table_or_grid(
        &mut self,
        cell_grid: Option<&typst::layout::grid::resolve::CellGrid>,
        is_semantic_table: bool,
        summary: Option<String>,
        children: &[TreeNode],
        inherited_align: HAlignIr,
    ) -> Option<TableIr> {
        let grid = cell_grid?;
        let num_cols = grid.non_gutter_column_count();
        let num_rows = grid.non_gutter_row_count();
        if num_cols == 0 || num_rows == 0 {
            return None;
        }

        // Collect laid-out non-repeated cell nodes keyed by `(x, y)` (non-gutter indices).
        let factor = if grid.has_gutter { 2 } else { 1 };
        let mut cell_nodes_map: HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)> =
            HashMap::new();
        Self::collect_table_cell_nodes(children, factor, &mut cell_nodes_map);

        // Estimate column widths and row heights from explicit grid sizings and laid-out cell frames.
        let mut col_widths_pt = vec![0.0_f64; num_cols];
        let mut row_heights_pt = vec![0.0_f64; num_rows];
        let mut row_has_explicit_height = vec![false; num_rows];

        for x in 0..num_cols {
            let raw_x = x * factor;
            if let Some(typst::layout::Sizing::Rel(rel)) = grid.cols.get(raw_x) {
                let abs_pt = rel.abs.abs.to_pt();
                if abs_pt > 1.0 && rel.rel.is_zero() {
                    col_widths_pt[x] = abs_pt;
                }
            }
        }
        for y in 0..num_rows {
            let raw_y = y * factor;
            if let Some(typst::layout::Sizing::Rel(rel)) = grid.rows.get(raw_y) {
                let abs_pt = rel.abs.abs.to_pt();
                if abs_pt > 1.0 && rel.rel.is_zero() {
                    row_heights_pt[y] = abs_pt;
                    row_has_explicit_height[y] = true;
                }
            }
        }

        for (&(cx, cy), (cell_elem, cnodes, csize)) in &cell_nodes_map {
            let colspan = Self::get_cell_colspan(cell_elem);
            let rowspan = Self::get_cell_rowspan(cell_elem);
            let w_pt = csize
                .x
                .to_pt()
                .max(Self::estimate_nodes_bbox(cnodes).2 + 12.0);
            let h_pt = csize
                .y
                .to_pt()
                .max(Self::estimate_nodes_bbox(cnodes).3 + 8.0);
            if colspan == 1 && cx < num_cols && col_widths_pt[cx] <= 1.0 {
                col_widths_pt[cx] = col_widths_pt[cx].max(w_pt);
            }
            if rowspan == 1 && cy < num_rows && !row_has_explicit_height[cy] {
                row_heights_pt[cy] = row_heights_pt[cy].max(h_pt);
            }
        }

        // Fill in any zero-width columns with a sensible default (e.g. from colspan cells or 90pt).
        for w in &mut col_widths_pt {
            if *w <= 1.0 {
                *w = 90.0;
            }
        }

        let header_rows = grid
            .headers
            .iter()
            .map(|h| h.range.end / factor)
            .max()
            .unwrap_or(0);

        let mut rows = Vec::with_capacity(num_rows);
        for y in 0..num_rows {
            let raw_y = y * factor;
            let is_header = y < header_rows;
            let mut row_cells = Vec::with_capacity(num_cols);

            for x in 0..num_cols {
                let raw_x = x * factor;
                let Some(entry) = grid.entry(raw_x, raw_y) else {
                    continue;
                };

                match entry {
                    typst::layout::grid::resolve::Entry::Cell(resolved_cell) => {
                        let colspan = resolved_cell.colspan.get();
                        let rowspan = resolved_cell.rowspan.get();
                        let vmerge = if rowspan > 1 {
                            Some(VMergeState::Restart)
                        } else {
                            None
                        };

                        if let Some(Paint::Tiling(_)) = &resolved_cell.fill {
                            self.record_unsupported_paint(
                                "table.cell.fill",
                                resolved_cell.body.span(),
                            );
                        }
                        let fill = resolved_cell.fill.as_ref().and_then(paint_to_hex);
                        let borders = Self::extract_cell_borders(resolved_cell, is_semantic_table);

                        let (inset, halign, valign, is_header_cell, cell_blocks) =
                            if let Some((cell_elem, cnodes, _)) = cell_nodes_map.get(&(x, y)) {
                                let (ins, ha, va, ih) =
                                    Self::extract_cell_props(cell_elem, is_header);
                                let blks = self.extract_blocks_from_nodes(cnodes, 0, ha);
                                (ins, ha, va, ih, blks)
                            } else {
                                (
                                    SidesPtIr::default(),
                                    HAlignIr::Left,
                                    VAlignIr::Top,
                                    is_header,
                                    Vec::new(),
                                )
                            };

                        row_cells.push(TableCellIr {
                            x,
                            y,
                            colspan,
                            rowspan,
                            vmerge,
                            hmerged_covered: false,
                            fill,
                            borders,
                            inset,
                            halign,
                            valign,
                            is_header_cell,
                            blocks: cell_blocks,
                        });
                    }
                    typst::layout::grid::resolve::Entry::Merged { parent } => {
                        let parent_x = *parent % num_cols;
                        let parent_y = *parent / num_cols;
                        // Get parent cell to know its colspan/rowspan/fill/borders.
                        let parent_raw_x = parent_x * factor;
                        let parent_raw_y = parent_y * factor;
                        let parent_cell = grid.cell(parent_raw_x, parent_raw_y);
                        let colspan = parent_cell.map(|c| c.colspan.get()).unwrap_or(1);
                        let _rowspan = parent_cell.map(|c| c.rowspan.get()).unwrap_or(1);
                        let fill = parent_cell.and_then(|c| c.fill.as_ref().and_then(paint_to_hex));
                        let borders = parent_cell
                            .map(|c| Self::extract_cell_borders(c, is_semantic_table))
                            .unwrap_or_default();

                        // If `x > parent_x`, this slot is covered horizontally by `gridSpan` (`colspan`).
                        // If `x == parent_x && y > parent_y`, this slot starts a `<w:vMerge w:val="continue"/>` cell with `gridSpan = colspan`.
                        let hmerged_covered = x > parent_x;
                        let vmerge = if y > parent_y && x == parent_x {
                            Some(VMergeState::Continue)
                        } else {
                            None
                        };

                        row_cells.push(TableCellIr {
                            x,
                            y,
                            colspan: if x == parent_x { colspan } else { 1 },
                            rowspan: 1,
                            vmerge,
                            hmerged_covered,
                            fill,
                            borders,
                            inset: SidesPtIr::default(),
                            halign: HAlignIr::Left,
                            valign: VAlignIr::Top,
                            is_header_cell: is_header,
                            blocks: Vec::new(),
                        });
                    }
                }
            }

            rows.push(TableRowIr {
                height_pt: if row_has_explicit_height[y] && row_heights_pt[y] > 1.0 {
                    Some(row_heights_pt[y])
                } else {
                    None
                },
                is_header,
                cells: row_cells,
            });
        }

        Some(TableIr {
            is_semantic_table,
            col_widths_pt,
            row_heights_pt,
            header_rows,
            rows,
            align: inherited_align,
            summary,
        })
    }

    fn collect_table_cell_nodes(
        nodes: &[TreeNode],
        factor: usize,
        out: &mut HashMap<(usize, usize), (Content, Vec<TreeNode>, Size)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(tc) = elem.to_packed::<TableCell>() {
                        if tc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(raw_x), Smart::Custom(raw_y)) = (
                            tc.x.get(StyleChain::default()),
                            tc.y.get(StyleChain::default()),
                        ) {
                            let x = raw_x / factor;
                            let y = raw_y / factor;
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    if let Some(gc) = elem.to_packed::<GridCell>() {
                        if gc.is_repeated.get(StyleChain::default()) {
                            continue;
                        }
                        if let (Smart::Custom(raw_x), Smart::Custom(raw_y)) = (
                            gc.x.get(StyleChain::default()),
                            gc.y.get(StyleChain::default()),
                        ) {
                            let x = raw_x / factor;
                            let y = raw_y / factor;
                            let size = Self::find_group_size(children);
                            out.entry((x, y))
                                .and_modify(|(_, existing_children, _)| {
                                    existing_children.extend(children.iter().cloned());
                                })
                                .or_insert_with(|| (elem.clone(), children.clone(), size));
                        }
                        continue;
                    }
                    // Do not recurse into nested tables/grids!
                    if elem.is::<TableElem>()
                        || elem.is::<GridElem>()
                        || elem.is::<ListElem>()
                        || elem.is::<EnumElem>()
                    {
                        continue;
                    }
                    Self::collect_table_cell_nodes(children, factor, out);
                }
                TreeNode::Group {
                    group, children, ..
                } => {
                    // If the group directly wraps a single cell element, pass its size down.
                    if children.len() == 1 {
                        if let TreeNode::Element {
                            elem,
                            children: cchildren,
                            ..
                        } = &children[0]
                        {
                            if elem.is::<TableCell>() || elem.is::<GridCell>() {
                                let mut temp = HashMap::new();
                                Self::collect_table_cell_nodes(children, factor, &mut temp);
                                for ((x, y), (ce, cc, sz)) in temp {
                                    let final_sz = if sz.x.to_pt() > 0.0 {
                                        sz
                                    } else {
                                        group.frame.size()
                                    };
                                    let _ = cchildren;
                                    out.entry((x, y))
                                        .and_modify(|(_, existing_children, _)| {
                                            existing_children.extend(cc.iter().cloned());
                                        })
                                        .or_insert((ce, cc, final_sz));
                                }
                                continue;
                            }
                        }
                    }
                    Self::collect_table_cell_nodes(children, factor, out);
                }
                _ => {}
            }
        }
    }

    fn get_cell_colspan(elem: &Content) -> usize {
        if let Some(tc) = elem.to_packed::<TableCell>() {
            tc.colspan.get(StyleChain::default()).get()
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            gc.colspan.get(StyleChain::default()).get()
        } else {
            1
        }
    }

    fn get_cell_rowspan(elem: &Content) -> usize {
        if let Some(tc) = elem.to_packed::<TableCell>() {
            tc.rowspan.get(StyleChain::default()).get()
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            gc.rowspan.get(StyleChain::default()).get()
        } else {
            1
        }
    }

    fn extract_cell_borders(
        cell: &typst::layout::grid::resolve::Cell,
        is_semantic_table: bool,
    ) -> BordersIr {
        let mut borders = BordersIr {
            top: cell.stroke.top.as_deref().map(Self::abs_stroke_to_ir),
            bottom: cell.stroke.bottom.as_deref().map(Self::abs_stroke_to_ir),
            left: cell.stroke.left.as_deref().map(Self::abs_stroke_to_ir),
            right: cell.stroke.right.as_deref().map(Self::abs_stroke_to_ir),
        };
        if is_semantic_table && !borders.has_any() && !cell.stroke_overridden.iter().any(|&b| b) {
            // Default 0.5pt black table border if not explicitly overridden to `none`.
            // Note: if `cell.stroke` has `None` on all sides because user set `stroke: none`,
            // `cell.stroke` is already `None`, so we respect `cell.stroke` directly!
        }
        let _ = &mut borders;
        borders
    }

    fn extract_cell_props(
        elem: &Content,
        default_is_header: bool,
    ) -> (SidesPtIr, HAlignIr, VAlignIr, bool) {
        if let Some(tc) = elem.to_packed::<TableCell>() {
            let align = tc.align.get(StyleChain::default());
            let (ha, va) = Self::alignment_to_ir(align);
            let inset = Self::sides_rel_length_to_pt(&tc.inset.get(StyleChain::default()));
            let is_header = matches!(
                tc.kind.get(StyleChain::default()),
                Smart::Custom(TableCellKind::Header(..))
            ) || default_is_header;
            (inset, ha, va, is_header)
        } else if let Some(gc) = elem.to_packed::<GridCell>() {
            let align = gc.align.get(StyleChain::default());
            let (ha, va) = Self::alignment_to_ir(align);
            let inset = Self::sides_rel_length_to_pt(&gc.inset.get(StyleChain::default()));
            (inset, ha, va, default_is_header)
        } else {
            (
                SidesPtIr::default(),
                HAlignIr::Left,
                VAlignIr::Top,
                default_is_header,
            )
        }
    }

    // =========================================================================
    // Code Blocks, Figures, Equations, Outlines, Bibliographies
    // =========================================================================

    fn extract_code_block(&self, raw: &Packed<RawElem>, children: &[TreeNode]) -> BlockNode {
        let lang = raw
            .lang
            .get_ref(StyleChain::default())
            .as_ref()
            .map(|s| s.to_string());

        let mut lines: Vec<Vec<TextRunIr>> = Vec::new();
        Self::collect_raw_lines(children, &mut lines);

        // Fallback if no `RawLine` tags were found: use `raw.lines` or `raw.text`.
        if lines.is_empty() {
            if let Some(raw_lines) = raw.lines.as_deref() {
                for rl in raw_lines {
                    lines.push(vec![TextRunIr {
                        text: rl.text.to_string(),
                        font_family: Some("Consolas".to_string()),
                        is_code: true,
                        ..Default::default()
                    }]);
                }
            }
        }

        BlockNode::CodeBlock {
            lang,
            lines,
            fill: Some("F6F8FA".to_string()),
            stroke: Some(StrokeIr {
                color: "D0D7DE".to_string(),
                thickness_pt: 0.75,
                ..Default::default()
            }),
        }
    }

    fn collect_raw_lines(nodes: &[TreeNode], out: &mut Vec<Vec<TextRunIr>>) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(raw_line) = elem.to_packed::<RawLine>() {
                        let mut runs = Vec::new();
                        Self::collect_raw_line_runs(children, &mut runs);
                        if runs.is_empty() {
                            runs.push(TextRunIr {
                                text: raw_line.text.to_string(),
                                font_family: Some("Consolas".to_string()),
                                is_code: true,
                                ..Default::default()
                            });
                        }
                        out.push(runs);
                    } else {
                        Self::collect_raw_lines(children, out);
                    }
                }
                TreeNode::Group { children, .. } => {
                    Self::collect_raw_lines(children, out);
                }
                _ => {}
            }
        }
    }

    fn collect_raw_line_runs(nodes: &[TreeNode], runs: &mut Vec<TextRunIr>) {
        for node in nodes {
            match node {
                TreeNode::Text { text, .. } => {
                    let state = ActiveInlineState {
                        is_code: true,
                        ..Default::default()
                    };
                    runs.push(Self::text_item_to_run(text, &state));
                }
                TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                    Self::collect_raw_line_runs(children, runs);
                }
                _ => {}
            }
        }
    }

    fn extract_figure(
        &mut self,
        fig: &Packed<FigureElem>,
        loc: Location,
        children: &[TreeNode],
        list_level: usize,
    ) -> BlockNode {
        let mut body_nodes = Vec::new();
        let mut caption_nodes_opt = None;
        let mut caption_top = false;
        let mut saw_body = false;

        Self::split_figure_body_and_caption(
            children,
            &mut body_nodes,
            &mut caption_nodes_opt,
            &mut caption_top,
            &mut saw_body,
        );

        let caption_inlines =
            caption_nodes_opt.map(|cap_nodes| self.extract_inlines_flat(&cap_nodes));
        let body = self.extract_blocks_from_nodes(&body_nodes, list_level, HAlignIr::Center);
        let kind = format!("{:?}", fig.kind.get_ref(StyleChain::default()));
        let bookmark = self.bookmarks.get(&loc).cloned();

        BlockNode::Figure {
            body,
            caption: caption_inlines,
            caption_top,
            kind,
            bookmark,
        }
    }

    fn split_figure_body_and_caption(
        nodes: &[TreeNode],
        body_out: &mut Vec<TreeNode>,
        caption_out: &mut Option<Vec<TreeNode>>,
        caption_top: &mut bool,
        saw_body: &mut bool,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if elem.is::<FigureCaption>() {
                        if !*saw_body {
                            *caption_top = true;
                        }
                        *caption_out = Some(children.clone());
                    } else if elem.is::<FigureElem>() {
                        *saw_body = true;
                        body_out.push(node.clone());
                    } else {
                        let mut filtered_children = Vec::new();
                        Self::split_figure_body_and_caption(
                            children,
                            &mut filtered_children,
                            caption_out,
                            caption_top,
                            saw_body,
                        );
                        *saw_body = true;
                        body_out.push(TreeNode::Element {
                            elem: elem.clone(),
                            loc: *loc,
                            pos: *pos,
                            children: filtered_children,
                        });
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let mut filtered_children = Vec::new();
                    Self::split_figure_body_and_caption(
                        children,
                        &mut filtered_children,
                        caption_out,
                        caption_top,
                        saw_body,
                    );
                    if !filtered_children.is_empty() {
                        *saw_body = true;
                        body_out.push(TreeNode::Group {
                            group: group.clone(),
                            pos: *pos,
                            children: filtered_children,
                        });
                    }
                }
                other => {
                    *saw_body = true;
                    body_out.push(other.clone());
                }
            }
        }
    }

    fn extract_equation(
        &self,
        eq: &Packed<EquationElem>,
        loc: Location,
        children: &[TreeNode],
    ) -> EquationIr {
        let is_block = eq.block.get(StyleChain::default());
        let (omml_xml, _) = convert_equation_to_omml(eq, StyleChain::default());
        let source_text = self
            .compiled
            .source_snippet(self.world, eq.span())
            .unwrap_or_else(|| eq.body.plain_text().to_string());
        let bookmark = self.bookmarks.get(&loc).cloned();

        // Check if this block equation has a resolved equation number (`ParElem` child inside `EquationElem`).
        let numbering_text = if is_block && eq.numbering.get_ref(StyleChain::default()).is_some() {
            Self::find_equation_number_text(children)
        } else {
            None
        };

        EquationIr {
            is_block,
            omml_xml,
            source_text,
            numbering_text,
            bookmark,
        }
    }

    fn find_equation_number_text(nodes: &[TreeNode]) -> Option<String> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<ParElem>() {
                        let txt = Self::nodes_to_plain_text(children).trim().to_string();
                        if !txt.is_empty() {
                            return Some(txt);
                        }
                    }
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn extract_outline(&mut self, children: &[TreeNode]) -> BlockNode {
        let mut title = None;
        let mut entries = Vec::new();

        for child in children {
            if let TreeNode::Element {
                elem,
                children: cchildren,
                ..
            } = child
            {
                if elem.is::<HeadingElem>() || elem.is::<TitleElem>() {
                    title = Some(self.extract_inlines_flat_with_bold(cchildren, true));
                    continue;
                }
            }
            self.collect_outline_entries(child, &mut entries);
        }

        BlockNode::Outline { title, entries }
    }

    fn collect_outline_entries(&mut self, node: &TreeNode, out: &mut Vec<OutlineEntryIr>) {
        match node {
            TreeNode::Element { elem, children, .. } => {
                if let Some(entry) = elem.to_packed::<OutlineEntry>() {
                    let level = entry.level.get();
                    let target_bookmark = entry
                        .element_location()
                        .ok()
                        .and_then(|loc| self.bookmarks.get(&loc).cloned());
                    let all_text = Self::nodes_to_plain_text(children).replace('\u{2060}', "");
                    let page = all_text
                        .split_whitespace()
                        .last()
                        .unwrap_or("1")
                        .to_string();

                    let prefix_str = Self::find_outline_entry_prefix(children);
                    let filtered_nodes = Self::filter_outline_entry_body_nodes(children);
                    let raw_inlines = self.extract_inlines_flat(&filtered_nodes);
                    let mut body_inlines = Self::unwrap_link_inlines(raw_inlines);
                    Self::strip_trailing_outline_page(&mut body_inlines, &page);

                    if body_inlines.is_empty() {
                        let fallback_title = entry
                            .body()
                            .ok()
                            .map(|c| c.plain_text().to_string())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| entry.element.plain_text().to_string());
                        body_inlines.push(InlineNode::Text(TextRunIr {
                            text: fallback_title,
                            font_family: Some("Heiti SC".to_string()),
                            font_size_pt: Some(11.0),
                            ..Default::default()
                        }));
                    }

                    let mut title_inlines = Vec::new();
                    if let Some(ref pref) = prefix_str {
                        let base_sz = body_inlines
                            .iter()
                            .find_map(|inl| {
                                if let InlineNode::Text(r) = inl {
                                    r.font_size_pt
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(11.0);
                        title_inlines.push(InlineNode::Text(TextRunIr {
                            text: format!("{pref}  "),
                            font_family: Some("Libertinus Serif".to_string()),
                            font_size_pt: Some(base_sz),
                            ..Default::default()
                        }));
                    }
                    title_inlines.extend(body_inlines);

                    out.push(OutlineEntryIr {
                        level,
                        prefix: prefix_str,
                        title: title_inlines,
                        page,
                        target_bookmark,
                    });
                    return;
                }
                for child in children {
                    self.collect_outline_entries(child, out);
                }
            }
            TreeNode::Group { children, .. } => {
                for child in children {
                    self.collect_outline_entries(child, out);
                }
            }
            _ => {}
        }
    }

    fn find_outline_entry_prefix(nodes: &[TreeNode]) -> Option<String> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        if matches!(marker.kind, PdfMarkerTagKind::Label) {
                            let txt = Self::nodes_to_plain_text(children)
                                .replace('\u{2060}', "")
                                .trim()
                                .to_string();
                            if !txt.is_empty() {
                                return Some(txt);
                            }
                        }
                    }
                    if let Some(found) = Self::find_outline_entry_prefix(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(found) = Self::find_outline_entry_prefix(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn filter_outline_entry_body_nodes(nodes: &[TreeNode]) -> Vec<TreeNode> {
        let mut out = Vec::new();
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        if matches!(marker.kind, PdfMarkerTagKind::Label) {
                            continue;
                        }
                    }
                    if elem.is::<RepeatElem>() || Self::contains_repeat_elem(children) {
                        continue;
                    }
                    out.push(TreeNode::Element {
                        elem: elem.clone(),
                        loc: *loc,
                        pos: *pos,
                        children: Self::filter_outline_entry_body_nodes(children),
                    });
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if Self::contains_repeat_elem(children) {
                        continue;
                    }
                    out.push(TreeNode::Group {
                        group: group.clone(),
                        pos: *pos,
                        children: Self::filter_outline_entry_body_nodes(children),
                    });
                }
                other => out.push(other.clone()),
            }
        }
        out
    }

    fn contains_repeat_elem(nodes: &[TreeNode]) -> bool {
        nodes.iter().any(|n| match n {
            TreeNode::Element { elem, children, .. } => {
                elem.is::<RepeatElem>() || Self::contains_repeat_elem(children)
            }
            TreeNode::Group { children, .. } => Self::contains_repeat_elem(children),
            _ => false,
        })
    }

    fn strip_trailing_outline_page(inlines: &mut Vec<InlineNode>, page: &str) {
        while let Some(last) = inlines.last_mut() {
            match last {
                InlineNode::HorizontalSpace { .. } => {
                    inlines.pop();
                }
                InlineNode::Text(r) => {
                    let cleaned = r.text.replace('\u{2060}', "");
                    let trimmed_end = cleaned.trim_end();
                    if trimmed_end.is_empty() || trimmed_end == page {
                        inlines.pop();
                    } else if let Some(before_page) = trimmed_end.strip_suffix(page) {
                        let stripped = before_page.trim_end();
                        if stripped.is_empty() {
                            inlines.pop();
                        } else {
                            r.text = stripped.to_string();
                            break;
                        }
                    } else {
                        r.text = trimmed_end.to_string();
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn extract_bibliography(&mut self, children: &[TreeNode]) -> BlockNode {
        let mut title = None;
        let mut entries = Vec::new();

        for child in children {
            if let TreeNode::Element {
                elem,
                children: cchildren,
                ..
            } = child
            {
                if elem.is::<HeadingElem>() || elem.is::<TitleElem>() {
                    title = Some(self.extract_inlines_flat(cchildren));
                    continue;
                }
            }
        }

        let mut marker_items = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut marker_items);
        let mut pending_prefix: Option<String> = None;

        for (kind, mchildren) in marker_items {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let txt = Self::nodes_to_plain_text(&mchildren).trim().to_string();
                    if !txt.is_empty() {
                        pending_prefix = Some(txt);
                    }
                }
                PdfMarkerTagKind::BibEntry => {
                    let content = self.extract_inlines_flat(&mchildren);
                    entries.push(BibEntryIr {
                        prefix: pending_prefix.take(),
                        content,
                        bookmark: None,
                    });
                }
                PdfMarkerTagKind::Bibliography(_) => {
                    let mut inner_markers = Vec::new();
                    Self::collect_pdf_marker_elements(&mchildren, &mut inner_markers);
                    for (ikind, ichildren) in inner_markers {
                        match ikind {
                            PdfMarkerTagKind::ListItemLabel => {
                                let txt = Self::nodes_to_plain_text(&ichildren).trim().to_string();
                                if !txt.is_empty() {
                                    pending_prefix = Some(txt);
                                }
                            }
                            PdfMarkerTagKind::BibEntry => {
                                let content = self.extract_inlines_flat(&ichildren);
                                entries.push(BibEntryIr {
                                    prefix: pending_prefix.take(),
                                    content,
                                    bookmark: None,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        BlockNode::Bibliography { title, entries }
    }

    // =========================================================================
    // Native Shapes (`RectElem`, `SquareElem`, `CircleElem`, `EllipseElem`,
    // `LineElem`, `PolygonElem`, `CurveElem`, `PathElem`) & CeTZ Canvases
    // =========================================================================

    fn is_native_shape_elem(elem: &Content) -> bool {
        elem.is::<RectElem>()
            || elem.is::<SquareElem>()
            || elem.is::<CircleElem>()
            || elem.is::<EllipseElem>()
            || elem.is::<PolygonElem>()
            || elem.is::<CurveElem>()
            || elem.is::<PathElem>()
            || elem.is::<LineElem>()
    }

    fn try_extract_transformed_shape(
        &mut self,
        elem: &Content,
        pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<ShapeIr> {
        let mut rot_deg = 0.0;
        let mut scale = (1.0, 1.0);

        if let Some(rot) = elem.to_packed::<RotateElem>() {
            rot_deg = rot.angle.get(StyleChain::default()).to_deg();
        }
        if let Some(sc) = elem.to_packed::<ScaleElem>() {
            if let Smart::Custom(ScaleAmount::Ratio(rx)) = sc.x.get(StyleChain::default()) {
                scale.0 = rx.get();
            }
            if let Smart::Custom(ScaleAmount::Ratio(ry)) = sc.y.get(StyleChain::default()) {
                scale.1 = ry.get();
            }
        }

        // Look for an inner shape element inside `children`.
        for child in children {
            match child {
                TreeNode::Element {
                    elem: inner_elem,
                    pos: inner_pos,
                    children: inner_children,
                    ..
                } if Self::is_native_shape_elem(inner_elem) => {
                    return self.extract_native_shape_elem(
                        inner_elem,
                        *inner_pos,
                        inner_children,
                        list_level,
                        inherited_align,
                        rot_deg,
                        scale,
                    );
                }
                TreeNode::Group {
                    children: gchildren,
                    ..
                } => {
                    for gchild in gchildren {
                        if let TreeNode::Element {
                            elem: inner_elem,
                            pos: inner_pos,
                            children: inner_children,
                            ..
                        } = gchild
                            && Self::is_native_shape_elem(inner_elem)
                        {
                            return self.extract_native_shape_elem(
                                inner_elem,
                                *inner_pos,
                                inner_children,
                                list_level,
                                inherited_align,
                                rot_deg,
                                scale,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        let _ = pos;
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn extract_native_shape_elem(
        &mut self,
        elem: &Content,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
        rotation_deg: f64,
        scale: (f64, f64),
    ) -> Option<ShapeIr> {
        // Collect all direct `Shape` items (background fill & border strokes) and any inner content nodes.
        let mut shape_items: Vec<(Point, Shape, Span)> = Vec::new();
        let mut body_tree_nodes: Vec<TreeNode> = Vec::new();
        let mut group_size: Option<Size> = None;

        Self::partition_shape_and_body_nodes(
            children,
            Point::zero(),
            &mut shape_items,
            &mut body_tree_nodes,
            &mut group_size,
        );

        let body_blocks =
            self.extract_blocks_from_nodes(&body_tree_nodes, list_level, HAlignIr::Center);

        // Check for unsupported tiling paint on the element or its shapes.
        for (_, sh, sp) in &shape_items {
            if let Some(Paint::Tiling(_)) = &sh.fill {
                let unsup = self.record_unsupported_paint(elem.elem().name(), *sp);
                let mut body = body_blocks;
                body.insert(0, BlockNode::UnsupportedBlock(unsup));
                return Some(ShapeIr {
                    kind: ShapeKindIr::Rect,
                    width_pt: group_size.map(|s| s.x.to_pt()).unwrap_or(100.0),
                    height_pt: group_size.map(|s| s.y.to_pt()).unwrap_or(36.0),
                    fill: Some("FFF5F5".to_string()),
                    fill_rule_even_odd: false,
                    stroke: Some(StrokeIr {
                        color: "D32F2F".to_string(),
                        thickness_pt: 1.0,
                        dash: DashPatternIr::Dashed,
                        ..Default::default()
                    }),
                    borders: BordersIr::default(),
                    corner_radius_pt: Some(4.0),
                    inset: SidesPtIr::default(),
                    rotation_deg,
                    scale_x: scale.0,
                    scale_y: scale.1,
                    body,
                    align: inherited_align,
                });
            }
        }

        // Determine shape dimensions from `group_size` or `shape_items` bounding box.
        let (bbox_x, bbox_y, bbox_w, bbox_h) = Self::shapes_bbox(&shape_items);
        let width_pt = group_size
            .map(|s| s.x.to_pt())
            .unwrap_or(bbox_w)
            .max(bbox_w);
        let height_pt = group_size
            .map(|s| s.y.to_pt())
            .unwrap_or(bbox_h)
            .max(bbox_h);

        let fill = shape_items
            .iter()
            .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
        let fill_rule_even_odd = shape_items
            .first()
            .is_some_and(|(_, s, _)| s.fill_rule == FillRule::EvenOdd);
        let stroke = shape_items
            .iter()
            .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));

        // Classify shape kind from the Typst element type.
        let (kind, corner_radius_pt, inset) = if let Some(rect) = elem.to_packed::<RectElem>() {
            let rad = Self::corners_radius_to_pt(
                &rect.radius.get(StyleChain::default()),
                width_pt.min(height_pt),
            );
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(rect.inset.get(StyleChain::default())));
            (ShapeKindIr::Rect, rad, ins)
        } else if let Some(sq) = elem.to_packed::<SquareElem>() {
            let rad = Self::corners_radius_to_pt(
                &sq.radius.get(StyleChain::default()),
                width_pt.min(height_pt),
            );
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(sq.inset.get(StyleChain::default())));
            (ShapeKindIr::Rect, rad, ins)
        } else if let Some(cir) = elem.to_packed::<CircleElem>() {
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(cir.inset.get(StyleChain::default())));
            (ShapeKindIr::Ellipse, None, ins)
        } else if let Some(ell) = elem.to_packed::<EllipseElem>() {
            let ins =
                Self::sides_rel_length_to_pt(&Smart::Custom(ell.inset.get(StyleChain::default())));
            (ShapeKindIr::Ellipse, None, ins)
        } else if elem.is::<LineElem>() {
            if let Some((
                lpos,
                Shape {
                    geometry: Geometry::Line(end),
                    ..
                },
                _,
            )) = shape_items.first()
            {
                let x1 = lpos.x.to_pt() - bbox_x;
                let y1 = lpos.y.to_pt() - bbox_y;
                let x2 = x1 + end.x.to_pt();
                let y2 = y1 + end.y.to_pt();
                (
                    ShapeKindIr::Line {
                        x1_pt: x1,
                        y1_pt: y1,
                        x2_pt: x2,
                        y2_pt: y2,
                    },
                    None,
                    SidesPtIr::default(),
                )
            } else {
                (
                    ShapeKindIr::Line {
                        x1_pt: 0.0,
                        y1_pt: 0.0,
                        x2_pt: width_pt.max(30.0),
                        y2_pt: 0.0,
                    },
                    None,
                    SidesPtIr::default(),
                )
            }
        } else if elem.is::<PolygonElem>() {
            let mut pts = Vec::new();
            if let Some((
                spos,
                Shape {
                    geometry: Geometry::Curve(curve),
                    ..
                },
                _,
            )) = shape_items.first()
            {
                for item in &curve.0 {
                    match item {
                        CurveItem::Move(p) | CurveItem::Line(p) => {
                            pts.push((
                                (spos.x + p.x).to_pt() - bbox_x,
                                (spos.y + p.y).to_pt() - bbox_y,
                            ));
                        }
                        _ => {}
                    }
                }
            }
            (
                ShapeKindIr::Polygon { points_pt: pts },
                None,
                SidesPtIr::default(),
            )
        } else {
            // `CurveElem` or `PathElem`
            let mut segments = Vec::new();
            let off_x = bbox_x;
            let off_y = bbox_y;
            for (spos, sh, _) in &shape_items {
                Self::geometry_to_curve_segments(&sh.geometry, *spos, off_x, off_y, &mut segments);
            }
            (
                ShapeKindIr::CustomPath { segments },
                None,
                SidesPtIr::default(),
            )
        };

        Some(ShapeIr {
            kind,
            width_pt: width_pt.max(1.0),
            height_pt: height_pt.max(1.0),
            fill,
            fill_rule_even_odd,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt,
            inset,
            rotation_deg,
            scale_x: scale.0,
            scale_y: scale.1,
            body: body_blocks,
            align: inherited_align,
        })
    }

    fn partition_shape_and_body_nodes(
        nodes: &[TreeNode],
        offset: Point,
        shape_items: &mut Vec<(Point, Shape, Span)>,
        body_nodes: &mut Vec<TreeNode>,
        group_size: &mut Option<Size>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Shape { shape, span, pos } => {
                    shape_items.push((offset + *pos, shape.clone(), *span));
                }
                TreeNode::Element {
                    elem,
                    children,
                    pos,
                    ..
                } => {
                    // `fill_and_stroke` wraps shape borders/backgrounds in `ArtifactElem(Other)`.
                    if elem.is::<ArtifactElem>() {
                        Self::partition_shape_and_body_nodes(
                            children,
                            offset + *pos,
                            shape_items,
                            body_nodes,
                            group_size,
                        );
                    } else {
                        body_nodes.push(node.clone());
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if group_size.is_none() {
                        *group_size = Some(group.frame.size());
                    }
                    // If this group is the direct container of the shape (has background `Shape` items),
                    // partition its children; otherwise treat it as body content.
                    if children.iter().any(|c| {
                        matches!(c, TreeNode::Shape { .. })
                            || matches!(c, TreeNode::Element { elem, .. } if elem.is::<ArtifactElem>())
                    }) && shape_items.is_empty()
                    {
                        Self::partition_shape_and_body_nodes(
                            children,
                            offset + *pos,
                            shape_items,
                            body_nodes,
                            group_size,
                        );
                    } else {
                        body_nodes.push(node.clone());
                    }
                }
                other => {
                    body_nodes.push(other.clone());
                }
            }
        }
    }

    fn try_extract_styled_container_group(
        &mut self,
        group: &GroupItem,
        _pos: Point,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        // Check if this group starts with background/border `Shape` items (e.g. `#block(fill: ..., stroke: ...)[...]`)
        // AND is not a table/grid/cetz canvas.
        if Self::contains_table_or_list_elem(children) || Self::is_cetz_group_nodes(children) {
            return None;
        }

        let mut bg_shapes = Vec::new();
        let mut content_nodes = Vec::new();
        for child in children {
            match child {
                TreeNode::Shape { shape, span, pos } => {
                    bg_shapes.push((*pos, shape.clone(), *span));
                }
                TreeNode::Element {
                    elem,
                    children: achildren,
                    pos,
                    ..
                } if elem.is::<ArtifactElem>() => {
                    for ac in achildren {
                        if let TreeNode::Shape {
                            shape,
                            span,
                            pos: spos,
                        } = ac
                        {
                            bg_shapes.push((*pos + *spos, shape.clone(), *span));
                        }
                    }
                }
                other => content_nodes.push(other.clone()),
            }
        }

        if bg_shapes.is_empty() || content_nodes.is_empty() {
            return None;
        }

        let fill = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.fill.as_ref().and_then(paint_to_hex));
        let stroke = bg_shapes
            .iter()
            .find_map(|(_, s, _)| s.stroke.as_ref().map(Self::fixed_stroke_to_ir));
        if fill.is_none() && stroke.is_none() {
            return None;
        }

        let body_blocks =
            self.extract_blocks_from_nodes(&content_nodes, list_level, inherited_align);
        if body_blocks.is_empty() {
            return None;
        }

        let size = group.frame.size();
        self.report.shapes_count += 1;
        Some(BlockNode::ShapeBlock(ShapeIr {
            kind: ShapeKindIr::Rect,
            width_pt: size.x.to_pt().max(40.0),
            height_pt: size.y.to_pt().max(20.0),
            fill,
            fill_rule_even_odd: false,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt: None,
            inset: SidesPtIr {
                top: 6.0,
                bottom: 6.0,
                left: 8.0,
                right: 8.0,
            },
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: body_blocks,
            align: inherited_align,
        }))
    }

    fn contains_table_or_list_elem(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if elem.is::<TableElem>()
                    || elem.is::<GridElem>()
                    || elem.is::<TableCell>()
                    || elem.is::<GridCell>()
                    || elem.is::<ListElem>()
                    || elem.is::<EnumElem>()
                    || elem.is::<RawElem>()
                {
                    return true;
                }
                if Self::contains_table_or_list_elem(children) {
                    return true;
                }
            }
        }
        false
    }

    fn extract_raw_shape_item(
        &mut self,
        shape: &Shape,
        span: Span,
        pos: Point,
        body: Vec<BlockNode>,
        align: HAlignIr,
    ) -> Option<ShapeIr> {
        if let Some(Paint::Tiling(_)) = &shape.fill {
            self.record_unsupported_paint("shape.fill", span);
        }
        let fill = shape.fill.as_ref().and_then(paint_to_hex);
        let stroke = shape.stroke.as_ref().map(Self::fixed_stroke_to_ir);
        let bbox = shape.geometry.bbox();
        let width_pt = bbox.size().x.to_pt().abs().max(1.0);
        let height_pt = bbox.size().y.to_pt().abs().max(1.0);

        let kind = match &shape.geometry {
            Geometry::Rect(_) => ShapeKindIr::Rect,
            Geometry::Line(end) => ShapeKindIr::Line {
                x1_pt: pos.x.to_pt(),
                y1_pt: pos.y.to_pt(),
                x2_pt: pos.x.to_pt() + end.x.to_pt(),
                y2_pt: pos.y.to_pt() + end.y.to_pt(),
            },
            Geometry::Curve(_) => {
                let mut segments = Vec::new();
                Self::geometry_to_curve_segments(
                    &shape.geometry,
                    Point::zero(),
                    bbox.min.x.to_pt().min(0.0),
                    bbox.min.y.to_pt().min(0.0),
                    &mut segments,
                );
                ShapeKindIr::CustomPath { segments }
            }
        };

        Some(ShapeIr {
            kind,
            width_pt,
            height_pt,
            fill,
            fill_rule_even_odd: shape.fill_rule == FillRule::EvenOdd,
            stroke: stroke.clone(),
            borders: BordersIr::splat(stroke),
            corner_radius_pt: None,
            inset: SidesPtIr::default(),
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body,
            align,
        })
    }

    // =========================================================================
    // CeTZ Canvas Extraction (`cetz.canvas(...)`)
    // =========================================================================

    fn contains_cetz_shapes_recursive(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            match node {
                TreeNode::Shape { span, .. } if is_cetz_span(*span) => return true,
                TreeNode::Element { elem, children, .. } => {
                    if is_cetz_span(elem.span()) && elem.is::<CurveElem>() {
                        return true;
                    }
                    if Self::contains_cetz_shapes_recursive(children) {
                        return true;
                    }
                }
                TreeNode::Group { children, .. } => {
                    if Self::contains_cetz_shapes_recursive(children) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn is_cetz_group_nodes(nodes: &[TreeNode]) -> bool {
        // A group is a CeTZ canvas box only if it directly contains CeTZ `Shape` items
        // and is not an outer multi-column/flow container holding `HeadingElem`, `ColbreakElem`,
        // `LayoutElem`, or `context`.
        let has_direct_cetz_shape = nodes
            .iter()
            .any(|n| matches!(n, TreeNode::Shape { span, .. } if is_cetz_span(*span)));
        if !has_direct_cetz_shape {
            return false;
        }
        let has_flow_elements = nodes.iter().any(|n| match n {
            TreeNode::Element { elem, .. } => {
                elem.is::<HeadingElem>()
                    || elem.is::<ColbreakElem>()
                    || elem.is::<PagebreakElem>()
                    || elem.is::<LayoutElem>()
                    || elem.elem().name() == "context"
            }
            _ => false,
        });
        !has_flow_elements
    }

    fn extract_cetz_canvas(
        &mut self,
        nodes: &[TreeNode],
        explicit_box_size: Option<Size>,
        align: HAlignIr,
    ) -> Option<CetzCanvasIr> {
        // Locate the inner `box(width: width, height: height, ...)` group if `nodes` is the outer `LayoutElem`.
        if explicit_box_size.is_none() {
            if let Some((box_size, box_children)) = Self::find_cetz_box_group(nodes) {
                return self.extract_cetz_canvas(&box_children, Some(box_size), align);
            }
        }

        let mut paths: Vec<CetzPathDrawableIr> = Vec::new();
        let mut labels: Vec<CetzLabelDrawableIr> = Vec::new();
        let mut background: Option<ColorHex> = None;

        self.collect_cetz_drawables(
            nodes,
            Point::zero(),
            0.0,
            explicit_box_size,
            &mut background,
            &mut paths,
            &mut labels,
        );

        if paths.is_empty() && labels.is_empty() {
            return None;
        }

        // Normalize coordinates if the CeTZ box was soft-flattened into page coordinates.
        let min_px = paths
            .iter()
            .map(|p| p.x_pt)
            .chain(labels.iter().map(|l| l.x_pt))
            .fold(f64::INFINITY, f64::min);
        let min_py = paths
            .iter()
            .map(|p| p.y_pt)
            .chain(labels.iter().map(|l| l.y_pt))
            .fold(f64::INFINITY, f64::min);
        if min_px.is_finite() && min_py.is_finite() {
            let should_shift = match explicit_box_size {
                Some(sz) => min_px > sz.x.to_pt() * 0.4 || min_py > sz.y.to_pt() * 0.4,
                None => min_px > 15.0 || min_py > 15.0,
            };
            if should_shift {
                for p in &mut paths {
                    p.x_pt = (p.x_pt - min_px).max(0.0);
                    p.y_pt = (p.y_pt - min_py).max(0.0);
                }
                for l in &mut labels {
                    l.x_pt = (l.x_pt - min_px).max(0.0);
                    l.y_pt = (l.y_pt - min_py).max(0.0);
                }
            }
        }

        let (width_pt, height_pt) = if let Some(sz) = explicit_box_size {
            (sz.x.to_pt().max(10.0), sz.y.to_pt().max(10.0))
        } else {
            let max_px = paths
                .iter()
                .map(|p| p.x_pt + p.width_pt)
                .fold(10.0_f64, f64::max);
            let max_py = paths
                .iter()
                .map(|p| p.y_pt + p.height_pt)
                .fold(10.0_f64, f64::max);
            let max_lx = labels
                .iter()
                .map(|l| l.x_pt + l.width_pt)
                .fold(max_px, f64::max);
            let max_ly = labels
                .iter()
                .map(|l| l.y_pt + l.height_pt)
                .fold(max_py, f64::max);
            (max_lx, max_ly)
        };

        Some(CetzCanvasIr {
            width_pt,
            height_pt,
            background,
            paths,
            labels,
            svg_fallback: None,
            align,
        })
    }

    fn find_cetz_box_group(nodes: &[TreeNode]) -> Option<(Size, Vec<TreeNode>)> {
        for node in nodes {
            match node {
                TreeNode::Group {
                    group, children, ..
                } => {
                    if Self::is_cetz_group_nodes(children) {
                        return Some((group.frame.size(), children.clone()));
                    }
                    if let Some(found) = Self::find_cetz_box_group(children) {
                        return Some(found);
                    }
                }
                TreeNode::Element { children, .. } => {
                    if let Some(found) = Self::find_cetz_box_group(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_cetz_drawables(
        &mut self,
        nodes: &[TreeNode],
        offset: Point,
        rotation_deg: f64,
        box_size: Option<Size>,
        background: &mut Option<ColorHex>,
        paths: &mut Vec<CetzPathDrawableIr>,
        labels: &mut Vec<CetzLabelDrawableIr>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Shape { shape, pos, .. } => {
                    let abs_pos = offset + *pos;
                    // Check if this is the canvas background rectangle (`Geometry::Rect` matching `box_size`).
                    if let Geometry::Rect(rect_sz) = shape.geometry {
                        if let Some(bsz) = box_size {
                            if (rect_sz.x - bsz.x).abs() < Abs::pt(0.5)
                                && (rect_sz.y - bsz.y).abs() < Abs::pt(0.5)
                                && shape.stroke.is_none()
                                && background.is_none()
                            {
                                *background = shape.fill.as_ref().and_then(paint_to_hex);
                                continue;
                            }
                        }
                    }

                    let bbox = shape.geometry.bbox();
                    let min_x = bbox.min.x.to_pt().min(0.0);
                    let min_y = bbox.min.y.to_pt().min(0.0);
                    let w_pt = (bbox.max.x.to_pt() - min_x).max(0.5);
                    let h_pt = (bbox.max.y.to_pt() - min_y).max(0.5);

                    let mut segments = Vec::new();
                    Self::geometry_to_curve_segments(
                        &shape.geometry,
                        Point::zero(),
                        min_x,
                        min_y,
                        &mut segments,
                    );

                    if !segments.is_empty() {
                        paths.push(CetzPathDrawableIr {
                            x_pt: abs_pos.x.to_pt() + min_x,
                            y_pt: abs_pos.y.to_pt() + min_y,
                            width_pt: w_pt,
                            height_pt: h_pt,
                            segments,
                            fill: shape.fill.as_ref().and_then(paint_to_hex),
                            fill_rule_even_odd: shape.fill_rule == FillRule::EvenOdd,
                            stroke: shape.stroke.as_ref().map(Self::fixed_stroke_to_ir),
                        });
                    }
                }
                TreeNode::Element { elem, children, .. } => {
                    let mut next_rot = rotation_deg;
                    if let Some(rot) = elem.to_packed::<RotateElem>() {
                        next_rot += rot.angle.get(StyleChain::default()).to_deg();
                    }

                    // If this element is an inline equation `$...$` or paragraph inside `cetz.draw.content`,
                    // extract it as a positioned `CetzLabelDrawableIr`!
                    if elem.is::<EquationElem>() || elem.is::<ParElem>() {
                        if !Self::contains_cetz_shapes_recursive(children) {
                            let inlines = if let Some(eq) = elem.to_packed::<EquationElem>() {
                                vec![InlineNode::EquationInline(self.extract_equation(
                                    eq,
                                    elem.location().unwrap_or_else(|| Location::new(0)),
                                    children,
                                ))]
                            } else {
                                self.extract_inlines_flat(children)
                            };
                            if !inlines.is_empty() {
                                let (bx, by, bw, bh) = Self::estimate_nodes_bbox(children);
                                let lx = offset.x.to_pt() + bx;
                                let ly = offset.y.to_pt() + by;
                                labels.push(CetzLabelDrawableIr {
                                    x_pt: lx,
                                    y_pt: ly,
                                    width_pt: bw.max(18.0),
                                    height_pt: bh.max(14.0),
                                    rotation_deg: next_rot,
                                    content: inlines,
                                });
                            }
                            continue;
                        }
                    }

                    self.collect_cetz_drawables(
                        children, offset, next_rot, box_size, background, paths, labels,
                    );
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let abs_pos =
                        offset + *pos + Point::new(group.transform.tx, group.transform.ty);
                    // Check if this group is a placed label group (contains text/equations and NO CeTZ curves).
                    if !Self::contains_cetz_shapes_recursive(children) {
                        let inlines = self.extract_inlines_flat(children);
                        if !inlines.is_empty() {
                            let gsize = group.frame.size();
                            labels.push(CetzLabelDrawableIr {
                                x_pt: abs_pos.x.to_pt(),
                                y_pt: abs_pos.y.to_pt(),
                                width_pt: gsize.x.to_pt().max(18.0),
                                height_pt: gsize.y.to_pt().max(14.0),
                                rotation_deg,
                                content: inlines,
                            });
                            continue;
                        }
                    }
                    self.collect_cetz_drawables(
                        children,
                        abs_pos,
                        rotation_deg,
                        box_size,
                        background,
                        paths,
                        labels,
                    );
                }
                TreeNode::Text { text, pos } => {
                    let abs_pos = offset + *pos;
                    let run = Self::text_item_to_run(text, &ActiveInlineState::default());
                    if !run.text.trim().is_empty() {
                        labels.push(CetzLabelDrawableIr {
                            x_pt: abs_pos.x.to_pt(),
                            y_pt: (abs_pos.y - text.size).to_pt().max(0.0),
                            width_pt: text.width().to_pt().max(16.0),
                            height_pt: (text.size.to_pt() * 1.3).max(12.0),
                            rotation_deg,
                            content: vec![InlineNode::Text(run)],
                        });
                    }
                }
                _ => {}
            }
        }
    }

    // =========================================================================
    // Image Extraction
    // =========================================================================

    fn extract_image_elem(
        &self,
        _img_elem: &Packed<typst::visualize::ImageElem>,
        children: &[TreeNode],
        align: HAlignIr,
    ) -> Option<ImageIr> {
        Self::find_image_in_nodes(children)
            .and_then(|(img, sz, sp)| self.extract_image_item(&img, sz, sp, align))
    }

    fn find_image_in_nodes(nodes: &[TreeNode]) -> Option<(Image, Size, Span)> {
        for node in nodes {
            match node {
                TreeNode::Image {
                    image, size, span, ..
                } => return Some((image.clone(), *size, *span)),
                TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                    if let Some(found) = Self::find_image_in_nodes(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn extract_image_item(
        &self,
        image: &Image,
        size: Size,
        span: Span,
        align: HAlignIr,
    ) -> Option<ImageIr> {
        let width_pt = size.x.to_pt().max(1.0);
        let height_pt = size.y.to_pt().max(1.0);
        let alt = image.alt().map(|s| s.to_string());

        let (data, format) = match image.kind() {
            ImageKind::Raster(raster) => match raster.format() {
                RasterFormat::Exchange(ExchangeFormat::Png) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Png)
                }
                RasterFormat::Exchange(ExchangeFormat::Jpg) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Jpg)
                }
                RasterFormat::Exchange(ExchangeFormat::Gif) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Gif)
                }
                _ => {
                    // Render WebP or raw pixel format via `typst_svg::svg_frame` into SVG.
                    let mut f = Frame::hard(size);
                    f.push(Point::zero(), FrameItem::Image(image.clone(), size, span));
                    (typst_svg::svg_frame(&f).into_bytes(), ImageFormatIr::Svg)
                }
            },
            ImageKind::Svg(svg) => (svg.data().as_slice().to_vec(), ImageFormatIr::Svg),
            ImageKind::Pdf(_) => {
                let mut f = Frame::hard(size);
                f.push(Point::zero(), FrameItem::Image(image.clone(), size, span));
                (typst_svg::svg_frame(&f).into_bytes(), ImageFormatIr::Svg)
            }
        };

        Some(ImageIr {
            data,
            format,
            width_pt,
            height_pt,
            alt,
            align,
        })
    }
}

/// Normalize `ExchangeFormat::Jpg` to `ImageFormatIr::Jpeg`.
#[allow(non_upper_case_globals)]
impl ImageFormatIr {
    pub const Jpg: Self = Self::Jpeg;
}

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Inline-level Extraction
    // =========================================================================

    fn extract_inlines_flat(&mut self, nodes: &[TreeNode]) -> Vec<InlineNode> {
        self.extract_inlines_flat_with_bold(nodes, false)
    }

    fn extract_inlines_flat_with_bold(
        &mut self,
        nodes: &[TreeNode],
        force_bold: bool,
    ) -> Vec<InlineNode> {
        let mut out = Vec::new();
        let mut state = ActiveInlineState {
            bold: force_bold,
            ..Default::default()
        };
        let mut last_y: Option<f64> = None;
        let mut last_x_end: Option<f64> = None;
        self.extract_inlines_from_nodes(nodes, &mut out, &mut state, &mut last_y, &mut last_x_end);
        Self::merge_adjacent_text_runs(&mut out);
        out
    }

    fn extract_inlines_from_nodes(
        &mut self,
        nodes: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if state.hidden {
                        continue;
                    }
                    let y_pt = pos.y.to_pt();
                    let x_pt = pos.x.to_pt();

                    // Detect inter-word or inter-line spacing from laid-out glyph coordinates!
                    if let Some(prev_y) = *last_y {
                        if (y_pt - prev_y).abs() > text.size.to_pt() * 0.55
                            && state.script == ScriptKindIr::Normal
                        {
                            // New visual line inside the paragraph: insert a space if needed.
                            if Self::should_insert_line_join_space(out, &text.text) {
                                out.push(InlineNode::Text(TextRunIr {
                                    text: " ".to_string(),
                                    ..Self::text_item_to_run(text, state)
                                }));
                            }
                        } else if let Some(prev_x_end) = *last_x_end {
                            let gap_pt = x_pt - prev_x_end;
                            if gap_pt > text.size.to_pt() * 1.2 {
                                out.push(InlineNode::HorizontalSpace { width_pt: gap_pt });
                            } else if gap_pt > text.size.to_pt() * 0.18
                                && state.script == ScriptKindIr::Normal
                            {
                                let already_spaced = matches!(
                                    out.last(),
                                    Some(InlineNode::HorizontalSpace { .. })
                                        | Some(InlineNode::Tab)
                                ) || matches!(
                                    out.last(),
                                    Some(InlineNode::Text(r)) if r.text.ends_with(' ')
                                );
                                if !already_spaced && !text.text.starts_with(' ') {
                                    out.push(InlineNode::Text(TextRunIr {
                                        text: " ".to_string(),
                                        ..Self::text_item_to_run(text, state)
                                    }));
                                }
                            }
                        }
                    }

                    if state.script == ScriptKindIr::Normal {
                        *last_y = Some(y_pt);
                    }
                    *last_x_end = Some(x_pt + text.width().to_pt());

                    if let Paint::Tiling(_) = &text.fill {
                        let unsup = self.record_unsupported_paint("text.fill", Span::detached());
                        out.push(InlineNode::UnsupportedInline(unsup));
                    }

                    out.push(InlineNode::Text(Self::text_item_to_run(text, state)));
                }
                TreeNode::Image {
                    image, size, span, ..
                } => {
                    if let Some(img_ir) =
                        self.extract_image_item(image, *size, *span, HAlignIr::Left)
                    {
                        self.report.images_count += 1;
                        out.push(InlineNode::ImageInline(img_ir));
                    }
                }
                TreeNode::Shape { shape, span, pos } => {
                    if let Some(shape_ir) =
                        self.extract_raw_shape_item(shape, *span, *pos, Vec::new(), HAlignIr::Left)
                    {
                        self.report.shapes_count += 1;
                        out.push(InlineNode::ShapeInline(shape_ir));
                    }
                }
                TreeNode::Group {
                    group,
                    pos: _,
                    children,
                } => {
                    if Self::is_cetz_group_nodes(children) {
                        if let Some(canvas) = self.extract_cetz_canvas(
                            children,
                            Some(group.frame.size()),
                            HAlignIr::Left,
                        ) {
                            self.report.cetz_canvases_count += 1;
                            out.push(InlineNode::CetzInline(canvas));
                            continue;
                        }
                    }
                    self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
                }
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    self.extract_inline_element(
                        elem, *loc, *pos, children, out, state, last_y, last_x_end,
                    );
                }
                TreeNode::Link { .. } | TreeNode::PageBreak => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn extract_inline_element(
        &mut self,
        elem: &Content,
        loc: Location,
        pos: Point,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) {
        // 1. Check for unsupported inline elements.
        if let Some(unsup) = self.check_unsupported_element(elem, loc) {
            out.push(InlineNode::UnsupportedInline(unsup));
            return;
        }

        // 2. Inline formatting tags (`StrongElem`, `EmphElem`, `UnderlineElem`, `StrikeElem`,
        //    `OverlineElem`, `HighlightElem`, `SubElem`, `SuperElem`, `SmallcapsElem`, `HideElem`).
        if elem.is::<StrongElem>() {
            let prev = state.bold;
            state.bold = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.bold = prev;
            return;
        }
        if elem.is::<EmphElem>() {
            let prev = state.italic;
            state.italic = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.italic = prev;
            return;
        }
        if let Some(ul) = elem.to_packed::<UnderlineElem>() {
            let prev = state.underline.clone();
            let stroke = ul.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.underline = Some(Self::abs_stroke_to_ir(&stroke));
            // Filter out the decorative line shape inside `children` so it isn't emitted as a shape!
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.underline = prev;
            return;
        }
        if let Some(st) = elem.to_packed::<StrikeElem>() {
            let prev = state.strike.clone();
            let stroke = st.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.strike = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.strike = prev;
            return;
        }
        if let Some(ol) = elem.to_packed::<OverlineElem>() {
            let prev = state.overline.clone();
            let stroke = ol.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.overline = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.overline = prev;
            return;
        }
        if let Some(hl) = elem.to_packed::<HighlightElem>() {
            let prev = state.highlight.clone();
            state.highlight = hl
                .fill
                .get_ref(StyleChain::default())
                .as_ref()
                .and_then(paint_to_hex)
                .or_else(|| Some("FFF59D".to_string()));
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.highlight = prev;
            return;
        }
        if elem.is::<SubElem>() {
            let prev = state.script;
            state.script = ScriptKindIr::Sub;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.script = prev;
            return;
        }
        if elem.is::<SuperElem>() {
            let prev = state.script;
            state.script = ScriptKindIr::Super;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.script = prev;
            return;
        }
        if elem.is::<SmallcapsElem>() {
            let prev = state.smallcaps;
            state.smallcaps = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.smallcaps = prev;
            return;
        }
        if elem.is::<HideElem>() {
            return;
        }

        // 3. Inline Code (`RawElem` with `block: false`).
        if elem.is::<RawElem>() {
            let prev = state.is_code;
            state.is_code = true;
            let before_len = out.len();
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            if out.len() == before_len {
                out.push(InlineNode::Text(TextRunIr {
                    text: elem.plain_text().to_string(),
                    font_family: Some("Consolas".to_string()),
                    is_code: true,
                    ..Default::default()
                }));
            }
            state.is_code = prev;
            return;
        }

        // 4. Inline Equation (`EquationElem`).
        if let Some(eq) = elem.to_packed::<EquationElem>() {
            let eq_ir = self.extract_equation(eq, loc, children);
            self.report.equations_count += 1;
            out.push(InlineNode::EquationInline(eq_ir));
            *last_x_end = None;
            return;
        }

        // 5. Footnote Reference (`FootnoteElem`).
        if elem.is::<FootnoteElem>() {
            let id = self.next_footnote_id;
            self.next_footnote_id += 1;
            let number_text = Self::nodes_to_plain_text(children).trim().to_string();
            let content = self.footnotes.get(&loc).cloned().unwrap_or_default();
            self.report.footnotes_count += 1;
            out.push(InlineNode::Footnote {
                id,
                number_text: if number_text.is_empty() {
                    id.to_string()
                } else {
                    number_text
                },
                content,
            });
            return;
        }

        // 6. Cross-reference (`RefElem`).
        if let Some(rf) = elem.to_packed::<RefElem>() {
            let target_str = rf.target.resolve().as_str().to_string();
            let anchor = self
                .label_bookmarks
                .get(&target_str)
                .cloned()
                .unwrap_or_else(|| target_str.clone());
            let mut ref_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut ref_inlines, state, last_y, last_x_end);
            // Unwrap inner Link if `RefElem` realized into a `DirectLinkElem`/`LinkMarker`.
            let clean_inlines = Self::unwrap_link_inlines(ref_inlines);
            out.push(InlineNode::Ref {
                target: anchor,
                content: clean_inlines,
            });
            return;
        }

        // 7. Citation (`CiteGroup`).
        if let Some(cg) = elem.to_packed::<CiteGroup>() {
            let keys: Vec<String> = cg
                .children
                .iter()
                .map(|c| c.key.resolve().as_str().to_string())
                .collect();
            let mut cite_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut cite_inlines, state, last_y, last_x_end);
            let clean_inlines = Self::unwrap_link_inlines(cite_inlines);
            out.push(InlineNode::Citation {
                keys,
                content: clean_inlines,
            });
            return;
        }

        // 8. Hyperlink (`LinkMarker`, `LinkElem`, `DirectLinkElem`).
        if elem.is::<LinkMarker>() || elem.is::<LinkElem>() || elem.is::<DirectLinkElem>() {
            let dest = Self::find_link_destination(elem, children);
            let (url, anchor) = match dest {
                Some(Destination::Url(u)) => (Some(u.to_string()), None),
                Some(Destination::Location(target_loc)) => (
                    None,
                    self.bookmarks
                        .get(&target_loc)
                        .cloned()
                        .or_else(|| Some(format!("_typst_loc_{:x}", target_loc.hash()))),
                ),
                Some(Destination::Position(_)) => (None, None),
                None => (None, None),
            };
            let mut link_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut link_inlines, state, last_y, last_x_end);
            let had_inner_link = link_inlines
                .iter()
                .any(|inl| matches!(inl, InlineNode::Link { .. }));
            let clean_inlines = Self::unwrap_link_inlines(link_inlines);
            if !had_inner_link {
                self.report.links_count += 1;
            }
            out.push(InlineNode::Link {
                url,
                anchor,
                content: clean_inlines,
            });
            return;
        }

        // 9. Inline Shapes (`RectElem`, `CircleElem`, etc. inside a paragraph or `#box(...)`).
        if elem.is::<RotateElem>() || elem.is::<ScaleElem>() || elem.is::<MoveElem>() {
            if let Some(shape_ir) =
                self.try_extract_transformed_shape(elem, pos, children, 0, HAlignIr::Left)
            {
                self.report.shapes_count += 1;
                out.push(InlineNode::ShapeInline(shape_ir));
                *last_x_end = None;
                return;
            }
        }
        if Self::is_native_shape_elem(elem) {
            if let Some(shape_ir) = self.extract_native_shape_elem(
                elem,
                pos,
                children,
                0,
                HAlignIr::Left,
                0.0,
                (1.0, 1.0),
            ) {
                self.report.shapes_count += 1;
                out.push(InlineNode::ShapeInline(shape_ir));
                *last_x_end = None;
                return;
            }
        }

        // 10. Inline CeTZ canvas.
        if (elem.is::<LayoutElem>() || elem.elem().name() == "context")
            && (is_cetz_span(elem.span()) || Self::contains_cetz_shapes_recursive(children))
        {
            if let Some(canvas) = self.extract_cetz_canvas(children, None, HAlignIr::Left) {
                self.report.cetz_canvases_count += 1;
                out.push(InlineNode::CetzInline(canvas));
                *last_x_end = None;
                return;
            }
        }

        // 11. Inline Image (`ImageElem`).
        if let Some(img_elem) = elem.to_packed::<typst::visualize::ImageElem>() {
            if let Some(img_ir) = self.extract_image_elem(img_elem, children, HAlignIr::Left) {
                self.report.images_count += 1;
                out.push(InlineNode::ImageInline(img_ir));
                *last_x_end = None;
                return;
            }
        }

        // 12. Explicit Linebreak (`LinebreakElem`).
        if elem.is::<LinebreakElem>() {
            out.push(InlineNode::LineBreak);
            *last_y = None;
            *last_x_end = None;
            return;
        }

        // 13. Recurse into children for any other container/tag.
        self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
    }

    // =========================================================================
    // Conversion & Geometry Helpers
    // =========================================================================

    fn text_item_to_run(item: &TextItem, state: &ActiveInlineState) -> TextRunIr {
        let info = item.font.info();
        let family_lower = info.family.to_lowercase();
        let bold =
            state.bold || info.variant.weight >= FontWeight::BOLD || family_lower.contains("bold");
        let italic = state.italic
            || matches!(
                info.variant.style,
                typst::text::FontStyle::Italic | typst::text::FontStyle::Oblique
            )
            || family_lower.contains("italic")
            || family_lower.contains("oblique");
        let is_mono = state.is_code
            || info.flags.contains(typst::text::FontFlags::MONOSPACE)
            || family_lower.contains("mono")
            || family_lower.contains("consolas")
            || family_lower.contains("courier")
            || family_lower.contains("dejavu sans mono");

        let color = paint_to_hex(&item.fill).filter(|hex| hex != "000000" || state.is_code);
        let mapped_family = if is_mono {
            "Consolas".to_string()
        } else {
            Self::normalize_word_font_family(&info.family)
        };

        TextRunIr {
            text: item.text.to_string(),
            font_family: Some(mapped_family),
            font_size_pt: Some(item.size.to_pt()),
            bold,
            italic,
            underline: state.underline.clone(),
            strike: state.strike.clone(),
            overline: state.overline.clone(),
            color,
            highlight: state.highlight.clone(),
            script: state.script,
            smallcaps: state.smallcaps,
            is_code: state.is_code,
        }
    }

    fn normalize_word_font_family(family: &str) -> String {
        match family.trim() {
            "Hei" | "STHeiti" => "Heiti SC".to_string(),
            "Kai" | "STKaiti" => "Kaiti SC".to_string(),
            "Song" | "STSong" => "Songti SC".to_string(),
            other => other.to_string(),
        }
    }

    fn merge_adjacent_text_runs(inlines: &mut Vec<InlineNode>) {
        let mut i = 1;
        while i < inlines.len() {
            let can_merge = match (&inlines[i - 1], &inlines[i]) {
                (InlineNode::Text(a), InlineNode::Text(b)) => {
                    a.font_family == b.font_family
                        && a.font_size_pt.map(|v| (v * 10.0).round() as i64)
                            == b.font_size_pt.map(|v| (v * 10.0).round() as i64)
                        && a.bold == b.bold
                        && a.italic == b.italic
                        && a.underline.is_some() == b.underline.is_some()
                        && a.strike.is_some() == b.strike.is_some()
                        && a.overline.is_some() == b.overline.is_some()
                        && a.color == b.color
                        && a.highlight == b.highlight
                        && a.script == b.script
                        && a.smallcaps == b.smallcaps
                        && a.is_code == b.is_code
                }
                _ => false,
            };
            if can_merge {
                let InlineNode::Text(b) = inlines.remove(i) else {
                    unreachable!()
                };
                if let InlineNode::Text(a) = &mut inlines[i - 1] {
                    a.text.push_str(&b.text);
                }
            } else {
                i += 1;
            }
        }
    }

    fn should_insert_line_join_space(existing: &[InlineNode], next_text: &str) -> bool {
        let Some(InlineNode::Text(last_run)) = existing.last() else {
            return false;
        };
        let Some(last_ch) = last_run.text.chars().last() else {
            return false;
        };
        let Some(next_ch) = next_text.chars().next() else {
            return false;
        };
        if last_ch.is_whitespace() || next_ch.is_whitespace() {
            return false;
        }
        // Do not insert spaces between CJK characters when wrapping lines.
        if Self::is_cjk(last_ch) || Self::is_cjk(next_ch) {
            return false;
        }
        true
    }

    fn is_cjk(c: char) -> bool {
        matches!(
            c as u32,
            0x4E00..=0x9FFF
                | 0x3400..=0x4DBF
                | 0x3000..=0x303F
                | 0xFF00..=0xFFEF
        )
    }

    fn fixed_stroke_to_ir(stroke: &FixedStroke) -> StrokeIr {
        let color = paint_to_hex(&stroke.paint).unwrap_or_else(|| "000000".to_string());
        let thickness_pt = stroke.thickness.to_pt().max(0.25);
        let cap = match stroke.cap {
            LineCap::Butt => LineCapIr::Butt,
            LineCap::Round => LineCapIr::Round,
            LineCap::Square => LineCapIr::Square,
        };
        let join = match stroke.join {
            LineJoin::Miter => LineJoinIr::Miter,
            LineJoin::Round => LineJoinIr::Round,
            LineJoin::Bevel => LineJoinIr::Bevel,
        };
        let dash = match &stroke.dash {
            None => DashPatternIr::Solid,
            Some(dp) => Self::classify_dash_array(&dp.array, thickness_pt),
        };
        StrokeIr {
            color,
            thickness_pt,
            dash,
            cap,
            join,
        }
    }

    fn abs_stroke_to_ir(stroke: &Stroke<Abs>) -> StrokeIr {
        Self::fixed_stroke_to_ir(&stroke.clone().unwrap_or_default())
    }

    fn classify_dash_array(array: &[Abs], thickness_pt: f64) -> DashPatternIr {
        if array.is_empty() {
            return DashPatternIr::Solid;
        }
        if array.len() == 2 {
            let d0 = array[0].to_pt();
            if d0 <= thickness_pt * 1.5 {
                return DashPatternIr::Dotted;
            }
            return DashPatternIr::Dashed;
        }
        if array.len() == 4 {
            return DashPatternIr::DashDot;
        }
        DashPatternIr::Dashed
    }

    fn geometry_to_curve_segments(
        geom: &Geometry,
        offset: Point,
        origin_x_pt: f64,
        origin_y_pt: f64,
        out: &mut Vec<CurveSegmentIr>,
    ) {
        let ox = offset.x.to_pt() - origin_x_pt;
        let oy = offset.y.to_pt() - origin_y_pt;
        match geom {
            Geometry::Line(end) => {
                out.push(CurveSegmentIr::MoveTo { x_pt: ox, y_pt: oy });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox + end.x.to_pt(),
                    y_pt: oy + end.y.to_pt(),
                });
            }
            Geometry::Rect(sz) => {
                let w = sz.x.to_pt();
                let h = sz.y.to_pt();
                out.push(CurveSegmentIr::MoveTo { x_pt: ox, y_pt: oy });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox + w,
                    y_pt: oy,
                });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox + w,
                    y_pt: oy + h,
                });
                out.push(CurveSegmentIr::LineTo {
                    x_pt: ox,
                    y_pt: oy + h,
                });
                out.push(CurveSegmentIr::Close);
            }
            Geometry::Curve(curve) => {
                for item in &curve.0 {
                    match item {
                        CurveItem::Move(p) => out.push(CurveSegmentIr::MoveTo {
                            x_pt: ox + p.x.to_pt(),
                            y_pt: oy + p.y.to_pt(),
                        }),
                        CurveItem::Line(p) => out.push(CurveSegmentIr::LineTo {
                            x_pt: ox + p.x.to_pt(),
                            y_pt: oy + p.y.to_pt(),
                        }),
                        CurveItem::Cubic(c1, c2, p) => out.push(CurveSegmentIr::CubicTo {
                            cx1_pt: ox + c1.x.to_pt(),
                            cy1_pt: oy + c1.y.to_pt(),
                            cx2_pt: ox + c2.x.to_pt(),
                            cy2_pt: oy + c2.y.to_pt(),
                            x_pt: ox + p.x.to_pt(),
                            y_pt: oy + p.y.to_pt(),
                        }),
                        CurveItem::Close => out.push(CurveSegmentIr::Close),
                    }
                }
            }
        }
    }

    fn shapes_bbox(shapes: &[(Point, Shape, Span)]) -> (f64, f64, f64, f64) {
        if shapes.is_empty() {
            return (0.0, 0.0, 45.0, 30.0);
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (pos, sh, _) in shapes {
            let b = sh.geometry.bbox();
            min_x = min_x.min(pos.x.to_pt() + b.min.x.to_pt());
            min_y = min_y.min(pos.y.to_pt() + b.min.y.to_pt());
            max_x = max_x.max(pos.x.to_pt() + b.max.x.to_pt());
            max_y = max_y.max(pos.y.to_pt() + b.max.y.to_pt());
        }
        if !min_x.is_finite() || !max_x.is_finite() {
            return (0.0, 0.0, 45.0, 30.0);
        }
        (
            min_x,
            min_y,
            (max_x - min_x).max(1.0),
            (max_y - min_y).max(1.0),
        )
    }

    fn corners_radius_to_pt(
        corners: &Corners<Option<Rel<Length>>>,
        min_side_pt: f64,
    ) -> Option<f64> {
        let resolve_one = |opt: &Option<Rel<Length>>| -> f64 {
            match opt {
                Some(rel) => rel.abs.abs.to_pt() + rel.rel.get() * (min_side_pt * 0.5),
                None => 0.0,
            }
        };
        let r = resolve_one(&corners.top_left)
            .max(resolve_one(&corners.top_right))
            .max(resolve_one(&corners.bottom_left))
            .max(resolve_one(&corners.bottom_right));
        if r > 0.1 { Some(r) } else { None }
    }

    fn sides_rel_length_to_pt(sides: &Smart<Sides<Option<Rel<Length>>>>) -> SidesPtIr {
        let Smart::Custom(s) = sides else {
            return SidesPtIr::default();
        };
        let get = |opt: &Option<Rel<Length>>| opt.map_or(5.0, |r| r.abs.abs.to_pt().max(0.0));
        SidesPtIr {
            top: get(&s.top),
            bottom: get(&s.bottom),
            left: get(&s.left),
            right: get(&s.right),
        }
    }

    fn alignment_to_ir(align: Smart<Alignment>) -> (HAlignIr, VAlignIr) {
        let Smart::Custom(a) = align else {
            return (HAlignIr::Left, VAlignIr::Top);
        };
        let ha = match a.x() {
            Some(HAlignment::Center) => HAlignIr::Center,
            Some(HAlignment::Right | HAlignment::End) => HAlignIr::Right,
            _ => HAlignIr::Left,
        };
        let va = match a.y() {
            Some(VAlignment::Horizon) => VAlignIr::Center,
            Some(VAlignment::Bottom) => VAlignIr::Bottom,
            _ => VAlignIr::Top,
        };
        (ha, va)
    }

    fn infer_node_halign(&self, nodes: &[TreeNode]) -> Option<HAlignIr> {
        // 1. Check if any child element is an explicit `AlignElem`.
        if let Some(explicit) = Self::find_explicit_align_elem(nodes) {
            return Some(explicit);
        }

        // 2. Infer horizontal alignment from laid-out glyph/frame X coordinates within the content area!
        let (min_x, _min_y, width, _height) = Self::estimate_nodes_bbox(nodes);
        if width <= 0.5 || !min_x.is_finite() {
            return None;
        }

        let (left_bound, right_bound) = if min_x >= self.content_left_pt - 8.0 {
            (self.content_left_pt, self.content_right_pt)
        } else {
            (
                0.0,
                (self.content_right_pt - self.content_left_pt).max(100.0),
            )
        };

        let max_x = min_x + width;
        let left_gap = (min_x - left_bound).max(0.0);
        let right_gap = (right_bound - max_x).max(0.0);

        if left_gap <= 8.0 {
            Some(HAlignIr::Left)
        } else if left_gap > 15.0 && right_gap <= 8.0 {
            Some(HAlignIr::Right)
        } else if left_gap > 12.0 && right_gap > 12.0 && (left_gap - right_gap).abs() <= 18.0 {
            Some(HAlignIr::Center)
        } else {
            None
        }
    }

    fn find_explicit_align_elem(nodes: &[TreeNode]) -> Option<HAlignIr> {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if let Some(al) = elem.to_packed::<AlignElem>() {
                    let (ha, _) = Self::alignment_to_ir(Smart::Custom(
                        al.alignment.get(StyleChain::default()),
                    ));
                    return Some(ha);
                }
                if let Some(found) = Self::find_explicit_align_elem(children) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn find_group_size(nodes: &[TreeNode]) -> Size {
        for node in nodes {
            if let TreeNode::Group { group, .. } = node {
                return group.frame.size();
            }
        }
        Size::zero()
    }

    fn estimate_nodes_bbox(nodes: &[TreeNode]) -> (f64, f64, f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        Self::accumulate_nodes_bbox(
            nodes,
            Point::zero(),
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
        );

        if !min_x.is_finite() || !max_x.is_finite() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (
            min_x,
            min_y,
            (max_x - min_x).max(0.0),
            (max_y - min_y).max(0.0),
        )
    }

    fn accumulate_nodes_bbox(
        nodes: &[TreeNode],
        offset: Point,
        min_x: &mut f64,
        min_y: &mut f64,
        max_x: &mut f64,
        max_y: &mut f64,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y - text.size).to_pt();
                    *min_x = min_x.min(x);
                    *min_y = min_y.min(y);
                    *max_x = max_x.max(x + text.width().to_pt());
                    *max_y = max_y.max(y + text.size.to_pt() * 1.25);
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let gx = (offset.x + pos.x).to_pt();
                    let gy = (offset.y + pos.y).to_pt();
                    let gsz = group.frame.size();
                    if gsz.x.to_pt() > 0.0 && gsz.y.to_pt() > 0.0 {
                        *min_x = min_x.min(gx);
                        *min_y = min_y.min(gy);
                        *max_x = max_x.max(gx + gsz.x.to_pt());
                        *max_y = max_y.max(gy + gsz.y.to_pt());
                    } else {
                        Self::accumulate_nodes_bbox(
                            children,
                            offset + *pos,
                            min_x,
                            min_y,
                            max_x,
                            max_y,
                        );
                    }
                }
                TreeNode::Element { children, .. } => {
                    Self::accumulate_nodes_bbox(children, offset, min_x, min_y, max_x, max_y);
                }
                TreeNode::Image { size, pos, .. } => {
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y).to_pt();
                    *min_x = min_x.min(x);
                    *min_y = min_y.min(y);
                    *max_x = max_x.max(x + size.x.to_pt());
                    *max_y = max_y.max(y + size.y.to_pt());
                }
                _ => {}
            }
        }
    }

    fn find_link_destination(elem: &Content, children: &[TreeNode]) -> Option<Destination> {
        if let Some(dl) = elem.to_packed::<DirectLinkElem>() {
            return Some(Destination::Location(dl.loc));
        }
        for node in children {
            match node {
                TreeNode::Link { dest, .. } => return Some(dest.clone()),
                TreeNode::Element {
                    elem: celem,
                    children: cchildren,
                    ..
                } => {
                    if let Some(found) = Self::find_link_destination(celem, cchildren) {
                        return Some(found);
                    }
                }
                TreeNode::Group {
                    children: cchildren,
                    ..
                } => {
                    if let Some(found) = Self::find_link_destination(elem, cchildren) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn unwrap_link_inlines(inlines: Vec<InlineNode>) -> Vec<InlineNode> {
        let mut out = Vec::new();
        for item in inlines {
            if let InlineNode::Link { content, .. } = item {
                out.extend(Self::unwrap_link_inlines(content));
            } else {
                out.push(item);
            }
        }
        out
    }

    fn nodes_to_plain_text(nodes: &[TreeNode]) -> String {
        let mut s = String::new();
        for node in nodes {
            match node {
                TreeNode::Text { text, .. } => s.push_str(&text.text),
                TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                    s.push_str(&Self::nodes_to_plain_text(children));
                }
                _ => {}
            }
        }
        s
    }

    fn inlines_to_plain_text(inlines: &[InlineNode]) -> String {
        let mut s = String::new();
        for inline in inlines {
            match inline {
                InlineNode::Text(r) => s.push_str(&r.text),
                InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. } => {
                    s.push_str(&Self::inlines_to_plain_text(content));
                }
                _ => {}
            }
        }
        s
    }
}
