//! CeTZ canvas -> paragraph wrapping a DrawingML `<wpg:wgp>` group.

use crate::docx::builder::DocxBuilder;
use crate::docx::drawingml::cetz_canvas_to_drawingml_run;
use crate::ir::CetzCanvasIr;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_cetz_block(&mut self, canvas: &CetzCanvasIr, out: &mut String) {
        let run_xml = cetz_canvas_to_drawingml_run(canvas);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(canvas.align, out);
        out.push_str(
            r#"<w:spacing w:before="20" w:after="0" w:line="240" w:lineRule="auto"/><w:rPr><w:sz w:val="4"/><w:szCs w:val="4"/></w:rPr></w:pPr>"#,
        );
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
