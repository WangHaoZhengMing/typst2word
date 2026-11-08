//! Block `raw` -> `BlockNode::CodeBlock` with syntax-highlighted runs per line.

use typst::foundations::{Packed, StyleChain};
use typst::text::{RawElem, RawLine};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{BlockNode, StrokeIr, TextRunIr};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Code Blocks, Figures, Equations, Outlines, Bibliographies
    // =========================================================================
    pub(crate) fn extract_code_block(
        &self,
        raw: &Packed<RawElem>,
        children: &[TreeNode],
    ) -> BlockNode {
        let lang = raw
            .lang
            .get_ref(StyleChain::default())
            .as_ref()
            .map(|s| s.to_string());

        let mut lines: Vec<Vec<TextRunIr>> = Vec::new();
        Self::collect_raw_lines(children, &mut lines);

        // Fallback if no `RawLine` tags were found: use `raw.lines` or `raw.text`.
        if lines.is_empty() {
            if let Some(raw_lines) = raw.lines.as_deref() {
                for rl in raw_lines {
                    lines.push(vec![TextRunIr {
                        text: rl.text.to_string(),
                        font_family: Some("Consolas".to_string()),
                        is_code: true,
                        ..Default::default()
                    }]);
                }
            }
        }

        BlockNode::CodeBlock {
            lang,
            lines,
            fill: Some("F6F8FA".to_string()),
            stroke: Some(StrokeIr {
                color: "D0D7DE".to_string(),
                thickness_pt: 0.75,
                ..Default::default()
            }),
        }
    }

    pub(crate) fn collect_raw_lines(nodes: &[TreeNode], out: &mut Vec<Vec<TextRunIr>>) {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(raw_line) = elem.to_packed::<RawLine>() {
                        let mut runs = Vec::new();
                        Self::collect_raw_line_runs(children, &mut runs);
                        if runs.is_empty() {
                            runs.push(TextRunIr {
                                text: raw_line.text.to_string(),
                                font_family: Some("Consolas".to_string()),
                                is_code: true,
                                ..Default::default()
                            });
                        }
                        out.push(runs);
                    } else {
                        Self::collect_raw_lines(children, out);
                    }
                }
                TreeNode::Group { children, .. } => {
                    Self::collect_raw_lines(children, out);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn collect_raw_line_runs(nodes: &[TreeNode], runs: &mut Vec<TextRunIr>) {
        for node in nodes {
            match node {
                TreeNode::Text { text, .. } => {
                    let state = ActiveInlineState {
                        is_code: true,
                        ..Default::default()
                    };
                    runs.push(Self::text_item_to_run(text, &state));
                }
                TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                    Self::collect_raw_line_runs(children, runs);
                }
                _ => {}
            }
        }
    }
}
