//! Block-level dispatch: routes `TreeNode`s and `Content` elements to their
//! dedicated extractors in `elements/` and `flow`.
//!
//! The order of checks in `extract_element_block` matters (artifacts and
//! unsupported elements first, CeTZ before generic layout containers, ...), so it
//! is kept as one explicit, routing-only `if` chain.

use typst::foundations::{Content, StyleChain};
use typst::introspection::Location;
use typst::layout::{
    AlignElem, BlockElem, ColbreakElem, ColumnsElem, GridElem, HideElem, LayoutElem, MoveElem,
    PadElem, PagebreakElem, Point, RotateElem, ScaleElem,
};
use typst::math::EquationElem;
use typst::model::{
    BibliographyElem, EnumElem, FigureElem, FootnoteEntry, HeadingElem, ListElem, OutlineElem,
    ParElem, QuoteElem, TableElem, TermsElem, TitleElem,
};
use typst::pdf::{ArtifactElem, ArtifactKind, PdfMarkerTag};
use typst::syntax::Span;
use typst::text::RawElem;
use typst::visualize::ImageElem;

use compiler::compile::is_cetz_span;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, HAlignIr, NodeId, SemanticRoleIr, ShapeKindIr, TableKindIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_block_from_node(
        &mut self,
        node: &TreeNode,
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        match node {
            TreeNode::Group {
                group,
                pos,
                children,
            } => {
                // 1. Standalone CeTZ canvas group.
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

                // 2. Styled `block(...)` or `box(...)` container with fill or border stroke.
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

                // 2.5. 2D `#place(dx, dy, ...)` positioned canvas group (`#block(width: 100%, height: ...)[ #place(...) ]`).
                if let Some(placed_para) = self.try_extract_placed_canvas_group(
                    group,
                    children,
                    list_level,
                    inherited_align,
                ) {
                    blocks.push(placed_para);
                    return;
                }

                // 3. Regular layout group: extract inner blocks and propagate horizontal offset.
                let prev_group_bounds = self.active_group_bounds_pt;
                let gw = group.frame.size().x.to_pt();
                if gw > 20.0 {
                    self.active_group_bounds_pt = Some((0.0, gw));
                }
                let mut inner =
                    self.extract_blocks_from_nodes(children, list_level, inherited_align);
                self.active_group_bounds_pt = prev_group_bounds;
                if inner.len() == 1
                    && let Some(BlockNode::CodeBlock { fill, stroke, .. }) = inner.first_mut()
                    && fill.is_none()
                    && stroke.is_none()
                {
                    let (f, s) = Self::find_code_block_box_style(children);
                    *fill = f;
                    *stroke = s;
                }
                self.apply_group_left_offset(group, *pos, list_level, &mut inner);
                let group_h_pt = group.frame.size().y.to_pt();
                if inner.len() >= 2
                    && let Some(BlockNode::Paragraph { id, style, .. }) = inner.last_mut()
                {
                    if style.resolved_par_spacing_pt.is_some() && !id.is_unset() {
                        self.reference_layout
                            .entry(*id)
                            .or_default()
                            .flow
                            .get_or_insert_with(Default::default)
                            .in_multi_block_group_tail = true;
                    } else if (1.5..6.0).contains(&style.space_after_pt) {
                        style.space_after_pt = 6.05;
                    }
                }
                if group_h_pt >= 15.0 && inner.len() >= 2 {
                    let last_idx = inner.len() - 1;
                    let keep_all = group_h_pt <= 240.0 && inner.len() <= 6;
                    for (idx, b) in inner.iter_mut().enumerate() {
                        if let BlockNode::Paragraph { style, .. } = b {
                            style.breakable = false;
                            if idx == 0 || (keep_all && idx < last_idx) {
                                style.sticky = true;
                            }
                        }
                    }
                }
                blocks.extend(inner);
            }
            TreeNode::Shape { shape, span, pos } => {
                if let Some(shape_ir) =
                    self.extract_raw_shape_item(shape, *span, *pos, Vec::new(), inherited_align)
                {
                    if let ShapeKindIr::Line {
                        x1_pt,
                        y1_pt,
                        x2_pt,
                        y2_pt,
                    } = shape_ir.kind
                        && (y2_pt - y1_pt).abs() < 0.5
                        && (x2_pt - x1_pt).abs() > 20.0
                    {
                        self.report.shapes_count += 1;
                        blocks.push(BlockNode::HorizontalRule {
                            length_pt: (x2_pt - x1_pt).abs(),
                            stroke: shape_ir.stroke.unwrap_or_default(),
                            align: inherited_align,
                        });
                        return;
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
                let prev_scope = self.active_scope_loc;
                if self.compiled.semantics.get(*loc).is_some() {
                    self.active_scope_loc = Some(*loc);
                }
                self.extract_element_block(
                    elem,
                    *loc,
                    *pos,
                    children,
                    blocks,
                    list_level,
                    inherited_align,
                );
                self.active_scope_loc = prev_scope;
            }
            TreeNode::Text { .. } | TreeNode::Link { .. } => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
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
        // 1. Skip page furniture artifacts (headers, footers, page decorations) and footnote entries.
        if let Some(artifact) = elem.to_packed::<ArtifactElem>() {
            match artifact.kind.get(StyleChain::default()) {
                ArtifactKind::Header
                | ArtifactKind::Footer
                | ArtifactKind::Page
                | ArtifactKind::Background
                | ArtifactKind::Watermark
                | ArtifactKind::PageNumber
                | ArtifactKind::LineNumber
                | ArtifactKind::Bates
                | ArtifactKind::PaginationOther => return,
                ArtifactKind::Other | ArtifactKind::Layout | ArtifactKind::Redaction => {}
            }
        }
        if elem.is::<FootnoteEntry>() {
            return;
        }

        // 2. Unsupported elements (`AttachElem`, `HtmlElem`, `FrameElem`, `SkewElem`, `RepeatElem`) & `HideElem`.
        if let Some(unsupported) = self.check_unsupported_element(elem, loc) {
            blocks.push(BlockNode::UnsupportedBlock(unsupported));
            return;
        }
        if elem.is::<HideElem>() {
            return;
        }

        // 3. CeTZ canvas (`LayoutElem` or `ContextElem` inside `@preview/cetz`).
        if (elem.is::<LayoutElem>() || elem.elem().name() == "context")
            && (is_cetz_span(elem.span()) || Self::contains_cetz_shapes_recursive(children))
            && let Some(canvas) = self.extract_cetz_canvas(children, None, inherited_align)
        {
            self.report.cetz_canvases_count += 1;
            blocks.push(BlockNode::CetzCanvas(canvas));
            return;
        }

        // 4. Title (`#title[...]`).
        if elem.is::<TitleElem>() {
            self.extract_title_block(elem.span(), loc, children, blocks);
            return;
        }

        // 5. Heading (`= Heading`, `== Subheading`).
        if let Some(heading) = elem.to_packed::<HeadingElem>() {
            self.extract_heading_block(heading, loc, children, blocks, inherited_align);
            return;
        }

        // 6. Paragraph (`ParElem`).
        if let Some(par) = elem.to_packed::<ParElem>() {
            self.extract_paragraph_block(par, loc, children, blocks, list_level, inherited_align);
            return;
        }

        // 7. Lists (`ListElem`, `EnumElem`, `TermsElem`) & orphaned cross-page list markers.
        if let Some(list) = elem.to_packed::<ListElem>() {
            self.extract_list_block(list, loc, children, blocks, list_level);
            return;
        }
        if let Some(en) = elem.to_packed::<EnumElem>() {
            self.extract_enum_block(en, loc, children, blocks, list_level);
            return;
        }
        if let Some(terms) = elem.to_packed::<TermsElem>() {
            self.extract_terms_block(terms, loc, children, blocks, list_level);
            return;
        }
        if let Some(marker) = elem.to_packed::<PdfMarkerTag>()
            && self.try_extract_orphaned_list_marker(marker, children, blocks, list_level)
        {
            return;
        }

        // 8. Table (`TableElem`) and Grid (`GridElem`).
        if let Some(table) = elem.to_packed::<TableElem>() {
            let summary = table
                .summary
                .get_cloned(self.scoped_style_chain(Some(loc)))
                .map(|s| s.to_string());
            if let Some(table_ir) = self.extract_table_or_grid(
                table.grid.as_deref(),
                elem.span(),
                Some(loc),
                TableKindIr::SemanticTable,
                summary,
                children,
                list_level,
                inherited_align,
            ) {
                self.report.tables_count += 1;
                blocks.push(BlockNode::Table(table_ir));
                return;
            }
        }
        if let Some(grid) = elem.to_packed::<GridElem>()
            && let Some(table_ir) = self.extract_table_or_grid(
                grid.grid.as_deref(),
                elem.span(),
                Some(loc),
                TableKindIr::LayoutGrid,
                None,
                children,
                list_level,
                inherited_align,
            )
        {
            self.report.tables_count += 1;
            blocks.push(BlockNode::Table(table_ir));
            return;
        }

        // 9. Block Quote (`QuoteElem`).
        if let Some(quote) = elem.to_packed::<QuoteElem>()
            && quote.block.get(self.scoped_style_chain(Some(loc)))
        {
            self.extract_quote_block(quote, loc, children, blocks, list_level, inherited_align);
            return;
        }

        // 10. Fenced / Block Code (`RawElem`) or Cross-Page Split `RawLine`.
        if let Some(raw) = elem.to_packed::<RawElem>()
            && raw.block.get(self.scoped_style_chain(Some(loc)))
        {
            let code_block = self.extract_code_block(raw, loc, children);
            self.report.code_blocks_count += 1;
            blocks.push(code_block);
            return;
        }
        if let Some(raw_line) = elem.to_packed::<typst::text::RawLine>() {
            let mut runs = Vec::new();
            Self::collect_raw_line_runs(children, &mut runs);
            if runs.is_empty() {
                runs.push(ir::TextRunIr {
                    text: raw_line.text.to_string(),
                    font_family: None,
                    is_code: true,
                    ..Default::default()
                });
            }
            if let Some(BlockNode::CodeBlock { lines, .. }) = blocks.last_mut() {
                lines.push(runs);
                Self::normalize_empty_code_line_metrics(lines);
            } else {
                let mut lines = vec![runs];
                Self::normalize_empty_code_line_metrics(&mut lines);
                let id = self.alloc_node_with_fragments(
                    elem.span(),
                    Some(loc),
                    Some(SemanticRoleIr::CodeBlock { lang: None }),
                    children,
                );
                blocks.push(BlockNode::CodeBlock {
                    id,
                    lang: None,
                    lines,
                    fill: None,
                    stroke: None,
                });
            }
            return;
        }

        // 11. Figure (`FigureElem`).
        if let Some(fig) = elem.to_packed::<FigureElem>() {
            let fig_block = self.extract_figure(fig, loc, children, list_level);
            self.report.figures_count += 1;
            blocks.push(fig_block);
            return;
        }

        // 12. Equation (`EquationElem`).
        if let Some(eq) = elem.to_packed::<EquationElem>() {
            let eq_ir = self.extract_equation(eq, loc, children);
            self.report.equations_count += 1;
            blocks.push(BlockNode::EquationBlock(eq_ir));
            return;
        }

        // 13. Native Shapes & Transforms (`RotateElem`, `ScaleElem`, `MoveElem`, `RectElem`, ...).
        if (elem.is::<RotateElem>() || elem.is::<ScaleElem>() || elem.is::<MoveElem>())
            && let Some(mut shape_ir) =
                self.try_extract_transformed_shape(elem, pos, children, list_level, inherited_align)
        {
            if shape_ir.placed_offset_pt.is_none()
                && !elem.is::<MoveElem>()
                && self.container_inset_pt <= 0.1
                && matches!(inherited_align, HAlignIr::Left | HAlignIr::Justify)
            {
                let left_bound = self
                    .active_group_bounds_pt
                    .map(|(l, _)| l)
                    .unwrap_or(self.content_left_pt);
                let dx = pos.x.to_pt() - left_bound;
                let (_, local_y, _) =
                    crate::frame_tree::decode_page_and_local_y(pos.y.to_pt(), pos.x.to_pt());
                let dy = if blocks.is_empty() && self.active_group_bounds_pt.is_none() {
                    (local_y - self.content_top_pt).max(0.0)
                } else {
                    0.0
                };
                if (1.0..=120.0).contains(&dx) || (1.0..=120.0).contains(&dy) {
                    shape_ir.placed_offset_pt = Some((dx.max(0.0), dy));
                }
            }
            self.report.shapes_count += 1;
            blocks.push(BlockNode::ShapeBlock(shape_ir));
            return;
        }
        if Self::is_native_shape_elem(elem)
            && let Some(shape_ir) = self.extract_native_shape_elem(
                elem,
                pos,
                children,
                list_level,
                inherited_align,
                0.0,
                (1.0, 1.0),
            )
        {
            if let ShapeKindIr::Line {
                x1_pt,
                y1_pt,
                x2_pt,
                y2_pt,
            } = shape_ir.kind
                && (y2_pt - y1_pt).abs() < 0.5
                && (x2_pt - x1_pt).abs() > 80.0
            {
                self.report.shapes_count += 1;
                blocks.push(BlockNode::HorizontalRule {
                    length_pt: (x2_pt - x1_pt).abs(),
                    stroke: shape_ir.stroke.unwrap_or_default(),
                    align: inherited_align,
                });
                return;
            }
            self.report.shapes_count += 1;
            blocks.push(BlockNode::ShapeBlock(shape_ir));
            return;
        }

        // 14. Image (`ImageElem`).
        if let Some(img_elem) = elem.to_packed::<ImageElem>()
            && let Some(img_ir) = self.extract_image_elem(img_elem, children, inherited_align)
        {
            self.report.images_count += 1;
            blocks.push(BlockNode::ImageBlock(img_ir));
            return;
        }

        // 15. Outline (`OutlineElem`) & Bibliography (`BibliographyElem`).
        if elem.is::<OutlineElem>() {
            let outline_block = self.extract_outline(children);
            blocks.push(outline_block);
            return;
        }
        if elem.is::<BibliographyElem>() {
            let bib_block = self.extract_bibliography(children);
            blocks.push(bib_block);
            return;
        }

        // 16. Explicit Pagebreak / Colbreak.
        if elem.is::<PagebreakElem>() {
            blocks.push(BlockNode::PageBreak);
            return;
        }
        if elem.is::<ColbreakElem>() {
            blocks.push(BlockNode::ColumnBreak);
            return;
        }

        // 17. Layout Containers (`AlignElem`, `PadElem`, `ColumnsElem`).
        if let Some(al) = elem.to_packed::<AlignElem>() {
            self.extract_align_container(al, children, blocks, list_level);
            return;
        }
        if let Some(pad) = elem.to_packed::<PadElem>() {
            self.extract_pad_container(pad, children, blocks, list_level, inherited_align);
            return;
        }
        if let Some(cols_elem) = elem.to_packed::<ColumnsElem>() {
            self.extract_columns_container(
                cols_elem,
                loc,
                children,
                blocks,
                list_level,
                inherited_align,
            );
            return;
        }

        // 18. Generic container fallback (recurse into children), honoring `BlockElem`
        // styled containers (`fill`/`stroke`) as well as `sticky` and `breakable: false`.
        let blk_props = elem.to_packed::<BlockElem>().map(|blk| {
            let ch = self.scoped_style_chain(Some(loc));
            (blk.breakable.get(ch), blk.sticky.get(ch))
        });
        if let Some((breakable, sticky)) = blk_props
            && let Some(styled_box) = self.try_extract_styled_container_nodes(
                None,
                elem.span(),
                Some(loc),
                breakable,
                sticky,
                pos,
                children,
                list_level,
                inherited_align,
            )
        {
            blocks.push(styled_box);
            return;
        }
        let mut child_blocks =
            self.extract_blocks_from_nodes(children, list_level, inherited_align);
        if let Some((breakable, sticky)) = blk_props {
            let unbreakable = !breakable;
            if sticky || unbreakable {
                let last_idx = child_blocks.len().saturating_sub(1);
                for (idx, b) in child_blocks.iter_mut().enumerate() {
                    if let BlockNode::Paragraph { style, .. } = b {
                        if unbreakable {
                            style.breakable = false;
                            if idx < last_idx {
                                style.sticky = true;
                            }
                        }
                        if sticky && idx == last_idx {
                            style.sticky = true;
                        }
                    }
                }
            }
        }
        blocks.extend(child_blocks);
    }

    fn unwrap_placed_child(mut node: &TreeNode) -> &TreeNode {
        while let TreeNode::Element { elem, children, .. } = node {
            if elem.is::<ImageElem>() || Self::is_native_shape_elem(elem) || children.len() != 1 {
                break;
            }
            node = &children[0];
        }
        node
    }

    fn try_extract_placed_canvas_group(
        &mut self,
        group: &typst::layout::GroupItem,
        children: &[TreeNode],
        list_level: usize,
        inherited_align: HAlignIr,
    ) -> Option<BlockNode> {
        let gw_pt = group.frame.size().x.to_pt();
        let gh_pt = group.frame.size().y.to_pt();
        let all_explicit_place = children.len() >= 2
            && children.iter().all(|ch| {
                matches!(ch, TreeNode::Element { elem, .. } if elem.is::<typst::layout::PlaceElem>())
            });
        if gw_pt < 120.0
            || gh_pt < 24.0
            || (!all_explicit_place && (gw_pt < 200.0 || gh_pt < 40.0 || children.len() < 3))
        {
            return None;
        }

        // Check if children represent 2D `#place(dx, dy)` items (overlapping Y spans with distinct X offsets >= 60pt).
        let mut item_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
        for raw_ch in children {
            match Self::unwrap_placed_child(raw_ch) {
                TreeNode::Group { group: cg, pos, .. } => {
                    item_boxes.push((
                        pos.x.to_pt(),
                        pos.y.to_pt(),
                        cg.frame.size().x.to_pt(),
                        cg.frame.size().y.to_pt(),
                    ));
                }
                TreeNode::Element { elem, pos, children: ec, .. } => {
                    if elem.is::<ImageElem>() {
                        let (bx, by, bw, bh) = Self::estimate_nodes_bbox(ec);
                        item_boxes.push((bx, by, bw.max(10.0), bh.max(2.0)));
                    } else if Self::is_native_shape_elem(elem) {
                        let (bx, by, bw, bh) =
                            Self::estimate_nodes_bbox(std::slice::from_ref(raw_ch));
                        let (bx, by, bw, bh) = if bx.is_finite() && by.is_finite() {
                            (bx, by, bw, bh)
                        } else {
                            (pos.x.to_pt(), pos.y.to_pt(), 10.0, 2.0)
                        };
                        item_boxes.push((bx, by, bw.max(10.0), bh.max(2.0)));
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        let has_2d_overlap = item_boxes.iter().enumerate().any(|(i, &(xa, ya, _wa, ha))| {
            item_boxes.iter().skip(i + 1).any(|&(xb, yb, _wb, hb)| {
                (xa - xb).abs() >= 60.0
                    && (xa >= 60.0 || xb >= 60.0)
                    && ya < yb + hb - 2.0
                    && yb < ya + ha - 2.0
            })
        });
        if !all_explicit_place && !has_2d_overlap {
            return None;
        }

        let mut inlines: Vec<ir::InlineNode> = Vec::new();
        // Invisible inline spacer reserving `gh_pt` vertical height in normal flow.
        inlines.push(ir::InlineNode::ShapeInline(ir::ShapeIr {
            id: NodeId::UNSET,
            kind: ir::ShapeKindIr::Rect,
            width_pt: 1.0,
            height_pt: gh_pt,
            fill: None,
            fill_rule_even_odd: false,
            stroke: None,
            borders: ir::BordersIr::default(),
            corner_radius_pt: None,
            inset: ir::SidesPtIr::zero(),
            rotation_deg: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            body: Vec::new(),
            align: HAlignIr::Left,
            placed_offset_pt: None,
            baseline_shift_pt: None,
        }));

        for raw_ch in children {
            match Self::unwrap_placed_child(raw_ch) {
                TreeNode::Element {
                    elem,
                    pos,
                    children: ec,
                    ..
                } if elem.is::<ImageElem>() => {
                    if let Some(img_elem) = elem.to_packed::<ImageElem>()
                        && let Some(mut img_ir) =
                            self.extract_image_elem(img_elem, ec, HAlignIr::Left)
                    {
                        let img_pos = ec
                            .iter()
                            .find_map(|n| match n {
                                TreeNode::Image { pos: ip, .. } => Some(*ip),
                                _ => None,
                            })
                            .unwrap_or(*pos);
                        img_ir.placed_offset_pt = Some((img_pos.x.to_pt(), img_pos.y.to_pt()));
                        self.report.images_count += 1;
                        inlines.push(ir::InlineNode::ImageInline(img_ir));
                    }
                }
                TreeNode::Element {
                    elem,
                    pos,
                    children: ec,
                    ..
                } if Self::is_native_shape_elem(elem) => {
                    if let Some(mut shape_ir) = self.extract_native_shape_elem(
                        elem,
                        *pos,
                        ec,
                        list_level,
                        HAlignIr::Left,
                        0.0,
                        (1.0, 1.0),
                    ) {
                        let (bx, by, _, _) =
                            Self::estimate_nodes_bbox(std::slice::from_ref(raw_ch));
                        let off = if bx.is_finite() && by.is_finite() {
                            (bx, by)
                        } else {
                            let sh_pos = ec
                                .iter()
                                .find_map(|n| match n {
                                    TreeNode::Shape { pos: sp, .. } => Some(*sp),
                                    _ => None,
                                })
                                .unwrap_or(*pos);
                            (sh_pos.x.to_pt(), sh_pos.y.to_pt())
                        };
                        shape_ir.placed_offset_pt = Some(off);
                        self.report.shapes_count += 1;
                        inlines.push(ir::InlineNode::ShapeInline(shape_ir));
                    }
                }
                TreeNode::Group {
                    group: cg,
                    pos: cpos,
                    children: cchildren,
                } => {
                    let cw_pt = cg.frame.size().x.to_pt();
                    let ch_pt = cg.frame.size().y.to_pt();
                    let prev_gw = self.active_group_bounds_pt;
                    self.active_group_bounds_pt = Some((0.0, cw_pt));
                    let sub_blocks =
                        self.extract_blocks_from_nodes(cchildren, list_level, inherited_align);
                    self.active_group_bounds_pt = prev_gw;
                    self.report.shapes_count += 1;
                    inlines.push(ir::InlineNode::ShapeInline(ir::ShapeIr {
                        id: NodeId::UNSET,
                        kind: ir::ShapeKindIr::Rect,
                        width_pt: cw_pt.max(20.0),
                        height_pt: (ch_pt + 20.0).max(24.0),
                        fill: None,
                        fill_rule_even_odd: false,
                        stroke: None,
                        borders: ir::BordersIr::default(),
                        corner_radius_pt: None,
                        inset: ir::SidesPtIr::zero(),
                        rotation_deg: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        body: sub_blocks,
                        align: HAlignIr::Left,
                        placed_offset_pt: Some((cpos.x.to_pt(), cpos.y.to_pt())),
                        baseline_shift_pt: None,
                    }));
                }
                _ => {}
            }
        }

        let id = self.alloc_node_with_fragments(
            Span::detached(),
            None,
            Some(SemanticRoleIr::Paragraph),
            children,
        );
        Some(BlockNode::Paragraph {
            id,
            content: inlines,
            style: ir::ParagraphStyleIr {
                align: HAlignIr::Left,
                space_before_pt: 0.0,
                space_after_pt: 0.0,
                line_spacing_pt: None,
                ..Default::default()
            },
            bookmark: None,
        })
    }
}
