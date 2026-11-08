//! Native shapes -> paragraph wrapping a DrawingML `<wps:wsp>` run.

use std::fmt::Write as _;

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
        let run_xml = shape_to_drawingml_run(shape);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(shape.align, out);
        if left_indent_dxa > 0 {
            let _ = write!(out, r#"<w:ind w:left="{left_indent_dxa}"/>"#);
        }
        out.push_str(
            r#"<w:spacing w:before="20" w:after="80" w:line="240" w:lineRule="auto"/></w:pPr>"#,
        );
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
