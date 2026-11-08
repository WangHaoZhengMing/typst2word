//! Explicit markers for unsupported Typst elements.

use serde::{Deserialize, Serialize};

/// Explicit representation of an unsupported Typst element or feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsupportedIr {
    pub element_name: String,
    pub reason: String,
    pub source_location: Option<String>,
}

impl UnsupportedIr {
    pub fn display_badge(&self) -> String {
        if self.reason.is_empty() {
            format!("[不支持的元素: {}]", self.element_name)
        } else {
            format!("[不支持的元素: {} ({})]", self.element_name, self.reason)
        }
    }
}
