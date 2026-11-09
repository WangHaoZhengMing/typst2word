//! Domain enums and layout constraints for Word (`FirstFit` + `Kinsoku` + `WidowOrphan`)
//! and Typst (`KnuthPlass` dynamic programming) layout reproduction.

use serde::{Deserialize, Serialize};

pub use crate::linebreak::ptls7::WordScriptOverridePolicy;

/// Typst default hyphenation penalty (`HYPH_COST`).
pub const TYPST_HYPH_COST: f64 = 0.5;
/// Typst default runt (lone short final line) penalty (`RUNT_COST`).
pub const TYPST_RUNT_COST: f64 = 0.5;
/// Typst default consecutive hyphen/dash line-end penalty (`CONSECUTIVE_DASH_COST`).
pub const TYPST_CONSECUTIVE_DASH_COST: f64 = 0.3;
/// Overfull line maximum cost (`MAX_COST`).
pub const TYPST_MAX_COST: f64 = 1_000_000.0;
/// Minimum allowed shrink ratio before a line is considered overfull (`MIN_RATIO`).
pub const TYPST_MIN_RATIO: f64 = -1.0;

/// Default CJK characters forbidden at the start of a line (JIS X 4051 / GB/T 15834 / Word Kinsoku).
pub const DEFAULT_KINSOKU_LINE_START_FORBIDDEN: &str =
    "，、。．！？；：）》」』】〉》〕］｝”’％‰°℃·—～…!%),.:;?]}";

/// Default CJK characters forbidden at the end of a line (JIS X 4051 / GB/T 15834 / Word Kinsoku).
pub const DEFAULT_KINSOKU_LINE_END_FORBIDDEN: &str = "（《「『【〈《〔［｛“‘¥＄£€$([{";

/// Line-breaking algorithm strategy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LineBreakPolicy {
    /// Word's native greedy `FirstFit` tokenizer + `KinsokuSet` backtracking (`StandardEastAsian`).
    FirstFit,
    /// Word's greedy `FirstFit` with explicit [`WordScriptOverridePolicy`] (e.g.
    /// [`WordScriptOverridePolicy::OpenTypeDfltLatinOverride`] or [`WordScriptOverridePolicy::AutoDetectFont`]
    /// for AAT-only CJK fonts like `"Songti SC"` that trigger `0x10044afdc` `script_id = 5` override).
    FirstFitWithScriptPolicy(WordScriptOverridePolicy),
    /// Typst's `KnuthPlass` dynamic programming optimizer with configurable cost parameters.
    KnuthPlass {
        hyph_cost: f64,
        runt_cost: f64,
        consecutive_dash_cost: f64,
        max_cost: f64,
        min_ratio: f64,
    },
}

impl Default for LineBreakPolicy {
    fn default() -> Self {
        Self::FirstFit
    }
}

impl LineBreakPolicy {
    /// Construct Word's default greedy `FirstFit` policy (`StandardEastAsian`).
    #[inline]
    pub const fn word_first_fit() -> Self {
        Self::FirstFit
    }

    /// Construct Word's `FirstFit` policy with an explicit [`WordScriptOverridePolicy`].
    #[inline]
    pub const fn word_first_fit_with_script(script_policy: WordScriptOverridePolicy) -> Self {
        Self::FirstFitWithScriptPolicy(script_policy)
    }

    /// Construct Word's `FirstFit` policy simulating the `0x10044afdc` OpenType `'DFLT'`
    /// (`script_id = 5`, `Latin`) override.
    #[inline]
    pub const fn word_first_fit_dflt_override() -> Self {
        Self::FirstFitWithScriptPolicy(WordScriptOverridePolicy::OpenTypeDfltLatinOverride)
    }

    /// Construct Word's `FirstFit` policy with automatic font detection (triggering `'DFLT'`
    /// `script_id = 5` override for AAT-only CJK fonts such as `"Songti SC"`).
    #[inline]
    pub const fn word_first_fit_auto_font() -> Self {
        Self::FirstFitWithScriptPolicy(WordScriptOverridePolicy::AutoDetectFont)
    }

