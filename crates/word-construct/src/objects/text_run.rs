//! Word TextRun Construct (`<w:r>` / `<w:rPr>`).
//!
//! Solves `<w:rFonts>` (`w:hint="eastAsia"` policy) and `<w:sz>` / `<w:spacing>`
//! (including inline code `DejaVu Sans Mono` -> `Consolas` advance-width compensation)
//! from [`TextRunIr`].

use ir::{ScriptKindIr, TextRunIr};

use crate::pipeline::font_recognize::classify_font_family;
use crate::pipeline::lang_script::TextScriptClassIr;

/// Whether a [`TextRunIr`] contains CJK ideographs/fullwidth characters.
#[inline]
pub fn run_has_cjk_chars(run: &TextRunIr) -> bool {
    TextScriptClassIr::classify(&run.text).has_cjk()
}

/// Whether a [`TextRunIr`] uses a CJK typeface or contains CJK characters.
#[inline]
pub fn run_has_cjk(run: &TextRunIr) -> bool {
    run_has_cjk_chars(run)
        || classify_font_family(run.font_family.as_deref(), run.is_code).is_cjk_font()
}

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
        let font_class = classify_font_family(run.font_family.as_deref(), run.is_code);
        let text_script = TextScriptClassIr::classify(&run.text);
        let east_asia_hint =
            if font_class.is_cjk_font() && text_script.has_east_asian_symbol() {
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
                    );
                if is_consolas_fallback && (sz_pt - 8.8).abs() < 0.15 {
                    RunFontSizeConstruct::ConsolasInlineCodeCompensated {
                        half_pts: 19,
                        char_spacing_dxa: 1,
                    }
                } else if is_consolas_fallback {
                    RunFontSizeConstruct::StandardHalfPts(
                        ((sz_pt * 1.09 * 2.0).round() as u32).clamp(10, 288),
                    )
                } else {
                    // Typst's `SubElem` / `SuperElem` pre-scales `item.size` by the font's OS/2
                    // `ySubscriptYSize` / `ySuperscriptYSize` ratio (`1331/2048 = 0.65`), and
                    // Word's `<w:vertAlign w:val="subscript|superscript"/>` scales `<w:sz>` down
                    // again and truncates to its internal `0.24pt` (`1/300 in`) grid (`w:sz="22"`
                    // -> `29 * 0.24pt = 6.96pt`, whereas `w:sz="23"` -> `30 * 0.24pt = 7.20pt`,
                    // matching Typst's `7.15pt` within `0.05pt`).
                    let unscaled_pt = match run.script {
                        ScriptKindIr::Normal => sz_pt,
                        ScriptKindIr::Sub | ScriptKindIr::Super => sz_pt / 0.635,
                    };
                    RunFontSizeConstruct::StandardHalfPts(
                        ((unscaled_pt * 2.0).round() as u32).clamp(4, 288),
                    )
                }
            }
        };

        let border_sz_eighths_pt = run
            .border
            .as_ref()
            .map(|bdr| ((bdr.thickness_pt * 8.0).round() as u32).clamp(2, 96));

        // 不改变原字体（Do not alter or substitute the original font family extracted from Typst）：
        // 原文档使用什么字体（例如 Typst 内置的 `"DejaVu Sans Mono"` 或 `"Libertinus Serif"`），
        // 生成的 `.docx` `<w:rFonts>` 就严格原样输出该字体名称，绝不在转换器代码中私自篡改为 `"Consolas"` 等替代字体。
        // 但是需要注意：由于 `DejaVu Sans Mono` 是编译进 Typst 二进制的内置字体、默认未安装在 macOS 系统字体目录中，
        // 当宿主机器未安装该字体时，Microsoft Word 会根据 `word/fontTable.xml` 中的 `<w:altName w:val="Consolas"/>`
        // 回退使用系统等宽字体 `Consolas`（字宽 `0.5498em` 略窄于 `DejaVu Sans Mono` 的 `0.602em`），因此上面通过
        // `half_pts: 19` (`9.5pt`) + `char_spacing_dxa: 1` (`+0.05pt/char`) 将单字符步进从 `4.84pt` 补偿至 `5.32pt`（与 Typst 的 `5.30pt` 误差仅 `0.025pt/char`）。
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
