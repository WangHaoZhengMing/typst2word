//! Block-level dispatch: walks `TreeNode`s and routes each element to its
//! extractor in `elements/`.
//!
//! The order of checks in `extract_element_block` matters (artifacts and
//! unsupported elements first, CeTZ before generic layout containers, ...), so it
//! is kept as one explicit, routing-only `if` chain.

use typst::foundations::{Content, StyleChain};
use typst::introspection::Location;
use typst::layout::{
    ColbreakElem, GridElem, HideElem, LayoutElem, MoveElem, PagebreakElem, Point, RotateElem,
    ScaleElem,
};
use typst::math::EquationElem;
use typst::model::{
    BibliographyElem, CiteGroup, DirectLinkElem, EmphElem, EnumElem, FigureElem, FootnoteElem,
    FootnoteEntry, HeadingElem, LinkElem, LinkMarker, ListElem, OutlineElem, ParElem, QuoteElem,
    RefElem, StrongElem, TableElem, TermsElem, TitleElem,
};
use typst::pdf::{ArtifactElem, ArtifactKind};
use typst::text::{
    HighlightElem, OverlineElem, RawElem, SmallcapsElem, StrikeElem, SubElem, SuperElem,
    UnderlineElem,
};

use crate::compiler::compile::is_cetz_span;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{BlockNode, HAlignIr, ParagraphStyleIr, ShapeKindIr};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Block-level Extraction
    // =========================================================================
    pub(crate) fn extract_blocks_from_nodes(
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

        // Merge adjacent 1-cell `__styled_container__` tables produced when a single
        // `#block(stroke: ..., fill: ...)[...]` broke across columns or pages in Typst.
        let mut i = 1;
        while i < blocks.len() {
            let can_merge = match (&blocks[i - 1], &blocks[i]) {
                (BlockNode::Table(a), BlockNode::Table(b)) => {
                    a.summary.as_deref() == Some("__styled_container__")
                        && b.summary.as_deref() == Some("__styled_container__")
                        && a.rows.len() == 1
                        && b.rows.len() == 1
                        && a.rows[0].cells.len() == 1
                        && b.rows[0].cells.len() == 1
                }
                _ => false,
            };
            if can_merge {
                let BlockNode::Table(mut b) = blocks.remove(i) else {
                    unreachable!()
                };
                if let BlockNode::Table(a) = &mut blocks[i - 1] {
                    let mut b_blocks = std::mem::take(&mut b.rows[0].cells[0].blocks);
                    let a_blocks = &mut a.rows[0].cells[0].blocks;
                    if let (
                        Some(BlockNode::Enum {
                            items: a_items,
                            level: a_lvl,
                            ..
                        }),
                        Some(BlockNode::Enum { level: b_lvl, .. }),
                    ) = (a_blocks.last_mut(), b_blocks.first())
                        && *a_lvl == *b_lvl
                    {
                        if let BlockNode::Enum { items: b_items, .. } = b_blocks.remove(0) {
                            a_items.extend(b_items);
                        }
                    } else if let (
                        Some(BlockNode::List {
                            items: a_items,
                            level: a_lvl,
                            ..
                        }),
                        Some(BlockNode::List { level: b_lvl, .. }),
                    ) = (a_blocks.last_mut(), b_blocks.first())
                        && *a_lvl == *b_lvl
                    {
                        if let BlockNode::List { items: b_items, .. } = b_blocks.remove(0) {
                            a_items.extend(b_items);
                        }
                    }
                    a_blocks.extend(b_blocks);
                }
            } else {
                i += 1;
            }
        }

        blocks
    }

    pub(crate) fn is_stray_inline_node(node: &TreeNode) -> bool {
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

    pub(crate) fn flush_stray_inlines(
        &mut self,
        stray: &mut Vec<TreeNode>,
        blocks: &mut Vec<BlockNode>,
        align: HAlignIr,
    ) {
        let inferred_align = self.infer_node_halign(stray).unwrap_or(align);
        let eff_align = if inferred_align == HAlignIr::Left && align != HAlignIr::Left {
            align
        } else {
            inferred_align
        };
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
                    align: eff_align,
                    space_after_pt: 2.0,
                    ..Default::default()
                },
                bookmark: None,
            });
        }
    }

    pub(crate) fn extract_block_from_node(
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

    pub(crate) fn extract_element_block(
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
                // Page furniture (Typst 0.15 tags the page background as `Background`
                // instead of `Page`, and adds finer-grained pagination kinds).
                ArtifactKind::Header
                | ArtifactKind::Footer
                | ArtifactKind::Page
                | ArtifactKind::Background
                | ArtifactKind::Watermark
                | ArtifactKind::PageNumber
                | ArtifactKind::LineNumber
                | ArtifactKind::Bates
                | ArtifactKind::PaginationOther => return,
                ArtifactKind::Other | ArtifactKind::Layout | ArtifactKind::Redaction => {
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
        if elem.is::<TitleElem>() {
            self.extract_title_block(children, blocks);
            return;
        }

        // 5. Heading (`= Heading`, `== Subheading`).
        if let Some(heading) = elem.to_packed::<HeadingElem>() {
            self.extract_heading_block(heading, loc, children, blocks, inherited_align);
            return;
        }

        // 6. Paragraph (`ParElem`).
        if let Some(par) = elem.to_packed::<ParElem>() {
            self.extract_paragraph_block(par, loc, children, blocks, inherited_align);
            return;
        }

        // 7. Bullet List (`ListElem`).
        if let Some(list) = elem.to_packed::<ListElem>() {
            self.extract_list_block(list, children, blocks, list_level);
            return;
        }

        // 8. Numbered List (`EnumElem`).
        if let Some(en) = elem.to_packed::<EnumElem>() {
            self.extract_enum_block(en, children, blocks, list_level);
            return;
        }

        // 9. Term / Definition List (`TermsElem`).
        if let Some(terms) = elem.to_packed::<TermsElem>() {
            self.extract_terms_block(terms, children, blocks, list_level);
            return;
        }

        // 9b. Orphaned `PdfMarkerTag(ListItemLabel | ListItemBody)` when a `ListElem` or `EnumElem`
        //     inside a hard `Group` (`#block`) split across columns or pages.
        if let Some(marker) = elem.to_packed::<typst::pdf::PdfMarkerTag>() {
            match marker.kind {
                typst::pdf::PdfMarkerTagKind::ListItemLabel => {
                    let lbl = Self::nodes_to_plain_text(children).trim().to_string();
                    if !lbl.is_empty() {
                        self.pending_list_marker = Some(lbl);
                    }
                    return;
                }
                typst::pdf::PdfMarkerTagKind::ListItemBody => {
                    let marker_str = self
                        .pending_list_marker
                        .take()
                        .unwrap_or_else(|| "•".to_string());
                    let item_blocks =
                        self.extract_blocks_from_nodes(children, list_level + 1, HAlignIr::Left);
                    let is_enum = marker_str
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric());
                    if is_enum {
                        let parsed_num = marker_str
                            .trim_matches(|c: char| !c.is_ascii_digit())
                            .parse::<u64>()
                            .ok();
                        let item = crate::ir::EnumItemIr {
                            number: parsed_num,
                            marker_text: marker_str,
                            blocks: item_blocks,
                        };
                        if let Some(BlockNode::Enum { items, level, .. }) = blocks.last_mut()
                            && *level == list_level
                        {
                            items.push(item);
                        } else {
                            let chain = self.style_chain();
                            let default_font_pt = Self::resolve_length_pt(
                                chain.get(typst::text::TextElem::size).0,
                                11.0,
                            );
                            let indent_pt = Self::resolve_length_pt(
                                chain.get(typst::model::EnumElem::indent),
                                default_font_pt,
                            );
                            let body_indent_pt = Self::resolve_length_pt(
                                chain.get(typst::model::EnumElem::body_indent),
                                default_font_pt,
                            );
                            self.report.lists_count += 1;
                            blocks.push(BlockNode::Enum {
                                items: vec![item],
                                tight: true,
                                level: list_level,
                                spacing_pt: None,
                                indent_pt,
                                hanging_pt: (11.5 + body_indent_pt).max(16.0),
                            });
                        }
                    } else {
                        let item = crate::ir::ListItemIr {
                            marker: marker_str,
                            blocks: item_blocks,
                        };
                        if let Some(BlockNode::List { items, level, .. }) = blocks.last_mut()
                            && *level == list_level
                        {
                            items.push(item);
                        } else {
                            let chain = self.style_chain();
                            let default_font_pt = Self::resolve_length_pt(
                                chain.get(typst::text::TextElem::size).0,
                                11.0,
                            );
                            let indent_pt = Self::resolve_length_pt(
                                chain.get(typst::model::ListElem::indent),
                                default_font_pt,
                            );
                            let body_indent_pt = Self::resolve_length_pt(
                                chain.get(typst::model::ListElem::body_indent),
                                default_font_pt,
                            );
                            self.report.lists_count += 1;
                            blocks.push(BlockNode::List {
                                items: vec![item],
                                tight: true,
                                level: list_level,
                                spacing_pt: None,
                                indent_pt,
                                hanging_pt: (6.0 + body_indent_pt).max(13.0),
                            });
                        }
                    }
                    return;
                }
                _ => {}
            }
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
                list_level,
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
                list_level,
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
                self.extract_quote_block(quote, children, blocks, list_level, inherited_align);
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
        //     `PolygonElem`, `CurveElem`, `LineElem`, and rotated/scaled shapes).
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

        // 20. Explicit `AlignElem` container: propagate its horizontal alignment to children.
        if let Some(al) = elem.to_packed::<typst::layout::AlignElem>() {
            let (ha, _) = Self::alignment_to_ir(typst::foundations::Smart::Custom(
                al.alignment.get(StyleChain::default()),
            ));
            let child_blocks = self.extract_blocks_from_nodes(children, list_level, ha);
            blocks.extend(child_blocks);
            return;
        }

        // 20b. Scoped multi-column layout (`ColumnsElem`, e.g. `#columns(2)[...]`).
        if let Some(cols_elem) = elem.to_packed::<typst::layout::ColumnsElem>() {
            let count = cols_elem.count.get(self.style_chain()).get();
            let gutter_rel = cols_elem.gutter.get(self.style_chain());
            let base_font_pt = Self::resolve_length_pt(
                self.style_chain().get(typst::text::TextElem::size).0,
                11.0,
            );
            let full_w = (self.content_right_pt - self.content_left_pt).max(100.0);
            let gutter_pt = (Self::resolve_length_pt(gutter_rel.abs, base_font_pt)
                + gutter_rel.rel.get() * full_w)
                .max(6.0);

            let prev_cols = self.active_columns;
            if count > 1 {
                self.active_columns = (count, gutter_pt);
            }
            let child_blocks = self.extract_blocks_from_nodes(children, list_level, inherited_align);
            self.active_columns = prev_cols;

            if count > 1 && self.page_columns <= 1 {
                blocks.push(BlockNode::Columns {
                    count,
                    gutter_pt,
                    body: child_blocks,
                });
            } else {
                blocks.extend(child_blocks);
            }
            return;
        }

        // 21. Generic container fallback (recurse into children).
        let child_blocks = self.extract_blocks_from_nodes(children, list_level, inherited_align);
        blocks.extend(child_blocks);
    }
}
