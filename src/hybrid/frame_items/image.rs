// MARK: Image items
use typst_preview_core::typst_scene::SceneItem;

use super::formula::{add_formula_region, FormulaRegion};
use super::super::{MathRange, SourceIndex};

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
