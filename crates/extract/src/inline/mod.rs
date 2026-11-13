//! Inline-level extraction: `TreeNode`s inside paragraphs, headings, captions, ...
//! -> `InlineNode`s, tracking active inline styles (`strong`, `emph`, `underline`, ...).
//!
//! Submodules (mirroring `crate::docx::inlines`):
//! - `style`: inline formatting wrappers (`strong`, `emph`, `underline`, `strike`, `highlight`, ...)
//! - `box_container`: inline `#box(...)` border, shading, and blank underline strips
//! - `link`: hyperlinks (`link`), cross-references (`ref`), and citations (`cite`)
//! - `footnote`: inline footnote references (`footnote`)
//! - `text`: `TextItem` -> `TextRunIr` conversion and run coalescing

mod box_container;
mod footnote;
mod link;
mod style;
mod text;

use typst::foundations::Content;
use typst::introspection::Location;
use typst::layout::{HideElem, LayoutElem, MoveElem, Point, RotateElem, ScaleElem};
use typst::math::EquationElem;
use typst::model::FootnoteElem;
use typst::syntax::Span;
use typst::text::LinebreakElem;
use typst::visualize::{ImageElem, Paint};

use compiler::compile::is_cetz_span;
use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{ColorHex, HAlignIr, InlineNode, ScriptKindIr, StrokeIr, TextRunIr};

/// Active inline formatting context accumulated while walking inside a paragraph.
#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveInlineState {
    pub(crate) scope_loc: Option<Location>,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: Option<StrokeIr>,
    pub(crate) strike: Option<StrokeIr>,
    pub(crate) overline: Option<StrokeIr>,
    pub(crate) highlight: Option<ColorHex>,
    pub(crate) border: Option<StrokeIr>,
    pub(crate) script: ScriptKindIr,
    pub(crate) smallcaps: bool,
    pub(crate) is_code: bool,
    pub(crate) hidden: bool,
}

