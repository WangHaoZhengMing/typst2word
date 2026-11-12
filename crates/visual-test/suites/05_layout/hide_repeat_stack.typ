// @func: stack
// @docs: https://typst.app/docs/reference/layout/stack/

// === @case layout_hide_span ===
// @title: Hidden Content Placeholder (#hide)
// @param: hide
// @var: hide[...]
// @size: 300x95
// @ir: Paragraph
#set text(font: "Times New Roman", size: 12pt)
Visible #hide[secret] tail.

// === @case layout_stack_horizontal ===
// @title: Horizontal Stack (dir: ltr, spacing: 12pt)
// @param: dir: ltr + spacing
// @var: stack(dir: ltr, spacing: 12pt)
// @size: 320x95
#set text(font: "Times New Roman", size: 12pt)
#stack(dir: ltr, spacing: 12pt, [Alpha], [Beta], [Gamma])
