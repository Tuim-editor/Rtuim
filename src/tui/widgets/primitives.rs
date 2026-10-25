use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};

#[derive(Clone, Copy, Debug, Default)]
pub struct ListState {
    pub selected: usize,
    pub scroll_offset: usize,
}

impl ListState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
        }
    }

    pub fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
            self.scroll_offset = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    pub fn move_up(&mut self, step: usize) {
        self.selected = self.selected.saturating_sub(step);
    }

    pub fn move_down(&mut self, len: usize, step: usize) {
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected + step).min(len - 1);
        }
    }

    pub fn move_home(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn move_end(&mut self, len: usize) {
        if len > 0 {
            self.selected = len - 1;
        }
    }

    pub fn adjust_scroll(&mut self, viewport_h: usize) {
        if viewport_h == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + viewport_h {
            self.scroll_offset = self.selected + 1 - viewport_h;
        }
    }
}

pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if text.width() <= max_width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in text.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_width {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

pub fn pad_to_width(text: &str, width: usize) -> String {
    let mut s = truncate_to_width(text, width);
    let cur = s.width();
    if cur < width {
        s.push_str(&" ".repeat(width - cur));
    }
    s
}

pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 { return Vec::new(); }
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.width() + 1 + word.width() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() { line.push(' '); }
        line.push_str(&truncate_to_width(word, width));
    }
    if !line.is_empty() { lines.push(line); }
    lines
}

