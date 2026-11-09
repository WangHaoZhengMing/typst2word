// Test 02_linebreaking/01_english_first_fit_vs_knuth_plass.typ
// Covers: Pure English greedy First-Fit line breaking vs Knuth-Plass,
// hyphenated compound words, slashes, long words, and punctuation attachment.

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

= 1. Greedy First-Fit Line Breaking in Standard English Prose

In contemporary digital typography, the fundamental distinction between batch-oriented global optimization and interactive document processing manifests most clearly in paragraph line-breaking algorithms. Donald Knuth and Michael Plass formulated dynamic programming line breaking in 1981 to minimize the sum of cubed badness across an entire paragraph simultaneously, allowing inter-word spaces to shrink down to two-thirds of their nominal width when doing so prevents an overly loose line later in the paragraph.

By contrast, Microsoft Word's PTS (`MicrosoftPTLS7`) engine executes a strict greedy First-Fit scan from left to right. Every space character in Times New Roman has an exact nominal width of 2048 / 4 = 512 font design units (0.25em = 2.75pt at 11pt), and Word never compresses inter-word spaces below 100% of this nominal advance during standard justified layout.

= 2. Explicit Hyphens and Compound Terminology

Self-calibrating, high-precision, state-of-the-art electro-optical interferometers rely on ultra-stable frequency-stabilized laser sources and low-noise photodetectors. When a compound hyphenated term such as `first-fit-compatible` or `cross-platform-verified` crosses the right margin at 415.3pt, Word breaks directly after the explicit hyphen (`U+002D`) while keeping the hyphen attached to the preceding sub-word.

= 3. Parentheses, Quotes, Commas, and Semicolons

Punctuation attachment rules prohibit breaking before closing delimiters (closing parentheses, right brackets, commas, periods, semicolons, colons, exclamation marks, and question marks) or after opening delimiters. For example, in parenthetical citations (Smith, 2021; Jones, 2022; Williams, 2023; Anderson, 2024; Henderson, 2025; Richardson, 2026), each semicolon and closing parenthesis remains bound to its preceding token.
