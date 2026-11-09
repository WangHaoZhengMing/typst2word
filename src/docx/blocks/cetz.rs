//! CeTZ canvas -> `CetzCanvasConstruct` -> paragraph wrapping a DrawingML `<wpg:wgp>` group.

use std::fmt::Write as _;

use crate::construct::cetz::CetzCanvasConstruct;
use crate::docx::builder::DocxBuilder;
use crate::docx::drawingml::cetz_canvas_to_drawingml_run;
use crate::ir::CetzCanvasIr;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_cetz_block(&mut self, canvas: &CetzCanvasIr, out: &mut String) {
        let construct = CetzCanvasConstruct::solve(canvas);
        let run_xml = cetz_canvas_to_drawingml_run(canvas);
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(construct.align, out);
        let _ = write!(
            out,
            r#"<w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/><w:rPr><w:sz w:val="{}"/><w:szCs w:val="{}"/></w:rPr></w:pPr>"#,
            construct.before_dxa,
            construct.after_dxa,
            construct.line_dxa,
            construct.mark_sz_half_pt,
            construct.mark_sz_half_pt
        );
        out.push_str(&run_xml);
        out.push_str("</w:p>");
    }
}
