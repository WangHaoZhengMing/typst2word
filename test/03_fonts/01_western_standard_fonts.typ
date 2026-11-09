// Test 03_fonts/01_western_standard_fonts.typ
// Covers: Standard cross-platform Western fonts (Times New Roman, Arial,
// Courier New, Georgia) across multiple point sizes (10pt, 11pt, 12pt, 14pt).

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#show raw: set text(font: "Courier New", size: 9pt)
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Times New Roman (11pt Proportional Serif)

#text(font: "Times New Roman", size: 11pt)[
  Times New Roman uses a 2048-unit em-square (`unitsPerEm = 2048`) with OS/2 typographic ascender 1825, descender -443, and lineGap 87. It serves as the primary serif typeface for verifying proportional advance widths and kerning behavior between Typst and Word.
]

= 2. Arial (11pt Proportional Sans-Serif)

#text(font: "Arial", size: 11pt)[
  Arial is the standard neo-grotesque sans-serif companion to Times New Roman in Microsoft Office (`unitsPerEm = 2048`). Its wider lowercase x-height and distinct character advance widths test whether font-specific glyph metrics wrap at identical word boundaries in both engines.
]

= 3. Georgia (11pt Transitional Serif)

#text(font: "Georgia", size: 11pt)[
  Georgia was designed by Matthew Carter specifically for screen legibility and high x-height clarity. Because Georgia's average lowercase glyph advance is noticeably wider than Times New Roman, fewer words fit onto a 415.3pt line, providing a direct check on horizontal advance summation.
]

= 4. Courier New (10pt Monospace Fixed-Pitch)

#text(font: "Courier New", size: 10pt)[
  Courier New is a fixed-pitch slab-serif typeface where every glyph—including narrow punctuation (`i`, `l`, `.`, `,`) and wide capitals (`M`, `W`)—occupies an identical 1229/2048 em advance (6.0009765625pt at 10pt). Exactly 69 characters fit on a 415.3pt line.
]

= 5. Multi-Size Scaling Check (10pt, 12pt, 14pt)

#text(font: "Times New Roman", size: 10pt)[
  *10pt Times New Roman:* Smaller body size (20 half-points in OOXML `w:sz`) increases character density per line proportionally while scaling vertical line height by 10/11.
]

#text(font: "Times New Roman", size: 12pt)[
  *12pt Times New Roman:* Standard manuscript size (24 half-points in OOXML `w:sz`) reduces words per line and expands vertical baseline distance proportionally.
]

#text(font: "Times New Roman", size: 14pt)[
  *14pt Times New Roman:* Display subheading size (28 half-points in OOXML `w:sz`) verifies larger font scaling and baseline placement.
]
