//! Word-compatible greedy `FirstFitBreaker` with CJK `KinsokuSet` (避头尾) post-processing
//! and PTLS7 emergency mid-word character truncation (`ForcedBreak`).

use crate::linebreak::{BrokenLine, InlineToken, LineBreakCause, LineBreaker, TokenKind};
use crate::policy::KinsokuSet;

/// Microsoft Word's greedy first-fit line-breaker with kinsoku (line-start/line-end forbidden)
/// backtracking and PTLS7 emergency mid-word character truncation.
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstFitBreaker;

impl FirstFitBreaker {
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    fn finalize_line(
        mut tokens: Vec<InlineToken>,
        avail_width_pt: f64,
        justify: bool,
        break_cause: LineBreakCause,
        is_last_line: bool,
    ) -> BrokenLine {
        while tokens.last().is_some_and(|t| t.is_whitespace()) {
            tokens.pop();
        }
        let width_pt: f64 = tokens.iter().map(|t| t.width_pt).sum();
        let stretch_sum: f64 = tokens.iter().map(|t| t.stretch_pt).sum();
        let ends_with_hyphen = tokens
            .iter()
            .rev()
            .find(|t| !t.is_whitespace() && !t.is_mandatory_break())
            .is_some_and(|t| t.ends_with_hyphen);
        let ended_by_mandatory_break = matches!(break_cause, LineBreakCause::MandatoryBreak);
        let stretch_ratio =
            if justify && !ended_by_mandatory_break && !is_last_line && stretch_sum > 1e-6 {
                ((avail_width_pt - width_pt) / stretch_sum).clamp(-1.0, 10.0)
            } else {
                0.0
            };
        BrokenLine {
            tokens,
            width_pt,
            x_start_pt: 0.0,
            stretch_ratio,
            ends_with_hyphen,
            ended_by_mandatory_break,
            break_cause,
        }
    }

    /// Place `tok` at the start of a line (possibly after Kinsoku-carried tokens in `current`),
    /// performing PTLS7 emergency mid-word character truncation (`ForcedBreak`) if `tok` is an
    /// unbreakable [`TokenKind::Word`] that still exceeds `max_w`.
    fn push_with_emergency_truncation(
        tok: InlineToken,
        current: &mut Vec<InlineToken>,
        current_w: &mut f64,
        lines: &mut Vec<BrokenLine>,
        max_w: f64,
        justify: bool,
    ) {
        let mut rem = tok;
        loop {
            let avail_rem = (max_w - *current_w).max(1.0);
            if *current_w + rem.width_pt <= max_w + 1e-4 || !matches!(rem.kind, TokenKind::Word) {
                *current_w += rem.width_pt;
                current.push(rem);
                break;
            }
            let (head, tail_opt) = rem.split_at_width_limit(avail_rem);
            if let Some(tail) = tail_opt {
                current.push(head);
                lines.push(Self::finalize_line(
                    std::mem::take(current),
                    max_w,
                    justify,
                    LineBreakCause::EmergencyMidWordTruncation,
                    false,
                ));
                *current_w = 0.0;
                rem = tail;
            } else {
                *current_w += head.width_pt;
                current.push(head);
                break;
            }
        }
    }
}

impl LineBreaker for FirstFitBreaker {
    fn break_tokens(
        &self,
        tokens: &[InlineToken],
        avail_width_pt: f64,
        justify: bool,
        kinsoku: &KinsokuSet,
    ) -> Vec<BrokenLine> {
        if tokens.is_empty() {
            return Vec::new();
        }
        let max_w = avail_width_pt.max(1.0);
        let mut lines: Vec<BrokenLine> = Vec::new();
        let mut current: Vec<InlineToken> = Vec::new();
        let mut current_w = 0.0_f64;

        for tok in tokens {
            if tok.is_mandatory_break() {
                lines.push(Self::finalize_line(
                    std::mem::take(&mut current),
                    max_w,
                    justify,
                    LineBreakCause::MandatoryBreak,
                    false,
                ));
                current_w = 0.0;
                continue;
            }

            // Skip leading whitespace at the beginning of a wrapped line.
            if current.is_empty() && tok.is_whitespace() {
                continue;
            }

            // Trailing whitespace is allowed to hang without triggering a wrap.
            if tok.is_whitespace() {
                current.push(tok.clone());
                current_w += tok.width_pt;
                continue;
            }

            if current.is_empty() {
                Self::push_with_emergency_truncation(
                    tok.clone(),
                    &mut current,
                    &mut current_w,
                    &mut lines,
                    max_w,
                    justify,
                );
            } else if current_w + tok.width_pt <= max_w + 1e-4 {
                current.push(tok.clone());
                current_w += tok.width_pt;
            } else {
                // Trim trailing whitespace before checking kinsoku constraints.
                while current.last().is_some_and(|t| t.is_whitespace()) {
                    current.pop();
                }

                let mut carried: Vec<InlineToken> = Vec::new();

                // 1. Kinsoku line-end forbidden check: if the last token on `current` is an opening
                // bracket/quote (`（`, `“`, `《`, etc.) and `current` has at least 2 tokens, move it
                // to the start of the next line.
                while current.len() >= 2
                    && current
                        .last()
                        .is_some_and(|last_t| last_t.is_kinsoku_line_end_forbidden(kinsoku))
                {
                    if let Some(popped) = current.pop() {
                        carried.insert(0, popped);
                    }
                }

                // 2. Kinsoku line-start forbidden check: if the first token of the new line (either
                // `carried[0]` or `tok`) is forbidden at line start (`，`, `。`, `）`, etc.) and
                // `current` still has >= 2 visible tokens, backtrack one token from `current`.
                let next_first_forbidden = carried
                    .first()
                    .unwrap_or(tok)
                    .is_kinsoku_line_start_forbidden(kinsoku);
                if next_first_forbidden && current.len() >= 2 {
                    while current.last().is_some_and(|t| t.is_whitespace()) {
                        current.pop();
                    }
                    if current.len() >= 2
                        && let Some(popped) = current.pop()
                    {
                        carried.insert(0, popped);
                    }
                }

                lines.push(Self::finalize_line(
                    std::mem::take(&mut current),
                    max_w,
                    justify,
                    LineBreakCause::NormalWrap,
                    false,
                ));

                current_w = carried.iter().map(|t| t.width_pt).sum();
                current = carried;
                Self::push_with_emergency_truncation(
                    tok.clone(),
                    &mut current,
                    &mut current_w,
                    &mut lines,
                    max_w,
                    justify,
                );
            }
        }

        if !current.is_empty() {
            lines.push(Self::finalize_line(
                current,
                max_w,
                justify,
                LineBreakCause::NormalWrap,
                true,
            ));
        }

        lines
    }
}
