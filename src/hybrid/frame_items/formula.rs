// MARK: Formula extraction
use std::cmp::Ordering;

use typst_preview_core::typst_scene::{SceneItem, SceneRect};

use super::super::{MathRange, SourceIndex, rect_union};
use super::{group, image, link, shape, tag, text};

#[derive(Clone, Debug)]
pub(crate) struct FormulaRegion {
    math_id: usize,
    bounds: SceneRect,
    baseline_sum: f32,
    baseline_count: usize,
}

impl FormulaRegion {
    fn new(math_id: usize, bounds: SceneRect, baseline: Option<f32>) -> Self {
        let (baseline_sum, baseline_count) = baseline.map(|value| (value, 1)).unwrap_or((0.0, 0));

        Self {
            math_id,
            bounds,
            baseline_sum,
            baseline_count,
        }
    }

    fn add(&mut self, bounds: SceneRect, baseline: Option<f32>) {
        self.bounds = rect_union(self.bounds, bounds);
        if let Some(value) = baseline {
            self.baseline_sum += value;
            self.baseline_count += 1;
        }
    }

    pub(crate) fn bounds(&self) -> SceneRect {
        SceneRect {
            x: self.bounds.x,
            y: self.bounds.y,
            width: self.bounds.width,
            height: self.bounds.height,
        }
    }

    pub(crate) fn baseline_y(&self) -> f32 {
        if self.baseline_count > 0 {
            self.baseline_sum / self.baseline_count as f32
        } else {
            self.bounds.y + self.bounds.height
        }
    }
}

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
        a.bounds
            .y
            .partial_cmp(&b.bounds.y)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                a.bounds
                    .x
                    .partial_cmp(&b.bounds.x)
                    .unwrap_or(Ordering::Equal)
            })
    });
    regions
}

pub(crate) fn add_formula_region(
    regions: &mut Vec<FormulaRegion>,
    math_id: usize,
    bounds: SceneRect,
    baseline: Option<f32>,
) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }

    if let Some(region) = regions.iter_mut().find(|region| region.math_id == math_id) {
        region.add(bounds, baseline);
    } else {
        regions.push(FormulaRegion::new(math_id, bounds, baseline));
    }
}

// MARK: SVG crop helpers
pub(crate) fn formula_crop(bounds: SceneRect, page_width: f32, page_height: f32) -> SceneRect {
    let padding = 2.0;
    let x = (bounds.x - padding).max(0.0);
    let y = (bounds.y - padding).max(0.0);
    let right = (bounds.x + bounds.width + padding).min(page_width);
    let bottom = (bounds.y + bounds.height + padding).min(page_height);

    SceneRect {
        x,
        y,
        width: (right - x).max(1.0),
        height: (bottom - y).max(1.0),
    }
}

pub(crate) fn crop_svg(svg: &str, crop: SceneRect) -> String {
    let mut output = svg.to_string();
    set_svg_attr(
        &mut output,
        "viewBox",
        &format!(
            "{} {} {} {}",
            svg_number(crop.x),
            svg_number(crop.y),
            svg_number(crop.width),
            svg_number(crop.height)
        ),
    );
    set_svg_attr(
        &mut output,
        "width",
        &format!("{}pt", svg_number(crop.width)),
    );
    set_svg_attr(
        &mut output,
        "height",
        &format!("{}pt", svg_number(crop.height)),
    );
    set_svg_attr(&mut output, "overflow", "hidden");
    output
}

fn set_svg_attr(svg: &mut String, name: &str, value: &str) {
    let Some(tag_start) = svg.find("<svg") else {
        return;
    };
    let Some(relative_tag_end) = svg[tag_start..].find('>') else {
        return;
    };
    let tag_end = tag_start + relative_tag_end;
    let pattern = format!("{name}=\"");

    if let Some(relative_attr_start) = svg[tag_start..tag_end].find(&pattern) {
        let value_start = tag_start + relative_attr_start + pattern.len();
        if let Some(relative_value_end) = svg[value_start..].find('"') {
            svg.replace_range(value_start..value_start + relative_value_end, value);
        }
    } else {
        svg.insert_str(tag_end, &format!(" {name}=\"{value}\""));
    }
}

fn svg_number(value: f32) -> String {
    format!("{value:.3}")
}
