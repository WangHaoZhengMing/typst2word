//! Block `raw` -> `BlockNode::CodeBlock` with syntax-highlighted runs per line.

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::syntax::Span;
use typst::text::{RawElem, RawLine};
use typst::visualize::Geometry;

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use crate::inline::ActiveInlineState;
use ir::{BlockLayoutSpanIr, BlockNode, ParagraphLineMetricsIr, SemanticRoleIr, StrokeIr, TextRunIr};
use typst::layout::Point;

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Code Blocks, Figures, Equations, Outlines, Bibliographies
    // =========================================================================
    pub(crate) fn extract_code_block(
        &mut self,
        raw: &Packed<RawElem>,
        loc: Location,
        children: &[TreeNode],
    ) -> BlockNode {
        let (lang, fallback_code_font_pt) = {
            let chain = self.scoped_style_chain(Some(loc));
            let lang = raw.lang.get_ref(chain).as_ref().map(|s| s.to_string());
            let sz = Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 9.6);
            (lang, sz)
        };
        let id = self.alloc_node_with_fragments(
            raw.span(),
            Some(loc),
            Some(SemanticRoleIr::CodeBlock { lang: lang.clone() }),
            children,
        );

        let mut lines: Vec<Vec<TextRunIr>> = Vec::new();
        Self::collect_raw_lines(children, &mut lines);
        Self::normalize_empty_code_line_metrics(&mut lines);

        // Record laid-out baseline metrics (`first_baseline_y_pt`, `uniform_baseline_step_pt`)
        // directly from `RawElem`'s descendant `Text` leaves (descending through any `#show raw:
        // block.with(fill: ...)` background group!) so `word-construct` solves exact `w:line`.
        let code_font_pt = lines
            .iter()
            .flatten()
            .find_map(|r| r.font_size_pt)
            .unwrap_or(fallback_code_font_pt)
            .max(6.0);
        if let Some(mut line_metrics) = self.collect_code_line_metrics(children, code_font_pt)
        {
            // Blank lines inside a `CodeBlock` skip baselines (`2 * step`, `3 * step`, ...);
            // when all lines lie in one fragment, compute exact single-line step from first..last
            // non-empty line indices if needed.
            let single_page = self
                .reference_layout
                .get(&id)
                .is_some_and(|l| l.fragments.len() == 1);
            if single_page
                && let (Some(first_idx), Some(last_idx)) = (
                    lines
                        .iter()
                        .position(|l| l.iter().any(|r| !r.text.trim().is_empty())),
                    lines
                        .iter()
                        .rposition(|l| l.iter().any(|r| !r.text.trim().is_empty())),
                )
                && last_idx > first_idx
            {
                let span_dy = line_metrics.last_baseline_y_pt - line_metrics.first_baseline_y_pt;
                let exact_step = span_dy / (last_idx - first_idx) as f64;
                if exact_step >= code_font_pt * 0.80 && exact_step <= code_font_pt * 3.0 {
                    line_metrics.uniform_baseline_step_pt = Some(exact_step);
                }
            }
            self.reference_layout.entry(id).or_default().paragraph_lines = Some(line_metrics);
        }

        // Fallback if no `RawLine` tags were found: use `raw.lines` or `raw.text`.
        if lines.is_empty() {
            if let Some(raw_lines) = raw.lines.as_deref() {
                for rl in raw_lines {
                    lines.push(vec![TextRunIr {
                        text: rl.text.to_string(),
                        font_family: None,
                        is_code: true,
                        ..Default::default()
                    }]);
                }
            }
        }

        // Only attach background fill or border stroke if the code block actually has a background shape!
        let (fill, stroke) = Self::find_code_block_box_style(children);

        BlockNode::CodeBlock {
            id,
            lang,
            lines,
            fill,
            stroke,
        }
    }

    pub(crate) fn find_code_block_box_style(
        nodes: &[TreeNode],
    ) -> (Option<String>, Option<StrokeIr>) {
        // Typst emits separate background fill and border stroke `Geometry::Curve` shapes for
        // `#block(fill: ..., stroke: ..., radius: ...)`, so accumulate both across siblings.
        let mut found_fill = None;
        let mut found_stroke = None;
        for node in nodes {
            match node {
                TreeNode::Shape { shape, .. } => {
                    let sz = shape.geometry.bbox(None).size();
                    if matches!(shape.geometry, Geometry::Rect(_) | Geometry::Curve(_))
                        && sz.x.to_pt().abs() > 20.0
                        && sz.y.to_pt().abs() > 6.0
                    {
                        if found_fill.is_none() {
                            found_fill = shape.fill.as_ref().and_then(paint_to_hex);
                        }
                        if found_stroke.is_none() {
                            found_stroke = shape.stroke.as_ref().map(Self::fixed_stroke_to_ir);
                        }
                    }
                }
                TreeNode::Group { children, .. } | TreeNode::Element { children, .. } => {
                    let (f, s) = Self::find_code_block_box_style(children);
                    if found_fill.is_none() {
                        found_fill = f;
                    }
                    if found_stroke.is_none() {
                        found_stroke = s;
                    }
                }
                _ => {}
            }
            if found_fill.is_some() && found_stroke.is_some() {
                break;
            }
        }
        (found_fill, found_stroke)
    }

    pub(crate) fn collect_raw_lines(nodes: &[TreeNode], out: &mut Vec<Vec<TextRunIr>>) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(raw_line) = elem.to_packed::<RawLine>() {
                        let mut runs = Vec::new();
                        Self::collect_raw_line_runs(children, &mut runs);
                        if runs.is_empty() {
                            // Only synthesize a blank `TextRunIr` when the source line itself is
                            // empty/whitespace; if `raw_line.text` is non-empty yet `children` has
                            // zero laid-out glyphs, it is an empty cross-page `Tag::Start(RawLine)`
                            // shell whose glyphs moved onto the continuation page.
                            if !raw_line.text.trim().is_empty() {
                                continue;
                            }
                            runs.push(TextRunIr {
                                text: raw_line.text.to_string(),
                                font_family: None,
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

    pub(crate) fn collect_raw_line_runs(nodes: &[TreeNode], runs: &mut Vec<TextRunIr>) {
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

    pub(crate) fn normalize_empty_code_line_metrics(lines: &mut [Vec<TextRunIr>]) {
        let sample = lines.iter().flatten().find_map(|r| {
            r.font_size_pt
                .map(|sz| (r.font_family.clone(), sz))
        });
        if let Some((fam, sz)) = sample {
            for line in lines.iter_mut() {
                for r in line.iter_mut() {
                    if r.font_size_pt.is_none() {
                        r.font_size_pt = Some(sz);
                    }
                    if r.font_family.is_none() {
                        r.font_family = fam.clone();
                    }
                }
            }
        }
    }

    /// Append an orphaned cross-page `RawLine` to the trailing `CodeBlock` (or create a new
    /// continuation `CodeBlock`), merging page fragments and updating laid-out baseline metrics
    /// (`first_baseline_y_pt`, `last_baseline_y_pt`, `uniform_baseline_step_pt`) so continuation
    /// code blocks keep exact fixed-pitch `w:lineRule="exact"` spacing.
    pub(crate) fn append_or_create_orphan_raw_line(
        &mut self,
        span: Span,
        loc: Option<Location>,
        source_nodes: &[TreeNode],
        runs: Vec<TextRunIr>,
        blocks: &mut Vec<BlockNode>,
    ) {
        let code_font_pt = runs
            .iter()
            .find_map(|r| r.font_size_pt)
            .unwrap_or(9.6)
            .max(6.0);
        let new_line_pm = self.collect_code_line_metrics(source_nodes, code_font_pt);
        let new_frags = self.collect_page_fragments(source_nodes);

        let (code_id, non_empty_span) =
            if let Some(BlockNode::CodeBlock { id, lines, .. }) = blocks.last_mut() {
                lines.push(runs);
                Self::normalize_empty_code_line_metrics(lines);
                let first_idx = lines
                    .iter()
                    .position(|l| l.iter().any(|r| !r.text.trim().is_empty()));
                let last_idx = lines
                    .iter()
                    .rposition(|l| l.iter().any(|r| !r.text.trim().is_empty()));
                (*id, first_idx.zip(last_idx))
            } else {
                let mut lines = vec![runs];
                Self::normalize_empty_code_line_metrics(&mut lines);
                let has_non_empty = lines[0].iter().any(|r| !r.text.trim().is_empty());
                let id = self.alloc_node_with_fragments(
                    span,
                    loc,
                    Some(SemanticRoleIr::CodeBlock { lang: None }),
                    source_nodes,
                );
                blocks.push(BlockNode::CodeBlock {
                    id,
                    lang: None,
                    lines,
                    fill: None,
                    stroke: None,
                });
                (id, has_non_empty.then_some((0, 0)))
            };

        let entry = self.reference_layout.entry(code_id).or_default();
        for nf in new_frags {
            if let Some(ef) = entry.fragments.iter_mut().find(|f| f.page_idx == nf.page_idx) {
                let min_x = ef.x_pt.min(nf.x_pt);
                let min_y = ef.top_y_pt.min(nf.top_y_pt);
                let max_x = (ef.x_pt + ef.width_pt).max(nf.x_pt + nf.width_pt);
                let max_y = (ef.top_y_pt + ef.height_pt).max(nf.top_y_pt + nf.height_pt);
                ef.x_pt = min_x;
                ef.top_y_pt = min_y;
                ef.width_pt = (max_x - min_x).max(0.0);
                ef.height_pt = (max_y - min_y).max(0.0);
            } else {
                entry.fragments.push(nf);
            }
        }
        if !entry.fragments.is_empty() {
            entry.span = BlockLayoutSpanIr::from_fragments(&entry.fragments);
        }

        if let Some(pm) = new_line_pm {
            let single_page = entry.fragments.len() <= 1;
            match entry.paragraph_lines.as_mut() {
                Some(existing) => {
                    existing.line_count += pm.line_count;
                    existing.last_baseline_y_pt = pm.last_baseline_y_pt;
                    if single_page
                        && let Some((first_idx, last_idx)) = non_empty_span
                        && last_idx > first_idx
                    {
                        let dy = existing.last_baseline_y_pt - existing.first_baseline_y_pt;
                        let step = dy / (last_idx - first_idx) as f64;
                        if step >= code_font_pt * 0.80 && step <= code_font_pt * 3.0 {
                            existing.uniform_baseline_step_pt = Some(step);
                        }
                    }
                }
                None => {
                    entry.paragraph_lines = Some(pm);
                }
            }
        }
    }

    /// Measure laid-out `RawElem` visual line baselines directly from descendant `Text` leaves,
    /// descending into `#show raw: block.with(fill: ...)` or `#show raw: box.with(fill: ...)`
    /// background groups (which `collect_paragraph_line_metrics` skips inside `ParElem`).
    fn collect_code_line_metrics(
        &self,
        nodes: &[TreeNode],
        code_font_pt: f64,
    ) -> Option<ParagraphLineMetricsIr> {
        let mut raw_lines: Vec<(usize, f64, f64)> = Vec::new();
        Self::walk_code_baselines(nodes, self.scope.group_origin_offset, &mut raw_lines);
        if raw_lines.is_empty() {
            return None;
        }

        let mut lines: Vec<(usize, f64, f64)> = Vec::with_capacity(raw_lines.len());
        let mut col_start = 0usize;
        for (p, x, y) in raw_lines {
            if let Some(&(last_p, _, last_y)) = lines.last()
                && (p != last_p || y < last_y - 3.0)
            {
                col_start = lines.len();
            }
            if let Some(existing) = lines[col_start..]
                .iter_mut()
                .find(|(lp, _, ly)| *lp == p && (*ly - y).abs() < 3.0)
            {
                existing.1 = existing.1.min(x);
            } else {
                lines.push((p, x, y));
            }
        }

        let line_count = lines.len();
        let first_baseline_y_pt = lines.first()?.2;
        let last_baseline_y_pt = lines.last()?.2;
        let min_step = code_font_pt * 0.80;
        let max_step = code_font_pt * 3.50;
        let mut steps: Vec<f64> = Vec::new();
        for pair in lines.windows(2) {
            let (p0, _, y0) = pair[0];
            let (p1, _, y1) = pair[1];
            if p0 == p1 && y1 > y0 {
                let dy = y1 - y0;
                if dy >= min_step && dy <= max_step {
                    steps.push(dy);
                }
            }
        }
        steps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let uniform_baseline_step_pt = (!steps.is_empty()).then(|| steps[steps.len() / 2]);

        Some(ParagraphLineMetricsIr {
            line_count,
            first_baseline_y_pt,
            last_baseline_y_pt,
            uniform_baseline_step_pt,
            has_tall_inline_object: false,
        })
    }

    fn walk_code_baselines(
        nodes: &[TreeNode],
        offset: Point,
        lines: &mut Vec<(usize, f64, f64)>,
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
                TreeNode::Group { pos, children, .. } => {
                    Self::walk_code_baselines(children, offset + *pos, lines);
                }
                TreeNode::Element { elem, children, .. } => {
                    if Self::is_page_furniture_elem(elem)
                        || elem.is::<typst::layout::HideElem>()
                    {
                        continue;
                    }
                    Self::walk_code_baselines(children, offset, lines);
                }
                _ => {}
            }
        }
    }
}
