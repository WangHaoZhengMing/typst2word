//! `image` -> `ImageIr`.

use typst::foundations::Packed;
use typst::layout::{Frame, FrameItem, Point, Size};
use typst::syntax::Span;
use typst::visualize::{ExchangeFormat, Image, ImageKind, RasterFormat};

use crate::extract::ExtractorContext;
use crate::extract::frame_tree::TreeNode;
use crate::ir::{HAlignIr, ImageFormatIr, ImageIr};

/// Normalize `ExchangeFormat::Jpg` to `ImageFormatIr::Jpeg`.
#[allow(non_upper_case_globals)]
impl ImageFormatIr {
    pub const Jpg: Self = Self::Jpeg;
}

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Image Extraction
    // =========================================================================
    pub(crate) fn extract_image_elem(
        &self,
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
        &self,
        image: &Image,
        size: Size,
        span: Span,
        align: HAlignIr,
    ) -> Option<ImageIr> {
        let width_pt = size.x.to_pt().max(1.0);
        let height_pt = size.y.to_pt().max(1.0);
        let alt = image.alt().map(|s| s.to_string());

        let (data, format) = match image.kind() {
            ImageKind::Raster(raster) => match raster.format() {
                RasterFormat::Exchange(ExchangeFormat::Png) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Png)
                }
                RasterFormat::Exchange(ExchangeFormat::Jpg) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Jpg)
                }
                RasterFormat::Exchange(ExchangeFormat::Gif) => {
                    (raster.data().as_slice().to_vec(), ImageFormatIr::Gif)
                }
                _ => {
                    // Render WebP or raw pixel format via `typst_svg::svg_frame` into SVG.
                    let mut f = Frame::hard(size);
                    f.push(Point::zero(), FrameItem::Image(image.clone(), size, span));
                    (typst_svg::svg_frame(&f).into_bytes(), ImageFormatIr::Svg)
                }
            },
            ImageKind::Svg(svg) => (svg.data().as_slice().to_vec(), ImageFormatIr::Svg),
            ImageKind::Pdf(_) => {
                let mut f = Frame::hard(size);
                f.push(Point::zero(), FrameItem::Image(image.clone(), size, span));
                (typst_svg::svg_frame(&f).into_bytes(), ImageFormatIr::Svg)
            }
        };

        Some(ImageIr {
            data,
            format,
            width_pt,
            height_pt,
            alt,
            align,
        })
    }
}
