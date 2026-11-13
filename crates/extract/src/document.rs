//! Document-level extraction: metadata, page setup (size, margins, columns, fill,
//! page numbering format), and header / footer artifacts.

use typst::foundations::{NativeElement as _, Smart, StyleChain, StyledElem, Styles};
use typst::introspection::Tag;
use typst::layout::{Dir, Frame, FrameItem, PageElem};
use typst::model::{Document, Numbering};
use typst::pdf::{ArtifactElem, ArtifactKind};
use typst::text::TextElem;

use compiler::PAGE_STYLES_LABEL;
use crate::ExtractorContext;
use crate::color::paint_to_hex;
use crate::frame_tree::TreeNode;
use ir::{BlockNode, HAlignIr, LocaleIr, MetadataIr, PageNumberFormatIr, PageSetupIr, TextDirIr};

impl<'a> ExtractorContext<'a> {
    /// Extract the shared page-level `Styles` trunk directly from `__typst2word_page_styles`
    /// tags in `frame`.
    pub(crate) fn page_styles_from_frame(frame: &Frame) -> Option<Styles> {
        let mut style_maps = Vec::new();
        Self::collect_frame_page_styles(frame, &mut style_maps);
        let (first, rest) = style_maps.split_first()?;
        let mut prefix = first.as_slice();
        for s in rest {
            let common = prefix
                .iter()
                .zip(s.as_slice())
                .take_while(|&(a, b)| a == b)
                .count();
            prefix = &prefix[..common];
        }
        Some(
            prefix
                .iter()
                .filter(|style| {
                    // Keep standard library defaults (!outside / standard) + liftable/page styles.
                    !style.outside()
                        || style.liftable()
                        || style.element() == Some(PageElem::ELEM)
                })
                .cloned()
                .collect(),
        )
    }

