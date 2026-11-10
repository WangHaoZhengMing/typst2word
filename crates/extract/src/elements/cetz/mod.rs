//! CeTZ canvases (`cetz.canvas(...)`) -> `CetzCanvasIr`.

pub(crate) mod collect;

use typst::layout::{ColbreakElem, LayoutElem, PagebreakElem, Point, Size};
use typst::model::HeadingElem;
use typst::visualize::CurveElem;

use compiler::compile::is_cetz_span;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr, ColorHex, HAlignIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn contains_cetz_shapes_recursive(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            match node {
                TreeNode::Shape { span, .. } if is_cetz_span(*span) => return true,
                TreeNode::Element { elem, children, .. } => {
                    if is_cetz_span(elem.span()) && elem.is::<CurveElem>() {
                        return true;
                    }
                    if Self::contains_cetz_shapes_recursive(children) {
                        return true;
                    }
                }
                TreeNode::Group { children, .. } => {
                    if Self::contains_cetz_shapes_recursive(children) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn contains_any_drawable_shapes_recursive(nodes: &[TreeNode]) -> bool {
        for node in nodes {
            match node {
                TreeNode::Shape { shape, .. }
                    if shape.fill.is_some() || shape.stroke.is_some() =>
                {
                    return true;
                }
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<CurveElem>() {
                        return true;
                    }
                    if Self::contains_any_drawable_shapes_recursive(children) {
                        return true;
                    }
                }
                TreeNode::Group { children, .. } => {
                    if Self::contains_any_drawable_shapes_recursive(children) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn is_cetz_group_nodes(nodes: &[TreeNode]) -> bool {
        let has_direct_cetz_shape = nodes
            .iter()
            .any(|n| matches!(n, TreeNode::Shape { span, .. } if is_cetz_span(*span)));
        if !has_direct_cetz_shape {
            return false;
        }
        let has_flow_elements = nodes.iter().any(|n| match n {
            TreeNode::Element { elem, .. } => {
                elem.is::<HeadingElem>()
                    || elem.is::<ColbreakElem>()
                    || elem.is::<PagebreakElem>()
                    || elem.is::<LayoutElem>()
                    || elem.elem().name() == "context"
            }
            _ => false,
        });
        !has_flow_elements
    }

    pub(crate) fn extract_cetz_canvas(
        &mut self,
        nodes: &[TreeNode],
        explicit_box_size: Option<Size>,
        align: HAlignIr,
    ) -> Option<CetzCanvasIr> {
        if explicit_box_size.is_none() {
            if let Some((box_size, box_children)) = Self::find_cetz_box_group(nodes) {
                return self.extract_cetz_canvas(&box_children, Some(box_size), align);
            }
        }

        let mut paths: Vec<CetzPathDrawableIr> = Vec::new();
        let mut labels: Vec<CetzLabelDrawableIr> = Vec::new();
        let mut background: Option<ColorHex> = None;

        self.collect_cetz_drawables(
            nodes,
            Point::zero(),
            0.0,
            explicit_box_size,
            &mut background,
            &mut paths,
            &mut labels,
        );

        if paths.is_empty() && labels.is_empty() {
            return None;
        }

        let min_px = paths
            .iter()
            .map(|p| p.x_pt)
            .chain(labels.iter().map(|l| l.x_pt))
            .fold(f64::INFINITY, f64::min);
        let min_py = paths
            .iter()
            .map(|p| p.y_pt)
            .chain(labels.iter().map(|l| l.y_pt))
            .fold(f64::INFINITY, f64::min);
        if min_px.is_finite() && min_py.is_finite() {
            let should_shift = match explicit_box_size {
                Some(sz) => min_px > sz.x.to_pt() * 0.4 || min_py > sz.y.to_pt() * 0.4,
                None => min_px > 15.0 || min_py > 15.0,
            };
            if should_shift {
                for p in &mut paths {
                    p.x_pt = (p.x_pt - min_px).max(0.0);
                    p.y_pt = (p.y_pt - min_py).max(0.0);
                }
                for l in &mut labels {
                    l.x_pt = (l.x_pt - min_px).max(0.0);
                    l.y_pt = (l.y_pt - min_py).max(0.0);
                }
            }
        }

        let (width_pt, height_pt) = if let Some(sz) = explicit_box_size {
            (sz.x.to_pt().max(10.0), sz.y.to_pt().max(10.0))
        } else {
            let max_px = paths
                .iter()
                .map(|p| p.x_pt + p.width_pt)
                .fold(10.0_f64, f64::max);
            let max_py = paths
                .iter()
                .map(|p| p.y_pt + p.height_pt + 1.0)
                .fold(10.0_f64, f64::max);
            let max_lx = labels
                .iter()
                .map(|l| l.x_pt + l.width_pt)
                .fold(max_px, f64::max);
            let max_ly = labels
                .iter()
                .map(|l| (l.y_pt + l.height_pt - 4.0).max(0.0))
                .fold(max_py, f64::max);
            (max_lx, max_ly)
        };

        Some(CetzCanvasIr {
            width_pt,
            height_pt,
            background,
            paths,
            labels,
            align,
        })
    }

    pub(crate) fn find_cetz_box_group(nodes: &[TreeNode]) -> Option<(Size, Vec<TreeNode>)> {
        for node in nodes {
            match node {
                TreeNode::Group {
                    group, children, ..
                } => {
                    if Self::is_cetz_group_nodes(children) {
                        return Some((group.frame.size(), children.clone()));
                    }
                    if let Some(found) = Self::find_cetz_box_group(children) {
                        return Some(found);
                    }
                }
                TreeNode::Element { children, .. } => {
                    if let Some(found) = Self::find_cetz_box_group(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
