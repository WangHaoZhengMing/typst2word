//! Reverse-engineered `Microsoft Word` + `MicrosoftPTLS7.framework` character classification,
//! 22×22 breaking-class pair table (`0x1d24e0`), and OpenType `'DFLT'` (`script_id = 5`) override model.
//!
//! ## Disassembled Call Chain in `Microsoft Word` (`x86_64`) & `MicrosoftPTLS7`
//!
//! When laying out a text run (`plsrun`), Microsoft Word executes the following shaping and
//! script-validation sequence in `0x1004431fc` -> `0x10044ac52`:
//!
//! 1. **Optimistic OpenType Activation (`0x1004435e8..0x1004435f1`)**:
//!    - Word calls `0x100447869` (`0x100447898`), which returns `1` for normal non-math runs
//!      (`0x78(%legacy_chp) == 0`).
//!    - At `0x1004435f1`, Word sets `0x13(%r12) = 1` (`fUseOpenType = 1`), while `0x2a(%r15)`
//!      (`fFontSupportsRunScript`) is initialized to `0`.
//!
//! 2. **Skipped OpenType Feature Check (`0x1004437c0..0x100443803`)**:
//!    - At `0x1004437f4..0x100443803`, Word calls `0x100447c05(%rbx)` to check whether explicit
//!      OpenType kerning (`0x7a(%rbx)`) or ligatures (`0x80(%rbx)`) are enabled on the `CHP`, and
//!      checks `-0x2a(%rbp)` (`(font_face->flags_0xd4 >> 12) & 1`, populated in `0x103ebb56c`).
//!    - When no explicit OpenType features are requested on the run (or bit 12 of `flags_0xd4` is set),
//!      `0x100443803` (`jne 0x1004436ae`) jumps over `0x103ebd8c1`, leaving `0x13(%r12) = 1` and
//!      `0x2a(%r15) = 0`. Even when `0x103ebd8c1` is called, Apple AAT fonts lacking OpenType
//!      `'hani'` `GSUB`/`GPOS` tables (such as `/System/Library/Fonts/Supplemental/Songti.ttc`,
//!      which uses AAT `feat` + `morx` tables) leave `0x2a(%r15) = 0`.
//!
//! 3. **East Asian Fallback Check (`0x100443791..0x1004437a7` -> `0x103ebda81`)**:
//!    - For an East Asian run (`0x1a(%r15) == 1`), Word calls `0x103ebda81(%r14, %r15)`.
//!    - `0x103ebda81` loads `eax = 0x64(%r15)` (populated at `0x103ebb71b..0x103ebb738` from
//!      `0x10041e8ed` -> `0x10042929e`), computes `ecx = eax + 1`, and tests `(0x47 >> ecx) & 1`.
//!    - Only if `0x103ebda81` returns `0` (`0x64(%r15)` in `{2, 3, 4}`) does `0x1004437a7` set
//!      `0x2a(%r15) = 1`. For `"Songti SC"` (`Songti.ttc`), `0x64(%r15)` is `-1` (`0xffffffff`),
//!      so `0x103ebda81` returns `1` and `0x2a(%r15)` remains `0`.
//!
//! 4. **OpenType `'DFLT'` / `script_id = 5` (`Latin`) Override (`0x10044afdc..0x10044affd`)**:
//!    - In `0x10044ac52`, because `0x13(%r12) == 1` (`fUseOpenType == 1`) and `0x2a(%r15) == 0`
//!      (`fFontSupportsRunScript == 0`), Word falls back to OpenType `'DFLT'` shaping by overwriting
//!      the `plsrun`'s script ID to `5` (`Latin`) and script tag to `'DFLT'` (`0x44464c54`):
//!      ```text
//!      000000010044afdc: movb $0x1, 0x105(%rbx)
//!      000000010044afe3: movb $0x1, 0x106(%rbx)
//!      000000010044afea: movw $0x5, 0x19e(%rbx)        ; plsrun->script_id = 5 (Latin)
//!      000000010044aff3: movl $0x44464c54, 0x1a4(%rbx) ; plsrun->ot_script_tag = 'DFLT'
//!      ```
//!
//! 5. **PTLS7 Breaking-Class Collapse & Emergency Mid-Word Truncation (`MicrosoftPTLS7`)**:
//!    - Because `plsrun->script_id == 5` (`Latin`), Word's `GetBreakingClasses` callback
//!      (`0x10048be5a` -> `0x100445724`) classifies all non-whitespace characters in the run as
//!      breaking class `0` ([`Ptls7BreakClass::LatinWord`]).
//!    - In PTLS7's 22×22 breaking-class pair table (`0x1d24e0` in `MicrosoftPTLS7`), breaking
//!      between `(0, 0)` is [`Ptls7PairAction::Prohibited`].
//!    - Consequently, contiguous non-space sequences (including Chinese ideographs, `，`, `、`, `。`,
//!      and digits `2020`/`2022`) form a single unbreakable word token:
//!      - If a line has a prior whitespace break opportunity (such as `"非常普通的 seaside "`),
//!        PTLS7 wraps the following oversized CJK sequence to the next line at `x = 0`.
//!      - When an oversized token starts at `x = 0` and still exceeds `avail_width_pt`, PTLS7
//!        executes **emergency mid-word character truncation** (`ForcedBreak`) at the last character
//!        boundary `<= avail_width_pt`, ignoring Kinsoku and slicing through multi-digit numbers.