    fn collect_frame_page_styles<'b>(frame: &'b Frame, out: &mut Vec<&'b Styles>) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Tag(Tag::Start(elem, _)) => {
                    if elem
                        .label()
                        .is_some_and(|l| l.resolve().as_str() == PAGE_STYLES_LABEL)
                        && let Some(styled) = elem.to_packed::<StyledElem>()
                    {
                        out.push(&styled.styles);
                    }
                }
                FrameItem::Group(group) if group.parent.is_none() => {
                    Self::collect_frame_page_styles(&group.frame, out);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn page_root_font_size_pt(&self, page_index: usize) -> f64 {
        if let Some(page) = self.compiled.document.pages().get(page_index)
            && let Some(trunk) = Self::page_styles_from_frame(&page.frame)
        {
            return Self::resolve_length_pt(StyleChain::new(&trunk).get(TextElem::size).0, 11.0);
        }
        Self::resolve_length_pt(self.style_chain().get(TextElem::size).0, 11.0)
    }

    pub(crate) fn extract_metadata(&self) -> MetadataIr {
        let info = self.compiled.document.info();
        let root_snap = self.compiled.semantics.style_of(None);
        let locale = info
            .locale
            .custom()
            .map(|loc| LocaleIr::new(loc.lang.as_str(), loc.region.map(|r| r.as_str().to_string())))
            .or_else(|| {
                (root_snap.lang != "en" || root_snap.region.is_some())
                    .then(|| LocaleIr::new(root_snap.lang.clone(), root_snap.region.clone()))
            });
        let dir = Some(
            if root_snap.dir_rtl || self.style_chain().resolve(TextElem::dir) == Dir::RTL {
                TextDirIr::Rtl
            } else {
                TextDirIr::Ltr
            },
        );

        MetadataIr {
            title: info.title.as_ref().map(|s| s.to_string()),
            authors: info.author.iter().map(|s| s.to_string()).collect(),
            description: info.description.as_ref().map(|s| s.to_string()),
            keywords: info.keywords.iter().map(|s| s.to_string()).collect(),
            locale,
            dir,
        }
    }

    pub(crate) fn extract_page_setup(
        &mut self,
        page_index: usize,
        page_nodes: &[TreeNode],
    ) -> PageSetupIr {
        self.current_page_index = page_index;
        let mut setup = PageSetupIr::default();
        if let Some(first_page) = self.compiled.document.pages().get(page_index) {
            let size = first_page.frame.size();
            setup.width_pt = size.x.to_pt();
            setup.height_pt = size.y.to_pt();
            setup.fill = first_page
                .fill
                .clone()
                .custom()
                .and_then(|p| p.as_ref().and_then(paint_to_hex));

            if let Some(Numbering::Pattern(ref pat)) = first_page.numbering
                && let Some((_, system)) = pat.pieces.first()
            {
                let mut single = pat.clone();
                single.pieces = [(Default::default(), *system)].into_iter().collect();
                single.suffix = Default::default();
                setup.page_number_format = match single.apply(None, &[1]).unwrap_or_default().as_str() {
                    "1" => Some(PageNumberFormatIr::Arabic),
                    "i" => Some(PageNumberFormatIr::LowerRoman),
                    "I" => Some(PageNumberFormatIr::UpperRoman),
                    "a" => Some(PageNumberFormatIr::LowerAlpha),
                    "A" => Some(PageNumberFormatIr::UpperAlpha),
                    _ => None,
                };
            }

            setup.columns = self.style_chain().get(PageElem::columns).get();
            self.page_columns = setup.columns;
            if setup.columns > 1 {
                self.active_columns = (setup.columns, 18.0);
            }

            let base_font_pt = self.page_root_font_size_pt(page_index);
            let chain = self.style_chain();
            let cfg_width_pt = match chain.get(PageElem::width) {
                Smart::Custom(len) => Some(Self::resolve_length_pt(len, base_font_pt)),
                Smart::Auto => None,
            };
            let cfg_height_pt = match chain.get(PageElem::height) {
                Smart::Custom(len) => Some(Self::resolve_length_pt(len, base_font_pt)),
                Smart::Auto => None,
            };
            let min_dim_pt = match (cfg_width_pt, cfg_height_pt) {
                (Some(w), Some(h)) if w.is_finite() && h.is_finite() => w.min(h),
                _ => typst::layout::Paper::A4.width().to_pt(),
            };
            let default_margin_pt = (2.5 / 21.0) * min_dim_pt;

            let margin_cfg = chain.get(PageElem::margin);
            let mut rel_axis_x = cfg_width_pt.filter(|v| v.is_finite());
            let mut rel_axis_y = cfg_height_pt.filter(|v| v.is_finite());
            if chain.get(PageElem::flipped) {
                std::mem::swap(&mut rel_axis_x, &mut rel_axis_y);
            }
            let resolve_side = |side: Option<Smart<typst::layout::Rel<typst::layout::Length>>>,
                                axis_pt: Option<f64>|
             -> Option<f64> {
                match side {
                    Some(Smart::Custom(rel)) => {
                        let abs_pt = Self::resolve_length_pt(rel.abs, base_font_pt);
                        let rel_pt = axis_pt.map_or(0.0, |whole| rel.rel.get() * whole);
                        Some((abs_pt + rel_pt).max(0.0))
                    }
                    _ => None,
                }
            };

            let (raw_left, raw_right, cfg_top, cfg_bottom, two_sided) = match margin_cfg {
                Smart::Custom(m) => (
                    resolve_side(m.sides.left, rel_axis_x),
                    resolve_side(m.sides.right, rel_axis_x),
                    resolve_side(m.sides.top, rel_axis_y),
                    resolve_side(m.sides.bottom, rel_axis_y),
                    m.two_sided.unwrap_or(false),
                ),
                Smart::Auto => (None, None, None, None, false),
            };

            let mut left_pt = raw_left.unwrap_or(default_margin_pt);
            let mut right_pt = raw_right.unwrap_or(default_margin_pt);
            if two_sided {
                setup.mirror_margins = true;
                let binding = match chain.get(PageElem::binding) {
                    Smart::Custom(b) => b,
                    Smart::Auto => {
                        if chain.resolve(TextElem::dir) == Dir::RTL {
                            typst::layout::Binding::Right
                        } else {
                            typst::layout::Binding::Left
                        }
                    }
                };
                if binding == typst::layout::Binding::Right {
                    std::mem::swap(&mut left_pt, &mut right_pt);
                }
            }

            setup.margin_left_pt = left_pt;
            setup.margin_right_pt = right_pt;
            setup.margin_top_pt = cfg_top.unwrap_or(default_margin_pt);
            setup.margin_bottom_pt = cfg_bottom.unwrap_or(default_margin_pt);

            self.content_left_pt = setup.margin_left_pt;
            self.content_top_pt = setup.margin_top_pt;
            self.content_right_pt =
                (setup.width_pt - setup.margin_right_pt).max(setup.margin_left_pt + 36.0);
            self.page_width_pt = setup.width_pt;
            self.mirror_margins = setup.mirror_margins;

            for node in page_nodes {
                self.collect_header_footer_artifacts(node, &mut setup.header, &mut setup.footer);
            }
            self.zero_header_footer_paragraph_spacing(&mut setup.header);
            self.zero_header_footer_paragraph_spacing(&mut setup.footer);

            setup.header_margin_pt = self
                .find_artifact_y_bounds(page_nodes, ArtifactKind::Header)
                .map(|(min_y, _)| {
                    if setup.margin_top_pt < 36.0 {
                        min_y.clamp(0.0, (setup.margin_top_pt - 6.0).max(0.0))
                    } else {
                        min_y.clamp(12.0, (setup.margin_top_pt - 12.0).max(12.0))
                    }
                })
                .unwrap_or_else(|| {
                    if setup.margin_top_pt < 48.0 {
                        (setup.margin_top_pt * 0.5).clamp(0.0, setup.margin_top_pt)
                    } else {
                        (setup.margin_top_pt * 0.58).max(28.0).min(setup.margin_top_pt)
                    }
                });
            setup.footer_margin_pt = self
                .find_artifact_y_bounds(page_nodes, ArtifactKind::Footer)
                .map(|(_, max_y)| {
                    let dist_bottom = (setup.height_pt - max_y).max(0.0);
                    if setup.margin_bottom_pt < 36.0 {
                        dist_bottom.clamp(0.0, (setup.margin_bottom_pt - 6.0).max(0.0))
                    } else {
                        dist_bottom.clamp(12.0, (setup.margin_bottom_pt - 12.0).max(12.0))
                    }
                })
                .unwrap_or_else(|| {
                    if setup.margin_bottom_pt < 48.0 {
                        (setup.margin_bottom_pt * 0.5).clamp(0.0, setup.margin_bottom_pt)
                    } else {
                        (setup.margin_bottom_pt * 0.58)
                            .max(28.0)
                            .min(setup.margin_bottom_pt)
                    }
                });
        }
        setup
    }

    pub(crate) fn zero_header_footer_paragraph_spacing(&mut self, blocks: &mut [BlockNode]) {
        for block in blocks {
            if let BlockNode::Paragraph { id, style, .. } = block {
                if !id.is_unset() {
                    self.reference_layout.remove(id);
                }
                style.space_before_pt = 0.0;
                style.space_after_pt = 0.0;
                style.resolved_par_spacing_pt = None;
                style.resolved_par_leading_pt = None;
            }
        }
    }

    pub(crate) fn find_artifact_y_bounds(
        &self,
        nodes: &[TreeNode],
        target_kind: ArtifactKind,
    ) -> Option<(f64, f64)> {
        for node in nodes {
            match node {
                TreeNode::Element { elem, children, .. } => {
                    if let Some(artifact) = elem.to_packed::<ArtifactElem>()
                        && artifact.kind.get(StyleChain::default()) == target_kind
                    {
                        let (_, min_y, width, height) = Self::estimate_nodes_bbox(children);
                        if (width > 0.0 || height > 0.0) && min_y.is_finite() && height.is_finite()
                        {
                            return Some((min_y, min_y + height));
                        }
                    }
                    if let Some(bounds) = self.find_artifact_y_bounds(children, target_kind) {
                        return Some(bounds);
                    }
                }
                TreeNode::Group { children, .. } => {
                    if let Some(bounds) = self.find_artifact_y_bounds(children, target_kind) {
                        return Some(bounds);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn collect_header_footer_artifacts(
        &mut self,
        node: &TreeNode,
        header: &mut Vec<BlockNode>,
        footer: &mut Vec<BlockNode>,
    ) {
        match node {
            TreeNode::Element { elem, children, .. } => {
                if let Some(artifact) = elem.to_packed::<ArtifactElem>() {
                    match artifact.kind.get(StyleChain::default()) {
                        ArtifactKind::Header => {
                            let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
                            let blocks = self.extract_blocks_from_nodes(children, 0, align);
                            header.extend(blocks);
                            return;
                        }
                        ArtifactKind::Footer => {
                            let align = self.infer_node_halign(children).unwrap_or(HAlignIr::Left);
                            let blocks = self.extract_blocks_from_nodes(children, 0, align);
                            footer.extend(blocks);
                            return;
                        }
                        _ => {}
                    }
                }
                for child in children {
                    self.collect_header_footer_artifacts(child, header, footer);
                }
            }
            TreeNode::Group { children, .. } => {
                for child in children {
                    self.collect_header_footer_artifacts(child, header, footer);
                }
            }
            _ => {}
        }
    }
}
