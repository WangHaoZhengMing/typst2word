//! Pre-layout `Content` pre-scanners (document-wide styles).

use typst::foundations::{Content, SequenceElem, StyledElem, Styles};

use crate::ExtractorContext;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn collect_document_styles(content: &Content, out: &mut Styles) {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            out.apply(styled.styles.clone());
            Self::collect_document_styles(&styled.child, out);
        } else if let Some(seq) = content.to_packed::<SequenceElem>() {
            for child in &seq.children {
                if child.is::<StyledElem>() || child.is::<SequenceElem>() {
                    Self::collect_document_styles(child, out);
                }
            }
        }
    }
}
