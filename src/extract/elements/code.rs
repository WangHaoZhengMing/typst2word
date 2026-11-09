//! Block `raw` -> `BlockNode::CodeBlock` with syntax-highlighted runs per line.

use typst::foundations::{Packed, StyleChain};
use typst::text::{RawElem, RawLine};
use typst::visualize::Geometry;

use crate::color::paint_to_hex;
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
                        font_family: None,
                        is_code: true,
                        ..Default::default()
                    }]);
                }
            }
        }

        // Only attach background fill or border stroke if the code block actually has a background shape!
        let (fill, stroke) = Self::find_code_block_box_style(children);

        BlockNode::CodeBlock {
            lang,
            lines,
            fill,
            stroke,
        }
    }

    pub(crate) fn find_code_block_box_style(
        nodes: &[TreeNode],
    ) -> (Option<String>, Option<StrokeIr>) {
        for node in nodes {
            match node {
                TreeNode::Shape { shape, .. } => {
                    if matches!(shape.geometry, Geometry::Rect(_)) {
                        let fill = shape.fill.as_ref().and_then(paint_to_hex);
                        let stroke = shape.stroke.as_ref().map(Self::fixed_stroke_to_ir);
                        if fill.is_some() || stroke.is_some() {
                            return (fill, stroke);
                        }
                    }
                }
                TreeNode::Group { children, .. } | TreeNode::Element { children, .. } => {
                    let (f, s) = Self::find_code_block_box_style(children);
                    if f.is_some() || s.is_some() {
                        return (f, s);
                    }
                }
                _ => {}
            }
        }
        (None, None)
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
                                font_family: None,
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
