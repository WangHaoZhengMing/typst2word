//! `outline` -> `BlockNode::Outline` (TOC entries with page numbers).

use typst::introspection::Location;
use typst::layout::RepeatElem;
use typst::model::{HeadingElem, Numbering, OutlineEntry, TitleElem};
use typst::pdf::{PdfMarkerTag, PdfMarkerTagKind};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, FieldKindIr, InlineNode, OutlineEntryIr, TextRunIr};

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
                    let target_loc = entry.element_location().ok();
                    let target_bookmark = target_loc
                        .and_then(|loc| self.bookmarks.get(&loc).cloned());

                    // Split laid-out `OutlineEntry` nodes around `RepeatElem` (the dot leader),
                    // recursing into wrapper `Group`/`Element` frames rather than discarding them
                    // so the shaped fonts and CJK-Latin spacing of both title and page are kept.
                    let mut seen_repeat = false;
                    let mut before_repeat = Vec::new();
                    let mut after_repeat = Vec::new();
                    Self::split_outline_entry_around_repeat(
                        children,
                        &mut seen_repeat,
                        &mut before_repeat,
                        &mut after_repeat,
                    );

                    let mut prefix_nodes = Vec::new();
                    let mut body_nodes = Vec::new();
                    Self::partition_outline_label_and_body(
                        &before_repeat,
                        &mut prefix_nodes,
                        &mut body_nodes,
                    );

                    let prefix_str = {
                        let txt = Self::nodes_to_plain_text(&prefix_nodes)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        (!txt.is_empty()).then_some(txt)
                    };

                    let mut prefix_inlines =
                        Self::unwrap_link_inlines(self.extract_inlines_flat(&prefix_nodes));
                    ir::coalesce_adjacent_text_runs(&mut prefix_inlines);

                    let mut body_inlines =
                        Self::unwrap_link_inlines(self.extract_inlines_flat(&body_nodes));
                    ir::coalesce_adjacent_text_runs(&mut body_inlines);

                    let mut after_inlines =
                        Self::unwrap_link_inlines(self.extract_inlines_flat(&after_repeat));
                    ir::coalesce_adjacent_text_runs(&mut after_inlines);

                    let (page, page_inlines) = if seen_repeat && !after_inlines.is_empty() {
                        let page_txt = Self::inlines_plain_text(&after_inlines)
                            .replace('\u{2060}', "")
                            .trim()
                            .to_string();
                        let p_inlines = self.build_outline_page_inlines(
                            &after_inlines,
                            target_loc,
                            target_bookmark.as_deref(),
                        );
                        (
                            if page_txt.is_empty() {
                                "1".to_string()
                            } else {
                                page_txt
                            },
                            p_inlines,
                        )
                    } else {
                        let all_text =
                            Self::nodes_to_plain_text(children).replace('\u{2060}', "");
                        let fallback_page = all_text
                            .split_whitespace()
                            .last()
                            .unwrap_or("1")
                            .to_string();
                        Self::strip_trailing_outline_page(&mut body_inlines, &fallback_page);
                        let p_inlines = self.build_fallback_outline_page_inlines(
                            &fallback_page,
                            &body_inlines,
                            target_bookmark.as_deref(),
                        );
                        (fallback_page, p_inlines)
                    };

                    if body_inlines.is_empty() {
                        let fallback_title = entry
                            .body()
                            .ok()
                            .map(|c| c.plain_text().to_string())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| entry.element.plain_text().to_string());
                        body_inlines.push(InlineNode::Text(TextRunIr {
                            text: fallback_title,
                            font_size_pt: Some(11.0),
                            ..Default::default()
                        }));
                    }

                    let mut title_inlines = Vec::new();
                    if !prefix_inlines.is_empty() {
                        let prefix_has_trailing_space =
                            prefix_inlines.last().is_some_and(|inl| match inl {
                                InlineNode::HorizontalSpace { .. } | InlineNode::Tab => true,
                                InlineNode::Text(r) => r.text.ends_with(char::is_whitespace),
                                _ => false,
                            });
                        let body_has_leading_space =
                            body_inlines.first().is_some_and(|inl| match inl {
                                InlineNode::HorizontalSpace { .. } | InlineNode::Tab => true,
                                InlineNode::Text(r) => r.text.starts_with(char::is_whitespace),
                                _ => false,
                            });
                        title_inlines.extend(prefix_inlines);
                        if !prefix_has_trailing_space && !body_has_leading_space {
                            if let Some(InlineNode::Text(last_pref)) = title_inlines.last_mut() {
                                last_pref.text.push_str("  ");
                            }
                        }
                    } else if let Some(ref pref) = prefix_str {
                        let base_sz = body_inlines
                            .iter()
                            .find_map(|inl| match inl {
                                InlineNode::Text(r) => r.font_size_pt,
                                _ => None,
                            })
                            .unwrap_or(11.0);
                        title_inlines.push(InlineNode::Text(TextRunIr {
                            text: format!("{pref}  "),
                            font_size_pt: Some(base_sz),
                            ..Default::default()
                        }));
                    }
                    title_inlines.extend(body_inlines);
                    ir::coalesce_adjacent_text_runs(&mut title_inlines);

                    out.push(OutlineEntryIr {
                        level,
                        prefix: prefix_str,
                        title: title_inlines,
                        page,
                        page_inlines,
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

    /// Recursively split `OutlineEntry` nodes into `(before_repeat, after_repeat)` while keeping
    /// enclosing `Group` and `Element` wrappers (and their local `pos` offsets) intact.
    fn split_outline_entry_around_repeat(
        nodes: &[TreeNode],
        seen_repeat: &mut bool,
        before_repeat: &mut Vec<TreeNode>,
        after_repeat: &mut Vec<TreeNode>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if elem.is::<RepeatElem>() {
                        *seen_repeat = true;
                        continue;
                    }
                    let mut sub_before = Vec::new();
                    let mut sub_after = Vec::new();
                    Self::split_outline_entry_around_repeat(
                        children,
                        seen_repeat,
                        &mut sub_before,
                        &mut sub_after,
                    );
                    if !sub_before.is_empty() {
                        before_repeat.push(TreeNode::Element {
                            elem: elem.clone(),
                            loc: *loc,
                            pos: *pos,
                            children: sub_before,
                        });
                    }
                    if !sub_after.is_empty() {
                        after_repeat.push(TreeNode::Element {
                            elem: elem.clone(),
                            loc: *loc,
                            pos: *pos,
                            children: sub_after,
                        });
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let mut sub_before = Vec::new();
                    let mut sub_after = Vec::new();
                    Self::split_outline_entry_around_repeat(
                        children,
                        seen_repeat,
                        &mut sub_before,
                        &mut sub_after,
                    );
                    if !sub_before.is_empty() {
                        before_repeat.push(TreeNode::Group {
                            group: group.clone(),
                            pos: *pos,
                            children: sub_before,
                        });
                    }
                    if !sub_after.is_empty() {
                        after_repeat.push(TreeNode::Group {
                            group: group.clone(),
                            pos: *pos,
                            children: sub_after,
                        });
                    }
                }
                other => {
                    if *seen_repeat {
                        after_repeat.push(other.clone());
                    } else {
                        before_repeat.push(other.clone());
                    }
                }
            }
        }
    }

    /// Partition `before_repeat` nodes into `PdfMarkerTag(Label)` prefix nodes and title body nodes,
    /// preserving enclosing `Group` / `Element` geometry.
    fn partition_outline_label_and_body(
        nodes: &[TreeNode],
        label_out: &mut Vec<TreeNode>,
        body_out: &mut Vec<TreeNode>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Element {
                    elem,
                    loc,
                    pos,
                    children,
                } => {
                    if let Some(marker) = elem.to_packed::<PdfMarkerTag>()
                        && matches!(marker.kind, PdfMarkerTagKind::Label)
                    {
                        label_out.push(node.clone());
                        continue;
                    }
                    let mut sub_lbl = Vec::new();
                    let mut sub_body = Vec::new();
                    Self::partition_outline_label_and_body(children, &mut sub_lbl, &mut sub_body);
                    if !sub_lbl.is_empty() {
                        label_out.push(TreeNode::Element {
                            elem: elem.clone(),
                            loc: *loc,
                            pos: *pos,
                            children: sub_lbl,
                        });
                    }
                    if !sub_body.is_empty() {
                        body_out.push(TreeNode::Element {
                            elem: elem.clone(),
                            loc: *loc,
                            pos: *pos,
                            children: sub_body,
                        });
                    }
                }
                TreeNode::Group {
                    group,
                    pos,
                    children,
                } => {
                    let mut sub_lbl = Vec::new();
                    let mut sub_body = Vec::new();
                    Self::partition_outline_label_and_body(children, &mut sub_lbl, &mut sub_body);
                    if !sub_lbl.is_empty() {
                        label_out.push(TreeNode::Group {
                            group: group.clone(),
                            pos: *pos,
                            children: sub_lbl,
                        });
                    }
                    if !sub_body.is_empty() {
                        body_out.push(TreeNode::Group {
                            group: group.clone(),
                            pos: *pos,
                            children: sub_body,
                        });
                    }
                }
                other => body_out.push(other.clone()),
            }
        }
    }

    fn inlines_plain_text(inlines: &[InlineNode]) -> String {
        let mut s = String::new();
        for inl in inlines {
            match inl {
                InlineNode::Text(r) => s.push_str(&r.text),
                InlineNode::HorizontalSpace { .. } => s.push(' '),
                InlineNode::Link { content, .. }
                | InlineNode::Ref { content, .. }
                | InlineNode::Citation { content, .. }
                | InlineNode::Field { content, .. } => {
                    s.push_str(&Self::inlines_plain_text(content));
                }
                _ => {}
            }
        }
        s
    }

    /// Construct formatted `page_inlines` (`[prefix, PAGEREF(bm), suffix]`) from the shaped
    /// `after_inlines` runs and the target page's `Numbering` pattern so custom numbering such as
    /// `"第1 页/共 1 页"` renders `"第"` + dynamic `PAGEREF` + `" 页"` in the document's exact fonts.
    fn build_outline_page_inlines(
        &self,
        after_inlines: &[InlineNode],
        target_loc: Option<Location>,
        target_bookmark: Option<&str>,
    ) -> Vec<InlineNode> {
        let mut runs: Vec<TextRunIr> = Vec::new();
        for inl in after_inlines {
            match inl {
                InlineNode::Text(r) => {
                    let cleaned = r.text.replace('\u{2060}', "");
                    if !cleaned.is_empty() {
                        let mut cr = r.clone();
                        cr.text = cleaned;
                        runs.push(cr);
                    }
                }
                InlineNode::HorizontalSpace { .. } => {
                    let mut sp = runs.last().cloned().unwrap_or_default();
                    sp.text = " ".to_string();
                    runs.push(sp);
                }
                _ => {}
            }
        }
        if runs.is_empty() {
            return Vec::new();
        }
        let Some(bm) = target_bookmark else {
            return runs.into_iter().map(InlineNode::Text).collect();
        };

        let full_text: String = runs.iter().map(|r| r.text.as_str()).collect();
        let target_numbering = target_loc.and_then(|loc| {
            let page_idx = self
                .compiled
                .introspector()
                .page(loc)?
                .get()
                .saturating_sub(1);
            self.compiled
                .document
                .pages()
                .get(page_idx)
                .and_then(|p| p.numbering.clone())
        });

        let (prefix_byte_len, word_format) = match target_numbering {
            Some(Numbering::Pattern(pattern)) => {
                if let Some((prefix, system)) = pattern.pieces.first() {
                    let pref = prefix.as_str();
                    let p_len = if full_text.starts_with(pref) {
                        pref.len()
                    } else {
                        0
                    };
                    let mut single = pattern.clone();
                    single.pieces = [(Default::default(), *system)].into_iter().collect();
                    single.suffix = Default::default();
                    let sample = single.apply(None, &[1]).unwrap_or_default();
                    // Standard Arabic numbering ("1") inherits the target section's format
                    // without an explicit switch; non-Arabic systems emit `\* <format>`.
                    let fmt = match sample.as_str() {
                        "i" => Some("roman".to_string()),
                        "I" => Some("ROMAN".to_string()),
                        "a" => Some("alphabetic".to_string()),
                        "A" => Some("ALPHABETIC".to_string()),
                        _ => None,
                    };
                    (p_len, fmt)
                } else {
                    (0, None)
                }
            }
            _ => (0, None),
        };

        // Locate the counter value span right after `prefix_byte_len` (or the first ASCII
        // digit/alphanumeric run if `prefix_byte_len == 0` and leading text is non-ASCII).
        let search_tail = &full_text[prefix_byte_len..];
        let rel_range = if let Some(first_ch) = search_tail.chars().next()
            && first_ch.is_ascii_alphanumeric()
        {
            let token_len: usize = search_tail
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .map(char::len_utf8)
                .sum();
            Some(0..token_len)
        } else {
            search_tail
                .char_indices()
                .find(|&(_, c)| c.is_ascii_digit())
                .map(|(start, _)| {
                    let len: usize = search_tail[start..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .map(char::len_utf8)
                        .sum();
                    start..(start + len)
                })
        };

        let Some(rel) = rel_range else {
            return vec![InlineNode::Field {
                kind: FieldKindIr::PageRef {
                    bookmark: bm.to_string(),
                },
                format: word_format,
                content: runs.into_iter().map(InlineNode::Text).collect(),
            }];
        };

        let num_start = prefix_byte_len + rel.start;
        let num_end = prefix_byte_len + rel.end;
        let prefix_runs = Self::slice_text_runs_bytes(&runs, 0, num_start);
        let num_runs = Self::slice_text_runs_bytes(&runs, num_start, num_end);
        let suffix_runs = Self::slice_text_runs_bytes(&runs, num_end, full_text.len());

        let mut out = Vec::new();
        out.extend(prefix_runs.into_iter().map(InlineNode::Text));
        out.push(InlineNode::Field {
            kind: FieldKindIr::PageRef {
                bookmark: bm.to_string(),
            },
            format: word_format,
            content: num_runs.into_iter().map(InlineNode::Text).collect(),
        });
        out.extend(suffix_runs.into_iter().map(InlineNode::Text));
        out
    }

    fn build_fallback_outline_page_inlines(
        &self,
        fallback_page: &str,
        body_inlines: &[InlineNode],
        target_bookmark: Option<&str>,
    ) -> Vec<InlineNode> {
        let sz = body_inlines
            .iter()
            .find_map(|inl| match inl {
                InlineNode::Text(r) => r.font_size_pt,
                _ => None,
            })
            .unwrap_or(11.0);
        let fam = body_inlines.iter().find_map(|inl| match inl {
            InlineNode::Text(r)
                if r.text.chars().any(|c| c.is_ascii_alphanumeric())
                    && r.font_family.is_some() =>
            {
                r.font_family.clone()
            }
            _ => None,
        });
        let run = TextRunIr {
            text: fallback_page.to_string(),
            font_family: fam,
            font_size_pt: Some(sz),
            ..Default::default()
        };
        if let Some(bm) = target_bookmark {
            vec![InlineNode::Field {
                kind: FieldKindIr::PageRef {
                    bookmark: bm.to_string(),
                },
                format: None,
                content: vec![InlineNode::Text(run)],
            }]
        } else {
            vec![InlineNode::Text(run)]
        }
    }

    fn slice_text_runs_bytes(runs: &[TextRunIr], start: usize, end: usize) -> Vec<TextRunIr> {
        if start >= end {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut cursor = 0usize;
        for r in runs {
            let r_start = cursor;
            let r_end = cursor + r.text.len();
            cursor = r_end;
            let ov_start = start.max(r_start);
            let ov_end = end.min(r_end);
            if ov_start < ov_end {
                let mut sub = r.clone();
                sub.text = r.text[(ov_start - r_start)..(ov_end - r_start)].to_string();
                out.push(sub);
            }
        }
        out
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
