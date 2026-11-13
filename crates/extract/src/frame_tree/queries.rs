//! Generic queries and helpers on `Frame` and `TreeNode`.

use typst::foundations::Content;
use typst::layout::{Frame, FrameItem, FrameKind, Point, Transform};
use typst::model::{EnumElem, ListElem, TermsElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use crate::ExtractorContext;
use crate::frame_tree::{OpenElement, TreeNode};

impl<'a> ExtractorContext<'a> {
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

    pub(crate) fn is_page_furniture_elem(elem: &Content) -> bool {
        if elem.is::<typst::model::FootnoteEntry>() {
            return true;
        }
        if let Some(artifact) = elem.to_packed::<typst::pdf::ArtifactElem>() {
            return matches!(
                artifact.kind.get(typst::foundations::StyleChain::default()),
                typst::pdf::ArtifactKind::Header
                    | typst::pdf::ArtifactKind::Footer
                    | typst::pdf::ArtifactKind::Page
                    | typst::pdf::ArtifactKind::Background
                    | typst::pdf::ArtifactKind::Watermark
                    | typst::pdf::ArtifactKind::PageNumber
                    | typst::pdf::ArtifactKind::LineNumber
                    | typst::pdf::ArtifactKind::Bates
                    | typst::pdf::ArtifactKind::PaginationOther
            );
        }
        false
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
