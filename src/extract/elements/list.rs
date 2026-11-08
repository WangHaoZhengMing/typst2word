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
        let chain = self.style_chain();
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let tight = list.tight.get(chain);
        let spacing_pt = Self::resolve_list_spacing_pt(list.spacing.get(chain), tight, chain);
        let indent_pt = Self::resolve_length_pt(list.indent.get(chain), default_font_pt);
        let body_indent_pt = Self::resolve_length_pt(list.body_indent.get(chain), default_font_pt);
        let (items, max_label_w_pt) = self.extract_list_items(children, list_level);
        let hanging_pt = (max_label_w_pt.max(6.0) + body_indent_pt).max(13.0);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::List {
                items,
                tight,
                level: list_level,
                spacing_pt,
                indent_pt,
                hanging_pt,
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
        let chain = self.style_chain();
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let tight = en.tight.get(chain);
        let spacing_pt = Self::resolve_list_spacing_pt(en.spacing.get(chain), tight, chain);
        let indent_pt = Self::resolve_length_pt(en.indent.get(chain), default_font_pt);
        let body_indent_pt = Self::resolve_length_pt(en.body_indent.get(chain), default_font_pt);
        let (items, max_label_w_pt) = self.extract_enum_items(en, children, list_level);
        let hanging_pt = (max_label_w_pt.max(11.0) + body_indent_pt).max(16.0);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::Enum {
                items,
                tight,
                level: list_level,
                spacing_pt,
                indent_pt,
                hanging_pt,
            });
        }
    }

    fn resolve_list_spacing_pt(
        spacing: typst::foundations::Smart<typst::layout::Length>,
        tight: bool,
        chain: StyleChain,
    ) -> Option<f64> {
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let leading_pt =
            Self::resolve_length_pt(chain.get(typst::model::ParElem::leading), default_font_pt);
        let raw_pt = match spacing {
            typst::foundations::Smart::Custom(len) => Self::resolve_length_pt(len, default_font_pt),
            typst::foundations::Smart::Auto if !tight => {
                match chain.get(typst::layout::BlockElem::above) {
                    typst::foundations::Smart::Custom(typst::layout::Spacing::Rel(rel)) => {
                        Self::resolve_length_pt(rel.abs, default_font_pt)
                    }
                    _ => Self::resolve_length_pt(
                        chain.get(typst::model::ParElem::spacing),
                        default_font_pt,
                    ),
                }
            }
            typst::foundations::Smart::Auto => return None,
        };
        Some((raw_pt - leading_pt).max(0.0))
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
    ) -> (Vec<ListItemIr>, f64) {
        let mut items = Vec::new();
        let mut current_marker = "•".to_string();
        let mut max_label_w_pt = 0.0_f64;

        let mut flat_markers = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut flat_markers);

        for (kind, marker_children) in flat_markers {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let (_, _, w_pt, _) = Self::estimate_nodes_bbox(&marker_children);
                    max_label_w_pt = max_label_w_pt.max(w_pt);
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

        (items, max_label_w_pt)
    }

    pub(crate) fn extract_enum_items(
        &mut self,
        en: &Packed<EnumElem>,
        children: &[TreeNode],
        list_level: usize,
    ) -> (Vec<EnumItemIr>, f64) {
        let mut items = Vec::new();
        let mut current_marker = "1.".to_string();
        let start_num = en.start.get(StyleChain::default()).unwrap_or(1);
        let mut counter = start_num;
        let mut max_label_w_pt = 0.0_f64;

        let mut flat_markers = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut flat_markers);

        for (kind, marker_children) in flat_markers {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let (_, _, w_pt, _) = Self::estimate_nodes_bbox(&marker_children);
                    max_label_w_pt = max_label_w_pt.max(w_pt);
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

        (items, max_label_w_pt)
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
