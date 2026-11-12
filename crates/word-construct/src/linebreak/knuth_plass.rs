//! Typst-compatible `KnuthPlassBreaker` with two-stage approximate upper-bound pruning.
//!
//! Mirrors `typst-layout/src/inline/linebreak.rs`:
//! - `DEFAULT_HYPH_COST` / `TYPST_HYPH_COST = 0.5`
//! - `DEFAULT_RUNT_COST` / `TYPST_RUNT_COST = 0.5`
//! - `TYPST_CONSECUTIVE_DASH_COST = 0.3`
//! - `TYPST_MAX_COST = 1_000_000.0`
//! - `TYPST_MIN_RATIO = -1.0`, `MIN_APPROX_RATIO = -0.5`

use crate::linebreak::{BrokenLine, InlineToken, LineBreakCause, LineBreaker};
use crate::policy::{
    KinsokuSet, TYPST_CONSECUTIVE_DASH_COST, TYPST_HYPH_COST, TYPST_MAX_COST, TYPST_MIN_RATIO,
    TYPST_RUNT_COST,
};

const MIN_APPROX_RATIO: f64 = -0.5;
const BOUND_EPS: f64 = 1e-3;

/// Candidate breakpoint kind in `KnuthPlassBreaker`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateKind {
    Normal,
    Mandatory,
    Hyphen,
}

/// A valid break opportunity at token boundary `end_idx` (`tokens[..end_idx]`).
#[derive(Debug, Clone, Copy)]
struct BreakCandidate {
    end_idx: usize,
    kind: CandidateKind,
}

/// Prefix-sum cumulative estimates for $O(1)$ slice width/stretch/shrink queries.
struct CumulativeEstimates {
    width: Vec<f64>,
    stretch: Vec<f64>,
    shrink: Vec<f64>,
    justifiables: Vec<usize>,
}

impl CumulativeEstimates {
    fn compute(tokens: &[InlineToken]) -> Self {
        let n = tokens.len();
        let mut width = Vec::with_capacity(n + 1);
        let mut stretch = Vec::with_capacity(n + 1);
        let mut shrink = Vec::with_capacity(n + 1);
        let mut justifiables = Vec::with_capacity(n + 1);
        width.push(0.0);
        stretch.push(0.0);
        shrink.push(0.0);
        justifiables.push(0);

        for t in tokens {
            width.push(width.last().copied().unwrap_or(0.0) + t.width_pt);
            stretch.push(stretch.last().copied().unwrap_or(0.0) + t.stretch_pt);
            shrink.push(shrink.last().copied().unwrap_or(0.0) + t.shrink_pt);
            let j_inc = usize::from(!t.is_whitespace() && !t.is_mandatory_break());
            justifiables.push(justifiables.last().copied().unwrap_or(0) + j_inc);
        }

        Self {
            width,
            stretch,
            shrink,
            justifiables,
        }
    }

    /// Return `(line_width, stretchability, shrinkability, justifiables, avg_font_size)`
    /// for `tokens[start..end]`, trimming leading and trailing whitespace.
    fn slice_metrics(
        &self,
        tokens: &[InlineToken],
         mut start: usize,
        mut end: usize,
    ) -> (f64, f64, f64, usize, f64) {
        while start < end
            && (tokens[start].is_whitespace() || tokens[start].is_mandatory_break())
        {
            start += 1;
        }
        while end > start && (tokens[end - 1].is_whitespace() || tokens[end - 1].is_mandatory_break())
        {
            end -= 1;
        }
        if start >= end {
            return (0.0, 0.0, 0.0, 0, 11.0);
        }
        let mut w = self.width[end] - self.width[start];
        let st = self.stretch[end] - self.stretch[start];
        let mut sh = self.shrink[end] - self.shrink[start];
        if tokens[start]
            .text
            .chars()
            .next()
            .is_some_and(crate::linebreak::is_cjk_left_half_space_punct)
        {
            w = (w - tokens[start].shrink_pt).max(0.0);
            sh = (sh - tokens[start].shrink_pt).max(0.0);
        }
        if tokens[end - 1]
            .text
            .chars()
            .next_back()
            .is_some_and(crate::linebreak::is_cjk_right_half_space_punct)
        {
            w = (w - tokens[end - 1].shrink_pt).max(0.0);
            sh = (sh - tokens[end - 1].shrink_pt).max(0.0);
        }
        let just = self.justifiables[end] - self.justifiables[start];
        let fs = tokens[start].font_size_pt.max(6.0);
        (w, st, sh, just, fs)
    }
}

