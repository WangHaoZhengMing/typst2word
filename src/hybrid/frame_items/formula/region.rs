// MARK: Formula regions
use typst_preview_core::typst_scene::SceneRect;

use super::super::super::rect_union;

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

    pub(crate) fn math_id(&self) -> usize {
        self.math_id
    }
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

    if let Some(region) = regions
        .iter_mut()
        .find(|region| region.math_id() == math_id)
    {
        region.add(bounds, baseline);
    } else {
        regions.push(FormulaRegion::new(math_id, bounds, baseline));
    }
}
