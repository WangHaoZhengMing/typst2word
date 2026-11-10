//! Pass 2 (`WordPipelinePassKind::LanguageAndScriptRecognition`):
//! Exhaustive script/character classification ([`TextScriptClassIr`]) for Word `<w:rFonts w:hint="eastAsia"/>`
//! and Office Math `<m:nor/>` routing.

use serde::{Deserialize, Serialize};

use ir::{DocumentIr, is_cjk_char};

/// Check if a character is a general punctuation / arrow / mathematical symbol (`U+2000..=U+2BFF`)
/// that requires `w:hint="eastAsia"` when rendered with a CJK font in Word.
#[inline]
pub const fn is_east_asian_symbol_char(c: char) -> bool {
    matches!(c, '\u{2000}'..='\u{2BFF}')
}

/// Exhaustive script/character classification of a text run in Microsoft Word.
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

/// Run Pass 2 (`LanguageAndScriptRecognition`).
///
/// Script classification is evaluated on-demand by [`TextScriptClassIr::classify`] without mutating
/// `DocumentIr` in place.
pub fn run_language_and_script_pass(_doc: &mut DocumentIr) {}
