//! Word-compatible page & column paginator implementing `word-rules` (P0–P10) and `word-rules2` (Q1–Q7):
//! - **P0**: Suppress `space_before` at the top of a soft-broken page (`SpaceBeforeTopPolicy`).
//! - **P1/P2**: Paragraph spacing and `<w:contextualSpacing/>` (`ContextualSpacingPolicy`).
//! - **P3**: Widow and orphan backtracking (`WidowOrphanPolicy`).
//! - **P4/Q1**: `<w:keepLines/>` (`KeepLinesPolicy`) and `<w:keepNext/>` chains (`KeepWithNextPolicy`).
//! - **P5/P8/Q2**: Table row `<w:cantSplit/>` (`RowBreakPolicy`) and `<w:tblHeader/>` (`RowHeaderRepeatPolicy`).

use serde::{Deserialize, Serialize};

use crate::anchor::AnchorLineIr;
use crate::font_metrics::FontMetricsProvider;
use crate::linebreak::{BrokenLine, InlineSpan, break_paragraph_with_policy};
use crate::policy::{
    ContextualSpacingPolicy, ParagraphLayoutConstraint, RowBreakPolicy, RowHeaderRepeatPolicy,
    SpaceBeforeTopPolicy,
};

/// Page geometry and pagination constraints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageConstraint {
    pub width_pt: f64,
    pub height_pt: f64,
    pub margin_top_pt: f64,
    pub margin_bottom_pt: f64,
    pub margin_left_pt: f64,
    pub margin_right_pt: f64,
    pub columns: usize,
    pub column_gutter_pt: f64,
    pub space_before_top: SpaceBeforeTopPolicy,
}

impl Default for PageConstraint {
    fn default() -> Self {
        Self::a4_default()
    }
}

impl PageConstraint {
    /// Standard A4 page (`595.28pt x 841.89pt`, `2.5cm` = `70.87pt` margins).
    #[inline]
    pub const fn a4_default() -> Self {
        Self {
            width_pt: 595.28,
            height_pt: 841.89,
            margin_top_pt: 70.87,
            margin_bottom_pt: 70.87,
            margin_left_pt: 70.87,
            margin_right_pt: 70.87,
            columns: 1,
            column_gutter_pt: 18.0,
            space_before_top: SpaceBeforeTopPolicy::SuppressAtPageTop,
        }
    }

    #[inline]
    pub fn content_width_pt(&self) -> f64 {
        let total = (self.width_pt - self.margin_left_pt - self.margin_right_pt).max(36.0);
        let cols = self.columns.max(1);
        if cols == 1 {
            total
        } else {
            ((total - (cols.saturating_sub(1) as f64) * self.column_gutter_pt) / (cols as f64))
                .max(36.0)
        }
    }

    #[inline]
    pub fn content_height_pt(&self) -> f64 {
        (self.height_pt - self.margin_top_pt - self.margin_bottom_pt).max(36.0)
    }

    #[inline]
    pub const fn with_content_height(mut self, content_h_pt: f64) -> Self {
        self.height_pt = self.margin_top_pt + self.margin_bottom_pt + content_h_pt;
        self
    }

    #[inline]
    pub const fn with_space_before_top(mut self, policy: SpaceBeforeTopPolicy) -> Self {
        self.space_before_top = policy;
        self
    }
}

/// A paragraph block ready for line-breaking and pagination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutParagraph {
    pub spans: Vec<InlineSpan>,
    pub style_id: Option<String>,
    pub justify: bool,
    pub space_before_pt: f64,
    pub space_after_pt: f64,
    pub explicit_line_height_pt: Option<f64>,
    pub left_indent_pt: f64,
    pub right_indent_pt: f64,
    pub first_line_indent_pt: f64,
    pub constraint: ParagraphLayoutConstraint,
}

impl LayoutParagraph {
    pub fn new(spans: Vec<InlineSpan>) -> Self {
        Self {
            spans,
            style_id: None,
            justify: false,
            space_before_pt: 0.0,
            space_after_pt: 0.0,
            explicit_line_height_pt: None,
            left_indent_pt: 0.0,
            right_indent_pt: 0.0,
            first_line_indent_pt: 0.0,
            constraint: ParagraphLayoutConstraint::default(),
        }
    }

