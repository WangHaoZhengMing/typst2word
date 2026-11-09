// Test 09_misc/01_quotes_code_figures_footnotes.typ
// Covers: Blockquotes (`#quote`), syntax-highlighted code blocks (```` ```rust ````),
// inline raw code, numbered figures with captions, hyperlinks (`#link`), and `#footnote`.

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

= 1. Blockquotes with Attribution (`#quote`)

#quote(block: true, attribution: [Donald E. Knuth, Digital Typography (1999)])[
  Mechanical rules for breaking paragraphs into lines have been used since the invention of movable type, and small differences in line-breaking rules produce surprisingly noticeable changes in page texture.
]

= 2. Syntax-Highlighted Code Blocks and Inline Code

Inline code tokens like `Ptls7BreakClass::classify(ch)` and `lookup_ptls7_pair(before, after)` use monospace styling, while fenced code blocks preserve syntax highlighting colors:

```rust
pub fn can_break_ptls7_script5(before: char, after: char) -> bool {
    let cls_before = Ptls7BreakClass::classify(before);
    let cls_after = Ptls7BreakClass::classify(after);
    lookup_ptls7_pair(cls_before, cls_after).allows_direct_break()
}
```

= 3. External Hyperlinks (`#link`) and Native Footnotes (`#footnote`)

This paragraph includes an active external hyperlink to the #link("https://typst.app/docs")[Typst Official Documentation] as well as two native Word footnotes: the first footnote#footnote[Converted directly into `word/footnotes.xml` with a `<w:footnoteReference>` marker in the body run.] explains OPC package relationships, and the second footnote#footnote[Supports *bold*, _italic_, and inline code `w:footnote` inside footnote bodies.] verifies sequential numbering at the bottom of the page.

= 4. Numbered Figure with Caption (`#figure`)

#figure(
  rect(
    width: 240pt,
    height: 45pt,
    fill: rgb("#e8f5e9"),
    stroke: 1.2pt + rgb("#2e7d32"),
    radius: 5pt,
  )[
    #align(center + horizon)[*Figure Payload:* Native DrawingML Vector Diagram]
  ],
  caption: [Schematic representation of a captioned figure element in Typst and Word.],
)
