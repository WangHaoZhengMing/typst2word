//! `par` -> `BlockNode::Paragraph`.

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::layout::Point;
use typst::model::ParElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, HAlignIr, InlineNode, ParagraphStyleIr};

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

        let content = self.extract_inlines_flat(children);
        if content.is_empty() {
            return;
        }

        let chain = self.style_chain();
        let default_font_pt =
            Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
        let base_font_pt = content
            .iter()
            .find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_size_pt,
                _ => None,
            })
            .unwrap_or(default_font_pt)
            .max(6.0);

        let leading_pt = Self::resolve_length_pt(par.leading.get(chain), base_font_pt);
        let spacing_pt = Self::resolve_length_pt(par.spacing.get(chain), base_font_pt);
        // Word's single line box already includes ~0.55em of internal vertical leading,
        // so subtract 0.55em from Typst's 1em-box-to-1em-box `par.spacing` to match visually.
        let space_after_pt = (spacing_pt - 0.55 * base_font_pt).max(2.0);

        let mut first_line_indent_pt =
            Self::resolve_length_pt(par.first_line_indent.get(chain).amount(), base_font_pt);
        let mut hanging_indent_pt =
            Self::resolve_length_pt(par.hanging_indent.get(chain), base_font_pt);

        let align = if par.justify.get(chain) {
            HAlignIr::Justify
        } else {
            let inferred = self.infer_node_halign(children).unwrap_or(inherited_align);
            if inferred == HAlignIr::Left && inherited_align != HAlignIr::Left {
                inherited_align
            } else {
                inferred
            }
        };

        if matches!(align, HAlignIr::Left | HAlignIr::Justify)
            && first_line_indent_pt < 0.5
            && hanging_indent_pt < 0.5
        {
            let (vis_first, vis_hang) = Self::infer_paragraph_line_indents(children);
            if vis_first > 2.0 {
                first_line_indent_pt = vis_first;
            } else if vis_hang > 2.0 {
                hanging_indent_pt = vis_hang;
            }
        }

        let style = ParagraphStyleIr {
            align,
            first_line_indent_pt,
            hanging_indent_pt,
            space_after_pt,
            line_spacing_pt: Some(leading_pt),
            ..Default::default()
        };

        self.report.paragraphs_count += 1;
        blocks.push(BlockNode::Paragraph {
            content,
            style,
            bookmark: self.bookmarks.get(&loc).cloned(),
        });
    }

    pub(crate) fn infer_paragraph_line_indents(nodes: &[TreeNode]) -> (f64, f64) {
        let mut line_starts: Vec<(f64, f64)> = Vec::new();
        Self::collect_line_left_edges(nodes, Point::zero(), &mut line_starts);
        if line_starts.len() < 2 {
            return (0.0, 0.0);
        }
        let x1 = line_starts[0].1;
        let x2 = line_starts[1].1;
        if x1 - x2 > 3.0 && x1 - x2 < 72.0 {
            (x1 - x2, 0.0)
        } else if x2 - x1 > 3.0 && x2 - x1 < 72.0 {
            (0.0, x2 - x1)
        } else {
            (0.0, 0.0)
        }
    }

    pub(crate) fn collect_line_left_edges(
        nodes: &[TreeNode],
        offset: Point,
        lines: &mut Vec<(f64, f64)>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if text.text.trim().is_empty() {
                        continue;
                    }
                    let x = (offset.x + pos.x).to_pt();
                    let y = (offset.y + pos.y).to_pt();
                    if let Some(existing) = lines.iter_mut().find(|(ly, _)| (*ly - y).abs() < 3.0)
                    {
                        existing.1 = existing.1.min(x);
                    } else {
                        lines.push((y, x));
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_line_left_edges(children, offset + *pos, lines);
                }
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<typst::math::EquationElem>()
                        || elem.is::<typst::text::SubElem>()
                        || elem.is::<typst::text::SuperElem>()
                    {
                        continue;
                    }
                    Self::collect_line_left_edges(children, offset, lines);
                }
                _ => {}
            }
        }
    }
}
