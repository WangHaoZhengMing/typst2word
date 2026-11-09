//! Styled text runs (`TextRunIr`) and exhaustive font/script classifications.

use serde::{Deserialize, Serialize};

use super::{ColorHex, StrokeIr};

/// Exhaustive classification of a run's font family, populated by the font recognition pipeline pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FontFamilyClassIr {
    /// Default or Western proportional font (`Libertinus Serif`, `Times New Roman`, `Arial`).
    #[default]
    LatinProportional,
    /// Modern CJK UI sans-serif font (`PingFang SC`, `Heiti SC`, `Microsoft YaHei`, `SimHei`).
    CjkUiSans,
    /// Traditional CJK serif/script font (`Songti SC`, `SimSun`, `Kaiti SC`, `FangSong`).
    CjkSerif,
    /// Fixed-width monospace font (`Consolas`, `SF Mono`, `Menlo`, `DejaVu Sans Mono`, `Courier New`).
    Monospace,
}

impl FontFamilyClassIr {
    #[inline]
    pub const fn is_cjk_font(self) -> bool {
        matches!(self, Self::CjkUiSans | Self::CjkSerif)
    }
}

/// Exhaustive script/character classification of a text run, populated by the language/script pipeline pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextScriptClassIr {
    /// Pure ASCII / Western Latin without CJK ideographs or `U+2000..=U+2BFF` East Asian symbols.
    #[default]
    LatinOnly,
    /// Contains `U+2000..=U+2BFF` symbols (e.g. `“`, `”`, `—`, `…`, `→`) without CJK ideographs.
    LatinWithEastAsianSymbols,
    /// Contains CJK ideographs / fullwidth forms without `U+2000..=U+2BFF` symbols.
    CjkIdeographs,
    /// Contains both CJK ideographs / fullwidth forms AND `U+2000..=U+2BFF` symbols.
    CjkWithEastAsianSymbols,
}

impl TextScriptClassIr {
    #[inline]
    pub const fn has_cjk(self) -> bool {
        matches!(self, Self::CjkIdeographs | Self::CjkWithEastAsianSymbols)
    }

    #[inline]
    pub const fn has_east_asian_symbol(self) -> bool {
        matches!(
            self,
            Self::LatinWithEastAsianSymbols | Self::CjkWithEastAsianSymbols
        )
    }
}

/// Richly styled text run.
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
    #[serde(default)]
    pub font_class: FontFamilyClassIr,
    #[serde(default)]
    pub text_script: TextScriptClassIr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScriptKindIr {
    #[default]
    Normal,
    Sub,
    Super,
}
