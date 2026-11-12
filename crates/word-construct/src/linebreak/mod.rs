//! Line-breaking engine supporting both Word's greedy `FirstFit` + `KinsokuSet`
//! (including PTLS7 `0x10044afdc` `'DFLT'` `script_id = 5` override and emergency mid-word
//! character truncation) and Typst's `KnuthPlass` dynamic programming optimizer.

pub mod first_fit;
pub mod knuth_plass;
pub mod ptls7;

use serde::{Deserialize, Serialize};

use crate::anchor::AnchorLineIr;
use crate::font_metrics::{
    FontMetricsProvider, WordFontMetricsProvider, is_cjk_or_fullwidth_char,
};
use crate::policy::{KinsokuSet, LineBreakPolicy};

pub use first_fit::FirstFitBreaker;
pub use knuth_plass::KnuthPlassBreaker;
pub use ptls7::{
    LineBreakCause, Ptls7BreakClass, Ptls7PairAction, WordScriptOverridePolicy,
    is_aat_only_cjk_font_triggering_dflt_override,
};

/// Semantic kind of an inline token during line-breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    /// A single CJK ideograph or fullwidth character (breakable before/after unless forbidden by `KinsokuSet`).
    CjkChar,
    /// A contiguous non-whitespace Latin/numeric word or (`script_id = 5` `'DFLT'` overridden) symbol cluster.
    Word,
    /// Stretchable/shrinkable inter-word whitespace (trimmed to zero width at the end of a wrapped line).
    Whitespace,
    /// Explicit mandatory line break (`\n` or Typst `\`).
    MandatoryBreak,
    /// Inline atomic box or math formula that cannot break internally.
    InlineObject,
}

/// An inline run passed into the tokenizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineSpan {
    pub text: String,
    pub font_family: Option<String>,
    pub font_size_pt: f64,
    pub explicit_width_pt: Option<f64>,
}

impl InlineSpan {
    pub fn new(text: impl Into<String>, font_size_pt: f64) -> Self {
        Self {
            text: text.into(),
            font_family: None,
            font_size_pt,
            explicit_width_pt: None,
        }
    }

    #[inline]
    pub fn with_font(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    #[inline]
    pub fn with_explicit_width(mut self, width_pt: f64) -> Self {
        self.explicit_width_pt = Some(width_pt);
        self
    }
}

/// A tokenized unit ready for `FirstFitBreaker` or `KnuthPlassBreaker`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineToken {
    pub text: String,
    pub width_pt: f64,
    pub stretch_pt: f64,
    pub shrink_pt: f64,
    pub font_size_pt: f64,
    pub font_family: Option<String>,
    pub kind: TokenKind,
    pub ends_with_hyphen: bool,
    /// Per-character advance widths (`pt`) for multi-character [`TokenKind::Word`] tokens,
    /// used by PTLS7 emergency mid-word character truncation (`ForcedBreak`) when a single
    /// unbreakable token at `x = 0` exceeds `avail_width_pt`.
    #[serde(default)]
    pub char_widths_pt: Vec<f64>,
}

impl InlineToken {
    /// Check if this token starts with a character forbidden at the start of a line.
    ///
    /// Note: Under Word's `0x10044afdc` OpenType `'DFLT'` (`script_id = 5`, `Latin`) override,
    /// CJK characters and punctuation are collapsed into [`TokenKind::Word`] (PTLS7 class `0`),
    /// where Kinsoku backtracking is bypassed.
    #[inline]
    pub fn is_kinsoku_line_start_forbidden(&self, kinsoku: &KinsokuSet) -> bool {
        matches!(self.kind, TokenKind::CjkChar)
            && self
                .text
                .chars()
                .next()
                .is_some_and(|ch| kinsoku.is_line_start_forbidden(ch))
    }