use serde::{Deserialize, Serialize};

use crate::font_metrics::{ShapingWorkaround, is_cjk_or_fullwidth_char, lookup_known_font};
use crate::policy::KinsokuSet;
pub use ir::policy::WordScriptOverridePolicy;

/// Resolve the effective [`WordScriptOverridePolicy`] for a given `font_family` via [`lookup_known_font`].
#[inline]
pub fn resolve_script_policy_for_font(
    policy: WordScriptOverridePolicy,
    font_family: Option<&str>,
) -> WordScriptOverridePolicy {
    policy.resolve_for_aat_font(is_aat_only_cjk_font_triggering_dflt_override(font_family))
}

/// Check whether `font_family` is a macOS Apple AAT-only CJK font (containing `feat`/`morx`
/// instead of OpenType `'hani'` `GSUB`/`GPOS` and returning `0x64(%r15) == -1` in `0x103ebda81`),
/// which triggers Word's `0x10044afdc` `script_id = 5` (`'DFLT'`) override.
#[inline]
pub fn is_aat_only_cjk_font_triggering_dflt_override(font_family: Option<&str>) -> bool {
    font_family
        .and_then(lookup_known_font)
        .is_some_and(|p| p.shaping_workaround() == ShapingWorkaround::ForceCjkScriptBinding)
}

/// PTLS7 character breaking class indices corresponding to the 22×22 pair table at `0x1d24e0`
/// in `MicrosoftPTLS7.framework`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ptls7BreakClass {
    /// Class `0`: Western Latin/numeric alphanumeric character (`(0, 0)` break is prohibited).
    LatinWord = 0,
    /// Class `4`: Inter-word whitespace (`U+0020`, `U+0009`), allowing a direct break after it.
    Whitespace = 4,
    /// Class `5`: Explicit hyphen (`-`, `U+00AD`), allowing a direct break after it.
    Hyphen = 5,
    /// Class `6`: Opening punctuation forbidden at line end (`（`, `“`, `《`, `[`, etc.).
    KinsokuOpen = 6,
    /// Class `7`: Closing/middle punctuation forbidden at line start (`，`, `。`, `、`, `）`, `”`, etc.).
    KinsokuClose = 7,
    /// Class `10`: East Asian ideograph or fullwidth symbol allowing per-character breaks.
    EastAsianIdeograph = 10,
    /// Class `21`: Mandatory paragraph/line break (`\n`).
    MandatoryBreak = 21,
}

