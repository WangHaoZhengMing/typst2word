//! Document-level extraction: metadata, page setup (size, margins, columns, fill),
//! header / footer artifacts and page-break bookkeeping.

use typst::foundations::{Content, NativeElement as _, SequenceElem, Smart, StyleChain, StyledElem};
use typst::layout::{PageElem, PagebreakElem};
use typst::model::Document;
use typst::pdf::{ArtifactElem, ArtifactKind};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, FlowMeasureIr, HAlignIr, MetadataIr, PageSetupIr, ParagraphPdfMetricsIr,
};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn page_styles(nodes: &[TreeNode]) -> Option<typst::foundations::Styles> {
        let mut style_maps = Vec::new();
        Self::collect_all_page_styles(nodes, &mut style_maps);
        let (first, rest) = style_maps.split_first()?;
        let mut prefix = first.as_slice();
        for s in rest {
            let common = prefix
                .iter()
                .zip(s.as_slice())
                .take_while(|&(a, b)| a == b)
                .count();
            prefix = &prefix[..common];
        }
        Some(
            prefix
                .iter()
                .filter(|style| {
                    style.outside()
                        && (style.liftable() || style.element() == Some(PageElem::ELEM))
                })
                .cloned()
                .collect(),
        )
    }

    pub(crate) fn collect_all_page_styles<'b>(
        nodes: &'b [TreeNode],
        out: &mut Vec<&'b typst::foundations::Styles>,
    ) {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if elem
                    .label()
                    .is_some_and(|l| l.resolve().as_str() == "__typst2word_page_styles")
                    && let Some(styled) = elem.to_packed::<StyledElem>()
                {
                    out.push(&styled.styles);
                }
                Self::collect_all_page_styles(children, out);
            } else if let TreeNode::Group { children, .. } = node {
                Self::collect_all_page_styles(children, out);
            }
        }
    }

    pub(crate) fn page_root_font_size_pt(&self, page_nodes: &[TreeNode]) -> f64 {
        if let Some(trunk) = Self::page_styles(page_nodes) {
            return Self::resolve_length_pt(
                StyleChain::new(&trunk).get(typst::text::TextElem::size).0,
                11.0,
            );
        }
        Self::resolve_length_pt(
            self.style_chain().get(typst::text::TextElem::size).0,
            11.0,
        )
    }

    pub(crate) fn extract_metadata(&self) -> MetadataIr {
        let info = self.compiled.document.info();
        MetadataIr {
            title: info.title.as_ref().map(|s| s.to_string()),
            authors: info.author.iter().map(|s| s.to_string()).collect(),
            description: info.description.as_ref().map(|s| s.to_string()),
            keywords: info.keywords.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub(crate) fn extract_page_setup(&mut self, page_index: usize, page_nodes: &[TreeNode]) -> PageSetupIr {
        self.current_page_index = page_index;
        let mut setup = PageSetupIr::default();
        if let Some(first_page) = self.compiled.document.pages().get(page_index) {
            let size = first_page.frame.size();
            setup.width_pt = size.x.to_pt();
            setup.height_pt = size.y.to_pt();
            setup.fill = first_page
                .fill
                .clone()
                .custom()
                .and_then(|p| p.as_ref().and_then(paint_to_hex));

            // Check if the document uses page-level columns (`#set page(columns: N)`).
            // Scoped `#columns(N)[...]` (`ColumnsElem`) is extracted as `BlockNode::Columns`
            // so that 1-column pages are not forced into multi-column layout.
            setup.columns = self.style_chain().get(PageElem::columns).get();
            self.page_columns = setup.columns;
            if setup.columns > 1 {
                self.active_columns = (setup.columns, 18.0);
            }

            // Match `typst-layout/src/pages/run.rs` & `finalize.rs` margin resolution 1:1:
            // - `em` lengths resolve against the page-root `TextElem::size` (unpolluted by
            //   inner scoped `#[#set text(size: ...); ...]` blocks).
            // - `auto` / omitted sides resolve to `(2.5 / 21.0) * min(width, height)`, where
            //   an infinite (`auto`) axis falls back to `Paper::A4.width()` (`595.276pt`).
            // - Two-sided `inside` / `outside` sets `mirror_margins = true` and orders odd-page
            //   `(left, right)` according to `PageElem::binding` (`auto` -> `Right` in RTL, `Left` in LTR).
            let base_font_pt = self.page_root_font_size_pt(page_nodes);
            let chain = self.style_chain();
            let cfg_width_pt = match chain.get(PageElem::width) {
                Smart::Custom(len) => Some(Self::resolve_length_pt(len, base_font_pt)),
                Smart::Auto => None,
            };
            let cfg_height_pt = match chain.get(PageElem::height) {
                Smart::Custom(len) => Some(Self::resolve_length_pt(len, base_font_pt)),
                Smart::Auto => None,
            };
            let min_dim_pt = match (cfg_width_pt, cfg_height_pt) {
                (Some(w), Some(h)) if w.is_finite() && h.is_finite() => w.min(h),
                _ => typst::layout::Paper::A4.width().to_pt(),
            };
            let default_margin_pt = (2.5 / 21.0) * min_dim_pt;

            let margin_cfg = chain.get(PageElem::margin);
            let mut rel_axis_x = cfg_width_pt.filter(|v| v.is_finite());
            let mut rel_axis_y = cfg_height_pt.filter(|v| v.is_finite());
            if chain.get(PageElem::flipped) {
                std::mem::swap(&mut rel_axis_x, &mut rel_axis_y);
            }
            let resolve_side = |side: Option<Smart<typst::layout::Rel<typst::layout::Length>>>,
                                axis_pt: Option<f64>|
             -> Option<f64> {
                match side {
                    Some(Smart::Custom(rel)) => {
                        let abs_pt = Self::resolve_length_pt(rel.abs, base_font_pt);
                        let rel_pt = axis_pt.map_or(0.0, |whole| rel.rel.get() * whole);
                        Some((abs_pt + rel_pt).max(0.0))
                    }
                    _ => None,
                }
            };

            let (raw_left, raw_right, cfg_top, cfg_bottom, two_sided) = match margin_cfg {
                Smart::Custom(m) => (
                    resolve_side(m.sides.left, rel_axis_x),
                    resolve_side(m.sides.right, rel_axis_x),
                    resolve_side(m.sides.top, rel_axis_y),
                    resolve_side(m.sides.bottom, rel_axis_y),
                    m.two_sided.unwrap_or(false),
                ),
                Smart::Auto => (None, None, None, None, false),
            };

            let mut left_pt = raw_left.unwrap_or(default_margin_pt);
            let mut right_pt = raw_right.unwrap_or(default_margin_pt);
            if two_sided {
                setup.mirror_margins = true;
                let binding = match chain.get(PageElem::binding) {
                    Smart::Custom(b) => b,
                    Smart::Auto => {
                        if chain.resolve(typst::text::TextElem::dir) == typst::layout::Dir::RTL {
                            typst::layout::Binding::Right
                        } else {
                            typst::layout::Binding::Left
                        }
                    }
                };
                if binding == typst::layout::Binding::Right {
                    std::mem::swap(&mut left_pt, &mut right_pt);
                }
            }

            setup.margin_left_pt = left_pt;
            setup.margin_right_pt = right_pt;
            setup.margin_top_pt = cfg_top.unwrap_or(default_margin_pt);
            setup.margin_bottom_pt = cfg_bottom.unwrap_or(default_margin_pt);

            self.content_left_pt = setup.margin_left_pt;
            self.content_top_pt = setup.margin_top_pt;
            self.content_right_pt =
                (setup.width_pt - setup.margin_right_pt).max(setup.margin_left_pt + 36.0);
            self.page_width_pt = setup.width_pt;
            self.mirror_margins = setup.mirror_margins;

            // Extract header & footer from ArtifactElem(Header / Footer) on the first page.
            for node in page_nodes {
                self.collect_header_footer_artifacts(node, &mut setup.header, &mut setup.footer);
            }
            Self::zero_header_footer_paragraph_spacing(&mut setup.header);
            Self::zero_header_footer_paragraph_spacing(&mut setup.footer);

            setup.header_margin_pt = self
                .find_artifact_y_bounds(page_nodes, ArtifactKind::Header)
                .map(|(min_y, _)| {
                    if setup.margin_top_pt < 36.0 {
                        min_y.clamp(0.0, (setup.margin_top_pt - 6.0).max(0.0))
                    } else {
                        min_y.clamp(12.0, (setup.margin_top_pt - 12.0).max(12.0))
                    }
                })
                .unwrap_or_else(|| {
                    if setup.margin_top_pt < 48.0 {
                        (setup.margin_top_pt * 0.5).clamp(0.0, setup.margin_top_pt)
                    } else {
                        (setup.margin_top_pt * 0.58).max(28.0).min(setup.margin_top_pt)
                    }
                });
            setup.footer_margin_pt = self
                .find_artifact_y_bounds(page_nodes, ArtifactKind::Footer)
                .map(|(_, max_y)| {
                    let dist_bottom = (setup.height_pt - max_y).max(0.0);
                    if setup.margin_bottom_pt < 36.0 {
                        dist_bottom.clamp(0.0, (setup.margin_bottom_pt - 6.0).max(0.0))
                    } else {
                        dist_bottom.clamp(12.0, (setup.margin_bottom_pt - 12.0).max(12.0))
                    }
                })
                .unwrap_or_else(|| {
                    if setup.margin_bottom_pt < 48.0 {
                        (setup.margin_bottom_pt * 0.5).clamp(0.0, setup.margin_bottom_pt)
                    } else {
                        (setup.margin_bottom_pt * 0.58)
                            .max(28.0)
                            .min(setup.margin_bottom_pt)
                    }
                });
        }
        setup
    }

    pub(crate) fn zero_header_footer_paragraph_spacing(blocks: &mut [BlockNode]) {
        for block in blocks {
            if let BlockNode::Paragraph { style, .. } = block {
                style.space_before_pt = 0.0;
                style.space_after_pt = 0.0;
                style.pdf_metrics = ParagraphPdfMetricsIr::Unmeasured;
                style.flow_measure = FlowMeasureIr::default();
            }
        }
    }

    pub(crate) fn find_artifact_y_bounds(
        &self,
        nodes: &[TreeNode],
        target_kind: ArtifactKind,
    ) -> Option<(f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(artifact) = elem.to_packed::<ArtifactElem>() {
                        if artifact.kind.get(StyleChain::default()) == target_kind {
                            let (_, min_y, width, height) = Self::estimate_nodes_bbox(children);
                            if (width > 0.0 || height > 0.0)
                                && min_y.is_finite()
                                && height.is_finite()
                            {
                                return Some((min_y, min_y + height));
                            }
                        }
                    }
                    if let Some(bounds) = self.find_artifact_y_bounds(children, target_kind) {
                        return Some(bounds);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(bounds) = self.find_artifact_y_bounds(children, target_kind) {
                        return Some(bounds);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn collect_header_footer_artifacts(
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

    pub(crate) fn flatten_content_leaves<'b>(content: &'b Content, out: &mut Vec<&'b Content>) {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            Self::flatten_content_leaves(&styled.child, out);
        } else if let Some(seq) = content.to_packed::<SequenceElem>() {
            for child in &seq.children {
                Self::flatten_content_leaves(child, out);
            }
        } else if let Some(align) = content.to_packed::<typst::layout::AlignElem>() {
            Self::flatten_content_leaves(&align.body, out);
        } else {
            out.push(content);
        }
    }

    pub(crate) fn collect_pagebreak_following_texts(content: &Content, out: &mut Vec<String>) {
        let mut leaves = Vec::new();
        Self::flatten_content_leaves(content, &mut leaves);
        for (i, leaf) in leaves.iter().enumerate() {
            if leaf.is::<PagebreakElem>() {
                let mut found_text = None;
                for next in &leaves[i + 1..] {
                    if next.is::<PagebreakElem>() {
                        break;
                    }
                    let plain: String = next
                        .plain_text()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .take(10)
                        .collect();
                    if !plain.is_empty() {
                        found_text = Some(plain);
                        break;
                    }
                }
                out.push(found_text.unwrap_or_else(|| "__PAGEBREAK_ANY__".to_string()));
            }
        }
    }
}