    /// Check if this token ends with a character forbidden at the end of a line.
    #[inline]
    pub fn is_kinsoku_line_end_forbidden(&self, kinsoku: &KinsokuSet) -> bool {
        matches!(self.kind, TokenKind::CjkChar)
            && self
                .text
                .chars()
                .next_back()
                .is_some_and(|ch| kinsoku.is_line_end_forbidden(ch))
    }

    #[inline]
    pub fn is_whitespace(&self) -> bool {
        matches!(self.kind, TokenKind::Whitespace)
    }

    #[inline]
    pub fn is_mandatory_break(&self) -> bool {
        matches!(self.kind, TokenKind::MandatoryBreak)
    }

    /// Slice an oversized [`TokenKind::Word`] at the maximal character prefix that fits within
    /// `max_width_pt` (always taking at least 1 character to guarantee forward progress),
    /// implementing PTLS7's emergency mid-word character truncation (`ForcedBreak`).
    pub fn split_at_width_limit(&self, max_width_pt: f64) -> (Self, Option<Self>) {
        let chars: Vec<char> = self.text.chars().collect();
        if chars.len() <= 1 || self.width_pt <= max_width_pt + 1e-4 {
            return (self.clone(), None);
        }

        let fallback_provider = WordFontMetricsProvider::default();
        let resolved_widths: Vec<f64> = if self.char_widths_pt.len() == chars.len() {
            self.char_widths_pt.clone()
        } else {
            let raw: Vec<f64> = chars
                .iter()
                .map(|&ch| {
                    fallback_provider.char_advance_pt(
                        self.font_family.as_deref(),
                        self.font_size_pt,
                        ch,
                    )
                })
                .collect();
            let raw_sum: f64 = raw.iter().sum();
            if raw_sum > 1e-6 {
                let scale = self.width_pt / raw_sum;
                raw.into_iter().map(|w| w * scale).collect()
            } else {
                vec![self.width_pt / (chars.len() as f64); chars.len()]
            }
        };

        let mut acc_w = 0.0_f64;
        let mut split_idx = 0_usize;
        for (i, &cw) in resolved_widths.iter().enumerate() {
            if i > 0 && acc_w + cw > max_width_pt + 1e-4 {
                break;
            }
            acc_w += cw;
            split_idx = i + 1;
        }

        if split_idx >= chars.len() {
            return (self.clone(), None);
        }

        let head_text: String = chars[..split_idx].iter().collect();
        let tail_text: String = chars[split_idx..].iter().collect();
        let head_widths = resolved_widths[..split_idx].to_vec();
        let tail_widths = resolved_widths[split_idx..].to_vec();
        let head_w: f64 = head_widths.iter().sum();
        let tail_w: f64 = tail_widths.iter().sum();

        let head = Self {
            text: head_text,
            width_pt: head_w,
            stretch_pt: 0.0,
            shrink_pt: 0.0,
            font_size_pt: self.font_size_pt,
            font_family: self.font_family.clone(),
            kind: self.kind,
            ends_with_hyphen: false,
            char_widths_pt: head_widths,
        };
        let tail = Self {
            text: tail_text,
            width_pt: tail_w,
            stretch_pt: self.stretch_pt,
            shrink_pt: self.shrink_pt,
            font_size_pt: self.font_size_pt,
            font_family: self.font_family.clone(),
            kind: self.kind,
            ends_with_hyphen: self.ends_with_hyphen,
            char_widths_pt: tail_widths,
        };
        (head, Some(tail))
    }
}

/// A single broken line output by a `LineBreaker`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BrokenLine {
    pub tokens: Vec<InlineToken>,
    pub width_pt: f64,
    pub x_start_pt: f64,
    pub stretch_ratio: f64,
    pub ends_with_hyphen: bool,
    pub ended_by_mandatory_break: bool,
    /// Semantic cause of how this line terminated (`NormalWrap`, `MandatoryBreak`, or
    /// `EmergencyMidWordTruncation`).
    #[serde(default)]
    pub break_cause: LineBreakCause,
}