/// Typst-style Knuth-Plass optimal line-breaker with two-stage approximate pruning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnuthPlassBreaker {
    pub hyph_cost: f64,
    pub runt_cost: f64,
    pub consecutive_dash_cost: f64,
    pub max_cost: f64,
    pub min_ratio: f64,
}

impl Default for KnuthPlassBreaker {
    fn default() -> Self {
        Self {
            hyph_cost: TYPST_HYPH_COST,
            runt_cost: TYPST_RUNT_COST,
            consecutive_dash_cost: TYPST_CONSECUTIVE_DASH_COST,
            max_cost: TYPST_MAX_COST,
            min_ratio: TYPST_MIN_RATIO,
        }
    }
}

impl KnuthPlassBreaker {
    #[inline]
    pub const fn new() -> Self {
        Self {
            hyph_cost: TYPST_HYPH_COST,
            runt_cost: TYPST_RUNT_COST,
            consecutive_dash_cost: TYPST_CONSECUTIVE_DASH_COST,
            max_cost: TYPST_MAX_COST,
            min_ratio: TYPST_MIN_RATIO,
        }
    }

    /// Enumerate legal break candidates in `tokens` respecting `KinsokuSet`.
    fn collect_candidates(tokens: &[InlineToken], kinsoku: &KinsokuSet) -> Vec<BreakCandidate> {
        let mut candidates = Vec::new();
        let n = tokens.len();
        for i in 0..n {
            let tok = &tokens[i];
            if tok.is_mandatory_break() {
                candidates.push(BreakCandidate {
                    end_idx: i + 1,
                    kind: CandidateKind::Mandatory,
                });
                continue;
            }
            if tok.is_whitespace() {
                continue;
            }
            // Find next visible token after `i`.
            let mut next_vis = i + 1;
            while next_vis < n && tokens[next_vis].is_whitespace() {
                next_vis += 1;
            }
            if next_vis == n {
                candidates.push(BreakCandidate {
                    end_idx: n,
                    kind: CandidateKind::Mandatory,
                });
                break;
            }
            if tokens[next_vis].is_mandatory_break() {
                // Handled when loop reaches `next_vis`.
                continue;
            }
            // Check Kinsoku constraints:
            // - `tok` cannot end a line if `tok.is_kinsoku_line_end_forbidden(kinsoku)`
            // - `tokens[next_vis]` cannot start a line if `tokens[next_vis].is_kinsoku_line_start_forbidden(kinsoku)`
            if tok.is_kinsoku_line_end_forbidden(kinsoku)
                || tokens[next_vis].is_kinsoku_line_start_forbidden(kinsoku)
            {
                continue;
            }
            let kind = if tok.ends_with_hyphen {
                CandidateKind::Hyphen
            } else {
                CandidateKind::Normal
            };
            candidates.push(BreakCandidate {
                end_idx: i + 1,
                kind,
            });
        }
        if candidates.last().is_none_or(|c| c.end_idx != n) {
            candidates.push(BreakCandidate {
                end_idx: n,
                kind: CandidateKind::Mandatory,
            });
        }
        candidates
    }

    /// Compute stretch/shrink ratio following Typst's `raw_ratio`.
    fn compute_ratio(
        avail_width_pt: f64,
        line_width_pt: f64,
        stretch_pt: f64,
        shrink_pt: f64,
        justifiables: usize,
        font_size_pt: f64,
        min_ratio: f64,
    ) -> f64 {
        let mut delta = avail_width_pt - line_width_pt;
        if delta.abs() < 1e-4 {
            delta = 0.0;
        }
        let adjustability = if delta >= 0.0 { stretch_pt } else { shrink_pt };
        let mut ratio = if adjustability > 1e-6 {
            delta / adjustability
        } else if delta == 0.0 {
            0.0
        } else if delta > 0.0 {
            2.0
        } else {
            min_ratio - 1.0
        };
        if ratio.is_nan() {
            ratio = 0.0;
        }
        if ratio > 1.0 {
            let extra_stretch = (delta - adjustability.max(0.0)) / (justifiables.max(1) as f64);
            ratio = 1.0 + extra_stretch / (font_size_pt / 2.0).max(1.0);
        }
        ratio.clamp(min_ratio - 1.0, 10.0)
    }

