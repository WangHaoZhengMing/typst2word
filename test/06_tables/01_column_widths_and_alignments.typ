// Test 06_tables/01_column_widths_and_alignments.typ
// Covers: Fixed point widths, fractional (`1fr`, `2fr`) widths, percentage widths,
// and horizontal/vertical cell alignments (`left`, `center`, `right`).

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

= 1. Fixed Point Column Widths and Per-Column Alignment

This table specifies explicit point widths `(95pt, 180pt, 140pt)` (total `415pt`, matching the `415.3pt` text width) and aligns columns left, center, and right respectively.

#table(
  columns: (95pt, 180pt, 140pt),
  align: (left + horizon, center + horizon, right + horizon),
  inset: 6pt,
  stroke: 0.6pt + rgb("#90a4ae"),
  [*Metric ID*], [*Subsystem Description*], [*Measured Value*],
  [`LAT-001`], [Primary Memory Bus Latency], [14.25 ns],
  [`BW-002`], [Sustained L3 Cache Bandwidth], [412.80 GB/s],
  [`IPC-003`], [Out-of-Order Execution Throughput], [4.85 insn/cycle],
  [`PWR-004`], [Thermal Design Power Envelope], [65.00 W],
)

= 2. Fractional Column Widths (`1fr`, `2fr`, `1fr`)

Fractional columns partition the available 415.3pt body width into proportions $(1/4, 2/4, 1/4)$:

#table(
  columns: (1fr, 2fr, 1fr),
  align: (left, left, right),
  inset: 6pt,
  stroke: 0.6pt + rgb("#78909c"),
  [*Phase*], [*Operational Milestone and Deliverable*], [*Target Quarter*],
  [Phase I], [Lexical analyzer, AST parser, and core IR schema stabilization], [2025 Q1],
  [Phase II], [PTLS7 line-breaking state machine and font metric tables], [2025 Q2],
  [Phase III], [DrawingML vector graphics, CeTZ groups, and OMML formulas], [2025 Q3],
)
