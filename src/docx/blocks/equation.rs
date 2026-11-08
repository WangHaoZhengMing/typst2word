//! Block `equation` -> centered `<m:oMathPara>`.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::EquationIr;
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_equation_block(&mut self, eq: &EquationIr, out: &mut String) {
        out.push_str(
            r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="120" w:after="120"/></w:pPr>"#,
        );
        let bm_id = eq
            .bookmark
            .as_ref()
            .map(|bm| self.emit_bookmark_start(bm, out));
        out.push_str("<m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr>");
        out.push_str(&eq.omml_xml);
        out.push_str("</m:oMathPara>");
        if let Some(num) = &eq.numbering_text {
            let _ = write!(
                out,
                r#"<w:r><w:t xml:space="preserve">    {}</w:t></w:r>"#,
                escape_xml(num)
            );
        }
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
