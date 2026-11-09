//! Page-margin policies. These affect exported Word properties, not the
//! diagnostic line-break/pagination simulator.

/// Explicitly select whether to apply the historical Word margin offsets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PageMarginCalibration {
    /// Retain the existing output while its historical visual samples are audited.
    #[default]
    LegacyWord,
    /// Preserve requested margins exactly; useful as the measurement baseline.
    Preserve,
}

impl PageMarginCalibration {
    pub fn margins(self, top_pt: f64, bottom_pt: f64, first_block_is_cjk: bool) -> (f64, f64) {
        match self {
            Self::Preserve => (top_pt, bottom_pt),
            Self::LegacyWord => {
                // Historical empirical values; not a universal font-metric model.
                let top_offset = if first_block_is_cjk { 5.0 } else { 3.5 };
                ((top_pt - top_offset).max(18.0), (bottom_pt - 6.0).max(18.0))
            }
        }
    }
}
