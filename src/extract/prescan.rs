//! Pre-layout `Content` pre-scanners (document-wide styles and explicit linebreaks).

use std::collections::HashSet;

use typst::foundations::{Content, SequenceElem, StyleChain, StyledElem, Styles};

use crate::extract::ExtractorContext;

impl<'a> ExtractorContext<'a> {
    pub(crate) fn collect_linebreak_preceding_texts(content: &Content, out: &mut HashSet<String>) {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            Self::collect_linebreak_preceding_texts(&styled.child, out);
        } else if let Some(seq) = content.to_packed::<SequenceElem>() {
            let mut recent = String::new();
            for child in &seq.children {
                if child.is::<typst::text::LinebreakElem>() {
                    let no_ws: String = recent.chars().filter(|c| !c.is_whitespace()).collect();
                    if !no_ws.is_empty() {
                        let suffix: String = no_ws
                            .chars()
                            .rev()
                            .take(6)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        out.insert(suffix);
                    }
                    recent.clear();
                } else {
                    Self::collect_linebreak_preceding_texts(child, out);
                    let plain = Self::content_plain_text_deep(child);
                    if !plain.is_empty() {
                        recent.push_str(&plain);
                    }
                }
            }
        } else if let Some(cols) = content.to_packed::<typst::layout::ColumnsElem>() {
            Self::collect_linebreak_preceding_texts(&cols.body, out);
        } else if let Some(pad) = content.to_packed::<typst::layout::PadElem>() {
            Self::collect_linebreak_preceding_texts(&pad.body, out);
        } else if let Some(item) = content.to_packed::<typst::model::EnumItem>() {
            Self::collect_linebreak_preceding_texts(&item.body, out);
        } else if let Some(item) = content.to_packed::<typst::model::ListItem>() {
            Self::collect_linebreak_preceding_texts(&item.body, out);
        } else if let Some(par) = content.to_packed::<typst::model::ParElem>() {
            Self::collect_linebreak_preceding_texts(&par.body, out);
        } else if let Some(align) = content.to_packed::<typst::layout::AlignElem>() {
            Self::collect_linebreak_preceding_texts(&align.body, out);
        } else if let Some(blk) = content.to_packed::<typst::layout::BlockElem>() {
            if let Some(typst::layout::BlockBody::Content(body)) =
                blk.body.get_ref(StyleChain::default())
            {
                Self::collect_linebreak_preceding_texts(body, out);
            }
        } else if let Some(bx) = content.to_packed::<typst::layout::BoxElem>() {
            if let Some(body) = bx.body.get_ref(StyleChain::default()) {
                Self::collect_linebreak_preceding_texts(body, out);
            }
        } else if let Some(tc) = content.to_packed::<typst::model::TableCell>() {
            Self::collect_linebreak_preceding_texts(&tc.body, out);
        } else if let Some(gc) = content.to_packed::<typst::layout::GridCell>() {
            Self::collect_linebreak_preceding_texts(&gc.body, out);
        } else if let Some(strong) = content.to_packed::<typst::model::StrongElem>() {
            Self::collect_linebreak_preceding_texts(&strong.body, out);
        } else if let Some(emph) = content.to_packed::<typst::model::EmphElem>() {
            Self::collect_linebreak_preceding_texts(&emph.body, out);
        } else if let Some(fig) = content.to_packed::<typst::model::FigureElem>() {
            Self::collect_linebreak_preceding_texts(&fig.body, out);
        } else if let Some(tbl) = content.to_packed::<typst::model::TableElem>() {
            for child in &tbl.children {
                match child {
                    typst::model::TableChild::Item(typst::model::TableItem::Cell(cell)) => {
                        Self::collect_linebreak_preceding_texts(&cell.body, out);
                    }
                    typst::model::TableChild::Header(hdr) => {
                        for item in &hdr.children {
                            if let typst::model::TableItem::Cell(cell) = item {
                                Self::collect_linebreak_preceding_texts(&cell.body, out);
                            }
                        }
                    }
                    typst::model::TableChild::Footer(ftr) => {
                        for item in &ftr.children {
                            if let typst::model::TableItem::Cell(cell) = item {
                                Self::collect_linebreak_preceding_texts(&cell.body, out);
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else if let Some(grid) = content.to_packed::<typst::layout::GridElem>() {
            for child in &grid.children {
                match child {
                    typst::layout::GridChild::Item(typst::layout::GridItem::Cell(cell)) => {
                        Self::collect_linebreak_preceding_texts(&cell.body, out);
                    }
                    typst::layout::GridChild::Header(hdr) => {
                        for item in &hdr.children {
                            if let typst::layout::GridItem::Cell(cell) = item {
                                Self::collect_linebreak_preceding_texts(&cell.body, out);
                            }
                        }
                    }
                    typst::layout::GridChild::Footer(ftr) => {
                        for item in &ftr.children {
                            if let typst::layout::GridItem::Cell(cell) = item {
                                Self::collect_linebreak_preceding_texts(&cell.body, out);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn content_plain_text_deep(content: &Content) -> String {
        if let Some(styled) = content.to_packed::<StyledElem>() {
            return Self::content_plain_text_deep(&styled.child);
        }
        if let Some(seq) = content.to_packed::<SequenceElem>() {
            let mut s = String::new();
            for ch in &seq.children {
                s.push_str(&Self::content_plain_text_deep(ch));
            }
            return s;
        }
        if let Some(bx) = content.to_packed::<typst::layout::BoxElem>() {
            if let Some(body) = bx.body.get_ref(StyleChain::default()) {
                return Self::content_plain_text_deep(body);
            }
        }
        if let Some(blk) = content.to_packed::<typst::layout::BlockElem>() {
            if let Some(typst::layout::BlockBody::Content(body)) =
                blk.body.get_ref(StyleChain::default())
            {
                return Self::content_plain_text_deep(body);
            }
        }
        if let Some(pad) = content.to_packed::<typst::layout::PadElem>() {
            return Self::content_plain_text_deep(&pad.body);
        }
        if let Some(al) = content.to_packed::<typst::layout::AlignElem>() {
            return Self::content_plain_text_deep(&al.body);
        }
        content.plain_text().to_string()
    }

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
