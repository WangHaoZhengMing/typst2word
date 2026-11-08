//! Typst alignment -> `HAlignIr` / `VAlignIr`, including alignment inferred from layout.

use typst::foundations::{Smart, StyleChain};
use typst::layout::{AlignElem, Alignment, HAlignment, VAlignment};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{HAlignIr, VAlignIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn alignment_to_ir(align: Smart<Alignment>) -> (HAlignIr, VAlignIr) {
        let Smart::Custom(a) = align else {
            return (HAlignIr::Left, VAlignIr::Top);
        };
        let ha = match a.x() {
            Some(HAlignment::Center) => HAlignIr::Center,
            Some(HAlignment::Right | HAlignment::End) => HAlignIr::Right,
            _ => HAlignIr::Left,
        };
        let va = match a.y() {
            Some(VAlignment::Horizon) => VAlignIr::Center,
            Some(VAlignment::Bottom) => VAlignIr::Bottom,
            _ => VAlignIr::Top,
        };
        (ha, va)
    }

    pub(crate) fn infer_node_halign(&self, nodes: &[TreeNode]) -> Option<HAlignIr> {
        // 1. Check if any child element is an explicit `AlignElem`.
        if let Some(explicit) = Self::find_explicit_align_elem(nodes) {
            return Some(explicit);
        }

        // 2. Infer horizontal alignment from laid-out glyph/frame X coordinates within the content area!
        let (min_x, _min_y, width, _height) = Self::estimate_nodes_bbox(nodes);
        if width <= 0.5 || !min_x.is_finite() {
            return None;
        }

        let (left_bound, right_bound) = if min_x >= self.content_left_pt - 8.0 {
            (self.content_left_pt, self.content_right_pt)
        } else {
            (
                0.0,
                (self.content_right_pt - self.content_left_pt).max(100.0),
            )
        };

        let max_x = min_x + width;
        let left_gap = (min_x - left_bound).max(0.0);
        let right_gap = (right_bound - max_x).max(0.0);

        if left_gap <= 8.0 {
            Some(HAlignIr::Left)
        } else if left_gap > 15.0 && right_gap <= 8.0 {
            Some(HAlignIr::Right)
        } else if left_gap > 12.0 && right_gap > 12.0 && (left_gap - right_gap).abs() <= 18.0 {
            Some(HAlignIr::Center)
        } else {
            None
        }
    }

    pub(crate) fn find_explicit_align_elem(nodes: &[TreeNode]) -> Option<HAlignIr> {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if let Some(al) = elem.to_packed::<AlignElem>() {
                    let (ha, _) = Self::alignment_to_ir(Smart::Custom(
                        al.alignment.get(StyleChain::default()),
                    ));
                    return Some(ha);
                }
                if let Some(found) = Self::find_explicit_align_elem(children) {
                    return Some(found);
                }
            }
        }
        None
    }
}
