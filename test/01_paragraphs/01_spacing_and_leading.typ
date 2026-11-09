// Test 01_paragraphs/01_spacing_and_leading.typ
// Covers: paragraph spacing (above/below), line leading (single, 1.5x, double),
// first-line indentation, and multi-paragraph vertical rhythm.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Standard Paragraph Leading and Spacing

This first paragraph establishes the baseline vertical rhythm using Times New Roman at 11pt with a standard 0.65em line leading and 1.2em inter-paragraph spacing. Every line break and baseline position is measured against Microsoft Word's layout engine to verify vertical and horizontal fidelity across multiple wrapped lines of prose.

This second paragraph immediately follows the first paragraph after the standard 1.2em paragraph gap. When converting from Typst to OOXML, line leading and paragraph spacing must translate accurately into w:spacing attributes so that cumulative vertical drift does not occur down the page.

= 2. First-Line Indentation and 0.8em Leading

#[
  #set par(first-line-indent: (amount: 2em, all: true), spacing: 1.0em, leading: 0.8em)

  First-line indentation of 2em (22pt at 11pt font size, corresponding to 440 dxa in OOXML) shifts the opening word of each paragraph inward while subsequent wrapped lines return to the left margin at 3.18cm. This section also increases the line leading to 0.8em to verify proportional line spacing adjustments.

  A second indented paragraph confirms that every subsequent paragraph in the scope inherits the 2em first-line indent and 1.0em inter-paragraph spacing consistently across multiple lines of text.
]

= 3. Wide Leading (1.2em) and 1.8em Spacing

#[
  #set par(first-line-indent: 0pt, spacing: 1.8em, leading: 1.2em)

  Double-like wide leading (1.2em) and generous inter-paragraph spacing (1.8em) are common in academic manuscripts and legal drafts. This paragraph tests whether large line spacing increments preserve exact baseline coordinates between Typst PDF output and Word PDF output.

  The concluding paragraph in this test verifies the 1.8em inter-paragraph transition before the final lines of the page.
]
