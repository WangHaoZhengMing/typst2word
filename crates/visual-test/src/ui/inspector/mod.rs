//! Right column: Xcode-style Code & OpenXML Package Inspector (`inspector/mod.rs`).
//!
//! Replaces wide horizontal tab strips and inline option buttons with two compact
//! macOS pop-up dropdown menus:
//! 1. **Document Selector (`[ 📄 Typst (.typ) ▾ ]`)**: switches seamlessly between
//!    Typst source, Semantic IR (`ir.json`), and **8 internal `.docx` OpenXML parts**
//!    (`word/document.xml`, `settings.xml`, `styles.xml`, `numbering.xml`,
//!    `footnotes.xml`, `header1.xml`, `footer1.xml`, `document.xml.rels`)!
//! 2. **Options Menu (`[ ⚙ ▾ ]`)**: soft line wrap, XML pretty-indentation, and
//!    quick desktop reveal actions.

use gpui_kit::{
    Anchor, IntoElement, ParentElement as _, Styled as _, Window,
    assets::IconName as Lucide,
    component::{
        ActiveTheme as _, IconName, Side, Sizable as _,
        button::{Button, ButtonVariants as _},
        clipboard::Clipboard,
        h_flex,
        input::Editor,
        menu::{DropdownMenu as _, PopupMenuItem},
        v_flex,
    },
    div, px, relative,
};

use crate::{
    model::CaseReport,
    ui::{docx_parts::InspectorDoc, state::ParityStudioView},
};

impl ParityStudioView {
    pub(crate) fn render_inspector_panel(
        &self,
        _case: &CaseReport,
        _window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_col = theme.border;
        let header_bg = theme.background;

        let cur_doc = self.inspector_doc;
        let doc_icon = match cur_doc {
            InspectorDoc::TypstSource => Lucide::FileCode,
            InspectorDoc::IrJson => Lucide::FileText,
            _ => Lucide::Package,
        };

        let active_editor = self.active_editor_entity();
        let copy_editor = active_editor.clone();

        let docx_entries = self.available_docx_entries.clone();
        let cur_wrap = self.word_wrap;
        let cur_pretty = self.pretty_xml;

        let view_for_doc = cx.entity();
        let view_for_opts = cx.entity();

        v_flex()
            .size_full()
            .bg(header_bg)
            .overflow_hidden()
            .child(
                // 38px compact Xcode Inspector header aligned with Sidebar & Center Stage
                h_flex()
                    .w_full()
                    .h(px(38.0))
                    .px_2p5()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border_col)
                    .child(
                        Button::new("inspector-doc-dropdown")
                            .outline()
                            .xsmall()
                            .icon(doc_icon)
                            .dropdown_caret(true)
                            .label(cur_doc.short_label())
                            .tooltip("Select Source, IR, or OpenXML Package Part (⌘1–⌘4)")
                            .dropdown_menu(move |mut menu, window, _cx| {
                                let v_typ = view_for_doc.clone();
                                let v_ir = view_for_doc.clone();
                                menu = menu
                                    .min_w(px(290.0))
                                    .check_side(Side::Left)
                                    .label("Compiler & IR")
                                    .item(
                                        PopupMenuItem::new(InspectorDoc::TypstSource.menu_label())
                                            .checked(cur_doc == InspectorDoc::TypstSource)
                                            .on_click(window.listener_for(
                                                &v_typ,
                                                |this, _, window, cx| {
                                                    this.select_inspector_doc(
                                                        InspectorDoc::TypstSource,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )),
                                    )
                                    .item(
                                        PopupMenuItem::new(InspectorDoc::IrJson.menu_label())
                                            .checked(cur_doc == InspectorDoc::IrJson)
                                            .on_click(window.listener_for(
                                                &v_ir,
                                                |this, _, window, cx| {
                                                    this.select_inspector_doc(
                                                        InspectorDoc::IrJson,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )),
                                    )
                                    .separator()
                                    .label("OpenXML Package (output.docx)");

                                for xml_doc in &InspectorDoc::ALL[2..] {
                                    let doc_val = *xml_doc;
                                    let v_xml = view_for_doc.clone();
                                    let is_optional = matches!(
                                        doc_val,
                                        InspectorDoc::NumberingXml
                                            | InspectorDoc::FootnotesXml
                                            | InspectorDoc::HeaderXml
                                            | InspectorDoc::FooterXml
                                    );
                                    let missing_optional = is_optional
                                        && !docx_entries.is_empty()
                                        && doc_val
                                            .docx_entry_name()
                                            .is_some_and(|e| !docx_entries.contains(e));

                                    menu = menu.item(
                                        PopupMenuItem::new(doc_val.menu_label())
                                            .checked(cur_doc == doc_val)
                                            .disabled(missing_optional)
                                            .on_click(window.listener_for(
                                                &v_xml,
                                                move |this, _, window, cx| {
                                                    this.select_inspector_doc(doc_val, window, cx);
                                                },
                                            )),
                                    );
                                }

                                menu
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_0p5()
                            .child(
                                Button::new("inspector-find-btn")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Search)
                                    .tooltip("Find in Code (⌘F)")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let ed = this.active_editor_entity().clone();
                                        ed.update(cx, |state, cx| {
                                            state.focus(window, cx);
                                            state.open_search(false, cx);
                                        });
                                    })),
                            )
                            .child(
                                Clipboard::new("inspector-copy-code")
                                    .xsmall()
                                    .tooltip("Copy Active Buffer to Clipboard")
                                    .value_fn(move |_, cx| copy_editor.read(cx).value()),
                            )
                            .child(
                                Button::new("inspector-options-dropdown")
                                    .ghost()
                                    .xsmall()
                                    .icon(Lucide::Settings2)
                                    .dropdown_caret(true)
                                    .tooltip("Editor & Formatting Options")
                                    .dropdown_menu_with_anchor(
                                        Anchor::TopRight,
                                        move |menu, window, _cx| {
                                            let v1 = view_for_opts.clone();
                                            let v2 = view_for_opts.clone();
                                            let v3 = view_for_opts.clone();
                                            let v4 = view_for_opts.clone();
                                            menu.min_w(px(230.0))
                                                .check_side(Side::Left)
                                                .label("Editor Display")
                                                .item(
                                                    PopupMenuItem::new("Soft Wrap Lines")
                                                        .checked(cur_wrap)
                                                        .on_click(window.listener_for(
                                                            &v1,
                                                            |this, _, window, cx| {
                                                                this.toggle_word_wrap(window, cx);
                                                            },
                                                        )),
                                                )
                                                .item(
                                                    PopupMenuItem::new(
                                                        "Pretty-Format OpenXML Tags",
                                                    )
                                                    .checked(cur_pretty)
                                                    .on_click(window.listener_for(
                                                        &v2,
                                                        |this, _, window, cx| {
                                                            this.toggle_pretty_xml(window, cx);
                                                        },
                                                    )),
                                                )
                                                .separator()
                                                .item(
                                                    PopupMenuItem::new(
                                                        "Open output.docx in Word",
                                                    )
                                                    .on_click(window.listener_for(
                                                        &v3,
                                                        |this, _, _, cx| {
                                                            this.open_selected_docx_in_word(cx);
                                                        },
                                                    )),
                                                )
                                                .item(
                                                    PopupMenuItem::new(
                                                        "Reveal Case Folder in Finder",
                                                    )
                                                    .on_click(window.listener_for(
                                                        &v4,
                                                        |this, _, _, cx| {
                                                            this.reveal_selected_case_in_finder(cx);
                                                        },
                                                    )),
                                                )
                                        },
                                    ),
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
