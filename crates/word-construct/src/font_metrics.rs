//! Microsoft Word Font Metrics, `FontParadigm` State Machine, and Line Box (`WordLineBox`) Spatial Model.
//!
//! ## How Word Computes Single-Line Height (`w:line="240" w:lineRule="auto"`)
//! When `<w:spacing w:line="W" w:lineRule="auto"/>` is used, Microsoft Word does NOT define
//! `1.0x` single-line spacing (`W = 240`) as `1.0 * font_size_pt`. Instead, Word classifies
//! each font into one of three OpenType/GDI metric paradigms ([`FontParadigm`]):
//!
//! 1. **[`FontParadigm::StandardWin`]**: Reads `OS/2` `(usWinAscent + usWinDescent) / unitsPerEm`:
//!    $$H_{\text{single}}^{\text{Word}}(S) = M_{\text{win}} \cdot S$$
//!    - Western proportional fonts (`Times New Roman`, `Libertinus Serif`): $M_{\text{win}} \approx 1.150$, $M_{\text{ink}} \approx 0.720$.
//!    - Modern CJK UI fonts (`PingFang SC`, `Heiti SC`): $M_{\text{win}} = 1.380$, $M_{\text{ink}} \approx 0.950\text{–}0.960$.
//! 2. **[`FontParadigm::LegacyCjkSerif`]**: Traditional CJK serif fonts declaring `CP936` (`SimSun`),
//!    where Word multiplies the `hhea` ratio ($1.000$) by $\gamma_{\text{CJK}} = 1.3 \implies M_{\text{win}} = 1.300$.
//! 3. **[`FontParadigm::AppleCjkQuirk`]**: macOS CJK serif fonts (`Songti SC`, `STSong`, `Kaiti SC`),
//!    whose `hhea` ratio is already $1.400$ and is compounded by $\gamma_{\text{CJK}} = 1.3 \implies M_{\text{win}} = 1.820$,
//!    while AAT-only tables simultaneously trigger `plsrun->script_id = 5` (`Latin` / `'DFLT'`) in `MicrosoftPTLS7`.

use serde::{Deserialize, Serialize};

use crate::units::{Pt, WLine};

/// Default Word CJK single-line magnification factor (`1.3`) for fonts declaring CJK codepages
/// (`CP932` / `CP936` / `CP949` / `CP950`, e.g. `SimSun` / `宋体` where `12pt` single spacing = `15.6pt`).
pub const WORD_CJK_FACTOR: f64 = 1.3;

// ============================================================================
// 1. Font Shaping Workaround & Paradigm State Machine (Enum-Driven)
// ============================================================================

/// Underlying shaping workaround required for Microsoft Word (`MicrosoftPTLS7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapingWorkaround {
    /// Standard OpenType / GDI shaping (`'hani'` or `'latn'`); no script override occurs.
    None,
    /// Apple AAT-only CJK font (`feat`/`morx` without `'hani'` `GSUB`/`GPOS`), which triggers
    /// Word's `0x10044afdc` `plsrun->script_id = 5` (`Latin` / `'DFLT'`) override and collapses
    /// CJK text into `Ptls7BreakClass::LatinWord` (`0`).
    ForceCjkScriptBinding,
}

/// Word's internal line-height and shaping paradigm for a font family.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FontParadigm {
    /// Standard Western proportional and modern CJK UI fonts (e.g. `Times New Roman`, `PingFang SC`).
    /// Directly uses `OS/2` `(usWinAscent + usWinDescent) / unitsPerEm`.
    StandardWin {
        m_win: f64,
        m_ink: f64,
        is_cjk: bool,
        workaround: ShapingWorkaround,
    },

    /// Traditional CJK serif fonts declaring `CP936` etc. (e.g. Windows `SimSun`).
    /// Triggers Word's `1.3x` CJK single-line magnification (`hhea_ratio * 1.3`).
    LegacyCjkSerif {
        hhea_ratio: f64,
        m_ink: f64,
    },

    /// macOS traditional CJK serif fonts (e.g. `Songti SC`, `STSong`, `Kaiti SC`).
    /// Suffers both:
    /// 1. `hhea_ratio (1.40) * 1.3 = 1.820` line-box inflation;
    /// 2. (for AAT-only fonts) `plsrun->script_id = 5` (`Latin` / `'DFLT'`) override in `MicrosoftPTLS7`.
    AppleCjkQuirk {
        hhea_ratio: f64,
        m_ink: f64,
        workaround: ShapingWorkaround,
    },

    /// Fixed-width monospace code fonts (`SF Mono`, `Menlo`, `Consolas`, `Courier New`).
    Monospace {
        m_win: f64,
        m_ink: f64,
    },
}

