//! Read-only line anchor extraction from Typst laid-out `Frame`s (`AnchorLineIr`).
//!
//! Produces ground-truth line boundaries `(page, line_index, first_token, last_token,
//! x_start_pt, baseline_y_pt, width_pt, ends_with_hyphen)` for verification harnesses.
//! Strictly decoupled from DOCX generation (`DocxBuilder` never reads `DocumentIr::anchors`).

use typst::layout::{Frame, FrameItem, Point};

use crate::ExtractorContext;
use ir::{AnchorLineIr, is_cjk_char};

#[derive(Debug, Clone)]
struct RawTextSpan {
    x_left_pt: f64,
    x_right_pt: f64,
    baseline_y_pt: f64,
    text: String,
}

impl<'a> ExtractorContext<'a> {
    /// Extract read-only `AnchorLineIr` line records across all laid-out pages.
    pub(crate) fn extract_page_line_anchors(&self) -> Vec<AnchorLineIr> {
        let mut all_anchors = Vec::new();
        for (page_idx, page) in self.compiled.document.pages().iter().enumerate() {
            let page_num = page_idx + 1;
            let mut spans = Vec::new();
            Self::collect_frame_text_spans(&page.frame, Point::zero(), &mut spans);

            // Filter out header/footer marginalia outside the main content Y band if possible.
            spans.retain(|s| !s.text.trim().is_empty());
            spans.sort_by(|a, b| {
                a.baseline_y_pt
                    .partial_cmp(&b.baseline_y_pt)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        a.x_left_pt
                            .partial_cmp(&b.x_left_pt)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            });

            // Cluster spans into horizontal lines (same baseline within 2.5pt and horizontal gap <= 36pt).
            let mut lines: Vec<Vec<RawTextSpan>> = Vec::new();
            for span in spans {
                if let Some(last_line) = lines.last_mut()
                    && let Some(prev) = last_line.last()
                    && (span.baseline_y_pt - prev.baseline_y_pt).abs() <= 2.5
                    && span.x_left_pt <= prev.x_right_pt + 36.0
                {
                    last_line.push(span);
                } else {
                    lines.push(vec![span]);
                }
            }

            for (line_index, cluster) in lines.into_iter().enumerate() {
                let x_start_pt = cluster
                    .iter()
                    .map(|s| s.x_left_pt)
                    .fold(f64::INFINITY, f64::min);
                let x_end_pt = cluster
                    .iter()
                    .map(|s| s.x_right_pt)
                    .fold(f64::NEG_INFINITY, f64::max);
                let baseline_y_pt = cluster.first().map(|s| s.baseline_y_pt).unwrap_or(0.0);
                let full_text: String = cluster.iter().map(|s| s.text.as_str()).collect();
                let trimmed = full_text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let first_token = Self::extract_boundary_token(trimmed, true);
                let last_token = Self::extract_boundary_token(trimmed, false);
                let ends_with_hyphen = trimmed.ends_with('-') || trimmed.ends_with('\u{00AD}');
                all_anchors.push(
                    AnchorLineIr::new(page_num, line_index, first_token, last_token)
                        .with_geometry(x_start_pt, baseline_y_pt, (x_end_pt - x_start_pt).max(0.0))
                        .with_hyphen(ends_with_hyphen),
                );
            }
        }
        all_anchors
    }

    fn collect_frame_text_spans(frame: &Frame, offset: Point, out: &mut Vec<RawTextSpan>) {
        for (pos, item) in frame.items() {
            let abs_pos = offset + *pos;
            match item {
                FrameItem::Group(g) => {
                    Self::collect_frame_text_spans(&g.frame, abs_pos, out);
                }
                FrameItem::Text(t) => {
                    let s = t.text.to_string();
                    if !s.trim().is_empty() {
                        let w = t.width().to_pt();
                        out.push(RawTextSpan {
                            x_left_pt: abs_pos.x.to_pt(),
                            x_right_pt: abs_pos.x.to_pt() + w,
                            baseline_y_pt: abs_pos.y.to_pt(),
                            text: s,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_boundary_token(trimmed: &str, is_first: bool) -> String {
        if is_first {
            let Some(ch) = trimmed.chars().next() else {
                return String::new();
            };
            if is_cjk_char(ch) {
                ch.to_string()
            } else {
                trimmed
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            }
        } else {
            let Some(ch) = trimmed.chars().next_back() else {
                return String::new();
            };
            if is_cjk_char(ch) {
                ch.to_string()
            } else {
                trimmed
                    .split_whitespace()
                    .next_back()
                    .unwrap_or("")
                    .to_string()
            }
        }
    }
}