impl BrokenLine {
    /// First visible (non-whitespace, non-break) token text on this line.
    pub fn first_token(&self) -> &str {
        self.tokens
            .iter()
            .find(|t| !t.is_whitespace() && !t.is_mandatory_break())
            .map(|t| t.text.as_str())
            .unwrap_or("")
    }

    /// Last visible (non-whitespace, non-break) token text on this line.
    pub fn last_token(&self) -> &str {
        self.tokens
            .iter()
            .rev()
            .find(|t| !t.is_whitespace() && !t.is_mandatory_break())
            .map(|t| t.text.as_str())
            .unwrap_or("")
    }

    /// Concatenate all visible text on this line (trimming trailing whitespace).
    pub fn visible_text(&self) -> String {
        let last_vis = self
            .tokens
            .iter()
            .rposition(|t| !t.is_whitespace() && !t.is_mandatory_break())
            .map(|idx| idx + 1)
            .unwrap_or(0);
        self.tokens[..last_vis]
            .iter()
            .filter(|t| !t.is_mandatory_break())
            .map(|t| t.text.as_str())
            .collect()
    }

    /// Check whether this line was broken via PTLS7 emergency mid-word character truncation (`ForcedBreak`).
    #[inline]
    pub const fn is_emergency_truncated(&self) -> bool {
        matches!(self.break_cause, LineBreakCause::EmergencyMidWordTruncation)
    }

    /// Convert this line into a read-only `AnchorLineIr` for verification tests.
    pub fn to_anchor(&self, page: usize, line_index: usize, baseline_y_pt: f64) -> AnchorLineIr {
        AnchorLineIr::new(page, line_index, self.first_token(), self.last_token())
            .with_geometry(self.x_start_pt, baseline_y_pt, self.width_pt)
            .with_hyphen(self.ends_with_hyphen)
    }
}

/// Common interface for line-breaking engines (`FirstFitBreaker`, `KnuthPlassBreaker`).
pub trait LineBreaker {
    fn break_tokens(
        &self,
        tokens: &[InlineToken],
        avail_width_pt: f64,
        justify: bool,
        kinsoku: &KinsokuSet,
    ) -> Vec<BrokenLine>;
}

/// Tokenize a slice of `InlineSpan`s into `InlineToken`s under [`WordScriptOverridePolicy::StandardEastAsian`].
#[inline]
pub fn tokenize_spans(spans: &[InlineSpan], metrics: &dyn FontMetricsProvider) -> Vec<InlineToken> {
    tokenize_spans_with_script_policy(spans, metrics, WordScriptOverridePolicy::StandardEastAsian)
}