impl FontParadigm {
    const CJK_SCALE_FACTOR: f64 = WORD_CJK_FACTOR;
    const WORD_LINE_UNIT: f64 = 240.0;

    /// Declaratively derive Word's single-line box height ratio $M_{\text{win}}$.
    #[inline]
    pub const fn m_win(self) -> f64 {
        match self {
            Self::StandardWin { m_win, .. } | Self::Monospace { m_win, .. } => m_win,
            Self::LegacyCjkSerif { hhea_ratio, .. } | Self::AppleCjkQuirk { hhea_ratio, .. } => {
                hhea_ratio * Self::CJK_SCALE_FACTOR
            }
        }
    }

    /// Effective glyph ink height ratio $M_{\text{ink}}^{\text{eff}}$.
    #[inline]
    pub const fn m_ink(self) -> f64 {
        match self {
            Self::StandardWin { m_ink, .. }
            | Self::LegacyCjkSerif { m_ink, .. }
            | Self::AppleCjkQuirk { m_ink, .. }
            | Self::Monospace { m_ink, .. } => m_ink,
        }
    }

    /// Whether this font declares a CJK codepage (`CP932`/`CP936`/`CP949`/`CP950`).
    #[inline]
    pub const fn is_cjk(self) -> bool {
        match self {
            Self::StandardWin { is_cjk, .. } => is_cjk,
            Self::LegacyCjkSerif { .. } | Self::AppleCjkQuirk { .. } => true,
            Self::Monospace { .. } => false,
        }
    }

    /// Derive whether this font triggers Word's `0x10044afdc` `'DFLT'` (`script_id = 5`) override.
    #[inline]
    pub const fn shaping_workaround(self) -> ShapingWorkaround {
        match self {
            Self::StandardWin { workaround, .. } | Self::AppleCjkQuirk { workaround, .. } => {
                workaround
            }
            Self::LegacyCjkSerif { .. } | Self::Monospace { .. } => ShapingWorkaround::None,
        }
    }

    /// Inverse mapping formula: convert Typst's `par(leading: L)` into Word's `w:line` ([`WLine`]).
    ///
    /// $$W = \text{round}\left( 240 \cdot \frac{M_{\text{ink}}^{\text{eff}} + L / S}{M_{\text{win}}} \right)$$
    #[inline]
    pub fn resolve_word_line(self, font_size: Pt, typst_leading: Pt) -> WLine {
        let leading_ratio = typst_leading.0 / font_size.0.max(1.0);
        let target_total_ratio = self.m_ink() + leading_ratio;
        let raw_w = Self::WORD_LINE_UNIT * (target_total_ratio / self.m_win());
        WLine(raw_w.round() as i64)
    }
}

// ============================================================================
// 2. Raw OpenType Table Feature Classifier & Compile-Time Known Font Table
// ============================================================================

/// Minimal OpenType table feature set extracted from a TTF/OTF/TTC font binary.
#[derive(Debug, Clone, Copy)]
pub struct RawFontMetrics<'a> {
    pub family_name: &'a str,
    pub upm: u16,
    pub win_ascent: u16,
    pub win_descent: u16,
    pub hhea_ascent: i16,
    pub hhea_descent: i16,
    /// `OS/2` `ulCodePageRange1` bits `17..=20` (`CP932` / `CP936` / `CP949` / `CP950`).
    pub has_cjk_codepage: bool,
    pub is_serif: bool,
    pub is_monospace: bool,
    /// Whether the font contains a valid OpenType `'hani'` script table in `GSUB`/`GPOS`.
    pub has_valid_gsub_hani: bool,
}

impl<'a> RawFontMetrics<'a> {
    #[inline]
    pub fn win_ratio(&self) -> f64 {
        (self.win_ascent + self.win_descent) as f64 / self.upm.max(1) as f64
    }

    #[inline]
    pub fn hhea_ratio(&self) -> f64 {
        (self.hhea_ascent as i32 - self.hhea_descent as i32) as f64 / self.upm.max(1) as f64
    }
}

