//! Read-only PDF line anchor representation and LCS line-level verification harness.
//!
//! `AnchorLineIr` records ground-truth line boundaries `(page, line_index, first_token,
//! last_token, x_start_pt, baseline_y_pt, width_pt, ends_with_hyphen)` extracted from a
//! reference PDF. It is strictly read-only and never used by the generator; instead,
//! `compare_line_anchors` pairs candidate lines against golden anchors using Longest
//! Common Subsequence (LCS) to diagnose rule discrepancies.

use serde::{Deserialize, Serialize};

/// Read-only verification anchor for a single laid-out text line extracted from a PDF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorLineIr {
    /// 1-based page number.
    pub page: usize,
    /// 0-based line index within the page.
    pub line_index: usize,
    /// First visible non-whitespace token on the line.
    pub first_token: String,
    /// Last visible non-whitespace token on the line.
    pub last_token: String,
    /// Left X coordinate of the line start in `pt`.
    pub x_start_pt: f64,
    /// Baseline (or bottom) Y coordinate of the line in `pt`.
    pub baseline_y_pt: f64,
    /// Total rendered width of the line in `pt`.
    pub width_pt: f64,
    /// Whether the line ends with a soft/hard hyphen (`-` or `\u{00AD}`).
    pub ends_with_hyphen: bool,
}

impl AnchorLineIr {
    /// Construct a new `AnchorLineIr`.
    pub fn new(
        page: usize,
        line_index: usize,
        first_token: impl Into<String>,
        last_token: impl Into<String>,
    ) -> Self {
        let last = last_token.into();
        let ends_with_hyphen = last.ends_with('-') || last.ends_with('\u{00AD}');
        Self {
            page,
            line_index,
            first_token: first_token.into(),
            last_token: last,
            x_start_pt: 0.0,
            baseline_y_pt: 0.0,
            width_pt: 0.0,
            ends_with_hyphen,
        }
    }

    #[inline]
    pub fn with_geometry(mut self, x_start_pt: f64, baseline_y_pt: f64, width_pt: f64) -> Self {
        self.x_start_pt = x_start_pt;
        self.baseline_y_pt = baseline_y_pt;
        self.width_pt = width_pt;
        self
    }

    #[inline]
    pub fn with_hyphen(mut self, ends_with_hyphen: bool) -> Self {
        self.ends_with_hyphen = ends_with_hyphen;
        self
    }

    /// Check whether two anchors have identical line-break boundary tokens and hyphenation parity.
    #[inline]
    pub fn has_same_tokens(&self, other: &Self) -> bool {
        self.first_token == other.first_token
            && self.last_token == other.last_token
            && self.ends_with_hyphen == other.ends_with_hyphen
    }
}

/// Classification of a discrepancy between a golden PDF line anchor and a candidate line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnchorMismatchKind {
    /// Paired lines differ in `first_token`, `last_token`, or `ends_with_hyphen` (line-breaker rule delta).
    TokenBoundaryMismatch {
        golden_first: String,
        golden_last: String,
        candidate_first: String,
        candidate_last: String,
    },
    /// Paired lines have matching boundary tokens but land on different pages (pagination/widow-orphan delta).
    PageBreakMismatch {
        golden_page: usize,
        candidate_page: usize,
    },
    /// Paired lines are on the same page but `|baseline_y_pt|` exceeds tolerance (line-height/spacing delta).
    BaselineShift { delta_y_pt: f64 },
    /// Paired lines are on the same page but `|x_start_pt|` exceeds tolerance (indent/justification delta).
    XStartShift { delta_x_pt: f64 },
    /// A golden line has no counterpart in the candidate output.
    MissingInCandidate,
    /// A candidate line has no counterpart in the golden anchors.
    ExtraInCandidate,
}

/// A single diagnostic entry produced by `compare_line_anchors`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorDiffEntry {
    pub golden_index: Option<usize>,
    pub candidate_index: Option<usize>,
    pub kind: AnchorMismatchKind,
}

/// Summary report from comparing golden PDF line anchors against candidate output.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnchorDiffReport {
    pub total_golden_lines: usize,
    pub total_candidate_lines: usize,
    pub exact_token_matches: usize,
    pub same_page_matches: usize,
    pub token_parity_rate: f64,
    pub max_baseline_delta_pt: f64,
    pub max_x_start_delta_pt: f64,
    pub page_count_delta: isize,
    pub mismatches: Vec<AnchorDiffEntry>,
}

impl AnchorDiffReport {
    /// Whether the candidate output meets the L3 acceptance criteria
    /// (`token_parity_rate >= min_parity_rate` and `page_count_delta == 0`).
    #[inline]
    pub fn passes_acceptance(&self, min_parity_rate: f64) -> bool {
        self.token_parity_rate >= min_parity_rate && self.page_count_delta == 0
    }
}

