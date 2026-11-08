//! Document-level extraction: metadata, page setup (size, margins, columns, fill),
//! header / footer artifacts and page-break bookkeeping.

use typst::foundations::{Content, SequenceElem, StyleChain, StyledElem};
use typst::layout::{ColumnsElem, PageElem, PagebreakElem};
use typst::model::FootnoteEntry;
use typst::pdf::{ArtifactElem, ArtifactKind};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr, MetadataIr, PageSetupIr};
use crate::math_omml::paint_to_hex;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_metadata(&self) -> MetadataIr {
        let info = &self.compiled.document.info;
        MetadataIr {
            title: info.title.as_ref().map(|s| s.to_string()),
            authors: info.author.iter().map(|s| s.to_string()).collect(),
            description: info.description.as_ref().map(|s| s.to_string()),
            keywords: info.keywords.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub(crate) fn extract_page_setup(&mut self, page_nodes: &[TreeNode]) -> PageSetupIr {
        let mut setup = PageSetupIr::default();
        if let Some(first_page) = self.compiled.document.pages.first() {
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

            // Estimate margins from the first page content position if available BEFORE extracting
            // headers/footers so `self.content_left_pt` and `self.content_right_pt` are accurate!
            if let Some((min_x, min_y)) = Self::find_first_content_origin(page_nodes) {
                if min_x > 10.0 && min_x < setup.width_pt * 0.4 {
                    setup.margin_left_pt = min_x;
                    setup.margin_right_pt = min_x;
                }
                if min_y > 10.0 && min_y < setup.height_pt * 0.4 {
                    setup.margin_top_pt = min_y;
                    setup.margin_bottom_pt = min_y;
                }
            }
            self.content_left_pt = setup.margin_left_pt;
            self.content_right_pt =
                (setup.width_pt - setup.margin_right_pt).max(setup.margin_left_pt + 100.0);

            // Extract header & footer from ArtifactElem(Header / Footer) on the first page.
            for node in page_nodes {
                self.collect_header_footer_artifacts(node, &mut setup.header, &mut setup.footer);
            }
        }
        setup
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

    pub(crate) fn has_explicit_pagebreak(content: &Content) -> bool {
        if content.is::<PagebreakElem>() {
            return true;
        }
        if let Some(styled) = content.to_packed::<StyledElem>() {
            return Self::has_explicit_pagebreak(&styled.child);
        }
        if let Some(seq) = content.to_packed::<SequenceElem>() {
            return seq.children.iter().any(Self::has_explicit_pagebreak);
        }
        false
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
