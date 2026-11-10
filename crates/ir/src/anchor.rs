//! Read-only PDF line anchor representation (`AnchorLineIr`).
//!
//! `AnchorLineIr` records ground-truth line boundaries `(page, line_index, first_token,
//! last_token, x_start_pt, baseline_y_pt, width_pt, ends_with_hyphen)` extracted from a
//! reference PDF. It is strictly read-only and never used by the generator.

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
