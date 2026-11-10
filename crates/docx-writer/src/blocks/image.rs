//! Block `image` -> paragraph wrapping an inline picture.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::ImageIr;

impl<'a> DocxWriter<'a> {
    /// `BlockNode::ImageBlock` -> centered paragraph holding an inline picture.
    pub(crate) fn render_image_block(&mut self, img: &ImageIr, out: &mut String) {
        self.render_image_block_with_layout(img, 0, false, out);
    }

    pub(crate) fn render_image_block_with_layout(
        &mut self,
        img: &ImageIr,
        left_indent_dxa: usize,
        keep_with_next: bool,
        out: &mut String,
    ) {
        out.push_str("<w:p><w:pPr>");
        if keep_with_next {
            out.push_str("<w:keepNext/>");
        }
        if left_indent_dxa > 0 {
            let _ = write!(out, r#"<w:ind w:left="{left_indent_dxa}"/>"#);
        }
        Self::render_jc(img.align, out);
        out.push_str("<w:spacing w:before=\"40\" w:after=\"20\"/></w:pPr>");
        self.render_image_run(img, out);
        out.push_str("</w:p>");
    }
}
