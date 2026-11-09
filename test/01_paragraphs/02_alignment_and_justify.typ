// Test 01_paragraphs/02_alignment_and_justify.typ
// Covers: left, center, right, and justified paragraph alignments,
// hanging indent, and left/right block inset margins.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Times New Roman", size: 11pt, lang: "en", hyphenate: false)
#set par(
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. Left-Aligned Paragraph (Ragged Right)

#align(left)[
  #set par(justify: false)
  Left-aligned paragraphs keep inter-word spaces at their natural nominal width (25% of em in Times New Roman, or 2.75pt at 11pt) and leave the right margin ragged. Line break decisions remain identical to justified text under Word's greedy First-Fit algorithm because Word never compresses inter-word spaces below 100% nominal width.
]

= 2. Fully Justified Paragraph

#align(left)[
  #set par(justify: true)
  Fully justified paragraphs distribute any remaining horizontal slack at the end of each non-final line evenly across all inter-word spaces on that line. The final line of the paragraph remains left-aligned at its natural inter-word spacing without artificial stretching.
]

= 3. Center-Aligned Paragraph

#align(center)[
  #set par(justify: false)
  Center-aligned text balances the remaining horizontal slack equally between the left and right margins on each individual line. This layout is frequently used for titles, epigraphs, and figure captions where visual symmetry around the vertical page axis is required.
]

= 4. Right-Aligned Paragraph

#align(right)[
  #set par(justify: false)
  Right-aligned text flushes the end of every line against the right margin at 17.82cm while leaving the left edge ragged. Date headers, signatures, and side-notes rely on exact right-edge alignment.
]

= 5. Hanging Indentation and Block Insets

#block(inset: (left: 24pt, right: 24pt))[
  #set par(justify: true, hanging-indent: 18pt)
  *Hanging Indent (18pt) with 24pt Left/Right Insets:* The first line of this paragraph begins at the 24pt left inset, whereas all wrapped continuation lines are indented an additional 18pt (42pt total from the body margin), matching bibliographic reference formatting in Word.
]
