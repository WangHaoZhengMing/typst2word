//! Unit conversions from Typst points to OpenXML units.

/// Convert points (`pt`, 1/72 inch) to DrawingML EMUs (`1 pt = 12700 EMU`).
#[inline]
pub fn pt_to_emu(pt: f64) -> i64 {
    if !pt.is_finite() {
        return 0;
    }
    (pt * 12700.0).round() as i64
}

/// Convert points (`pt`, 1/72 inch) to OpenXML twentieths of a point (`dxa` / twips, `1 pt = 20 dxa`).
#[inline]
pub fn pt_to_dxa(pt: f64) -> i64 {
    if !pt.is_finite() {
        return 0;
    }
    (pt * 20.0).round() as i64
}
