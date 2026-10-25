use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives;

pub const BUG_CATEGORIES: &[&str] = &[
    "Editor",
    "Terminal",
    "Git",
    "Extensions",
    "Performance",
    "Other",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BugStage {
    Form,
    Consent,
    Submitting,
    Success,
    Failure,
}

#[derive(Debug, Serialize)]
struct BugPayload {
    category: String,
    summary: String,
    description: String,
    diagnostics: String,
}

#[derive(Debug, Deserialize)]
struct BugResponse {
    #[serde(rename = "issueUrl", default)]
    issue_url: String,
}

static NEXT_REPORT: AtomicU64 = AtomicU64::new(0);

struct Submission {
    child: Option<Child>,
    payload_path: std::path::PathBuf,
    response_path: std::path::PathBuf,
    owns_payload: bool,
    owns_response: bool,
}

impl Drop for Submission {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.owns_payload { let _ = fs::remove_file(&self.payload_path); }
        if self.owns_response { let _ = fs::remove_file(&self.response_path); }
    }
}

pub struct BugReportWidget {
    pub is_open: bool,
    pub stage: BugStage,
    pub focus: usize, // 0: Category, 1: Summary, 2: Description, 3: Send button
    pub category_idx: usize,
    pub summary: String,
    pub description: String,
    pub data_dir: String,
    pub home_dir: String,
    pub report_endpoint: String,
    pub issue_url: String,
    pub error_message: String,
    submission: Option<Submission>,
}

impl BugReportWidget {
    pub fn new(data_dir: &str, home_dir: &str) -> Self {
        let endpoint = std::env::var("TUIM_BUG_REPORT_URL")
            .unwrap_or_else(|_| "https://tuim-bug-report.workers.dev/report".to_string());
        Self {
            is_open: false,
            stage: BugStage::Form,
            focus: 1,
            category_idx: 0,
            summary: String::new(),
            description: String::new(),
            data_dir: data_dir.to_string(),
            home_dir: home_dir.to_string(),
            report_endpoint: endpoint,
            issue_url: String::new(),
            error_message: String::new(),
            submission: None,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        // Reopening must not strand a child by resetting its submission state.
        self.stage = if self.submission.is_some() { BugStage::Submitting } else { BugStage::Form };
    }

    pub fn modal_rect(screen: Rect) -> Rect {
        let w = 76u16.min(screen.w.saturating_sub(4)).max(42.min(screen.w));
        let h = 23u16.min(screen.h.saturating_sub(2)).max(16.min(screen.h));
        let x = screen.x + (screen.w.saturating_sub(w)) / 2;
        let y = screen.y + (screen.h.saturating_sub(h)) / 2;
        Rect::new(x, y, w, h)
    }

    pub fn redact_text(&self, input: &str) -> String {
        let mut s = if !self.home_dir.is_empty() {
            input.replace(&self.home_dir, "~")
        } else {
            input.to_string()
        };

        let secret_prefixes = [
            "ghp_",
            "github_pat_",
            "gho_",
            "ghu_",
            "ghs_",
            "ghr_",
            "sk-",
            "Bearer ",
            "token=",
            "api_key=",
            "apikey=",
            "secret=",
            "password=",
        ];

        for prefix in secret_prefixes {
            let mut cursor = 0;
            while let Some(offset) = s[cursor..].find(prefix) {
                let pos = cursor + offset;
                let start = pos + prefix.len();
                let end = s[start..]
                    .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '&')
                    .map(|off| start + off)
                    .unwrap_or(s.len());
                if end > start {
                    s.replace_range(start..end, "[REDACTED]");
                    cursor = start + "[REDACTED]".len();
                } else {
                    cursor = start;
                }
            }
        }
        s
    }

    pub fn submit(&mut self, diagnostics_json: &str) {
        self.submit_with_program(diagnostics_json, std::ffi::OsStr::new("curl"));
    }

