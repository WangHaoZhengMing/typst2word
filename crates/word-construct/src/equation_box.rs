//! Microsoft Word Office Math (`<m:oMath>` / `<m:oMathPara>`) 2D Box & Typography Solver.
//!
//! Solves the two-degree-of-freedom (2-DoF) mapping from Typst's laid-out equation
//! bounding box `(typst_width_pt, typst_height_pt)` and glyph sizes to Word OOXML:
//! 1. **Vertical Height ($H$)**: controlled via `<w:sz>` (`run_font_size_pt`, isotropic scale $\text{scale}_y$).
//! 2. **Horizontal Width ($W$)**: decoupled from $\text{scale}_y$ via `<w:w>` (`width_scale_pct`, horizontal
//!    character stretch/compression percentage $\text{scale}_x = W_{\text{target}} / (W_{\text{word}}(S_0) \cdot \text{scale}_y)$).

use serde::{Deserialize, Serialize};

use crate::units::pt_to_dxa;

/// Input metrics measured from Typst's laid-out equation frame and semantic `MathNodeIr` AST.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquationBoxInput {
    /// Whether this equation is a block display equation (`$ ... $` with spaces).
    pub is_block: bool,
    /// Tight horizontal width (`pt`) measured from Typst's laid-out equation frame.
    pub typst_width_pt: Option<f64>,
    /// Tight vertical height (`pt`) measured from Typst's laid-out equation frame.
    pub typst_height_pt: Option<f64>,
    /// Maximum glyph point size (`TextItem.size` in `pt`) inside the laid-out equation frame.
    pub typst_max_glyph_pt: Option<f64>,
    /// Minimum glyph point size (`TextItem.size` in `pt`) inside the laid-out equation frame.
    pub typst_min_glyph_pt: Option<f64>,
    /// Active surrounding paragraph/document font size (`TextElem::size` in `pt`).
    pub context_font_size_pt: f64,
    /// Maximum available column or container width (`pt`), if constrained.
    pub max_available_width_pt: Option<f64>,
    /// Number of vertically stacked rows (`1` for single-line, `> 1` for `EquationArray` or `Matrix`).
    pub row_count: usize,
    /// Maximum nesting depth of vertical fractions (`<m:f>` with `bar` or `noBar`).
    pub max_fraction_depth: usize,
    /// Whether the equation contains tall constructs (radicals, under/over limits, group braces, scripts).
    pub has_tall_construct: bool,
    /// Whether all top-level glyphs in the equation belong to a fraction or script (no unscaled root glyphs).
    pub all_glyphs_in_fraction_or_script: bool,
    /// Estimated horizontal width of the equation in `em` units under Word's `Cambria Math` + `Songti SC` metrics.
    pub expected_em_width: f64,
}

/// Vertical structural profile of an equation for natural Word OMML height estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquationVerticalProfile {
    /// Multi-row `EquationArray` or `Matrix` (`rows > 1`).
    MultiRow {
        rows: usize,
        max_fraction_depth: usize,
    },
    /// Single-row block equation containing vertical fractions (`<m:f>`).
    BlockFraction { depth: usize },
    /// Single-row inline equation containing vertical fractions (`<m:f>`).
    InlineFraction { depth: usize },
    /// Single-row equation containing tall radicals, limits, braces, or scripts.
    TallConstruct,
    /// Flat single-line equation without tall vertical structures.
    FlatSingleLine,
}

impl EquationVerticalProfile {
    /// Classify an [`EquationBoxInput`] into an [`EquationVerticalProfile`].
    #[inline]
    pub const fn classify(input: &EquationBoxInput) -> Self {
        let rows = match input.row_count {
            0 => 1,
            r => r,
        };
        match (
            rows > 1,
            input.max_fraction_depth,
            input.is_block,
            input.has_tall_construct,
        ) {
            (true, depth, _, _) => Self::MultiRow {
                rows,
                max_fraction_depth: depth,
            },
            (false, depth @ 1.., true, _) => Self::BlockFraction { depth },
            (false, depth @ 1.., false, _) => Self::InlineFraction { depth },
            (false, 0, _, true) => Self::TallConstruct,
            (false, 0, _, false) => Self::FlatSingleLine,
        }
    }

