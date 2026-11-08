//! Inline footnote reference (`#footnote[...]`) -> `InlineNode::Footnote`.

use typst::introspection::Location;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::InlineNode;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_inline_footnote(
        &mut self,
        loc: Location,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
    ) {
        let id = self.next_footnote_id;
        self.next_footnote_id += 1;
        let number_text = Self::nodes_to_plain_text(children).trim().to_string();
        let content = self.footnotes.get(&loc).cloned().unwrap_or_default();
        self.report.footnotes_count += 1;
        out.push(InlineNode::Footnote {
            id,
            number_text: if number_text.is_empty() {
                id.to_string()
            } else {
                number_text
            },
            content,
        });
    }
}
