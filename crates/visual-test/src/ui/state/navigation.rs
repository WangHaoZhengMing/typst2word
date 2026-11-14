//! Case selection, Prev/Next keyboard navigation, filtering, and `.docx` / Editor synchronization.

use std::fs;

use gpui_kit::{App, Context, Entity, Window, component::input::EditorState};

use super::{ParityStudioView, StatusFilter, ZOOM_STEPS};
use crate::{
    model::{CaseStatus, Category},
    ui::{
        docx_parts::{InspectorDoc, list_docx_entries, read_docx_xml_part},
        typst_highlight::format_xml_for_display,
    },
};

impl ParityStudioView {
    pub(crate) fn active_editor_entity(&self) -> &Entity<EditorState> {
        match self.inspector_doc {
            InspectorDoc::TypstSource => &self.typst_editor,
            InspectorDoc::IrJson => &self.ir_editor,
            _ => &self.xml_editor,
        }
    }

    pub(crate) fn select_case(
        &mut self,
        case_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_case_id == case_id && self.synced_case_id == case_id {
            return;
        }
        self.selected_case_id = case_id.to_string();
        if let Some(c) = self.report.cases.iter().find(|c| c.id == case_id) {
            self.collapsed_categories.remove(&c.category);
            self.collapsed_suites.remove(&c.suite_rel_path);
        }
        self.sync_editors_for_selected_case(window, cx);
        cx.notify();
    }

    pub(crate) fn select_adjacent_case(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filtered = self.filtered_indices(cx);
        if filtered.is_empty() {
            return;
        }
        let cur_pos = filtered
            .iter()
            .position(|&idx| self.report.cases[idx].id == self.selected_case_id)
            .unwrap_or(0);
        let len = filtered.len() as isize;
        let next_pos = (cur_pos as isize + delta).rem_euclid(len) as usize;
        let next_id = self.report.cases[filtered[next_pos]].id.clone();
        self.select_case(&next_id, window, cx);
    }

    pub(crate) fn select_inspector_doc(
        &mut self,
        doc: InspectorDoc,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.inspector_doc = doc;
        self.show_right_inspector = true;
        if doc.is_xml() {
            self.sync_xml_editor_part(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn step_zoom(&mut self, delta: isize, cx: &mut Context<Self>) {
        let cur_idx = ZOOM_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((*a - self.zoom).abs())
                    .partial_cmp(&((*b - self.zoom).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(2);
        let next_idx = (cur_idx as isize + delta).clamp(0, ZOOM_STEPS.len() as isize - 1) as usize;
        self.zoom = ZOOM_STEPS[next_idx];
        cx.notify();
    }

    pub(crate) fn expand_all_folders(&mut self, cx: &mut Context<Self>) {
        self.collapsed_categories.clear();
        self.collapsed_suites.clear();
        cx.notify();
    }

    pub(crate) fn collapse_all_folders(&mut self, cx: &mut Context<Self>) {
        self.collapsed_categories = Category::ALL.into_iter().collect();
        cx.notify();
    }

    pub(crate) fn reset_all_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.category_filter = None;
        self.status_filter = StatusFilter::All;
        self.search_input.update(cx, |st, cx| {
            st.set_value("", window, cx);
        });
        self.ensure_filtered_selection(window, cx);
        cx.notify();
    }

    pub(crate) fn toggle_word_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.word_wrap = !self.word_wrap;
        let wrap = self.word_wrap;
        for editor in [&self.typst_editor, &self.ir_editor, &self.xml_editor] {
            editor.update(cx, |st, cx| {
                st.set_soft_wrap(wrap, window, cx);
            });
        }
        cx.notify();
    }

    pub(crate) fn toggle_pretty_xml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pretty_xml = !self.pretty_xml;
        let xml_display = if self.pretty_xml {
            format_xml_for_display(&self.raw_xml_cache)
        } else {
            self.raw_xml_cache.clone()
        };
        self.xml_editor.update(cx, |st, cx| {
            st.set_value(xml_display, window, cx);
        });
        cx.notify();
    }

    pub(crate) fn has_auto_expand_filter(&self, cx: &App) -> bool {
        self.status_filter != StatusFilter::All
            || self.category_filter.is_some()
            || !self.search_input.read(cx).value().trim().is_empty()
    }

    pub(crate) fn ensure_filtered_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filtered = self.filtered_indices(cx);
        if !filtered.is_empty()
            && !filtered
                .iter()
                .any(|&idx| self.report.cases[idx].id == self.selected_case_id)
        {
            self.selected_case_id = self.report.cases[filtered[0]].id.clone();
            self.sync_editors_for_selected_case(window, cx);
        }
    }

    pub(crate) fn sync_editors_for_selected_case(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(case) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
            .cloned()
        else {
            return;
        };

        let typst_src = case.typst_source.clone();
        let ir_src = if case.ir_json_path.is_empty() {
            "// Click ▶ Run (⌘R) above to generate ir.json\n".to_string()
        } else {
            fs::read_to_string(&case.ir_json_path)
                .unwrap_or_else(|_| "// Click ▶ Run (⌘R) above to generate ir.json\n".to_string())
        };

        self.available_docx_entries = list_docx_entries(&case.docx_path);
        self.synced_case_id = case.id.clone();

        self.typst_editor.update(cx, |st, cx| {
            st.set_value(typst_src, window, cx);
        });
        self.ir_editor.update(cx, |st, cx| {
            st.set_value(ir_src, window, cx);
        });

        self.sync_xml_editor_part(window, cx);
    }

    pub(crate) fn sync_xml_editor_part(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let xml_target = if self.inspector_doc.is_xml() {
            self.inspector_doc
        } else {
            InspectorDoc::DocumentXml
        };

        let Some(case) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
        else {
            return;
        };

        let raw_xml = read_docx_xml_part(&case.docx_path, &case.document_xml_path, xml_target);
        let xml_display = if self.pretty_xml {
            format_xml_for_display(&raw_xml)
        } else {
            raw_xml.clone()
        };

        self.raw_xml_cache = raw_xml;
        self.synced_xml_doc = xml_target;
        self.xml_editor.update(cx, |st, cx| {
            st.set_value(xml_display, window, cx);
        });
    }

    pub(crate) fn filtered_indices(&self, cx: &App) -> Vec<usize> {
        let query = self
            .search_input
            .read(cx)
            .value()
            .trim()
            .to_ascii_lowercase();
        self.report
            .cases
            .iter()
            .enumerate()
            .filter(|(_, c)| self.category_filter.is_none_or(|cat| c.category == cat))
            .filter(|(_, c)| match self.status_filter {
                StatusFilter::All => true,
                StatusFilter::Pass => c.status == CaseStatus::Pass,
                StatusFilter::Drift => c.status == CaseStatus::Drift,
                StatusFilter::Fail => c.status == CaseStatus::Fail,
                StatusFilter::IrOnly => c.status == CaseStatus::IrOnly,
            })
            .filter(|(_, c)| {
                if query.is_empty() {
                    true
                } else {
                    c.id.to_ascii_lowercase().contains(&query)
                        || c.title.to_ascii_lowercase().contains(&query)
                        || c.isolated_variable.to_ascii_lowercase().contains(&query)
                        || c.suite_rel_path.to_ascii_lowercase().contains(&query)
                        || c.target_func.to_ascii_lowercase().contains(&query)
                        || c.target_param.to_ascii_lowercase().contains(&query)
                }
            })
            .map(|(i, _)| i)
            .collect()
    }
}
