//! macOS Preview-style physical paper card with interactive Word render empty-state (`stage/paper_card.rs`).

use std::path::PathBuf;

use gpui_kit::{
    AnyElement, FontWeight, IntoElement as _, ParentElement as _, Styled as _,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Sizable as _,
        button::Button,
        h_flex, v_flex,
    },
    div, img,
    prelude::FluentBuilder as _,
    px, rgb,
};

use crate::{
    model::BBoxPt,
    ui::state::{ParityStudioView, RunEngine, RunScope},
};

/// Physical paper background for PDF raster comparison sheets (always white paper).
const PAPER_WHITE_HEX: u32 = 0xFFFFFF;
/// Crisp border around rasterized page bitmaps on studio backdrop.
const PAPER_BORDER_LIGHT_HEX: u32 = 0xCFD6DF;
const PAPER_BORDER_DARK_HEX: u32 = 0x3A3A42;

impl ParityStudioView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_paper_card(
        &self,
        id_tag: &str,
        title: &str,
        meta: &str,
        png_path: Option<String>,
        subtitle: Option<&str>,
        empty_hint: &str,
        offer_word_run_btn: bool,
        width_px: f32,
        aspect: f32,
        cx: &mut gpui_kit::Context<Self>,
    ) -> AnyElement {
        let has_file = png_path
            .as_ref()
            .is_some_and(|p| !p.is_empty() && PathBuf::from(p).exists());
        let is_dark = cx.theme().is_dark();
        let border_col = cx.theme().border;
        let muted_fg = cx.theme().muted_foreground;
        let radius_sm = cx.theme().radius / 2.0;
        let mono = cx.theme().mono_font_family.clone();
        let placeholder_h = (width_px * aspect).clamp(124.0, 380.0);
        let paper_border = if is_dark {
            rgb(PAPER_BORDER_DARK_HEX)
        } else {
            rgb(PAPER_BORDER_LIGHT_HEX)
        };

        v_flex()
            .w(px(width_px))
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .px_0p5()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .truncate()
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_xs()
                            .text_color(muted_fg)
                            .child(meta.to_string()),
                    ),
            )
            .child(if has_file {
                let p = PathBuf::from(png_path.unwrap());
                div()
                    .w(px(width_px))
                    .rounded(radius_sm)
                    .overflow_hidden()
                    .border_1()
                    .border_color(paper_border)
                    .bg(rgb(PAPER_WHITE_HEX))
                    .shadow_sm()
                    .child(img(p).w(px(width_px)))
                    .into_any_element()
            } else {
                v_flex()
                    .w(px(width_px))
                    .h(px(placeholder_h))
                    .px_4()
                    .gap_2p5()
                    .rounded(radius_sm)
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(border_col)
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(muted_fg)
                            .child(empty_hint.to_string()),
                    )
                    .when(offer_word_run_btn, |el| {
                        el.child(
                            Button::new(gpui_kit::SharedString::from(format!(
                                "card-run-word-{id_tag}"
                            )))
                            .outline()
                            .xsmall()
                            .icon(IconName::Play)
                            .loading(self.is_running)
                            .disabled(self.is_running)
                            .label("Render Word PDF")
                            .tooltip("Silently export selected case via Microsoft Word (⇧⌘R)")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.trigger_run_with(
                                    RunScope::SelectedCase,
                                    RunEngine::SilentWordBatch,
                                    true,
                                    window,
                                    cx,
                                );
                            })),
                        )
                    })
                    .into_any_element()
            })
            .when_some(subtitle, |el, sub| {
                el.child(
                    div()
                        .px_0p5()
                        .font_family(mono)
                        .text_xs()
                        .text_color(muted_fg)
                        .truncate()
                        .child(sub.to_string()),
                )
            })
            .into_any_element()
    }
}

pub(super) fn format_bbox_line(bbox: Option<BBoxPt>) -> Option<String> {
    bbox.map(|b| {
        format!(
            "Ink L:{:.1}  T:{:.1}  W:{:.1}  H:{:.1} pt",
            b.left_pt, b.top_pt, b.width_pt, b.height_pt
        )
    })
}
