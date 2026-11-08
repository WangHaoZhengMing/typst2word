//! CeTZ canvas -> paragraph wrapping a DrawingML `<wpg:wgp>` group.

use crate::docx::builder::DocxBuilder;
use crate::ir::CetzCanvasIr;
use crate::shape_dml::cetz_canvas_to_drawingml_run;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_cetz_block(&mut self, canvas: &CetzCanvasIr, out: &mut String) {
        let run_xml = cetz_canvas_to_drawingml_run(canvas);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(canvas.align, out);
        out.push_str("<w:spacing w:before=\"120\" w:after=\"120\"/></w:pPr>");
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
