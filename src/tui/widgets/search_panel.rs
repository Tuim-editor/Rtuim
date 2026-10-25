use std::process::Command;

use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub snippet: String,
}

pub struct SearchPanel {
    pub query: String,
    pub focus_field: usize, // 0: list, 1: query input
    pub hits: Vec<SearchHit>,
    pub list_state: ListState,
    pub open_hit_requested: Option<(String, usize, usize)>,
}

impl SearchPanel {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            focus_field: 1,
            hits: Vec::new(),
            list_state: ListState::new(),
            open_hit_requested: None,
        }
    }

    pub fn run_search(&mut self) {
        self.hits.clear();
        let q = self.query.trim();
        if q.is_empty() {
            return;
        }
        let out = Command::new("rg")
            .args([
                "--vimgrep",
                "--no-heading",
                "--color=never",
                "--max-count=100",
                q,
            ])
            .output()
            .or_else(|_| Command::new("grep").args(["-rnI", q, "."]).output());

        if let Ok(o) = out {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines().take(200) {
                let mut parts = line.splitn(4, ':');
                if let (Some(path), Some(l_str), Some(c_or_s)) =
                    (parts.next(), parts.next(), parts.next())
                {
                    let line_no = l_str.parse::<usize>().unwrap_or(1);
                    if let Some(rest) = parts.next() {
                        let col_no = c_or_s.parse::<usize>().unwrap_or(1);
                        self.hits.push(SearchHit {
                            path: path.to_string(),
                            line: line_no,
                            col: col_no,
                            snippet: rest.trim().to_string(),
                        });
                    } else {
                        self.hits.push(SearchHit {
                            path: path.to_string(),
                            line: line_no,
                            col: 1,
                            snippet: c_or_s.trim().to_string(),
                        });
                    }
                }
            }
        }
        self.list_state.clamp(self.hits.len());
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        match ev {
            InputEvent::Paste(text) if self.focus_field == 1 => {
                for ch in text.chars() {
                    if ch != '\n' && ch != '\r' {
                        self.query.push(ch);
                    }
                }
                self.run_search();
                true
            }
            InputEvent::Key(k) => {
                if self.focus_field == 1 {
                    match k.as_str() {
                        "<Esc>" | "<Down>" => {
                            self.focus_field = 0;
                        }
                        "<CR>" => {
                            self.run_search();
                            self.focus_field = 0;
                        }
                        "<BS>" => {
                            self.query.pop();
                            self.run_search();
                        }
                        s if s.chars().count() == 1 && !s.starts_with('<') => {
                            self.query.push_str(s);
                            self.run_search();
                        }
                        _ => {}
                    }
                    return true;
                }
                match k.as_str() {
                    "/" | "i" => {
                        self.focus_field = 1;
                    }
                    "j" | "<Down>" => {
                        self.list_state.move_down(self.hits.len(), 1);
                    }
                    "k" | "<Up>" => {
                        self.list_state.move_up(1);
                    }
                    "<CR>" => {
                        if let Some(hit) = self.hits.get(self.list_state.selected) {
                            self.open_hit_requested =
                                Some((hit.path.clone(), hit.line, hit.col));
                        }
                    }
                    _ => {}
                }
                true
            }
            _ => false,
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
        if !pressed || !rect.contains(mx, my) {
            return false;
        }
        if my == rect.y + 1 {
            self.focus_field = 1;
            return true;
        }
        if my >= rect.y + 3 && button == 0 {
            let idx = self.list_state.scroll_offset + (my - (rect.y + 3)) as usize;
            if idx < self.hits.len() {
                self.list_state.selected = idx;
                let hit = &self.hits[idx];
                self.open_hit_requested = Some((hit.path.clone(), hit.line, hit.col));
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
        renderer.draw_text_styled(
            rect.x + 1,
            rect.y,
            "SEARCH",
            c.text_muted,
            c.panel_bg,
            true,
            false,
        );

        let input_r = Rect::new(rect.x + 1, rect.y + 1, rect.w.saturating_sub(2), 1);
        primitives::draw_text_input(
            renderer,
            input_r,
            &self.query,
            "Search project...",
            focused && self.focus_field == 1,
            theme,
        );

        let list_y = rect.y + 3;
        let view_h = (rect.y + rect.h).saturating_sub(list_y) as usize;
        self.list_state.adjust_scroll(view_h);

        for i in 0..view_h {
            let idx = self.list_state.scroll_offset + i;
            if idx >= self.hits.len() {
                break;
            }
            let hit = &self.hits[idx];
            let ry = list_y + i as u16;
            let sel = idx == self.list_state.selected;
            let bg = if sel && focused {
                c.row_selected_bg
            } else {
                c.panel_bg
            };
            renderer.fill_rect(Rect::new(rect.x, ry, rect.w, 1), bg);
            let label = format!(" {}:{} {}", hit.path, hit.line, hit.snippet);
            let trunc = primitives::truncate_to_width(&label, rect.w.saturating_sub(1) as usize);
            renderer.draw_text_styled(rect.x, ry, &trunc, c.text, bg, sel && focused, false);
        }
    }
}

impl Default for SearchPanel {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ConsolePanel {
    pub title: String,
    pub lines: Vec<String>,
    pub list_state: ListState,
}

impl ConsolePanel {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            lines: Vec::new(),
            list_state: ListState::new(),
        }
    }

    pub fn push_line(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > 500 {
            self.lines.remove(0);
        }
        self.list_state.selected = self.lines.len().saturating_sub(1);
    }

    pub fn draw(&mut self, renderer: &mut Renderer, rect: Rect, theme: &Theme) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let c = chrome(theme);
        renderer.fill_rect(rect, theme.bg_terminal);
        if self.lines.is_empty() {
            let msg = format!("{} ready.", self.title);
            renderer.draw_text(rect.x + 1, rect.y, &msg, c.text_muted, theme.bg_terminal);
            return;
        }
        let view_h = rect.h as usize;
        self.list_state.adjust_scroll(view_h);
        for i in 0..view_h {
            let idx = self.list_state.scroll_offset + i;
            if idx >= self.lines.len() {
                break;
            }
            let ry = rect.y + i as u16;
            let trunc =
                primitives::truncate_to_width(&self.lines[idx], rect.w.saturating_sub(2) as usize);
            renderer.draw_text(rect.x + 1, ry, &trunc, c.text, theme.bg_terminal);
        }
    }
}
