//! Inline style wrappers (`strong`, `emph`, `underline`, `strike`, `overline`,
//! `highlight`, `sub`, `super`, `smallcaps`, inline `raw`).

use typst::foundations::{Content, StyleChain};
use typst::model::{EmphElem, StrongElem};
use typst::text::{
    HighlightElem, OverlineElem, RawElem, SmallcapsElem, StrikeElem, SubElem, SuperElem,
    UnderlineElem,
};

use crate::color::paint_to_hex;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use crate::inline::ActiveInlineState;
use ir::{InlineNode, ScriptKindIr, TextRunIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn try_extract_inline_style_element(
        &mut self,
        elem: &Content,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) -> bool {
        if elem.is::<StrongElem>() {
            let prev = state.bold;
            state.bold = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.bold = prev;
            return true;
        }
        if elem.is::<EmphElem>() {
            let prev = state.italic;
            state.italic = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.italic = prev;
            return true;
        }
        if let Some(ul) = elem.to_packed::<UnderlineElem>() {
            let prev = state.underline.clone();
            let stroke = ul.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.underline = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only = Self::filter_non_shape_nodes(children);
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.underline = prev;
            return true;
        }
        if let Some(st) = elem.to_packed::<StrikeElem>() {
            let prev = state.strike.clone();
            let stroke = st.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.strike = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only = Self::filter_non_shape_nodes(children);
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.strike = prev;
            return true;
        }
        if let Some(ol) = elem.to_packed::<OverlineElem>() {
            let prev = state.overline.clone();
            let stroke = ol.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.overline = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only = Self::filter_non_shape_nodes(children);
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.overline = prev;
            return true;
        }
        if let Some(hl) = elem.to_packed::<HighlightElem>() {
            let prev = state.highlight.clone();
            state.highlight = hl
                .fill
                .get_ref(StyleChain::default())
                .as_ref()
                .and_then(paint_to_hex)
                .or_else(|| Some("FFF59D".to_string()));
            let text_only = Self::filter_non_shape_nodes(children);
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.highlight = prev;
            return true;
        }
        if elem.is::<SubElem>() {
            let prev = state.script;
            state.script = ScriptKindIr::Sub;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.script = prev;
            return true;
        }
        if elem.is::<SuperElem>() {
            let prev = state.script;
            state.script = ScriptKindIr::Super;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.script = prev;
            return true;
        }
        if elem.is::<SmallcapsElem>() {
            let prev = state.smallcaps;
            state.smallcaps = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.smallcaps = prev;
            return true;
        }
        if elem.is::<RawElem>() {
            let prev = state.is_code;
            state.is_code = true;
            let before_len = out.len();
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            if out.len() == before_len {
                out.push(InlineNode::Text(TextRunIr {
                    text: elem.plain_text().to_string(),
                    font_family: None,
                    is_code: true,
                    ..Default::default()
                }));
            }
            state.is_code = prev;
            return true;
        }
        false
    }

    fn filter_non_shape_nodes(nodes: &[TreeNode]) -> Vec<TreeNode> {
        nodes
            .iter()
            .filter(|c| !matches!(c, TreeNode::Shape { .. }))
            .cloned()
            .collect()
    }
}