/// Tokenize a slice of `InlineSpan`s into `InlineToken`s according to `script_policy`.
///
/// - Under [`WordScriptOverridePolicy::StandardEastAsian`], each CJK ideograph or fullwidth symbol
///   becomes a breakable [`TokenKind::CjkChar`].
/// - Under [`WordScriptOverridePolicy::OpenTypeDfltLatinOverride`] (or [`WordScriptOverridePolicy::AutoDetectFont`]
///   when the span uses an AAT-only CJK font such as `"Songti SC"`), Word's `0x10044afdc` override sets
///   `plsrun->script_id = 5` (`Latin`) and `ot_script_tag = 'DFLT'`, causing PTLS7 (`0x10048be5a` ->
///   `0x100445724`) to classify all non-whitespace characters as breaking class `0` (`LatinWord`) and
///   group them into unbreakable [`TokenKind::Word`] tokens.
pub fn tokenize_spans_with_script_policy(
    spans: &[InlineSpan],
    metrics: &dyn FontMetricsProvider,
    script_policy: WordScriptOverridePolicy,
) -> Vec<InlineToken> {
    let mut out = Vec::new();
    for span in spans {
        if let Some(explicit_w) = span.explicit_width_pt {
            out.push(InlineToken {
                text: span.text.clone(),
                width_pt: explicit_w,
                stretch_pt: 0.0,
                shrink_pt: 0.0,
                font_size_pt: span.font_size_pt,
                font_family: span.font_family.clone(),
                kind: TokenKind::InlineObject,
                ends_with_hyphen: false,
                char_widths_pt: vec![explicit_w],
            });
            continue;
        }

        let family = span.font_family.as_deref();
        let size = span.font_size_pt;
        let resolved_script = ptls7::resolve_script_policy_for_font(script_policy, family);
        let dflt_latin_override = resolved_script.is_dflt_latin_override();

        let mut word_buf = String::new();
        let mut word_w = 0.0;
        let mut word_char_widths: Vec<f64> = Vec::new();

        let flush_word = |buf: &mut String,
                          w: &mut f64,
                          char_widths: &mut Vec<f64>,
                          out: &mut Vec<InlineToken>| {
            if !buf.is_empty() {
                let ends_with_hyphen = buf.ends_with('-') || buf.ends_with('\u{00AD}');
                out.push(InlineToken {
                    text: std::mem::take(buf),
                    width_pt: *w,
                    stretch_pt: 0.0,
                    shrink_pt: 0.0,
                    font_size_pt: size,
                    font_family: span.font_family.clone(),
                    kind: TokenKind::Word,
                    ends_with_hyphen,
                    char_widths_pt: std::mem::take(char_widths),
                });
                *w = 0.0;
            }
        };

        let chars: Vec<char> = span.text.chars().collect();
        for (idx, &ch) in chars.iter().enumerate() {
            if ch == '\n' {
                flush_word(&mut word_buf, &mut word_w, &mut word_char_widths, &mut out);
                out.push(InlineToken {
                    text: "\n".to_string(),
                    width_pt: 0.0,
                    stretch_pt: 0.0,
                    shrink_pt: 0.0,
                    font_size_pt: size,
                    font_family: span.font_family.clone(),
                    kind: TokenKind::MandatoryBreak,
                    ends_with_hyphen: false,
                    char_widths_pt: Vec::new(),
                });
            } else if ch.is_whitespace() {
                flush_word(&mut word_buf, &mut word_w, &mut word_char_widths, &mut out);
                let adv = metrics.char_advance_pt(family, size, ch);
                out.push(InlineToken {
                    text: ch.to_string(),
                    width_pt: adv,
                    stretch_pt: adv * 0.50,
                    shrink_pt: adv * (1.0 / 3.0),
                    font_size_pt: size,
                    font_family: span.font_family.clone(),
                    kind: TokenKind::Whitespace,
                    ends_with_hyphen: false,
                    char_widths_pt: vec![adv],
                });
            } else if !dflt_latin_override
                && (is_cjk_or_fullwidth_char(ch)
                    || matches!(ch, '—' | '…' | '“' | '”' | '‘' | '’'))
            {
                flush_word(&mut word_buf, &mut word_w, &mut word_char_widths, &mut out);
                let prev_ch = idx.checked_sub(1).and_then(|pi| chars.get(pi).copied());
                let next_ch = chars.get(idx + 1).copied();
                let is_right_punct = is_cjk_right_half_space_punct(ch);
                let is_left_punct = is_cjk_left_half_space_punct(ch);
                let prev_is_right = prev_ch.is_some_and(is_cjk_right_half_space_punct);
                let prev_is_left = prev_ch.is_some_and(is_cjk_left_half_space_punct);
                let next_is_right = next_ch.is_some_and(is_cjk_right_half_space_punct);
                let next_is_left = next_ch.is_some_and(is_cjk_left_half_space_punct);

                // OpenType `chws` contextual halfwidth compression for adjacent fullwidth CJK punctuation:
                // - Closing/middle followed by closing/middle or opening (`》，`, `》）`, `，（`): first is `0.5em`.
                // - Opening preceded by opening (`（《`, `「『`): second is `0.5em`.
                let chws_compressed_self =
                    (is_right_punct && (next_is_right || next_is_left)) || (is_left_punct && prev_is_left);
                let chws_adjacent_partner =
                    (is_right_punct && prev_is_right) || (is_left_punct && (prev_is_right || next_is_left));

                let adv = if chws_compressed_self {
                    0.5 * size
                } else {
                    metrics.char_advance_pt(family, size, ch)
                };
                let shrink_pt = if (is_right_punct || is_left_punct)
                    && !chws_compressed_self
                    && !chws_adjacent_partner
                {
                    0.5 * size
                } else {
                    0.0
                };
                out.push(InlineToken {
                    text: ch.to_string(),
                    width_pt: adv,
                    stretch_pt: 0.0,
                    shrink_pt,
                    font_size_pt: size,
                    font_family: span.font_family.clone(),
                    kind: TokenKind::CjkChar,
                    ends_with_hyphen: false,
                    char_widths_pt: vec![adv],
                });
            } else {
                let adv = metrics.char_advance_pt(family, size, ch);
                word_buf.push(ch);
                word_w += adv;
                word_char_widths.push(adv);
                // In PTLS7 (`0x1d24e0`), hyphens and dashes (`-`, `–`, `—`) are class 5 (`Hyphen`):
                // breaking between consecutive dashes `(5, 5)` is prohibited (so `——` stays together),
                // whereas breaking after the last dash `(5, 0)` is permitted.
                let is_dash = matches!(ch, '-' | '–' | '—');
                let next_is_dash = chars
                    .get(idx + 1)
                    .is_some_and(|&next_ch| matches!(next_ch, '-' | '–' | '—'));
                if is_dash && !next_is_dash {
                    flush_word(&mut word_buf, &mut word_w, &mut word_char_widths, &mut out);
                }
            }
        }
        flush_word(&mut word_buf, &mut word_w, &mut word_char_widths, &mut out);
    }
    out
}

