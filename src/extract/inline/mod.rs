//! Inline-level extraction: `TreeNode`s inside paragraphs, headings, captions, ...
//! -> `InlineNode`s, tracking active inline styles (`strong`, `emph`, `underline`, ...).

mod link;
mod text;

use typst::foundations::{Content, StyleChain};
use typst::introspection::Location;
use typst::layout::{HideElem, LayoutElem, MoveElem, Point, RotateElem, ScaleElem};
use typst::math::EquationElem;
use typst::model::{
    CiteGroup, Destination, DirectLinkElem, EmphElem, FootnoteElem, LinkElem, LinkMarker, RefElem,
    StrongElem,
};
use typst::syntax::Span;
use typst::text::{
    HighlightElem, LinebreakElem, OverlineElem, RawElem, SmallcapsElem, StrikeElem, SubElem,
    SuperElem, UnderlineElem,
};
use typst::visualize::Paint;

use crate::compiler::compile::is_cetz_span;
use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{ColorHex, HAlignIr, InlineNode, ScriptKindIr, StrokeIr, TextRunIr};
use crate::math_omml::paint_to_hex;

/// Active inline formatting context accumulated while walking inside a paragraph.
#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveInlineState {
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: Option<StrokeIr>,
    pub(crate) strike: Option<StrokeIr>,
    pub(crate) overline: Option<StrokeIr>,
    pub(crate) highlight: Option<ColorHex>,
    pub(crate) script: ScriptKindIr,
    pub(crate) smallcaps: bool,
    pub(crate) is_code: bool,
    pub(crate) hidden: bool,
}

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Inline-level Extraction
    // =========================================================================
    pub(crate) fn extract_inlines_flat(&mut self, nodes: &[TreeNode]) -> Vec<InlineNode> {
        self.extract_inlines_flat_with_bold(nodes, false)
    }

    pub(crate) fn extract_inlines_flat_with_bold(
        &mut self,
        nodes: &[TreeNode],
        force_bold: bool,
    ) -> Vec<InlineNode> {
        let mut out = Vec::new();
        let mut state = ActiveInlineState {
            bold: force_bold,
            ..Default::default()
        };
        let mut last_y: Option<f64> = None;
        let mut last_x_end: Option<f64> = None;
        self.extract_inlines_from_nodes(nodes, &mut out, &mut state, &mut last_y, &mut last_x_end);
        Self::merge_adjacent_text_runs(&mut out);
        out
    }

    pub(crate) fn extract_inlines_from_nodes(
        &mut self,
        nodes: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Text { text, pos } => {
                    if state.hidden {
                        continue;
                    }
                    let y_pt = pos.y.to_pt();
                    let x_pt = pos.x.to_pt();

                    // Detect inter-word or inter-line spacing from laid-out glyph coordinates!
                    if let Some(prev_y) = *last_y {
                        if (y_pt - prev_y).abs() > text.size.to_pt() * 0.55
                            && state.script == ScriptKindIr::Normal
                        {
                            // New visual line inside the paragraph: insert a space if needed.
                            if Self::should_insert_line_join_space(out, &text.text) {
                                out.push(InlineNode::Text(TextRunIr {
                                    text: " ".to_string(),
                                    ..Self::text_item_to_run(text, state)
                                }));
                            }
                        } else if let Some(prev_x_end) = *last_x_end {
                            let gap_pt = x_pt - prev_x_end;
                            if gap_pt > text.size.to_pt() * 1.2 {
                                out.push(InlineNode::HorizontalSpace { width_pt: gap_pt });
                            } else if gap_pt > text.size.to_pt() * 0.18
                                && state.script == ScriptKindIr::Normal
                            {
                                let already_spaced = matches!(
                                    out.last(),
                                    Some(InlineNode::HorizontalSpace { .. })
                                        | Some(InlineNode::Tab)
                                ) || matches!(
                                    out.last(),
                                    Some(InlineNode::Text(r)) if r.text.ends_with(' ')
                                );
                                if !already_spaced && !text.text.starts_with(' ') {
                                    out.push(InlineNode::Text(TextRunIr {
                                        text: " ".to_string(),
                                        ..Self::text_item_to_run(text, state)
                                    }));
                                }
                            }
                        }
                    }

                    if state.script == ScriptKindIr::Normal {
                        *last_y = Some(y_pt);
                    }
                    *last_x_end = Some(x_pt + text.width().to_pt());

                    if let Paint::Tiling(_) = &text.fill {
                        let unsup = self.record_unsupported_paint("text.fill", Span::detached());
                        out.push(InlineNode::UnsupportedInline(unsup));
                    }

                    out.push(InlineNode::Text(Self::text_item_to_run(text, state)));
                }
                TreeNode::Image {
                    image, size, span, ..
                } => {
                    if let Some(img_ir) =
                        self.extract_image_item(image, *size, *span, HAlignIr::Left)
                    {
                        self.report.images_count += 1;
                        out.push(InlineNode::ImageInline(img_ir));
                    }
                }
                TreeNode::Shape { shape, span, pos } => {
                    if let Some(shape_ir) =
                        self.extract_raw_shape_item(shape, *span, *pos, Vec::new(), HAlignIr::Left)
                    {
                        self.report.shapes_count += 1;
                        out.push(InlineNode::ShapeInline(shape_ir));
                    }
                }
                TreeNode::Group {
                    group,
                    pos: _,
                    children,
                } => {
                    if Self::is_cetz_group_nodes(children) {
                        if let Some(canvas) = self.extract_cetz_canvas(
                            children,
                            Some(group.frame.size()),
                            HAlignIr::Left,
                        ) {
                            self.report.cetz_canvases_count += 1;
                            out.push(InlineNode::CetzInline(canvas));
                            continue;
                        }
                    }
                    self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
                }
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    self.extract_inline_element(
                        elem, *loc, *pos, children, out, state, last_y, last_x_end,
                    );
                }
                TreeNode::Link { .. } | TreeNode::PageBreak => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn extract_inline_element(
        &mut self,
        elem: &Content,
        loc: Location,
        pos: Point,
        children: &[TreeNode],
        out: &mut Vec<InlineNode>,
        state: &mut ActiveInlineState,
        last_y: &mut Option<f64>,
        last_x_end: &mut Option<f64>,
    ) {
        // 1. Check for unsupported inline elements.
        if let Some(unsup) = self.check_unsupported_element(elem, loc) {
            out.push(InlineNode::UnsupportedInline(unsup));
            return;
        }

        // 2. Inline formatting tags (`StrongElem`, `EmphElem`, `UnderlineElem`, `StrikeElem`,
        //    `OverlineElem`, `HighlightElem`, `SubElem`, `SuperElem`, `SmallcapsElem`, `HideElem`).
        if elem.is::<StrongElem>() {
            let prev = state.bold;
            state.bold = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.bold = prev;
            return;
        }
        if elem.is::<EmphElem>() {
            let prev = state.italic;
            state.italic = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.italic = prev;
            return;
        }
        if let Some(ul) = elem.to_packed::<UnderlineElem>() {
            let prev = state.underline.clone();
            let stroke = ul.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.underline = Some(Self::abs_stroke_to_ir(&stroke));
            // Filter out the decorative line shape inside `children` so it isn't emitted as a shape!
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.underline = prev;
            return;
        }
        if let Some(st) = elem.to_packed::<StrikeElem>() {
            let prev = state.strike.clone();
            let stroke = st.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.strike = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.strike = prev;
            return;
        }
        if let Some(ol) = elem.to_packed::<OverlineElem>() {
            let prev = state.overline.clone();
            let stroke = ol.stroke.resolve(StyleChain::default()).unwrap_or_default();
            state.overline = Some(Self::abs_stroke_to_ir(&stroke));
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.overline = prev;
            return;
        }
        if let Some(hl) = elem.to_packed::<HighlightElem>() {
            let prev = state.highlight.clone();
            state.highlight = hl
                .fill
                .get_ref(StyleChain::default())
                .as_ref()
                .and_then(paint_to_hex)
                .or_else(|| Some("FFF59D".to_string()));
            let text_only: Vec<TreeNode> = children
                .iter()
                .filter(|c| !matches!(c, TreeNode::Shape { .. }))
                .cloned()
                .collect();
            self.extract_inlines_from_nodes(&text_only, out, state, last_y, last_x_end);
            state.highlight = prev;
            return;
        }
        if elem.is::<SubElem>() {
            let prev = state.script;
            state.script = ScriptKindIr::Sub;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.script = prev;
            return;
        }
        if elem.is::<SuperElem>() {
            let prev = state.script;
            state.script = ScriptKindIr::Super;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.script = prev;
            return;
        }
        if elem.is::<SmallcapsElem>() {
            let prev = state.smallcaps;
            state.smallcaps = true;
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            state.smallcaps = prev;
            return;
        }
        if elem.is::<HideElem>() {
            return;
        }

        // 3. Inline Code (`RawElem` with `block: false`).
        if elem.is::<RawElem>() {
            let prev = state.is_code;
            state.is_code = true;
            let before_len = out.len();
            self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
            if out.len() == before_len {
                out.push(InlineNode::Text(TextRunIr {
                    text: elem.plain_text().to_string(),
                    font_family: Some("Consolas".to_string()),
                    is_code: true,
                    ..Default::default()
                }));
            }
            state.is_code = prev;
            return;
        }

        // 4. Inline Equation (`EquationElem`).
        if let Some(eq) = elem.to_packed::<EquationElem>() {
            let eq_ir = self.extract_equation(eq, loc, children);
            self.report.equations_count += 1;
            out.push(InlineNode::EquationInline(eq_ir));
            *last_x_end = None;
            return;
        }

        // 5. Footnote Reference (`FootnoteElem`).
        if elem.is::<FootnoteElem>() {
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
            return;
        }

        // 6. Cross-reference (`RefElem`).
        if let Some(rf) = elem.to_packed::<RefElem>() {
            let target_str = rf.target.resolve().as_str().to_string();
            let anchor = self
                .label_bookmarks
                .get(&target_str)
                .cloned()
                .unwrap_or_else(|| target_str.clone());
            let mut ref_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut ref_inlines, state, last_y, last_x_end);
            // Unwrap inner Link if `RefElem` realized into a `DirectLinkElem`/`LinkMarker`.
            let clean_inlines = Self::unwrap_link_inlines(ref_inlines);
            out.push(InlineNode::Ref {
                target: anchor,
                content: clean_inlines,
            });
            return;
        }

        // 7. Citation (`CiteGroup`).
        if let Some(cg) = elem.to_packed::<CiteGroup>() {
            let keys: Vec<String> = cg
                .children
                .iter()
                .map(|c| c.key.resolve().as_str().to_string())
                .collect();
            let mut cite_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut cite_inlines, state, last_y, last_x_end);
            let clean_inlines = Self::unwrap_link_inlines(cite_inlines);
            out.push(InlineNode::Citation {
                keys,
                content: clean_inlines,
            });
            return;
        }

        // 8. Hyperlink (`LinkMarker`, `LinkElem`, `DirectLinkElem`).
        if elem.is::<LinkMarker>() || elem.is::<LinkElem>() || elem.is::<DirectLinkElem>() {
            let dest = Self::find_link_destination(elem, children);
            let (url, anchor) = match dest {
                Some(Destination::Url(u)) => (Some(u.to_string()), None),
                Some(Destination::Location(target_loc)) => (
                    None,
                    self.bookmarks
                        .get(&target_loc)
                        .cloned()
                        .or_else(|| Some(format!("_typst_loc_{:x}", target_loc.hash()))),
                ),
                Some(Destination::Position(_)) => (None, None),
                None => (None, None),
            };
            let mut link_inlines = Vec::new();
            self.extract_inlines_from_nodes(children, &mut link_inlines, state, last_y, last_x_end);
            let had_inner_link = link_inlines
                .iter()
                .any(|inl| matches!(inl, InlineNode::Link { .. }));
            let clean_inlines = Self::unwrap_link_inlines(link_inlines);
            if !had_inner_link {
                self.report.links_count += 1;
            }
            out.push(InlineNode::Link {
                url,
                anchor,
                content: clean_inlines,
            });
            return;
        }

        // 9. Inline Shapes (`RectElem`, `CircleElem`, etc. inside a paragraph or `#box(...)`).
        if elem.is::<RotateElem>() || elem.is::<ScaleElem>() || elem.is::<MoveElem>() {
            if let Some(shape_ir) =
                self.try_extract_transformed_shape(elem, pos, children, 0, HAlignIr::Left)
            {
                self.report.shapes_count += 1;
                out.push(InlineNode::ShapeInline(shape_ir));
                *last_x_end = None;
                return;
            }
        }
        if Self::is_native_shape_elem(elem) {
            if let Some(shape_ir) = self.extract_native_shape_elem(
                elem,
                pos,
                children,
                0,
                HAlignIr::Left,
                0.0,
                (1.0, 1.0),
            ) {
                self.report.shapes_count += 1;
                out.push(InlineNode::ShapeInline(shape_ir));
                *last_x_end = None;
                return;
            }
        }

        // 10. Inline CeTZ canvas.
        if (elem.is::<LayoutElem>() || elem.elem().name() == "context")
            && (is_cetz_span(elem.span()) || Self::contains_cetz_shapes_recursive(children))
        {
            if let Some(canvas) = self.extract_cetz_canvas(children, None, HAlignIr::Left) {
                self.report.cetz_canvases_count += 1;
                out.push(InlineNode::CetzInline(canvas));
                *last_x_end = None;
                return;
            }
        }

        // 11. Inline Image (`ImageElem`).
        if let Some(img_elem) = elem.to_packed::<typst::visualize::ImageElem>() {
            if let Some(img_ir) = self.extract_image_elem(img_elem, children, HAlignIr::Left) {
                self.report.images_count += 1;
                out.push(InlineNode::ImageInline(img_ir));
                *last_x_end = None;
                return;
            }
        }

        // 12. Explicit Linebreak (`LinebreakElem`).
        if elem.is::<LinebreakElem>() {
            out.push(InlineNode::LineBreak);
            *last_y = None;
            *last_x_end = None;
            return;
        }

        // 13. Recurse into children for any other container/tag.
        self.extract_inlines_from_nodes(children, out, state, last_y, last_x_end);
    }

    pub(crate) fn inlines_to_plain_text(inlines: &[InlineNode]) -> String {
        let mut s = String::new();
        for inline in inlines {
            match inline {
                InlineNode::Text(r) => s.push_str(&r.text),
                InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. } => {
                    s.push_str(&Self::inlines_to_plain_text(content));
                }
                _ => {}
            }
        }
        s
    }
}