/// Declarative macro building the compile-time known font lookup table (exact + substring match).
macro_rules! define_known_fonts {
    (
        exact: {
            $(
                $($exact_name:literal)|+ => $exact_variant:ident { $( $efield:ident : $eval:expr ),* $(,)? }
            ),* $(,)?
        },
        contains: {
            $(
                $($sub_pat:literal)|+ => $sub_variant:ident { $( $sfield:ident : $sval:expr ),* $(,)? }
            ),* $(,)?
        }
    ) => {
        /// Look up a font family name in the compile-time calibrated font table.
        pub fn lookup_known_font(family: &str) -> Option<FontParadigm> {
            let normalized = family.trim().to_ascii_lowercase();
            match normalized.as_str() {
                $(
                    $($exact_name)|+ => Some(FontParadigm::$exact_variant { $( $efield: $eval ),* }),
                )*
                s => match () {
                    $(
                        _ if $( s.contains($sub_pat) )||+ => {
                            Some(FontParadigm::$sub_variant { $( $sfield: $sval ),* })
                        }
                    )*
                    _ => None,
                },
            }
        }
    };
}

define_known_fonts! {
    exact: {
        "times new roman" | "libertinus serif" | "arial" | "calibri" | "cambria" | "cambria math"
            => StandardWin { m_win: 1.150, m_ink: 0.720, is_cjk: false, workaround: ShapingWorkaround::None },
        "pingfang sc" | "pingfang tc" | "pingfang hk" | "simhei" | "microsoft yahei" | "黑体"
            => StandardWin { m_win: 1.380, m_ink: 0.950, is_cjk: true, workaround: ShapingWorkaround::None },
        "heiti sc" | "heiti tc" | "stheiti"
            => StandardWin { m_win: 1.380, m_ink: 0.950, is_cjk: true, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "simsun" | "nsimsun" | "mingliu" | "pmingliu" | "ms mincho" | "batang"
            => LegacyCjkSerif { hhea_ratio: 1.000, m_ink: 0.850 },
        "songti sc" | "songti tc" | "songti" | "stsong" | "kaiti sc" | "kaiti tc" | "stkaiti" | "stfangsong"
            | "baoli sc" | "libian sc" | "wawati sc" | "weibei sc" | "xingkai sc" | "yuanti sc"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.720, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "宋体" | "楷体" | "仿宋" | "kaiti" | "fangsong"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.720, workaround: ShapingWorkaround::None },
        "sf mono" | "menlo" | "consolas" | "courier new" | "courier"
            => Monospace { m_win: 1.180, m_ink: 0.750 },
    },
    contains: {
        "songti" | "stsong" | "kaiti sc" | "kaiti tc" | "stkaiti" | "stfangsong"
            | "baoli sc" | "libian sc" | "wawati sc" | "weibei sc" | "xingkai sc" | "yuanti sc"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.720, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "宋体" | "楷体" | "仿宋" | "kaiti" | "fangsong"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.720, workaround: ShapingWorkaround::None },
        "heiti sc" | "heiti tc" | "stheiti"
            => StandardWin { m_win: 1.380, m_ink: 0.950, is_cjk: true, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "pingfang" | "heiti" | "黑体" | "simhei" | "yahei" | "gothic"
            => StandardWin { m_win: 1.380, m_ink: 0.950, is_cjk: true, workaround: ShapingWorkaround::None },
        "simsun" | "nsimsun" | "mingliu" | "mincho" | "batang"
            => LegacyCjkSerif { hhea_ratio: 1.000, m_ink: 0.850 },
        "mono" | "menlo" | "consolas" | "courier"
            => Monospace { m_win: 1.180, m_ink: 0.750 },
    }
}