/// Break action between two adjacent `Ptls7BreakClass` values from PTLS7's `0x1d24e0` pair table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ptls7PairAction {
    /// Direct break is permitted between `prev` and `next` without backtracking.
    DirectBreak,
    /// Breaking between `prev` and `next` is prohibited (must stay in the same token, or backtrack).
    Prohibited,
    /// Breaking immediately before `next` (or after `prev`) violates Kinsoku and requires backtracking.
    KinsokuBacktrack,
}

impl Ptls7BreakClass {
    /// Classify a character `ch` under the resolved [`WordScriptOverridePolicy`] and [`KinsokuSet`].
    #[inline]
    pub fn classify(
        ch: char,
        script_policy: WordScriptOverridePolicy,
        kinsoku: &KinsokuSet,
    ) -> Self {
        match (ch, script_policy.is_dflt_latin_override()) {
            ('\n', _) => Self::MandatoryBreak,
            (c, _) if c.is_whitespace() => Self::Whitespace,
            // Under Word's 0x10044afdc OpenType 'DFLT' / script_id = 5 (Latin) override,
            // `GetBreakingClasses` (`0x10048be5a` -> `0x100445724`) collapses all non-whitespace,
            // non-dash characters in the run into class 0 (`LatinWord`), while hyphens/dashes
            // (`-`, `–`, `—`) remain class 5 (`Hyphen`, allowing a break after a dash run).
            ('-' | '\u{00AD}' | '–' | '—', _) => Self::Hyphen,
            (_, true) => Self::LatinWord,
            (c, false) if kinsoku.is_line_end_forbidden(c) => Self::KinsokuOpen,
            (c, false) if kinsoku.is_line_start_forbidden(c) => Self::KinsokuClose,
            (c, false) if is_cjk_or_fullwidth_char(c) || matches!(c, '—' | '…') => {
                Self::EastAsianIdeograph
            }
            (_, false) => Self::LatinWord,
        }
    }

    /// Query PTLS7's 22×22 breaking-class pair table (`0x1d24e0` in `MicrosoftPTLS7.framework`)
    /// for the transition `(self, next)`.
    #[inline]
    pub const fn pair_action(self, next: Self) -> Ptls7PairAction {
        match (self, next) {
            (Self::MandatoryBreak, _) | (_, Self::MandatoryBreak) => Ptls7PairAction::DirectBreak,
            (Self::Whitespace, _) => Ptls7PairAction::DirectBreak,
            (_, Self::Whitespace) => Ptls7PairAction::Prohibited,
            // Opening punctuation (class 6) cannot end a line -> backtrack before `self`.
            (Self::KinsokuOpen, _) => Ptls7PairAction::KinsokuBacktrack,
            // Closing punctuation (class 7) cannot start a line -> backtrack before `self`.
            (_, Self::KinsokuClose) => Ptls7PairAction::KinsokuBacktrack,
            // Class (0, 0) and (5, 5): two adjacent LatinWord characters or two consecutive dashes (`——`)
            // cannot break between each other.
            (Self::LatinWord, Self::LatinWord) | (Self::Hyphen, Self::Hyphen) => {
                Ptls7PairAction::Prohibited
            }
            // Hyphen (class 5) followed by non-hyphen, or EastAsianIdeograph (class 10) allows direct break.
            (Self::Hyphen, _)
            | (Self::EastAsianIdeograph, _)
            | (_, Self::EastAsianIdeograph)
            | (_, Self::KinsokuOpen) => Ptls7PairAction::DirectBreak,
            (Self::KinsokuClose, Self::LatinWord | Self::Hyphen) => Ptls7PairAction::DirectBreak,
            (Self::LatinWord, Self::Hyphen) => Ptls7PairAction::Prohibited,
        }
    }
}

/// Classification of how a [`crate::linebreak::BrokenLine`] terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineBreakCause {
    /// Normal greedy or optimal wrap at a legal token boundary (or end of paragraph).
    #[default]
    NormalWrap,
    /// Explicit newline (`\n` or Typst `\`).
    MandatoryBreak,
    /// PTLS7 emergency mid-word character truncation (`ForcedBreak`) when a single unbreakable
    /// token at `x = 0` exceeds `avail_width_pt`.
    EmergencyMidWordTruncation,
}