    /// Extract the [`WordScriptOverridePolicy`] for this line-break policy.
    #[inline]
    pub const fn script_override_policy(self) -> WordScriptOverridePolicy {
        match self {
            Self::FirstFit | Self::KnuthPlass { .. } => WordScriptOverridePolicy::StandardEastAsian,
            Self::FirstFitWithScriptPolicy(script_policy) => script_policy,
        }
    }

    /// Construct Typst's default `KnuthPlass` policy (`HYPH_COST = 0.5`, `RUNT_COST = 0.5`,
    /// `CONSECUTIVE_DASH_COST = 0.3`, `MAX_COST = 1_000_000.0`, `MIN_RATIO = -1.0`).
    #[inline]
    pub const fn typst_knuth_plass() -> Self {
        Self::KnuthPlass {
            hyph_cost: TYPST_HYPH_COST,
            runt_cost: TYPST_RUNT_COST,
            consecutive_dash_cost: TYPST_CONSECUTIVE_DASH_COST,
            max_cost: TYPST_MAX_COST,
            min_ratio: TYPST_MIN_RATIO,
        }
    }
}

/// Kinsoku Shori (避头尾) character sets restricting line-start and line-end break positions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KinsokuSet {
    /// Characters forbidden from appearing at the start of a wrapped line.
    pub line_start_forbidden: String,
    /// Characters forbidden from appearing at the end of a wrapped line.
    pub line_end_forbidden: String,
}

impl Default for KinsokuSet {
    fn default() -> Self {
        Self {
            line_start_forbidden: DEFAULT_KINSOKU_LINE_START_FORBIDDEN.to_string(),
            line_end_forbidden: DEFAULT_KINSOKU_LINE_END_FORBIDDEN.to_string(),
        }
    }
}

impl KinsokuSet {
    /// Empty kinsoku set (disables punctuation backtracking).
    pub fn empty() -> Self {
        Self {
            line_start_forbidden: String::new(),
            line_end_forbidden: String::new(),
        }
    }

    /// Fluent setter for line-start forbidden characters.
    pub fn with_line_start_forbidden(mut self, chars: impl Into<String>) -> Self {
        self.line_start_forbidden = chars.into();
        self
    }

    /// Fluent setter for line-end forbidden characters.
    pub fn with_line_end_forbidden(mut self, chars: impl Into<String>) -> Self {
        self.line_end_forbidden = chars.into();
        self
    }

    /// Check whether `ch` is forbidden at the beginning of a wrapped line.
    #[inline]
    pub fn is_line_start_forbidden(&self, ch: char) -> bool {
        self.line_start_forbidden.contains(ch)
    }

    /// Check whether `ch` is forbidden at the end of a wrapped line.
    #[inline]
    pub fn is_line_end_forbidden(&self, ch: char) -> bool {
        self.line_end_forbidden.contains(ch)
    }
}

/// Widow and orphan control policy (`<w:widowControl/>`, ECMA-376 §17.3.1.44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidowOrphanPolicy {
    /// Allow single orphan lines at page bottom and single widow lines at page top.
    Disabled,
    /// Enforce minimum lines at top of next page (`min_lines_at_top`, widow control)
    /// and minimum lines at bottom of current page (`min_lines_at_bottom`, orphan control).
    Enabled {
        min_lines_at_top: usize,
        min_lines_at_bottom: usize,
    },
}

impl Default for WidowOrphanPolicy {
    fn default() -> Self {
        Self::word_default()
    }
}

impl WidowOrphanPolicy {
    /// Word's default `<w:widowControl/>` (`min_lines_at_top = 2`, `min_lines_at_bottom = 2`).
    #[inline]
    pub const fn word_default() -> Self {
        Self::Enabled {
            min_lines_at_top: 2,
            min_lines_at_bottom: 2,
        }
    }

