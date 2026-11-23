//! `equation` -> `EquationIr` (pure Typst math AST + `EquationPdfMetricsIr`).

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::layout::Point;
use typst::math::EquationElem;
use typst::model::{Numbering, ParElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use crate::inline::ActiveInlineState;
use crate::math::{EquationFonts, extract_equation_math_nodes};
use ir::{EquationIr, EquationLayoutMetricsIr, SemanticRoleIr, TextRunIr};

#[derive(Default)]
struct EquationTypography {
    number: Vec<TextRunIr>,
    fonts: EquationFonts,
    context_font_size_pt: f64,
    max_glyph_pt: Option<f64>,
    min_glyph_pt: Option<f64>,
    math_font: Option<String>,
    cjk_font: Option<String>,
}

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_equation(
        &mut self,
        eq: &Packed<EquationElem>,
        loc: Location,
        children: &[TreeNode],
    ) -> EquationIr {
        let chain = self.scoped_style_chain(Some(loc));
        let is_block = eq.block.get(chain);
        let cfg_numbering = eq.numbering.get_cloned(chain);
        let number_span = (is_block && cfg_numbering.is_some() && !eq.span().is_detached())
            .then_some(eq.span());
        let typo = Self::resolve_equation_typography(children, chain, number_span);
        let measured_bbox = Self::measure_equation_bbox(
            children,
            is_block,
            typo.max_glyph_pt.unwrap_or(typo.context_font_size_pt),
        );

        let body = extract_equation_math_nodes(
            self.compiled,
            self.world,
            eq,
            chain,
            Some(typo.context_font_size_pt),
            typo.math_font.as_deref(),
            typo.cjk_font.as_deref(),
            &typo.fonts,
        );
        let max_available_width_pt =
            Some((self.current_column_width_pt() - self.scope.container_inset_pt).max(60.0));

        let layout_metrics = EquationLayoutMetricsIr::from_bbox(
            measured_bbox,
            typo.context_font_size_pt,
            typo.max_glyph_pt,
            typo.min_glyph_pt,
            max_available_width_pt,
        );

        let base_font_pt = match (typo.max_glyph_pt, is_block) {
            (Some(max_g), false)
                if (max_g / typo.context_font_size_pt.max(6.0) - 0.70).abs() < 0.09 =>
            {
                typo.context_font_size_pt
            }
            (Some(max_g), _) => max_g,
            (None, _) => typo.context_font_size_pt,
        };

        let source_text = self
            .compiled
            .source_snippet(self.world, eq.span())
            .unwrap_or_else(|| eq.body.plain_text().to_string());
        let bookmark = self.bookmarks.get(&loc).cloned();

        // Extract resolved equation number (e.g. `(1)`, `(2)`, `(3)`) from laid-out `children`
        // or from the active `#set math.equation(numbering: ...)` pattern.
        let mut numbering_runs = typo.number;
        let numbering_text = if is_block {
            if !numbering_runs.is_empty() {
                self.next_equation_number += 1;
                Some(numbering_runs.iter().map(|run| run.text.as_str()).collect())
            } else if let Some((found, runs)) = Self::find_equation_number(children) {
                numbering_runs = runs;
                self.next_equation_number += 1;
                Some(found)
            } else if let Some(numbering) = cfg_numbering {
                let n = self.next_equation_number as u64;
                self.next_equation_number += 1;
                match numbering {
                    Numbering::Pattern(pat) => pat
                        .apply(None, &[n])
                        .ok()
                        .map(|s| s.to_string())
                        .or_else(|| Some(format!("({n})"))),
                    Numbering::Func(_) => Some(format!("({n})")),
                }
            } else {
                None
            }
        } else {
            None
        };

        if self.math_font_family.is_none() {
            self.math_font_family.clone_from(&typo.math_font);
        }

        let id = self.alloc_node_with_fragments(
            eq.span(),
            Some(loc),
            Some(SemanticRoleIr::Equation {
                is_block,
                numbering: numbering_text.clone(),
            }),
            children,
        );

        EquationIr {
            id,
            is_block,
            body,
            source_text,
            numbering_text,
            numbering_runs,
            math_font_family: typo.math_font,
            bookmark,
            font_size_pt: Some((base_font_pt * 2.0).round() / 2.0),
            layout_metrics,
        }
    }

    fn resolve_equation_typography(
        nodes: &[TreeNode],
        chain: typst::foundations::StyleChain,
        number_span: Option<typst::syntax::Span>,
    ) -> EquationTypography {
        let mut typo = EquationTypography {
            context_font_size_pt: Self::resolve_length_pt(
                chain.get(typst::text::TextElem::size).0,
                11.0,
            ),
            ..Default::default()
        };
        Self::scan_equation_text_nodes(nodes, &mut typo, number_span);
        if typo.cjk_font.is_none() {
            typo.cjk_font = chain
                .get_ref(typst::text::TextElem::font)
                .into_iter()
                .find(|fam| Self::is_cjk_font_family(fam.as_str()))
                .map(|fam| fam.as_str().to_string());
        }
        typo
    }

    fn scan_equation_text_nodes(
        nodes: &[TreeNode],
        typo: &mut EquationTypography,
        number_span: Option<typst::syntax::Span>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, .. } => {
                    // Typst assigns the displayed counter the span of the whole equation.
                    if !text.glyphs.is_empty()
                        && text.glyphs.iter().all(|g| Some(g.span.0) == number_span)
                    {
                        if let Some(run) = Self::equation_number_run(text) {
                            typo.number.push(run);
                        }
                        continue;
                    }
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let sz = text.size.to_pt();
                    if sz > 1.0 {
                        let sz = (sz * 10.0).round() / 10.0;
                        typo.max_glyph_pt = Some(typo.max_glyph_pt.map_or(sz, |max| sz.max(max)));
                        typo.min_glyph_pt = Some(typo.min_glyph_pt.map_or(sz, |min| sz.min(min)));
                    }
                    let family = text.font.info().family.as_str();
                    if typo.cjk_font.is_none()
                        && (text.text.chars().any(ir::is_cjk_char)
                            || Self::is_cjk_font_family(family))
                    {
                        typo.cjk_font = Some(family.to_owned());
                    }
                    let math = crate::font_embed::math_family(&text.font);
                    if typo.math_font.is_none() {
                        typo.math_font.clone_from(&math);
                    }
                    typo.fonts.record(text, math.as_deref());
                }
                TreeNode::Group { pos, children, .. } => {
                    if pos.x.to_pt() > 220.0 {
                        let txt = Self::nodes_to_plain_text(children)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        if Self::looks_like_equation_number(&txt) {
                            continue;
                        }
                    }
                    Self::scan_equation_text_nodes(children, typo, number_span);
                }
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>()
                        && matches!(marker.kind, PdfMarkerTagKind::Label)
                    {
                        continue;
                    }
                    Self::scan_equation_text_nodes(children, typo, number_span);
                }
                _ => {}
            }
        }
    }

    /// Measure the tight bounding box `(width_pt, height_pt)` of an equation from its
    /// laid-out `TreeNode`s, excluding equation numbering labels and detecting line-wrapped
    /// inline equations.
    fn measure_equation_bbox(
        nodes: &[TreeNode],
        is_block: bool,
        ref_font_pt: f64,
    ) -> Option<(f64, f64)> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut first_page_idx: Option<usize> = None;
        let mut last_text_pos: Option<(f64, f64)> = None;
        let mut wrapped_inline = false;

        Self::accumulate_equation_bbox(
            nodes,
            Point::zero(),
            is_block,
            ref_font_pt.max(6.0),
            &mut first_page_idx,
            &mut last_text_pos,
            &mut wrapped_inline,
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
        );

        if wrapped_inline || !min_x.is_finite() || !max_x.is_finite() {
            return None;
        }
        let w = (max_x - min_x).max(0.0);
        let h = (max_y - min_y).max(0.0);
        if w > 0.5 && h > 0.5 {
            Some(((w * 100.0).round() / 100.0, (h * 100.0).round() / 100.0))
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn accumulate_equation_bbox(
        nodes: &[TreeNode],
        offset: Point,
        is_block: bool,
        ref_font_pt: f64,
        first_page_idx: &mut Option<usize>,
        last_text_pos: &mut Option<(f64, f64)>,
        wrapped_inline: &mut bool,
        min_x: &mut f64,
        min_y: &mut f64,
        max_x: &mut f64,
        max_y: &mut f64,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos, .. } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let x = (offset.x + pos.x).to_pt();
                    let raw_y = (offset.y + pos.y).to_pt().max(0.0);
                    let page_idx = (raw_y / 10000.0).floor() as usize;
                    let page = *first_page_idx.get_or_insert(page_idx);
                    if page_idx != page {
                        continue;
                    }
                    let baseline_y = raw_y - page_idx as f64 * 10000.0;
                    if !is_block
                        && let Some((prev_x, prev_y)) = *last_text_pos
                        && baseline_y > prev_y + ref_font_pt * 0.95
                        && x < prev_x - 15.0
                    {
                        *wrapped_inline = true;
                    }
                    *last_text_pos = Some((x, baseline_y));

                    let sz = text.size.to_pt();
                    let y_top = baseline_y - sz * 0.78;
                    let y_bot = baseline_y + sz * 0.22;
                    *min_x = min_x.min(x);
                    *min_y = min_y.min(y_top);
                    *max_x = max_x.max(x + text.width().to_pt());
                    *max_y = max_y.max(y_bot);
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let x1 = (offset.x + pos.x + b.min.x).to_pt();
                    let x2 = (offset.x + pos.x + b.max.x).to_pt();
                    let raw_y1 = (offset.y + pos.y + b.min.y).to_pt().max(0.0);
                    let raw_y2 = (offset.y + pos.y + b.max.y).to_pt().max(0.0);
                    let page_idx = (raw_y1 / 10000.0).floor() as usize;
                    let page = *first_page_idx.get_or_insert(page_idx);
                    if page_idx != page {
                        continue;
                    }
                    let y1 = raw_y1 - page_idx as f64 * 10000.0;
                    let y2 = raw_y2 - page_idx as f64 * 10000.0;
                    *min_x = min_x.min(x1.min(x2));
                    *min_y = min_y.min(y1.min(y2));
                    *max_x = max_x.max(x1.max(x2));
                    *max_y = max_y.max(y1.max(y2));
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if is_block && pos.x.to_pt() > 220.0 {
                        let txt = Self::nodes_to_plain_text(children)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        if Self::looks_like_equation_number(&txt) {
                            continue;
                        }
                    }
                    let next_offset =
                        offset + *pos + Point::new(group.transform.tx, group.transform.ty);
                    if !children.is_empty() {
                        Self::accumulate_equation_bbox(
                            children,
                            next_offset,
                            is_block,
                            ref_font_pt,
                            first_page_idx,
                            last_text_pos,
                            wrapped_inline,
                            min_x,
                            min_y,
                            max_x,
                            max_y,
                        );
                    } else {
                        let gsz = group.frame.size();
                        if gsz.x.to_pt() > 0.0 && gsz.y.to_pt() > 0.0 {
                            let gx = next_offset.x.to_pt();
                            let raw_gy = next_offset.y.to_pt().max(0.0);
                            let page_idx = (raw_gy / 10000.0).floor() as usize;
                            let page = *first_page_idx.get_or_insert(page_idx);
                            if page_idx == page {
                                let gy = raw_gy - page_idx as f64 * 10000.0;
                                *min_x = min_x.min(gx);
                                *min_y = min_y.min(gy);
                                *max_x = max_x.max(gx + gsz.x.to_pt());
                                *max_y = max_y.max(gy + gsz.y.to_pt());
                            }
                        }
                    }
                }
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>()
                        && matches!(marker.kind, PdfMarkerTagKind::Label)
                    {
                        continue;
                    }
                    Self::accumulate_equation_bbox(
                        children,
                        offset,
                        is_block,
                        ref_font_pt,
                        first_page_idx,
                        last_text_pos,
                        wrapped_inline,
                        min_x,
                        min_y,
                        max_x,
                        max_y,
                    );
                }
                _ => {}
            }
        }
    }

    pub(crate) fn is_cjk_font_family(fam: &str) -> bool {
        let low = fam.trim().to_lowercase();
        low.contains("pingfang")
            || low.contains("song")
            || low.contains("simsun")
            || low.contains("hei")
            || low.contains("yahei")
            || low.contains("kai")
            || low.contains("fangsong")
            || low.contains("source han")
            || low.contains("noto serif cjk")
            || low.contains("noto sans cjk")
            || low.contains("苹方")
            || low.contains("宋")
            || low.contains("黑")
            || low.contains("楷")
            || low.contains("仿宋")
    }

    fn equation_number_run(text: &typst::text::TextItem) -> Option<TextRunIr> {
        let mut run = Self::text_item_to_run(text, &ActiveInlineState::default());
        run.text = run.text.replace('\u{2060}', "");
        if let Some(family) = crate::font_embed::math_family(&text.font) {
            run.font_family = Some(family);
        }
        (!run.text.is_empty()).then_some(run)
    }

    fn find_equation_number(nodes: &[TreeNode]) -> Option<(String, Vec<TextRunIr>)> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    let label = elem
                        .to_packed::<PdfMarkerTag>()
                        .is_some_and(|m| matches!(m.kind, PdfMarkerTagKind::Label));
                    if (label || elem.is::<ParElem>())
                        && let Some(number) = Self::equation_label_runs(children, None)
                    {
                        return Some(number);
                    }
                    if let Some(number) = Self::find_equation_number(children) {
                        return Some(number);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    if let Some(number) = Self::find_equation_number(children) {
                        return Some(number);
                    }
                    if pos.x.to_pt() > 220.0
                        && let Some(number) = Self::equation_label_runs(children, None)
                        && Self::looks_like_equation_number(&number.0)
                    {
                        return Some(number);
                    }
                }
                _ => {}
            }
        }
        Self::equation_label_runs(nodes, Some(260.0))
            .filter(|(text, _)| Self::looks_like_equation_number(text))
    }

    fn equation_label_runs(
        nodes: &[TreeNode],
        min_x: Option<f64>,
    ) -> Option<(String, Vec<TextRunIr>)> {
        fn collect(nodes: &[TreeNode], min_x: Option<f64>, runs: &mut Vec<TextRunIr>) {
            for node in nodes {
                match node {
                    TreeNode::Text { text, pos, .. } if min_x.is_none_or(|x| pos.x.to_pt() > x) => {
                        if let Some(run) = ExtractorContext::equation_number_run(text) {
                            runs.push(run);
                        }
                    }
                    TreeNode::Group { children, pos, .. }
                    | TreeNode::Element { children, pos, .. } => {
                        collect(children, min_x.map(|x| x - pos.x.to_pt()), runs);
                    }
                    _ => {}
                }
            }
        }
        let mut runs = Vec::new();
        collect(nodes, min_x, &mut runs);
        let label: String = runs.iter().map(|run| run.text.as_str()).collect();
        let trimmed = label.trim();
        (!trimmed.is_empty()).then(|| (trimmed.to_string(), runs))
    }

    pub(crate) fn looks_like_equation_number(s: &str) -> bool {
        if s.len() < 2 || s.len() > 16 {
            return false;
        }
        let has_parens =
            (s.starts_with('(') && s.ends_with(')')) || (s.starts_with('[') && s.ends_with(']'));
        has_parens && s.chars().any(|c| c.is_ascii_alphanumeric())
    }
}
