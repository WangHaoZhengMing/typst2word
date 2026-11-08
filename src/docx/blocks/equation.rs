//! Block `equation` -> centered `<m:oMathPara>` (or 3-column borderless layout row when numbered).

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::EquationIr;
use crate::units::pt_to_dxa;
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    pub(crate) fn render_equation_block(&mut self, eq: &EquationIr, out: &mut String) {
        if let Some(num) = &eq.numbering_text {
            // In Word, `<m:oMathPara>` (display math mode) takes over its entire paragraph
            // and ignores sibling `<w:r>` runs or right tab stops. To preserve true Display Math
            // (`<m:oMathPara>`) horizontally centered at 50% of the page while displaying the
            // equation number `(1)` flush-right and vertically centered on the same line, emit
            // Word's standard 3-column borderless equation layout row (`[12% | 76% | 12%]`).
            let content_w_pt = (self.ir.page_setup.width_pt
                - self.ir.page_setup.margin_left_pt
                - self.ir.page_setup.margin_right_pt)
                .max(360.0);
            let total_dxa = pt_to_dxa(content_w_pt);
            let side_dxa = (total_dxa * 12 / 100).max(720);
            let center_dxa = (total_dxa - 2 * side_dxa).max(2880);

            let _ = write!(
                out,
                concat!(
                    r#"<w:tbl><w:tblPr>"#,
                    r#"<w:tblW w:w="{total_dxa}" w:type="dxa"/>"#,
                    r#"<w:tblBorders>"#,
                    r#"<w:top w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                    r#"<w:left w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                    r#"<w:bottom w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                    r#"<w:right w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                    r#"<w:insideH w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                    r#"<w:insideV w:val="none" w:sz="0" w:space="0" w:color="auto"/>"#,
                    r#"</w:tblBorders>"#,
                    r#"<w:tblCellMar>"#,
                    r#"<w:top w:w="0" w:type="dxa"/><w:left w:w="0" w:type="dxa"/>"#,
                    r#"<w:bottom w:w="0" w:type="dxa"/><w:right w:w="0" w:type="dxa"/>"#,
                    r#"</w:tblCellMar>"#,
                    r#"</w:tblPr>"#,
                    r#"<w:tblGrid><w:gridCol w:w="{side_dxa}"/><w:gridCol w:w="{center_dxa}"/><w:gridCol w:w="{side_dxa}"/></w:tblGrid>"#,
                    r#"<w:tr><w:trPr><w:cantSplit/></w:trPr>"#,
                    r#"<w:tc><w:tcPr><w:tcW w:w="{side_dxa}" w:type="dxa"/><w:vAlign w:val="center"/></w:tcPr>"#,
                    r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:p></w:tc>"#,
                    r#"<w:tc><w:tcPr><w:tcW w:w="{center_dxa}" w:type="dxa"/><w:vAlign w:val="center"/></w:tcPr>"#,
                    r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="50" w:after="50" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                ),
                total_dxa = total_dxa,
                side_dxa = side_dxa,
                center_dxa = center_dxa,
            );
            let bm_id = eq
                .bookmark
                .as_ref()
                .map(|bm| self.emit_bookmark_start(bm, out));
            out.push_str("<m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr>");
            out.push_str(&eq.omml_xml);
            out.push_str("</m:oMathPara>");
            if let Some(id) = bm_id {
                Self::emit_bookmark_end(id, out);
            }
            let _ = write!(
                out,
                concat!(
                    r#"</w:p></w:tc>"#,
                    r#"<w:tc><w:tcPr><w:tcW w:w="{side_dxa}" w:type="dxa"/><w:vAlign w:val="center"/></w:tcPr>"#,
                    r#"<w:p><w:pPr><w:jc w:val="right"/><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>"#,
                    r#"<w:r><w:rPr><w:rFonts w:ascii="Libertinus Serif" w:hAnsi="Libertinus Serif" w:eastAsia="Heiti SC" w:cs="Libertinus Serif"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr>"#,
                    r#"<w:t xml:space="preserve">{}</w:t></w:r></w:p></w:tc>"#,
                    r#"</w:tr></w:tbl>"#,
                ),
                escape_xml(num),
                side_dxa = side_dxa,
            );
            return;
        }

        out.push_str(
            r#"<w:p><w:pPr><w:jc w:val="center"/><w:spacing w:before="80" w:after="80"/></w:pPr>"#,
        );
        let bm_id = eq
            .bookmark
            .as_ref()
            .map(|bm| self.emit_bookmark_start(bm, out));
        out.push_str("<m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr>");
        out.push_str(&eq.omml_xml);
        out.push_str("</m:oMathPara>");
        if let Some(id) = bm_id {
            Self::emit_bookmark_end(id, out);
        }
        out.push_str("</w:p>");
    }
}
