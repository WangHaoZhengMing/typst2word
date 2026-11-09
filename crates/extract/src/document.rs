//! Document-level extraction: metadata, page setup (size, margins, columns, fill),
//! header / footer artifacts and page-break bookkeeping.

use typst::foundations::{Content, SequenceElem, Smart, StyleChain, StyledElem};
use typst::layout::{PageElem, PagebreakElem};
use typst::model::Document;
use typst::pdf::{ArtifactElem, ArtifactKind};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockNode, HAlignIr, InterBlockFlowIr, MetadataIr, PageSetupIr, ParagraphPdfMetricsIr,
};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn page_styles(nodes: &[TreeNode]) -> Option<typst::foundations::Styles> {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if elem.label().is_some_and(|l| l.resolve().as_str() == "__typst2word_page_styles") {
                    if let Some(styled) = elem.to_packed::<StyledElem>() { return Some(styled.styles.clone()); }
                }
                if let Some(styles) = Self::page_styles(children) { return Some(styles); }
            } else if let TreeNode::Group { children, .. } = node {
                if let Some(styles) = Self::page_styles(children) { return Some(styles); }
            }
        }
        None
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

            // Typst's default margin is 2.5 / 21 of the shorter page dimension (~70.87pt for A4).
            let default_margin_pt = (2.5 / 21.0) * setup.width_pt.min(setup.height_pt);
            let chain = self.style_chain();
            let base_font_pt =
                Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
            let margin_cfg = chain.get(PageElem::margin);
            let resolve_side = |side: Option<Smart<typst::layout::Rel<typst::layout::Length>>>,
                                axis_pt: f64|
             -> Option<f64> {
                match side {
                    Some(Smart::Custom(rel)) => {
                        let abs_pt = Self::resolve_length_pt(rel.abs, base_font_pt);
                        Some((abs_pt + rel.rel.get() * axis_pt).max(0.0))
                    }
                    _ => None,
                }
            };

            let (cfg_left, cfg_right, cfg_top, cfg_bottom) = match margin_cfg {
                Smart::Custom(m) => (
                    resolve_side(m.sides.left, setup.width_pt),
                    resolve_side(m.sides.right, setup.width_pt),
                    resolve_side(m.sides.top, setup.height_pt),
                    resolve_side(m.sides.bottom, setup.height_pt),
                ),
                Smart::Auto => (None, None, None, None),
            };

            // Page styles now survive realization; glyph positions can be
            // indented/aligned and must not be treated as page margins.
            setup.margin_left_pt = cfg_left.unwrap_or(default_margin_pt);
            setup.margin_right_pt = cfg_right.unwrap_or(default_margin_pt);
            setup.margin_top_pt = cfg_top.unwrap_or(default_margin_pt);
            setup.margin_bottom_pt = cfg_bottom.unwrap_or(default_margin_pt);

            self.content_left_pt = setup.margin_left_pt;
            self.content_right_pt =
                (setup.width_pt - setup.margin_right_pt).max(setup.margin_left_pt + 100.0);

            // Extract header & footer from ArtifactElem(Header / Footer) on the first page.
            for node in page_nodes {
                self.collect_header_footer_artifacts(node, &mut setup.header, &mut setup.footer);
            }
            Self::zero_header_footer_paragraph_spacing(&mut setup.header);
            Self::zero_header_footer_paragraph_spacing(&mut setup.footer);

            setup.header_margin_pt = self
                .find_artifact_top_y(page_nodes, ArtifactKind::Header)
                .map(|y| y.clamp(18.0, (setup.margin_top_pt - 12.0).max(18.0)))
                .unwrap_or_else(|| (setup.margin_top_pt * 0.58).max(28.0));
            setup.footer_margin_pt = self
                .find_artifact_top_y(page_nodes, ArtifactKind::Footer)
                .map(|y| {
                    (setup.height_pt - y - 12.0)
                        .clamp(18.0, (setup.margin_bottom_pt - 12.0).max(18.0))
                })
                .unwrap_or_else(|| (setup.margin_bottom_pt * 0.58).max(28.0));
        }
        setup
    }

    pub(crate) fn zero_header_footer_paragraph_spacing(blocks: &mut [BlockNode]) {
        for block in blocks {
            if let BlockNode::Paragraph { style, .. } = block {
                style.space_before_pt = 0.0;
                style.space_after_pt = 0.0;
                style.pdf_metrics = ParagraphPdfMetricsIr::Unmeasured;
                style.inter_block_flow = InterBlockFlowIr::Standard;
                style.extra_trailing_space_pt = 0.0;
            }
        }
    }

    pub(crate) fn find_artifact_top_y(
        &self,
        nodes: &[TreeNode],
        target_kind: ArtifactKind,
    ) -> Option<f64> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(artifact) = elem.to_packed::<ArtifactElem>() {
                        if artifact.kind.get(StyleChain::default()) == target_kind {
                            let (_, min_y, _, _) = Self::estimate_nodes_bbox(children);
                            if min_y.is_finite() && min_y > 0.0 {
                                return Some(min_y);
                            }
                        }
                    }
                    if let Some(y) = self.find_artifact_top_y(children, target_kind) {
                        return Some(y);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(y) = self.find_artifact_top_y(children, target_kind) {
                        return Some(y);
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
