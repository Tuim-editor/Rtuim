use std::process::Command;

use crate::git_snapshot::{collect_snapshot, GitFileEntry, GitSnapshot};
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

pub struct GitPanel {
    pub snapshot: GitSnapshot,
    pub staged: Vec<GitFileEntry>,
    pub unstaged: Vec<GitFileEntry>,
    pub list_state: ListState,
    pub commit_msg: String,
    pub is_typing_commit: bool,
    pub status_feedback: String,
    pub open_file_requested: Option<String>,
    pub open_detailed_requested: bool,
    pub refresh_requested: bool,
}

impl GitPanel {
    pub fn new() -> Self {
        Self {
            snapshot: GitSnapshot::default(),
            staged: Vec::new(),
            unstaged: Vec::new(),
            list_state: ListState::new(),
            commit_msg: String::new(),
            is_typing_commit: false,
            status_feedback: String::new(),
            open_file_requested: None,
            open_detailed_requested: false,
            refresh_requested: false,
        }
    }

    pub fn apply_snapshot(&mut self, snap: GitSnapshot) {
        let prev_path = self.selected_entry().map(|(e, _)| e.path.clone());
        self.staged.clear();
        self.unstaged.clear();
        for f in &snap.files {
            if f.is_staged() {
                self.staged.push(f.clone());
            }
            if f.is_unstaged() {
                self.unstaged.push(f.clone());
            }
        }
        self.snapshot = snap;
        let total = self.total_items();
        if let Some(target) = prev_path {
            if let Some(new_idx) = self.find_path_index(&target) {
                self.list_state.selected = new_idx;
            }
        }
        self.list_state.clamp(total);
    }

    fn find_path_index(&self, path: &str) -> Option<usize> {
        for (i, f) in self.staged.iter().enumerate() {
            if f.path == path {
                return Some(i);
            }
        }
        for (i, f) in self.unstaged.iter().enumerate() {
            if f.path == path {
                return Some(self.staged.len() + i);
            }
        }
        None
    }

    pub fn refresh_sync(&mut self) {
        if let Some(cwd) = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
        {
            let snap = collect_snapshot(&cwd);
            self.apply_snapshot(snap);
        }
    }

    pub fn total_items(&self) -> usize {
        self.staged.len() + self.unstaged.len()
    }

    pub fn selected_entry(&self) -> Option<(&GitFileEntry, bool)> {
        let idx = self.list_state.selected;
        if idx < self.staged.len() {
            Some((&self.staged[idx], true))
        } else {
            let u_idx = idx.checked_sub(self.staged.len())?;
            self.unstaged.get(u_idx).map(|e| (e, false))
        }
    }

