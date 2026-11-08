//! `link` destinations and link-content unwrapping.

use typst::foundations::Content;
use typst::model::{Destination, DirectLinkElem};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::InlineNode;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn find_link_destination(
        elem: &Content,
        children: &[TreeNode],
    ) -> Option<Destination> {
        if let Some(dl) = elem.to_packed::<DirectLinkElem>() {
            return Some(Destination::Location(dl.loc));
        }
        for node in children {
            match node {
                TreeNode::Link { dest, .. } => return Some(dest.clone()),
                TreeNode::Element {
                    elem: celem,
                    children: cchildren,
                    ..
                } => {
                    if let Some(found) = Self::find_link_destination(celem, cchildren) {
                        return Some(found);
                    }
                }
                TreeNode::Group {
                    children: cchildren,
                    ..
                } => {
                    if let Some(found) = Self::find_link_destination(elem, cchildren) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn unwrap_link_inlines(inlines: Vec<InlineNode>) -> Vec<InlineNode> {
        let mut out = Vec::new();
        for item in inlines {
            if let InlineNode::Link { content, .. } = item {
                out.extend(Self::unwrap_link_inlines(content));
            } else {
                out.push(item);
            }
        }
        out
    }
}
