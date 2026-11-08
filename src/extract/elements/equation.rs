//! `equation` -> `EquationIr` (OMML via `crate::math`).

use typst::foundations::{Packed, StyleChain};
use typst::introspection::Location;
use typst::math::EquationElem;
use typst::model::ParElem;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::EquationIr;
use crate::math::convert_equation_to_omml;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_equation(
        &self,
        eq: &Packed<EquationElem>,
        loc: Location,
        children: &[TreeNode],
    ) -> EquationIr {
        let is_block = eq.block.get(StyleChain::default());
        let (omml_xml, _) = convert_equation_to_omml(eq, StyleChain::default());
        let source_text = self
            .compiled
            .source_snippet(self.world, eq.span())
            .unwrap_or_else(|| eq.body.plain_text().to_string());
        let bookmark = self.bookmarks.get(&loc).cloned();

        // Check if this block equation has a resolved equation number (`ParElem` child inside `EquationElem`).
        let numbering_text = if is_block && eq.numbering.get_ref(StyleChain::default()).is_some() {
            Self::find_equation_number_text(children)
        } else {
            None
        };

        EquationIr {
            is_block,
            omml_xml,
            source_text,
            numbering_text,
            bookmark,
        }
    }

    pub(crate) fn find_equation_number_text(nodes: &[TreeNode]) -> Option<String> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if elem.is::<ParElem>() {
                        let txt = Self::nodes_to_plain_text(children).trim().to_string();
                        if !txt.is_empty() {
                            return Some(txt);
                        }
                    }
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(found) = Self::find_equation_number_text(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
