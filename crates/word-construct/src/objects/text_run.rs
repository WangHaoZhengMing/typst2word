//! Word TextRun Construct (`<w:r>` / `<w:rPr>`).
//!
//! Solves `<w:rFonts>` (`w:hint="eastAsia"` policy) and `<w:sz>` / `<w:spacing>`
//! (including inline code `DejaVu Sans Mono` -> `Consolas` advance-width compensation)
//! from pipeline-annotated [`TextRunIr`].

use ir::TextRunIr;

/// Exhaustive East Asian font hint policy for `<w:rFonts>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EastAsiaHintPolicy {
    /// Do not emit `w:hint="eastAsia"`.
    Omit,
    /// Emit `w:hint="eastAsia"` so Word routes ambiguous Unicode symbols (`U+2000..=U+2BFF`)
    /// to the `w:eastAsia` font slot instead of `w:ascii`/`w:hAnsi`.
    EmitEastAsia,
}

impl EastAsiaHintPolicy {
    #[inline]
    pub const fn as_xml_attr(self) -> &'static str {
        match self {
            Self::Omit => "",
            Self::EmitEastAsia => r#" w:hint="eastAsia""#,
        }
    }
}

/// Exhaustive `<w:sz>` + `<w:spacing>` font size construct for `<w:rPr>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunFontSizeConstruct {
    /// Inherit default paragraph/document font size.
    Inherited,
    /// Standard half-point size (`<w:sz w:val="{0}"/><w:szCs w:val="{0}"/>`).
    StandardHalfPts(u32),
    /// Inline code `DejaVu Sans Mono` (`8.8pt`) mapped to `Consolas` (`19` half-pts + `1 dxa` spacing)
    /// to match Typst's inline raw advance width to within `0.025pt/char`.
    ConsolasInlineCodeCompensated {
        half_pts: u32,
        char_spacing_dxa: i32,
    },
}

/// Solved Word construct for a `<w:r>` text run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRunConstruct<'a> {
    pub font_family: Option<&'a str>,
    pub east_asia_hint: EastAsiaHintPolicy,
    pub font_size: RunFontSizeConstruct,
    pub border_sz_eighths_pt: Option<u32>,
}

impl<'a> TextRunConstruct<'a> {
    pub fn solve(run: &'a TextRunIr) -> Self {
        let east_asia_hint =
            if run.font_class.is_cjk_font() && run.text_script.has_east_asian_symbol() {
                EastAsiaHintPolicy::EmitEastAsia
            } else {
                EastAsiaHintPolicy::Omit
            };

        let font_size = match run.font_size_pt {
            None => RunFontSizeConstruct::Inherited,
            Some(sz_pt) => {
                let is_consolas_fallback = run.is_code
                    && matches!(
                        run.font_family.as_deref(),
                        None | Some("Consolas" | "DejaVu Sans Mono")
                    )
                    && (sz_pt - 8.8).abs() < 0.15;
                if is_consolas_fallback {
                    RunFontSizeConstruct::ConsolasInlineCodeCompensated {
                        half_pts: 19,
                        char_spacing_dxa: 1,
                    }
                } else {
                    RunFontSizeConstruct::StandardHalfPts(((sz_pt * 2.0).round() as u32).clamp(4, 288))
                }
            }
        };

        let border_sz_eighths_pt = run
            .border
            .as_ref()
            .map(|bdr| ((bdr.thickness_pt * 8.0).round() as u32).clamp(2, 96));

        Self {
            font_family: run
                .font_family
                .as_deref()
                .or(run.is_code.then_some("DejaVu Sans Mono")),
            east_asia_hint,
            font_size,
            border_sz_eighths_pt,
        }
    }
}
