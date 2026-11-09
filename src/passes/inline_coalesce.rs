//! Pass 2 (`PipelinePassKind::InlineCoalescing`):
//! Merges adjacent compatible `TextRunIr`s across all inline sequences after font normalization.

use crate::ir::{DocumentIr, InlineNode};
use crate::passes::visit_document_inline_lists_mut;

/// Run Pass 2 (`InlineCoalescing`) across all inline lists in `doc`.
pub fn run_inline_coalescing_pass(doc: &mut DocumentIr) {
    visit_document_inline_lists_mut(doc, &mut coalesce_adjacent_text_runs);
}

/// Merge adjacent `InlineNode::Text` runs that share identical formatting and normalized font family.
pub fn coalesce_adjacent_text_runs(inlines: &mut Vec<InlineNode>) {
    let mut i = 1;
    while i < inlines.len() {
        let can_merge = match (&inlines[i - 1], &inlines[i]) {
            (InlineNode::Text(a), InlineNode::Text(b)) => {
                a.font_family == b.font_family
                    && a.font_size_pt.map(|v| (v * 10.0).round() as i64)
                        == b.font_size_pt.map(|v| (v * 10.0).round() as i64)
                    && a.bold == b.bold
                    && a.italic == b.italic
                    && a.underline.is_some() == b.underline.is_some()
                    && a.strike.is_some() == b.strike.is_some()
                    && a.overline.is_some() == b.overline.is_some()
                    && a.color == b.color
                    && a.highlight == b.highlight
                    && a.border.is_some() == b.border.is_some()
                    && a.script == b.script
                    && a.smallcaps == b.smallcaps
                    && a.is_code == b.is_code
            }
            _ => false,
        };
        if can_merge {
            let InlineNode::Text(b) = inlines.remove(i) else {
                unreachable!()
            };
            if let InlineNode::Text(a) = &mut inlines[i - 1] {
                a.text.push_str(&b.text);
            }
        } else {
            i += 1;
        }
    }
}
