// @func: par
// @docs: https://typst.app/docs/reference/model/par/

// === @case par_leading_04em ===
// @title: Paragraph Leading: 0.4em (Tight)
// @param: leading: 0.4em
// @var: par.leading = 0.4em
// @size: 300x140
// @ir: Paragraph
// @xml: `<w:spacing `
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(leading: 0.4em, linebreaks: "simple")
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_leading_065em ===
// @title: Paragraph Leading: 0.65em (Default)
// @param: leading: 0.65em
// @var: par.leading = 0.65em
// @size: 300x150
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(leading: 0.65em, linebreaks: "simple")
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_leading_12em ===
// @title: Paragraph Leading: 1.2em (Wide)
// @param: leading: 1.2em
// @var: par.leading = 1.2em
// @size: 300x180
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(leading: 1.2em, linebreaks: "simple")
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_spacing_06em ===
// @title: Inter-Paragraph Spacing: 0.6em
// @param: spacing: 0.6em
// @var: par.spacing = 0.6em
// @size: 300x120
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(spacing: 0.6em)
First paragraph line.

Second paragraph line.

// === @case par_spacing_18em ===
// @title: Inter-Paragraph Spacing: 1.8em
// @param: spacing: 1.8em
// @var: par.spacing = 1.8em
// @size: 300x140
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(spacing: 1.8em)
First paragraph line.

Second paragraph line.

// === @case par_justify_false ===
// @title: Justification: false (Ragged Right)
// @param: justify: false
// @var: par.justify = false
// @size: 300x150
// @ir: Paragraph
// @xml: w:val="left"
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(justify: false, linebreaks: "simple")
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_justify_true ===
// @title: Justification: true (Both Edges)
// @param: justify: true
// @var: par.justify = true
// @size: 300x150
// @ir: Paragraph
// @xml: w:val="both"
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(justify: true)
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_linebreaks_simple ===
// @title: Linebreak Algorithm: simple (Greedy)
// @param: linebreaks: simple
// @var: par.linebreaks = "simple"
// @size: 280x160
// @ir: Paragraph
#set text(font: "Times New Roman", size: 11pt, hyphenate: false)
#set par(justify: true, linebreaks: "simple")
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_linebreaks_optimized ===
// @title: Linebreak Algorithm: optimized (Knuth-Plass)
// @param: linebreaks: optimized
// @var: par.linebreaks = "optimized"
// @size: 280x160
// @ir: Paragraph
#set text(font: "Times New Roman", size: 11pt, hyphenate: false)
#set par(justify: true, linebreaks: "optimized")
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.

// === @case par_first_indent_1line_cjk ===
// @title: First-Line Indent (2em) on 1-Line CJK Paragraph
// @param: first-line-indent
// @var: par.first-line-indent = (2em, all: true) [1-line]
// @size: 360x100
// @ir: Paragraph
// @xml: w:firstLine=
#set text(font: "Songti SC", size: 10.5pt, lang: "zh")
#set par(first-line-indent: (amount: 2em, all: true))
主要函数及其职责如表2.1所示。

// === @case par_first_indent_multiline_cjk ===
// @title: First-Line Indent (2em) on Multi-Line CJK Paragraph
// @param: first-line-indent
// @var: par.first-line-indent = (2em, all: true) [multi-line]
// @size: 300x140
// @ir: Paragraph
// @xml: w:firstLine=
#set text(font: "Songti SC", size: 12pt, lang: "zh")
#set par(justify: true, first-line-indent: (amount: 2em, all: true))
在现代编译器的中间表示设计中，保持源文档的语义结构与物理排版度量相互独立，是实现高质量跨格式转换的核心前提。

// === @case par_hanging_indent ===
// @title: Hanging Indent (24pt)
// @param: hanging-indent
// @var: par.hanging-indent = 24pt
// @size: 300x140
// @ir: Paragraph
// @xml: w:hanging=
#set text(font: "Times New Roman", size: 12pt, hyphenate: false)
#set par(hanging-indent: 24pt)
In olden times when wishing still helped one, there lived a king whose daughters were all beautiful, and the youngest was so beautiful that the sun itself wondered.