impl<'a> ExtractorContext<'a> {
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
            scope_loc: self.active_scope_loc,
            bold: force_bold,
            ..Default::default()
        };
        let mut last_y: Option<f64> = None;
        let mut last_x_end: Option<f64> = None;
        self.extract_inlines_from_nodes(nodes, &mut out, &mut state, &mut last_y, &mut last_x_end);
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
                TreeNode::Text {
                    text,
                    pos,
                    tracking_pt,
                    baseline_shift_pt,
                } => {
                    if state.hidden {
                        continue;
                    }
                    let y_pt = pos.y.to_pt();
                    let line_y_pt = y_pt + baseline_shift_pt.unwrap_or(0.0);
                    let x_pt = pos.x.to_pt();

                    // Detect inter-word or inter-line spacing from laid-out glyph coordinates.
                    if let Some(prev_y) = *last_y {
                        if (line_y_pt - prev_y).abs() > text.size.to_pt() * 0.55
                            && state.script == ScriptKindIr::Normal
                        {
                            if Self::strip_trailing_soft_hyphen_on_line_wrap(out) {
                                // Word wrapped with a Typst soft hyphen (`\u{ad}`); re-join without space.
                            } else if Self::should_insert_line_join_space(out, &text.text) {
                                out.push(InlineNode::Text(TextRunIr {
                                    text: " ".to_string(),
                                    ..Self::text_item_to_run_with_metrics(
                                        text,
                                        state,
                                        *tracking_pt,
                                        *baseline_shift_pt,
                                    )
                                }));
                            }
                        } else if let Some(prev_x_end) = *last_x_end {
                            let gap_pt = x_pt - prev_x_end;
                            if gap_pt > text.size.to_pt() * 0.75 {
                                out.push(InlineNode::HorizontalSpace { width_pt: gap_pt });
                            }
                        }
                    }

                    if state.script == ScriptKindIr::Normal {
                        *last_y = Some(line_y_pt);
                    }
                    *last_x_end = Some(x_pt + text.width().to_pt());

                    if let Paint::Tiling(_) = &text.fill {
                        let unsup = self.record_unsupported_paint("text.fill", Span::detached());
                        out.push(InlineNode::UnsupportedInline(unsup));
                    }

                    out.push(InlineNode::Text(Self::text_item_to_run_with_metrics(
                        text,
                        state,
                        *tracking_pt,
                        *baseline_shift_pt,
                    )));
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
                    if state.underline.is_some()
                        || state.strike.is_some()
                        || state.overline.is_some()
                        || state.highlight.is_some()
                    {
                        continue;
                    }
                    if let Some(shape_ir) =
                        self.extract_raw_shape_item(shape, *span, *pos, Vec::new(), HAlignIr::Left)
                    {
                        self.report.shapes_count += 1;
                        out.push(InlineNode::ShapeInline(shape_ir));
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if Self::is_cetz_group_nodes(children)
                        && let Some(canvas) = self.extract_cetz_canvas(
                            children,
                            Some(group.frame.size()),
                            HAlignIr::Left,
                        )
                    {
                        self.report.cetz_canvases_count += 1;
                        out.push(InlineNode::CetzInline(canvas));
                        continue;
                    }

                    if let (Some(prev_y), Some(prev_x_end)) = (*last_y, *last_x_end)
                        && (pos.y.to_pt() - prev_y).abs() <= 12.0
                    {
                        let gap_pt = pos.x.to_pt() - prev_x_end;
                        if gap_pt > 2.0 {
                            out.push(InlineNode::HorizontalSpace { width_pt: gap_pt });
                        }
                    }

                    if self.try_extract_inline_box_group(
                        group, *pos, children, out, state, last_y, last_x_end,
                    ) {
                        continue;
                    }

                    let mut inner_y = None;
                    let mut inner_x = None;
                    self.extract_inlines_from_nodes(
                        children,
                        out,
                        state,
                        &mut inner_y,
                        &mut inner_x,
                    );
                    *last_x_end = Some(pos.x.to_pt() + group.frame.size().x.to_pt());
                }
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    let prev_scope = state.scope_loc;
                    if self.compiled.semantics.get(*loc).is_some() {
                        state.scope_loc = Some(*loc);
                    }
                    self.extract_inline_element(
                        elem, *loc, *pos, children, out, state, last_y, last_x_end,
                    );
                    state.scope_loc = prev_scope;
                }
                TreeNode::Link { .. } => {}
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
        if Self::is_page_furniture_elem(elem) || elem.is::<HideElem>() {
            return;
        }

        if let Some(unsup) = self.check_unsupported_element(elem, loc) {
            out.push(InlineNode::UnsupportedInline(unsup));
            return;
        }

        if elem.is::<typst::text::LinebreakElem>() {
            out.push(InlineNode::LineBreak);
            *last_y = None;
            *last_x_end = None;
            return;
        }

        if let Some(h) = elem.to_packed::<typst::layout::HElem>() {
            let chain = self.scoped_style_chain(Some(loc));
            if !h.weak.get(chain)
                && let typst::layout::Spacing::Rel(rel) = h.amount
            {
                let base_em =
                    Self::resolve_length_pt(chain.get(typst::text::TextElem::size).0, 11.0);
                let col_w = self.current_column_width_pt();
                let w_pt = Self::resolve_length_pt(rel.abs, base_em) + rel.rel.get() * col_w;
                if w_pt > 0.5 {
                    out.push(InlineNode::HorizontalSpace { width_pt: w_pt });
                    if let Some(prev_x) = last_x_end.as_mut() {
                        *prev_x += w_pt;
                    }
                }
            }
            return;
        }

        if let Some(counter) = elem.to_packed::<typst::introspection::CounterDisplayElem>() {
            // Typst keeps counter-display's fields private and excludes them from
            // reflection. Compare the raw native element hash (no span/location)
            // with the page-numbering element constructed by Typst's page layouter.
            use typst::introspection::{Counter, CounterKey, CounterDisplayElem};
            if let Some(typst::model::Numbering::Pattern(pattern)) = self.compiled.document.pages().get(self.current_page_index).and_then(|p| p.numbering.clone()) {
                let both = pattern.pieces() >= 2;
                let expected = CounterDisplayElem::new(Counter::new(CounterKey::Page), typst::foundations::Smart::Custom(typst::model::Numbering::Pattern(pattern.clone())), both);
                if typst_utils::hash128(&**counter) == typst_utils::hash128(&expected) {
                    let cached = self.extract_inlines_flat(children);
                    let template = cached.iter().find_map(|i| if let InlineNode::Text(r) = i { Some(r.clone()) } else { None }).unwrap_or_default();
                    let count = if both { 2 } else { 1 };
                    for index in 0..count {
                        let Some((prefix, system)) = pattern.pieces.get(index).or_else(|| pattern.pieces.last()) else { break };
                        let mut prefix = prefix.to_string();
                        if index >= pattern.pieces.len() && prefix.is_empty() { prefix = pattern.suffix.to_string(); }
                        if !prefix.is_empty() { let mut r = template.clone(); r.text = prefix; out.push(InlineNode::Text(r)); }
                        let mut single = pattern.clone();
                        single.pieces = [(Default::default(), *system)].into_iter().collect();
                        single.suffix = Default::default();
                        let sample = single.apply(None, &[1]).unwrap_or_default();
                        let format = match sample.as_str() { "1" => Some("Arabic"), "i" => Some("roman"), "I" => Some("ROMAN"), "a" => Some("alphabetic"), "A" => Some("ALPHABETIC"), _ => None };
                        if format.is_none() {
                            self.report.compiler_warnings.push("Page numbering format is not supported by Word fields; using Arabic numerals".into());
                        }
                        let value = if index == 0 { 1 } else { self.compiled.document.pages().len() as u64 };
                        let mut run = template.clone();
                        run.text = single.apply(None, &[value]).unwrap_or_else(|_| value.to_string().into()).to_string();
                        out.push(InlineNode::Field { kind: if index == 0 { ir::FieldKindIr::Page } else { ir::FieldKindIr::NumPages }, format: Some(format.unwrap_or("Arabic").into()), content: vec![InlineNode::Text(run)] });
                    }
                    if !pattern.suffix.is_empty() { let mut r = template; r.text = pattern.suffix.to_string(); out.push(InlineNode::Text(r)); }
                    *last_y = None;
                    *last_x_end = None;
                    return;
                }
            }
            let auto_page = CounterDisplayElem::new(
                Counter::new(CounterKey::Page),
                typst::foundations::Smart::Auto,
                false,
            );
            if typst_utils::hash128(&**counter) == typst_utils::hash128(&auto_page) {
                let cached = self.extract_inlines_flat(children);
                let mut run = cached
                    .iter()
                    .find_map(|i| {
                        if let InlineNode::Text(r) = i {
                            Some(r.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                run.text = "1".to_string();
                out.push(InlineNode::Field {
                    kind: ir::FieldKindIr::Page,
                    format: Some("Arabic".into()),
                    content: vec![InlineNode::Text(run)],
                });
                *last_y = None;
                *last_x_end = None;
                return;
            }
        }

        // 1. Inline formatting & code tags (`style.rs`).
        if self.try_extract_inline_style_element(elem, children, out, state, last_y, last_x_end) {
            return;
        }

        // 2. Inline Equation (`EquationElem`).
        if let Some(eq) = elem.to_packed::<EquationElem>() {
            let eq_ir = self.extract_equation(eq, loc, children);
            self.report.equations_count += 1;
            out.push(InlineNode::EquationInline(eq_ir));
            *last_x_end = None;
            return;
        }

        // 3. Footnote Reference (`footnote.rs`).
        if elem.is::<FootnoteElem>() {
            self.extract_inline_footnote(loc, children, out);
            *last_x_end = None;
            return;
        }

        // 4. Links, Cross-references, Citations (`link.rs`).
        if self.try_extract_inline_link_or_ref(elem, children, out, state, last_y, last_x_end) {
            return;
        }

        // 5. Inline Shapes (`RectElem`, `CircleElem`, etc. inside a paragraph or `#box(...)`).
        if (elem.is::<RotateElem>() || elem.is::<ScaleElem>() || elem.is::<MoveElem>())
            && let Some(shape_ir) =
                self.try_extract_transformed_shape(elem, pos, children, 0, HAlignIr::Left)
        {
            self.report.shapes_count += 1;
            out.push(InlineNode::ShapeInline(shape_ir));
            *last_x_end = None;
            return;
        }
        if Self::is_native_shape_elem(elem)
            && let Some(shape_ir) = self.extract_native_shape_elem(
                elem,
                pos,
                children,
                0,
                HAlignIr::Left,
                0.0,
                (1.0, 1.0),
            )
        {
            if let (Some(prev_y), Some(prev_x_end)) = (*last_y, *last_x_end)
                && (pos.y.to_pt() - prev_y).abs() <= 10.0
            {
                let gap_pt = pos.x.to_pt() - prev_x_end;
                if gap_pt > 1.5 {
                    out.push(InlineNode::HorizontalSpace { width_pt: gap_pt });
                }
            }
            let w_pt = shape_ir.width_pt;
            self.report.shapes_count += 1;
            out.push(InlineNode::ShapeInline(shape_ir));
            *last_y = Some(pos.y.to_pt());
            *last_x_end = Some(pos.x.to_pt() + w_pt);
            return;
        }

        // 6. Inline CeTZ canvas.
        if (elem.is::<LayoutElem>() || elem.elem().name() == "context")
            && (is_cetz_span(elem.span()) || Self::contains_cetz_shapes_recursive(children))
            && let Some(canvas) = self.extract_cetz_canvas(children, None, HAlignIr::Left)
        {
            self.report.cetz_canvases_count += 1;
            out.push(InlineNode::CetzInline(canvas));
            *last_x_end = None;
            return;
        }

        // 7. Inline Image (`ImageElem`).
        if let Some(img_elem) = elem.to_packed::<ImageElem>()
            && let Some(img_ir) = self.extract_image_elem(img_elem, children, HAlignIr::Left)
        {
            self.report.images_count += 1;
            out.push(InlineNode::ImageInline(img_ir));
            *last_x_end = None;
            return;
        }

        // 8. Explicit Linebreak (`LinebreakElem`).
        if elem.is::<LinebreakElem>() {
            out.push(InlineNode::LineBreak);
            *last_y = None;
            *last_x_end = None;
            return;
        }

        // 9. Recurse into children for any other container/tag.
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
