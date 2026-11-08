//! `list` / `enum` / `terms` -> `BlockNode::List` / `Enum` / `Terms`.

use typst::foundations::{Packed, StyleChain};
use typst::model::{EnumElem, ListElem, TermsElem};
use typst::pdf::{ArtifactElem, PdfMarkerTagKind};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, EnumItemIr, HAlignIr, ListItemIr, TermItemIr};

impl<'a> ExtractorContext<'a> {
    /// `ListElem` -> `BlockNode::List`.
    pub(crate) fn extract_list_block(
        &mut self,
        list: &Packed<ListElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let tight = list.tight.get(StyleChain::default());
        let items = self.extract_list_items(children, list_level);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::List {
                items,
                tight,
                level: list_level,
            });
        }
    }

    /// `EnumElem` -> `BlockNode::Enum`.
    pub(crate) fn extract_enum_block(
        &mut self,
        en: &Packed<EnumElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let tight = en.tight.get(StyleChain::default());
        let items = self.extract_enum_items(en, children, list_level);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::Enum {
                items,
                tight,
                level: list_level,
            });
        }
    }

    /// `TermsElem` -> `BlockNode::Terms`.
    pub(crate) fn extract_terms_block(
        &mut self,
        terms: &Packed<TermsElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let tight = terms.tight.get(StyleChain::default());
        let items = self.extract_term_items(children, list_level);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::Terms { items, tight });
        }
    }

    pub(crate) fn extract_list_items(
        &mut self,
        children: &[TreeNode],
        list_level: usize,
    ) -> Vec<ListItemIr> {
        let mut items = Vec::new();
        let mut current_marker = "•".to_string();

        let mut flat_markers = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut flat_markers);

        for (kind, marker_children) in flat_markers {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let txt = Self::nodes_to_plain_text(&marker_children)
                        .trim()
                        .to_string();
                    if !txt.is_empty() {
                        current_marker = txt;
                    }
                }
                PdfMarkerTagKind::ListItemBody => {
                    let blocks = self.extract_blocks_from_nodes(
                        &marker_children,
                        list_level + 1,
                        HAlignIr::Left,
                    );
                    items.push(ListItemIr {
                        marker: current_marker.clone(),
                        blocks,
                    });
                }
                _ => {}
            }
        }

        items
    }

    pub(crate) fn extract_enum_items(
        &mut self,
        en: &Packed<EnumElem>,
        children: &[TreeNode],
        list_level: usize,
    ) -> Vec<EnumItemIr> {
        let mut items = Vec::new();
        let mut current_marker = "1.".to_string();
        let start_num = en.start.get(StyleChain::default()).unwrap_or(1);
        let mut counter = start_num;

        let mut flat_markers = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut flat_markers);

        for (kind, marker_children) in flat_markers {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let txt = Self::nodes_to_plain_text(&marker_children)
                        .trim()
                        .to_string();
                    if !txt.is_empty() {
                        current_marker = txt;
                    }
                }
                PdfMarkerTagKind::ListItemBody => {
                    let blocks = self.extract_blocks_from_nodes(
                        &marker_children,
                        list_level + 1,
                        HAlignIr::Left,
                    );
                    let parsed_num = current_marker
                        .trim_matches(|c: char| !c.is_ascii_digit())
                        .parse::<u64>()
                        .ok()
                        .or(Some(counter));
                    items.push(EnumItemIr {
                        number: parsed_num,
                        marker_text: current_marker.clone(),
                        blocks,
                    });
                    counter += 1;
                }
                _ => {}
            }
        }

        items
    }

    pub(crate) fn extract_term_items(
        &mut self,
        children: &[TreeNode],
        list_level: usize,
    ) -> Vec<TermItemIr> {
        let mut items = Vec::new();
        let mut term_bodies = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut term_bodies);

        for (kind, body_children) in term_bodies {
            if matches!(kind, PdfMarkerTagKind::TermsItemBody) {
                let mut term_inlines = Vec::new();
                let mut desc_nodes = Vec::new();
                for child in body_children {
                    if Self::is_pdf_marker(&child, |k| {
                        matches!(k, PdfMarkerTagKind::TermsItemLabel)
                    }) {
                        if let TreeNode::Element {
                            children: lbl_children,
                            ..
                        } = &child
                        {
                            term_inlines = self.extract_inlines_flat(lbl_children);
                        }
                    } else if let TreeNode::Element { elem, .. } = &child
                        && elem.is::<ArtifactElem>()
                    {
                        // Skip the separator artifact between term and description.
                        continue;
                    } else {
                        desc_nodes.push(child);
                    }
                }
                let description =
                    self.extract_blocks_from_nodes(&desc_nodes, list_level + 1, HAlignIr::Left);
                items.push(TermItemIr {
                    term: term_inlines,
                    description,
                });
            }
        }

        items
    }
}
