//! Inline links (`#link`), cross-references (`@ref`), and citations (`@cite`).

use typst::foundations::Content;
use typst::model::{
    CiteElem, CiteGroup, Destination, DirectLinkElem, LinkElem, LinkMarker, RefElem,
};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use crate::inline::ActiveInlineState;
use ir::{InlineNode, LinkTargetIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn try_extract_inline_link_or_ref(
        &mut self,
        elem: &Content,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) -> bool {
        // 1. Cross-reference (`RefElem`).
        if let Some(rf) = elem.to_packed::<RefElem>() {
            let target_str = rf.target.resolve().as_str().to_string();
            let anchor = self
                .label_bookmarks
                .get(&target_str)
                .cloned()
                .unwrap_or_else(|| target_str.clone());
            let mut ref_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut ref_inlines, state, last_y, last_x_end);
            let clean_inlines = Self::unwrap_link_inlines(ref_inlines);
            out.push(InlineNode::Ref {
                target: anchor,
                content: clean_inlines,
            });
            return true;
        }

        // 2. Citation (`CiteGroup`).
        if let Some(cg) = elem.to_packed::<CiteGroup>() {
            let keys: Vec<String> = cg
                .children
                .iter()
                .filter_map(|c| c.to_packed::<CiteElem>())
                .map(|c| c.key.resolve().as_str().to_string())
                .collect();
            let mut cite_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut cite_inlines, state, last_y, last_x_end);
            let clean_inlines = Self::unwrap_link_inlines(cite_inlines);
            out.push(InlineNode::Citation {
                keys,
                content: clean_inlines,
            });
            return true;
        }

        // 3. Hyperlink (`LinkMarker`, `LinkElem`, `DirectLinkElem`).
        if elem.is::<LinkMarker>() || elem.is::<LinkElem>() || elem.is::<DirectLinkElem>() {
            let dest = Self::find_link_destination(elem, children);
            let target = match dest {
                Some(Destination::Url(u)) => Some(LinkTargetIr::Url(u.to_string())),
                Some(Destination::Location(target_loc)) => {
                    let anchor = self
                        .bookmarks
                        .get(&target_loc)
                        .cloned()
                        .unwrap_or_else(|| format!("_typst_loc_{:x}", target_loc.hash()));
                    Some(LinkTargetIr::Anchor(anchor))
                }
                Some(Destination::Position(_)) | None => None,
            };
            let mut link_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut link_inlines, state, last_y, last_x_end);
            let had_inner_link = link_inlines
                .iter()
                .any(|inl| matches!(inl, InlineNode::Link { .. }));
            let clean_inlines = Self::unwrap_link_inlines(link_inlines);
            if !had_inner_link {
                self.report.links_count += 1;
            }
            out.push(InlineNode::Link {
                target,
                content: clean_inlines,
            });
            return true;
        }

        false
    }

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