/// Convert raw OpenType table metrics into a [`FontParadigm`] using pure pattern matching.
impl<'a> From<&RawFontMetrics<'a>> for FontParadigm {
    fn from(raw: &RawFontMetrics<'a>) -> Self {
        lookup_known_font(raw.family_name).unwrap_or_else(|| {
            match (
                raw.is_monospace,
                raw.has_cjk_codepage,
                raw.is_serif,
                raw.has_valid_gsub_hani,
                raw.hhea_ratio(),
            ) {
                (true, false, _, _, _) => Self::Monospace {
                    m_win: raw.win_ratio(),
                    m_ink: 0.750,
                },
                (false, true, true, false, hhea) if hhea >= 1.30 => Self::AppleCjkQuirk {
                    hhea_ratio: hhea,
                    m_ink: 0.720,
                    workaround: ShapingWorkaround::ForceCjkScriptBinding,
                },
                (false, true, true, _, hhea) => Self::LegacyCjkSerif {
                    hhea_ratio: hhea,
                    m_ink: 0.850,
                },
                (_, has_cjk, _, _, _) => Self::StandardWin {
                    m_win: raw.win_ratio(),
                    m_ink: match has_cjk {
                        true => 0.950,
                        false => 0.720,
                    },
                    is_cjk: has_cjk,
                    workaround: ShapingWorkaround::None,
                },
            }
        })
    }
}

// ============================================================================
// 3. Word Line-Box Spec & Font Metric Classification
// ============================================================================

/// Declarative Word line-box layout specification computed from a [`FontParadigm`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordLineBoxSpec {
    pub font_family: String,
    pub w_line: WLine,
    pub line_height: Pt,
    pub delta_top: Pt,
    pub delta_bot: Pt,
    pub workaround: ShapingWorkaround,
}

impl WordLineBoxSpec {
    /// Compute Word line-box geometry and shaping workaround from a [`FontParadigm`].
    pub fn compute(family: &str, paradigm: FontParadigm, size: Pt, leading: Pt) -> Self {
        let w_line = paradigm.resolve_word_line(size, leading);
        let line_height = size * (paradigm.m_win() * (w_line.0 as f64 / 240.0));
        let ink_height = size * paradigm.m_ink();

        let delta_internal = (line_height.0 - ink_height.0).max(0.0);
        let delta_top = (0.06 * size.0).min(0.25 * delta_internal);
        let delta_bot = delta_internal - delta_top;

        Self {
            font_family: family.to_string(),
            w_line,
            line_height,
            delta_top: Pt(delta_top),
            delta_bot: Pt(delta_bot),
            workaround: paradigm.shaping_workaround(),
        }
    }
}

/// Font metric family classification for Word line-box computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordFontClass {
    /// Modern CJK UI fonts (`PingFang SC`, `Heiti SC`, `SimHei`, `Microsoft YaHei`).
    /// `OS/2` `(usWinAscent + usWinDescent) / UPEM = 1.380`, glyph ink height ratio `0.960`.
    Cjk,
    /// Traditional serif CJK fonts (`Songti SC`, `STSong`, `Kaiti SC`, `FangSong`) whose
    /// `hhea` ratio is `1.400` and which declare legacy CJK codepages (`CP936`), causing Word
    /// to scale their single-line (`w:line="240"`) box to `1.400 * 1.3 = 1.820 * S`.
    CjkSongti,
    /// Proportional Western serif/sans-serif fonts (`Libertinus Serif`, `Times New Roman`, `Arial`, `Calibri`).
    /// `OS/2` `(usWinAscent + usWinDescent) / UPEM = 1.150`, glyph ink height ratio `0.720`.
    Latin,
    /// Monospace code fonts (`SF Mono`, `Menlo`, `Consolas`, `Courier New`).
    /// `OS/2` `(usWinAscent + usWinDescent) / UPEM = 1.180`, glyph ink height ratio `0.750`.
    Monospace,
}

impl WordFontClass {
    /// Classify a font family name and CJK character presence via [`lookup_known_font`].
    #[inline]
    pub fn classify(font_family: Option<&str>, has_cjk_chars: bool) -> Self {
        match (font_family.and_then(lookup_known_font), has_cjk_chars) {
            (Some(FontParadigm::AppleCjkQuirk { .. }), _) => Self::CjkSongti,
            (
                Some(
                    FontParadigm::LegacyCjkSerif { .. }
                    | FontParadigm::StandardWin { is_cjk: true, .. },
                ),
                _,
            ) => Self::Cjk,
            (Some(FontParadigm::Monospace { .. }), false) => Self::Monospace,
            (_, true) => Self::Cjk,
            _ => Self::Latin,
        }
    }

    /// `(usWinAscent + usWinDescent) / unitsPerEm` ratio used by Word for `w:line="240"`.
    #[inline]
    pub const fn win_box_ratio(self, has_cjk_chars: bool) -> f64 {
        match (self, has_cjk_chars) {
            (Self::CjkSongti, true) => 1.820,
            (Self::CjkSongti, false) => 1.400,
            (Self::Cjk, true) => 1.380,
            (Self::Cjk | Self::Latin, false) | (Self::Latin, true) => 1.150,
            (Self::Monospace, _) => 1.180,
        }
    }

