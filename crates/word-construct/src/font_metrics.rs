//! Microsoft Word Font Metrics, `FontParadigm` State Machine, and Line Box (`WordLineBox`) Spatial Model.
//!
//! ## How Word Computes Single-Line Height (`w:line="240" w:lineRule="auto"`)
//! When `<w:spacing w:line="W" w:lineRule="auto"/>` is used, Microsoft Word does NOT define
//! `1.0x` single-line spacing (`W = 240`) as `1.0 * font_size_pt`. Instead, Word classifies
//! each font into one of four OpenType/GDI metric paradigms ([`FontParadigm`]):
//!
//! 1. **[`FontParadigm::StandardWin`]**: Western proportional fonts without CJK codepages
//!    (`Times New Roman`, `Libertinus Serif`, `Arial`, `Calibri`):
//!    $$H_{\text{single}}^{\text{Word}}(S) = M_{\text{win}} \cdot S \approx 1.150 \cdot S$$
//! 2. **[`FontParadigm::CjkUiSans`]**: Modern CJK UI sans-serif fonts declaring `CP936`/`CP950`
//!    (`PingFang SC` with `hhea = 1.400`, `Heiti SC` with `hhea = 1.000`, `SimHei` / `Microsoft YaHei`
//!    with `hhea = 1.059`), where Word multiplies the `hhea` span by $\gamma_{\text{CJK}} = 1.3$:
//!    - `PingFang SC`: $1.400 \times 1.3 = 1.820$ ($20.02\text{pt}$ at $11\text{pt}$), $sCapHeight = 0.860$.
//!    - `Heiti SC`: $1.000 \times 1.3 = 1.300$, $sCapHeight = 0.718$.
//! 3. **[`FontParadigm::LegacyCjkSerif`]**: Traditional Windows CJK serif fonts declaring `CP936`
//!    (`SimSun`), where Word multiplies `hhea = 1.000` by $\gamma_{\text{CJK}} = 1.3 \implies M_{\text{win}} = 1.300$.
//! 4. **[`FontParadigm::AppleCjkQuirk`]**: macOS CJK serif fonts (`Songti SC`, `STSong`, `Kaiti SC`),
//!    whose `hhea = 1.400` is compounded by $\gamma_{\text{CJK}} = 1.3 \implies M_{\text{win}} = 1.820$,
//!    while AAT-only tables simultaneously trigger `plsrun->script_id = 5` (`Latin` / `'DFLT'`) in `MicrosoftPTLS7`.

use ir::{FontFaceMetricsIr, ParagraphLineMetricsIr};
use serde::{Deserialize, Serialize};

use crate::units::{Pt, WLine};