    /// Minimum lines required at the top of the next page when a paragraph splits.
    #[inline]
    pub const fn min_top(self) -> usize {
        match self {
            Self::Disabled => 1,
            Self::Enabled {
                min_lines_at_top, ..
            } => min_lines_at_top,
        }
    }

    /// Minimum lines required at the bottom of the current page before a paragraph splits.
    #[inline]
    pub const fn min_bottom(self) -> usize {
        match self {
            Self::Disabled => 1,
            Self::Enabled {
                min_lines_at_bottom,
                ..
            } => min_lines_at_bottom,
        }
    }
}

/// Policy for keeping all lines of a paragraph on the same page (`<w:keepLines/>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum KeepLinesPolicy {
    #[default]
    AllowBreak,
    KeepTogether,
}

impl KeepLinesPolicy {
    #[inline]
    pub const fn from_bool(keep: bool) -> Self {
        if keep {
            Self::KeepTogether
        } else {
            Self::AllowBreak
        }
    }

    #[inline]
    pub const fn is_kept(self) -> bool {
        matches!(self, Self::KeepTogether)
    }
}

/// Policy for keeping a paragraph on the same page as the next block (`<w:keepNext/>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum KeepWithNextPolicy {
    #[default]
    Independent,
    KeepWithNext,
}

impl KeepWithNextPolicy {
    #[inline]
    pub const fn from_bool(keep: bool) -> Self {
        if keep {
            Self::KeepWithNext
        } else {
            Self::Independent
        }
    }

    #[inline]
    pub const fn is_kept(self) -> bool {
        matches!(self, Self::KeepWithNext)
    }
}

/// Policy for collapsing vertical spacing between consecutive paragraphs of the same style (`<w:contextualSpacing/>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ContextualSpacingPolicy {
    #[default]
    Standard,
    SuppressBetweenSameStyle,
}

/// Policy for handling `space_before` at the top of a page or column (`word-rules` P0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpaceBeforeTopPolicy {
    /// Suppress `space_before` at the top of a soft-broken page (Word default).
    #[default]
    SuppressAtPageTop,
    /// Preserve `space_before` even at the top of a page.
    PreserveAtTop,
}

/// Policy for splitting a table row across a page boundary (`<w:cantSplit/>`, `word-rules` P5/P8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RowBreakPolicy {
    AllowBreakAcrossPages,
    #[default]
    CantSplit,
}

/// Policy for repeating a table header row at the top of continuation pages (`<w:tblHeader/>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RowHeaderRepeatPolicy {
    #[default]
    BodyRow,
    RepeatHeaderAtTop,
}

/// Complete layout constraint bundle attached to a paragraph in IR and `word-construct`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ParagraphLayoutConstraint {
    pub line_break_policy: LineBreakPolicy,
    pub kinsoku: KinsokuSet,
    pub widow_orphan: WidowOrphanPolicy,
    pub keep_lines: KeepLinesPolicy,
    pub keep_with_next: KeepWithNextPolicy,
    pub contextual_spacing: ContextualSpacingPolicy,
}

impl ParagraphLayoutConstraint {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_line_break_policy(mut self, policy: LineBreakPolicy) -> Self {
        self.line_break_policy = policy;
        self
    }

    #[inline]
    pub fn with_kinsoku(mut self, kinsoku: KinsokuSet) -> Self {
        self.kinsoku = kinsoku;
        self
    }

    #[inline]
    pub fn with_widow_orphan(mut self, policy: WidowOrphanPolicy) -> Self {
        self.widow_orphan = policy;
        self
    }

    #[inline]
    pub fn with_keep_lines(mut self, policy: KeepLinesPolicy) -> Self {
        self.keep_lines = policy;
        self
    }

    #[inline]
    pub fn with_keep_with_next(mut self, policy: KeepWithNextPolicy) -> Self {
        self.keep_with_next = policy;
        self
    }

    #[inline]
    pub fn with_contextual_spacing(mut self, policy: ContextualSpacingPolicy) -> Self {
        self.contextual_spacing = policy;
        self
    }
}
