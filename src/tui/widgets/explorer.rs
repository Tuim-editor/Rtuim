use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::git_snapshot::GitSnapshot;
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

#[derive(Clone, Debug)]
pub struct ExplorerEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    None,
    NewFile,
    NewFolder,
    Rename,
    ConfirmDelete,
}

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ia = a.chars().peekable();
    let mut ib = b.chars().peekable();
    loop {
        match (ia.peek(), ib.peek()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&ca), Some(&cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let mut na = 0u64;
                    while let Some(&d) = ia.peek() {
                        if d.is_ascii_digit() {
                            na = na.saturating_mul(10).saturating_add((d as u8 - b'0') as u64);
                            ia.next();
                        } else {
                            break;
                        }
                    }
                    let mut nb = 0u64;
                    while let Some(&d) = ib.peek() {
                        if d.is_ascii_digit() {
                            nb = nb.saturating_mul(10).saturating_add((d as u8 - b'0') as u64);
                            ib.next();
                        } else {
                            break;
                        }
                    }
                    if na != nb {
                        return na.cmp(&nb);
                    }
                } else {
                    let la = ca.to_ascii_lowercase();
                    let lb = cb.to_ascii_lowercase();
                    if la != lb {
                        return la.cmp(&lb);
                    }
                    ia.next();
                    ib.next();
                }
            }
        }
    }
}

pub struct FileExplorer {
    pub root_dir: String,
    pub entries: Vec<ExplorerEntry>,
    pub expanded: HashSet<String>,
    pub list_state: ListState,
    pub git_badges: HashMap<String, char>,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub context_menu_open: bool,
    pub context_menu_x: u16,
    pub context_menu_y: u16,
    pub context_menu_idx: usize,
    pub open_file_requested: Option<String>,
    pub preview_file_requested: Option<String>,
    pub delete_buffer_requested: Option<String>,
    pub rename_buffer_requested: Option<(String, String)>,
}