    /// Visible glyph ink height ratio ($H_{\text{ink}} / S$).
    #[inline]
    pub const fn ink_height_ratio(self, has_cjk_chars: bool) -> f64 {
        match (self, has_cjk_chars) {
            (Self::CjkSongti | Self::Cjk, true) => 0.960,
            (Self::CjkSongti | Self::Cjk, false) => 0.760,
            (Self::Latin, _) => 0.720,
            (Self::Monospace, _) => 0.750,
        }
    }
}

/// Inline content profile of a paragraph for Word line-box calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphInlineProfile {
    /// Paragraph contains ONLY an inline equation (`<m:oMath>`), no text runs.
    OnlyInlineEquation,
    /// Paragraph contains text runs mixed with an inline equation (`<m:oMath>`).
    TextWithInlineEquation,
    /// Standard text paragraph without inline equations.
    StandardText,
}

impl ParagraphInlineProfile {
    #[inline]
    pub const fn from_flags(has_inline_eq: bool, only_inline_eq: bool) -> Self {
        match (only_inline_eq, has_inline_eq) {
            (true, _) => Self::OnlyInlineEquation,
            (false, true) => Self::TextWithInlineEquation,
            (false, false) => Self::StandardText,
        }
    }
}

/// Exact spatial model of a single line box in Microsoft Word.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WordLineBox {
    /// Font size $S$ in points (`pt`).
    pub font_size_pt: f64,
    /// Word `w:line` value in 240ths of a single line (`w:lineRule="auto"`).
    pub w_line_240: i64,
    /// Total height of the line box in Word ($H_{\text{line}}^{\text{Word}}$ in `pt`).
    pub line_box_height_pt: f64,
    /// Visible glyph ink height ($H_{\text{ink}}$ in `pt`).
    pub ink_height_pt: f64,
    /// Total internal empty space inside the line box ($\Delta_{\text{internal}} = H_{\text{line}} - H_{\text{ink}}$ in `pt`).
    pub internal_leading_pt: f64,
    /// Empty space above glyph ink inside the line box ($\delta_{\text{top}}$ in `pt`).
    pub top_internal_pt: f64,
    /// Empty space below glyph ink inside the line box ($\delta_{\text{bot}}$ in `pt`).
    pub bottom_internal_pt: f64,
}

impl WordLineBox {
    /// Construct a `WordLineBox` that matches a target Typst `leading_pt` (`par.leading`).
    pub fn from_typst_metrics(
        font_family: Option<&str>,
        has_cjk_chars: bool,
        has_inline_eq: bool,
        only_inline_eq: bool,
        font_size_pt: f64,
        leading_pt: Option<f64>,
    ) -> Self {
        let s = font_size_pt.max(6.0);
        let class = WordFontClass::classify(font_family, has_cjk_chars);
        let is_cjk_font_or_text =
            matches!(class, WordFontClass::Cjk | WordFontClass::CjkSongti) || has_cjk_chars;
        let inline_profile = ParagraphInlineProfile::from_flags(has_inline_eq, only_inline_eq);
        let leading = leading_pt.map(|ls| (ls, (ls / s).clamp(0.1, 2.0)));

        let w_line_240 = match (inline_profile, class, is_cjk_font_or_text, leading) {
            (ParagraphInlineProfile::OnlyInlineEquation, _, _, _) => 276,
            (ParagraphInlineProfile::TextWithInlineEquation, _, true, Some((ls_pt, _)))
                if ls_pt > 3.5 && s >= 14.0 =>
            {
                240
            }
            (_, WordFontClass::CjkSongti, true, Some((ls_pt, ratio))) if ls_pt > 3.5 => {
                let metric_div = class.win_box_ratio(true);
                ((240.0 * (0.692 + ratio) / metric_div).round() as i64).clamp(174, 720)
            }
            (_, _, true, Some((ls_pt, ratio))) if ls_pt > 3.5 => {
                let metric_div = class.win_box_ratio(has_cjk_chars);
                let ink_em = match s < 10.0 {
                    true => 0.90,
                    false => 0.95,
                };
                ((240.0 * (ink_em + ratio) / metric_div).round() as i64).clamp(240, 720)
            }
            (_, _, false, Some((ls_pt, ratio))) if ls_pt > 3.5 => {
                let fam_lower = font_family.unwrap_or("").trim().to_ascii_lowercase();
                let (cap_em, metric_div) = if fam_lower.contains("arial")
                    || fam_lower.contains("helvetica")
                    || fam_lower.contains("calibri")
                {
                    (0.7163, 1.150)
                } else if fam_lower.contains("georgia") {
                    (0.6929, 1.1362)
                } else if fam_lower.contains("courier") {
                    (0.5713, 1.1328)
                } else {
                    (0.6621, class.win_box_ratio(false))
                };
                ((240.0 * (cap_em + ratio) / metric_div).round() as i64).clamp(216, 720)
            }
            (_, WordFontClass::CjkSongti, _, Some((_, ratio))) if ratio <= 1.05 => {
                ((212.0 + ratio * 24.0) * (1.380 / 1.820)).round() as i64
            }
            (_, _, _, Some((_, ratio))) if ratio <= 1.05 => (212.0 + ratio * 24.0).round() as i64,
            (_, _, _, Some((_, ratio))) => {
                ((236.0 * (1.0 + ratio) / 2.0).round() as i64).clamp(180, 720)
            }
            (_, WordFontClass::CjkSongti, _, None) => 177,
            (_, _, true, None) => 276,
            (_, _, false, None) => 242,
        };

        Self::with_explicit_w_line(font_family, has_cjk_chars, s, w_line_240)
    }

