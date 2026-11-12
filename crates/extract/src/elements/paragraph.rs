//! `par` -> `BlockNode::Paragraph`.

use typst::foundations::{Packed, Smart};
use typst::introspection::Location;
use typst::layout::Point;
use typst::model::{Linebreaks, ParElem};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, HAlignIr, InlineNode, LinebreaksIr, ParagraphPdfMetricsIr, ParagraphStyleIr,
};

impl<'a> ExtractorContext<'a> {
    /// `ParElem` -> `BlockNode::Paragraph` (or a CeTZ canvas / block-structured `#box(...)` if the paragraph only wraps one).
    pub(crate) fn extract_paragraph_block(
        &mut self,
        par: &Packed<ParElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        // Check if this paragraph is actually just a wrapper around a single CeTZ canvas or block-structured `#box(...)`!
        if let Some((unwrapped, eff_align)) = Self::unwrap_single_par_child(children, inherited_align) {
            match unwrapped {
                TreeNode::Group {
                    group,
                    pos: gpos,
                    children: gchildren,
                } => {
                    if Self::is_cetz_group_nodes(gchildren) {
                        if let Some(canvas) = self.extract_cetz_canvas(
                            gchildren,
                            Some(group.frame.size()),
                            eff_align,
                        ) {
                            self.report.cetz_canvases_count += 1;
                            blocks.push(BlockNode::CetzCanvas(canvas));
                            return;
                        }
                    }
                    if Self::is_block_structured_box_group(gchildren) {
                        if let Some(styled_box) = self.try_extract_styled_container_group(
                            group,
                            *gpos,
                            gchildren,
                            list_level,
                            eff_align,
                        ) {
                            blocks.push(styled_box);
                            return;
                        }
                        if Self::contains_block_layout_elem(gchildren) {
                            self.extract_block_from_node(unwrapped, blocks, list_level, eff_align);
                            return;
                        }
                    }
                }
                TreeNode::Element {
                    elem,
                    pos: epos,
                    children: echildren,
                    ..
                } if elem.is::<typst::layout::BoxElem>()
                    && Self::is_block_structured_box_group(echildren) =>
                {
                    if let Some(styled_box) = self.try_extract_styled_container_nodes(
                        None,
                        *epos,
                        echildren,
                        list_level,
                        eff_align,
                    ) {
                        blocks.push(styled_box);
                        return;
                    }
                    if Self::contains_block_layout_elem(echildren) {
                        let inner =
                            self.extract_blocks_from_nodes(echildren, list_level, eff_align);
                        blocks.extend(inner);
                        return;
                    }
                }
                _ => {}
            }
        }

        let content = self.extract_inlines_flat(children);
        if content.is_empty() {
            return;
        }

        let chain = self.style_chain();
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let base_font_pt = content
            .iter()
            .find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_size_pt,
                InlineNode::EquationInline(eq) => eq.font_size_pt,
                _ => None,
            })
            .unwrap_or(default_font_pt)
            .max(6.0);

        let par_em_pt = if base_font_pt >= 15.0 {
            default_font_pt
        } else {
            base_font_pt
        };
        let line_leading_pt = Self::resolve_length_pt(par.leading.get(chain), par_em_pt);
        let leading_pt = Self::resolve_length_pt(par.leading.get(chain), default_font_pt);
        let spacing_pt = Self::resolve_length_pt(par.spacing.get(chain), default_font_pt);
        let ends_with_explicit_linebreak = matches!(content.last(), Some(InlineNode::LineBreak));

        let fl_indent_cfg = par.first_line_indent.get(chain);
        let mut first_line_indent_pt =
            Self::resolve_length_pt(fl_indent_cfg.amount(), base_font_pt);
        if first_line_indent_pt > 0.5
            && !fl_indent_cfg.all()
            && !matches!(blocks.last(), Some(BlockNode::Paragraph { .. }))
        {
            let (vis_first, _) = Self::infer_paragraph_line_indents(children);
            if vis_first <= 2.0 {
                first_line_indent_pt = 0.0;
            }
        }
        let mut hanging_indent_pt =
            Self::resolve_length_pt(par.hanging_indent.get(chain), base_font_pt);

        let inferred = self.infer_node_halign(children).unwrap_or(inherited_align);
        let base_align = if inferred == HAlignIr::Left && inherited_align != HAlignIr::Left {
            inherited_align
        } else {
            inferred
        };
        let align = if base_align == HAlignIr::Left && par.justify.get(chain) {
            HAlignIr::Justify
        } else {
            base_align
        };

        let mut left_indent_pt = 0.0;
        let mut right_indent_pt = 0.0;
        if matches!(align, HAlignIr::Left | HAlignIr::Justify) {
            if first_line_indent_pt < 0.5 && hanging_indent_pt < 0.5 {
                let (vis_first, vis_hang) = Self::infer_paragraph_line_indents(children);
                if vis_first > 2.0 {
                    first_line_indent_pt = vis_first;
                } else if vis_hang > 2.0 {
                    hanging_indent_pt = vis_hang;
                }
            }
            if list_level == 0
                && let Some(vis_left) = self.node_left_offset_in_column(children)
            {
                let mut line_starts: Vec<(f64, f64)> = Vec::new();
                Self::collect_line_left_edges(children, Point::zero(), &mut line_starts);
                let block_vis_left = if line_starts.len() <= 1 && first_line_indent_pt > 0.5 {
                    (vis_left - first_line_indent_pt).max(0.0)
                } else {
                    vis_left
                };
                if (2.0..=48.0).contains(&block_vis_left) {
                    left_indent_pt = block_vis_left;
                }
            }
            if list_level == 0 && align == HAlignIr::Justify && left_indent_pt > 2.0 {
                let (_, _, width, _) = Self::estimate_nodes_bbox(children);
                let col_w = self.current_column_width_pt();
                let r_gap = col_w - left_indent_pt - width;
                if (2.0..=48.0).contains(&r_gap) {
                    right_indent_pt = r_gap;
                }
            }
        }

        let linebreaks = match par.linebreaks.get(chain) {
            Smart::Custom(Linebreaks::Optimized) => LinebreaksIr::Optimized,
            Smart::Custom(Linebreaks::Simple) => LinebreaksIr::Simple,
            Smart::Auto => LinebreaksIr::Auto,
        };
        let justify = par.justify.get(chain);

        let style = ParagraphStyleIr {
            align,
            first_line_indent_pt,
            hanging_indent_pt,
            left_indent_pt,
            right_indent_pt,
            space_after_pt: (spacing_pt - leading_pt).max(0.0),
            line_spacing_pt: Some(line_leading_pt),
            linebreaks,
            justify,
            pdf_metrics: ParagraphPdfMetricsIr::Measured {
                ends_with_explicit_linebreak,
                base_font_pt,
                default_font_pt,
                par_spacing_pt: spacing_pt,
                par_leading_pt: leading_pt,
                in_multi_block_group_tail: false,
            },
            ..Default::default()
        };

        self.report.paragraphs_count += 1;
        blocks.push(BlockNode::Paragraph {
            content,
            style,
            bookmark: self.bookmarks.get(&loc).cloned(),
        });
    }

    pub(crate) fn infer_paragraph_line_indents(nodes: &[TreeNode]) -> (f64, f64) {
        let mut line_starts: Vec<(f64, f64)> = Vec::new();
        Self::collect_line_left_edges(nodes, Point::zero(), &mut line_starts);
        line_starts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if line_starts.len() < 2 {
            return (0.0, 0.0);
        }
        let x1 = line_starts[0].1;
        let x2 = line_starts[1].1;
        if x1 - x2 > 3.0 && x1 - x2 < 72.0 {
            (x1 - x2, 0.0)
        } else if x2 - x1 > 3.0 && x2 - x1 < 72.0 {
            (0.0, x2 - x1)
        } else {
            (0.0, 0.0)
        }
    }

    pub(crate) fn collect_line_left_edges(
        nodes: &[TreeNode],
        offset: Point,
        lines: &mut Vec<(f64, f64)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y).to_pt();
                    if let Some(existing) = lines.iter_mut().find(|(ly, _)| (*ly - y).abs() < 3.0) {
                        existing.1 = existing.1.min(x);
                    } else {
                        lines.push((y, x));
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_line_left_edges(children, offset + *pos, lines);
                }
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<typst::math::EquationElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                    {
                        continue;
                    }
                    Self::collect_line_left_edges(children, offset, lines);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn collect_line_x_spans(
        nodes: &[TreeNode],
        offset: Point,
        lines: &mut Vec<(f64, f64, f64)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let x = (offset.x + pos.x).to_pt();
                    let x_end = x + text.width().to_pt();
                    let y = (offset.y + pos.y).to_pt();
                    if let Some(existing) =
                        lines.iter_mut().find(|(ly, _, _)| (*ly - y).abs() < 3.0)
                    {
                        existing.1 = existing.1.min(x);
                        existing.2 = existing.2.max(x_end);
                    } else {
                        lines.push((y, x, x_end));
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_line_x_spans(children, offset + *pos, lines);
                }
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<typst::math::EquationElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                    {
                        continue;
                    }
                    Self::collect_line_x_spans(children, offset, lines);
                }
                _ => {}
            }
        }
    }

    fn is_ignorable_par_sibling(node: &TreeNode) -> bool {
        match node {
            TreeNode::Text { text, .. } => text.text.trim().is_empty(),
            TreeNode::Element { elem, children, .. } => {
                children.is_empty()
                    && !Self::is_native_shape_elem(elem)
                    && !elem.is::<typst::visualize::ImageElem>()
                    && !elem.is::<typst::text::LinebreakElem>()
                    && !elem.is::<typst::introspection::CounterDisplayElem>()
            }
            _ => false,
        }
    }

    fn unwrap_single_par_child(
        children: &[TreeNode],
        inherited_align: HAlignIr,
    ) -> Option<(&TreeNode, HAlignIr)> {
        let mut non_ws = children.iter().filter(|n| !Self::is_ignorable_par_sibling(n));
        let first = non_ws.next()?;
        if non_ws.next().is_some() {
            return None;
        }

        let mut eff_align = inherited_align;
        let mut cur = first;
        while let TreeNode::Element {
            elem,
            children: echildren,
            ..
        } = cur
        {
            let is_wrapper = elem.is::<typst::layout::BoxElem>()
                || elem.is::<typst::layout::AlignElem>()
                || elem.is::<typst::layout::LayoutElem>()
                || matches!(
                    elem.elem().name(),
                    "par-line-marker" | "styled" | "context"
                );
            if !is_wrapper {
                break;
            }
            if let Some(al) = elem.to_packed::<typst::layout::AlignElem>() {
                let (ha, _) = Self::alignment_to_ir(Smart::Custom(
                    al.alignment.get(typst::foundations::StyleChain::default()),
                ));
                eff_align = ha;
            }
            let mut inner_non_ws = echildren.iter().filter(|n| !Self::is_ignorable_par_sibling(n));
            let Some(only_child) = inner_non_ws.next() else {
                break;
            };
            if inner_non_ws.next().is_some() {
                break;
            }
            cur = only_child;
        }

        Some((cur, eff_align))
    }

    fn contains_block_layout_elem(nodes: &[TreeNode]) -> bool {
        let mut par_count = 0usize;
        Self::count_block_paras_and_check_block_elems(nodes, &mut par_count)
    }

    fn count_block_paras_and_check_block_elems(nodes: &[TreeNode], par_count: &mut usize) -> bool {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<typst::model::TableElem>()
                        || elem.is::<typst::layout::GridElem>()
                        || elem.is::<typst::model::ListElem>()
                        || elem.is::<typst::model::EnumElem>()
                        || elem.is::<typst::model::TermsElem>()
                        || elem.is::<typst::model::HeadingElem>()
                        || elem.is::<typst::model::TitleElem>()
                        || elem.is::<typst::model::FigureElem>()
                        || elem.is::<typst::layout::BlockElem>()
                        || elem.is::<typst::layout::ColumnsElem>()
                    {
                        return true;
                    }
                    if let Some(q) = elem.to_packed::<typst::model::QuoteElem>()
                        && q.block.get(typst::foundations::StyleChain::default())
                    {
                        return true;
                    }
                    if let Some(raw) = elem.to_packed::<typst::text::RawElem>()
                        && raw.block.get(typst::foundations::StyleChain::default())
                    {
                        return true;
                    }
                    if let Some(eq) = elem.to_packed::<typst::math::EquationElem>()
                        && eq.block.get(typst::foundations::StyleChain::default())
                    {
                        return true;
                    }
                    if elem.is::<typst::model::ParElem>() {
                        *par_count += 1;
                        if *par_count >= 2 {
                            return true;
                        }
                    }
                    if Self::count_block_paras_and_check_block_elems(children, par_count) {
                        return true;
                    }
                }
                TreeNode::Group { children, .. } => {
                    if Self::count_block_paras_and_check_block_elems(children, par_count) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn is_block_structured_box_group(nodes: &[TreeNode]) -> bool {
        if Self::contains_block_layout_elem(nodes) {
            return true;
        }
        let mut line_starts: Vec<(f64, f64)> = Vec::new();
        Self::collect_line_left_edges(nodes, Point::zero(), &mut line_starts);
        line_starts.len() >= 2
    }
}
