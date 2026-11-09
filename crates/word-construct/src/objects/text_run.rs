//! Word TextRun Construct (`<w:r>` / `<w:rPr>`).
//!
//! Solves `<w:rFonts>` (`w:hint="eastAsia"` policy) and `<w:sz>` / `<w:spacing>`
//! (including inline code `DejaVu Sans Mono` -> `Consolas` advance-width compensation)
//! from pipeline-annotated [`TextRunIr`].

use ir::{ScriptKindIr, TextRunIr};

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
        // 但是需要注意：由于 `DejaVu Sans Mono` 是编译进 Typst 二进制的内置字体、并未安装在 macOS 系统字体目录中，
        // 若宿主机器未安装该字体，macOS 版 Microsoft Word 在导出 PDF 时甚至会忽略 `word/fontTable.xml` 中的
        // `<w:altName w:val="Consolas"/>` 回退声明，直接将其回退为比例字体 `Helvetica`，从而产生字宽与断行差异。
        // 因此若需在未安装 `DejaVu Sans Mono` 的环境中严格对比代码块/行内代码排版，应在 `.typ` 源文件侧显式声明
        // 系统已安装的等宽字体（如 `#show raw: set text(font: "Courier New", size: 9pt)`）。
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
