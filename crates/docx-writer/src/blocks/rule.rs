//! Horizontal `line` -> horizontal rule (paragraph bottom border with exact width & alignment).

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::{HAlignIr, StrokeIr};
use word_construct::units::pt_to_dxa;

impl<'a> DocxWriter<'a> {
    /// `BlockNode::HorizontalRule` -> empty paragraph with a bottom border.
    pub(crate) fn render_horizontal_rule(
        &mut self,
        length_pt: f64,
        stroke: &StrokeIr,
        align: HAlignIr,
        collapse_before: bool,
        out: &mut String,
    ) {
        let sz = ((stroke.thickness_pt * 8.0).round() as u32).clamp(2, 96);
        let val = Self::dash_to_w_border_val(&stroke.dash);
        let content_w_pt = (self.current_setup.width_pt
            - self.current_setup.margin_left_pt
            - self.current_setup.margin_right_pt)
            .max(72.0);
        let ind_xml = if length_pt > 1.0 && length_pt < content_w_pt - 2.0 {
            let gap_pt = (content_w_pt - length_pt).max(0.0);
            match align {
                HAlignIr::Right => format!(r#"<w:ind w:left="{}"/>"#, pt_to_dxa(gap_pt)),
                HAlignIr::Center => {
                    let half = pt_to_dxa(gap_pt * 0.5);
                    format!(r#"<w:ind w:left="{half}" w:right="{half}"/>"#)
                }
                HAlignIr::Left | HAlignIr::Justify => {
                    format!(r#"<w:ind w:right="{}"/>"#, pt_to_dxa(gap_pt))
                }
            }
        } else {
            String::new()
        };
        let before_dxa = if collapse_before { 0 } else { 60 };
        let _ = write!(
            out,
            r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="{val}" w:sz="{sz}" w:space="0" w:color="{}"/></w:pBdr>{ind_xml}<w:spacing w:before="{before_dxa}" w:after="80" w:line="20" w:lineRule="exact"/><w:rPr><w:sz w:val="2"/><w:szCs w:val="2"/></w:rPr></w:pPr></w:p>"#,
            stroke.color
        );
    }
}
