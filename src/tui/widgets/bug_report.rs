use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

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

pub struct BugReportWidget {
    pub is_open: bool,
    pub stage: BugStage,
    pub focus: usize, // 0: Category, 1: Summary, 2: Description, 3: Send button
    pub category_idx: usize,
    pub summary: String,
    pub description: String,
    pub home_dir: String,
    pub report_endpoint: String,
    pub issue_url: String,
    pub error_message: String,
    submission: Option<Receiver<Result<String, String>>>,
}

impl BugReportWidget {
    pub fn new(home_dir: &str) -> Self {
        let endpoint = std::env::var("TUIM_BUG_REPORT_URL")
            .ok().filter(|url| !url.trim().is_empty())
            .or_else(|| option_env!("TUIM_BUG_REPORT_URL")
                .filter(|url| !url.trim().is_empty()).map(str::to_string))
            .unwrap_or_else(|| "https://tuim-bug-report.workers.dev/report".to_string());
        Self {
            is_open: false,
            stage: BugStage::Form,
            focus: 1,
            category_idx: 0,
            summary: String::new(),
            description: String::new(),
            home_dir: home_dir.to_string(),
            report_endpoint: endpoint,
            issue_url: String::new(),
            error_message: String::new(),
            submission: None,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        if self.submission.is_some() {
            self.stage = BugStage::Submitting;
        } else if self.stage == BugStage::Success {
            self.summary.clear();
            self.description.clear();
            self.issue_url.clear();
            self.stage = BugStage::Form;
        }
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
        if self.submission.is_some() {
            return;
        }
        self.error_message.clear();
        self.issue_url.clear();
        if self.summary.trim().is_empty() {
            self.error_message = "Please enter a summary before sending.".to_string();
            self.focus = 1;
            return;
        }
        let payload = BugPayload {
            category: BUG_CATEGORIES[self.category_idx].to_string(),
            summary: self.redact_text(&self.summary).chars().take(120).collect(),
            description: self.redact_text(&self.description).chars().take(6000).collect(),
            diagnostics: self.redact_text(diagnostics_json).chars().take(40_000).collect(),
        };
        let endpoint = self.report_endpoint.clone();
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new().name("bug-report".into()).spawn(move || {
            let _ = sender.send(Self::upload(&endpoint, &payload));
        }) {
            Ok(_) => {
                self.submission = Some(receiver);
                self.stage = BugStage::Submitting;
            }
            Err(error) => {
                self.error_message = format!("Cannot start report upload: {error}");
                self.stage = BugStage::Failure;
            }
        }
    }

    fn upload(endpoint: &str, payload: &BugPayload) -> Result<String, String> {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build().new_agent();
        let mut response = agent.post(endpoint).send_json(payload)
            .map_err(|_| "Could not reach the report service. Check your connection and try again.".to_string())?;
        match response.status().as_u16() {
            200 | 201 => {
                let result: BugResponse = response.body_mut().with_config().limit(16_384)
                    .read_json().map_err(|_| "Report service returned an invalid response.".to_string())?;
                if !result.issue_url.starts_with("https://") {
                    return Err("Report service returned an invalid response.".to_string());
                }
                Ok(result.issue_url)
            }
            429 => Err("Too many reports. Please try again in an hour.".to_string()),
            400 | 413 | 422 => Err("The service rejected this report. Edit it and try again.".to_string()),
            code => Err(format!("Report service unavailable (HTTP {code}). Please try again later.")),
        }
    }

    pub fn poll_submission(&mut self) -> bool {
        let Some(submission) = &self.submission else { return false; };
        let result = match submission.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err("Report upload stopped. Please try again.".to_string()),
        };
        self.submission = None;
        match result {
            Ok(url) => {
                self.issue_url = url;
                self.stage = BugStage::Success;
            }
            Err(error) => {
                self.error_message = error;
                self.stage = BugStage::Failure;
            }
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
                "<S-Tab>" => {
                    self.focus = (self.focus + 3) % 4;
                    true
                }
                "<C-s>" => {
                    self.submit(diagnostics_json);
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
                    } else if self.focus == 3 && key == "<CR>" {
                        self.submit(diagnostics_json);
                    }
                    true
                }
            },
            BugStage::Submitting => {
                if key == "<Esc>" {
                    self.is_open = false;
                }
                true
            }
            BugStage::Failure => {
                match key {
                    "<CR>" => {
                        self.stage = BugStage::Form;
                        self.error_message.clear();
                    }
                    "<C-s>" => self.submit(diagnostics_json),
                    "<Esc>" | "q" => self.is_open = false,
                    _ => {}
                }
                true
            }
            BugStage::Success => {
                if matches!(key, "<Esc>" | "<CR>" | "q") {
                    self.is_open = false;
                }
                true
            }
        }
    }

    pub fn handle_mouse(&mut self, screen: Rect, mx: u16, my: u16, button: u8, pressed: bool, diagnostics_json: &str) {
        if !self.is_open || !pressed || button != 0 {
            return;
        }
        let mr = Self::modal_rect(screen);
        if !mr.contains(mx, my) {
            return;
        }
        if self.stage == BugStage::Form {
            if Rect::new(mr.x + 14, mr.y + 2, mr.w.saturating_sub(16), 1).contains(mx, my) {
                self.focus = 0;
                self.category_idx = (self.category_idx + 1) % BUG_CATEGORIES.len();
            } else if Rect::new(mr.x + 2, mr.y + 5, mr.w.saturating_sub(4), 1).contains(mx, my) {
                self.focus = 1;
            } else if Rect::new(mr.x + 2, mr.y + 8, mr.w.saturating_sub(4), mr.h.saturating_sub(14)).contains(mx, my) {
                self.focus = 2;
            } else if Rect::new(mr.x + mr.w.saturating_sub(12), mr.y + mr.h.saturating_sub(2), 10, 1).contains(mx, my) {
                self.submit(diagnostics_json);
            }
        } else if self.stage == BugStage::Failure && my == mr.y + mr.h.saturating_sub(2) {
            self.stage = BugStage::Form;
            self.error_message.clear();
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
                    mr.h.saturating_sub(14),
                );
                let desc_bg = if self.focus == 2 {
                    c.row_selected_bg
                } else {
                    c.surface_bg
                };
                renderer.fill_rect(desc_r, desc_bg);
                let lines: Vec<_> = self.description.split('\n').collect();
                let start = lines.len().saturating_sub(desc_r.h as usize);
                for (i, line) in lines[start..].iter().enumerate() {
                    let trunc = primitives::truncate_to_width(line, desc_r.w.saturating_sub(2) as usize);
                    renderer.draw_text(desc_r.x + 1, desc_r.y + i as u16, &trunc, c.text_strong, desc_bg);
                }

                for (i, line) in [
                    "Public GitHub issue, basic diagnostics",
                    "No files or logs attached.",
                    "Remove private details before sending.",
                ].iter().enumerate() {
                    renderer.draw_text(mr.x + 2, mr.y + mr.h.saturating_sub(6) + i as u16,
                        &primitives::truncate_to_width(line, mr.w.saturating_sub(4) as usize), c.text_muted, c.panel_bg);
                }
                let hint = if self.error_message.is_empty() { "Ctrl+S: Send  Esc: Close" } else { &self.error_message };
                renderer.draw_text(mr.x + 2, mr.y + mr.h.saturating_sub(3),
                    &primitives::truncate_to_width(hint, mr.w.saturating_sub(4) as usize),
                    if self.error_message.is_empty() { c.text_muted } else { theme.error }, c.panel_bg);
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
            BugStage::Submitting => {
                renderer.draw_text(
                    mr.x + 2,
                    mr.y + 5,
                    "Submitting bug report...",
                    c.accent,
                    c.panel_bg,
                );
                renderer.draw_text(mr.x + 2, mr.y + 7, "Esc hides this dialog; sending continues.", c.text_muted, c.panel_bg);
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
                for (i, line) in primitives::wrap_text(&self.error_message, mr.w.saturating_sub(4) as usize).iter().take(4).enumerate() {
                    renderer.draw_text(mr.x + 2, mr.y + 4 + i as u16, line, theme.error, c.panel_bg);
                }
                renderer.draw_text(mr.x + 2, mr.y + 10, "Your report text is still here.", c.text_muted, c.panel_bg);
                renderer.draw_text(mr.x + 2, mr.y + mr.h.saturating_sub(4), "Ctrl+S: Retry  Esc: Close", c.text_muted, c.panel_bg);
                renderer.draw_text(mr.x + 2, mr.y + mr.h.saturating_sub(2), "[ Enter: Edit Report ]", c.text_strong, c.surface_bg);
            }
        }
    }
}

#[cfg(test)]
#[path = "bug_report_tests.rs"]
mod tests;
