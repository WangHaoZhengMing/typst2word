// @func: text
// @docs: https://typst.app/docs/reference/text/text/

// === @case font_size_08pt ===
// @title: Font Size: 8pt
// @param: size: 8pt
// @var: text.size = 8pt
// @size: 360x100
// @ir: Paragraph
// @xml: w:val="16"
#set text(font: "Times New Roman", size: 8pt, hyphenate: false)
Compact eight-point typography baseline.

// === @case font_size_10_5pt_wuhao ===
// @title: Font Size: 10.5pt (五号)
// @param: size: 10.5pt
// @var: text.size = 10.5pt
// @size: 360x100
// @ir: Paragraph
// @xml: w:val="21"
#set text(font: "Songti SC", size: 10.5pt, lang: "zh")
五号宋体单变量字号测试样本。

// === @case font_size_16pt ===
// @title: Font Size: 16pt
// @param: size: 16pt
// @var: text.size = 16pt
// @size: 360x110
// @ir: Paragraph
// @xml: w:val="32"
#set text(font: "Times New Roman", size: 16pt, hyphenate: false)
Sixteen Point Scale

// === @case font_size_24pt ===
// @title: Font Size: 24pt
// @param: size: 24pt
// @var: text.size = 24pt
// @size: 360x120
// @ir: Paragraph
// @xml: w:val="48"
// @tol: 3.5, 6.0, 14.0, 12.0
#set text(font: "Times New Roman", size: 24pt, hyphenate: false)
Display 24pt

// === @case inline_color_rgb ===
// @title: Inline Text Color (RGB)
// @param: fill
// @var: text(fill: rgb("#D92D20"))
// @size: 360x100
// @ir: Paragraph
// @xml: D92D20
#set text(font: "Times New Roman", size: 12pt)
#text(fill: rgb("#D92D20"))[Crimson colored run]
