// MARK: Formula extraction
use std::cmp::Ordering;

use typst_preview_core::typst_scene::SceneItem;

use super::super::super::{MathRange, SourceIndex};
use super::super::{group, image, link, shape, tag, text};
use super::region::FormulaRegion;

pub(crate) fn formula_regions(
    items: &[SceneItem],
    source_index: &SourceIndex,
    math_ranges: &[MathRange],
) -> Vec<FormulaRegion> {
    let mut regions = Vec::new();

    text::collect_formula_regions_from_text(items, source_index, math_ranges, &mut regions);
    shape::collect_formula_regions_from_shapes(items, source_index, math_ranges, &mut regions);
    image::collect_formula_regions_from_images(items, source_index, math_ranges, &mut regions);
    group::collect_formula_regions_from_groups(items, &mut regions);
    link::collect_formula_regions_from_links(items, &mut regions);
    tag::collect_formula_regions_from_tags(items, &mut regions);

    regions.sort_by(|a, b| {
        a.bounds()
            .y
            .partial_cmp(&b.bounds().y)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                a.bounds()
                    .x
                    .partial_cmp(&b.bounds().x)
                    .unwrap_or(Ordering::Equal)
            })
    });
    regions
}