    /// Construct a `WordLineBox` with an explicit `w:line` value (`w_line_240`).
    pub fn with_explicit_w_line(
        font_family: Option<&str>,
        has_cjk_chars: bool,
        font_size_pt: f64,
        w_line_240: i64,
    ) -> Self {
        let s = font_size_pt.max(6.0);
        let class = WordFontClass::classify(font_family, has_cjk_chars);
        let win_ratio = class.win_box_ratio(has_cjk_chars);
        let ink_ratio = class.ink_height_ratio(has_cjk_chars);

        let line_box_height_pt = win_ratio * s * (w_line_240 as f64 / 240.0);
        let ink_height_pt = ink_ratio * s;
        let internal_leading_pt = (line_box_height_pt - ink_height_pt).max(0.0);
        let top_internal_pt = (0.06 * s).min(internal_leading_pt * 0.25);
        let bottom_internal_pt = (internal_leading_pt - top_internal_pt).max(0.0);

        Self {
            font_size_pt: s,
            w_line_240,
            line_box_height_pt,
            ink_height_pt,
            internal_leading_pt,
            top_internal_pt,
            bottom_internal_pt,
        }
    }
}

// ============================================================================
// 4. Font Metrics Provider Trait & Default Implementation
// ============================================================================

/// Provider interface for font line-height and character advance metrics (injected outside IR).
pub trait FontMetricsProvider {
    /// Compute Word-compatible single-line height (`w:line="240"`) for `font_family` at `font_size_pt`.
    fn single_line_height_pt(&self, font_family: Option<&str>, font_size_pt: f64) -> f64;

    /// Compute baseline-to-top ascent (including half of Word's CJK external leading expansion).
    fn ascent_pt(&self, font_family: Option<&str>, font_size_pt: f64) -> f64;

    /// Compute baseline-to-bottom descent (including half of Word's CJK external leading expansion).
    fn descent_pt(&self, font_family: Option<&str>, font_size_pt: f64) -> f64;

    /// Estimate horizontal advance width of `ch` at `font_size_pt`.
    fn char_advance_pt(&self, font_family: Option<&str>, font_size_pt: f64, ch: char) -> f64;
}

/// Default Word-compatible font metrics provider implementing Windows `OS/2` (`usWinAscent + usWinDescent`)
/// and CJK codepage (`CP936`/`CP932`/`CP949`/`CP950`) `word_cjk_factor = 1.3` line height scaling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WordFontMetricsProvider {
    /// CJK codepage line-height factor (`1.3` by default -> `12pt` SimSun = `15.6pt`).
    pub word_cjk_factor: f64,
    /// Western proportional font single-line factor (`1.15` by default).
    pub latin_factor: f64,
    /// Monospace font single-line factor (`1.18` by default).
    pub monospace_factor: f64,
}