    /// Natural vertical height in `em` units rendered by Word's OMML engine at `1.0 * base_pt`.
    #[inline]
    pub fn natural_height_em(self) -> f64 {
        match self {
            Self::MultiRow {
                rows,
                max_fraction_depth,
            } => {
                let per_row_em = 1.35 + 0.65 * max_fraction_depth.min(2) as f64;
                rows as f64 * per_row_em
            }
            Self::BlockFraction { depth } => 1.30 + 0.85 * depth as f64,
            Self::InlineFraction { depth } => 1.00 + 0.58 * depth as f64,
            Self::TallConstruct => 1.30,
            Self::FlatSingleLine => 1.02,
        }
    }

    /// Whether this profile represents a multi-story 2D construct eligible for vertical height calibration.
    #[inline]
    pub const fn is_multi_story(self) -> bool {
        matches!(
            self,
            Self::MultiRow { .. } | Self::BlockFraction { .. } | Self::InlineFraction { .. }
        )
    }
}

/// Compensation mode when Typst's fraction glyph scaling (`display` vs `inline`) differs from Word OMML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FractionScaleMode {
    /// Native OMML fraction scaling matches Typst; no override needed.
    NativeOmml,
    /// Typst used `$ display(a/b) $` inside an inline equation (`100%` glyph size),
    /// whereas Word inline `<m:oMath>` shrinks `<m:f>` by `~0.73` -> compensate by `/ 0.73`.
    InlineDisplayCompensated,
    /// Typst used `$ inline(a/b) $` inside a single-line block equation (`~70%` glyph size),
    /// whereas Word `<m:oMathPara>` renders top-level `<m:f>` at `100%` -> compensate by `* 0.73`.
    BlockInlineCompensated,
}

impl FractionScaleMode {
    #[inline]
    pub fn classify(input: &EquationBoxInput, base_pt: f64) -> Self {
        let rows = input.row_count.max(1);
        match (
            input.max_fraction_depth,
            input.is_block,
            rows,
            input.typst_min_glyph_pt.filter(|&s| s > 1.0),
        ) {
            (0, _, _, _) | (_, _, _, None) => Self::NativeOmml,
            (_, false, _, Some(min_g)) if min_g >= base_pt * 0.92 => {
                Self::InlineDisplayCompensated
            }
            (1, true, 1, Some(min_g)) if min_g <= base_pt * 0.76 => Self::BlockInlineCompensated,
            _ => Self::NativeOmml,
        }
    }

    #[inline]
    pub fn resolve_font_size_pt(self, run_font_size_pt: f64) -> Option<f64> {
        match self {
            Self::NativeOmml => None,
            Self::InlineDisplayCompensated => {
                let compensated = ((run_font_size_pt / 0.73) * 2.0).round() / 2.0;
                Some(compensated.clamp(6.0, 72.0))
            }
            Self::BlockInlineCompensated => {
                let compensated = ((run_font_size_pt * 0.73) * 2.0).round() / 2.0;
                Some(compensated.clamp(5.0, 72.0))
            }
        }
    }
}

/// Solved spatial and typographic parameters for rendering `<m:oMath>` / `<m:oMathPara>` in Word.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WordEquationBox {
    /// Calibrated base `<w:sz>` font size in points (`0.5pt` precision) for top-level math runs.
    pub run_font_size_pt: f64,
    /// Optional calibrated `<w:sz>` font size in points for vertical fraction numerators/denominators
    /// when Typst's display/inline fraction scale differs from Word's OMML context.
    pub fraction_font_size_pt: Option<f64>,
    /// Optional horizontal character width scale percentage (`<w:w w:val="..."/>`, `100` = 100%).
    pub width_scale_pct: Option<u32>,
    /// Target rendered width of the equation in points.
    pub width_pt: f64,
    /// Target rendered height of the equation in points.
    pub height_pt: f64,
    /// Solved `w:before` spacing in `dxa` (`1/20 pt`) for block equations (`<m:oMathPara>`).
    pub block_before_dxa: i64,
    /// Solved `w:after` spacing in `dxa` (`1/20 pt`) for block equations (`<m:oMathPara>`).
    pub block_after_dxa: i64,
}

