//! `par` -> `BlockNode::Paragraph`.

use typst::foundations::{Packed, StyleChain};
use typst::introspection::Location;
use typst::model::ParElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr, ParagraphStyleIr};

impl<'a> ExtractorContext<'a> {
    /// `ParElem` -> `BlockNode::Paragraph` (or a CeTZ canvas if the paragraph only wraps one).
    pub(crate) fn extract_paragraph_block(
        &mut self,
        par: &Packed<ParElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        inherited_align: HAlignIr,
    ) {
        // Check if this paragraph is actually just a wrapper around a single CeTZ canvas or block shape!
        if children.len() == 1 {
            if let TreeNode::Group {
                group,
                children: gchildren,
                ..
            } = &children[0]
            {
                if Self::is_cetz_group_nodes(gchildren) {
                    if let Some(canvas) = self.extract_cetz_canvas(
                        gchildren,
                        Some(group.frame.size()),
                        inherited_align,
                    ) {
                        self.report.cetz_canvases_count += 1;
                        blocks.push(BlockNode::CetzCanvas(canvas));
                        return;
                    }
                }
            }
        }

        let mut style = ParagraphStyleIr {
            align: if par.justify.get(StyleChain::default()) {
                HAlignIr::Justify
            } else {
                self.infer_node_halign(children).unwrap_or(inherited_align)
            },
            first_line_indent_pt: par
                .first_line_indent
                .get(StyleChain::default())
                .amount()
                .abs
                .to_pt(),
            hanging_indent_pt: par.hanging_indent.get(StyleChain::default()).abs.to_pt(),
            space_after_pt: par.spacing.get(StyleChain::default()).abs.to_pt().max(4.0),
            line_spacing_pt: Some(par.leading.get(StyleChain::default()).abs.to_pt()),
            ..Default::default()
        };
        if style.align == HAlignIr::Left && inherited_align != HAlignIr::Left {
            style.align = inherited_align;
        }

        let content = self.extract_inlines_flat(children);
        if !content.is_empty() {
            self.report.paragraphs_count += 1;
            blocks.push(BlockNode::Paragraph {
                content,
                style,
                bookmark: self.bookmarks.get(&loc).cloned(),
            });
        }
    }
}
