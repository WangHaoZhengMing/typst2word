//! Orphaned cross-page list marker recovery.

use typst::foundations::Packed;
use typst::model::{EnumElem, ListElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};
use typst::text::TextElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, EnumItemIr, HAlignIr, ListItemIr};

impl<'a> ExtractorContext<'a> {
    /// Reconstruct list or enum items when a `ListElem` or `EnumElem` inside a hard `Group`
    /// (`#block`) split across columns or pages, leaving orphaned `PdfMarkerTag` nodes.
    pub(crate) fn try_extract_orphaned_list_marker(
        &mut self,
        marker: &Packed<PdfMarkerTag>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
    ) -> bool {
        match marker.kind {
            PdfMarkerTagKind::ListItemLabel => {
                let lbl = Self::nodes_to_plain_text(children).trim().to_string();
                if !lbl.is_empty() {
                    self.pending_list_marker = Some(lbl);
                }
                true
            }
            PdfMarkerTagKind::ListItemBody => {
                let marker_str = self
                    .pending_list_marker
                    .take()
                    .unwrap_or_else(|| "•".to_string());
                let item_blocks =
                    self.extract_blocks_from_nodes(children, list_level + 1, HAlignIr::Left);
                let is_enum = marker_str
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric());
                if is_enum {
                    let parsed_num = marker_str
                        .trim_matches(|c: char| !c.is_ascii_digit())
                        .parse::<u64>()
                        .ok();
                    let item = EnumItemIr {
                        number: parsed_num,
                        marker_text: marker_str,
                        blocks: item_blocks,
                    };
                    if let Some(BlockNode::Enum { items, level, .. }) = blocks.last_mut()
                        && *level == list_level
                    {
                        items.push(item);
                    } else {
                        let chain = self.style_chain();
                        let default_font_pt =
                            Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
                        let indent_pt =
                            Self::resolve_length_pt(chain.get(EnumElem::indent), default_font_pt);
                        let body_indent_pt = Self::resolve_length_pt(
                            chain.get(EnumElem::body_indent),
                            default_font_pt,
                        );
                        self.report.lists_count += 1;
                        blocks.push(BlockNode::Enum {
                            items: vec![item],
                            tight: true,
                            level: list_level,
                            spacing_pt: None,
                            indent_pt,
                            hanging_pt: (11.5 + body_indent_pt).max(16.0),
                        });
                    }
                } else {
                    let item = ListItemIr {
                        marker: marker_str,
                        blocks: item_blocks,
                    };
                    if let Some(BlockNode::List { items, level, .. }) = blocks.last_mut()
                        && *level == list_level
                    {
                        items.push(item);
                    } else {
                        let chain = self.style_chain();
                        let default_font_pt =
                            Self::resolve_length_pt(chain.get(TextElem::size).0, 11.0);
                        let indent_pt =
                            Self::resolve_length_pt(chain.get(ListElem::indent), default_font_pt);
                        let body_indent_pt = Self::resolve_length_pt(
                            chain.get(ListElem::body_indent),
                            default_font_pt,
                        );
                        self.report.lists_count += 1;
                        blocks.push(BlockNode::List {
                            items: vec![item],
                            tight: true,
                            level: list_level,
                            spacing_pt: None,
                            indent_pt,
                            hanging_pt: (6.0 + body_indent_pt).max(13.0),
                        });
                    }
                }
                true
            }
            _ => false,
        }
    }
}
