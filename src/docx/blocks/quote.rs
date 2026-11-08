//! `quote(block: true)` -> `Quote` paragraphs with attribution.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{BlockNode, InlineNode, TextRunIr};

impl<'a> DocxBuilder<'a> {
    // =========================================================================
    // Blockquotes, Code Blocks, Figures, Equations, Shapes, CeTZ
    // =========================================================================
    pub(crate) fn render_quote(
        &mut self,
        body: &[BlockNode],
        attribution: Option<&[InlineNode]>,
        out: &mut String,
    ) {
        let after_body = if attribution.is_some() { 130 } else { 240 };
        for block in body {
            if let BlockNode::Paragraph { content, .. } = block {
                let _ = write!(
                    out,
                    r#"<w:p><w:pPr><w:pStyle w:val="Quote"/><w:ind w:left="440" w:right="440"/><w:spacing w:before="340" w:after="{after_body}" w:line="260" w:lineRule="auto"/></w:pPr>"#,
                );
                self.render_inlines(content, out);
                out.push_str("</w:p>");
            } else {
                self.render_block(block, out);
            }
        }
        if let Some(attr) = attribution {
            out.push_str(
                r#"<w:p><w:pPr><w:pStyle w:val="Quote"/><w:ind w:left="440" w:right="440"/><w:jc w:val="right"/><w:spacing w:before="60" w:after="280" w:line="240" w:lineRule="auto"/></w:pPr>"#,
            );
            let plain_attr = Self::plain_text_of_inlines(attr);
            let trimmed_attr = plain_attr.trim_start();
            if !trimmed_attr.starts_with('—') && !trimmed_attr.starts_with('-') {
                let dash_run = TextRunIr {
                    text: "— ".to_string(),
                    font_family: Some("Libertinus Serif".to_string()),
                    font_size_pt: Some(11.0),
                    ..Default::default()
                };
                self.render_text_run(&dash_run, out);
            }
            self.render_inlines(attr, out);
            out.push_str("</w:p>");
        }
    }
}
