//! Frame tree construction: merges the introspection tag stream (`Tag::Start` /
//! `Tag::End`) with laid-out `FrameItem`s into a `TreeNode` tree.

pub(crate) mod queries;

use typst::foundations::Content;
use typst::introspection::{Location, Tag};
use typst::layout::{Frame, FrameItem, GroupItem, Inherit, Point, Size};
use typst::model::Destination;
use typst::syntax::Span;
use typst::text::TextItem;
use typst::visualize::{Image, Shape};

use crate::ExtractorContext;

/// Virtual vertical stride (`10000.0 pt`) separating consecutive pages in the unified
/// multi-page `TreeNode` coordinate space.
pub(crate) const PAGE_Y_STRIDE_PT: f64 = 10000.0;

#[inline]
pub(crate) fn encode_page_y_offset(page_idx: usize) -> typst::layout::Abs {
    typst::layout::Abs::pt(page_idx as f64 * PAGE_Y_STRIDE_PT)
}

#[inline]
pub(crate) fn decode_page_and_local_y(raw_y_pt: f64, x_pt: f64) -> (usize, f64, f64) {
    let clamped_y = raw_y_pt.max(0.0);
    let page_idx = (clamped_y / PAGE_Y_STRIDE_PT).floor() as usize;
    let local_y = clamped_y - page_idx as f64 * PAGE_Y_STRIDE_PT;
    (page_idx, local_y, x_pt)
}

