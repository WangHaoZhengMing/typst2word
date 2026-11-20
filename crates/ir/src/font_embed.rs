//! Embedded font payload metadata carried from Typst extraction (`extract`) to OOXML packaging (`docx-writer`).

use serde::{Deserialize, Serialize};

/// Controls whether and how realized fonts are embedded into the generated `.docx` package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontEmbedMode {
    /// Embed subsetted text fonts (retaining `cmap`, `OS/2`, `name`, etc.) and complete math fonts
    /// (preserving the OpenType `MATH` table). Default mode.
    #[default]
    Subset,
    /// Embed complete standalone SFNT font files for every realized face (including `.ttc` face extraction).
    Full,
    /// Do not embed any font files into `.docx` (only emit `word/fontTable.xml` `<w:altName>` fallbacks).
    None,
}

impl FontEmbedMode {
    pub fn parse_cli(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "subset" => Ok(Self::Subset),
            "full" => Ok(Self::Full),
            "none" | "off" | "false" => Ok(Self::None),
            other => Err(format!(
                "invalid --embed-fonts value '{other}' (expected 'subset', 'full', or 'none')"
            )),
        }
    }
}

/// OOXML `<w:font>` style slot (`<w:embedRegular>`, `<w:embedBold>`, `<w:embedItalic>`, `<w:embedBoldItalic>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EmbeddedFontSlot {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl EmbeddedFontSlot {
    pub const fn ooxml_tag(self) -> &'static str {
        match self {
            Self::Regular => "embedRegular",
            Self::Bold => "embedBold",
            Self::Italic => "embedItalic",
            Self::BoldItalic => "embedBoldItalic",
        }
    }
}

/// A standalone SFNT (`.ttf` or CFF `.otf`) font face ready for OOXML `.odttf` obfuscation and packaging.
#[derive(Debug, Clone)]
pub struct EmbeddedFontFaceIr {
    /// Typst realized font family name (as emitted in `<w:rFonts>` for text runs).
    pub typst_family: String,
    /// OpenType `nameID 1` (Windows Family Name) used by Word to bind `<w:font w:name="...">`
    /// and `<m:mathFont m:val="..."/>` (e.g. `"NewComputerModernMath"`).
    pub word_family: String,
    /// Style slot (`Regular`, `Bold`, `Italic`, `BoldItalic`).
    pub slot: EmbeddedFontSlot,
    /// Whether this face contains an OpenType `MATH` table.
    pub is_math: bool,
    /// Whether `sfnt_data` was subsetted to used characters (controls `w:subsetted="1"` and `<w:saveSubsetFonts/>`).
    pub is_subset: bool,
    /// Standalone SFNT binary bytes (`.ttf` or `.otf`, never `.ttc` and not yet XOR-obfuscated).
    pub sfnt_data: Vec<u8>,
}
