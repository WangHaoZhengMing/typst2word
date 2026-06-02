// MARK: Image items
use std::collections::HashMap;

use docx_rs::{Paragraph, Pic};
use typst_preview_core::typst_scene::{SceneImageFormat, SceneItem};

use super::super::{MathRange, SourceIndex};
use super::super::{floating_raster_paragraph, floating_svg_paragraph};
use super::formula::{FormulaRegion, add_formula_region};

pub(crate) fn collect_formula_regions_from_images(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
    regions: &mut Vec<FormulaRegion>,
) {
    for item in items {
        let SceneItem::Image(image) = item else {
            continue;
        };

        if let Some(math_id) = source_index.location_math_id(image.source.as_ref(), math_ranges) {
            add_formula_region(regions, math_id, image.bounds, None);
        }
    }
}

pub(crate) fn collect_image_fallbacks(
    items: &[SceneItem],
    page_index: usize,
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
    svg_media: &mut HashMap<String, Vec<u8>>,
    paragraphs: &mut Vec<Paragraph>,
) -> usize {
    let mut converted = 0;

    for (item_index, item) in items.iter().enumerate() {
        let SceneItem::Image(image) = item else {
            continue;
        };

        if source_index
            .location_math_id(image.source.as_ref(), math_ranges)
            .is_some()
        {
            continue;
        }

        let media_id = format!("rIdTypstImageP{}N{}", page_index + 1, item_index + 1);
        match image.format {
            SceneImageFormat::Svg => {
                svg_media.insert(media_id.clone(), image.bytes.to_vec());
                paragraphs.push(floating_svg_paragraph(media_id, image.bounds));
            }
            SceneImageFormat::Png
            | SceneImageFormat::Jpeg
            | SceneImageFormat::Gif
            | SceneImageFormat::Webp => {
                let pic = Pic::new(image.bytes.as_ref()).id(media_id);
                paragraphs.push(floating_raster_paragraph(pic, image.bounds));
            }
        }

        converted += 1;
    }

    converted
}
