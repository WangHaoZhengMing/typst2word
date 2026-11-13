//! `image` -> `ImageIr`.

use typst::foundations::{Content, Packed, Smart};
use typst::layout::{Abs, Frame, FrameItem, Point, Sides, Size};
use typst::syntax::Span;
use typst::visualize::{ExchangeFormat, Image, ImageKind, RasterFormat};

use crate::ExtractorContext;
use crate::frame_tree::TreeNode;
use ir::{HAlignIr, ImageFormatIr, ImageIr, SemanticRoleIr};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Image Extraction
    // =========================================================================
    pub(crate) fn extract_image_elem(
        &mut self,
        _img_elem: &Packed<typst::visualize::ImageElem>,
        children: &[TreeNode],
        align: HAlignIr,
    ) -> Option<ImageIr> {
        Self::find_image_in_nodes(children)
            .and_then(|(img, sz, sp)| self.extract_image_item(&img, sz, sp, align))
    }

    pub(crate) fn find_image_in_nodes(nodes: &[TreeNode]) -> Option<(Image, Size, Span)> {
        for node in nodes {
            match node {
                TreeNode::Image {
                    image, size, span, ..
                } => return Some((image.clone(), *size, *span)),
                TreeNode::Element { children, .. } | TreeNode::Group { children, .. } => {
                    if let Some(found) = Self::find_image_in_nodes(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn extract_image_item(
        &mut self,
        image: &Image,
        size: Size,
        span: Span,
        align: HAlignIr,
    ) -> Option<ImageIr> {
        let id = self.alloc_node_id(span, None, Some(SemanticRoleIr::Image));
        let width_pt = size.x.to_pt().max(1.0);
        let height_pt = size.y.to_pt().max(1.0);
        let alt = image.alt().map(|s| s.to_string());

        let (data, format) = match image.kind() {
            ImageKind::Raster(raster) => match raster.format() {
                RasterFormat::Exchange(ExchangeFormat::Png) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Png)
                }
                RasterFormat::Exchange(ExchangeFormat::Jpg) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Jpeg)
                }
                RasterFormat::Exchange(ExchangeFormat::Gif) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Gif)
                }
                _ => {
                    // Render WebP or raw pixel format into SVG via `typst_svg`.
                    let mut f = Frame::hard(size);
                    f.push(Point::zero(), FrameItem::Image(image.clone(), size, span));
                    (frame_to_svg(f).into_bytes(), ImageFormatIr::Svg)
                }
            },
            ImageKind::Svg(svg) => (svg.data().as_slice().to_vec(), ImageFormatIr::Svg),
            ImageKind::Pdf(_) => {
                let mut f = Frame::hard(size);
                f.push(Point::zero(), FrameItem::Image(image.clone(), size, span));
                (frame_to_svg(f).into_bytes(), ImageFormatIr::Svg)
            }
        };

        Some(ImageIr {
            id,
            data,
            format,
            width_pt,
            height_pt,
            alt,
            align,
            placed_offset_pt: None,
        })
    }
}

/// Render a single frame to a standalone SVG (transparent background, no bleed).
///
/// Typst 0.15 removed `typst_svg::svg_frame`; wrap the frame in a bare `Page` instead.
fn frame_to_svg(frame: Frame) -> String {
    let page = typst_layout::Page {
        frame,
        bleed: Sides::splat(Abs::zero()),
        fill: Smart::Custom(None),
        numbering: None,
        supplement: Content::empty(),
        number: 1,
    };
    typst_svg::svg(&page, &typst_svg::SvgOptions::default())
}
