//! Language/region locale (`LocaleIr`) and text progression direction (`TextDirIr`).

use serde::{Deserialize, Serialize};

/// Cascaded Typst text locale (`#set text(lang: "...", region: "...")`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct LocaleIr {
    /// ISO 639 lowercase language code (e.g., `"en"`, `"zh"`, `"ja"`, `"ar"`).
    pub lang: String,
    /// Optional ISO 3166 uppercase region code (e.g., `"US"`, `"CN"`, `"TW"`, `"JP"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

impl LocaleIr {
    #[inline]
    pub fn new(lang: impl Into<String>, region: Option<String>) -> Self {
        Self {
            lang: lang.into().to_ascii_lowercase(),
            region: region.map(|r| r.to_ascii_uppercase()),
        }
    }

    /// Format as an OpenXML BCP-47 `<w:lang>` tag (e.g., `"en-US"`, `"zh-CN"`, `"ja-JP"`).
    pub fn to_word_lang_tag(&self) -> String {
        if let Some(reg) = &self.region
            && !reg.is_empty()
        {
            return format!("{}-{reg}", self.lang);
        }
        match self.lang.as_str() {
            "en" => "en-US".to_string(),
            "zh" => "zh-CN".to_string(),
            "ja" => "ja-JP".to_string(),
            "ko" => "ko-KR".to_string(),
            "de" => "de-DE".to_string(),
            "fr" => "fr-FR".to_string(),
            "es" => "es-ES".to_string(),
            "pt" => "pt-BR".to_string(),
            "it" => "it-IT".to_string(),
            "ru" => "ru-RU".to_string(),
            "ar" => "ar-SA".to_string(),
            "he" => "he-IL".to_string(),
            "fa" => "fa-IR".to_string(),
            other => other.to_string(),
        }
    }

    #[inline]
    pub fn is_east_asian(&self) -> bool {
        matches!(self.lang.as_str(), "zh" | "ja" | "ko")
    }

    #[inline]
    pub fn is_bidi(&self) -> bool {
        matches!(self.lang.as_str(), "ar" | "he" | "fa" | "ur" | "yi" | "dv")
    }
}

/// Horizontal inline/paragraph text direction (`#set text(dir: ltr | rtl)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextDirIr {
    #[default]
    Ltr,
    Rtl,
}
