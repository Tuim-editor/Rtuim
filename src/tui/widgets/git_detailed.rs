use std::process::Command;

use crate::git_snapshot::GitSnapshot;
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

pub struct GitDetailedWidget {
    pub is_open: bool,
    pub active_tab: usize, // 0: History, 1: Branches
    pub snapshot: GitSnapshot,
    pub list_state: ListState,
    pub status_msg: String,
    pub refresh_requested: bool,
}

impl GitDetailedWidget {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: 0,
            snapshot: GitSnapshot::default(),
            list_state: ListState::new(),
            status_msg: String::new(),
            refresh_requested: false,
        }
    }

    pub fn apply_snapshot(&mut self, snap: GitSnapshot) {
        self.snapshot = snap;
        self.list_state.clamp(self.current_len());
    }

    pub fn current_len(&self) -> usize {
        if self.active_tab == 0 {
            self.snapshot.commits.len()
        } else {
            self.snapshot.branches.len()
        }
    }

    pub fn modal_rect(screen: Rect) -> Rect {
        primitives::centered_rect(screen, 88, 26)
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        if !self.is_open {
            return false;
        }
        let InputEvent::Key(k) = ev else {
            return false;
        };
        let len = self.current_len();
        match k.as_str() {
            "<Esc>" | "q" => {
                self.is_open = false;
            }
            "1" => {
                self.active_tab = 0;
                self.list_state.move_home();
            }
            "2" => {
                self.active_tab = 1;
                self.list_state.move_home();
            }
            "<Tab>" => {
                self.active_tab = (self.active_tab + 1) % 2;
                self.list_state.move_home();
            }
            "j" | "<Down>" => {
                self.list_state.move_down(len, 1);
            }
            "k" | "<Up>" => {
                self.list_state.move_up(1);
            }
            "c" | "<CR>" => {
                if self.active_tab == 0 {
                    if let Some(cm) = self.snapshot.commits.get(self.list_state.selected) {
                        let _ = Command::new("git").args(["checkout", &cm.hash]).output();
                        self.refresh_requested = true;
                    }
                } else if let Some(br) = self.snapshot.branches.get(self.list_state.selected) {
                    let _ = Command::new("git").args(["checkout", br]).output();
                    self.refresh_requested = true;
                }
            }
            "r" => {
                if self.active_tab == 0 {
                    if let Some(cm) = self.snapshot.commits.get(self.list_state.selected) {
                        let _ = Command::new("git")
                            .args(["reset", "--hard", &cm.hash])
                            .output();
                        self.refresh_requested = true;
                    }
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
        if my == mr.y + 2 {
            let rel_x = mx - mr.x;
            if rel_x < 16 {
                self.active_tab = 0;
                self.list_state.move_home();
            } else if rel_x < 32 {
                self.active_tab = 1;
                self.list_state.move_home();
            }
            return true;
        }
        if my >= mr.y + 4 && my < mr.y + mr.h - 2 {
            let idx = self.list_state.scroll_offset + (my - (mr.y + 4)) as usize;
            if idx < self.current_len() {
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
        primitives::draw_modal_frame(renderer, mr, "Git History & Branches", theme);

        let t0_bg = if self.active_tab == 0 {
            c.row_selected_bg
        } else {
            c.surface_bg
        };
        let t1_bg = if self.active_tab == 1 {
            c.row_selected_bg
        } else {
            c.surface_bg
        };
        renderer.draw_text_styled(
            mr.x + 2,
            mr.y + 2,
            " 1 History ",
            c.text_strong,
            t0_bg,
            self.active_tab == 0,
            false,
        );
        renderer.draw_text_styled(
            mr.x + 15,
            mr.y + 2,
            " 2 Branches ",
            c.text_strong,
            t1_bg,
            self.active_tab == 1,
            false,
        );

        let list_y = mr.y + 4;
        let list_h = mr.h.saturating_sub(6) as usize;
        self.list_state.adjust_scroll(list_h);

        for i in 0..list_h {
            let idx = self.list_state.scroll_offset + i;
            if idx >= self.current_len() {
                break;
            }
            let ry = list_y + i as u16;
            let sel = idx == self.list_state.selected;
            let bg = if sel { c.row_selected_bg } else { c.panel_bg };
            renderer.fill_rect(Rect::new(mr.x + 1, ry, mr.w - 2, 1), bg);

            let line = if self.active_tab == 0 {
                let cm = &self.snapshot.commits[idx];
                format!(" {}  {} ({}, {})", cm.hash, cm.message, cm.author, cm.date)
            } else {
                let br = &self.snapshot.branches[idx];
                let mark = if *br == self.snapshot.branch { "*" } else { " " };
                format!(" {} {}", mark, br)
            };
            let trunc = primitives::truncate_to_width(&line, mr.w.saturating_sub(4) as usize);
            renderer.draw_text_styled(mr.x + 2, ry, &trunc, c.text_strong, bg, sel, false);
        }
    }
}

impl Default for GitDetailedWidget {
    fn default() -> Self {
        Self::new()
    }
}
