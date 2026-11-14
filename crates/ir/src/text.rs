//! Styled text runs (`TextRunIr`) and script positioning.

use serde::{Deserialize, Serialize};

use super::{ColorHex, LocaleIr, StrokeIr};

/// Check if a character is a CJK ideograph, CJK punctuation, Hiragana/Katakana, or fullwidth glyph.
#[inline]
pub const fn is_cjk_char(c: char) -> bool {
    matches!(
        c as u32,
        0x2E80..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE1F
            | 0xFE30..=0xFE6F
            | 0xFF01..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x2FA1F
    )
}

/// Richly styled text run extracted from Typst.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TextRunIr {
    pub text: String,
    pub font_family: Option<String>,
    pub font_size_pt: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub underline: Option<StrokeIr>,
    pub strike: Option<StrokeIr>,
    pub overline: Option<StrokeIr>,
    pub color: Option<ColorHex>,
    pub highlight: Option<ColorHex>,
    pub border: Option<StrokeIr>,
    pub script: ScriptKindIr,
    pub smallcaps: bool,
    pub is_code: bool,
    /// Inter-character tracking (`#set text(tracking: ...)`) in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracking_pt: Option<f64>,
    /// Vertical baseline shift (`#set text(baseline: ...)`) in points (positive = raised).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_shift_pt: Option<f64>,
    /// True glyph outline stroke (`#set text(stroke: ...)`), distinct from synthetic bold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_outline: Option<StrokeIr>,
    /// Resolved run locale (`TextItem::lang` + `TextItem::region`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<LocaleIr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScriptKindIr {
    #[default]
    Normal,
    Sub,
    Super,
}

/// Intrinsic OpenType/TrueType binary table metrics extracted from a resolved `typst::text::Font`.
///
/// Stores pure font-binary design units (`head.unitsPerEm`, `hhea`, `OS/2`, `GSUB`, `post`) and
/// host OS installation status without any Microsoft Word `dxa` / `w:line` / `1.3x` heuristics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFaceMetricsIr {
    /// `head.unitsPerEm` (design units per em, typically `1000` or `2048`).
    pub upem: u16,
    /// `hhea.ascender` in design units.
    pub hhea_ascender: i16,
    /// `hhea.descender` in design units (negative distance below baseline, e.g. `-340`).
    pub hhea_descender: i16,
    /// `hhea.lineGap` in design units.
    pub hhea_line_gap: i16,
    /// `OS/2.sCapHeight` in design units, when present (`OS/2` version >= 2).
    /// Typst's default `text.top-edge: "cap-height"` anchors line top edges to this exact metric.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s_cap_height: Option<i16>,
    /// `OS/2.sTypoAscender` in design units, when `OS/2` table is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s_typo_ascender: Option<i16>,
    /// `OS/2.sTypoDescender` in design units, when `OS/2` table is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s_typo_descender: Option<i16>,
    /// `OS/2.usWinAscent` in design units, when `OS/2` table is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub us_win_ascent: Option<u16>,
    /// `OS/2.usWinDescent` in design units (unsigned distance below baseline), when `OS/2` is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub us_win_descent: Option<u16>,
    /// Whether `OS/2.ulCodePageRange1` sets any East Asian codepage bit (`17..=20`: `CP932`/`CP936`/`CP949`/`CP950`).
    #[serde(default)]
    pub has_cjk_codepage: bool,
    /// Whether OpenType `GSUB` exposes a `'hani'` script table (`false` for AAT-only macOS CJK fonts).
    #[serde(default)]
    pub has_gsub_hani: bool,
    /// Whether `post.isFixedPitch != 0` (monospaced typeface).
    #[serde(default)]
    pub is_monospaced: bool,
    /// Whether this font family is installed in the host OS font catalog (`true`), meaning
    /// both Typst and Microsoft Word resolve the exact same font binary, vs loaded solely from
    /// Typst's embedded fallback bundle (`false`, substituted by Word via `<w:altName>`).
    #[serde(default)]
    pub is_system_installed: bool,
}

impl FontFaceMetricsIr {
    /// Intrinsic `OS/2.sCapHeight / unitsPerEm` ratio, if `sCapHeight > 0`.
    #[inline]
    pub fn cap_height_ratio(&self) -> Option<f64> {
        self.s_cap_height
            .filter(|&cap| cap > 0)
            .map(|cap| cap as f64 / self.upem.max(1) as f64)
    }

    /// Intrinsic `(hhea.ascender - hhea.descender) / unitsPerEm` span ratio.
    #[inline]
    pub fn hhea_span_ratio(&self) -> f64 {
        (self.hhea_ascender as i32 - self.hhea_descender as i32).max(1) as f64
            / self.upem.max(1) as f64
    }

    /// Intrinsic `(hhea.ascender - hhea.descender + max(hhea.lineGap, 0)) / unitsPerEm` ratio.
    #[inline]
    pub fn hhea_full_ratio(&self) -> f64 {
        (self.hhea_ascender as i32 - self.hhea_descender as i32 + self.hhea_line_gap.max(0) as i32)
            .max(1) as f64
            / self.upem.max(1) as f64
    }

    /// Intrinsic `(OS/2.usWinAscent + OS/2.usWinDescent) / unitsPerEm` ratio, if `OS/2` is present.
    #[inline]
    pub fn win_span_ratio(&self) -> Option<f64> {
        let asc = self.us_win_ascent? as u32;
        let desc = self.us_win_descent? as u32;
        Some((asc + desc) as f64 / self.upem.max(1) as f64)
    }
}
