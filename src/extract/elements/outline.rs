//! `outline` -> `BlockNode::Outline` (TOC entries with page numbers).

use typst::layout::RepeatElem;
use typst::model::{HeadingElem, OutlineEntry, TitleElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{BlockNode, InlineNode, OutlineEntryIr, TextRunIr};

impl<'a> ExtractorContext<'a> {
    pub(crate) fn extract_outline(&mut self, children: &[TreeNode]) -> BlockNode {
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
                    title = Some(self.extract_inlines_flat_with_bold(cchildren, true));
                    continue;
                }
            }
            self.collect_outline_entries(child, &mut entries);
        }

        BlockNode::Outline { title, entries }
    }

    pub(crate) fn collect_outline_entries(
        &mut self,
        node: &TreeNode,
        out: &mut Vec<OutlineEntryIr>,
    ) {
        match node {
            TreeNode::Element { elem, children, .. } => {
                if let Some(entry) = elem.to_packed::<OutlineEntry>() {
                    let level = entry.level.get();
                    let target_bookmark = entry
                        .element_location()
                        .ok()
                        .and_then(|loc| self.bookmarks.get(&loc).cloned());
                    let all_text = Self::nodes_to_plain_text(children).replace('\u{2060}', "");
                    let page = all_text
                        .split_whitespace()
                        .last()
                        .unwrap_or("1")
                        .to_string();

                    let prefix_str = Self::find_outline_entry_prefix(children);
                    let filtered_nodes = Self::filter_outline_entry_body_nodes(children);
                    let raw_inlines = self.extract_inlines_flat(&filtered_nodes);
                    let mut body_inlines = Self::unwrap_link_inlines(raw_inlines);
                    Self::strip_trailing_outline_page(&mut body_inlines, &page);

                    if body_inlines.is_empty() {
                        let fallback_title = entry
                            .body()
                            .ok()
                            .map(|c| c.plain_text().to_string())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| entry.element.plain_text().to_string());
                        body_inlines.push(InlineNode::Text(TextRunIr {
                            text: fallback_title,
                            font_family: Some("Heiti SC".to_string()),
                            font_size_pt: Some(11.0),
                            ..Default::default()
                        }));
                    }

                    let mut title_inlines = Vec::new();
                    if let Some(ref pref) = prefix_str {
                        let base_sz = body_inlines
                            .iter()
                            .find_map(|inl| {
                                if let InlineNode::Text(r) = inl {
                                    r.font_size_pt
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(11.0);
                        title_inlines.push(InlineNode::Text(TextRunIr {
                            text: format!("{pref}  "),
                            font_family: Some("Libertinus Serif".to_string()),
                            font_size_pt: Some(base_sz),
                            ..Default::default()
                        }));
                    }
                    title_inlines.extend(body_inlines);

                    out.push(OutlineEntryIr {
                        level,
                        prefix: prefix_str,
                        title: title_inlines,
                        page,
                        target_bookmark,
                    });
                    return;
                }
                for child in children {
                    self.collect_outline_entries(child, out);
                }
            }
            TreeNode::Group { children, .. } => {
                for child in children {
                    self.collect_outline_entries(child, out);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn find_outline_entry_prefix(nodes: &[TreeNode]) -> Option<String> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        if matches!(marker.kind, PdfMarkerTagKind::Label) {
                            let txt = Self::nodes_to_plain_text(children)
                                .replace('\u{2060}', "")
                                .trim()
                                .to_string();
                            if !txt.is_empty() {
                                return Some(txt);
                            }
                        }
                    }
                    if let Some(found) = Self::find_outline_entry_prefix(children) {
                        return Some(found);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(found) = Self::find_outline_entry_prefix(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn filter_outline_entry_body_nodes(nodes: &[TreeNode]) -> Vec<TreeNode> {
        let mut out = Vec::new();
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>() {
                        if matches!(marker.kind, PdfMarkerTagKind::Label) {
                            continue;
                        }
                    }
                    if elem.is::<RepeatElem>() || Self::contains_repeat_elem(children) {
                        continue;
                    }
                    out.push(TreeNode::Element {
                        elem: elem.clone(),
                        loc: *loc,
                        pos: *pos,
                        children: Self::filter_outline_entry_body_nodes(children),
                    });
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    if Self::contains_repeat_elem(children) {
                        continue;
                    }
                    out.push(TreeNode::Group {
                        group: group.clone(),
                        pos: *pos,
                        children: Self::filter_outline_entry_body_nodes(children),
                    });
                }
                other => out.push(other.clone()),
            }
        }
        out
    }

    pub(crate) fn contains_repeat_elem(nodes: &[TreeNode]) -> bool {
        nodes.iter().any(|n| match n {
            TreeNode::Element { elem, children, .. } => {
                elem.is::<RepeatElem>() || Self::contains_repeat_elem(children)
            }
            TreeNode::Group { children, .. } => Self::contains_repeat_elem(children),
            _ => false,
        })
    }

    pub(crate) fn strip_trailing_outline_page(inlines: &mut Vec<InlineNode>, page: &str) {
        while let Some(last) = inlines.last_mut() {
            match last {
                InlineNode::HorizontalSpace { .. } => {
                    inlines.pop();
                }
                InlineNode::Text(r) => {
                    let cleaned = r.text.replace('\u{2060}', "");
                    let trimmed_end = cleaned.trim_end();
                    if trimmed_end.is_empty() || trimmed_end == page {
                        inlines.pop();
                    } else if let Some(before_page) = trimmed_end.strip_suffix(page) {
                        let stripped = before_page.trim_end();
                        if stripped.is_empty() {
                            inlines.pop();
                        } else {
                            r.text = stripped.to_string();
                            break;
                        }
                    } else {
                        r.text = trimmed_end.to_string();
                        break;
                    }
                }
                _ => break,
            }
        }
    }
}