impl WordEquationBox {
    /// Declaratively solve the Word `<m:oMath>` / `<m:oMathPara>` typographic and 2D box parameters
    /// from Typst's laid-out frame geometry `(typst_width_pt, typst_height_pt)` and AST structure.
    pub fn solve(input: EquationBoxInput) -> Self {
        let ctx_pt = input.context_font_size_pt.max(6.0);

        // 1. Resolve true unscaled base font size (`base_pt`) via pattern matching.
        let raw_base_pt = match (
            input.typst_max_glyph_pt.filter(|&s| s > 1.0),
            input.is_block,
            input.all_glyphs_in_fraction_or_script,
        ) {
            (Some(max_g), false, true) if (max_g / ctx_pt - 0.70).abs() < 0.09 => ctx_pt,
            (Some(max_g), _, _) => max_g,
            (None, _, _) => ctx_pt,
        };
        let base_pt = (raw_base_pt * 2.0).round() / 2.0;

        // 2. Estimate Word OMML's natural vertical height at `base_pt` and solve vertical scale `scale_y`.
        let profile = EquationVerticalProfile::classify(&input);
        let word_natural_h_pt = profile.natural_height_em() * base_pt;

        let height_scale_y = match (
            profile.is_multi_story(),
            input.typst_height_pt.filter(|&h| h > base_pt * 1.1),
        ) {
            (true, Some(typst_h)) => {
                let ratio = (typst_h / word_natural_h_pt).clamp(0.86, 1.12);
                match (ratio - 1.0).abs() > 0.05 {
                    true => ratio,
                    false => 1.0,
                }
            }
            _ => 1.0,
        };

        // 3. Estimate Word OMML's natural horizontal width at `base_pt` and apply column-width fit.
        let em_w = input.expected_em_width.max(0.5);
        let fallback_w_pt = (em_w * base_pt).max(4.0);
        let is_unwrapped_measurement = input.is_block
            || input
                .typst_height_pt
                .is_none_or(|h| h <= word_natural_h_pt.max(base_pt * 2.4) * 1.35);

        let typst_tight_w_pt = is_unwrapped_measurement
            .then_some(input.typst_width_pt)
            .flatten()
            .filter(|&w| w >= 4.0);

        let word_natural_w_pt = typst_tight_w_pt
            .map(|tw| tw * 1.03)
            .unwrap_or(fallback_w_pt);

        let fit_scale_y = match input.max_available_width_pt.filter(|&w| w > 40.0) {
            Some(max_w) if word_natural_w_pt * height_scale_y > max_w * 0.98 => {
                let scaled_w = word_natural_w_pt * height_scale_y;
                height_scale_y * (max_w * 0.96 / scaled_w).clamp(0.80, 1.0)
            }
            _ => height_scale_y,
        };

        let run_font_size_pt = (((base_pt * fit_scale_y) * 2.0).round() / 2.0).clamp(5.0, 72.0);
        let actual_scale_y = run_font_size_pt / base_pt;

        // 4. Calibrate fraction numerator/denominator font size via `FractionScaleMode`.
        let fraction_font_size_pt =
            FractionScaleMode::classify(&input, base_pt).resolve_font_size_pt(run_font_size_pt);

        // 5. Solve horizontal width scale (`<w:w w:val="..."/>`) to decouple width from `actual_scale_y`.
        let word_after_sz_w_pt = word_natural_w_pt * actual_scale_y;
        let target_w_pt = match (typst_tight_w_pt, input.max_available_width_pt) {
            (Some(tw), Some(max_w)) if max_w > 40.0 => Some(tw.min(max_w * 0.97)),
            (Some(tw), _) => Some(tw),
            (None, Some(max_w)) if max_w > 40.0 && word_after_sz_w_pt > max_w * 0.97 => {
                Some(max_w * 0.95)
            }
            _ => None,
        };

        let width_scale_pct = target_w_pt
            .filter(|&w| w >= 8.0)
            .map(|target_w| target_w / word_after_sz_w_pt.max(1.0))
            .filter(|&scale_x| (scale_x - 1.0).abs() >= 0.04)
            .map(|scale_x| (scale_x.clamp(0.82, 1.12) * 100.0).round() as u32)
            .filter(|&pct| pct != 100);

        let final_width_pt = typst_tight_w_pt.unwrap_or(word_after_sz_w_pt).max(1.0);
        let final_height_pt = input
            .typst_height_pt
            .unwrap_or(word_natural_h_pt * actual_scale_y)
            .max(1.0);

        // 6. Solve block equation `<w:spacing w:before w:after/>`.
        let (block_before_dxa, block_after_dxa) = match (input.is_block, profile.is_multi_story()) {
            (true, true) => {
                let dxa = pt_to_dxa(3.0).max(40);
                (dxa, dxa)
            }
            (true, false) => {
                let dxa = pt_to_dxa(4.0).max(40);
                (dxa, dxa)
            }
            (false, _) => (0, 0),
        };

        Self {
            run_font_size_pt,
            fraction_font_size_pt,
            width_scale_pct,
            width_pt: final_width_pt,
            height_pt: final_height_pt,
            block_before_dxa,
            block_after_dxa,
        }
    }
}
