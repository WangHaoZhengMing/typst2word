//! Frame tree construction: merges the introspection tag stream (`Tag::Start` /
//! `Tag::End`) with laid-out `FrameItem`s into a `TreeNode` tree, plus generic
//! tree queries used by all element extractors.

use typst::foundations::Content;
use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem, FrameKind, GroupItem, Inherit, Point, Size, Transform};
use typst::model::{Destination, EnumElem, ListElem, TermsElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};
use typst::syntax::Span;
use typst::text::TextItem;
use typst::visualize::{Image, Shape};

use crate::extract::ExtractorContext;

/// Tree representation unifying `Tag::Start` / `Tag::End` semantic nodes with
/// laid-out `FrameItem` visual nodes.
#[derive(Debug, Clone)]
pub(crate) enum TreeNode {
    Element {
        elem: Content,
        loc: Location,
        pos: Point,
        children: Vec<TreeNode>,
    },
    Group {
        group: GroupItem,
        pos: Point,
        children: Vec<TreeNode>,
    },
    Text {
        text: TextItem,
        pos: Point,
    },
    Shape {
        shape: Shape,
        span: Span,
        pos: Point,
    },
    Image {
        image: Image,
        size: Size,
        span: Span,
        pos: Point,
    },
    Link {
        dest: Destination,
        #[allow(dead_code)]
        size: Size,
        #[allow(dead_code)]
        pos: Point,
    },
    PageBreak,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenElement {
    pub(crate) elem: Content,
    pub(crate) loc: Location,
    pub(crate) pos: Point,
    pub(crate) children: Vec<TreeNode>,
}

impl<'a> ExtractorContext<'a> {
    /// Pre-scan a frame to collect `GroupItem`s that have `group.parent.is_some()`.
    pub(crate) fn collect_detached_parent_groups(&mut self, frame: &Frame, offset: Point) {
        for (pos, item) in frame.items() {
            let abs_pos = offset + *pos;
            if let FrameItem::Group(group) = item {
                if let Some(parent) = group.parent {
                    match parent.inherit {
                        Inherit::No => {
                            self.footnote_raw_groups
                                .entry(parent.location)
                                .or_default()
                                .push((abs_pos, group.clone()));
                        }
                        Inherit::Yes => {
                            self.logical_child_groups
                                .entry(parent.location)
                                .or_default()
                                .push((abs_pos, group.clone()));
                        }
                    }
                } else {
                    self.collect_detached_parent_groups(&group.frame, abs_pos);
                }
            }
        }
    }

