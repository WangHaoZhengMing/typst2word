//! Styled text runs (`TextRunIr`) and script positioning.

use serde::{Deserialize, Serialize};

use super::{ColorHex, StrokeIr};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScriptKindIr {
    #[default]
    Normal,
    Sub,
    Super,
}