/// Tree representation unifying `Tag::Start` / `Tag::End` semantic nodes with
/// laid-out `FrameItem` visual nodes.
#[derive(Debug, Clone)]
pub(crate) enum TreeNode {
    Element {
        elem: Content,
        loc: Location,
        pos: Point,
        children: Vec<TreeNode>,
    },
    Group {
        group: GroupItem,
        pos: Point,
        children: Vec<TreeNode>,
    },
    Text {
        text: TextItem,
        pos: Point,
    },
    Shape {
        shape: Shape,
        span: Span,
        pos: Point,
    },
    Image {
        image: Image,
        size: Size,
        span: Span,
        pos: Point,
    },
    Link {
        dest: Destination,
        #[allow(dead_code)]
        size: Size,
        #[allow(dead_code)]
        pos: Point,
    },
    PageBreak,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenElement {
    pub(crate) elem: Content,
    pub(crate) loc: Location,
    pub(crate) pos: Point,
    pub(crate) children: Vec<TreeNode>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingSplitGroup {
    pub(crate) group: GroupItem,
    pub(crate) pos: Point,
    pub(crate) children: Vec<TreeNode>,
    pub(crate) stack: Vec<OpenElement>,
    pub(crate) cum_y: typst::layout::Abs,
}

impl<'a> ExtractorContext<'a> {
    /// Pre-scan a frame to collect `GroupItem`s that have `group.parent.is_some()`.
    pub(crate) fn collect_detached_parent_groups(
        &mut self,
        page_idx: usize,
        frame: &Frame,
        offset: Point,
    ) {
        for (pos, item) in frame.items() {
            let abs_pos = offset + *pos;
            if let FrameItem::Group(group) = item {
                if let Some(parent) = group.parent {
                    match parent.inherit {
                        Inherit::No => {
                            let top_y = self
                                .footnote_top_y_by_page
                                .entry(page_idx)
                                .or_insert(f64::INFINITY);
                            *top_y = top_y.min(abs_pos.y.to_pt());
                            self.footnote_raw_groups
                                .entry(parent.location)
                                .or_default()
                                .push((abs_pos, group.clone()));
                        }
                        Inherit::Yes => {
                            self.logical_child_groups
                                .entry(parent.location)
                                .or_default()
                                .push((abs_pos, group.clone()));
                        }
                    }
                } else {
                    self.collect_detached_parent_groups(page_idx, &group.frame, abs_pos);
                }
            }
        }
    }

    /// Build a hierarchical `Vec<TreeNode>` from `frame`, flattening soft untransformed
    /// line frames while keeping hard/transformed groups and pairing `Tag::Start` / `Tag::End`.
    pub(crate) fn build_frame_tree(&mut self, frame: &Frame, base_offset: Point) -> Vec<TreeNode> {
        let mut root_nodes: Vec<TreeNode> = Vec::new();
        let mut stack: Vec<OpenElement> = Vec::new();
        self.append_frame_items_to_tree(frame, base_offset, &mut root_nodes, &mut stack);
        self.flush_open_stack(&mut root_nodes, &mut stack);
        root_nodes
    }

    /// Build a continuous `Vec<TreeNode>` across all pages of the document, preserving
    /// open tags (`EnumElem`, `ListElem`, `ParElem`, etc.) across natural page wraps and
    /// only inserting `TreeNode::PageBreak` where an explicit `#pagebreak()` occurred.
    pub(crate) fn build_document_frame_tree(
        &mut self,
        mut pagebreak_targets: Vec<String>,
        page_range: std::ops::Range<usize>,
    ) -> Vec<TreeNode> {
        let mut root_nodes: Vec<TreeNode> = Vec::new();
        let mut stack: Vec<OpenElement> = Vec::new();
        let pages = self.compiled.document.pages();

        for (page_idx, page) in pages.iter().enumerate().take(page_range.end).skip(page_range.start) {
            if page_idx > page_range.start && !pagebreak_targets.is_empty() {
                let page_head: String = Self::frame_body_plain_text(&page.frame)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .take(40)
                    .collect();
                if let Some(pos_idx) = pagebreak_targets.iter().position(|t| {
                    t == "__PAGEBREAK_ANY__"
                        || (!page_head.is_empty() && page_head.contains(t.as_str()))
                }) {
                    pagebreak_targets.remove(pos_idx);
                    self.flush_open_stack(&mut root_nodes, &mut stack);
                    root_nodes.push(TreeNode::PageBreak);
                }
            }
            let x_shift_pt = if self.mirror_margins && page_idx % 2 == 1 {
                let right_margin_pt = (self.page_width_pt - self.content_right_pt).max(0.0);
                self.content_left_pt - right_margin_pt
            } else {
                0.0
            };
            let base_offset = Point::new(
                typst::layout::Abs::pt(x_shift_pt),
                encode_page_y_offset(page_idx),
            );
            self.append_frame_items_to_tree(&page.frame, base_offset, &mut root_nodes, &mut stack);
        }

        self.flush_open_stack(&mut root_nodes, &mut stack);
        root_nodes
    }

    pub(crate) fn append_frame_items_to_tree(
        &mut self,
        frame: &Frame,
        base_offset: Point,
        root_nodes: &mut Vec<TreeNode>,
        stack: &mut Vec<OpenElement>,
    ) {
        let mut flat_items: Vec<(Point, FrameItem)> = Vec::new();
        Self::flatten_soft_groups(frame, base_offset, &mut flat_items);

        for (pos, item) in flat_items {
            match item {
                FrameItem::Tag(Tag::Start(elem, _flags)) => {
                    let loc = elem
                        .location()
                        .unwrap_or_else(|| Location::new(typst_utils::hash128(&elem)));
                    stack.push(OpenElement {
                        elem,
                        loc,
                        pos,
                        children: Vec::new(),
                    });
                }
                FrameItem::Tag(Tag::End(loc, _key, _flags)) => {
                    if let Some(match_idx) = stack.iter().rposition(|open| open.loc == loc) {
                        if self.group_tree_depth == 0
                            && !stack
                                .iter()
                                .any(|open| Self::is_page_furniture_elem(&open.elem))
                        {
                            self.flush_pending_split_group(root_nodes, stack);
                        }
                        while stack.len() > match_idx {
                            let mut closed = stack.pop().unwrap();
                            if let Some(detached) = self.logical_child_groups.remove(&closed.loc) {
                                for (gpos, grp) in detached {
                                    self.group_tree_depth += 1;
                                    let gchildren = self.build_frame_tree(&grp.frame, gpos);
                                    self.group_tree_depth -= 1;
                                    closed.children.extend(gchildren);
                                }
                            }
                            let is_furniture = Self::is_page_furniture_elem(&closed.elem);
                            let node = TreeNode::Element {
                                elem: closed.elem,
                                loc: closed.loc,
                                pos: closed.pos,
                                children: closed.children,
                            };
                            if is_furniture
                                && !stack
                                    .iter()
                                    .any(|open| Self::is_page_furniture_elem(&open.elem))
                            {
                                root_nodes.push(node);
                            } else {
                                Self::push_node(root_nodes, stack, node);
                            }
                        }
                    }
                }
                FrameItem::Group(group) => {
                    if group.parent.is_some() {
                        continue;
                    }
                    let in_furniture = stack
                        .iter()
                        .any(|open| Self::is_page_furniture_elem(&open.elem));
                    let has_root_shape = group
                        .frame
                        .items()
                        .any(|(_, it)| matches!(it, FrameItem::Shape(..)));
                    let can_split_merge = self.group_tree_depth == 0
                        && !in_furniture
                        && stack.is_empty()
                        && ((self.pending_split_group.is_some()
                            && group.frame.size().x.to_pt() > 100.0)
                            || (!has_root_shape && group.frame.size().x.to_pt() > 350.0));
                    if can_split_merge {
                        if let Some(mut pending) = self.pending_split_group.take() {
                            let offset = Point::new(typst::layout::Abs::zero(), pending.cum_y);
                            self.group_tree_depth += 1;
                            self.append_frame_items_to_tree(
                                &group.frame,
                                offset,
                                &mut pending.children,
                                &mut pending.stack,
                            );
                            self.group_tree_depth -= 1;
                            pending.cum_y += group.frame.size().y;
                            if pending.stack.is_empty() {
                                let node = TreeNode::Group {
                                    group: pending.group,
                                    pos: pending.pos,
                                    children: pending.children,
                                };
                                Self::push_node(root_nodes, stack, node);
                            } else {
                                self.pending_split_group = Some(pending);
                            }
                        } else {
                            let mut g_children: Vec<TreeNode> = Vec::new();
                            let mut g_stack: Vec<OpenElement> = Vec::new();
                            self.group_tree_depth += 1;
                            self.append_frame_items_to_tree(
                                &group.frame,
                                Point::zero(),
                                &mut g_children,
                                &mut g_stack,
                            );
                            self.group_tree_depth -= 1;
                            if g_stack.is_empty() {
                                let node = TreeNode::Group {
                                    group,
                                    pos,
                                    children: g_children,
                                };
                                Self::push_node(root_nodes, stack, node);
                            } else {
                                let cum_y = group.frame.size().y;
                                self.pending_split_group = Some(PendingSplitGroup {
                                    group,
                                    pos,
                                    children: g_children,
                                    stack: g_stack,
                                    cum_y,
                                });
                            }
                        }
                    } else {
                        if self.group_tree_depth == 0 && !in_furniture {
                            self.flush_pending_split_group(root_nodes, stack);
                        }
                        self.group_tree_depth += 1;
                        let children = self.build_frame_tree(&group.frame, Point::zero());
                        self.group_tree_depth -= 1;
                        let node = TreeNode::Group {
                            group,
                            pos,
                            children,
                        };
                        Self::push_node(root_nodes, stack, node);
                    }
                }
                FrameItem::Text(text) => {
                    Self::push_node(root_nodes, stack, TreeNode::Text { text, pos });
                }
                FrameItem::Shape(shape, span) => {
                    Self::push_node(root_nodes, stack, TreeNode::Shape { shape, span, pos });
                }
                FrameItem::Image(image, size, span) => {
                    Self::push_node(
                        root_nodes,
                        stack,
                        TreeNode::Image {
                            image,
                            size,
                            span,
                            pos,
                        },
                    );
                }
                FrameItem::Link(dest, size) => {
                    Self::push_node(root_nodes, stack, TreeNode::Link { dest, size, pos });
                }
            }
        }
    }

    pub(crate) fn flush_pending_split_group(
        &mut self,
        root_nodes: &mut Vec<TreeNode>,
        stack: &mut Vec<OpenElement>,
    ) {
        if let Some(mut pending) = self.pending_split_group.take() {
            self.group_tree_depth += 1;
            self.flush_open_stack(&mut pending.children, &mut pending.stack);
            self.group_tree_depth -= 1;
            let node = TreeNode::Group {
                group: pending.group,
                pos: pending.pos,
                children: pending.children,
            };
            Self::push_node(root_nodes, stack, node);
        }
    }

    pub(crate) fn flush_open_stack(
        &mut self,
        root_nodes: &mut Vec<TreeNode>,
        stack: &mut Vec<OpenElement>,
    ) {
        if self.group_tree_depth == 0 {
            self.flush_pending_split_group(root_nodes, stack);
        }
        while let Some(mut closed) = stack.pop() {
            if let Some(detached) = self.logical_child_groups.remove(&closed.loc) {
                for (gpos, grp) in detached {
                    self.group_tree_depth += 1;
                    let gchildren = self.build_frame_tree(&grp.frame, gpos);
                    self.group_tree_depth -= 1;
                    closed.children.extend(gchildren);
                }
            }
            let node = TreeNode::Element {
                elem: closed.elem,
                loc: closed.loc,
                pos: closed.pos,
                children: closed.children,
            };
            Self::push_node(root_nodes, stack, node);
        }
    }
}