pub fn centered_rect(screen: Rect, max_w: u16, max_h: u16) -> Rect {
    let w = max_w.min(screen.w.saturating_sub(2)).max(10.min(screen.w));
    let h = max_h.min(screen.h.saturating_sub(2)).max(6.min(screen.h));
    let x = screen.x + (screen.w.saturating_sub(w)) / 2;
    let y = screen.y + (screen.h.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

pub fn draw_rounded_border(
    renderer: &mut Renderer,
    rect: Rect,
    title: Option<&str>,
    fg: u32,
    bg: u32,
) {
    if rect.w < 2 || rect.h < 2 {
        return;
    }
    let x2 = rect.x + rect.w - 1;
    let y2 = rect.y + rect.h - 1;
    renderer.draw_hline(rect.x + 1, rect.y, rect.w - 2, "─", fg, bg);
    renderer.draw_hline(rect.x + 1, y2, rect.w - 2, "─", fg, bg);
    renderer.draw_vline(rect.x, rect.y + 1, rect.h - 2, "│", fg, bg);
    renderer.draw_vline(x2, rect.y + 1, rect.h - 2, "│", fg, bg);
    renderer.draw_text(rect.x, rect.y, "╭", fg, bg);
    renderer.draw_text(x2, rect.y, "╮", fg, bg);
    renderer.draw_text(rect.x, y2, "╰", fg, bg);
    renderer.draw_text(x2, y2, "╯", fg, bg);

    if let Some(t) = title {
        if !t.is_empty() && rect.w > 4 {
            let max_inner = (rect.w - 2) as usize;
            let label = format!(" {} ", t);
            if label.width() <= max_inner.saturating_sub(2) {
                renderer.draw_text_styled(rect.x + 2, rect.y, &label, fg, bg, true, false);
            } else {
                let trunc = truncate_to_width(t, max_inner);
                renderer.draw_text_styled(rect.x + 1, rect.y, &trunc, fg, bg, true, false);
            }
        }
    }
}

pub fn draw_modal_frame(renderer: &mut Renderer, rect: Rect, title: &str, theme: &Theme) {
    let c = chrome(theme);
    renderer.fill_rect(rect, c.panel_bg);
    draw_rounded_border(renderer, rect, Some(title), c.border, c.panel_bg);
}

/// Shared geometry for settings and management dialogs. Drawing and mouse
/// handling use the same rectangles, including after terminal resizing.
#[derive(Clone, Copy, Debug)]
pub struct ManagerLayout {
    pub outer: Rect,
    pub search: Rect,
    pub nav: Rect,
    pub content: Rect,
    pub footer: Rect,
}

impl ManagerLayout {
    pub fn new(screen: Rect) -> Self {
        let mx = if screen.w >= 100 { screen.w / 16 } else { u16::from(screen.w > 2) };
        let my = if screen.h >= 24 { 2 } else { u16::from(screen.h > 2) };
        let outer = Rect::new(screen.x + mx, screen.y + my,
            screen.w.saturating_sub(mx * 2), screen.h.saturating_sub(my * 2));
        let nav_w = if outer.w >= 74 { 24 } else if outer.w >= 52 { 18 } else { 0 };
        let body_h = outer.h.saturating_sub(6);
        let nav = Rect::new(outer.x + 2, outer.y + 3, nav_w, body_h);
        let content_x = outer.x + 2 + if nav_w > 0 { nav_w + 2 } else { 0 };
        Self {
            outer,
            search: Rect::new(outer.x + 2, outer.y + 1, outer.w.saturating_sub(4), 1),
            nav,
            content: Rect::new(content_x, nav.y,
                (outer.x + outer.w).saturating_sub(content_x + 2), body_h),
            footer: Rect::new(outer.x + 2, outer.y + outer.h.saturating_sub(2), outer.w.saturating_sub(4), 1),
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, title: &str, theme: &Theme) {
        let c = chrome(theme);
        renderer.dim(theme.bg_editor);
        draw_modal_frame(renderer, self.outer, title, theme);
        if self.outer.h < 7 || self.outer.w < 8 { return; }
        renderer.draw_hline(self.outer.x + 1, self.outer.y + 2,
            self.outer.w.saturating_sub(2), "─", c.border, c.panel_bg);
        renderer.draw_hline(self.outer.x + 1, self.footer.y - 1,
            self.outer.w.saturating_sub(2), "─", c.border, c.panel_bg);
        if self.nav.w > 0 {
            renderer.draw_vline(self.nav.x + self.nav.w, self.nav.y,
                self.nav.h, "│", c.border, c.panel_bg);
        }
    }

    pub fn action_rect(&self, labels: &[&str], index: usize) -> Rect {
        let total: u16 = labels.iter().map(|s| s.width() as u16 + 2).sum();
        let preceding: u16 = labels.iter().take(index).map(|s| s.width() as u16 + 2).sum();
        Rect::new(self.footer.x + self.footer.w.saturating_sub(total) + preceding,
            self.footer.y, labels[index].width() as u16, 1)
    }

    pub fn draw_footer(&self, renderer: &mut Renderer, hint: &str, actions: &[&str], theme: &Theme) {
        let c = chrome(theme);
        let reserve: u16 = actions.iter().map(|s| s.width() as u16 + 2).sum();
        text_in(renderer, Rect::new(self.footer.x, self.footer.y,
            self.footer.w.saturating_sub(reserve + 1), 1), hint, c.text_muted, c.panel_bg, false);
        for (i, label) in actions.iter().enumerate() {
            let rect = self.action_rect(actions, i);
            text_in(renderer, Rect::new(rect.x, rect.y,
                rect.w.min((self.footer.x + self.footer.w).saturating_sub(rect.x)), 1),
                label, c.text_strong, c.surface_bg, false);
        }
    }
}

pub fn text_in(renderer: &mut Renderer, rect: Rect, text: &str, fg: u32, bg: u32, bold: bool) {
    if rect.w == 0 || rect.h == 0 { return; }
    let text = if text.width() > rect.w as usize {
        format!("{}…", truncate_to_width(text, rect.w.saturating_sub(1) as usize))
    } else {
        text.to_string()
    };
    renderer.draw_text_styled(rect.x, rect.y, &text, fg, bg, bold, false);
}

pub fn draw_button(
    renderer: &mut Renderer,
    x: u16,
    y: u16,
    label: &str,
    active: bool,
    hover: bool,
    theme: &Theme,
) -> u16 {
    let c = chrome(theme);
    let bg = if active {
        c.accent
    } else if hover {
        c.row_hover_bg
    } else {
        c.elevated_bg
    };
    let fg = if active { c.panel_bg } else { c.text_strong };
    let text = format!(" {} ", label);
    renderer.draw_text_styled(x, y, &text, fg, bg, active || hover, false);
    text.width() as u16
}

pub fn draw_text_input(
    renderer: &mut Renderer,
    rect: Rect,
    value: &str,
    placeholder: &str,
    focused: bool,
    theme: &Theme,
) {
    if rect.w == 0 || rect.h == 0 {
        return;
    }
    let c = chrome(theme);
    let bg = if focused { c.elevated_bg } else { c.surface_bg };
    renderer.fill_rect(rect, bg);
    if value.is_empty() {
        let cursor_width = u16::from(focused);
        if focused {
            renderer.draw_text(rect.x, rect.y, "▏", c.text_strong, bg);
        }
        let trunc = truncate_to_width(placeholder, rect.w.saturating_sub(cursor_width) as usize);
        renderer.draw_text(rect.x + cursor_width, rect.y, &trunc, c.text_muted, bg);
    } else {
        let display = if focused {
            format!("{}▏", value)
        } else {
            value.to_string()
        };
        let trunc = truncate_to_width(&display, rect.w as usize);
        renderer.draw_text_styled(rect.x, rect.y, &trunc, c.text_strong, bg, focused, false);
    }
}
