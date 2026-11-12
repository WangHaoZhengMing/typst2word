// @func: list
// @docs: https://typst.app/docs/reference/model/list/

// === @case list_tight ===
// @title: Bullet List: Tight
// @param: tight: true
// @var: list(tight: true)
// @size: 320x130
// @ir: List
// @xml: <w:numPr>
#set text(font: "Times New Roman", size: 12pt)
- First bullet item
- Second bullet item
- Third bullet item

// === @case list_loose ===
// @title: Bullet List: Loose (spacing: 1.4em)
// @param: tight: false + spacing
// @var: list(tight: false, spacing: 1.4em)
// @size: 320x160
// @ir: List
#set text(font: "Times New Roman", size: 12pt)
#set list(tight: false, spacing: 1.4em)
- Loose item alpha

- Loose item beta

- Loose item gamma

// === @case list_nested ===
// @title: Bullet List: 3 Nested Levels
// @param: nested
// @var: nested list levels 0..2
// @size: 320x150
// @ir: List
// @xml: w:ilvl
#set text(font: "Times New Roman", size: 12pt)
- Outer item
  - Middle item
    - Inner item

// === @case list_custom_marker ===
// @title: Bullet List: Custom Marker (‣)
// @param: marker
// @var: list(marker: [‣])
// @size: 320x120
// @ir: List
#set text(font: "Times New Roman", size: 12pt)
#set list(marker: [‣])
- Custom marker one
- Custom marker two
