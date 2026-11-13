//! Block `quote` -> `BlockNode::Quote`.

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::model::QuoteElem;

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, HAlignIr, SemanticRoleIr};

impl<'a> ExtractorContext<'a> {
    /// Block `QuoteElem` -> `BlockNode::Quote` (last paragraph becomes the attribution).
    pub(crate) fn extract_quote_block(
        &mut self,
        quote: &Packed<QuoteElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        list_level: usize,
        inherited_align: HAlignIr,
    ) {
        let id = self.alloc_node_with_fragments(
            quote.span(),
            Some(loc),
            Some(SemanticRoleIr::Quote),
            children,
        );
        self.parent_node_stack.push(id);
        let chain = self.scoped_style_chain(Some(loc));
        let has_attr = quote.attribution.get_ref(chain).is_some();
        let mut body = self.extract_blocks_from_nodes(children, list_level, inherited_align);
        self.parent_node_stack.pop();
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
        blocks.push(BlockNode::Quote {
            id,
            body,
            attribution,
        });
    }
}
