//! `figure` -> `BlockNode::Figure` (body + caption).

use typst::foundations::{Packed, StyleChain};
use typst::introspection::Location;
use typst::model::{FigureCaption, FigureElem};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, CaptionPositionIr, FigureKindIr, HAlignIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_figure(
        &mut self,
        fig: &Packed<FigureElem>,
        loc: Location,
        children: &[TreeNode],
        list_level: usize,
    ) -> BlockNode {
        let mut body_nodes = Vec::new();
        let mut caption_nodes_opt = None;
        let mut caption_pos = CaptionPositionIr::Bottom;
        let mut saw_body = false;

        Self::split_figure_body_and_caption(
            children,
            &mut body_nodes,
            &mut caption_nodes_opt,
            &mut caption_pos,
            &mut saw_body,
        );

        let caption_inlines =
            caption_nodes_opt.map(|cap_nodes| self.extract_inlines_flat(&cap_nodes));
        let body = self.extract_blocks_from_nodes(&body_nodes, list_level, HAlignIr::Center);
        let kind = FigureKindIr::from_typst_repr(&format!(
            "{:?}",
            fig.kind.get_ref(StyleChain::default())
        ));
        let bookmark = self.bookmarks.get(&loc).cloned();

        BlockNode::Figure {
            body,
            caption: caption_inlines,
            caption_pos,
            kind,
            bookmark,
        }
    }

    pub(crate) fn split_figure_body_and_caption(
        nodes: &[TreeNode],
        body_out: &mut Vec<TreeNode>,
        caption_out: &mut Option<Vec<TreeNode>>,
        caption_pos: &mut CaptionPositionIr,
        saw_body: &mut bool,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if elem.is::<FigureCaption>() {
                        if !*saw_body {
                            *caption_pos = CaptionPositionIr::Top;
                        }
                        *caption_out = Some(children.clone());
                    } else if elem.is::<FigureElem>() {
                        *saw_body = true;
                        body_out.push(node.clone());
                    } else {
                        let mut filtered_children = Vec::new();
                        Self::split_figure_body_and_caption(
                            children,
                            &mut filtered_children,
                            caption_out,
                            caption_pos,
                            saw_body,
                        );
                        *saw_body = true;
                        body_out.push(TreeNode::Element {
                            elem: elem.clone(),
                            loc: *loc,
                            pos: *pos,
                            children: filtered_children,
                        });
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let mut filtered_children = Vec::new();
                    Self::split_figure_body_and_caption(
                        children,
                        &mut filtered_children,
                        caption_out,
                        caption_pos,
                        saw_body,
                    );
                    if !filtered_children.is_empty() {
                        *saw_body = true;
                        body_out.push(TreeNode::Group {
                            group: group.clone(),
                            pos: *pos,
                            children: filtered_children,
                        });
                    }
                }
                other => {
                    *saw_body = true;
                    body_out.push(other.clone());
                }
            }
        }
    }
}