    /// Compute line cost following Typst's `raw_cost`.
    #[allow(clippy::too_many_arguments)]
    fn compute_cost(
        &self,
        kind: CandidateKind,
        ratio: f64,
        justify: bool,
        unbreakable: bool,
        consecutive_dash: bool,
        approx: bool,
    ) -> f64 {
        let min_r = if approx {
            self.min_ratio.max(MIN_APPROX_RATIO)
        } else {
            self.min_ratio
        };
        let badness = if ratio < min_r {
            self.max_cost
        } else if kind != CandidateKind::Mandatory || justify || ratio < 0.0 {
            100.0 * ratio.abs().powi(3)
        } else {
            0.0
        };

        let mut penalty = 0.0;
        // Scale normalized Typst cost constants (0.5 / 0.3) into Typst's internal 100-scale penalty domain.
        let hyph_pen = self.hyph_cost * 270.0;
        let runt_pen = self.runt_cost * 200.0;
        let consec_pen = self.consecutive_dash_cost * 450.0;

        if unbreakable && kind == CandidateKind::Mandatory {
            penalty += runt_pen;
        }
        if kind == CandidateKind::Hyphen {
            penalty += hyph_pen;
        }
        if consecutive_dash {
            penalty += consec_pen;
        }

        (1.0 + badness + penalty).powi(2)
    }

    /// Stage 1: Approximate DP pass (`MIN_APPROX_RATIO`) to obtain a tight upper bound for pruning.
    fn estimate_upper_bound(
        &self,
        tokens: &[InlineToken],
        candidates: &[BreakCandidate],
        estimates: &CumulativeEstimates,
        line0_max_w: f64,
        rest_max_w: f64,
        justify: bool,
    ) -> f64 {
        #[derive(Clone, Copy)]
        struct ApproxEntry {
            pred: usize,
            total: f64,
            end_idx: usize,
            kind: CandidateKind,
            unbreakable: bool,
        }

        let mut table = vec![ApproxEntry {
            pred: 0,
            total: 0.0,
            end_idx: 0,
            kind: CandidateKind::Mandatory,
            unbreakable: false,
        }];
        let mut active = 0usize;
        let mut prev_end = 0usize;

        for &cand in candidates {
            let mut best: Option<ApproxEntry> = None;
            for (pred_idx, pred) in table.iter().enumerate().skip(active) {
                let start = pred.end_idx;
                let line_avail = if start == 0 { line0_max_w } else { rest_max_w };
                let unbreakable = prev_end == start;
                let line_justify = justify && cand.kind != CandidateKind::Mandatory;
                let consecutive_dash =
                    pred.kind == CandidateKind::Hyphen && cand.kind == CandidateKind::Hyphen;
                let (w, st, sh, just, fs) = estimates.slice_metrics(tokens, start, cand.end_idx);
                let ratio = Self::compute_ratio(line_avail, w, st, sh, just, fs, self.min_ratio);
                let cost = self.compute_cost(
                    cand.kind,
                    ratio,
                    line_justify,
                    unbreakable,
                    consecutive_dash,
                    true,
                );
                if ratio < self.min_ratio && active == pred_idx {
                    active += 1;
                }
                let total = pred.total + cost;
                if best.is_none_or(|b| b.total >= total) {
                    best = Some(ApproxEntry {
                        pred: pred_idx,
                        total,
                        end_idx: cand.end_idx,
                        kind: cand.kind,
                        unbreakable,
                    });
                }
            }
            if cand.kind == CandidateKind::Mandatory {
                active = table.len();
            }
            if let Some(b) = best {
                table.push(b);
            }
            prev_end = cand.end_idx;
        }

        let mut indices = Vec::new();
        let mut idx = table.len() - 1;
        while idx != 0 {
            indices.push(idx);
            idx = table[idx].pred;
        }

        let mut exact_total = 0.0;
        let mut start = 0usize;
        let mut prev_kind = CandidateKind::Mandatory;
        for i in indices.into_iter().rev() {
            let entry = table[i];
            let line_avail = if start == 0 { line0_max_w } else { rest_max_w };
            let (w, st, sh, just, fs) = estimates.slice_metrics(tokens, start, entry.end_idx);
            let ratio = Self::compute_ratio(line_avail, w, st, sh, just, fs, self.min_ratio);
            if ratio < self.min_ratio {
                return f64::INFINITY;
            }
            let line_justify = justify && entry.kind != CandidateKind::Mandatory;
            let consecutive_dash =
                prev_kind == CandidateKind::Hyphen && entry.kind == CandidateKind::Hyphen;
            exact_total += self.compute_cost(
                entry.kind,
                ratio,
                line_justify,
                entry.unbreakable,
                consecutive_dash,
                false,
            );
            start = entry.end_idx;
            prev_kind = entry.kind;
        }
        exact_total
    }

