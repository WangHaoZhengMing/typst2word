//! Block `equation` -> `EquationBlockConstruct` -> centered `<m:oMathPara>` or 3-column numbered row.

use std::fmt::Write as _;

use crate::construct::equation::EquationBlockConstruct;
use crate::docx_writer::writer::DocxWriter;
use crate::docx_writer::math::render_equation_omml;
use crate::ir::EquationIr;
use crate::xml::escape_xml;

impl<'a> DocxWriter<'a> {
    pub(crate) fn render_equation_block(&mut self, eq: &EquationIr, out: &mut String) {
        self.render_equation_block_with_layout(eq, 80, 80, 0, out);
    }

    pub(crate) fn render_equation_block_with_layout(
        &mut self,
        eq: &EquationIr,
        before_dxa: i64,
        after_dxa: i64,
        indent_left_dxa: usize,
        out: &mut String,
    ) {
        let page_content_w_pt = self.ir.page_setup.width_pt
            - self.ir.page_setup.margin_left_pt
            - self.ir.page_setup.margin_right_pt;
        let construct = EquationBlockConstruct::solve(
            eq,
            page_content_w_pt,
            before_dxa,
            after_dxa,
            indent_left_dxa,
        );

        match construct {
            EquationBlockConstruct::NumberedThreeColumnRow {
                total_dxa,
                side_dxa,
                center_dxa,
                num_half_pts,
                number_text,
            } => {
                // In Word, `<m:oMathPara>` (display math mode) takes over its entire paragraph
                // and ignores sibling `<w:r>` runs or right tab stops. To preserve true Display Math
                // (`<m:oMathPara>`) horizontally centered at 50% of the page while displaying the
                // equation number `(1)` flush-right and vertically centered on the same line, emit
                // Word's standard 3-column borderless equation layout row (`[12% | 76% | 12%]`).
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
                out.push_str(
                    r#"<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr>"#,
                );
                render_equation_omml(eq, out);
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
                        r#"<w:r><w:rPr><w:rFonts w:ascii="Libertinus Serif" w:hAnsi="Libertinus Serif" w:eastAsia="Heiti SC" w:cs="Libertinus Serif"/><w:sz w:val="{num_half_pts}"/><w:szCs w:val="{num_half_pts}"/></w:rPr>"#,
                        r#"<w:t xml:space="preserve">{}</w:t></w:r></w:p></w:tc>"#,
                        r#"</w:tr></w:tbl>"#,
                    ),
                    escape_xml(number_text),
                    side_dxa = side_dxa,
                    num_half_pts = num_half_pts,
                );
            }
            EquationBlockConstruct::CenteredDisplayParagraph {
                indent_left_dxa,
                before_dxa,
                after_dxa,
            } => {
                out.push_str("<w:p><w:pPr>");
                if indent_left_dxa > 0 {
                    let _ = write!(out, r#"<w:ind w:left="{indent_left_dxa}"/>"#);
                }
                let _ = write!(
                    out,
                    r#"<w:jc w:val="center"/><w:spacing w:before="{before_dxa}" w:after="{after_dxa}"/></w:pPr>"#
                );
                let bm_id = eq
                    .bookmark
                    .as_ref()
                    .map(|bm| self.emit_bookmark_start(bm, out));
                out.push_str(
                    r#"<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr>"#,
                );
                render_equation_omml(eq, out);
                out.push_str("</m:oMathPara>");
                if let Some(id) = bm_id {
                    Self::emit_bookmark_end(id, out);
                }
                out.push_str("</w:p>");
            }
        }
    }
}
