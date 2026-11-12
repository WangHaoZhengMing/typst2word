//! Page-margin policies calibrated from binary OpenType font tables (`OS/2.sCapHeight`
//! vs `usWinAscent`/`hhea.ascent`) and Microsoft Word's 300-DPI PDF `Tm` first-line baselines.

/// Measured 12pt first-line baseline deltas (`Word PDF Tm baseline - Typst OS/2.sCapHeight`, in `pt`):
/// - `Times New Roman` (`11.280 - 7.945 = 3.335 pt`)
pub const TIMES_LATIN_12PT_TOP_DELTA_PT: f64 = 3.335;
/// - `Libertinus Serif` (`11.280 - 7.896 = 3.384 pt`)
pub const LIBERTINUS_12PT_TOP_DELTA_PT: f64 = 3.384;
/// - `Arial` / Western sans-serif (`11.280 - 8.596 = 2.684 pt`)
pub const ARIAL_SANS_12PT_TOP_DELTA_PT: f64 = 2.684;
/// - `Georgia` (`11.040 - 8.314 = 2.726 pt`)
pub const GEORGIA_12PT_TOP_DELTA_PT: f64 = 2.726;
/// - `Courier New` / monospace (`10.080 - 6.855 = 3.225 pt`)
pub const COURIER_MONO_12PT_TOP_DELTA_PT: f64 = 3.225;
/// - `Songti SC` / `Kaiti SC` (`11.280 - 8.340 = 2.940 pt`)
pub const SONGTI_KAITI_12PT_TOP_DELTA_PT: f64 = 2.940;
/// - `Heiti SC` / `SimSun` (`12.240 - 8.616 = 3.624 pt`)
pub const HEITI_SIMSUN_12PT_TOP_DELTA_PT: f64 = 3.624;
/// - `PingFang SC` / modern CJK UI sans (`15.120 - 10.320 = 4.800 pt`)
pub const PINGFANG_CJK_12PT_TOP_DELTA_PT: f64 = 4.800;

/// Compute the exact first-line top baseline overhang (`pt`) introduced by Word (`MicrosoftPTLS7`)
/// relative to Typst's `text(top-edge: "cap-height")`, scaled linearly by `font_size_pt / 12.0`.
pub fn measured_first_line_top_delta_pt(
    font_family: Option<&str>,
    font_size_pt: Option<f64>,
    first_block_is_cjk: bool,
) -> f64 {
    let delta_12pt = match font_family.map(|f| f.trim().to_ascii_lowercase()) {
        Some(fl) if fl.contains("pingfang") || fl.contains("yahei") || fl.contains("微软雅黑") => {
            PINGFANG_CJK_12PT_TOP_DELTA_PT
        }
        Some(fl)
            if fl.contains("heiti")
                || fl.contains("stheiti")
                || fl.contains("simhei")
                || fl.contains("simsun")
                || fl.contains("nsimsun")
                || fl.contains("黑体") =>
        {
            HEITI_SIMSUN_12PT_TOP_DELTA_PT
        }
        Some(fl)
            if fl.contains("song")
                || fl.contains("kai")
                || fl.contains("fangsong")
                || fl.contains("stfangsong")
                || fl.contains("宋体")
                || fl.contains("楷体")
                || fl.contains("仿宋") =>
        {
            SONGTI_KAITI_12PT_TOP_DELTA_PT
        }
        Some(fl)
            if fl.contains("arial")
                || fl.contains("helvetica")
                || fl.contains("calibri")
                || fl.contains("verdana") =>
        {
            ARIAL_SANS_12PT_TOP_DELTA_PT
        }
        Some(fl) if fl.contains("georgia") => GEORGIA_12PT_TOP_DELTA_PT,
        Some(fl)
            if fl.contains("courier")
                || fl.contains("menlo")
                || fl.contains("sf mono")
                || fl.contains("consolas")
                || fl.contains("monaco") =>
        {
            COURIER_MONO_12PT_TOP_DELTA_PT
        }
        Some(fl) if fl.contains("libertinus") => LIBERTINUS_12PT_TOP_DELTA_PT,
        Some(fl) if fl.contains("times") || fl.contains("cambria") => {
            TIMES_LATIN_12PT_TOP_DELTA_PT
        }
        _ if first_block_is_cjk => PINGFANG_CJK_12PT_TOP_DELTA_PT,
        _ => TIMES_LATIN_12PT_TOP_DELTA_PT,
    };

    let sz = font_size_pt.unwrap_or(12.0).clamp(6.0, 72.0);
    delta_12pt * (sz / 12.0)
}

/// Explicitly select whether to apply Word first-line baseline margin calibration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PageMarginCalibration {
    /// Compensate `<w:pgMar w:top>` by the measured Word vs Typst first-line baseline delta.
    #[default]
    LegacyWord,
    /// Preserve requested margins verbatim; useful as an uncalibrated baseline.
    Preserve,
}

impl PageMarginCalibration {
    /// Calibrate page top/bottom margins when only CJK vs Latin classification is supplied
    /// (uses measured 12pt defaults: `3.335pt` Latin / `4.800pt` CJK).
    pub fn margins(self, top_pt: f64, bottom_pt: f64, first_block_is_cjk: bool) -> (f64, f64) {
        self.margins_for_first_line(top_pt, bottom_pt, None, None, first_block_is_cjk)
    }

    /// Calibrate page top/bottom margins using the exact first-line font family and font size (`pt`).
    pub fn margins_for_first_line(
        self,
        top_pt: f64,
        bottom_pt: f64,
        font_family: Option<&str>,
        font_size_pt: Option<f64>,
        first_block_is_cjk: bool,
    ) -> (f64, f64) {
        match self {
            Self::Preserve => (top_pt, bottom_pt),
            Self::LegacyWord => {
                // Never inflate explicit zero margins (`margin: 0pt`) into 18pt, and never emit
                // negative `w:top` (which Word interprets as fixed-margin mode and pushes down).
                let top_offset =
                    measured_first_line_top_delta_pt(font_family, font_size_pt, first_block_is_cjk);
                let top = if top_pt <= 0.5 {
                    top_pt.max(0.0)
                } else {
                    (top_pt - top_offset).max(18.0)
                };
                let bottom = if bottom_pt <= 0.5 {
                    bottom_pt.max(0.0)
                } else {
                    (bottom_pt - 6.0).max(18.0)
                };
                (top, bottom)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_12pt_defaults_match_pdf_tm_forensics() {
        assert!((measured_first_line_top_delta_pt(None, None, false) - 3.335).abs() < 1e-6);
        assert!((measured_first_line_top_delta_pt(None, None, true) - 4.800).abs() < 1e-6);
        assert!(
            (measured_first_line_top_delta_pt(Some("Songti SC"), Some(12.0), true) - 2.940).abs()
                < 1e-6
        );
        assert!(
            (measured_first_line_top_delta_pt(Some("Heiti SC"), Some(12.0), true) - 3.624).abs()
                < 1e-6
        );
        assert!(
            (measured_first_line_top_delta_pt(Some("Arial"), Some(12.0), false) - 2.684).abs()
                < 1e-6
        );
        assert!(
            (measured_first_line_top_delta_pt(Some("Times New Roman"), Some(24.0), false) - 6.670)
                .abs()
                < 1e-6
        );
    }
}
