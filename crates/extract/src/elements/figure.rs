//! `figure` -> `BlockNode::Figure` (body + caption).

use typst::foundations::{Packed, StyleChain};
use typst::introspection::Location;
use typst::model::{FigureCaption, FigureElem};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, CaptionPositionIr, FigureKindIr, HAlignIr};

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

        let mut cap_min_y = None;
        let mut body_min_y = None;
        Self::collect_figure_caption_and_body_min_y(
            children,
            typst::layout::Point::zero(),
            false,
            &mut cap_min_y,
            &mut body_min_y,
        );
        if let (Some(cap_y), Some(body_y)) = (cap_min_y, body_min_y) {
            caption_pos = if cap_y + 0.5 < body_y {
                CaptionPositionIr::Top
            } else {
                CaptionPositionIr::Bottom
            };
        }

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

    fn collect_figure_caption_and_body_min_y(
        nodes: &[TreeNode],
        offset: typst::layout::Point,
        in_caption: bool,
        cap_min_y: &mut Option<f64>,
        body_min_y: &mut Option<f64>,
    ) {
        let update_min = |slot: &mut Option<f64>, y: f64| {
            *slot = Some(slot.map_or(y, |prev| prev.min(y)));
        };
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if Self::is_page_furniture_elem(elem) {
                        continue;
                    }
                    let next_in_cap = in_caption || elem.is::<FigureCaption>();
                    Self::collect_figure_caption_and_body_min_y(
                        children,
                        offset,
                        next_in_cap,
                        cap_min_y,
                        body_min_y,
                    );
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_figure_caption_and_body_min_y(
                        children,
                        offset + *pos,
                        in_caption,
                        cap_min_y,
                        body_min_y,
                    );
                }
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let y = (offset.y + pos.y - text.size * 0.8).to_pt();
                    if in_caption {
                        update_min(cap_min_y, y);
                    } else {
                        update_min(body_min_y, y);
                    }
                }
                TreeNode::Shape { shape, pos, .. } => {
                    let b = shape.geometry.bbox(None);
                    let y = (offset.y + pos.y + b.min.y).to_pt();
                    if in_caption {
                        update_min(cap_min_y, y);
                    } else {
                        update_min(body_min_y, y);
                    }
                }
                TreeNode::Image { pos, .. } => {
                    let y = (offset.y + pos.y).to_pt();
                    if in_caption {
                        update_min(cap_min_y, y);
                    } else {
                        update_min(body_min_y, y);
                    }
                }
                _ => {}
            }
        }
    }
}
