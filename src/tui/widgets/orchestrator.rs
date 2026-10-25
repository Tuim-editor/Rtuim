use std::process::Command;

use crate::git_snapshot::{GitSnapshot, WorktreeEntry};
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

pub struct WorktreeOrchestrator {
    pub is_open: bool,
    pub worktrees: Vec<WorktreeEntry>,
    pub list_state: ListState,
    pub is_creating: bool,
    pub branch_input: String,
    pub status_msg: String,
    pub open_agent_cmd: Option<(String, String)>,
    pub refresh_requested: bool,
}

impl WorktreeOrchestrator {
    pub fn new() -> Self {
        Self {
            is_open: false,
            worktrees: Vec::new(),
            list_state: ListState::new(),
            is_creating: false,
            branch_input: String::new(),
            status_msg: String::new(),
            open_agent_cmd: None,
            refresh_requested: false,
        }
    }

    pub fn apply_snapshot(&mut self, snap: &GitSnapshot) {
        self.worktrees = snap.worktrees.clone();
        self.list_state.clamp(self.worktrees.len());
    }

    pub fn modal_rect(screen: Rect) -> Rect {
        primitives::centered_rect(screen, 80, 22)
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        if !self.is_open {
            return false;
        }
        let InputEvent::Key(k) = ev else {
            return false;
        };

        if self.is_creating {
            match k.as_str() {
                "<Esc>" => {
                    self.is_creating = false;
                    self.branch_input.clear();
                }
                "<CR>" => {
                    let br = self.branch_input.trim().to_string();
                    self.is_creating = false;
                    self.branch_input.clear();
                    if !br.is_empty() {
                        let dir = format!("../wt-{}", br.replace('/', "-"));
                        let ok = Command::new("git")
                            .args(["worktree", "add", "-b", &br, &dir])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);
                        if ok {
                            self.status_msg = format!("Created worktree {} ({})", dir, br);
                            self.refresh_requested = true;
                        } else {
                            self.status_msg = format!("Failed to create worktree '{}'", br);
                        }
                    }
                }
                "<BS>" => {
                    self.branch_input.pop();
                }
                s if s.chars().count() == 1 && !s.starts_with('<') => {
                    self.branch_input.push_str(s);
                }
                _ => {}
            }
            return true;
        }

        match k.as_str() {
            "<Esc>" | "q" => {
                self.is_open = false;
            }
            "j" | "<Down>" => {
                self.list_state.move_down(self.worktrees.len(), 1);
            }
            "k" | "<Up>" => {
                self.list_state.move_up(1);
            }
            "n" | "c" => {
                self.is_creating = true;
                self.branch_input.clear();
            }
            "a" | "<CR>" => {
                if let Some(wt) = self.worktrees.get(self.list_state.selected) {
                    self.open_agent_cmd = Some(("codex".to_string(), wt.path.clone()));
                    self.is_open = false;
                }
            }
            _ => {}
        }
        true
    }

    pub fn handle_mouse(
        &mut self,
        screen: Rect,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
    ) -> bool {
        if !self.is_open || !pressed {
            return false;
        }
        let mr = Self::modal_rect(screen);
        if !mr.contains(mx, my) {
            if button == 0 {
                self.is_open = false;
            }
            return true;
        }
        if my >= mr.y + 4 && my < mr.y + mr.h - 2 {
            let idx = self.list_state.scroll_offset + (my - (mr.y + 4)) as usize;
            if idx < self.worktrees.len() {
                self.list_state.selected = idx;
            }
        }
        true
    }

    pub fn draw(&mut self, renderer: &mut Renderer, screen: Rect, theme: &Theme) {
        if !self.is_open {
            return;
        }
        let mr = Self::modal_rect(screen);
        let c = chrome(theme);
        primitives::draw_modal_frame(renderer, mr, "Worktree & Agent Orchestrator", theme);

        let sub = if self.is_creating {
            format!("New branch name: {}▏", self.branch_input)
        } else if !self.status_msg.is_empty() {
            self.status_msg.clone()
        } else {
            "n: New Worktree | a/Enter: Spawn AI Agent in Worktree | Esc: Close".to_string()
        };
        let trunc = primitives::truncate_to_width(&sub, mr.w.saturating_sub(4) as usize);
        renderer.draw_text(mr.x + 2, mr.y + 2, &trunc, c.text_muted, c.panel_bg);

        let list_y = mr.y + 4;
        let list_h = mr.h.saturating_sub(6) as usize;
        self.list_state.adjust_scroll(list_h);

        for i in 0..list_h {
            let idx = self.list_state.scroll_offset + i;
            if idx >= self.worktrees.len() {
                break;
            }
            let wt = &self.worktrees[idx];
            let ry = list_y + i as u16;
            let sel = idx == self.list_state.selected;
            let bg = if sel { c.row_selected_bg } else { c.panel_bg };
            renderer.fill_rect(Rect::new(mr.x + 1, ry, mr.w - 2, 1), bg);
            let line = format!(" [{}]  {}  ({})", wt.branch, wt.path, wt.head);
            let t = primitives::truncate_to_width(&line, mr.w.saturating_sub(4) as usize);
            renderer.draw_text_styled(mr.x + 2, ry, &t, c.text_strong, bg, sel, false);
        }
    }
}

impl Default for WorktreeOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}
