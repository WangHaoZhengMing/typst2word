//! Block `image` -> paragraph wrapping an inline picture.

use crate::docx::builder::DocxBuilder;
use crate::ir::ImageIr;

impl<'a> DocxBuilder<'a> {
    /// `BlockNode::ImageBlock` -> centered paragraph holding an inline picture.
    pub(crate) fn render_image_block(&mut self, img: &ImageIr, out: &mut String) {
        out.push_str("<w:p><w:pPr>");
        Self::render_jc(img.align, out);
        out.push_str("<w:spacing w:before=\"40\" w:after=\"20\"/></w:pPr>");
        self.render_image_run(img, out);
        out.push_str("</w:p>");
    }
}
