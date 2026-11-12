//! Right column: Syntax-highlighted `Editor` Code Inspector (`code_panel.rs`).
//!
//! Hosts three live read-only `gpui_kit::component::input::Editor` buffers:
//! - Typst Source (`.typ`) with custom Typst syntax highlighting & folding
//! - Extracted IR (`ir.json`) with `syntect` JSON syntax highlighting & folding
//! - Word OpenXML (`document.xml`) with `syntect` XML syntax highlighting & pretty-formatting

use gpui_kit::{
    IntoElement, ParentElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
    relative,
    component::{
        ActiveTheme as _, IconName, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        clipboard::Clipboard,
        h_flex,
        input::Editor,
        tab::{Tab, TabBar},
        v_flex,
    },
};

use crate::{
    model::CaseReport,
    ui::state::{CodeTab, ParityStudioView},
};

impl ParityStudioView {
    pub(crate) fn render_code_panel(
        &self,
        _case: &CaseReport,
        _window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_col = theme.border;
        let header_bg = theme.background;

        let tab_idx = match self.code_tab {
            CodeTab::Typst => 0,
            CodeTab::IrJson => 1,
            CodeTab::Ooxml => 2,
        };

        let active_editor = self.active_editor_entity();
        let copy_editor = active_editor.clone();

        v_flex()
            .size_full()
            .bg(header_bg)
            .overflow_hidden()
            .child(
                // Aligned 40px top toolbar matching center stage header height
                h_flex()
                    .w_full()
                    .h(px(40.0))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border_col)
                    .child(
                        TabBar::new("right-code-tabs")
                            .segmented()
                            .xsmall()
                            .selected_index(tab_idx)
                            .children([
                                Tab::new().label("Typst"),
                                Tab::new().label("IR"),
                                Tab::new().label("Word XML"),
                            ])
                            .on_click(cx.listener(|this, idx, _, cx| {
                                this.code_tab = match *idx {
                                    1 => CodeTab::IrJson,
                                    2 => CodeTab::Ooxml,
                                    _ => CodeTab::Typst,
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_0p5()
                            .when(self.code_tab == CodeTab::Ooxml, |el| {
                                el.child(
                                    Button::new("btn-toggle-pretty-xml")
                                        .ghost()
                                        .xsmall()
                                        .selected(self.pretty_xml)
                                        .label("Format")
                                        .tooltip("Toggle pretty-indented OpenXML tags & attributes")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.toggle_pretty_xml(window, cx);
                                        })),
                                )
                            })
                            .child(
                                Button::new("btn-toggle-word-wrap")
                                    .ghost()
                                    .xsmall()
                                    .selected(self.word_wrap)
                                    .label("Wrap")
                                    .tooltip("Toggle soft line wrapping in Editor")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_word_wrap(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-open-editor-find")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Search)
                                    .tooltip("Find in Editor (⌘F)")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let ed = this.active_editor_entity().clone();
                                        ed.update(cx, |state, cx| {
                                            state.focus(window, cx);
                                            state.open_search(false, cx);
                                        });
                                    })),
                            )
                            .child(
                                Clipboard::new("copy-active-code")
                                    .xsmall()
                                    .tooltip("Copy code to clipboard")
                                    .value_fn(move |_, cx| copy_editor.read(cx).value()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(
                        Editor::new(active_editor)
                            .readonly(true)
                            .bordered(false)
                            .xsmall()
                            .w_full()
                            .h(relative(1.0)),
                    ),
            )
    }
}
