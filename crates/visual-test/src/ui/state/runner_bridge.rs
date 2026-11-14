//! Asynchronous GUI <-> Silent Mega-Doc `run_suite` execution bridge & external openers.

use std::time::Duration;

use gpui_kit::{Context, Window};

use super::{ParityStudioView, RunEngine, RunScope};
use crate::{
    runner::{RunConfig, run_suite},
    ui::{
        docx_parts::{open_file_in_preview, open_file_in_word, reveal_in_finder},
        load_or_placeholder_report,
    },
};

impl ParityStudioView {
    /// Run whatever `RunScope` + `RunEngine` is currently selected in the Xcode-style Scheme bar.
    pub(crate) fn trigger_active_scheme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fresh = self.force_fresh_word
            || (self.run_scope == RunScope::SelectedCase
                && self.run_engine == RunEngine::SilentWordBatch);
        self.trigger_run_with(self.run_scope, self.run_engine, fresh, window, cx);
    }

    /// Execute `run_suite` in the background using the silent sandbox Word Mega-Doc pipeline
    /// (or fast IR-only mode) and merge updated telemetry back into `report.json` on completion.
    pub(crate) fn trigger_run_with(
        &mut self,
        scope: RunScope,
        engine: RunEngine,
        fresh_word: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_running {
            return;
        }

        let active_case = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id);

        let mut cfg = RunConfig {
            out_dir: self.report_dir.clone(),
            skip_word: engine == RunEngine::IrOnly,
            fresh_word: fresh_word && engine == RunEngine::SilentWordBatch,
            raster_scale: 2.0,
            ..RunConfig::default()
        };

        let target_desc = match scope {
            RunScope::SelectedCase => {
                if self.selected_case_id.is_empty() {
                    return;
                }
                cfg.exact_case_id = Some(self.selected_case_id.clone());
                format!("case `{}`", self.selected_case_id)
            }
            RunScope::CurrentSuite => {
                let Some(case) = active_case else {
                    return;
                };
                let suite_path = case.suite_rel_path.clone();
                let n = self
                    .report
                    .cases
                    .iter()
                    .filter(|c| c.suite_rel_path == suite_path)
                    .count();
                cfg.suite_filter = Some(suite_path.clone());
                format!("suite `{suite_path}` ({n} cases)")
            }
            RunScope::CurrentCategory => {
                let Some(case) = active_case else {
                    return;
                };
                let cat = case.category;
                let n = self.report.cases.iter().filter(|c| c.category == cat).count();
                cfg.category_filter = Some(cat);
                format!("{} ({n} cases)", cat.title())
            }
            RunScope::FullSuite => format!("full catalog ({} cases)", self.report.cases.len()),
        };

        let engine_desc = match engine {
            RunEngine::SilentWordBatch => "Silent Word Batch",
            RunEngine::IrOnly => "Fast IR & XML",
        };

        self.is_running = true;
        self.banner_message = Some(format!("Running {target_desc} · {engine_desc}…"));
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { run_suite(&cfg) })
                .await;
            let _ = cx.background_executor().timer(Duration::from_millis(16)).await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.is_running = false;
                match res {
                    Ok(rep) => {
                        if rep.word_enabled {
                            view.banner_message = Some(format!(
                                "Done ({} ran) · {} pass · {} drift · {} fail",
                                rep.cases.len(),
                                rep.stats.pass_count,
                                rep.stats.drift_count,
                                rep.stats.fail_count
                            ));
                        } else {
                            let ok = rep.cases.len().saturating_sub(rep.stats.fail_count);
                            view.banner_message = Some(format!(
                                "Verified {ok}/{} case(s) (IR & XML) · {} fail",
                                rep.cases.len(),
                                rep.stats.fail_count
                            ));
                        }
                        view.reload_from_disk(window, cx);
                    }
                    Err(err) => {
                        view.banner_message = Some(format!("Run error: {err}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn reload_from_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.report = load_or_placeholder_report(&self.report_dir);
        if !self
            .report
            .cases
            .iter()
            .any(|c| c.id == self.selected_case_id)
            && let Some(first) = self.report.cases.first()
        {
            self.selected_case_id = first.id.clone();
        }
        self.synced_case_id.clear();
        self.sync_editors_for_selected_case(window, cx);
    }

    pub(crate) fn open_selected_docx_in_word(&mut self, cx: &mut Context<Self>) {
        if let Some(case) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
        {
            match open_file_in_word(&case.docx_path) {
                Ok(()) => {
                    self.banner_message =
                        Some(format!("Opened `{}/output.docx` in Microsoft Word", case.id));
                }
                Err(err) => {
                    self.banner_message = Some(err);
                }
            }
            cx.notify();
        }
    }

    pub(crate) fn open_selected_pdf_in_preview(&mut self, word_pdf: bool, cx: &mut Context<Self>) {
        if let Some(case) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
        {
            let target = if word_pdf {
                case.word_pdf_path.as_deref().unwrap_or("")
            } else {
                case.typst_pdf_path.as_str()
            };
            match open_file_in_preview(target) {
                Ok(()) => {
                    let kind = if word_pdf { "word.pdf" } else { "typst.pdf" };
                    self.banner_message = Some(format!("Opened `{}/{kind}` in Preview", case.id));
                }
                Err(err) => {
                    self.banner_message = Some(err);
                }
            }
            cx.notify();
        }
    }

    pub(crate) fn reveal_selected_case_in_finder(&mut self, cx: &mut Context<Self>) {
        if let Some(case) = self
            .report
            .cases
            .iter()
            .find(|c| c.id == self.selected_case_id)
        {
            match reveal_in_finder(&case.case_dir) {
                Ok(()) => {
                    self.banner_message = Some(format!("Revealed `{}` in Finder", case.id));
                }
                Err(err) => {
                    self.banner_message = Some(err);
                }
            }
            cx.notify();
        }
    }
}
