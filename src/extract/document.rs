//! Document-level extraction: metadata, page setup (size, margins, columns, fill),
//! header / footer artifacts and page-break bookkeeping.

use typst::foundations::{Content, SequenceElem, Smart, StyleChain, StyledElem};
use typst::layout::{ColumnsElem, PageElem, PagebreakElem};
use typst::model::{Document, FootnoteEntry};
use typst::pdf::{ArtifactElem, ArtifactKind};

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr, MetadataIr, PageSetupIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_metadata(&self) -> MetadataIr {
        let info = self.compiled.document.info();
        MetadataIr {
            title: info.title.as_ref().map(|s| s.to_string()),
            authors: info.author.iter().map(|s| s.to_string()).collect(),
            description: info.description.as_ref().map(|s| s.to_string()),
            keywords: info.keywords.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub(crate) fn extract_page_setup(&mut self, page_nodes: &[TreeNode]) -> PageSetupIr {
        let mut setup = PageSetupIr::default();
        if let Some(first_page) = self.compiled.document.pages().first() {
            let size = first_page.frame.size();
            setup.width_pt = size.x.to_pt();
            setup.height_pt = size.y.to_pt();
            setup.fill = first_page
                .fill
                .clone()
                .custom()
                .and_then(|p| p.as_ref().and_then(paint_to_hex));

            // Check if the document uses columns (`#set page(columns: N)` or `#columns(N)`).
            if let Some(cols) = Self::find_columns_in_content(
                &self.compiled.evaluated_content,
                StyleChain::default(),
            ) {
                setup.columns = cols;
            } else {
                let cols = self.compiled.query_elem::<ColumnsElem>();
                if let Some(col_elem) = cols.first() {
                    setup.columns = col_elem.count.get(StyleChain::default()).get();
                }
            }

            // Typst's default margin is 2.5 / 21 of the shorter page dimension (~70.87pt for A4).
            let default_margin_pt = (2.5 / 21.0) * setup.width_pt.min(setup.height_pt);
            let chain = self.style_chain();
            let base_font_pt = Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
            let margin_cfg = chain.get(PageElem::margin);
            let resolve_side = |side: Option<Smart<typst::layout::Rel<typst::layout::Length>>>,
                                axis_pt: f64|
             -> Option<f64> {
                match side {
                    Some(Smart::Custom(rel)) => {
                        let abs_pt = Self::resolve_length_pt(rel.abs, base_font_pt);
                        Some((abs_pt + rel.rel.get() * axis_pt).max(14.0))
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

            let inferred_left = Self::find_first_content_origin(page_nodes)
                .map(|(lx, _)| lx)
                .filter(|&lx| {
                    (lx - default_margin_pt).abs() > 4.0 && lx > 14.0 && lx < setup.width_pt * 0.35
                });

            setup.margin_left_pt = cfg_left.or(inferred_left).unwrap_or(default_margin_pt);
            setup.margin_right_pt = cfg_right.or(inferred_left).unwrap_or(default_margin_pt);
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

    pub(crate) fn find_columns_in_content(content: &Content, styles: StyleChain) -> Option<usize> {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            let chained = styles.chain(&styled.styles);
            let page_cols = chained.get(PageElem::columns).get();
            if page_cols > 1 {
                return Some(page_cols);
            }
            return Self::find_columns_in_content(&styled.child, chained);
        }
        if let Some(col_elem) = content.to_packed::<ColumnsElem>() {
            let c = col_elem.count.get(styles).get();
            if c > 1 {
                return Some(c);
            }
        }
        if let Some(seq) = content.to_packed::<SequenceElem>() {
            for child in &seq.children {
                if let Some(c) = Self::find_columns_in_content(child, styles) {
                    return Some(c);
                }
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

    pub(crate) fn find_first_content_origin(nodes: &[TreeNode]) -> Option<(f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    pos,
                    children,
                    ..
                } => {
                    if elem.is::<ArtifactElem>() || elem.is::<FootnoteEntry>() {
                        continue;
                    }
                    if pos.x.to_pt() > 0.0 && pos.y.to_pt() > 0.0 {
                        return Some((pos.x.to_pt(), pos.y.to_pt()));
                    }
                    if let Some(found) = Self::find_first_content_origin(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    if let Some((cx, cy)) = Self::find_first_content_origin(children) {
                        return Some((pos.x.to_pt() + cx, pos.y.to_pt() + cy));
                    }
                }
                TreeNode::Text { pos, .. } => {
                    if pos.x.to_pt() > 0.0 && pos.y.to_pt() > 0.0 {
                        return Some((pos.x.to_pt(), pos.y.to_pt()));
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn collect_pagebreak_following_texts(content: &Content, out: &mut Vec<String>) {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            Self::collect_pagebreak_following_texts(&styled.child, out);
        } else if let Some(seq) = content.to_packed::<SequenceElem>() {
            for (i, child) in seq.children.iter().enumerate() {
                if child.is::<PagebreakElem>() {
                    let mut found_text = None;
                    for next in &seq.children[i + 1..] {
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
                } else {
                    Self::collect_pagebreak_following_texts(child, out);
                }
            }
        }
    }

    pub(crate) fn clean_redundant_pagebreaks(&self, blocks: &mut Vec<BlockNode>) {
        // Trim leading or trailing PageBreaks and collapse consecutive PageBreaks.
        while matches!(blocks.first(), Some(BlockNode::PageBreak)) {
            blocks.remove(0);
        }
        while matches!(blocks.last(), Some(BlockNode::PageBreak)) {
            blocks.pop();
        }
        let mut i = 1;
        while i < blocks.len() {
            if matches!(blocks[i], BlockNode::PageBreak)
                && matches!(blocks[i - 1], BlockNode::PageBreak)
            {
                blocks.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