    fn submit_with_program(&mut self, diagnostics_json: &str, program: &std::ffi::OsStr) {
        if self.submission.is_some() {
            return;
        }
        self.error_message.clear();
        self.issue_url.clear();
        let payload = BugPayload {
            category: BUG_CATEGORIES[self.category_idx].to_string(),
            summary: self.redact_text(&self.summary),
            description: self.redact_text(&self.description),
            diagnostics: self.redact_text(diagnostics_json),
        };
        let id = NEXT_REPORT.fetch_add(1, Ordering::Relaxed);
        let base = std::path::Path::new(&self.data_dir);
        let mut submission = Submission {
            child: None,
            payload_path: base.join(format!("bug-report-{}-{}.json", std::process::id(), id)),
            response_path: base.join(format!("bug-report-{}-{}.response", std::process::id(), id)),
            owns_payload: false,
            owns_response: false,
        };
        let result = (|| -> Result<Child, String> {
            use std::io::Write;
            let json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
            let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
                .open(&submission.payload_path).map_err(|e| format!("Cannot create report: {e}"))?;
            submission.owns_payload = true;
            file.write_all(&json).map_err(|e| format!("Cannot write report: {e}"))?;
            fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
                .open(&submission.response_path).map_err(|e| format!("Cannot create response file: {e}"))?;
            submission.owns_response = true;
            Command::new(program)
                .args(["--disable", "--silent", "--show-error", "--connect-timeout", "10",
                    "--max-time", "30", "--proto", "=http,https", "--output"])
                .arg(&submission.response_path)
                .args(["--write-out", "%{http_code}", "--request", "POST",
                    "--header", "Content-Type: application/json", "--data-binary"])
                .arg(format!("@{}", submission.payload_path.display()))
                .arg("--url").arg(&self.report_endpoint)
                .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
                .spawn().map_err(|e| format!("Cannot start report upload: {e}"))
        })();
        match result {
            Ok(child) => {
                submission.child = Some(child);
                self.submission = Some(submission);
                self.stage = BugStage::Submitting;
            }
            Err(error) => {
                self.error_message = error;
                self.stage = BugStage::Failure;
            }
        }
    }

    pub fn poll_submission(&mut self) -> bool {
        if self.stage != BugStage::Submitting { return false; }
        let Some(submission) = self.submission.as_mut() else { return false; };
        match submission.child.as_mut().unwrap().try_wait() {
            Ok(None) => return false,
            Err(error) => {
                self.error_message = format!("Cannot monitor report upload: {error}");
                self.stage = BugStage::Failure;
                self.submission = None;
                return true;
            }
            Ok(Some(_)) => {}
        }
        let mut submission = self.submission.take().unwrap();
        let result = submission.child.take().unwrap().wait_with_output();
        self.stage = BugStage::Failure;
        match result {
            Ok(output) if output.status.success() => {
                let code = String::from_utf8_lossy(&output.stdout);
                if matches!(code.trim(), "200" | "201") {
                    let response = fs::read(&submission.response_path).ok()
                        .and_then(|bytes| serde_json::from_slice::<BugResponse>(&bytes).ok());
                    if let Some(response) = response.filter(|r| r.issue_url.starts_with("https://")) {
                        self.issue_url = response.issue_url;
                        self.stage = BugStage::Success;
                    } else {
                        self.error_message = "Report service returned an invalid response.".to_string();
                    }
                } else {
                    self.error_message = format!("Submission failed (HTTP {}).", code.trim());
                }
            }
            Ok(output) => self.error_message = format!("Report upload failed ({}).", output.status),
            Err(error) => self.error_message = format!("Cannot finish report upload: {error}"),
        }
        true
    }

