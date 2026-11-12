//! Block `image` -> paragraph wrapping an inline picture.

use std::fmt::Write as _;

use crate::writer::DocxWriter;
use word_construct::model::ImageIr;

impl<'a> DocxWriter<'a> {
    /// `BlockNode::ImageBlock` -> centered paragraph holding an inline picture.
    pub(crate) fn render_image_block(&mut self, img: &ImageIr, out: &mut String) {
        self.render_image_block_with_collapse(img, 0, false, false, out);
    }

    pub(crate) fn render_image_block_collapsed(
        &mut self,
        img: &ImageIr,
        collapse_before: bool,
        out: &mut String,
    ) {
        self.render_image_block_with_collapse(img, 0, false, collapse_before, out);
    }

    pub(crate) fn render_image_block_with_layout(
        &mut self,
        img: &ImageIr,
        left_indent_dxa: usize,
        keep_with_next: bool,
        out: &mut String,
    ) {
        self.render_image_block_with_collapse(img, left_indent_dxa, keep_with_next, false, out);
    }

    pub(crate) fn render_image_block_with_collapse(
        &mut self,
        img: &ImageIr,
        left_indent_dxa: usize,
        keep_with_next: bool,
        collapse_before: bool,
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
        let before = if collapse_before { 0 } else { 40 };
        let _ = write!(out, r#"<w:spacing w:before="{before}" w:after="20"/></w:pPr>"#);
        self.render_image_run(img, out);
        out.push_str("</w:p>");
    }
}