/// Compare golden PDF line anchors against candidate lines using Longest Common Subsequence (LCS)
/// pairing on `(first_token, last_token)`, falling back to `first_token` similarity.
pub fn compare_line_anchors(
    golden: &[AnchorLineIr],
    candidate: &[AnchorLineIr],
    baseline_tol_pt: f64,
    x_tol_pt: f64,
) -> AnchorDiffReport {
    let n = golden.len();
    let m = candidate.len();
    if n == 0 && m == 0 {
        return AnchorDiffReport {
            token_parity_rate: 1.0,
            ..Default::default()
        };
    }

    // Build LCS DP table over `(golden[i], candidate[j])` where exact `(first_token, last_token)`
    // match scores 2 and partial (`first_token` or `last_token`) match scores 1.
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let score = if golden[i].has_same_tokens(&candidate[j]) {
                2
            } else if golden[i].first_token == candidate[j].first_token
                || golden[i].last_token == candidate[j].last_token
            {
                1
            } else {
                0
            };
            let take = if score > 0 {
                score + dp[i + 1][j + 1]
            } else {
                0
            };
            dp[i][j] = take.max(dp[i + 1][j]).max(dp[i][j + 1]);
        }
    }

    let mut exact_token_matches = 0usize;
    let mut same_page_matches = 0usize;
    let mut max_baseline_delta_pt = 0.0_f64;
    let mut max_x_start_delta_pt = 0.0_f64;
    let mut mismatches = Vec::new();

    let mut i = 0;
    let mut j = 0;
    while i < n && j < m {
        let g = &golden[i];
        let c = &candidate[j];
        let score = if g.has_same_tokens(c) {
            2
        } else if g.first_token == c.first_token || g.last_token == c.last_token {
            1
        } else {
            0
        };
        if score > 0 && dp[i][j] == score + dp[i + 1][j + 1] {
            if g.has_same_tokens(c) {
                exact_token_matches += 1;
            } else {
                mismatches.push(AnchorDiffEntry {
                    golden_index: Some(i),
                    candidate_index: Some(j),
                    kind: AnchorMismatchKind::TokenBoundaryMismatch {
                        golden_first: g.first_token.clone(),
                        golden_last: g.last_token.clone(),
                        candidate_first: c.first_token.clone(),
                        candidate_last: c.last_token.clone(),
                    },
                });
            }
            if g.page == c.page {
                same_page_matches += 1;
                let dy = c.baseline_y_pt - g.baseline_y_pt;
                let dx = c.x_start_pt - g.x_start_pt;
                max_baseline_delta_pt = max_baseline_delta_pt.max(dy.abs());
                max_x_start_delta_pt = max_x_start_delta_pt.max(dx.abs());
                if dy.abs() > baseline_tol_pt {
                    mismatches.push(AnchorDiffEntry {
                        golden_index: Some(i),
                        candidate_index: Some(j),
                        kind: AnchorMismatchKind::BaselineShift { delta_y_pt: dy },
                    });
                }
                if dx.abs() > x_tol_pt {
                    mismatches.push(AnchorDiffEntry {
                        golden_index: Some(i),
                        candidate_index: Some(j),
                        kind: AnchorMismatchKind::XStartShift { delta_x_pt: dx },
                    });
                }
            } else {
                mismatches.push(AnchorDiffEntry {
                    golden_index: Some(i),
                    candidate_index: Some(j),
                    kind: AnchorMismatchKind::PageBreakMismatch {
                        golden_page: g.page,
                        candidate_page: c.page,
                    },
                });
            }
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            mismatches.push(AnchorDiffEntry {
                golden_index: Some(i),
                candidate_index: None,
                kind: AnchorMismatchKind::MissingInCandidate,
            });
            i += 1;
        } else {
            mismatches.push(AnchorDiffEntry {
                golden_index: None,
                candidate_index: Some(j),
                kind: AnchorMismatchKind::ExtraInCandidate,
            });
            j += 1;
        }
    }
    while i < n {
        mismatches.push(AnchorDiffEntry {
            golden_index: Some(i),
            candidate_index: None,
            kind: AnchorMismatchKind::MissingInCandidate,
        });
        i += 1;
    }
    while j < m {
        mismatches.push(AnchorDiffEntry {
            golden_index: None,
            candidate_index: Some(j),
            kind: AnchorMismatchKind::ExtraInCandidate,
        });
        j += 1;
    }

    let golden_max_page = golden.iter().map(|a| a.page).max().unwrap_or(0) as isize;
    let candidate_max_page = candidate.iter().map(|a| a.page).max().unwrap_or(0) as isize;
    let token_parity_rate = if n > 0 {
        exact_token_matches as f64 / n as f64
    } else {
        1.0
    };

    AnchorDiffReport {
        total_golden_lines: n,
        total_candidate_lines: m,
        exact_token_matches,
        same_page_matches,
        token_parity_rate,
        max_baseline_delta_pt,
        max_x_start_delta_pt,
        page_count_delta: candidate_max_page - golden_max_page,
        mismatches,
    }
}
