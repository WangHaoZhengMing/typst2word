//! Native shapes -> `ShapeBlockConstruct` -> paragraph wrapping a DrawingML `<wps:wsp>` run.

use std::fmt::Write as _;

use crate::construct::shape::ShapeBlockConstruct;
use crate::docx::builder::DocxBuilder;
use crate::docx::drawingml::shape_to_drawingml_run;
use crate::ir::ShapeIr;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_shape_block(&mut self, shape: &ShapeIr, out: &mut String) {
        self.render_shape_block_indented(shape, 0, out);
    }

    pub(crate) fn render_shape_block_indented(
        &mut self,
        shape: &ShapeIr,
        left_indent_dxa: usize,
        out: &mut String,
    ) {
        let construct = ShapeBlockConstruct::solve(shape, left_indent_dxa);
        let run_xml = shape_to_drawingml_run(shape);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(construct.align, out);
        if construct.left_indent_dxa > 0 {
            let _ = write!(out, r#"<w:ind w:left="{}"/>"#, construct.left_indent_dxa);
        }
        let _ = write!(
            out,
            r#"<w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/></w:pPr>"#,
            construct.before_dxa, construct.after_dxa, construct.line_dxa
        );
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
