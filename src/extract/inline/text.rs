//! Laid-out `TextItem`s -> `TextRunIr` (fonts, run merging, CJK line joining).

use typst::text::{FontWeight, TextItem};

use crate::color::paint_to_hex;
use crate::extract::ExtractorContext;
use crate::extract::inline::ActiveInlineState;
use crate::ir::{InlineNode, TextRunIr};

impl<'a> ExtractorContext<'a> {
    // =========================================================================
    // Conversion & Geometry Helpers
    // =========================================================================
    pub(crate) fn text_item_to_run(item: &TextItem, state: &ActiveInlineState) -> TextRunIr {
        let info = item.font.info();
        let family_lower = info.family.to_lowercase();
        let bold = state.bold
            || info.variant.weight >= FontWeight::BOLD
            || family_lower.contains("bold")
            || item.stroke.is_some();
        let italic = state.italic
            || matches!(
                info.variant.style,
                typst::text::FontStyle::Italic | typst::text::FontStyle::Oblique
            )
            || family_lower.contains("italic")
            || family_lower.contains("oblique");
        let is_mono = state.is_code
            || info.flags.contains(typst::text::FontFlags::MONOSPACE)
            || family_lower.contains("mono")
            || family_lower.contains("consolas")
            || family_lower.contains("courier")
            || family_lower.contains("dejavu sans mono");

        let color = paint_to_hex(&item.fill).filter(|hex| hex != "000000" || state.is_code);
        let mapped_family = if is_mono {
            "Consolas".to_string()
        } else {
            Self::normalize_word_font_family(&info.family)
        };

        TextRunIr {
            text: item.text.to_string(),
            font_family: Some(mapped_family),
            font_size_pt: Some(item.size.to_pt()),
            bold,
            italic,
            underline: state.underline.clone(),
            strike: state.strike.clone(),
            overline: state.overline.clone(),
            color,
            highlight: state.highlight.clone(),
            border: state.border.clone(),
            script: state.script,
            smallcaps: state.smallcaps,
            is_code: state.is_code,
        }
    }

    pub(crate) fn normalize_word_font_family(family: &str) -> String {
        match family.trim() {
            "Hei" | "STHeiti" => "Heiti SC".to_string(),
            "Kai" | "STKaiti" => "Kaiti SC".to_string(),
            "Song" | "STSong" => "Songti SC".to_string(),
            "Pingfang sc" | "pingfang sc" | "PingFang SC" => "PingFang SC".to_string(),
            other => other.to_string(),
        }
    }

    pub(crate) fn merge_adjacent_text_runs(inlines: &mut Vec<InlineNode>) {
        let mut i = 1;
        while i < inlines.len() {
            let can_merge = match (&inlines[i - 1], &inlines[i]) {
                (InlineNode::Text(a), InlineNode::Text(b)) => {
                    a.font_family == b.font_family
                        && a.font_size_pt.map(|v| (v * 10.0).round() as i64)
                            == b.font_size_pt.map(|v| (v * 10.0).round() as i64)
                        && a.bold == b.bold
                        && a.italic == b.italic
                        && a.underline.is_some() == b.underline.is_some()
                        && a.strike.is_some() == b.strike.is_some()
                        && a.overline.is_some() == b.overline.is_some()
                        && a.color == b.color
                        && a.highlight == b.highlight
                        && a.border.is_some() == b.border.is_some()
                        && a.script == b.script
                        && a.smallcaps == b.smallcaps
                        && a.is_code == b.is_code
                }
                _ => false,
            };
            if can_merge {
                let InlineNode::Text(b) = inlines.remove(i) else {
                    unreachable!()
                };
                if let InlineNode::Text(a) = &mut inlines[i - 1] {
                    a.text.push_str(&b.text);
                }
            } else {
                i += 1;
            }
        }
    }

    pub(crate) fn should_insert_line_join_space(existing: &[InlineNode], next_text: &str) -> bool {
        let Some(InlineNode::Text(last_run)) = existing.last() else {
            return false;
        };
        let Some(last_ch) = last_run.text.chars().last() else {
            return false;
        };
        let Some(next_ch) = next_text.chars().next() else {
            return false;
        };
        if last_ch.is_whitespace() || next_ch.is_whitespace() {
            return false;
        }
        // Do not insert spaces between CJK characters when wrapping lines.
        if Self::is_cjk(last_ch) || Self::is_cjk(next_ch) {
            return false;
        }
        true
    }

    pub(crate) fn is_cjk(c: char) -> bool {
        matches!(
            c as u32,
            0x4E00..=0x9FFF
                | 0x3400..=0x4DBF
                | 0x3000..=0x303F
                | 0xFF00..=0xFFEF
        )
    }
}