impl FileExplorer {
    pub fn new() -> Self {
        let root_dir = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string());
        let mut exp = Self {
            root_dir,
            entries: Vec::new(),
            expanded: HashSet::new(),
            list_state: ListState::new(),
            git_badges: HashMap::new(),
            input_mode: InputMode::None,
            input_buffer: String::new(),
            context_menu_open: false,
            context_menu_x: 0,
            context_menu_y: 0,
            context_menu_idx: 0,
            open_file_requested: None,
            preview_file_requested: None,
            delete_buffer_requested: None,
            rename_buffer_requested: None,
        };
        exp.refresh();
        exp
    }

    pub fn refresh(&mut self) {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(s) = cwd.to_str() {
                self.root_dir = s.to_string();
            }
        }
        self.entries.clear();
        let root = self.root_dir.clone();
        self.scan_dir(&root, 0);
        self.list_state.clamp(self.entries.len());
    }

    fn scan_dir(&mut self, dir: &str, depth: usize) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        let mut list: Vec<(String, String, bool)> = Vec::new();
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == "target" || name == "zig-out" || name == ".zig-cache" {
                continue;
            }
            let path = entry.path().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            list.push((path, name, is_dir));
        }
        list.sort_by(|a, b| match (a.2, b.2) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => natural_cmp(&a.1, &b.1),
        });

        for (path, name, is_dir) in list {
            let expanded = is_dir && self.expanded.contains(&path);
            self.entries.push(ExplorerEntry {
                path: path.clone(),
                name,
                is_dir,
                depth,
                expanded,
            });
            if expanded {
                self.scan_dir(&path, depth + 1);
            }
        }
    }

    pub fn update_git_status(&mut self, snap: &GitSnapshot) {
        self.git_badges.clear();
        for f in &snap.files {
            let ch = f.status_badge(f.is_staged());
            self.git_badges.insert(f.path.clone(), ch);
        }
    }

    pub fn selected_entry(&self) -> Option<&ExplorerEntry> {
        self.entries.get(self.list_state.selected)
    }

    fn target_parent_dir(&self) -> PathBuf {
        if let Some(sel) = self.selected_entry() {
            let p = Path::new(&sel.path);
            if sel.is_dir {
                return p.to_path_buf();
            }
            if let Some(parent) = p.parent() {
                return parent.to_path_buf();
            }
        }
        PathBuf::from(&self.root_dir)
    }

    fn execute_input_action(&mut self) {
        let name = self.input_buffer.trim().to_string();
        let mode = self.input_mode;
        self.input_mode = InputMode::None;
        self.input_buffer.clear();
        if name.is_empty() {
            return;
        }
        match mode {
            InputMode::NewFile => {
                let target = self.target_parent_dir().join(&name);
                if let Some(p) = target.parent() {
                    let _ = fs::create_dir_all(p);
                }
                if fs::write(&target, "").is_ok() {
                    let path_str = target.to_string_lossy().to_string();
                    self.refresh();
                    self.open_file_requested = Some(path_str);
                }
            }
            InputMode::NewFolder => {
                let target = self.target_parent_dir().join(&name);
                let _ = fs::create_dir_all(&target);
                self.refresh();
            }
            InputMode::Rename => {
                if let Some(sel) = self.selected_entry().cloned() {
                    let old_p = Path::new(&sel.path);
                    if let Some(parent) = old_p.parent() {
                        let new_p = parent.join(&name);
                        if fs::rename(old_p, &new_p).is_ok() {
                            self.rename_buffer_requested =
                                Some((sel.path, new_p.to_string_lossy().to_string()));
                            self.refresh();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        if self.context_menu_open {
            if let InputEvent::Key(k) = ev {
                match k.as_str() {
                    "<Esc>" => self.context_menu_open = false,
                    "<Up>" | "k" => {
                        self.context_menu_idx = self.context_menu_idx.saturating_sub(1);
                    }
                    "<Down>" | "j" => {
                        self.context_menu_idx = (self.context_menu_idx + 1).min(3);
                    }
                    "<CR>" => {
                        self.context_menu_open = false;
                        self.trigger_context_action(self.context_menu_idx);
                    }
                    _ => {}
                }
                return true;
            }
        }

        if self.input_mode == InputMode::ConfirmDelete {
            if let InputEvent::Key(k) = ev {
                match k.as_str() {
                    "y" | "Y" | "<CR>" => {
                        if let Some(sel) = self.selected_entry().cloned() {
                            if sel.is_dir {
                                let _ = fs::remove_dir_all(&sel.path);
                            } else {
                                let _ = fs::remove_file(&sel.path);
                            }
                            self.delete_buffer_requested = Some(sel.path);
                            self.refresh();
                        }
                        self.input_mode = InputMode::None;
                    }
                    _ => {
                        self.input_mode = InputMode::None;
                    }
                }
                return true;
            }
        }

        if self.input_mode != InputMode::None {
            match ev {
                InputEvent::Paste(text) => {
                    for ch in text.chars() {
                        if ch != '\n' && ch != '\r' {
                            self.input_buffer.push(ch);
                        }
                    }
                    return true;
                }
                InputEvent::Key(k) => {
                    match k.as_str() {
                        "<Esc>" => {
                            self.input_mode = InputMode::None;
                            self.input_buffer.clear();
                        }
                        "<CR>" => self.execute_input_action(),
                        "<BS>" => {
                            self.input_buffer.pop();
                        }
                        s if s.chars().count() == 1 && !s.starts_with('<') => {
                            self.input_buffer.push_str(s);
                        }
                        _ => {}
                    }
                    return true;
                }
                _ => return false,
            }
        }

        let InputEvent::Key(k) = ev else {
            return false;
        };

        match k.as_str() {
            "j" | "<Down>" => {
                self.list_state.move_down(self.entries.len(), 1);
                if let Some(sel) = self.selected_entry() {
                    if !sel.is_dir {
                        self.preview_file_requested = Some(sel.path.clone());
                    }
                }
                true
            }
            "k" | "<Up>" => {
                self.list_state.move_up(1);
                if let Some(sel) = self.selected_entry() {
                    if !sel.is_dir {
                        self.preview_file_requested = Some(sel.path.clone());
                    }
                }
                true
            }
            "<Home>" | "g" => {
                self.list_state.move_home();
                true
            }
            "<End>" | "G" => {
                self.list_state.move_end(self.entries.len());
                true
            }
            "<CR>" | "o" | "l" | "<Right>" => {
                if let Some(sel) = self.selected_entry().cloned() {
                    if sel.is_dir {
                        if self.expanded.contains(&sel.path) {
                            self.expanded.remove(&sel.path);
                        } else {
                            self.expanded.insert(sel.path);
                        }
                        self.refresh();
                    } else {
                        self.open_file_requested = Some(sel.path);
                    }
                }
                true
            }
            "h" | "<Left>" => {
                if let Some(sel) = self.selected_entry().cloned() {
                    if sel.is_dir && self.expanded.contains(&sel.path) {
                        self.expanded.remove(&sel.path);
                        self.refresh();
                    }
                }
                true
            }
            "a" | "n" | "c" => {
                self.input_mode = InputMode::NewFile;
                self.input_buffer.clear();
                true
            }
            "A" | "N" => {
                self.input_mode = InputMode::NewFolder;
                self.input_buffer.clear();
                true
            }
            "r" => {
                if let Some(sel) = self.selected_entry().cloned() {
                    self.input_mode = InputMode::Rename;
                    self.input_buffer = sel.name;
                }
                true
            }
            "d" | "x" | "<Del>" => {
                if self.selected_entry().is_some() {
                    self.input_mode = InputMode::ConfirmDelete;
                }
                true
            }
            "R" => {
                self.refresh();
                true
            }
            _ => false,
        }
    }

    fn trigger_context_action(&mut self, idx: usize) {
        match idx {
            0 => {
                self.input_mode = InputMode::NewFile;
                self.input_buffer.clear();
            }
            1 => {
                self.input_mode = InputMode::NewFolder;
                self.input_buffer.clear();
            }
            2 => {
                if let Some(sel) = self.selected_entry().cloned() {
                    self.input_mode = InputMode::Rename;
                    self.input_buffer = sel.name;
                }
            }
            3 => {
                if self.selected_entry().is_some() {
                    self.input_mode = InputMode::ConfirmDelete;
                }
            }
            _ => {}
        }
    }

    pub fn handle_mouse(
        &mut self,
        rect: Rect,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
    ) -> bool {
        if !pressed {
            return false;
        }
        if button == 64 {
            self.list_state.move_up(1);
            return true;
        }
        if button == 65 {
            self.list_state.move_down(self.entries.len(), 1);
            return true;
        }

        if self.context_menu_open {
            let menu_r = Rect::new(self.context_menu_x, self.context_menu_y, 16, 6);
            if menu_r.contains(mx, my) && my > menu_r.y && my < menu_r.y + 5 {
                let act = (my - menu_r.y - 1) as usize;
                self.context_menu_open = false;
                self.trigger_context_action(act);
                return true;
            }
            self.context_menu_open = false;
            return true;
        }

        if !rect.contains(mx, my) {
            return false;
        }

        let rel_y = my - rect.y;
        let rel_x = mx - rect.x;
        if rel_y == 0 {
            if rel_x >= rect.w.saturating_sub(9) && rel_x < rect.w.saturating_sub(6) {
                self.input_mode = InputMode::NewFile;
                self.input_buffer.clear();
            } else if rel_x >= rect.w.saturating_sub(6) && rel_x < rect.w.saturating_sub(3) {
                self.input_mode = InputMode::NewFolder;
                self.input_buffer.clear();
            } else if rel_x >= rect.w.saturating_sub(3) {
                self.refresh();
            }
            return true;
        }

        let list_start_y = if self.input_mode != InputMode::None {
            rect.y + 2
        } else {
            rect.y + 1
        };

        if my >= list_start_y {
            let idx = self.list_state.scroll_offset + (my - list_start_y) as usize;
            if idx < self.entries.len() {
                self.list_state.selected = idx;
                if button == 2 {
                    self.context_menu_open = true;
                    self.context_menu_x = mx;
                    self.context_menu_y = my;
                    self.context_menu_idx = 0;
                } else if button == 0 {
                    let sel = self.entries[idx].clone();
                    if sel.is_dir {
                        if self.expanded.contains(&sel.path) {
                            self.expanded.remove(&sel.path);
                        } else {
                            self.expanded.insert(sel.path);
                        }
                        self.refresh();
                    } else {
                        self.open_file_requested = Some(sel.path);
                    }
                }
            }
        }
        true
    }

    pub fn draw(&mut self, renderer: &mut Renderer, rect: Rect, theme: &Theme, focused: bool) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let c = chrome(theme);
        renderer.fill_rect(rect, c.panel_bg);

        // Header: EXPLORER  +F +D ↻
        let hdr_max = if rect.w >= 16 {
            rect.w.saturating_sub(10) as usize
        } else {
            rect.w.saturating_sub(2) as usize
        };
        let hdr = primitives::truncate_to_width("EXPLORER", hdr_max);
        renderer.draw_text_styled(
            rect.x + 1,
            rect.y,
            &hdr,
            c.text_muted,
            c.panel_bg,
            true,
            false,
        );
        if rect.w >= 16 {
            renderer.draw_text(
                rect.x + rect.w.saturating_sub(9),
                rect.y,
                "+F +D ↻",
                c.text_muted,
                c.panel_bg,
            );
        }

        let mut list_y = rect.y + 1;
        if self.input_mode != InputMode::None && list_y < rect.y + rect.h {
            let prompt = match self.input_mode {
                InputMode::NewFile => format!("New file: {}▏", self.input_buffer),
                InputMode::NewFolder => format!("New dir: {}▏", self.input_buffer),
                InputMode::Rename => format!("Rename: {}▏", self.input_buffer),
                InputMode::ConfirmDelete => "Delete selected? [y/N]".to_string(),
                InputMode::None => String::new(),
            };
            renderer.fill_rect(Rect::new(rect.x, list_y, rect.w, 1), c.elevated_bg);
            let trunc = primitives::truncate_to_width(&prompt, rect.w.saturating_sub(2) as usize);
            renderer.draw_text_styled(
                rect.x + 1,
                list_y,
                &trunc,
                c.accent,
                c.elevated_bg,
                true,
                false,
            );
            list_y += 1;
        }

        let view_h = (rect.y + rect.h).saturating_sub(list_y) as usize;
        self.list_state.adjust_scroll(view_h);

        for i in 0..view_h {
            let idx = self.list_state.scroll_offset + i;
            if idx >= self.entries.len() {
                break;
            }
            let item = &self.entries[idx];
            let ry = list_y + i as u16;
            let selected = idx == self.list_state.selected;
            let bg = if selected && focused {
                c.row_selected_bg
            } else {
                c.panel_bg
            };
            renderer.fill_rect(Rect::new(rect.x, ry, rect.w, 1), bg);

            let indent = "  ".repeat(item.depth.min(6));
            let prefix = if item.is_dir {
                if item.expanded {
                    "▾ "
                } else {
                    "▸ "
                }
            } else {
                "  "
            };
            let line = format!(" {}{}{}", indent, prefix, item.name);
            let trunc = primitives::truncate_to_width(&line, rect.w.saturating_sub(3) as usize);
            let fg = if item.is_dir {
                c.accent
            } else if selected && focused {
                c.text_strong
            } else {
                c.text
            };
            renderer.draw_text_styled(rect.x, ry, &trunc, fg, bg, item.is_dir || selected, false);

            // Git badge on right edge
            let rel_path = item
                .path
                .strip_prefix(&self.root_dir)
                .map(|s| s.trim_start_matches('/'))
                .unwrap_or(&item.path);
            if let Some(&badge) = self.git_badges.get(rel_path) {
                let b_fg = match badge {
                    'M' => c.warning,
                    'A' => c.success,
                    'D' => c.error,
                    _ => c.text_muted,
                };
                renderer.draw_text_styled(
                    rect.x + rect.w.saturating_sub(2),
                    ry,
                    &badge.to_string(),
                    b_fg,
                    bg,
                    true,
                    false,
                );
            }
        }
    }

    pub fn draw_overlay(&self, renderer: &mut Renderer, _sidebar_rect: Rect, theme: &Theme) {
        if !self.context_menu_open {
            return;
        }
        let c = chrome(theme);
        let r = Rect::new(self.context_menu_x, self.context_menu_y, 16, 6);
        primitives::draw_modal_frame(renderer, r, "", theme);
        let items = ["New File", "New Folder", "Rename", "Delete"];
        for (i, label) in items.iter().enumerate() {
            let ry = r.y + 1 + i as u16;
            let sel = i == self.context_menu_idx;
            let bg = if sel { c.row_selected_bg } else { c.panel_bg };
            renderer.fill_rect(Rect::new(r.x + 1, ry, r.w - 2, 1), bg);
            renderer.draw_text_styled(r.x + 2, ry, label, c.text_strong, bg, sel, false);
        }
    }
}

impl Default for FileExplorer {
    fn default() -> Self {
        Self::new()
    }
}
