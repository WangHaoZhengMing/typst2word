//! Native shapes -> paragraph wrapping a DrawingML `<wps:wsp>` run.

use crate::docx::builder::DocxBuilder;
use crate::docx::drawingml::shape_to_drawingml_run;
use crate::ir::ShapeIr;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_shape_block(&mut self, shape: &ShapeIr, out: &mut String) {
        let run_xml = shape_to_drawingml_run(shape);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(shape.align, out);
        out.push_str(
            r#"<w:spacing w:before="0" w:after="240" w:line="240" w:lineRule="auto"/></w:pPr>"#,
        );
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
