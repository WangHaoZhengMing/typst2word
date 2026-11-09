//! `equation` -> `EquationIr` (pure Typst math AST + `EquationPdfMetricsIr`).

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::layout::Point;
use typst::math::EquationElem;
use typst::model::{Numbering, ParElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use crate::math::extract_equation_math_nodes;
use ir::{EquationIr, EquationPdfMetricsIr};

struct EquationTypography {
    context_font_size_pt: f64,
    max_glyph_pt: Option<f64>,
    min_glyph_pt: Option<f64>,
    math_font: Option<&'static str>,
    cjk_font: Option<&'static str>,
}

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_equation(
        &mut self,
        eq: &Packed<EquationElem>,
        loc: Location,
        children: &[TreeNode],
    ) -> EquationIr {
        let chain = self.style_chain();
        let is_block = eq.block.get(chain);
        let typo = Self::resolve_equation_typography(children, chain);
        let measured_bbox = Self::measure_equation_bbox(
            children,
            is_block,
            typo.max_glyph_pt.unwrap_or(typo.context_font_size_pt),
        );

        let body = extract_equation_math_nodes(
            eq,
            chain,
            Some(typo.context_font_size_pt),
            typo.math_font,
            typo.cjk_font,
        );
        let max_available_width_pt =
            Some((self.current_column_width_pt() - self.container_inset_pt).max(60.0));

        let pdf_metrics = EquationPdfMetricsIr::from_bbox(
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
        let cfg_numbering = eq.numbering.get_cloned(chain);
        let numbering_text = if is_block {
            if let Some(found) = Self::find_equation_number_text(children) {
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

        EquationIr {
            is_block,
            body,
            source_text,
            numbering_text,
            bookmark,
            font_size_pt: Some((base_font_pt * 2.0).round() / 2.0),
            pdf_metrics,
        }
    }

    fn resolve_equation_typography(
        nodes: &[TreeNode],
        chain: typst::foundations::StyleChain,
    ) -> EquationTypography {
        let mut max_size_pt = 0.0_f64;
        let mut min_size_pt = f64::INFINITY;
        let mut math_font: Option<&'static str> = None;
        let mut cjk_font: Option<&'static str> = None;
        Self::scan_equation_text_nodes(
            nodes,
            &mut max_size_pt,
            &mut min_size_pt,
            &mut math_font,
            &mut cjk_font,
        );

        let context_font_size_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let max_glyph_pt = if max_size_pt > 1.0 {
            Some((max_size_pt * 10.0).round() / 10.0)
        } else {
            None
        };
        let min_glyph_pt = if min_size_pt.is_finite() && min_size_pt > 1.0 {
            Some((min_size_pt * 10.0).round() / 10.0)
        } else {
            None
        };

        if cjk_font.is_none() {
            for fam in chain.get_ref(typst::text::TextElem::font).into_iter() {
                if let Some(mapped) = Self::classify_cjk_font_static(fam.as_str()) {
                    cjk_font = Some(mapped);
                    break;
                }
            }
        }

        EquationTypography {
            context_font_size_pt,
            max_glyph_pt,
            min_glyph_pt,
            math_font,
            cjk_font,
        }
    }

    fn scan_equation_text_nodes(
        nodes: &[TreeNode],
        max_size_pt: &mut f64,
        min_size_pt: &mut f64,
        math_font: &mut Option<&'static str>,
        cjk_font: &mut Option<&'static str>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, .. } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let sz = text.size.to_pt();
                    if sz > *max_size_pt {
                        *max_size_pt = sz;
                    }
                    if sz > 1.0 && sz < *min_size_pt {
                        *min_size_pt = sz;
                    }
                    let fam = text.font.info().family.as_str();
                    if cjk_font.is_none()
                        && let Some(cjk) = Self::classify_cjk_font_static(fam)
                    {
                        *cjk_font = Some(cjk);
                    } else if math_font.is_none()
                        && let Some(mf) = Self::classify_math_font_static(fam)
                    {
                        *math_font = Some(mf);
                    }
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
                    Self::scan_equation_text_nodes(
                        children,
                        max_size_pt,
                        min_size_pt,
                        math_font,
                        cjk_font,
                    );
                }
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>()
                        && matches!(marker.kind, PdfMarkerTagKind::Label)
                    {
                        continue;
                    }
                    Self::scan_equation_text_nodes(
                        children,
                        max_size_pt,
                        min_size_pt,
                        math_font,
                        cjk_font,
                    );
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
                TreeNode::Text { text, pos } => {
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

    pub(crate) fn classify_cjk_font_static(fam: &str) -> Option<&'static str> {
        let low = fam.trim().to_ascii_lowercase();
        if low.contains("pingfang") {
            Some("PingFang SC")
        } else if low.contains("song") || low.contains("simsun") {
            Some("Songti SC")
        } else if low.contains("hei") || low.contains("yahei") {
            Some("Heiti SC")
        } else if low.contains("kai") {
            Some("Kaiti SC")
        } else if low.contains("fangsong") {
            Some("Fangsong SC")
        } else {
            None
        }
    }

    pub(crate) fn classify_math_font_static(fam: &str) -> Option<&'static str> {
        let low = fam.trim().to_ascii_lowercase();
        if low.contains("stix") || low.contains("xits") {
            Some("STIX Two Math")
        } else if low.contains("cambria") {
            Some("Cambria Math")
        } else if low.contains("fira") {
            Some("Fira Math")
        } else if low.contains("libertinus") {
            Some("Libertinus Math")
        } else if low.contains("computer modern")
            || low.contains("newcm")
            || low.contains("latin modern")
        {
            Some("New Computer Modern Math")
        } else {
            None
        }
    }

    pub(crate) fn find_equation_number_text(nodes: &[TreeNode]) -> Option<String> {
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
                    if elem.is::<ParElem>() {
                        let txt = Self::nodes_to_plain_text(children)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        if !txt.is_empty() {
                            return Some(txt);
                        }
                    }
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                    if pos.x.to_pt() > 220.0 {
                        let txt = Self::nodes_to_plain_text(children)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        if Self::looks_like_equation_number(&txt) {
                            return Some(txt);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut right_text = String::new();
        Self::collect_right_edge_equation_text(nodes, 0.0, &mut right_text);
        let trimmed = right_text.replace('\u{2060}', "").trim().to_string();
        if Self::looks_like_equation_number(&trimmed) {
            return Some(trimmed);
        }
        None
    }

    pub(crate) fn collect_right_edge_equation_text(
        nodes: &[TreeNode],
        offset_x_pt: f64,
        out: &mut String,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if offset_x_pt + pos.x.to_pt() > 260.0 {
                        out.push_str(&text.text);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_right_edge_equation_text(
                        children,
                        offset_x_pt + pos.x.to_pt(),
                        out,
                    );
                }
                TreeNode::Element { pos, children, .. } => {
                    Self::collect_right_edge_equation_text(
                        children,
                        offset_x_pt + pos.x.to_pt(),
                        out,
                    );
                }
                _ => {}
            }
        }
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
