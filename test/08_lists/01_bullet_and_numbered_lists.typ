// Test 08_lists/01_bullet_and_numbered_lists.typ
// Covers: Multi-level unordered bullet lists (`-`), ordered numbered lists (`+`),
// mixed bullet/numbered nesting, and Word's 671 dxa / 300 dxa hanging indent.

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

= 1. Multi-Level Unordered Bullet Lists (`-`)

Each list item below spans multiple wrapped lines to test hanging indentation (`left: 33.55pt = 671 dxa`, `hanging: 15pt = 300 dxa`) and marker alignment:

- *Level 1 Bullet Item A:* The primary document pipeline compiles Typst source files into both a fully typeset `PagedDocument` and an evaluated AST/Content tree for semantic reconstruction.
  - *Level 2 Sub-Bullet A.1:* Nested bullet items increase their left indentation proportionally while preserving the hanging marker offset between the bullet glyph and the text tab stop.
  - *Level 2 Sub-Bullet A.2:* A second nested item verifies consistent inter-item vertical spacing within the same list block.
- *Level 1 Bullet Item B:* The secondary stage serializes `DocumentIr` into a valid OpenXML `.docx` ZIP package with styles, numbering definitions, footnotes, and DrawingML vector graphics.

= 2. Multi-Level Ordered Numbered Lists (`+`)

+ *First Numbered Step:* Parse font metrics (`unitsPerEm`, `hmtx` advance widths, `OS/2` ascender/descender) from system font collections.
+ *Second Numbered Step:* Classify characters according to `WordScriptOverridePolicy` and execute either standard UAX \#14 + Kinsoku line breaking or PTLS7's 22x22 pair table.
  + *Sub-Step 2.1:* Check whether the line overflows the right margin (`415.3pt`).
  + *Sub-Step 2.2:* Backtrack across prohibited pair transitions or apply emergency mid-word truncation if a single token spans the entire line.
+ *Third Numbered Step:* Emit OOXML paragraphs and verify 100% line-by-line parity against Microsoft Word.