impl Default for WordFontMetricsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl WordFontMetricsProvider {
    #[inline]
    pub const fn new() -> Self {
        Self {
            word_cjk_factor: WORD_CJK_FACTOR,
            latin_factor: 1.15,
            monospace_factor: 1.18,
        }
    }

    #[inline]
    pub const fn with_cjk_factor(mut self, factor: f64) -> Self {
        self.word_cjk_factor = factor;
        self
    }

    #[inline]
    pub const fn with_latin_factor(mut self, factor: f64) -> Self {
        self.latin_factor = factor;
        self
    }

    /// Check whether `font_family` declares a CJK codepage (`CP932`/`CP936`/`CP949`/`CP950`).
    #[inline]
    pub fn is_cjk_codepage_font(font_family: Option<&str>) -> bool {
        font_family
            .and_then(lookup_known_font)
            .is_some_and(FontParadigm::is_cjk)
    }
}

impl FontMetricsProvider for WordFontMetricsProvider {
    fn single_line_height_pt(&self, font_family: Option<&str>, font_size_pt: f64) -> f64 {
        let s = font_size_pt.max(1.0);
        match (
            Self::is_cjk_codepage_font(font_family),
            WordFontClass::classify(font_family, false),
        ) {
            (true, _) | (false, WordFontClass::Cjk | WordFontClass::CjkSongti) => {
                s * self.word_cjk_factor
            }
            (false, WordFontClass::Monospace) => s * self.monospace_factor,
            (false, WordFontClass::Latin) => s * self.latin_factor,
        }
    }

    fn ascent_pt(&self, font_family: Option<&str>, font_size_pt: f64) -> f64 {
        let s = font_size_pt.max(1.0);
        let total = self.single_line_height_pt(font_family, s);
        let extra = (total - s).max(0.0);
        0.80 * s + 0.5 * extra
    }

    fn descent_pt(&self, font_family: Option<&str>, font_size_pt: f64) -> f64 {
        let s = font_size_pt.max(1.0);
        let total = self.single_line_height_pt(font_family, s);
        (total - self.ascent_pt(font_family, s)).max(0.0)
    }

    fn char_advance_pt(&self, font_family: Option<&str>, font_size_pt: f64, ch: char) -> f64 {
        let s = font_size_pt.max(1.0);
        let class = WordFontClass::classify(font_family, false);
        match (class, ch) {
            (_, c) if is_cjk_or_fullwidth_char(c) || matches!(c, '—' | '…') => s,
            // CoreText `Songti SC` (`/System/Library/Fonts/Supplemental/Songti.ttc`, UPEM = 1000)
            // glyph advance ratios when shaped by Word (`0x10044afdc` `'DFLT'` / `script_id = 5`):
            (WordFontClass::CjkSongti, c) if c.is_whitespace() => 0.250 * s,
            (WordFontClass::CjkSongti, '0'..='9') => 0.469 * s,
            (WordFontClass::CjkSongti, '.') => 0.219 * s,
            (WordFontClass::CjkSongti, '%') => 0.823 * s,
            (WordFontClass::CjkSongti, '“' | '”' | '‘' | '’') => 0.400 * s,
            (WordFontClass::CjkSongti, c) if c.is_ascii_uppercase() => 0.703 * s,
            (WordFontClass::CjkSongti, c) if c.is_ascii_lowercase() => 0.430 * s,
            (WordFontClass::CjkSongti, c) if c.is_ascii_punctuation() => 0.330 * s,
            (WordFontClass::CjkSongti, _) => 0.500 * s,
            (_, c) if c.is_whitespace() => 0.280 * s,
            (WordFontClass::Monospace, _) => 0.600 * s,
            (_, c) if c.is_ascii_uppercase() => 0.620 * s,
            (_, c) if c.is_ascii_digit() => 0.550 * s,
            (_, c) if c.is_ascii_punctuation() => 0.330 * s,
            _ => 0.500 * s,
        }
    }
}

/// Check if a single character is a CJK ideograph, CJK symbol, or fullwidth glyph.
#[inline]
pub fn is_cjk_or_fullwidth_char(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{3000}'..='\u{303F}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{FF00}'..='\u{FFEF}'
    )
}

/// Helper to check if a text string contains any CJK ideographs or fullwidth punctuation.
#[inline]
pub fn str_has_cjk_chars(text: &str) -> bool {
    text.chars().any(is_cjk_or_fullwidth_char)
}
