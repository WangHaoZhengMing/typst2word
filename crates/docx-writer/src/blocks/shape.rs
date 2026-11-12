//! Native shapes -> `ShapeBlockConstruct` -> paragraph wrapping a DrawingML `<wps:wsp>` run.

use std::fmt::Write as _;

use word_construct::shape::ShapeBlockConstruct;
use crate::writer::DocxWriter;
use crate::drawingml::shape_to_drawingml_run;
use word_construct::model::ShapeIr;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_shape_block(&mut self, shape: &ShapeIr, out: &mut String) {
        self.render_shape_block_with_collapse(shape, 0, false, false, out);
    }

    pub(crate) fn render_shape_block_collapsed(
        &mut self,
        shape: &ShapeIr,
        collapse_before: bool,
        out: &mut String,
    ) {
        self.render_shape_block_with_collapse(shape, 0, false, collapse_before, out);
    }

    pub(crate) fn render_shape_block_indented(
        &mut self,
        shape: &ShapeIr,
        left_indent_dxa: usize,
        out: &mut String,
    ) {
        self.render_shape_block_with_collapse(shape, left_indent_dxa, false, false, out);
    }

    pub(crate) fn render_shape_block_with_layout(
        &mut self,
        shape: &ShapeIr,
        left_indent_dxa: usize,
        keep_with_next: bool,
        out: &mut String,
    ) {
        self.render_shape_block_with_collapse(shape, left_indent_dxa, keep_with_next, false, out);
    }

    pub(crate) fn render_shape_block_with_collapse(
        &mut self,
        shape: &ShapeIr,
        left_indent_dxa: usize,
        keep_with_next: bool,
        collapse_before: bool,
        out: &mut String,
    ) {
        let construct =
            ShapeBlockConstruct::solve_with_collapse(shape, left_indent_dxa, collapse_before);
        let run_xml = shape_to_drawingml_run(self, shape);
        out.push_str("<w:p><w:pPr>");
        if keep_with_next {
            out.push_str("<w:keepNext/>");
        }
        Self::render_jc(construct.align, out);
        if construct.left_indent_dxa > 0 {
            let _ = write!(out, r#"<w:ind w:left="{}"/>"#, construct.left_indent_dxa);
        }
        let _ = write!(
            out,
            r#"<w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/></w:pPr>"#,
            construct.before_dxa, construct.after_dxa, construct.line_dxa
        );
        if let Some((_, dy_pt)) = shape.placed_offset_pt {
            let spacer = ShapeIr {
                kind: word_construct::model::ShapeKindIr::Rect,
                width_pt: 1.0,
                height_pt: (shape.height_pt + dy_pt.max(0.0)).max(1.0),
                fill: None,
                fill_rule_even_odd: false,
                stroke: None,
                borders: Default::default(),
                corner_radius_pt: None,
                inset: Default::default(),
                rotation_deg: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                body: Vec::new(),
                align: word_construct::model::HAlignIr::Left,
                placed_offset_pt: None,
            };
            out.push_str(&shape_to_drawingml_run(self, &spacer));
        }
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
