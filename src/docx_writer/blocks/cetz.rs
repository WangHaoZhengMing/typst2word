//! CeTZ canvas -> `CetzCanvasConstruct` -> paragraph wrapping a DrawingML `<wpg:wgp>` group.

use std::fmt::Write as _;

use crate::construct::cetz::CetzCanvasConstruct;
use crate::docx_writer::writer::DocxWriter;
use crate::docx_writer::drawingml::cetz_canvas_to_drawingml_run;
use crate::ir::CetzCanvasIr;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_cetz_block(&mut self, canvas: &CetzCanvasIr, out: &mut String) {
        self.render_cetz_block_with_layout(canvas, 0, false, out);
    }

    pub(crate) fn render_cetz_block_with_layout(
        &mut self,
        canvas: &CetzCanvasIr,
        left_indent_dxa: usize,
        keep_with_next: bool,
        out: &mut String,
    ) {
        let construct = CetzCanvasConstruct::solve(canvas);
        let run_xml = cetz_canvas_to_drawingml_run(canvas);
        out.push_str("<w:p><w:pPr>");
        if keep_with_next {
            out.push_str("<w:keepNext/>");
        }
        if left_indent_dxa > 0 {
            let _ = write!(out, r#"<w:ind w:left="{left_indent_dxa}"/>"#);
        }
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