    pub fn handle_key(&mut self, ev: &InputEvent, diagnostics_json: &str) -> bool {
        if !self.is_open {
            return false;
        }
        let key = match ev {
            InputEvent::Key(k) => k.as_str(),
            InputEvent::Paste(text) => {
                if self.stage == BugStage::Form {
                    if self.focus == 1 {
                        for ch in text.chars() {
                            if ch != '\n' && ch != '\r' && self.summary.chars().count() < 120 {
                                self.summary.push(ch);
                            }
                        }
                    } else if self.focus == 2 {
                        for ch in text.chars() {
                            if self.description.chars().count() < 6000 {
                                self.description.push(ch);
                            }
                        }
                    }
                }
                return true;
            }
            _ => return false,
        };

        match self.stage {
            BugStage::Form => match key {
                "<Esc>" => {
                    self.is_open = false;
                    true
                }
                "<Tab>" => {
                    self.focus = (self.focus + 1) % 4;
                    true
                }
                "<C-s>" => {
                    if !self.summary.trim().is_empty() {
                        self.stage = BugStage::Consent;
                    }
                    true
                }
                _ => {
                    if self.focus == 0 {
                        match key {
                            "h" | "<Left>" => {
                                self.category_idx =
                                    (self.category_idx + BUG_CATEGORIES.len() - 1)
                                        % BUG_CATEGORIES.len();
                            }
                            "l" | "<Right>" | " " | "<CR>" => {
                                self.category_idx = (self.category_idx + 1) % BUG_CATEGORIES.len();
                            }
                            _ => {}
                        }
                    } else if self.focus == 1 {
                        match key {
                            "<BS>" => {
                                self.summary.pop();
                            }
                            "<CR>" | "<Down>" => {
                                self.focus = 2;
                            }
                            k if k.chars().count() == 1 && !k.starts_with('<') => {
                                if self.summary.chars().count() < 120 {
                                    self.summary.push_str(k);
                                }
                            }
                            _ => {}
                        }
                    } else if self.focus == 2 {
                        match key {
                            "<BS>" => {
                                self.description.pop();
                            }
                            "<CR>" => {
                                if self.description.chars().count() < 6000 {
                                    self.description.push('\n');
                                }
                            }
                            k if k.chars().count() == 1 && !k.starts_with('<') => {
                                if self.description.chars().count() < 6000 {
                                    self.description.push_str(k);
                                }
                            }
                            _ => {}
                        }
                    } else if self.focus == 3 && key == "<CR>" && !self.summary.trim().is_empty() {
                        self.stage = BugStage::Consent;
                    }
                    true
                }
            },
            BugStage::Consent => match key {
                "y" | "Y" | "<CR>" => {
                    self.submit(diagnostics_json);
                    true
                }
                "n" | "N" | "<Esc>" => {
                    self.stage = BugStage::Form;
                    true
                }
                _ => true,
            },
            BugStage::Submitting => {
                if key == "<Esc>" {
                    self.is_open = false;
                }
                true
            }
            BugStage::Success | BugStage::Failure => {
                if matches!(key, "<Esc>" | "<CR>" | "q") {
                    self.is_open = false;
                }
                true
            }
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, screen: Rect, theme: &Theme) {
        if !self.is_open {
            return;
        }
        let mr = Self::modal_rect(screen);
        let c = chrome(theme);

        primitives::draw_modal_frame(renderer, mr, " Report a Bug ", theme);

        match self.stage {
            BugStage::Form => {
                let cat_bg = if self.focus == 0 {
                    c.row_selected_bg
                } else {
                    c.surface_bg
                };
                renderer.draw_text(mr.x + 2, mr.y + 2, "Category:", c.text_muted, c.panel_bg);
                renderer.draw_text_styled(
                    mr.x + 14,
                    mr.y + 2,
                    &format!(" < {} > ", BUG_CATEGORIES[self.category_idx]),
                    c.text_strong,
                    cat_bg,
                    self.focus == 0,
                    false,
                );

                renderer.draw_text(mr.x + 2, mr.y + 4, "Summary:", c.text_muted, c.panel_bg);
                primitives::draw_text_input(
                    renderer,
                    Rect::new(mr.x + 2, mr.y + 5, mr.w.saturating_sub(4), 1),
                    &self.summary,
                    "What went wrong?",
                    self.focus == 1,
                    theme,
                );

                renderer.draw_text(
                    mr.x + 2,
                    mr.y + 7,
                    "Description / Steps to Reproduce:",
                    c.text_muted,
                    c.panel_bg,
                );
                let desc_r = Rect::new(
                    mr.x + 2,
                    mr.y + 8,
                    mr.w.saturating_sub(4),
                    mr.h.saturating_sub(12),
                );
                let desc_bg = if self.focus == 2 {
                    c.row_selected_bg
                } else {
                    c.surface_bg
                };
                renderer.fill_rect(desc_r, desc_bg);
                for (i, line) in self.description.lines().enumerate() {
                    if i >= desc_r.h as usize {
                        break;
                    }
                    let trunc = primitives::truncate_to_width(line, desc_r.w.saturating_sub(2) as usize);
                    renderer.draw_text(desc_r.x + 1, desc_r.y + i as u16, &trunc, c.text_strong, desc_bg);
                }

                let btn_bg = if self.focus == 3 { c.accent } else { c.surface_bg };
                let btn_fg = if self.focus == 3 { c.panel_bg } else { c.text_strong };
                renderer.draw_text_styled(
                    mr.x + mr.w.saturating_sub(12),
                    mr.y + mr.h.saturating_sub(2),
                    " [ Send ] ",
                    btn_fg,
                    btn_bg,
                    true,
                    false,
                );
            }
            BugStage::Consent => {
                renderer.draw_text_styled(
                    mr.x + 2,
                    mr.y + 3,
                    "Confirm Bug Report Submission",
                    c.text_strong,
                    c.panel_bg,
                    true,
                    false,
                );
                renderer.draw_text(
                    mr.x + 2,
                    mr.y + 5,
                    "Redacted runtime diagnostics will be attached (paths & tokens stripped).",
                    c.text_muted,
                    c.panel_bg,
                );
                renderer.draw_text(
                    mr.x + 4,
                    mr.y + mr.h.saturating_sub(4),
                    "[Y / Enter] Submit Report",
                    theme.success,
                    c.panel_bg,
                );
                renderer.draw_text(
                    mr.x + 34,
                    mr.y + mr.h.saturating_sub(4),
                    "[N / Esc] Back",
                    c.text_muted,
                    c.panel_bg,
                );
            }
            BugStage::Submitting => {
                renderer.draw_text(
                    mr.x + 2,
                    mr.y + 5,
                    "Submitting bug report...",
                    c.accent,
                    c.panel_bg,
                );
            }
            BugStage::Success => {
                renderer.draw_text_styled(
                    mr.x + 2,
                    mr.y + 4,
                    "Bug report submitted! Thank you.",
                    theme.success,
                    c.panel_bg,
                    true,
                    false,
                );
                if !self.issue_url.is_empty() {
                    renderer.draw_text(
                        mr.x + 2,
                        mr.y + 6,
                        &format!("Issue URL: {}", self.issue_url),
                        c.text,
                        c.panel_bg,
                    );
                }
            }
            BugStage::Failure => {
                renderer.draw_text_styled(
                    mr.x + 2,
                    mr.y + 4,
                    &self.error_message,
                    theme.error,
                    c.panel_bg,
                    true,
                    false,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (std::path::PathBuf, BugReportWidget) {
        let id = NEXT_REPORT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tuim-report-test-{}-{id}' space", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let widget = BugReportWidget::new(dir.to_str().unwrap(), "/home/test");
        (dir, widget)
    }

    #[test]
    fn redaction_terminates_for_repeated_empty_and_unicode_secrets() {
        let widget = BugReportWidget::new("", "/home/test");
        for (input, expected) in [
            ("ghp_abc ghp_def", "ghp_[REDACTED] ghp_[REDACTED]"),
            ("token= token=abc&password=秘密", "token= token=[REDACTED]&password=[REDACTED]"),
            ("token=", "token="), ("sk-[REDACTED]", "sk-[REDACTED]"),
            ("/home/test/日本語 api_key=鍵", "~/日本語 api_key=[REDACTED]"),
        ] {
            assert_eq!(widget.redact_text(input), expected);
            assert_eq!(widget.redact_text(expected), expected);
        }
    }

    #[test]
    fn upload_start_and_write_errors_cleanup_and_allow_retry() {
        let (dir, mut widget) = fixture();
        widget.submit_with_program("{}", dir.join("missing-curl").as_os_str());
        assert_eq!(widget.stage, BugStage::Failure);
        assert!(widget.error_message.contains("Cannot start"));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        widget.data_dir = dir.join("missing-directory").to_str().unwrap().into();
        widget.submit_with_program("{}", std::ffi::OsStr::new("curl"));
        assert_eq!(widget.stage, BugStage::Failure);
        assert!(widget.error_message.contains("Cannot create"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn uploads_use_literal_arguments_and_validate_transport_http_and_response() {
        for (code, response, exit, expected) in [
            ("201", r#"{"issueUrl":"https://example.invalid/issues/1"}"#, 0, BugStage::Success),
            ("500", "{}", 0, BugStage::Failure),
            ("200", "not json", 0, BugStage::Failure),
            ("200", r#"{"issueUrl":"http://example.invalid"}"#, 0, BugStage::Failure),
            ("000", "", 7, BugStage::Failure),
        ] {
            let (dir, mut widget) = fixture();
            let program = dir.join("mock-curl");
            let endpoint = "https://example.invalid/' ; touch INJECTION ; $(false)";
            widget.report_endpoint = endpoint.into();
            let script = format!(r##"#!/bin/sh
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) shift; response="$1" ;;
        --url) shift; endpoint="$1" ;;
        --data-binary) shift; payload="${{1#@}}" ;;
    esac
    shift
done
[ "$endpoint" = "https://example.invalid/' ; touch INJECTION ; \$(false)" ] || exit 99
[ -f "$payload" ] || exit 98
printf '%s' '{response}' > "$response"
printf '%s' '{code}'
exit {exit}
"##);
            fs::write(&program, script).unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
            widget.submit_with_program("{}", program.as_os_str());
            widget.is_open = false;
            widget.open();
            assert_eq!(widget.stage, BugStage::Submitting);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !widget.poll_submission() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert_eq!(widget.stage, expected, "{}", widget.error_message);
            assert!(widget.submission.is_none());
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn dropping_inflight_upload_reaps_child_and_removes_private_files() {
        let (dir, mut widget) = fixture();
        widget.submit_with_program("{}", std::ffi::OsStr::new("/bin/cat"));
        let submission = widget.submission.as_ref().unwrap();
        let pid = submission.child.as_ref().unwrap().id();
        assert_eq!(fs::metadata(&submission.payload_path).unwrap().permissions().mode() & 0o777, 0o600);
        drop(widget);
        assert_eq!(unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) }, -1);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }
}
