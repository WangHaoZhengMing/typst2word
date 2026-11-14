//! `par` -> `BlockNode::Paragraph`.

use typst::foundations::{Packed, Smart};
use typst::introspection::Location;
use typst::layout::Point;
use typst::model::{Linebreaks, ParElem};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    AdjacentTextLineIr, BlockNode, HAlignIr, InlineBoxLineContextIr, InlineNode, LinebreaksIr,
    LocaleIr, NodeId, ParagraphLineMetricsIr, ParagraphStyleIr, SemanticRoleIr, TextDirIr,
    is_cjk_char,
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
                    loc: eloc,
                    pos: epos,
                    children: echildren,
                } if elem.is::<typst::layout::BoxElem>()
                    && Self::is_block_structured_box_group(echildren) =>
                {
                    if let Some(styled_box) = self.try_extract_styled_container_nodes(
                        None,
                        elem.span(),
                        Some(*eloc),
                        false,
                        false,
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

        let chain = self.scoped_style_chain(Some(loc));
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
        let lang = chain.get(typst::text::TextElem::lang);
        let region = chain.get(typst::text::TextElem::region);
        let locale = (lang.as_str() != "en" || region.is_some())
            .then(|| LocaleIr::new(lang.as_str(), region.map(|r| r.as_str().to_string())));
        let dir = (chain.resolve(typst::text::TextElem::dir) == typst::layout::Dir::RTL)
            .then_some(TextDirIr::Rtl);

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
            ends_with_explicit_linebreak,
            resolved_font_size_pt: Some(base_font_pt),
            resolved_default_font_size_pt: Some(default_font_pt),
            resolved_par_spacing_pt: Some(spacing_pt),
            resolved_par_leading_pt: Some(leading_pt),
            locale,
            dir,
            ..Default::default()
        };

        let id = self.alloc_node_with_fragments(
            par.span(),
            Some(loc),
            Some(SemanticRoleIr::Paragraph),
            children,
        );
        if let Some(para_lines) = Self::collect_paragraph_line_metrics(children, base_font_pt) {
            self.reference_layout.entry(id).or_default().paragraph_lines = Some(para_lines);
        }
        self.record_inline_shape_line_contexts(children, &content, base_font_pt);
        self.report.paragraphs_count += 1;
        blocks.push(BlockNode::Paragraph {
            id,
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
                TreeNode::Text { text, pos, .. } => {
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
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    // For a bordered/shaded inline `#box(...)` group (which carries its own
                    // background `Shape`), record the box's outer frame left edge rather than
                    // descending into its `inset`-padded inner text; otherwise an inline `#box`
                    // with `inset: 12pt` on line 2 would falsely look like `hanging_indent = 12pt`.
                    if Self::group_has_direct_bg_shape(children)
                        && group.frame.size().x.to_pt() > 0.0
                    {
                        let x = (offset.x + pos.x).to_pt();
                        let top_y = (offset.y + pos.y).to_pt();
                        let h_pt = group.frame.size().y.to_pt();
                        let bot_y = top_y + h_pt;
                        let approx_base_y = top_y + h_pt * 0.8;
                        if let Some(existing) = lines
                            .iter_mut()
                            .find(|(ly, _)| *ly >= top_y - 1.5 && *ly <= bot_y + 1.5)
                        {
                            existing.1 = existing.1.min(x);
                        } else {
                            lines.push((approx_base_y, x));
                        }
                    } else {
                        Self::collect_line_left_edges(children, offset + *pos, lines);
                    }
                }
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<typst::math::EquationElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                        || Self::is_native_shape_elem(elem)
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
                TreeNode::Text { text, pos, .. } => {
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
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if Self::group_has_direct_bg_shape(children)
                        && group.frame.size().x.to_pt() > 0.0
                    {
                        let x = (offset.x + pos.x).to_pt();
                        let x_end = x + group.frame.size().x.to_pt();
                        let top_y = (offset.y + pos.y).to_pt();
                        let h_pt = group.frame.size().y.to_pt();
                        let bot_y = top_y + h_pt;
                        let approx_base_y = top_y + h_pt * 0.8;
                        if let Some(existing) = lines
                            .iter_mut()
                            .find(|(ly, _, _)| *ly >= top_y - 1.5 && *ly <= bot_y + 1.5)
                        {
                            existing.1 = existing.1.min(x);
                            existing.2 = existing.2.max(x_end);
                        } else {
                            lines.push((approx_base_y, x, x_end));
                        }
                    } else {
                        Self::collect_line_x_spans(children, offset + *pos, lines);
                    }
                }
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<typst::math::EquationElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                        || Self::is_native_shape_elem(elem)
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

    pub(crate) fn group_has_direct_bg_shape(children: &[TreeNode]) -> bool {
        children.iter().any(|c| match c {
            TreeNode::Shape { .. } => true,
            TreeNode::Element {
                elem,
                children: achildren,
                ..
            } if elem.is::<typst::pdf::ArtifactElem>() => {
                achildren.iter().any(|ac| matches!(ac, TreeNode::Shape { .. }))
            }
            _ => false,
        })
    }

    /// Measure laid-out normal-text baseline Y coordinates and compute the uniform baseline step
    /// ($\Delta y = y_{k+1} - y_k$) for a paragraph in Typst's reference layout.
    ///
    /// Excludes sub/superscripts, equations, footnotes, and multi-line/tall inline `#box` interiors
    /// so tall inline objects never contaminate the paragraph's text line-spacing measurement.
    pub(crate) fn collect_paragraph_line_metrics(
        nodes: &[TreeNode],
        base_font_pt: f64,
    ) -> Option<ParagraphLineMetricsIr> {
        let mut raw_lines: Vec<(usize, f64, f64)> = Vec::new();
        let mut has_tall_inline_object = false;
        Self::walk_paragraph_baselines(
            nodes,
            Point::zero(),
            base_font_pt,
            &mut raw_lines,
            &mut has_tall_inline_object,
        );
        if raw_lines.is_empty() {
            return None;
        }

        raw_lines.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Deduplicate baselines on the same page and column (`|dx| < 200pt`) within 3.0pt tolerance.
        let mut lines: Vec<(usize, f64, f64)> = Vec::with_capacity(raw_lines.len());
        for (p, x, y) in raw_lines {
            if let Some(last) = lines.last_mut()
                && last.0 == p
                && (x - last.1).abs() < 200.0
                && (y - last.2).abs() < 3.0
            {
                continue;
            }
            lines.push((p, x, y));
        }

        let line_count = lines.len();
        let first_baseline_y_pt = lines.first()?.2;
        let last_baseline_y_pt = lines.last()?.2;

        // Collect same-page, same-column consecutive baseline steps.
        // When `has_tall_inline_object` is true, cap candidate steps at `2.2 * S` so a line hosting
        // a tall `#box` / image / display fraction is excluded from the normal text baseline step.
        let min_step = base_font_pt * 0.80;
        let max_step = if has_tall_inline_object {
            base_font_pt * 2.20
        } else {
            base_font_pt * 5.00
        };
        let mut steps: Vec<f64> = Vec::new();
        for pair in lines.windows(2) {
            let (p0, x0, y0) = pair[0];
            let (p1, x1, y1) = pair[1];
            if p0 == p1 && (x1 - x0).abs() < 200.0 {
                let dy = y1 - y0;
                if dy >= min_step && dy <= max_step {
                    steps.push(dy);
                }
            }
        }
        steps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let uniform_baseline_step_pt = if steps.is_empty() {
            None
        } else if has_tall_inline_object {
            // Prefer lower-quartile normal step when tall inline objects are present.
            Some(steps[(steps.len() - 1) / 4])
        } else {
            Some(steps[steps.len() / 2])
        };

        Some(ParagraphLineMetricsIr {
            line_count,
            first_baseline_y_pt,
            last_baseline_y_pt,
            uniform_baseline_step_pt,
            has_tall_inline_object,
        })
    }

    fn walk_paragraph_baselines(
        nodes: &[TreeNode],
        offset: Point,
        base_font_pt: f64,
        lines: &mut Vec<(usize, f64, f64)>,
        has_tall_inline_object: &mut bool,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text {
                    text,
                    pos,
                    baseline_shift_pt,
                    ..
                } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let abs_x = (offset.x + pos.x).to_pt();
                    let raw_y = (offset.y + pos.y).to_pt() + baseline_shift_pt.unwrap_or(0.0);
                    let (page_idx, local_y, x_pt) =
                        crate::frame_tree::decode_page_and_local_y(raw_y, abs_x);
                    lines.push((page_idx, x_pt, local_y));
                }
                TreeNode::Shape { shape, .. } => {
                    if shape.geometry.bbox(None).size().y.to_pt().abs() > base_font_pt * 1.35 {
                        *has_tall_inline_object = true;
                    }
                }
                TreeNode::Image { size, .. } => {
                    if size.y.to_pt().abs() > base_font_pt * 1.35 {
                        *has_tall_inline_object = true;
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let gh = group.frame.size().y.to_pt().abs();
                    if gh > base_font_pt * 1.35
                        && (Self::group_has_direct_bg_shape(children)
                            || Self::is_cetz_group_nodes(children))
                    {
                        *has_tall_inline_object = true;
                        continue;
                    }
                    Self::walk_paragraph_baselines(
                        children,
                        offset + *pos,
                        base_font_pt,
                        lines,
                        has_tall_inline_object,
                    );
                }
                TreeNode::Element { elem, children, .. } => {
                    if Self::is_page_furniture_elem(elem)
                        || elem.is::<typst::layout::HideElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                        || elem.is::<typst::model::FootnoteElem>()
                    {
                        continue;
                    }
                    if elem.is::<typst::math::EquationElem>() {
                        let (_, _, _, h) = Self::estimate_nodes_bbox(children);
                        if h > base_font_pt * 1.35 {
                            *has_tall_inline_object = true;
                        }
                        continue;
                    }
                    Self::walk_paragraph_baselines(
                        children,
                        offset,
                        base_font_pt,
                        lines,
                        has_tall_inline_object,
                    );
                }
                _ => {}
            }
        }
    }

    /// Record exact surrounding paragraph-line clearances (`InlineBoxLineContextIr`) for every
    /// `InlineNode::ShapeInline` inside a paragraph so `word-construct` can compute exact
    /// `<wp:effectExtent>` clearances without hardcoded constants.
    pub(crate) fn record_inline_shape_line_contexts(
        &mut self,
        nodes: &[TreeNode],
        inlines: &[InlineNode],
        base_font_pt: f64,
    ) {
        let mut shape_ids = Vec::new();
        Self::collect_inline_shape_ids(inlines, &mut shape_ids);
        if shape_ids.is_empty() {
            return;
        }

        let mut raw_text_lines = Vec::new();
        let mut box_frames = Vec::new();
        self.walk_inline_box_line_env(
            nodes,
            Point::zero(),
            base_font_pt,
            &mut raw_text_lines,
            &mut box_frames,
        );

        raw_text_lines.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });
        let mut text_lines: Vec<(usize, f64, f64, Option<String>, f64, bool)> = Vec::new();
        for item in raw_text_lines {
            if let Some(last) = text_lines.last_mut()
                && last.0 == item.0
                && (item.1 - last.1).abs() < 200.0
                && (item.2 - last.2).abs() < 3.0
            {
                last.5 |= item.5;
                continue;
            }
            text_lines.push(item);
        }

        let mut used_frames = vec![false; box_frames.len()];
        for shape_id in shape_ids {
            let target_loc = self.origins.get(&shape_id).and_then(|o| o.typst_location);
            let matched_idx = target_loc
                .and_then(|loc| {
                    box_frames
                        .iter()
                        .enumerate()
                        .find(|(i, f)| !used_frames[*i] && f.0 == Some(loc))
                        .map(|(i, _)| i)
                })
                .or_else(|| {
                    box_frames
                        .iter()
                        .enumerate()
                        .find(|(i, _)| !used_frames[*i])
                        .map(|(i, _)| i)
                });
            let Some(frame_idx) = matched_idx else {
                continue;
            };
            used_frames[frame_idx] = true;
            let (_, page_idx, box_x, box_top_y, box_bot_y) = box_frames[frame_idx];

            let shares_line_with_text = text_lines.iter().any(|&(p, lx, ly, ..)| {
                p == page_idx
                    && (lx - box_x).abs() < 250.0
                    && ly > box_top_y + 0.5
                    && ly <= box_bot_y + 1.5
            });

            let prev_line = text_lines
                .iter()
                .filter(|&&(p, lx, ly, ..)| {
                    p == page_idx && (lx - box_x).abs() < 250.0 && ly <= box_top_y + 0.5
                })
                .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, _, ly, fam, sz, is_cjk)| AdjacentTextLineIr {
                    gap_pt: (box_top_y - *ly).max(0.0),
                    font_family: fam.clone(),
                    font_size_pt: *sz,
                    is_cjk: *is_cjk,
                });

            let next_line = text_lines
                .iter()
                .filter(|&&(p, lx, ly, ..)| {
                    p == page_idx && (lx - box_x).abs() < 250.0 && ly > box_bot_y + 1.5
                })
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, _, ly, fam, sz, is_cjk)| AdjacentTextLineIr {
                    gap_pt: (*ly - box_bot_y).max(0.0),
                    font_family: fam.clone(),
                    font_size_pt: *sz,
                    is_cjk: *is_cjk,
                });

            self.reference_layout
                .entry(shape_id)
                .or_default()
                .inline_box_context = Some(InlineBoxLineContextIr {
                shares_line_with_text,
                prev_line,
                next_line,
            });
        }
    }

    fn collect_inline_shape_ids(inlines: &[InlineNode], out: &mut Vec<NodeId>) {
        for inl in inlines {
            match inl {
                InlineNode::ShapeInline(shape) if shape.id.is_set() => out.push(shape.id),
                InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. }
                | InlineNode::Field { content, .. } => {
                    Self::collect_inline_shape_ids(content, out);
                }
                _ => {}
            }
        }
    }

    fn walk_inline_box_line_env(
        &self,
        nodes: &[TreeNode],
        offset: Point,
        base_font_pt: f64,
        text_lines: &mut Vec<(usize, f64, f64, Option<String>, f64, bool)>,
        box_frames: &mut Vec<(Option<u128>, usize, f64, f64, f64)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text {
                    text,
                    pos,
                    baseline_shift_pt,
                    ..
                } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let abs_x = (offset.x + pos.x).to_pt();
                    let raw_y = (offset.y + pos.y).to_pt() + baseline_shift_pt.unwrap_or(0.0);
                    let (page_idx, local_y, x_pt) =
                        crate::frame_tree::decode_page_and_local_y(raw_y, abs_x);
                    let fam = Some(text.font.info().family.clone());
                    let sz = text.size.to_pt();
                    let has_cjk = text.text.chars().any(is_cjk_char);
                    text_lines.push((page_idx, x_pt, local_y, fam, sz, has_cjk));
                }
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if Self::is_page_furniture_elem(elem)
                        || elem.is::<typst::layout::HideElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                        || elem.is::<typst::model::FootnoteElem>()
                        || elem.is::<typst::math::EquationElem>()
                    {
                        continue;
                    }
                    if let Some(box_elem) = elem.to_packed::<typst::layout::BoxElem>() {
                        let chain = self.scoped_style_chain(Some(*loc));
                        let has_fill_or_stroke = box_elem.fill.get_ref(chain).is_some()
                            || box_elem
                                .stroke
                                .resolve(chain)
                                .iter()
                                .any(|s| s.as_ref().is_some_and(|o| o.is_some()));
                        if has_fill_or_stroke
                            && let Some((gpos, gw, gh)) = children.iter().find_map(|c| match c {
                                TreeNode::Group { group, pos: gp, .. } => {
                                    Some((*gp, group.frame.size().x.to_pt(), group.frame.size().y.to_pt()))
                                }
                                _ => None,
                            })
                        {
                            // Record spatial box bounds and do not descend into its inner text.
                            if gh > base_font_pt * 1.35 || gw > 24.0 {
                                let abs_x = (offset.x + gpos.x).to_pt();
                                let raw_y = (offset.y + gpos.y).to_pt();
                                let (p, top_y, x_pt) =
                                    crate::frame_tree::decode_page_and_local_y(raw_y, abs_x);
                                box_frames.push((Some(loc.hash()), p, x_pt, top_y, top_y + gh));
                                continue;
                            }
                        }
                    }
                    if Self::is_native_shape_elem(elem) {
                        let (spos, sh) = children
                            .iter()
                            .find_map(|c| match c {
                                TreeNode::Group { group, pos: gp, .. } => {
                                    Some((*gp, group.frame.size().y.to_pt()))
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| {
                                let (_, _, _, h) = Self::estimate_nodes_bbox(children);
                                (*pos, h)
                            });
                        let abs_x = (offset.x + spos.x).to_pt();
                        let raw_y = (offset.y + spos.y).to_pt();
                        let (p, top_y, x_pt) =
                            crate::frame_tree::decode_page_and_local_y(raw_y, abs_x);
                        box_frames.push((Some(loc.hash()), p, x_pt, top_y, top_y + sh));
                        continue;
                    }
                    self.walk_inline_box_line_env(
                        children,
                        offset,
                        base_font_pt,
                        text_lines,
                        box_frames,
                    );
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let gh = group.frame.size().y.to_pt().abs();
                    if gh > base_font_pt * 1.35 && Self::group_has_direct_bg_shape(children) {
                        let abs_x = (offset.x + pos.x).to_pt();
                        let raw_y = (offset.y + pos.y).to_pt();
                        let (p, top_y, x_pt) =
                            crate::frame_tree::decode_page_and_local_y(raw_y, abs_x);
                        box_frames.push((None, p, x_pt, top_y, top_y + gh));
                        continue;
                    }
                    self.walk_inline_box_line_env(
                        children,
                        offset + *pos,
                        base_font_pt,
                        text_lines,
                        box_frames,
                    );
                }
                _ => {}
            }
        }
    }
}
