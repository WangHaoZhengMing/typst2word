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
    /// Exhaustively classify a text string into one of the four [`TextScriptClassIr`] variants.
    #[inline]
    pub fn classify(text: &str) -> Self {
        let mut has_cjk = false;
        let mut has_ea_symbol = false;
        for ch in text.chars() {
            if is_cjk_char(ch) {
                has_cjk = true;
            } else if is_east_asian_symbol_char(ch) {
                has_ea_symbol = true;
            }
            if has_cjk && has_ea_symbol {
                return Self::CjkWithEastAsianSymbols;
            }
        }
        match (has_cjk, has_ea_symbol) {
            (true, true) => Self::CjkWithEastAsianSymbols,
            (true, false) => Self::CjkIdeographs,
            (false, true) => Self::LatinWithEastAsianSymbols,
            (false, false) => Self::LatinOnly,
        }
    }

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

/// Check if a character is a general punctuation / arrow / mathematical symbol (`U+2000..=U+2BFF`)
/// that requires `w:hint="eastAsia"` when rendered with a CJK font in Word.
#[inline]
pub const fn is_east_asian_symbol_char(c: char) -> bool {
    matches!(c, '\u{2000}'..='\u{2BFF}')
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
