//! CeTZ canvases (`cetz.canvas(...)`) -> `CetzCanvasIr`.

pub(crate) mod collect;

use typst::layout::{ColbreakElem, GroupItem, LayoutElem, PagebreakElem, Point, Size};
use typst::model::{EnumElem, FootnoteElem, HeadingElem, ListElem, TableElem, TermsElem};
use typst::text::RawElem;
use typst::visualize::{CurveElem, Geometry};

use compiler::compile::is_cetz_span;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{
    CetzCanvasIr, CetzLabelDrawableIr, CetzPathDrawableIr, ColorHex, HAlignIr, SemanticRoleIr,
};

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

    /// Structural detector for any 2D vector diagram / plot node tree (CeTZ, Lilaq, Fletcher,
    /// custom `#curve`/`#line`/`#place` graphics) without package-name hardcoding.
    ///
    /// A node slice qualifies when it contains no document-flow structures (headings, lists,
    /// semantic tables, code blocks, page/column breaks, footnotes, or long prose paragraphs)
    /// and contains a rich multi-primitive vector graphic (`>= 5` drawable shapes with `>= 3`
    /// non-rectangle `Line`/`Curve` primitives).
    pub(crate) fn is_vector_canvas_nodes(children: &[TreeNode]) -> bool {
        #[derive(Default)]
        struct CanvasScan {
            disqualified: bool,
            has_table: bool,
            drawable_shapes: usize,
            vector_primitives: usize,
            total_text_chars: usize,
        }
        fn scan(nodes: &[TreeNode], st: &mut CanvasScan) {
            if st.disqualified {
                return;
            }
            for node in nodes {
                match node {
                    TreeNode::Shape { shape, .. } => {
                        if shape.fill.is_some() || shape.stroke.is_some() {
                            st.drawable_shapes += 1;
                            if matches!(shape.geometry, Geometry::Line(_) | Geometry::Curve(_)) {
                                st.vector_primitives += 1;
                            }
                        }
                    }
                    TreeNode::Text { text, .. } => {
                        let chars = text.text.chars().count();
                        if chars > 80 {
                            st.disqualified = true;
                            return;
                        }
                        st.total_text_chars += chars;
                        if st.total_text_chars > 400 {
                            st.disqualified = true;
                            return;
                        }
                    }
                    TreeNode::Element { elem, children, .. } => {
                        if elem.is::<HeadingElem>()
                            || elem.is::<ListElem>()
                            || elem.is::<EnumElem>()
                            || elem.is::<TermsElem>()
                            || elem.is::<ColbreakElem>()
                            || elem.is::<PagebreakElem>()
                            || elem.is::<FootnoteElem>()
                            || elem.is::<RawElem>()
                        {
                            st.disqualified = true;
                            return;
                        }
                        if elem.is::<TableElem>() {
                            st.has_table = true;
                        }
                        scan(children, st);
                    }
                    TreeNode::Group { children, .. } => scan(children, st),
                    _ => {}
                }
            }
        }
        let mut st = CanvasScan::default();
        scan(children, &mut st);
        // Plotting packages (e.g. Lilaq) often use a small `table` for the plot legend alongside
        // dozens of axes/grid/curve vector primitives; only allow `TableElem` when the vector
        // primitive count clearly establishes a 2D plot/diagram (`>= 10` lines/curves).
        !st.disqualified
            && st.drawable_shapes >= 5
            && st.vector_primitives >= 3
            && (!st.has_table || st.vector_primitives >= 10)
    }

    pub(crate) fn is_vector_canvas_group(group: &GroupItem, children: &[TreeNode]) -> bool {
        let sz = group.frame.size();
        sz.x.to_pt() >= 20.0 && sz.y.to_pt() >= 20.0 && Self::is_vector_canvas_nodes(children)
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
                Some(sz) => {
                    min_px < -0.5
                        || min_py < -0.5
                        || min_px > sz.x.to_pt() * 0.4
                        || min_py > sz.y.to_pt() * 0.4
                }
                None => min_px < -0.5 || min_py < -0.5 || min_px > 15.0 || min_py > 15.0,
            };
            if should_shift {
                let shift_x = if min_px < 0.0 || explicit_box_size.is_none() || min_px > explicit_box_size.map_or(0.0, |s| s.x.to_pt() * 0.4) {
                    min_px
                } else {
                    0.0
                };
                let shift_y = if min_py < 0.0 || explicit_box_size.is_none() || min_py > explicit_box_size.map_or(0.0, |s| s.y.to_pt() * 0.4) {
                    min_py
                } else {
                    0.0
                };
                for p in &mut paths {
                    p.x_pt = (p.x_pt - shift_x).max(0.0);
                    p.y_pt = (p.y_pt - shift_y).max(0.0);
                }
                for l in &mut labels {
                    l.x_pt = (l.x_pt - shift_x).max(0.0);
                    l.y_pt = (l.y_pt - shift_y).max(0.0);
                }
            }
        }

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
        let (width_pt, height_pt) = if let Some(sz) = explicit_box_size {
            (
                sz.x.to_pt().max(max_lx).max(10.0),
                sz.y.to_pt().max(max_ly).max(10.0),
            )
        } else {
            (max_lx, max_ly)
        };

        let id = self.alloc_node_with_fragments(
            typst::syntax::Span::detached(),
            self.scope.active_scope_loc,
            Some(SemanticRoleIr::CetzCanvas),
            nodes,
        );
        // Block-level `#align(center)[#canvas(...)]` realizes as a style (no `AlignElem` tag),
        // so honor the active `StyleChain` alignment when the caller only inherited `Left`.
        let align = if align == HAlignIr::Left {
            let chain = self.scoped_style_chain(self.scope.active_scope_loc);
            let (style_align, _) = Self::alignment_to_ir(typst::foundations::Smart::Custom(
                chain.get(typst::layout::AlignElem::alignment),
            ));
            if matches!(style_align, HAlignIr::Center | HAlignIr::Right) {
                style_align
            } else {
                align
            }
        } else {
            align
        };
        Some(CetzCanvasIr {
            id,
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