/// Fullwidth CJK closing / middle punctuation whose `0.5em` blank space sits on the RIGHT half of the `1.0em` advance box.
#[inline]
pub const fn is_cjk_right_half_space_punct(ch: char) -> bool {
    matches!(
        ch,
        '，' | '、' | '。' | '．' | '；' | '：' | '！' | '？' | '）' | '》' | '〉' | '」' | '』' | '】' | '〕' | '］' | '｝'
    )
}

/// Fullwidth CJK opening punctuation whose `0.5em` blank space sits on the LEFT half of the `1.0em` advance box.
#[inline]
pub const fn is_cjk_left_half_space_punct(ch: char) -> bool {
    matches!(
        ch,
        '（' | '《' | '〈' | '「' | '『' | '【' | '〔' | '［' | '｛'
    )
}

/// Dispatch paragraph line-breaking according to `LineBreakPolicy`.
pub fn break_paragraph_with_policy(
    spans: &[InlineSpan],
    avail_width_pt: f64,
    justify: bool,
    policy: LineBreakPolicy,
    kinsoku: &KinsokuSet,
    metrics: &dyn FontMetricsProvider,
) -> Vec<BrokenLine> {
    let tokens = tokenize_spans_with_script_policy(spans, metrics, policy.script_override_policy());
    match policy {
        LineBreakPolicy::FirstFit | LineBreakPolicy::FirstFitWithScriptPolicy(_) => {
            FirstFitBreaker.break_tokens(&tokens, avail_width_pt, justify, kinsoku)
        }
        LineBreakPolicy::KnuthPlass {
            hyph_cost,
            runt_cost,
            consecutive_dash_cost,
            max_cost,
            min_ratio,
        } => KnuthPlassBreaker {
            hyph_cost,
            runt_cost,
            consecutive_dash_cost,
            max_cost,
            min_ratio,
        }
        .break_tokens(&tokens, avail_width_pt, justify, kinsoku),
    }
}
