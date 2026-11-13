//! `list` / `enum` / `terms` -> `BlockNode::List` / `Enum` / `Terms`.

use typst::foundations::{Packed, StyleChain};
use typst::introspection::Location;
use typst::model::{EnumElem, ListElem, TermsElem};
use typst::pdf::{ArtifactElem, PdfMarkerTagKind};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    BlockLayoutSpanIr, BlockNode, EnumItemIr, HAlignIr, ListItemIr, NextBlockKindIr,
    NextBlockLayoutMeasureIr, SemanticRoleIr, TermItemIr,
};

/// Shared intermediate item extracted from `PdfMarkerTagKind::ListItemLabel` + `ListItemBody`
/// before specializing into `ListItemIr` (bullet list) or `EnumItemIr` (numbered list).
struct RawExtractedListItem {
    marker_text: String,
    blocks: Vec<BlockNode>,
    first_y: Option<(usize, f64, f64)>,
    last_y: Option<(usize, f64, f64)>,
}

impl<'a> ExtractorContext<'a> {
    /// `ListElem` -> `BlockNode::List`.
    pub(crate) fn extract_list_block(
        &mut self,
        list: &Packed<ListElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let id = self.alloc_node_with_fragments(
            list.span(),
            Some(loc),
            Some(SemanticRoleIr::List),
            children,
        );
        self.parent_node_stack.push(id);
        let chain = self.scoped_style_chain(Some(loc));
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let tight = list.tight.get(chain);
        let spacing_pt = Self::resolve_list_spacing_pt(list.spacing.get(chain), tight, chain);
        let mut indent_pt = Self::resolve_length_pt(list.indent.get(chain), default_font_pt);
        if list_level == 0
            && let Some(vis_indent) = self.node_left_offset_in_column(children)
            && vis_indent <= 48.0
        {
            indent_pt = indent_pt.max(vis_indent);
        }
        let body_indent_pt = Self::resolve_length_pt(list.body_indent.get(chain), default_font_pt);
        let (items, max_label_w_pt) = self.extract_list_items(children, list_level);
        self.parent_node_stack.pop();
        let label_w = if max_label_w_pt > 0.5 {
            max_label_w_pt
        } else {
            6.0
        };
        let hanging_pt = (label_w + body_indent_pt).max(10.0);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::List {
                id,
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
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let id = self.alloc_node_with_fragments(
            en.span(),
            Some(loc),
            Some(SemanticRoleIr::Enum),
            children,
        );
        self.parent_node_stack.push(id);
        let chain = self.scoped_style_chain(Some(loc));
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let tight = en.tight.get(chain);
        let spacing_pt = Self::resolve_list_spacing_pt(en.spacing.get(chain), tight, chain);
        let mut indent_pt = Self::resolve_length_pt(en.indent.get(chain), default_font_pt);
        if list_level == 0
            && let Some(vis_indent) = self.node_left_offset_in_column(children)
            && vis_indent <= 48.0
        {
            indent_pt = indent_pt.max(vis_indent);
        }
        let body_indent_pt = Self::resolve_length_pt(en.body_indent.get(chain), default_font_pt);
        let (items, max_label_w_pt) = self.extract_enum_items(en, Some(loc), children, list_level);
        self.parent_node_stack.pop();
        let label_w = if max_label_w_pt > 0.5 {
            max_label_w_pt
        } else {
            11.0
        };
        let hanging_pt = (label_w + body_indent_pt).max(15.0);
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::Enum {
                id,
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
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) {
        let id = self.alloc_node_with_fragments(
            terms.span(),
            Some(loc),
            Some(SemanticRoleIr::Terms),
            children,
        );
        self.parent_node_stack.push(id);
        let chain = self.scoped_style_chain(Some(loc));
        let tight = terms.tight.get(chain);
        let items = self.extract_term_items(children, list_level);
        self.parent_node_stack.pop();
        if !items.is_empty() {
            self.report.lists_count += 1;
            blocks.push(BlockNode::Terms { id, items, tight });
        }
    }

    pub(crate) fn extract_list_items(
        &mut self,
        children: &[TreeNode],
        list_level: usize,
    ) -> (Vec<ListItemIr>, f64) {
        let (raw_items, max_label_w_pt) = self.extract_raw_list_items(children, list_level, "•");
        let items = raw_items
            .into_iter()
            .map(|raw| ListItemIr {
                marker: raw.marker_text,
                blocks: raw.blocks,
            })
            .collect();
        (items, max_label_w_pt)
    }

    pub(crate) fn extract_enum_items(
        &mut self,
        en: &Packed<EnumElem>,
        loc_opt: Option<Location>,
        children: &[TreeNode],
        list_level: usize,
    ) -> (Vec<EnumItemIr>, f64) {
        let chain = self.scoped_style_chain(loc_opt);
        let start_num = en.start.get(chain).unwrap_or(1);
        let mut counter = start_num;
        let (raw_items, max_label_w_pt) = self.extract_raw_list_items(children, list_level, "1.");
        let items = raw_items
            .into_iter()
            .map(|raw| {
                let parsed_num = raw
                    .marker_text
                    .trim_matches(|c: char| !c.is_ascii_digit())
                    .parse::<u64>()
                    .ok()
                    .or(Some(counter));
                counter += 1;
                EnumItemIr {
                    number: parsed_num,
                    marker_text: raw.marker_text,
                    blocks: raw.blocks,
                }
            })
            .collect();
        (items, max_label_w_pt)
    }

    fn extract_raw_list_items(
        &mut self,
        children: &[TreeNode],
        list_level: usize,
        default_marker: &str,
    ) -> (Vec<RawExtractedListItem>, f64) {
        let mut items = Vec::new();
        let mut current_marker = default_marker.to_string();
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
                    let first_y = Self::nodes_first_page_top_y(&marker_children);
                    let last_y = Self::nodes_last_page_bottom_y(&marker_children);
                    let blocks = self.extract_blocks_from_nodes(
                        &marker_children,
                        list_level + 1,
                        HAlignIr::Left,
                    );
                    items.push(RawExtractedListItem {
                        marker_text: current_marker.clone(),
                        blocks,
                        first_y,
                        last_y,
                    });
                }
                _ => {}
            }
        }

        for i in 0..items.len().saturating_sub(1) {
            let span_a = BlockLayoutSpanIr::from_start_end(items[i].first_y, items[i].last_y);
            let span_b =
                BlockLayoutSpanIr::from_start_end(items[i + 1].first_y, items[i + 1].last_y);
            let measure = NextBlockLayoutMeasureIr {
                kind: NextBlockKindIr::ListItemSibling {
                    list_level,
                    active_columns: self.active_columns.0,
                    prev_item_block_count: items[i].blocks.len(),
                    next_item_starts_with_paragraph: matches!(
                        items[i + 1].blocks.first(),
                        Some(BlockNode::Paragraph { .. })
                    ),
                },
                span: span_b,
                prev_font_pt: 11.0,
                next_font_pt: 11.0,
            };
            self.record_trailing_transition_on_last_block(&mut items[i].blocks, span_a, measure);
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