    fn run_git<I, S>(&mut self, args: I) -> Result<String, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        match Command::new("git").args(args).output() {
            Ok(out) => {
                if out.status.success() {
                    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
                } else {
                    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn stage_file(&mut self, path: &str) {
        match self.run_git(["add", "--", path]) {
            Ok(_) => self.status_feedback = format!("Staged {}", path),
            Err(e) => self.status_feedback = format!("Stage error: {}", e),
        }
        self.refresh_sync();
        self.refresh_requested = true;
    }

    pub fn unstage_file(&mut self, path: &str) {
        let res = if self.run_git(["rev-parse", "--verify", "HEAD"]).is_ok() {
            self.run_git(["restore", "--staged", "--", path])
                .or_else(|_| self.run_git(["reset", "HEAD", "--", path]))
        } else {
            // An unborn branch has no HEAD to restore from; keep the working file.
            self.run_git(["rm", "--cached", "--", path])
        };
        match res {
            Ok(_) => self.status_feedback = format!("Unstaged {}", path),
            Err(e) => self.status_feedback = format!("Unstage error: {}", e),
        }
        self.refresh_sync();
        self.refresh_requested = true;
    }

    pub fn toggle_stage_selected(&mut self) {
        if let Some((entry, is_staged)) = self.selected_entry() {
            let path = entry.path.clone();
            if is_staged {
                self.unstage_file(&path);
            } else {
                self.stage_file(&path);
            }
        }
    }

    pub fn stage_all(&mut self) {
        match self.run_git(["add", "-A"]) {
            Ok(_) => self.status_feedback = "Staged all changes".to_string(),
            Err(e) => self.status_feedback = format!("Stage all failed: {}", e),
        }
        self.refresh_sync();
        self.refresh_requested = true;
    }

    pub fn unstage_all(&mut self) {
        match self.run_git(["reset"]) {
            Ok(_) => self.status_feedback = "Unstaged all changes".to_string(),
            Err(e) => self.status_feedback = format!("Unstage all failed: {}", e),
        }
        self.refresh_sync();
        self.refresh_requested = true;
    }

    pub fn discard_selected(&mut self) {
        if let Some((entry, is_staged)) = self.selected_entry() {
            if is_staged {
                self.status_feedback = "Unstage before discarding".to_string();
                return;
            }
            let path = entry.path.clone();
            let is_untracked = entry.index_status == '?' && entry.worktree_status == '?';
            let res = if is_untracked {
                self.run_git(["clean", "-f", "--", &path])
            } else {
                self.run_git(["checkout", "--", &path])
            };
            match res {
                Ok(_) => self.status_feedback = format!("Discarded {}", path),
                Err(e) => self.status_feedback = format!("Discard failed: {}", e),
            }
            self.refresh_sync();
            self.refresh_requested = true;
        }
    }

    pub fn commit(&mut self) {
        let msg = self.commit_msg.trim().to_string();
        if msg.is_empty() {
            self.status_feedback = "Commit message cannot be empty".to_string();
            return;
        }
        match self.run_git(["commit", "-m", &msg]) {
            Ok(_) => {
                self.status_feedback = "Committed changes".to_string();
                self.commit_msg.clear();
                self.is_typing_commit = false;
            }
            Err(e) => {
                self.status_feedback = format!("Commit failed: {}", e);
            }
        }
        self.refresh_sync();
        self.refresh_requested = true;
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        let key = match ev {
            InputEvent::Key(k) => k.as_str(),
            InputEvent::Paste(text) => {
                if self.is_typing_commit {
                    for ch in text.chars() {
                        if ch != '\n' && ch != '\r' {
                            self.commit_msg.push(ch);
                        }
                    }
                    return true;
                }
                return false;
            }
            _ => return false,
        };

        if self.is_typing_commit {
            match key {
                "<Esc>" => {
                    self.is_typing_commit = false;
                    return true;
                }
                "<CR>" => {
                    self.commit();
                    return true;
                }
                "<BS>" => {
                    self.commit_msg.pop();
                    return true;
                }
                k if k.chars().count() == 1 && !k.starts_with('<') => {
                    self.commit_msg.push_str(k);
                    return true;
                }
                _ => return true,
            }
        }

        let total = self.total_items();
        match key {
            "j" | "<Down>" => {
                self.list_state.move_down(total, 1);
                true
            }
            "k" | "<Up>" => {
                self.list_state.move_up(1);
                true
            }
            "<Home>" | "g" => {
                self.list_state.selected = 0;
                true
            }
            "<End>" | "G" => {
                if total > 0 {
                    self.list_state.selected = total - 1;
                }
                true
            }
            " " | "s" => {
                self.toggle_stage_selected();
                true
            }
            "+" => {
                if let Some((entry, _)) = self.selected_entry() {
                    let path = entry.path.clone();
                    self.stage_file(&path);
                }
                true
            }
            "u" | "-" => {
                if let Some((entry, _)) = self.selected_entry() {
                    let path = entry.path.clone();
                    self.unstage_file(&path);
                }
                true
            }
            "a" => {
                self.stage_all();
                true
            }
            "U" => {
                self.unstage_all();
                true
            }
            "d" => {
                self.discard_selected();
                true
            }
            "c" | "i" => {
                self.is_typing_commit = true;
                true
            }
            "m" => {
                self.open_detailed_requested = true;
                true
            }
            "p" => {
                match self.run_git(["pull"]) {
                    Ok(_) => self.status_feedback = "Pulled latest changes".to_string(),
                    Err(e) => self.status_feedback = format!("Pull failed: {}", e),
                }
                self.refresh_sync();
                self.refresh_requested = true;
                true
            }
            "P" => {
                match self.run_git(["push"]) {
                    Ok(_) => self.status_feedback = "Pushed changes".to_string(),
                    Err(e) => self.status_feedback = format!("Push failed: {}", e),
                }
                self.refresh_sync();
                self.refresh_requested = true;
                true
            }
            "r" => {
                self.refresh_sync();
                self.refresh_requested = true;
                true
            }
            "<CR>" => {
                if let Some((entry, _)) = self.selected_entry() {
                    self.open_file_requested = Some(entry.path.clone());
                }
                true
            }
            _ => false,
        }
    }

    pub fn handle_mouse(
        &mut self,
        r: Rect,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
    ) -> bool {
        if !r.contains(mx, my) || !pressed {
            return false;
        }
        if button == 64 {
            self.list_state.move_up(1);
            return true;
        }
        if button == 65 {
            self.list_state.move_down(self.total_items(), 1);
            return true;
        }
        if button != 0 {
            return false;
        }

        let rel_y = my - r.y;
        let rel_x = mx - r.x;

        // Row 0: Header with "[ More ]" on the right
        if rel_y == 0 {
            if rel_x >= r.w.saturating_sub(10) {
                self.open_detailed_requested = true;
            }
            return true;
        }
        // Row 1: Commit message input box
        if rel_y == 1 {
            self.is_typing_commit = true;
            return true;
        }
        // Row 2: Commit / Stage All / More buttons
        if rel_y == 2 {
            if rel_x < 10 {
                self.commit();
            } else if rel_x < 22 {
                self.stage_all();
            } else {
                self.open_detailed_requested = true;
            }
            return true;
        }

        // Dynamic rows matching draw()
        let mut cy = r.y + 4;
        if !self.staged.is_empty() {
            if my == cy {
                return true; // "Staged Changes" header
            }
            cy += 1;
            for (idx, f) in self.staged.iter().enumerate() {
                if my == cy {
                    self.list_state.selected = idx;
                    let path = f.path.clone();
                    if rel_x >= r.w.saturating_sub(3) {
                        self.unstage_file(&path);
                    } else {
                        self.open_file_requested = Some(path);
                    }
                    return true;
                }
                cy += 1;
            }
        }

        if my == cy {
            return true; // "Changes" header
        }
        cy += 1;
        for (u_idx, f) in self.unstaged.iter().enumerate() {
            if my == cy {
                self.list_state.selected = self.staged.len() + u_idx;
                let path = f.path.clone();
                if rel_x >= r.w.saturating_sub(3) {
                    self.stage_file(&path);
                } else {
                    self.open_file_requested = Some(path);
                }
                return true;
            }
            cy += 1;
        }

        true
    }

    pub fn draw(&mut self, renderer: &mut Renderer, r: Rect, theme: &Theme, focused: bool) {
        if r.w == 0 || r.h == 0 {
            return;
        }
        let c = chrome(theme);
        renderer.fill_rect(r, c.panel_bg);

        let hdr = primitives::truncate_to_width("SOURCE CONTROL", r.w.saturating_sub(10) as usize);
        renderer.draw_text_styled(r.x + 1, r.y, &hdr, c.text_strong, c.panel_bg, true, false);
        if r.w >= 12 {
            renderer.draw_text(
                r.x + r.w.saturating_sub(9),
                r.y,
                "[ More ]",
                c.accent,
                c.panel_bg,
            );
        }

        if r.h < 4 {
            return;
        }

        // Commit message input on row 1
        primitives::draw_text_input(
            renderer,
            Rect::new(r.x + 1, r.y + 1, r.w.saturating_sub(2), 1),
            &self.commit_msg,
            "Message (Enter to commit)",
            self.is_typing_commit,
            theme,
        );

        // Action buttons on row 2
        let commit_btn = primitives::truncate_to_width("[Commit]", r.w.saturating_sub(2) as usize);
        renderer.draw_text_styled(
            r.x + 1,
            r.y + 2,
            &commit_btn,
            c.text_strong,
            c.surface_bg,
            false,
            false,
        );
        if r.w > 11 {
            let stage_btn =
                primitives::truncate_to_width("[Stage All]", r.w.saturating_sub(11) as usize);
            renderer.draw_text(r.x + 10, r.y + 2, &stage_btn, c.text, c.surface_bg);
        }
        if r.w >= 31 {
            renderer.draw_text(r.x + 22, r.y + 2, "[ More ]", c.accent, c.surface_bg);
        }

        let mut cy = r.y + 4;
        let max_y = r.y + r.h.saturating_sub(3);

        if !self.staged.is_empty() && cy < max_y {
            let hdr = primitives::truncate_to_width(
                &format!("Staged Changes ({})", self.staged.len()),
                r.w.saturating_sub(2) as usize,
            );
            renderer.draw_text_styled(r.x + 1, cy, &hdr, c.text_muted, c.panel_bg, true, false);
            cy += 1;
            for (idx, f) in self.staged.iter().enumerate() {
                if cy >= max_y {
                    break;
                }
                let selected = idx == self.list_state.selected;
                let bg = if selected {
                    if focused {
                        c.row_selected_bg
                    } else {
                        c.row_hover_bg
                    }
                } else {
                    c.panel_bg
                };
                renderer.fill_rect(Rect::new(r.x, cy, r.w, 1), bg);
                let prefix = if selected { "> " } else { "  " };
                let line = format!("{}{}", prefix, f.path);
                let truncated =
                    primitives::truncate_to_width(&line, r.w.saturating_sub(3) as usize);
                renderer.draw_text(r.x, cy, &truncated, theme.success, bg);
                renderer.draw_text(r.x + r.w.saturating_sub(2), cy, "-", c.text_muted, bg);
                cy += 1;
            }
        }

        if cy < max_y {
            let hdr = primitives::truncate_to_width(
                &format!("Changes ({})", self.unstaged.len()),
                r.w.saturating_sub(2) as usize,
            );
            renderer.draw_text_styled(r.x + 1, cy, &hdr, c.text_muted, c.panel_bg, true, false);
            cy += 1;
            for (u_idx, f) in self.unstaged.iter().enumerate() {
                if cy >= max_y {
                    break;
                }
                let global_idx = self.staged.len() + u_idx;
                let selected = global_idx == self.list_state.selected;
                let bg = if selected {
                    if focused {
                        c.row_selected_bg
                    } else {
                        c.row_hover_bg
                    }
                } else {
                    c.panel_bg
                };
                renderer.fill_rect(Rect::new(r.x, cy, r.w, 1), bg);
                let prefix = if selected { "> " } else { "  " };
                let line = format!("{}{}", prefix, f.path);
                let truncated =
                    primitives::truncate_to_width(&line, r.w.saturating_sub(3) as usize);
                renderer.draw_text(r.x, cy, &truncated, theme.warning, bg);
                renderer.draw_text(r.x + r.w.saturating_sub(2), cy, "+", c.text_muted, bg);
                cy += 1;
            }
        }

        // Draw recent commits at bottom of sidebar with ASCII "..." truncation
        let commits_y = cy + 1;
        if commits_y < r.y + r.h {
            for (i, commit) in self.snapshot.commits.iter().take(3).enumerate() {
                let ry = commits_y + i as u16;
                if ry >= r.y + r.h {
                    break;
                }
                let raw_line = format!("{} - {}", commit.hash, commit.message);
                let max_w = r.w.saturating_sub(2) as usize;
                let formatted = if raw_line.chars().count() > max_w && max_w > 3 {
                    let s: String = raw_line.chars().take(max_w - 3).collect();
                    format!("{}...", s)
                } else {
                    raw_line
                };
                renderer.draw_text(r.x + 1, ry, &formatted, c.text_muted, c.panel_bg);
            }
        }
    }
}

impl Default for GitPanel {
    fn default() -> Self {
        Self::new()
    }
}
