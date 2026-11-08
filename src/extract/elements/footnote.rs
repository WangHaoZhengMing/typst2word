//! Footnote entry bodies (`footnote.entry`) -> blocks.

use typst::model::FootnoteEntry;
use typst::pdf::PdfMarkerTagKind;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_footnote_entry_blocks(&mut self, nodes: &[TreeNode]) -> Vec<BlockNode> {
        for node in nodes {
            if let TreeNode::Element { elem, children, .. } = node {
                if elem.is::<FootnoteEntry>() {
                    // Skip the `PdfMarkerTag::Label` prefix child so Word doesn't duplicate
                    // the footnote superscript number inside the footnote text!
                    let filtered: Vec<TreeNode> = children
                        .iter()
                        .filter(|c| {
                            !Self::is_pdf_marker(c, |k| matches!(k, PdfMarkerTagKind::Label))
                        })
                        .cloned()
                        .collect();
                    return self.extract_blocks_from_nodes(&filtered, 0, HAlignIr::Left);
                }
            }
        }
        self.extract_blocks_from_nodes(nodes, 0, HAlignIr::Left)
    }
}