/// Default Word CJK single-line magnification factor (`1.3`) for fonts declaring CJK codepages
/// (`CP932` / `CP936` / `CP949` / `CP950`, e.g. `SimSun` `1.00 * 1.3 = 1.30` and `PingFang SC` `1.40 * 1.3 = 1.82`).
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
    /// Standard Western proportional fonts without CJK codepage bits (e.g. `Times New Roman`, `Arial`).
    /// Directly uses `hhea` / `OS/2` single-line ratio without `1.3x` CJK expansion.
    StandardWin {
        m_win: f64,
        m_ink: f64,
        is_cjk: bool,
        workaround: ShapingWorkaround,
    },

    /// Modern CJK UI sans-serif fonts declaring `CP932`/`CP936`/`CP949`/`CP950`
    /// (e.g. `PingFang SC` `hhea = 1.400`, `Heiti SC` `hhea = 1.000`, `Microsoft YaHei` `hhea = 1.059`).
    /// Because `OS/2.ulCodePageRange1` sets CJK codepage bits, Word multiplies `hhea_ratio` by `1.3`.
    CjkUiSans {
        hhea_ratio: f64,
        m_ink: f64,
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
            Self::CjkUiSans { hhea_ratio, .. }
            | Self::LegacyCjkSerif { hhea_ratio, .. }
            | Self::AppleCjkQuirk { hhea_ratio, .. } => hhea_ratio * Self::CJK_SCALE_FACTOR,
        }
    }

    /// Typst default `top-edge: "cap-height"` ratio $M_{\text{cap}} = \text{sCapHeight} / \text{upem}$.
    #[inline]
    pub const fn m_ink(self) -> f64 {
        match self {
            Self::StandardWin { m_ink, .. }
            | Self::CjkUiSans { m_ink, .. }
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
            Self::CjkUiSans { .. } | Self::LegacyCjkSerif { .. } | Self::AppleCjkQuirk { .. } => {
                true
            }
            Self::Monospace { .. } => false,
        }
    }

    /// Derive whether this font triggers Word's `0x10044afdc` `'DFLT'` (`script_id = 5`) override.
    #[inline]
    pub const fn shaping_workaround(self) -> ShapingWorkaround {
        match self {
            Self::StandardWin { workaround, .. }
            | Self::CjkUiSans { workaround, .. }
            | Self::AppleCjkQuirk { workaround, .. } => workaround,
            Self::LegacyCjkSerif { .. } | Self::Monospace { .. } => ShapingWorkaround::None,
        }
    }

    /// Inverse mapping formula: convert Typst's `par(leading: L)` into Word's `w:line` ([`WLine`]).
    ///
    /// In Typst (`top-edge: "cap-height"`, `bottom-edge: "baseline"`), consecutive baselines step by
    /// $\Delta Y = S \cdot M_{\text{cap}} + L$. In Word (`w:lineRule="auto"`), baselines step by
    /// $\Delta Y_{\text{Word}} = S \cdot M_{\text{win}} \cdot (W / 240)$. Solving $\Delta Y_{\text{Word}} = \Delta Y$:
    /// $$W = \text{round}\left( 240 \cdot \frac{M_{\text{cap}} + L / S}{M_{\text{win}}} \right)$$
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
        "times new roman" | "libertinus serif" | "new computer modern" | "cambria" | "cambria math"
            => StandardWin { m_win: 1.150, m_ink: 0.6621, is_cjk: false, workaround: ShapingWorkaround::None },
        "arial" | "calibri" | "helvetica" | "helvetica neue"
            => StandardWin { m_win: 1.150, m_ink: 0.7163, is_cjk: false, workaround: ShapingWorkaround::None },
        "georgia"
            => StandardWin { m_win: 1.1362, m_ink: 0.6929, is_cjk: false, workaround: ShapingWorkaround::None },
        "pingfang sc" | "pingfang tc" | "pingfang hk"
            => CjkUiSans { hhea_ratio: 1.400, m_ink: 0.860, workaround: ShapingWorkaround::None },
        "heiti sc" | "heiti tc" | "stheiti"
            => CjkUiSans { hhea_ratio: 1.000, m_ink: 0.718, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "simhei" | "microsoft yahei" | "黑体" | "dengxian" | "等线"
            => CjkUiSans { hhea_ratio: 1.059, m_ink: 0.800, workaround: ShapingWorkaround::None },
        "simsun" | "nsimsun" | "mingliu" | "pmingliu" | "ms mincho" | "batang"
            => LegacyCjkSerif { hhea_ratio: 1.000, m_ink: 0.850 },
        "songti sc" | "songti tc" | "songti" | "stsong" | "kaiti sc" | "kaiti tc" | "stkaiti" | "stfangsong"
            | "baoli sc" | "libian sc" | "wawati sc" | "weibei sc" | "xingkai sc" | "yuanti sc"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.695, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "宋体" | "楷体" | "仿宋" | "kaiti" | "fangsong"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.695, workaround: ShapingWorkaround::None },
        "courier new" | "courier" | "dejavu sans mono"
            => Monospace { m_win: 1.1328, m_ink: 0.5713 },
        "sf mono" | "menlo" | "consolas"
            => Monospace { m_win: 1.180, m_ink: 0.720 },
    },
    contains: {
        "songti" | "stsong" | "kaiti sc" | "kaiti tc" | "stkaiti" | "stfangsong"
            | "baoli sc" | "libian sc" | "wawati sc" | "weibei sc" | "xingkai sc" | "yuanti sc"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.695, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "宋体" | "楷体" | "仿宋" | "kaiti" | "fangsong"
            => AppleCjkQuirk { hhea_ratio: 1.400, m_ink: 0.695, workaround: ShapingWorkaround::None },
        "pingfang"
            => CjkUiSans { hhea_ratio: 1.400, m_ink: 0.860, workaround: ShapingWorkaround::None },
        "heiti sc" | "heiti tc" | "stheiti"
            => CjkUiSans { hhea_ratio: 1.000, m_ink: 0.718, workaround: ShapingWorkaround::ForceCjkScriptBinding },
        "heiti" | "黑体" | "simhei" | "yahei" | "gothic"
            => CjkUiSans { hhea_ratio: 1.059, m_ink: 0.800, workaround: ShapingWorkaround::None },
        "simsun" | "nsimsun" | "mingliu" | "mincho" | "batang"
            => LegacyCjkSerif { hhea_ratio: 1.000, m_ink: 0.850 },
        "courier" | "dejavu sans mono"
            => Monospace { m_win: 1.1328, m_ink: 0.5713 },
        "mono" | "menlo" | "consolas"
            => Monospace { m_win: 1.180, m_ink: 0.720 },
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
                    m_ink: 0.720,
                },
                (false, true, true, false, hhea) if hhea >= 1.30 => Self::AppleCjkQuirk {
                    hhea_ratio: hhea,
                    m_ink: 0.695,
                    workaround: ShapingWorkaround::ForceCjkScriptBinding,
                },
                (false, true, true, _, hhea) => Self::LegacyCjkSerif {
                    hhea_ratio: hhea,
                    m_ink: 0.850,
                },
                (false, true, false, has_hani, hhea) => Self::CjkUiSans {
                    hhea_ratio: hhea,
                    m_ink: 0.860,
                    workaround: if has_hani {
                        ShapingWorkaround::None
                    } else {
                        ShapingWorkaround::ForceCjkScriptBinding
                    },
                },
                (_, has_cjk, _, _, _) => Self::StandardWin {
                    m_win: raw.win_ratio(),
                    m_ink: match has_cjk {
                        true => 0.860,
                        false => 0.6621,
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
    Cjk,
    /// Traditional serif CJK fonts (`Songti SC`, `STSong`, `Kaiti SC`, `FangSong`) whose
    /// `hhea` ratio is `1.400` and which declare legacy CJK codepages (`CP936`), causing Word
    /// to scale their single-line (`w:line="240"`) box to `1.400 * 1.3 = 1.820 * S`.
    CjkSongti,
    /// Proportional Western serif/sans-serif fonts (`Libertinus Serif`, `Times New Roman`, `Arial`, `Calibri`).
    Latin,
    /// Monospace code fonts (`SF Mono`, `Menlo`, `Consolas`, `Courier New`).
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
                    FontParadigm::CjkUiSans { .. }
                    | FontParadigm::LegacyCjkSerif { .. }
                    | FontParadigm::StandardWin { is_cjk: true, .. },
                ),
                _,
            ) => Self::Cjk,
            (Some(FontParadigm::Monospace { .. }), false) => Self::Monospace,
            (_, true) => Self::Cjk,
            _ => Self::Latin,
        }
    }

    /// Fallback single-line box ratio (`w:line="240"`) when no binary `FontFaceMetricsIr` or specific
    /// `FontParadigm` overrides it.
    #[inline]
    pub const fn win_box_ratio(self, has_cjk_chars: bool) -> f64 {
        match (self, has_cjk_chars) {
            (Self::CjkSongti, true) => 1.820,
            (Self::CjkSongti, false) => 1.400,
            (Self::Cjk, true) => 1.820,
            (Self::Cjk | Self::Latin, false) | (Self::Latin, true) => 1.150,
            (Self::Monospace, _) => 1.180,
        }
    }

    /// Visible glyph cap/ink height ratio ($H_{\text{ink}} / S$).
    #[inline]
    pub const fn ink_height_ratio(self, has_cjk_chars: bool) -> f64 {
        match (self, has_cjk_chars) {
            (Self::CjkSongti, true) => 0.695,
            (Self::Cjk, true) => 0.860,
            (Self::CjkSongti | Self::Cjk, false) => 0.760,
            (Self::Latin, _) => 0.6621,
            (Self::Monospace, _) => 0.720,
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
    /// Resolve `(cap_height_ratio, word_single_line_ratio)` for a font family, combining
    /// OpenType binary metrics (`FontFaceMetricsIr`) with Word's CJK `1.3x` expansion rule and
    /// fallback `fontTable.xml` substitution (`is_system_installed == false`).
    fn resolve_cap_and_word_ratios(
        font_family: Option<&str>,
        font_face: Option<&FontFaceMetricsIr>,
        has_cjk_chars: bool,
    ) -> (f64, f64) {
        let resolved_alias = font_family.map(crate::pipeline::font_recognize::resolve_font_alias_for_metrics);
        let paradigm = resolved_alias.and_then(lookup_known_font);
        let class = WordFontClass::classify(resolved_alias, has_cjk_chars);

        // 1. Typst's `top-edge: "cap-height"` ratio: prefer exact binary `OS/2.sCapHeight / upem`
        //    from the actual font Typst shaped (`FontFaceMetricsIr`), falling back to calibrated table.
        let cap_ratio = font_face
            .and_then(FontFaceMetricsIr::cap_height_ratio)
            .or_else(|| paradigm.map(FontParadigm::m_ink))
            .unwrap_or_else(|| class.ink_height_ratio(has_cjk_chars));

        // 2. Word's `w:line="240"` single-line box ratio (`M_word`):
        //    - When `font_face` is installed on the host OS (`is_system_installed == true`), Word
        //      opens that exact font binary: CJK-codepage fonts receive `hhea_span * 1.3`, while
        //      Western/monospace fonts use `(hhea.ascender - hhea.descender + hhea.lineGap) / upem`.
        //    - When `font_face` is a Typst-embedded-only font (`is_system_installed == false`, e.g.
        //      `Libertinus Serif`, `New Computer Modern`, `DejaVu Sans Mono`), Word substitutes it via
        //      `word/fontTable.xml` `<w:altName>` (`Times New Roman` / `Courier New`), so we MUST use
        //      the substituted font's `M_word` from `paradigm`!
        let word_ratio = match font_face {
            Some(face) if face.is_system_installed => {
                if face.has_cjk_codepage {
                    face.hhea_span_ratio() * WORD_CJK_FACTOR
                } else if has_cjk_chars && paradigm.is_none_or(|p| !p.is_cjk()) {
                    // Mixed paragraph starting on a Western run followed by CJK glyphs:
                    // Word's East Asian fallback (`PingFang SC`) expands the line box if not CJK-aware.
                    face.hhea_full_ratio()
                } else {
                    face.hhea_full_ratio()
                }
            }
            _ => match paradigm {
                Some(p) => {
                    if p.is_cjk() && !has_cjk_chars {
                        // Pure ASCII inside a CJK font still uses the font's CJK `m_win` in Word
                        // whenever the font itself is bound to `<w:rFonts w:ascii="..."/>`.
                        p.m_win()
                    } else {
                        p.m_win()
                    }
                }
                None => class.win_box_ratio(has_cjk_chars),
            },
        };

        (cap_ratio, word_ratio.max(0.8))
    }

    /// Construct a calibrated `WordLineBox` using both extracted OpenType binary font metrics
    /// ([`FontFaceMetricsIr`]) and optional ground-truth Typst paragraph baseline measurements
    /// ([`ParagraphLineMetricsIr`]).
    pub fn from_paragraph_metrics(
        font_family: Option<&str>,
        font_face: Option<&FontFaceMetricsIr>,
        para_lines: Option<&ParagraphLineMetricsIr>,
        has_cjk_chars: bool,
        has_inline_eq: bool,
        only_inline_eq: bool,
        font_size_pt: f64,
        leading_pt: Option<f64>,
    ) -> Self {
        let s = font_size_pt.max(6.0);
        let (cap_ratio, word_ratio) =
            Self::resolve_cap_and_word_ratios(font_family, font_face, has_cjk_chars);
        let inline_profile = ParagraphInlineProfile::from_flags(has_inline_eq, only_inline_eq);

        let w_line_240 = match inline_profile {
            ParagraphInlineProfile::OnlyInlineEquation => 276,
            ParagraphInlineProfile::TextWithInlineEquation
                if has_cjk_chars && s >= 14.0 && leading_pt.is_some_and(|ls| ls > 3.5) =>
            {
                240
            }
            _ => {
                // Priority 1: Exact uniform baseline-to-baseline step measured from Typst's frame
                // (`uniform_baseline_step_pt`, e.g. `16.61pt` for 11pt `PingFang SC` + `0.65em`).
                if let Some(step_pt) = para_lines.and_then(|p| p.uniform_baseline_step_pt) {
                    ((240.0 * step_pt / (s * word_ratio)).round() as i64).clamp(120, 720)
                } else if let Some(ls_pt) = leading_pt
                    && ls_pt <= 3.5
                {
                    // Compact synthetic table-cell / layout-grid leading (`0.0pt` or `2.0pt` set by
                    // `solve_table_cell_paragraph_style`), scaled for `1.82x` CJK fonts (`PingFang SC`, `Songti SC`).
                    let ratio = (ls_pt / s).clamp(0.0, 1.0);
                    let base = 212.0 + ratio * 24.0;
                    let scaled = if word_ratio > 1.50 {
                        base * (1.380 / word_ratio)
                    } else {
                        base
                    };
                    (scaled.round() as i64).clamp(120, 720)
                } else {
                    // Priority 2: Exact OpenType + Typst `top-edge: "cap-height"` model:
                    // `target_step_pt = S * cap_ratio + leading_pt` (default `0.65em` when `None`).
                    // Never clamp below `240` — CJK fonts with `word_ratio = 1.820` (`PingFang SC`,
                    // `Songti SC`) require `w:line = 199` / `177` (`< 240`) to match Typst's `16.61pt`!
                    let ls_pt = leading_pt.unwrap_or(0.65 * s);
                    let target_step_pt = s * cap_ratio + ls_pt;
                    ((240.0 * target_step_pt / (s * word_ratio)).round() as i64).clamp(120, 720)
                }
            }
        };

        Self::with_explicit_ratios(s, cap_ratio, word_ratio, w_line_240)
    }

    /// Construct a `WordLineBox` that matches a target Typst `leading_pt` (`par.leading`)
    /// when binary `FontFaceMetricsIr` / `ParagraphLineMetricsIr` are not provided.
    pub fn from_typst_metrics(
        font_family: Option<&str>,
        has_cjk_chars: bool,
        has_inline_eq: bool,
        only_inline_eq: bool,
        font_size_pt: f64,
        leading_pt: Option<f64>,
    ) -> Self {
        Self::from_paragraph_metrics(
            font_family,
            None,
            None,
            has_cjk_chars,
            has_inline_eq,
            only_inline_eq,
            font_size_pt,
            leading_pt,
        )
    }

    /// Construct a `WordLineBox` with an explicit `w:line` value (`w_line_240`).
    pub fn with_explicit_w_line(
        font_family: Option<&str>,
        has_cjk_chars: bool,
        font_size_pt: f64,
        w_line_240: i64,
    ) -> Self {
        let s = font_size_pt.max(6.0);
        let (ink_ratio, win_ratio) =
            Self::resolve_cap_and_word_ratios(font_family, None, has_cjk_chars);
        Self::with_explicit_ratios(s, ink_ratio, win_ratio, w_line_240)
    }

    #[inline]
    fn with_explicit_ratios(
        s: f64,
        ink_ratio: f64,
        win_ratio: f64,
        w_line_240: i64,
    ) -> Self {
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
        if is_cjk_or_fullwidth_char(ch) || ch == '…' {
            return s;
        }
        if let Some(fam) = font_family
            && let Some(units_2048) = lookup_calibrated_glyph_advance_2048(fam, ch)
        {
            return (units_2048 as f64 / 2048.0) * s;
        }
        let class = WordFontClass::classify(font_family, false);
        match (class, ch) {
            (_, '—') => s,
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

/// Exact TrueType `hmtx` horizontal advance widths (`2048` design-units-per-em scale)
/// for printable ASCII (`0x20..=0x7E`), extracted directly from system/Typst font binaries.
const ASCII_2048_TIMES_ROMAN: [u16; 95] = [
    512, 682, 836, 1024, 1024, 1706, 1593, 369, 682, 682, 1024, 1155, 512, 682, 512, 569, 1024,
    1024, 1024, 1024, 1024, 1024, 1024, 1024, 1024, 1024, 569, 569, 1155, 1155, 1155, 909, 1886,
    1479, 1366, 1366, 1479, 1251, 1139, 1479, 1479, 682, 797, 1479, 1251, 1821, 1479, 1479, 1139,
    1479, 1366, 1139, 1251, 1479, 1479, 1933, 1479, 1479, 1251, 682, 569, 682, 961, 1024, 682,
    909, 1024, 909, 1024, 909, 682, 1024, 1024, 569, 569, 1024, 569, 1593, 1024, 1024, 1024, 1024,
    682, 797, 569, 1024, 1024, 1479, 1024, 1024, 909, 983, 410, 983, 1108,
];

const ASCII_2048_ARIAL: [u16; 95] = [
    569, 569, 727, 1139, 1139, 1821, 1366, 391, 682, 682, 797, 1196, 569, 682, 569, 569, 1139,
    1139, 1139, 1139, 1139, 1139, 1139, 1139, 1139, 1139, 569, 569, 1196, 1196, 1196, 1139, 2079,
    1366, 1366, 1479, 1479, 1366, 1251, 1593, 1479, 569, 1024, 1366, 1139, 1706, 1479, 1593, 1366,
    1593, 1479, 1366, 1251, 1479, 1366, 1933, 1366, 1366, 1251, 569, 569, 569, 961, 1139, 682,
    1139, 1139, 1024, 1139, 1139, 569, 1139, 1139, 455, 455, 1024, 455, 1706, 1139, 1139, 1139,
    1139, 682, 1024, 569, 1139, 1024, 1479, 1024, 1024, 1024, 684, 532, 684, 1196,
];

const ASCII_2048_GEORGIA: [u16; 95] = [
    494, 678, 843, 1317, 1249, 1674, 1455, 441, 768, 768, 967, 1317, 552, 766, 552, 960, 1257,
    880, 1144, 1130, 1157, 1082, 1159, 1029, 1221, 1159, 640, 640, 1317, 1317, 1317, 980, 1902,
    1374, 1339, 1315, 1534, 1338, 1227, 1485, 1669, 798, 1060, 1422, 1236, 1899, 1571, 1524, 1249,
    1524, 1437, 1149, 1267, 1549, 1365, 1998, 1455, 1260, 1232, 768, 960, 768, 1317, 1317, 1024,
    1032, 1147, 930, 1176, 990, 666, 1043, 1192, 600, 598, 1097, 586, 1804, 1210, 1104, 1170,
    1146, 839, 885, 707, 1178, 1017, 1510, 1034, 1008, 909, 881, 768, 881, 1317,
];

const ASCII_2048_COURIER_NEW: [u16; 95] = [1229; 95];

const ASCII_2048_SONGTI_SC: [u16; 95] = [
    512, 449, 831, 1366, 918, 1686, 1493, 362, 598, 598, 874, 1366, 449, 641, 449, 1024, 961, 961,
    961, 961, 961, 961, 961, 961, 961, 961, 449, 449, 1366, 1366, 1366, 748, 1878, 1391, 1266,
    1300, 1579, 1348, 1159, 1579, 1569, 725, 682, 1518, 1180, 1735, 1583, 1597, 1159, 1577, 1303,
    977, 1264, 1454, 1393, 1819, 1434, 1350, 1337, 555, 1024, 555, 1024, 1026, 682, 1032, 1067,
    844, 1053, 883, 670, 961, 1090, 553, 459, 971, 557, 1608, 1090, 1040, 1075, 1071, 741, 745,
    662, 1061, 963, 1366, 936, 864, 868, 981, 1024, 981, 1366,
];

const ASCII_2048_HEITI_SC: [u16; 95] = [
    512, 604, 633, 1475, 1135, 1587, 1550, 406, 756, 756, 870, 1241, 567, 680, 567, 895, 1135,
    1135, 1135, 1135, 1135, 1135, 1135, 1135, 1135, 1135, 567, 567, 1241, 1241, 1241, 1014, 1776,
    1516, 1176, 1665, 1524, 1098, 993, 1786, 1399, 463, 987, 1210, 946, 1882, 1516, 1780, 1212,
    1784, 1243, 1020, 872, 1341, 1438, 1966, 1247, 1212, 983, 719, 1239, 719, 1376, 1024, 774,
    1399, 1397, 1325, 1403, 1331, 643, 1378, 1249, 410, 416, 1028, 410, 1921, 1249, 1341, 1397,
    1397, 616, 795, 694, 1245, 1135, 1702, 983, 1098, 870, 719, 1376, 719, 1241,
];

#[inline]
fn lookup_calibrated_glyph_advance_2048(family: &str, ch: char) -> Option<u16> {
    let f = family.trim().to_ascii_lowercase();
    let cp = ch as u32;
    let is_ascii = (0x20..=0x7E).contains(&cp);

    if f.contains("times") || f.contains("libertinus") || f.contains("cambria") {
        if is_ascii {
            return Some(ASCII_2048_TIMES_ROMAN[(cp - 0x20) as usize]);
        }
        return match ch {
            '\u{00A0}' => Some(512),
            '‘' | '’' => Some(682),
            '“' | '”' => Some(909),
            '–' => Some(1024),
            '—' => Some(2048),
            _ => None,
        };
    }
    if f.contains("arial") || f.contains("helvetica") || f.contains("calibri") {
        if is_ascii {
            return Some(ASCII_2048_ARIAL[(cp - 0x20) as usize]);
        }
        return match ch {
            '\u{00A0}' => Some(569),
            '‘' | '’' => Some(455),
            '“' | '”' => Some(682),
            '–' => Some(1139),
            '—' => Some(2048),
            _ => None,
        };
    }
    if f.contains("georgia") {
        if is_ascii {
            return Some(ASCII_2048_GEORGIA[(cp - 0x20) as usize]);
        }
        return match ch {
            '\u{00A0}' => Some(494),
            '‘' | '’' => Some(464),
            '“' | '”' => Some(840),
            '–' => Some(1317),
            '—' => Some(1755),
            _ => None,
        };
    }
    if f.contains("courier") || f.contains("mono") || f.contains("menlo") || f.contains("consolas")
    {
        if is_ascii {
            return Some(ASCII_2048_COURIER_NEW[(cp - 0x20) as usize]);
        }
        return match ch {
            '\u{00A0}' | '‘' | '’' | '“' | '”' | '–' | '—' => Some(1229),
            _ => None,
        };
    }
    if f.contains("songti") || f.contains("stsong") || f.contains("kaiti") || f == "宋体" || f == "楷体"
    {
        if is_ascii {
            return Some(ASCII_2048_SONGTI_SC[(cp - 0x20) as usize]);
        }
        return match ch {
            '\u{00A0}' => Some(512),
            '‘' => Some(479),
            '’' => Some(549),
            '“' => Some(797),
            '”' => Some(844),
            '–' | '—' => Some(2048),
            _ => None,
        };
    }
    if f.contains("heiti")
        || f.contains("pingfang")
        || f.contains("simhei")
        || f.contains("yahei")
        || f == "黑体"
    {
        if is_ascii {
            return Some(ASCII_2048_HEITI_SC[(cp - 0x20) as usize]);
        }
        return match ch {
            '\u{00A0}' => Some(512),
            '‘' | '’' | '“' | '”' | '—' => Some(2048),
            '–' => Some(1024),
            _ => None,
        };
    }
    None
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
