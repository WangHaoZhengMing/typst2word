//! Full-width `line` -> horizontal rule (paragraph bottom border).

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::StrokeIr;

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::HorizontalRule` -> empty paragraph with a bottom border.
    pub(crate) fn render_horizontal_rule(&mut self, stroke: &StrokeIr, out: &mut String) {
        let sz = ((stroke.thickness_pt * 8.0).round() as u32).clamp(2, 96);
        let val = Self::dash_to_w_border_val(&stroke.dash);
        let _ = write!(
            out,
            r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="{val}" w:sz="{sz}" w:space="1" w:color="{}"/></w:pBdr><w:spacing w:before="90" w:after="90"/></w:pPr></w:p>"#,
            stroke.color
        );
    }
}
