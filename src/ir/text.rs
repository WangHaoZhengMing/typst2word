//! Styled text runs (`TextRunIr`).

use serde::{Deserialize, Serialize};

use super::{ColorHex, StrokeIr};

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
