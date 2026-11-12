// @func: page
// @docs: https://typst.app/docs/reference/layout/page/

// === @case section_landscape_switch ===
// @title: Section Geometry Switch (Portrait -> Landscape)
// @param: width + height + flipped
// @var: set page(width: 280pt, height: 160pt, margin: 10pt)
// @raw_page: 200x240
// @margins: 10, 10, 10, 10
// @section_margins: [10, 10, 10, 10]; [10, 10, 10, 10]
// @ir: Paragraph
// @xml: w:orient="landscape"
#set page(width: 200pt, height: 240pt, margin: 10pt)
#set text(font: "Times New Roman", size: 11pt)
Portrait section.
#set page(width: 280pt, height: 160pt, margin: 10pt)
Landscape section.

// === @case page_fill_tinted ===
// @title: Page Background Fill Color
// @param: fill
// @var: page(fill: rgb("#FEFCE8"))
// @raw_page: 280x110
// @margins: 10, 10, 10, 10
// @ir: Paragraph
// @xml: w:background w:color="FEFCE8"
#set page(width: 280pt, height: 110pt, margin: 10pt, fill: rgb("#FEFCE8"))
#set text(font: "Times New Roman", size: 11pt)
Warm tinted page background.