    #[inline]
    pub fn with_style_id(mut self, style_id: impl Into<String>) -> Self {
        self.style_id = Some(style_id.into());
        self
    }

    #[inline]
    pub fn with_spacing(mut self, before_pt: f64, after_pt: f64) -> Self {
        self.space_before_pt = before_pt;
        self.space_after_pt = after_pt;
        self
    }

    #[inline]
    pub fn with_line_height(mut self, line_height_pt: f64) -> Self {
        self.explicit_line_height_pt = Some(line_height_pt);
        self
    }

    #[inline]
    pub fn with_constraint(mut self, constraint: ParagraphLayoutConstraint) -> Self {
        self.constraint = constraint;
        self
    }
}

/// A table row ready for pagination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutTableRow {
    pub label: String,
    pub height_pt: f64,
    pub break_policy: RowBreakPolicy,
    pub header_policy: RowHeaderRepeatPolicy,
}

impl LayoutTableRow {
    pub fn new(label: impl Into<String>, height_pt: f64) -> Self {
        Self {
            label: label.into(),
            height_pt,
            break_policy: RowBreakPolicy::CantSplit,
            header_policy: RowHeaderRepeatPolicy::BodyRow,
        }
    }

    #[inline]
    pub const fn as_header(mut self) -> Self {
        self.header_policy = RowHeaderRepeatPolicy::RepeatHeaderAtTop;
        self
    }
}

/// Block-level flow element passed to `paginate_blocks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutBlock {
    Paragraph(LayoutParagraph),
    Table { rows: Vec<LayoutTableRow> },
}

/// A single placed line on a paginated page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaginatedLine {
    pub block_index: usize,
    pub line_in_block: usize,
    pub first_token: String,
    pub last_token: String,
    pub x_start_pt: f64,
    pub top_y_pt: f64,
    pub baseline_y_pt: f64,
    pub height_pt: f64,
    pub width_pt: f64,
    pub ends_with_hyphen: bool,
}

impl PaginatedLine {
    pub fn to_anchor(&self, page: usize, line_index: usize) -> AnchorLineIr {
        AnchorLineIr::new(page, line_index, &self.first_token, &self.last_token)
            .with_geometry(self.x_start_pt, self.baseline_y_pt, self.width_pt)
            .with_hyphen(self.ends_with_hyphen)
    }
}

/// A single paginated page containing placed lines.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PaginatedPage {
    pub page_number: usize,
    pub lines: Vec<PaginatedLine>,
    pub used_height_pt: f64,
}

impl PaginatedPage {
    pub fn to_anchors(&self) -> Vec<AnchorLineIr> {
        self.lines
            .iter()
            .enumerate()
            .map(|(idx, l)| l.to_anchor(self.page_number, idx))
            .collect()
    }
}

struct PreparedParagraph {
    block_index: usize,
    lines: Vec<BrokenLine>,
    line_height_pt: f64,
    ascent_pt: f64,
    x_start_pt: f64,
    first_line_indent_pt: f64,
    space_before_pt: f64,
    space_after_pt: f64,
    style_id: Option<String>,
    constraint: ParagraphLayoutConstraint,
}

impl PreparedParagraph {
    /// Minimum vertical height required on the current page to start this paragraph
    /// without violating `KeepLinesPolicy` or `WidowOrphanPolicy`.
    fn required_initial_lines(&self) -> usize {
        let total = self.lines.len();
        if total == 0 {
            return 0;
        }
        if self.constraint.keep_lines.is_kept() {
            return total;
        }
        let min_bot = self.constraint.widow_orphan.min_bottom();
        let min_top = self.constraint.widow_orphan.min_top();
        if total < min_bot + min_top && min_bot > 1 {
            // A 2- or 3-line paragraph with min_bottom=2, min_top=2 cannot split into 1+1 or 2+1!
            total
        } else {
            min_bot.min(total)
        }
    }
}

