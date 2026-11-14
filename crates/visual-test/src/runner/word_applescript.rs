//! Low-level silent Microsoft Word AppleScript driver for macOS.
//!
//! Eliminates UI focus stealing, window popups, macOS sandbox *Grant File Access* prompts,
//! and `margin: 0pt` printable-area modal dialogs by:
//! 1. Staging all `.docx` inputs and `.pdf` outputs inside Microsoft Word's own App Sandbox
//!    temporary container (`~/Library/Containers/com.microsoft.Word/Data/tmp/...`).
//! 2. Pre-launching Word in the background without focus (`open -g -j -a "Microsoft Word"`)
//!    and keeping the process hidden when launched by the test runner.
//! 3. Suppressing startup galleries and modal alerts (`set startup dialog to false`,
//!    `set display alerts to alerts none`, `with read only without add to recent files`).

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// A staged `.docx` -> `.pdf` export pair inside Word's sandbox staging directory.
#[derive(Debug, Clone)]
pub(super) struct StagedWordJob {
    pub staged_docx: PathBuf,
    pub staged_pdf: PathBuf,
}

/// Session handle tracking whether Word was launched silently by this runner.
pub(super) struct SilentWordSession {
    pub stage_dir: PathBuf,
    launched_by_runner: bool,
}

impl SilentWordSession {
    /// Prepare the sandbox staging directory and ensure Microsoft Word is running silently.
    pub fn prepare() -> Result<Self, String> {
        let stage_dir = resolve_word_stage_dir();
        let _ = fs::remove_dir_all(&stage_dir);
        fs::create_dir_all(&stage_dir)
            .map_err(|e| format!("create Word sandbox stage dir {}: {e}", stage_dir.display()))?;

        let was_running = is_word_running();
        if !was_running {
            launch_word_in_background()?;
        } else {
            wait_for_word_ready(Duration::from_secs(15));
        }

        Ok(Self {
            stage_dir,
            launched_by_runner: !was_running,
        })
    }

    /// Convert a slice of staged `.docx` files into `.pdf` in a single silent AppleScript session.
    pub fn export_jobs(&self, jobs: &[StagedWordJob], timeout: Duration) -> Result<(), String> {
        if jobs.is_empty() {
            return Ok(());
        }

        for job in jobs {
            let _ = fs::remove_file(&job.staged_pdf);
        }

        let apple_timeout_secs = timeout.as_secs().saturating_sub(3).max(15);
        let mut lines = vec![
            format!("with timeout of {apple_timeout_secs} seconds"),
            "tell application \"Microsoft Word\"".to_string(),
            "  try".to_string(),
            "    set startup dialog to false".to_string(),
            "  end try".to_string(),
            "  set prevAlerts to display alerts".to_string(),
            "  set display alerts to alerts none".to_string(),
            "  try".to_string(),
            "    repeat with d in (every document whose name starts with \"typst2word_stage_\")".to_string(),
            "      close d saving no".to_string(),
            "    end repeat".to_string(),
            "  end try".to_string(),
            "  try".to_string(),
        ];

        for job in jobs {
            let docx_esc = escape_applescript_path(&job.staged_docx);
            let pdf_esc = escape_applescript_path(&job.staged_pdf);
            lines.push(format!(
                "    open POSIX file \"{docx_esc}\" with read only without add to recent files"
            ));
            lines.push("    set targetDoc to active document".to_string());
            lines.push(format!(
                "    save as targetDoc file name \"{pdf_esc}\" file format format PDF without add to recent files"
            ));
            lines.push("    close targetDoc saving no".to_string());
        }

        lines.extend([
            "  on error errMsg number errNum".to_string(),
            "    try".to_string(),
            "      set display alerts to prevAlerts".to_string(),
            "    end try".to_string(),
            "    error errMsg number errNum".to_string(),
            "  end try".to_string(),
            "  try".to_string(),
            "    set display alerts to prevAlerts".to_string(),
            "  end try".to_string(),
            "end tell".to_string(),
            "end timeout".to_string(),
        ]);

        let script = lines.join("\n");
        let res = run_osascript_with_timeout(&script, timeout);

        if self.launched_by_runner {
            hide_word_process_best_effort();
        }

        res
    }
}

impl Drop for SilentWordSession {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.stage_dir);
    }
}

/// Resolve a staging path inside Microsoft Word's macOS App Sandbox container (`Data/tmp`)
/// so Word never prompts for external folder access. Falls back to `/private/tmp` only if
/// Word's container directory does not exist yet.
fn resolve_word_stage_dir() -> PathBuf {
    let pid = std::process::id();
    if let Some(home) = env::var_os("HOME") {
        let container_data =
            PathBuf::from(home).join("Library/Containers/com.microsoft.Word/Data");
        if container_data.is_dir() {
            return container_data
                .join("tmp")
                .join(format!("typst2word_visual_test_stage_{pid}"));
        }
    }
    PathBuf::from(format!("/private/tmp/typst2word_visual_test_stage_{pid}"))
}

fn is_word_running() -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg("Microsoft Word")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn launch_word_in_background() -> Result<(), String> {
    let status = Command::new("open")
        .args(["-g", "-j", "-a", "Microsoft Word"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("failed to background-launch Microsoft Word: {e}"))?;

    if !status.success() {
        return Err(format!(
            "`open -g -j -a \"Microsoft Word\"` exited with {status}"
        ));
    }

    wait_for_word_ready(Duration::from_secs(18));
    hide_word_process_best_effort();
    Ok(())
}

fn wait_for_word_ready(max_wait: Duration) {
    let deadline = Instant::now() + max_wait;
    while Instant::now() < deadline {
        if is_word_running()
            && run_osascript_with_timeout(
                "tell application \"Microsoft Word\" to get version",
                Duration::from_secs(4),
            )
            .is_ok()
        {
            return;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn hide_word_process_best_effort() {
    let _ = run_osascript_with_timeout(
        "tell application \"System Events\" to if exists process \"Microsoft Word\" then set visible of process \"Microsoft Word\" to false",
        Duration::from_secs(2),
    );
}

fn escape_applescript_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn run_osascript_with_timeout(script: &str, timeout: Duration) -> Result<(), String> {
    let mut child = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn osascript: {e}"))?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let mut stderr = String::new();
                if let Some(mut err_pipe) = child.stderr.take() {
                    let _ = err_pipe.read_to_string(&mut stderr);
                }
                return Err(format!("osascript exited with {status}: {}", stderr.trim()));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "osascript timed out after {}s (Word may be blocked by a modal prompt)",
                        timeout.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(80));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("osascript wait error: {e}"));
            }
        }
    }
}
