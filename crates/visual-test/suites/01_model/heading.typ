// @func: heading
// @docs: https://typst.app/docs/reference/model/heading/

// === @case heading_l1 ===
// @title: Heading Level 1
// @param: level: 1
// @var: = Heading 1
// @size: 360x100
// @ir: Heading
// @xml: Heading1
#set text(font: "Times New Roman", size: 12pt)
= Primary Section

// === @case heading_l2 ===
// @title: Heading Level 2
// @param: level: 2
// @var: == Heading 2
// @size: 360x100
// @ir: Heading
// @xml: Heading2
#set text(font: "Times New Roman", size: 12pt)
== Secondary Section

// === @case heading_l3 ===
// @title: Heading Level 3
// @param: level: 3
// @var: === Heading 3
// @size: 360x100
// @ir: Heading
// @xml: Heading3
#set text(font: "Times New Roman", size: 12pt)
=== Tertiary Section

// === @case heading_l4 ===
// @title: Heading Level 4
// @param: level: 4
// @var: ==== Heading 4
// @size: 360x100
// @ir: Heading
// @xml: Heading4
#set text(font: "Times New Roman", size: 12pt)
==== Quaternary Section

// === @case heading_numbered ===
// @title: Heading Numbering ("1.1")
// @param: numbering
// @var: heading(numbering: "1.1")
// @size: 360x130
// @ir: Heading
// @xml: 1.1
#set text(font: "Times New Roman", size: 12pt)
#set heading(numbering: "1.1")
= Architecture
== Pipeline

// === @case heading_centered ===
// @title: Centered Heading Show Rule
// @param: show + align
// @var: show heading: set align(center)
// @size: 360x100
// @ir: Heading
// @xml: w:val="center"
#set text(font: "Times New Roman", size: 12pt)
#show heading: set align(center)
= Centered Heading

// === @case heading_cjk ===
// @title: CJK Heading (Heiti SC)
// @param: lang: zh
// @var: CJK = 一、实验目的
// @size: 360x100
// @ir: Heading
// @xml: Heiti SC
#set text(font: "Heiti SC", size: 12pt, lang: "zh")
= 一、实验目的与要求
