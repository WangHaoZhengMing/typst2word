// Test 08_lists/02_terms_and_multiline_items.typ
// Covers: Term/definition lists (`/ Term: Definition`), list items containing
// inline math and multiple paragraphs, and CJK list items.

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

= 1. Term and Definition Lists (`/ Term: Description`)

/ `FirstFitBreaker`: Greedy single-pass line-breaking engine that replicates Microsoft Word's left-to-right scan without shrinking inter-word spaces below 100% nominal width.
/ `KnuthPlassBreaker`: Dynamic programming line-breaking engine that minimizes total paragraph demerits across all feasible breakpoint sequences.
/ `Ptls7BreakClass`: The 22-class character classification system extracted from `MicrosoftPTLS7.framework` (`0x1d24e0`) governing `Latin` (`script_id = 5`) pair transitions.
/ `WordScriptOverridePolicy`: Policy enum controlling when Word overrides a font's East Asian script classification due to missing OpenType `GSUB`/`GPOS` script tables.

= 2. List Items with Inline Math and Multi-Paragraph Continuation

+ *Eigenvalue Decomposition:* Given a real symmetric matrix $A in bb(R)^(n times n)$, there exists an orthogonal matrix $Q$ such that $A = Q Lambda Q^T$, where $Lambda = "diag"(lambda_1, dots, lambda_n)$ contains the real eigenvalues of $A$.

  Furthermore, the eigenvectors corresponding to distinct eigenvalues are mutually orthogonal with respect to the standard Euclidean inner product $chevron.l u, v chevron.r = u^T v$.

+ *Singular Value Decomposition (SVD):* Every matrix $M in bb(R)^(m times n)$ admits a factorization $M = U Sigma V^T$, where $U in bb(R)^(m times m)$ and $V in bb(R)^(n times n)$ are orthogonal and $Sigma in bb(R)^(m times n)$ is non-negative diagonal.
