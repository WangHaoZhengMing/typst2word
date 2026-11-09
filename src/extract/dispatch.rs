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
use typst::text::RawElem;
use typst::visualize::ImageElem;

use crate::compiler::compile::is_cetz_span;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr, ShapeKindIr};

impl<'a> ExtractorContext<'a> {
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

                // 3. Regular layout group: extract inner blocks and propagate horizontal offset.
                let mut inner =
                    self.extract_blocks_from_nodes(children, list_level, inherited_align);
                self.apply_group_left_offset(group, *pos, list_level, &mut inner);
                let group_h_pt = group.frame.size().y.to_pt();
                if inner.len() >= 2
                    && let Some(BlockNode::Paragraph { style, .. }) = inner.last_mut()
                    && (1.5..6.0).contains(&style.space_after_pt)
                {
                    style.space_after_pt = 6.05;
                }
                if group_h_pt >= 15.0 && inner.len() >= 2 {
                    let last_idx = inner.len() - 1;
                    let keep_all = group_h_pt <= 240.0 && inner.len() <= 6;
                    for (idx, b) in inner.iter_mut().enumerate() {
                        if let BlockNode::Paragraph { style, .. } = b {
                            style.keep_lines = true;
                            if idx == 0 || (keep_all && idx < last_idx) {
                                style.keep_with_next = true;
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
            self.extract_paragraph_block(par, loc, children, blocks, list_level, inherited_align);
            return;
        }

        // 7. Lists (`ListElem`, `EnumElem`, `TermsElem`) & orphaned cross-page list markers.
        if let Some(list) = elem.to_packed::<ListElem>() {
            self.extract_list_block(list, children, blocks, list_level);
            return;
        }
        if let Some(en) = elem.to_packed::<EnumElem>() {
            self.extract_enum_block(en, children, blocks, list_level);
            return;
        }
        if let Some(terms) = elem.to_packed::<TermsElem>() {
            self.extract_terms_block(terms, children, blocks, list_level);
            return;
        }
        if let Some(marker) = elem.to_packed::<PdfMarkerTag>()
            && self.try_extract_orphaned_list_marker(marker, children, blocks, list_level)
        {
            return;
        }

        // 8. Table (`TableElem`) and Grid (`GridElem`).
        if let Some(table) = elem.to_packed::<TableElem>()
            && let Some(table_ir) = self.extract_table_or_grid(
                table.grid.as_deref(),
                true,
                table
                    .summary
                    .get_cloned(StyleChain::default())
                    .map(|s| s.to_string()),
                children,
                list_level,
                inherited_align,
            )
        {
            self.report.tables_count += 1;
            blocks.push(BlockNode::Table(table_ir));
            return;
        }
        if let Some(grid) = elem.to_packed::<GridElem>()
            && let Some(table_ir) = self.extract_table_or_grid(
                grid.grid.as_deref(),
                false,
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
            && quote.block.get(StyleChain::default())
        {
            self.extract_quote_block(quote, children, blocks, list_level, inherited_align);
            return;
        }

        // 10. Fenced / Block Code (`RawElem`).
        if let Some(raw) = elem.to_packed::<RawElem>()
            && raw.block.get(StyleChain::default())
        {
            let code_block = self.extract_code_block(raw, children);
            self.report.code_blocks_count += 1;
            blocks.push(code_block);
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
            && let Some(shape_ir) =
                self.try_extract_transformed_shape(elem, pos, children, list_level, inherited_align)
        {
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
                children,
                blocks,
                list_level,
                inherited_align,
            );
            return;
        }

        // 18. Generic container fallback (recurse into children), honoring `BlockElem`
        // styled containers (`fill`/`stroke`) as well as `sticky` (`keep_with_next`) and `breakable: false` (`keep_lines`).
        if elem.is::<BlockElem>()
            && let Some(styled_box) = self.try_extract_styled_container_nodes(
                None,
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
        if let Some(blk) = elem.to_packed::<BlockElem>() {
            let sticky = blk.sticky.get(StyleChain::default());
            let unbreakable = !blk.breakable.get(StyleChain::default());
            if sticky || unbreakable {
                let last_idx = child_blocks.len().saturating_sub(1);
                for (idx, b) in child_blocks.iter_mut().enumerate() {
                    if let BlockNode::Paragraph { style, .. } = b {
                        if unbreakable {
                            style.keep_lines = true;
                            if idx < last_idx {
                                style.keep_with_next = true;
                            }
                        }
                        if sticky && idx == last_idx {
                            style.keep_with_next = true;
                        }
                    }
                }
            }
        }
        blocks.extend(child_blocks);
    }
}
