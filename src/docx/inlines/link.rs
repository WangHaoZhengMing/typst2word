//! `link` / `ref` -> `<w:hyperlink>`.

use std::fmt::Write as _;

use crate::docx::builder::DocxBuilder;
use crate::ir::{InlineNode, LinkTargetIr, StrokeIr};
use crate::xml::escape_xml;

impl<'a> DocxBuilder<'a> {
    /// `InlineNode::Link` -> external (`r:id`) or internal (`w:anchor`) `<w:hyperlink>`.
    pub(crate) fn render_link(
        &mut self,
        target: Option<&LinkTargetIr>,
        content: &[InlineNode],
        out: &mut String,
    ) {
        match target {
            Some(LinkTargetIr::Url(u)) => {
                let r_id = self.register_hyperlink(u);
                let _ = write!(out, r#"<w:hyperlink r:id="{r_id}" w:history="1">"#);
                self.render_link_styled_inlines(content, out);
                out.push_str("</w:hyperlink>");
            }
            Some(LinkTargetIr::Anchor(a)) => {
                let _ = write!(
                    out,
                    r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
                    escape_xml(a)
                );
                self.render_link_styled_inlines(content, out);
                out.push_str("</w:hyperlink>");
            }
            None => {
                self.render_inlines(content, out);
            }
        }
    }

    /// `InlineNode::Ref` -> internal `<w:hyperlink>` to the target bookmark.
    pub(crate) fn render_ref(&mut self, target: &str, content: &[InlineNode], out: &mut String) {
        let _ = write!(
            out,
            r#"<w:hyperlink w:anchor="{}" w:history="1">"#,
            escape_xml(target)
        );
        self.render_inlines(content, out);
        out.push_str("</w:hyperlink>");
    }

    pub(crate) fn render_link_styled_inlines(&mut self, inlines: &[InlineNode], out: &mut String) {
        for inline in inlines {
            if let InlineNode::Text(run) = inline {
                let mut styled = run.clone();
                if styled.color.is_none() {
                    styled.color = Some("0563C1".to_string());
                }
                if styled.underline.is_none() {
                    styled.underline = Some(StrokeIr {
                        color: "0563C1".to_string(),
                        thickness_pt: 0.75,
                        ..Default::default()
                    });
                }
                self.render_text_run(&styled, out);
            } else {
                self.render_inline(inline, out);
            }
        }
    }
}
