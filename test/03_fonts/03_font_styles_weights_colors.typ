// Test 03_fonts/03_font_styles_weights_colors.typ
// Covers: Bold, Italic, Bold+Italic, Underline, Strikethrough, Highlight,
// Text Fill Colors, Subscript/Superscript, and Smallcaps.

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

= 1. Weight and Posture Variations (Regular, Bold, Italic, Bold-Italic)

This paragraph mixes regular roman text with *boldface emphasis (`<w:b/>`)*, _italicized terminology (`<w:i/>`)_, and *_combined bold-italic phrases (`<w:b/><w:i/>`)_* within a single continuous flow. Because bold glyphs in Times New Roman have slightly wider horizontal advances than regular glyphs, accurate run-level font switching is essential to preserve identical line break positions.

= 2. Text Decorations: Underline, Strikethrough, and Highlight

Document revision workflows rely on inline text decorations: #underline[underlined clauses (`<w:u w:val="single"/>`)], #strike[strikethrough deletions (`<w:strike/>`)], and #highlight(fill: rgb("#fff3a8"))[highlighted key findings]. Each decoration must apply strictly to its delimited character range without leaking into adjacent runs or altering baseline vertical coordinates.

= 3. RGB Fill Colors and Semantic Inline Styling

Inline color runs support arbitrary RGB hex values: #text(fill: rgb("#c0392b"))[*crimson alert text (`#C0392B`)*], #text(fill: rgb("#1f618d"))[*navy reference text (`#1F618D`)*], #text(fill: rgb("#1e8449"))[*emerald status text (`#1E8449`)*], and #text(fill: rgb("#7d3c98"))[*violet annotation text (`#7D3C98`)*] embedded seamlessly inside standard black prose.

= 4. Superscripts, Subscripts, and Small Capitals

Chemical formulas such as H#sub[2]SO#sub[4], CO#sub[2], and Ca(OH)#sub[2] use subscript runs (`<w:vertAlign w:val="subscript"/>`), whereas footnote markers#super[1] and isotopic mass numbers like #super[14]C and E = mc#super[2] use superscript runs (`<w:vertAlign w:val="superscript"/>`). Typographic #smallcaps[Small Capitals (`<w:smallCaps/>`)] are frequently used for acronyms and section lead-ins.
