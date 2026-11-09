//! Block `quote` -> `BlockNode::Quote`.

use typst::foundations::{Packed, StyleChain};
use typst::model::QuoteElem;

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, HAlignIr};

impl<'a> ExtractorContext<'a> {
    /// Block `QuoteElem` -> `BlockNode::Quote` (last paragraph becomes the attribution).
    pub(crate) fn extract_quote_block(
        &mut self,
        quote: &Packed<QuoteElem>,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let has_attr = quote.attribution.get_ref(StyleChain::default()).is_some();
        let mut body = self.extract_blocks_from_nodes(children, list_level, inherited_align);
        let attribution = if has_attr && body.len() >= 2 {
            match body.pop() {
                Some(BlockNode::Paragraph { content, .. }) => Some(content),
                Some(other) => {
                    body.push(other);
                    None
                }
                None => None,
            }
        } else {
            None
        };
        self.report.quotes_count += 1;
        blocks.push(BlockNode::Quote { body, attribution });
    }
}
