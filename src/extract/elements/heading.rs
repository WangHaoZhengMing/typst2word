//! `title` / `heading` -> `BlockNode::Title` / `BlockNode::Heading`.

use typst::foundations::{Packed, StyleChain};
use typst::introspection::Location;
use typst::model::HeadingElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr, InlineNode};

impl<'a> ExtractorContext<'a> {
    /// `TitleElem` -> `BlockNode::Title`.
    pub(crate) fn extract_title_block(
        &mut self,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
    ) {
        let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
        let content = self.extract_inlines_flat_with_bold(children, true);
        self.report.headings_count += 1;
        blocks.push(BlockNode::Title { content, align });
    }

    /// `HeadingElem` -> `BlockNode::Heading` (with numbering prefix and bookmark).
    pub(crate) fn extract_heading_block(
        &mut self,
        heading: &Packed<HeadingElem>,
        loc: Location,
        children: &[TreeNode],
        blocks: &mut Vec<BlockNode>,
        inherited_align: HAlignIr,
    ) {
        let level = heading.resolve_level(StyleChain::default()).get();
        let align = self.infer_node_halign(children).unwrap_or(inherited_align);
        let bookmark = self.bookmarks.get(&loc).cloned();
        let (numbering, content) = self.extract_heading_numbering_and_body(heading, children);
        self.report.headings_count += 1;
        blocks.push(BlockNode::Heading {
            level,
            numbering,
            content,
            bookmark,
            align,
        });
    }

    // =========================================================================
    // Headings, Lists, Enums, Terms
    // =========================================================================
    pub(crate) fn extract_heading_numbering_and_body(
        &mut self,
        heading: &Packed<HeadingElem>,
        children: &[TreeNode],
    ) -> (Option<String>, Vec<InlineNode>) {
        let all_inlines = self.extract_inlines_flat_with_bold(children, true);
        if heading.numbering.get_ref(StyleChain::default()).is_none() {
            return (None, all_inlines);
        }

        // First check synthesized `heading.numbers` if populated by Typst synthesis.
        if let Some(ref synth_eco) = heading.numbers {
            let synth_num = synth_eco.trim();
            if !synth_num.is_empty() {
                if let Some(stripped) =
                    Self::strip_leading_prefix_from_inlines(&all_inlines, synth_num)
                {
                    if !stripped.is_empty() {
                        return (Some(synth_num.to_string()), stripped);
                    }
                }
            }
        }

        // Fallback: look for the first `HorizontalSpace` (or space run after numbering text).
        for (idx, inline) in all_inlines.iter().enumerate() {
            if matches!(inline, InlineNode::HorizontalSpace { .. }) && idx > 0 {
                let prefix_str = Self::inlines_to_plain_text(&all_inlines[..idx])
                    .trim()
                    .to_string();
                let body_inlines = all_inlines[idx + 1..].to_vec();
                if !prefix_str.is_empty() && !body_inlines.is_empty() {
                    return (Some(prefix_str), body_inlines);
                }
            }
        }

        (None, all_inlines)
    }

    pub(crate) fn strip_leading_prefix_from_inlines(
        inlines: &[InlineNode],
        prefix: &str,
    ) -> Option<Vec<InlineNode>> {
        let mut rem_prefix = prefix;
        let mut out = Vec::new();
        let mut prefix_done = false;

        for inline in inlines {
            if prefix_done {
                if out.is_empty() && matches!(inline, InlineNode::HorizontalSpace { .. }) {
                    continue;
                }
                if out.is_empty() {
                    if let InlineNode::Text(r) = inline {
                        let trimmed = r.text.trim_start();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if trimmed.len() != r.text.len() {
                            let mut nr = r.clone();
                            nr.text = trimmed.to_string();
                            out.push(InlineNode::Text(nr));
                            continue;
                        }
                    }
                }
                out.push(inline.clone());
                continue;
            }

            match inline {
                InlineNode::Text(r) => {
                    let t = r.text.as_str();
                    if let Some(rest_run) = t.strip_prefix(rem_prefix) {
                        rem_prefix = "";
                        prefix_done = true;
                        let trimmed_rest = rest_run.trim_start();
                        if !trimmed_rest.is_empty() {
                            let mut nr = r.clone();
                            nr.text = trimmed_rest.to_string();
                            out.push(InlineNode::Text(nr));
                        }
                    } else if let Some(rest_pref) = rem_prefix.strip_prefix(t) {
                        rem_prefix = rest_pref;
                        if rem_prefix.is_empty() {
                            prefix_done = true;
                        }
                    } else {
                        return None;
                    }
                }
                InlineNode::HorizontalSpace { .. } if rem_prefix.is_empty() => {
                    prefix_done = true;
                }
                _ => return None,
            }
        }

        if prefix_done { Some(out) } else { None }
    }
}
