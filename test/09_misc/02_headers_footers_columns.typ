// Test 09_misc/02_headers_footers_columns.typ
// Covers: Running page headers and footers, section heading numbering (`1.1`),
// document title, and two-column layout (`#columns(2)`).

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
  header: align(right)[#text(size: 9pt, fill: rgb("#555555"))[_Typst2Word Layout Verification Suite — Header_]],
  footer: align(center)[#text(size: 9pt, fill: rgb("#555555"))[Page 1]],
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)
#set heading(numbering: "1.1")

= Section Heading Numbering and Running Headers

This test verifies that running page headers (`word/header1.xml`), page footers (`word/footer1.xml`), and hierarchical section numbers (`1`, `1.1`, `1.2`) render in matching positions.

== Subsection Level 2 (`1.1`)

Hierarchical headings preserve their numbering prefix and apply consistent font sizing and before/after spacing.

=== Sub-subsection Level 3 (`1.1.1`)

Level 3 headings test deeper outline hierarchy and `keepWithNext` binding to the following paragraph.

= Two-Column Layout (`#columns(2)`)

#columns(2, gutter: 18pt)[
  Two-column page layouts partition the 415.3pt body width into two equal columns separated by an 18pt gutter (`<w:cols w:num="2" w:space="360"/>` in OOXML section properties).

  In narrower columns (`198.65pt` per column), line wrapping occurs more frequently, providing a stringent test of word advance summation and greedy First-Fit line breaking across column breaks.

  Each column maintains the same 11pt Times New Roman font metrics and 0.65em line leading as single-column body prose.
]