    /// Build a hierarchical `Vec<TreeNode>` from `frame`, flattening soft untransformed
    /// line frames while keeping hard/transformed groups and pairing `Tag::Start` / `Tag::End`.
    pub(crate) fn build_frame_tree(&mut self, frame: &Frame, base_offset: Point) -> Vec<TreeNode> {
        let mut flat_items: Vec<(Point, FrameItem)> = Vec::new();
        Self::flatten_soft_groups(frame, base_offset, &mut flat_items);

        let mut root_nodes: Vec<TreeNode> = Vec::new();
        let mut stack: Vec<OpenElement> = Vec::new();

        for (pos, item) in flat_items {
            match item {
                FrameItem::Tag(Tag::Start(elem, _flags)) => {
                    let loc = elem
                        .location()
                        .unwrap_or_else(|| Location::new(typst_utils::hash128(&elem)));
                    stack.push(OpenElement {
                        elem,
                        loc,
                        pos,
                        children: Vec::new(),
                    });
                }
                FrameItem::Tag(Tag::End(loc, _key, _flags)) => {
                    if let Some(match_idx) = stack.iter().rposition(|open| open.loc == loc) {
                        while stack.len() > match_idx {
                            let mut closed = stack.pop().unwrap();
                            // Attach any logical child groups registered for `closed.loc`.
                            if let Some(detached) = self.logical_child_groups.remove(&closed.loc) {
                                for (gpos, grp) in detached {
                                    let gchildren = self.build_frame_tree(&grp.frame, gpos);
                                    closed.children.extend(gchildren);
                                }
                            }
                            let node = TreeNode::Element {
                                elem: closed.elem,
                                loc: closed.loc,
                                pos: closed.pos,
                                children: closed.children,
                            };
                            Self::push_node(&mut root_nodes, &mut stack, node);
                        }
                    }
                }
                FrameItem::Group(group) => {
                    if group.parent.is_some() {
                        // Handled via `footnote_raw_groups` / `logical_child_groups`.
                        continue;
                    }
                    let children = self.build_frame_tree(&group.frame, Point::zero());
                    let node = TreeNode::Group {
                        group,
                        pos,
                        children,
                    };
                    Self::push_node(&mut root_nodes, &mut stack, node);
                }
                FrameItem::Text(text) => {
                    Self::push_node(&mut root_nodes, &mut stack, TreeNode::Text { text, pos });
                }
                FrameItem::Shape(shape, span) => {
                    Self::push_node(
                        &mut root_nodes,
                        &mut stack,
                        TreeNode::Shape { shape, span, pos },
                    );
                }
                FrameItem::Image(image, size, span) => {
                    Self::push_node(
                        &mut root_nodes,
                        &mut stack,
                        TreeNode::Image {
                            image,
                            size,
                            span,
                            pos,
                        },
                    );
                }
                FrameItem::Link(dest, size) => {
                    Self::push_node(
                        &mut root_nodes,
                        &mut stack,
                        TreeNode::Link { dest, size, pos },
                    );
                }
            }
        }

        // Close any remaining open tags at the end of the frame.
        while let Some(mut closed) = stack.pop() {
            if let Some(detached) = self.logical_child_groups.remove(&closed.loc) {
                for (gpos, grp) in detached {
                    let gchildren = self.build_frame_tree(&grp.frame, gpos);
                    closed.children.extend(gchildren);
                }
            }
            let node = TreeNode::Element {
                elem: closed.elem,
                loc: closed.loc,
                pos: closed.pos,
                children: closed.children,
            };
            Self::push_node(&mut root_nodes, &mut stack, node);
        }

        root_nodes
    }

    pub(crate) fn flatten_soft_groups(
        frame: &Frame,
        offset: Point,
        out: &mut Vec<(Point, FrameItem)>,
    ) {
        for (pos, item) in frame.items() {
            let abs_pos = offset + *pos;
            if let FrameItem::Group(group) = item {
                if group.parent.is_none()
                    && group.frame.kind() == FrameKind::Soft
                    && group.transform == Transform::identity()
                    && group.clip.is_none()
                    && group.label.is_none()
                {
                    Self::flatten_soft_groups(&group.frame, abs_pos, out);
                    continue;
                }
            }
            out.push((abs_pos, item.clone()));
        }
    }

    pub(crate) fn push_node(root: &mut Vec<TreeNode>, stack: &mut [OpenElement], node: TreeNode) {
        if let Some(top) = stack.last_mut() {
            top.children.push(node);
        } else {
            root.push(node);
        }
    }

    pub(crate) fn is_pdf_marker(node: &TreeNode, pred: impl Fn(&PdfMarkerTagKind) -> bool) -> bool {
        if let TreeNode::Element { elem, .. } = node {
            if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                return pred(&marker.kind);
            }
        }
        false
    }

    pub(crate) fn collect_pdf_marker_elements(
        nodes: &[TreeNode],
        out: &mut Vec<(PdfMarkerTagKind, Vec<TreeNode>)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        out.push((marker.kind.clone(), children.clone()));
                    } else {
                        // Do not recurse into nested ListElem/EnumElem/TermsElem when collecting
                        // top-level list items!
                        if elem.is::<ListElem>() || elem.is::<EnumElem>() || elem.is::<TermsElem>()
                        {
                            continue;
                        }
                        Self::collect_pdf_marker_elements(children, out);
                    }
                }
                TreeNode::Group { children, .. } => {
                    Self::collect_pdf_marker_elements(children, out);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn nodes_to_plain_text(nodes: &[TreeNode]) -> String {
        let mut s = String::new();
        for node in nodes {
            match node {
                TreeNode::Text { text, .. } => s.push_str(&text.text),
                TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                    s.push_str(&Self::nodes_to_plain_text(children));
                }
                _ => {}
            }
        }
        s
    }
}
