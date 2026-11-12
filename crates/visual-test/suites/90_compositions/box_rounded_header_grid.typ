// @func: box
// @docs: https://typst.app/docs/reference/layout/box/

// === @case box_rounded_header_grid ===
// @title: Rounded Box Card: Title + 2-Col Metadata Grid
// @param: radius + stroke + align(center)[grid(1fr, auto)]
// @var: box(radius: 6pt, stroke: 2pt, inset: 1em)[align(center)[text + grid(1fr, auto)]]
// @size: 440x115
// @ir: ShapeBlock
// @xml: roundRect | <a:spAutoFit/> | <w:tbl>
#set text(font: ("New Computer Modern", "PingFang SC"), size: 11pt)
#box(radius: 6pt, stroke: 2pt, inset: 1em)[
  #align(center)[
    #text(17pt, weight: "bold")[Homework 2]
    #grid(
      columns: (1fr, auto),
      gutter: 1em,
      align: left,
      [*Course*: Algorithm Design and Analysis], [*Due Date*: 2025/3/23],
      [*Instructor*: Lei Li], [*Semester*: Spring 2025],
    )
  ]
]
