// @func: enum
// @docs: https://typst.app/docs/reference/model/enum/

// === @case enum_arabic ===
// @title: Numbered Enum: Default Arabic
// @param: children
// @var: enum 1..3
// @size: 320x130
// @ir: Enum
// @xml: <w:numPr>
#set text(font: "Times New Roman", size: 12pt)
+ Step one
+ Step two
+ Step three

// === @case enum_start_5 ===
// @title: Numbered Enum: Custom Start (5)
// @param: start
// @var: enum(start: 5)
// @size: 320x120
// @ir: Enum
#set text(font: "Times New Roman", size: 12pt)
#enum(start: 5)[Fifth entry][Sixth entry]

// === @case enum_nested_pattern ===
// @title: Numbered Enum: Nested Pattern ("1.a.")
// @param: numbering
// @var: enum(numbering: "1.a.")
// @size: 320x150
// @ir: Enum
#set text(font: "Times New Roman", size: 12pt)
#set enum(numbering: "1.a.")
+ Primary
  + Sub-entry A
  + Sub-entry B