/// Paginate a sequence of `LayoutBlock`s according to `PageConstraint` and `FontMetricsProvider`.
pub fn paginate_blocks(
    blocks: &[LayoutBlock],
    page: &PageConstraint,
    metrics: &dyn FontMetricsProvider,
) -> Vec<PaginatedPage> {
    let page_h = page.content_height_pt();
    let col_w = page.content_width_pt();
    let mut pages: Vec<PaginatedPage> = Vec::new();
    let mut current_page = PaginatedPage {
        page_number: 1,
        lines: Vec::new(),
        used_height_pt: 0.0,
    };
    let mut prev_space_after_pt = 0.0_f64;
    let mut prev_style_id: Option<String> = None;
    let mut prev_contextual = ContextualSpacingPolicy::Standard;

    // Pre-break all paragraphs so `KeepWithNext` lookahead can inspect successor line heights.
    let mut prepared: Vec<Option<PreparedParagraph>> = Vec::with_capacity(blocks.len());
    for (b_idx, block) in blocks.iter().enumerate() {
        match block {
            LayoutBlock::Paragraph(p) => {
                let avail_w = (col_w - p.left_indent_pt - p.right_indent_pt).max(20.0);
                let broken = break_paragraph_with_policy(
                    &p.spans,
                    avail_w,
                    p.justify,
                    p.constraint.line_break_policy,
                    &p.constraint.kinsoku,
                    metrics,
                );
                let first_font = p.spans.first().and_then(|s| s.font_family.as_deref());
                let first_size = p.spans.first().map(|s| s.font_size_pt).unwrap_or(11.0);
                let line_h = p
                    .explicit_line_height_pt
                    .unwrap_or_else(|| metrics.single_line_height_pt(first_font, first_size));
                let ascent = metrics.ascent_pt(first_font, first_size).min(line_h);
                prepared.push(Some(PreparedParagraph {
                    block_index: b_idx,
                    lines: broken,
                    line_height_pt: line_h,
                    ascent_pt: ascent,
                    x_start_pt: page.margin_left_pt + p.left_indent_pt,
                    first_line_indent_pt: p.first_line_indent_pt,
                    space_before_pt: p.space_before_pt,
                    space_after_pt: p.space_after_pt,
                    style_id: p.style_id.clone(),
                    constraint: p.constraint.clone(),
                }));
            }
            LayoutBlock::Table { .. } => prepared.push(None),
        }
    }

    for (b_idx, block) in blocks.iter().enumerate() {
        match block {
            LayoutBlock::Paragraph(_) => {
                let Some(prep) = &prepared[b_idx] else {
                    continue;
                };
                if prep.lines.is_empty() {
                    continue;
                }

                // Compute inter-paragraph vertical gap (P0: suppress space_before at page top;
                // P2: contextualSpacing suppresses gap between consecutive paragraphs of same style).
                let compute_gap = |on_empty_page: bool,
                                   prev_after: f64,
                                   prev_style: &Option<String>,
                                   prev_ctx: ContextualSpacingPolicy|
                 -> f64 {
                    if on_empty_page {
                        match page.space_before_top {
                            SpaceBeforeTopPolicy::SuppressAtPageTop => 0.0,
                            SpaceBeforeTopPolicy::PreserveAtTop => prep.space_before_pt,
                        }
                    } else if matches!(
                        prep.constraint.contextual_spacing,
                        ContextualSpacingPolicy::SuppressBetweenSameStyle
                    ) && matches!(prev_ctx, ContextualSpacingPolicy::SuppressBetweenSameStyle)
                        && prep.style_id.is_some()
                        && prep.style_id == *prev_style
                    {
                        0.0
                    } else {
                        prev_after + prep.space_before_pt
                    }
                };

                // Lookahead for `KeepWithNext` chain required height.
                let mut chain_extra_h = 0.0_f64;
                if prep.constraint.keep_with_next.is_kept() {
                    let mut look = b_idx;
                    while look < blocks.len() {
                        let Some(cur_p) = &prepared[look] else {
                            break;
                        };
                        if !cur_p.constraint.keep_with_next.is_kept() {
                            break;
                        }
                        if let Some(Some(next_p)) = prepared.get(look + 1) {
                            let req_lines = next_p.required_initial_lines();
                            chain_extra_h += cur_p.space_after_pt
                                + next_p.space_before_pt
                                + (req_lines as f64) * next_p.line_height_pt;
                        } else if let Some(LayoutBlock::Table { rows }) = blocks.get(look + 1)
                            && let Some(first_row) = rows.first()
                        {
                            chain_extra_h += cur_p.space_after_pt + first_row.height_pt;
                        }
                        look += 1;
                    }
                }

                let req_initial = prep.required_initial_lines();
                let gap = compute_gap(
                    current_page.lines.is_empty(),
                    prev_space_after_pt,
                    &prev_style_id,
                    prev_contextual,
                );
                let needed_h = gap
                    + (req_initial as f64) * prep.line_height_pt
                    + if req_initial == prep.lines.len() {
                        chain_extra_h
                    } else {
                        0.0
                    };

                if !current_page.lines.is_empty()
                    && current_page.used_height_pt + needed_h > page_h + 1e-4
                {
                    let next_num = current_page.page_number + 1;
                    pages.push(std::mem::replace(
                        &mut current_page,
                        PaginatedPage {
                            page_number: next_num,
                            lines: Vec::new(),
                            used_height_pt: 0.0,
                        },
                    ));
                }

                // Determine how many lines fit on `current_page`.
                let active_gap = compute_gap(
                    current_page.lines.is_empty(),
                    prev_space_after_pt,
                    &prev_style_id,
                    prev_contextual,
                );
                let total_lines = prep.lines.len();
                let mut rem_h = page_h - (current_page.used_height_pt + active_gap);
                let mut fit_count = if rem_h >= prep.line_height_pt - 1e-4 {
                    ((rem_h + 1e-4) / prep.line_height_pt).floor() as usize
                } else {
                    0
                };
                fit_count = fit_count.min(total_lines);

                // Enforce `WidowOrphanPolicy` (P3): if splitting leaves `< min_top` widow lines on
                // the next page, backtrack `min_top - rem_lines` lines from `fit_count`.
                if fit_count > 0 && fit_count < total_lines {
                    let rem_lines = total_lines - fit_count;
                    let min_top = prep.constraint.widow_orphan.min_top();
                    let min_bot = prep.constraint.widow_orphan.min_bottom();
                    if rem_lines < min_top {
                        let need_backtrack = min_top - rem_lines;
                        if fit_count >= min_bot + need_backtrack {
                            fit_count -= need_backtrack;
                        } else if !current_page.lines.is_empty() {
                            // Cannot satisfy both min_bottom and min_top -> move whole paragraph to next page!
                            let next_num = current_page.page_number + 1;
                            pages.push(std::mem::replace(
                                &mut current_page,
                                PaginatedPage {
                                    page_number: next_num,
                                    lines: Vec::new(),
                                    used_height_pt: 0.0,
                                },
                            ));
                            let top_gap = compute_gap(
                                true,
                                prev_space_after_pt,
                                &prev_style_id,
                                prev_contextual,
                            );
                            rem_h = page_h - top_gap;
                            fit_count = (((rem_h + 1e-4) / prep.line_height_pt).floor() as usize)
                                .min(total_lines)
                                .max(1);
                        }
                    }
                }

                // Place lines 0..total_lines across pages.
                let mut line_idx = 0usize;
                let mut first_chunk = true;
                while line_idx < total_lines {
                    let chunk_gap = if first_chunk {
                        compute_gap(
                            current_page.lines.is_empty(),
                            prev_space_after_pt,
                            &prev_style_id,
                            prev_contextual,
                        )
                    } else {
                        0.0
                    };
                    current_page.used_height_pt += chunk_gap;

                    let count_this_page = if first_chunk && fit_count > 0 {
                        fit_count
                    } else {
                        let avail = page_h - current_page.used_height_pt;
                        (((avail + 1e-4) / prep.line_height_pt).floor() as usize)
                            .max(1)
                            .min(total_lines - line_idx)
                    };

                    for offset in 0..count_this_page {
                        let k = line_idx + offset;
                        let bl = &prep.lines[k];
                        let top_y = page.margin_top_pt + current_page.used_height_pt;
                        let x_start = prep.x_start_pt
                            + if k == 0 {
                                prep.first_line_indent_pt
                            } else {
                                0.0
                            };
                        current_page.lines.push(PaginatedLine {
                            block_index: prep.block_index,
                            line_in_block: k,
                            first_token: bl.first_token().to_string(),
                            last_token: bl.last_token().to_string(),
                            x_start_pt: x_start,
                            top_y_pt: top_y,
                            baseline_y_pt: top_y + prep.ascent_pt,
                            height_pt: prep.line_height_pt,
                            width_pt: bl.width_pt,
                            ends_with_hyphen: bl.ends_with_hyphen,
                        });
                        current_page.used_height_pt += prep.line_height_pt;
                    }

                    line_idx += count_this_page;
                    first_chunk = false;
                    if line_idx < total_lines {
                        let next_num = current_page.page_number + 1;
                        pages.push(std::mem::replace(
                            &mut current_page,
                            PaginatedPage {
                                page_number: next_num,
                                lines: Vec::new(),
                                used_height_pt: 0.0,
                            },
                        ));
                    }
                }

                prev_space_after_pt = prep.space_after_pt;
                prev_style_id = prep.style_id.clone();
                prev_contextual = prep.constraint.contextual_spacing;
            }
            LayoutBlock::Table { rows } => {
                let header_rows: Vec<&LayoutTableRow> = rows
                    .iter()
                    .take_while(|r| {
                        matches!(r.header_policy, RowHeaderRepeatPolicy::RepeatHeaderAtTop)
                    })
                    .collect();

                for (r_idx, row) in rows.iter().enumerate() {
                    if !current_page.lines.is_empty()
                        && current_page.used_height_pt + row.height_pt > page_h + 1e-4
                    {
                        let next_num = current_page.page_number + 1;
                        pages.push(std::mem::replace(
                            &mut current_page,
                            PaginatedPage {
                                page_number: next_num,
                                lines: Vec::new(),
                                used_height_pt: 0.0,
                            },
                        ));
                        // Repeat `<w:tblHeader/>` rows at the top of continuation page (Q2).
                        if r_idx >= header_rows.len() {
                            for (h_idx, h_row) in header_rows.iter().enumerate() {
                                let top_y = page.margin_top_pt + current_page.used_height_pt;
                                current_page.lines.push(PaginatedLine {
                                    block_index: b_idx,
                                    line_in_block: h_idx,
                                    first_token: h_row.label.clone(),
                                    last_token: h_row.label.clone(),
                                    x_start_pt: page.margin_left_pt,
                                    top_y_pt: top_y,
                                    baseline_y_pt: top_y + h_row.height_pt * 0.75,
                                    height_pt: h_row.height_pt,
                                    width_pt: col_w,
                                    ends_with_hyphen: false,
                                });
                                current_page.used_height_pt += h_row.height_pt;
                            }
                        }
                    }

                    let top_y = page.margin_top_pt + current_page.used_height_pt;
                    current_page.lines.push(PaginatedLine {
                        block_index: b_idx,
                        line_in_block: r_idx,
                        first_token: row.label.clone(),
                        last_token: row.label.clone(),
                        x_start_pt: page.margin_left_pt,
                        top_y_pt: top_y,
                        baseline_y_pt: top_y + row.height_pt * 0.75,
                        height_pt: row.height_pt,
                        width_pt: col_w,
                        ends_with_hyphen: false,
                    });
                    current_page.used_height_pt += row.height_pt;
                }
                prev_space_after_pt = 0.0;
                prev_style_id = None;
                prev_contextual = ContextualSpacingPolicy::Standard;
            }
        }
    }

    if !current_page.lines.is_empty() || pages.is_empty() {
        pages.push(current_page);
    }
    pages
}
