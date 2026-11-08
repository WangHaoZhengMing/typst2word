//! `bibliography` -> `BlockNode::Bibliography`.

use typst::model::{HeadingElem, TitleElem};
use typst::pdf::PdfMarkerTagKind;

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BibEntryIr, BlockNode};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_bibliography(&mut self, children: &[TreeNode]) -> BlockNode {
        let mut title = None;
        let mut entries = Vec::new();

        for child in children {
            if let TreeNode::Element {
                elem,
                children: cchildren,
                ..
            } = child
            {
                if elem.is::<HeadingElem>() || elem.is::<TitleElem>() {
                    title = Some(self.extract_inlines_flat(cchildren));
                    continue;
                }
            }
        }

        let mut marker_items = Vec::new();
        Self::collect_pdf_marker_elements(children, &mut marker_items);
        let mut pending_prefix: Option<String> = None;

        for (kind, mchildren) in marker_items {
            match kind {
                PdfMarkerTagKind::ListItemLabel => {
                    let txt = Self::nodes_to_plain_text(&mchildren).trim().to_string();
                    if !txt.is_empty() {
                        pending_prefix = Some(txt);
                    }
                }
                PdfMarkerTagKind::BibEntry => {
                    let content = self.extract_inlines_flat(&mchildren);
                    entries.push(BibEntryIr {
                        prefix: pending_prefix.take(),
                        content,
                        bookmark: None,
                    });
                }
                PdfMarkerTagKind::Bibliography(_) => {
                    let mut inner_markers = Vec::new();
                    Self::collect_pdf_marker_elements(&mchildren, &mut inner_markers);
                    for (ikind, ichildren) in inner_markers {
                        match ikind {
                            PdfMarkerTagKind::ListItemLabel => {
                                let txt = Self::nodes_to_plain_text(&ichildren).trim().to_string();
                                if !txt.is_empty() {
                                    pending_prefix = Some(txt);
                                }
                            }
                            PdfMarkerTagKind::BibEntry => {
                                let content = self.extract_inlines_flat(&ichildren);
                                entries.push(BibEntryIr {
                                    prefix: pending_prefix.take(),
                                    content,
                                    bookmark: None,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        BlockNode::Bibliography { title, entries }
    }
}
