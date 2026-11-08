//! Math equations (pre-rendered OMML).

use serde::{Deserialize, Serialize};

/// Mathematical equation IR with native Word OMML (`<m:oMath>` / `<m:oMathPara>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationIr {
    pub is_block: bool,
    /// Complete `<m:oMath>...</m:oMath>` XML fragment.
    pub omml_xml: String,
    /// Plain-text/TeX-like fallback representation for accessibility or debugging.
    pub source_text: String,
    /// Optional equation number text (e.g., `"(1)"` or `"(2.1)"`).
    pub numbering_text: Option<String>,
    /// Optional bookmark name for `@ref` cross-referencing.
    pub bookmark: Option<String>,
}
