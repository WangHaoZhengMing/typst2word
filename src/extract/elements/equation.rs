//! `equation` -> `EquationIr` (OMML via `crate::math`).

use typst::foundations::Packed;
use typst::introspection::Location;
use typst::math::EquationElem;
use typst::model::{Numbering, ParElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::extract::math::extract_equation_math_nodes;
use crate::ir::EquationIr;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_equation(
        &mut self,
        eq: &Packed<EquationElem>,
        loc: Location,
        children: &[TreeNode],
    ) -> EquationIr {
        let chain = self.style_chain();
        let is_block = eq.block.get(chain);
        let body = extract_equation_math_nodes(eq, chain);
        let source_text = self
            .compiled
            .source_snippet(self.world, eq.span())
            .unwrap_or_else(|| eq.body.plain_text().to_string());
        let bookmark = self.bookmarks.get(&loc).cloned();

        // Extract resolved equation number (e.g. `(1)`, `(2)`, `(3)`) from laid-out `children`
        // or from the active `#set math.equation(numbering: ...)` pattern.
        let cfg_numbering = eq.numbering.get_cloned(chain);
        let numbering_text = if is_block {
            if let Some(found) = Self::find_equation_number_text(children) {
                self.next_equation_number += 1;
                Some(found)
            } else if let Some(numbering) = cfg_numbering {
                let n = self.next_equation_number as u64;
                self.next_equation_number += 1;
                match numbering {
                    Numbering::Pattern(pat) => pat
                        .apply(None, &[n])
                        .ok()
                        .map(|s| s.to_string())
                        .or_else(|| Some(format!("({n})"))),
                    Numbering::Func(_) => Some(format!("({n})")),
                }
            } else {
                None
            }
        } else {
            None
        };

        EquationIr {
            is_block,
            body,
            source_text,
            numbering_text,
            bookmark,
        }
    }

    pub(crate) fn find_equation_number_text(nodes: &[TreeNode]) -> Option<String> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        if matches!(marker.kind, PdfMarkerTagKind::Label) {
                            let txt = Self::nodes_to_plain_text(children)
                                .replace('\u{2060}', "")
                                .trim()
                                .to_string();
                            if !txt.is_empty() {
                                return Some(txt);
                            }
                        }
                    }
                    if elem.is::<ParElem>() {
                        let txt = Self::nodes_to_plain_text(children)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        if !txt.is_empty() {
                            return Some(txt);
                        }
                    }
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                    if pos.x.to_pt() > 220.0 {
                        let txt = Self::nodes_to_plain_text(children)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        if Self::looks_like_equation_number(&txt) {
                            return Some(txt);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut right_text = String::new();
        Self::collect_right_edge_equation_text(nodes, 0.0, &mut right_text);
        let trimmed = right_text.replace('\u{2060}', "").trim().to_string();
        if Self::looks_like_equation_number(&trimmed) {
            return Some(trimmed);
        }
        None
    }

    pub(crate) fn collect_right_edge_equation_text(
        nodes: &[TreeNode],
        offset_x_pt: f64,
        out: &mut String,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if offset_x_pt + pos.x.to_pt() > 260.0 {
                        out.push_str(&text.text);
                    }
                }
                TreeNode::Group { pos, children, .. } => {
                    Self::collect_right_edge_equation_text(
                        children,
                        offset_x_pt + pos.x.to_pt(),
                        out,
                    );
                }
                TreeNode::Element { pos, children, .. } => {
                    Self::collect_right_edge_equation_text(
                        children,
                        offset_x_pt + pos.x.to_pt(),
                        out,
                    );
                }
                _ => {}
            }
        }
    }

    pub(crate) fn looks_like_equation_number(s: &str) -> bool {
        if s.len() < 2 || s.len() > 16 {
            return false;
        }
        let has_parens =
            (s.starts_with('(') && s.ends_with(')')) || (s.starts_with('[') && s.ends_with(']'));
        has_parens && s.chars().any(|c| c.is_ascii_alphanumeric())
    }
}