    /// Break `tokens` with explicit first-line and hanging indents (`pt`).
    pub fn break_tokens_with_indents(
        &self,
        tokens: &[InlineToken],
        avail_width_pt: f64,
        first_line_indent_pt: f64,
        hanging_indent_pt: f64,
        justify: bool,
        kinsoku: &KinsokuSet,
    ) -> Vec<BrokenLine> {
        if tokens.is_empty() {
            return Vec::new();
        }
        let line0_max_w = (avail_width_pt - first_line_indent_pt).max(1.0);
        let rest_max_w = (avail_width_pt - hanging_indent_pt).max(1.0);
        let candidates = Self::collect_candidates(tokens, kinsoku);
        let estimates = CumulativeEstimates::compute(tokens);
        let upper_bound = self.estimate_upper_bound(
            tokens,
            &candidates,
            &estimates,
            line0_max_w,
            rest_max_w,
            justify,
        );

        #[derive(Clone, Copy)]
        struct DpEntry {
            pred: usize,
            total: f64,
            end_idx: usize,
            kind: CandidateKind,
            ratio: f64,
            width_pt: f64,
        }

        let mut table = vec![DpEntry {
            pred: 0,
            total: 0.0,
            end_idx: 0,
            kind: CandidateKind::Mandatory,
            ratio: 0.0,
            width_pt: 0.0,
        }];
        let mut active = 0usize;
        let mut prev_end = 0usize;

        for &cand in &candidates {
            let mut best: Option<DpEntry> = None;
            let mut line_lower_bound: Option<f64> = None;

            for (pred_idx, pred) in table.iter().enumerate().skip(active) {
                let start = pred.end_idx;
                let line_avail = if start == 0 { line0_max_w } else { rest_max_w };
                let unbreakable = prev_end == start;

                if line_lower_bound
                    .is_some_and(|lower| pred.total + lower > upper_bound + BOUND_EPS)
                {
                    continue;
                }

                let line_justify = justify && cand.kind != CandidateKind::Mandatory;
                let consecutive_dash =
                    pred.kind == CandidateKind::Hyphen && cand.kind == CandidateKind::Hyphen;
                let (w, st, sh, just, fs) = estimates.slice_metrics(tokens, start, cand.end_idx);
                let ratio = Self::compute_ratio(line_avail, w, st, sh, just, fs, self.min_ratio);
                let cost = self.compute_cost(
                    cand.kind,
                    ratio,
                    line_justify,
                    unbreakable,
                    consecutive_dash,
                    false,
                );

                if ratio < self.min_ratio && active == pred_idx {
                    active += 1;
                }
                let total = pred.total + cost;
                if ratio > 0.0 && line_lower_bound.is_none() {
                    line_lower_bound = Some(cost);
                }
                if total > upper_bound + BOUND_EPS && best.is_some() {
                    continue;
                }
                if best.is_none_or(|b| b.total >= total) {
                    best = Some(DpEntry {
                        pred: pred_idx,
                        total,
                        end_idx: cand.end_idx,
                        kind: cand.kind,
                        ratio,
                        width_pt: w,
                    });
                }
            }

            if cand.kind == CandidateKind::Mandatory {
                active = table.len();
            }
            if let Some(b) = best {
                table.push(b);
            }
            prev_end = cand.end_idx;
        }

        let mut chosen = Vec::new();
        let mut idx = table.len() - 1;
        while idx != 0 {
            let entry = table[idx];
            chosen.push((table[entry.pred].end_idx, entry));
            idx = entry.pred;
        }
        chosen.reverse();

        let mut lines = Vec::with_capacity(chosen.len());
        for (start, entry) in chosen {
            let mut slice: Vec<InlineToken> = tokens[start..entry.end_idx]
                .iter()
                .skip_while(|t| t.is_whitespace())
                .filter(|t| !t.is_mandatory_break())
                .cloned()
                .collect();
            while slice.last().is_some_and(|t| t.is_whitespace()) {
                slice.pop();
            }
            lines.push(BrokenLine {
                tokens: slice,
                width_pt: entry.width_pt,
                x_start_pt: 0.0,
                stretch_ratio: if (justify && entry.kind != CandidateKind::Mandatory)
                    || entry.ratio < 0.0
                {
                    entry.ratio
                } else {
                    0.0
                },
                ends_with_hyphen: entry.kind == CandidateKind::Hyphen,
                ended_by_mandatory_break: entry.kind == CandidateKind::Mandatory,
                break_cause: if entry.kind == CandidateKind::Mandatory {
                    LineBreakCause::MandatoryBreak
                } else {
                    LineBreakCause::NormalWrap
                },
            });
        }
        lines
    }
}

impl LineBreaker for KnuthPlassBreaker {
    fn break_tokens(
        &self,
        tokens: &[InlineToken],
        avail_width_pt: f64,
        justify: bool,
        kinsoku: &KinsokuSet,
    ) -> Vec<BrokenLine> {
        self.break_tokens_with_indents(tokens, avail_width_pt, 0.0, 0.0, justify, kinsoku)
    }
}
