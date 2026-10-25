use crate::metrics::{ScopedTimer, GLOBAL};
use crate::tui::layout::Rect;
use std::io::Write;
use std::sync::atomic::Ordering;
use unicode_width::UnicodeWidthChar;
use crate::tui::theme::{blend_u32, readable_u32};

#[cfg(test)]
mod contrast_tests {
    use super::*;
    use crate::tui::theme::contrast_ratio_u32;

    #[test]
    fn native_labels_remain_readable_on_light_and_dark_selections() {
        let mut renderer = Renderer::new(40, 1);
        for background in [0x88aadd, 0xfabd2f, 0xffffff, 0x094771, 0x202020] {
            for foreground in [0xd4d4d4, 0xffffff, 0x969696, 0x3794ff, 0x202020] {
                renderer.draw_text(0, 0, "Selected plugin", foreground, background);
                let cell = renderer.buf[0];
                assert_eq!(cell.bg, background);
                assert!(contrast_ratio_u32(cell.fg, cell.bg) >= 4.5);
                renderer.apply_hover(0, 0, 0x303030);
                assert!(contrast_ratio_u32(renderer.buf[0].fg, renderer.buf[0].bg) >= 4.5);
                assert_eq!(renderer.buf[0].ch[0], b'S');
            }
        }
        renderer.put_cell(0, 0, "N", 0x888888, 0xaaaaaa, false, false, false);
        assert_eq!(renderer.buf[0].fg, 0x888888, "Neovim highlights must remain unchanged");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: [u8; 12],
    pub len: u8,
    pub continuation: bool,
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for Cell {
    fn default() -> Self {
        let mut ch = [0u8; 12];
        ch[0] = b' ';
        Self {
            ch,
            len: 1,
            continuation: false,
            fg: 0xd4d4d4,
            bg: 0x1e1e1e,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

impl Cell {
    pub fn set_char(&mut self, slice: &str) {
        let bytes = slice.as_bytes();
        let copy_len = bytes.len().min(self.ch.len());
        self.ch = [0u8; 12];
        if copy_len == 0 {
            self.ch[0] = b' ';
            self.len = 1;
        } else {
            self.ch[..copy_len].copy_from_slice(&bytes[..copy_len]);
            self.len = copy_len as u8;
        }
        self.continuation = false;
    }

    pub fn append_bytes(&mut self, slice: &[u8]) {
        let cur = self.len as usize;
        let available = self.ch.len().saturating_sub(cur);
        let copy_len = slice.len().min(available);
        self.ch[cur..cur + copy_len].copy_from_slice(&slice[..copy_len]);
        self.len = (cur + copy_len) as u8;
    }
}

pub fn unicode_cell_width(cp: char) -> u16 {
    UnicodeWidthChar::width(cp).unwrap_or(0) as u16
}

pub struct Renderer {
    pub buf: Vec<Cell>,
    pub prev: Vec<Cell>,
    pub width: u16,
    pub height: u16,
    pub force_full_redraw: bool,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub cursor_visible: bool,
    out_buf: Vec<u8>,
}

impl Renderer {
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            buf: vec![Cell::default(); size],
            prev: vec![Cell::default(); size],
            width,
            height,
            force_full_redraw: true,
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: false,
            out_buf: Vec::with_capacity(65536),
        }
    }

    pub fn resize(&mut self, new_w: u16, new_h: u16) {
        let size = (new_w as usize) * (new_h as usize);
        self.buf.clear();
        self.buf.resize(size, Cell::default());
        self.prev.clear();
        self.prev.resize(size, Cell::default());
        self.width = new_w;
        self.height = new_h;
        self.force_full_redraw = true;
    }

    pub fn set_cursor(&mut self, x: u16, y: u16, visible: bool) {
        self.cursor_x = x;
        self.cursor_y = y;
        self.cursor_visible = visible;
    }

    pub fn fill_rect(&mut self, rect: Rect, bg: u32) {
        let max_y = self.height.min(rect.y.saturating_add(rect.h));
        let max_x = self.width.min(rect.x.saturating_add(rect.w));
        for y in rect.y..max_y {
            let row_start = (y as usize) * (self.width as usize);
            for x in rect.x..max_x {
                let c = &mut self.buf[row_start + (x as usize)];
                c.set_char(" ");
                c.fg = 0xd4d4d4;
                c.bg = bg;
                c.bold = false;
                c.italic = false;
                c.underline = false;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn put_cell(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        fg: u32,
        bg: u32,
        bold: bool,
        italic: bool,
        underline: bool,
    ) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        let mut cell = Cell {
            fg,
            bg,
            bold,
            italic,
            underline,
            ..Default::default()
        };
        if text.is_empty() {
            cell.ch = [0u8; 12];
            cell.len = 0;
            cell.continuation = true;
        } else {
            cell.set_char(text);
        }
        self.buf[idx] = cell;
    }

    pub fn draw_text(&mut self, x: u16, y: u16, text: &str, fg: u32, bg: u32) {
        self.draw_text_styled(x, y, text, fg, bg, false, false);
    }

    pub fn draw_text_styled(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        fg: u32,
        bg: u32,
        bold: bool,
        italic: bool,
    ) {
        if y >= self.height || x >= self.width {
            return;
        }
        // Native controls can use theme selection/accent backgrounds that differ
        // from the panel. Resolve contrast against the background actually drawn.
        // Neovim grids use put_cell and retain their own highlight definitions.
        let fg = readable_u32(fg, bg, 4.5);
        let max_width = self.width.saturating_sub(x);
        let mut col = x;
        let mut char_indices = text.char_indices().peekable();
        while let Some((byte_idx, cp)) = char_indices.next() {
            let next_byte = char_indices
                .peek()
                .map(|(idx, _)| *idx)
                .unwrap_or(text.len());
            let slice = &text[byte_idx..next_byte];
            let cell_width = unicode_cell_width(cp);
            if cell_width == 0 {
                if col > x && col - 1 < self.width {
                    let row_start = (y as usize) * (self.width as usize);
                    let prev_col =
                        if self.buf[row_start + (col as usize) - 1].continuation && col >= 2 {
                            col - 2
                        } else {
                            col - 1
                        };
                    self.buf[row_start + (prev_col as usize)].append_bytes(slice.as_bytes());
                }
                continue;
            }
            if col >= self.width || (col - x) + cell_width > max_width {
                break;
            }
            let row_start = (y as usize) * (self.width as usize);
            let mut cell = Cell {
                fg,
                bg,
                bold,
                italic,
                underline: false,
                ..Default::default()
            };
            cell.set_char(slice);
            self.buf[row_start + (col as usize)] = cell;

            if cell_width == 2 && col + 1 < self.width {
                self.buf[row_start + (col as usize) + 1] = Cell {
                    ch: [0u8; 12],
                    len: 0,
                    continuation: true,
                    fg,
                    bg,
                    bold,
                    italic,
                    underline: false,
                };
            }
            col += cell_width;
        }
    }

    pub fn draw_hline(&mut self, x: u16, y: u16, w: u16, ch: &str, fg: u32, bg: u32) {
        if y >= self.height {
            return;
        }
        let end_x = self.width.min(x.saturating_add(w));
        for cx in x..end_x {
            self.put_cell(cx, y, ch, fg, bg, false, false, false);
        }
    }

    pub fn draw_vline(&mut self, x: u16, y: u16, h: u16, ch: &str, fg: u32, bg: u32) {
        if x >= self.width {
            return;
        }
        let end_y = self.height.min(y.saturating_add(h));
        for cy in y..end_y {
            self.put_cell(x, cy, ch, fg, bg, false, false, false);
        }
    }

    pub fn dim(&mut self, background: u32) {
        for cell in &mut self.buf {
            cell.fg = blend_u32(background, cell.fg, 38);
            cell.bg = blend_u32(background, cell.bg, 50);
        }
        self.cursor_visible = false;
    }

    pub fn highlight_rect(&mut self, rect: Rect, hover_bg: u32) {
        for y in rect.y..rect.y.saturating_add(rect.h).min(self.height) {
            for x in rect.x..rect.x.saturating_add(rect.w).min(self.width) {
                let cell = &mut self.buf[y as usize * self.width as usize + x as usize];
                cell.bg = hover_bg;
                cell.fg = readable_u32(cell.fg, hover_bg, 4.5);
            }
        }
    }

    pub fn apply_hover(&mut self, x: u16, y: u16, hover_bg: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if self.buf[idx].ch[0] == b' ' && self.buf[idx].len == 1 {
            // Include label padding, but leave empty background cells alone.
            let cell = self.buf[idx];
            let adjacent_label = [x.checked_sub(1), x.checked_add(1)]
                .into_iter()
                .flatten()
                .filter(|&nx| nx < self.width)
                .any(|nx| {
                    let neighbor = self.buf[y as usize * self.width as usize + nx as usize];
                    neighbor.ch[0] != b' ' && neighbor.fg == cell.fg && neighbor.bg == cell.bg
                });
            if !adjacent_label {
                return;
            }
        }
        // Highlight contiguous non-border label or row segment under pointer
        self.buf[idx].bg = hover_bg;
        self.buf[idx].fg = readable_u32(self.buf[idx].fg, hover_bg, 4.5);
        self.buf[idx].bold = true;
    }

    fn emit_style(
        out: &mut Vec<u8>,
        cell: &Cell,
        cur_fg: &mut Option<u32>,
        cur_bg: &mut Option<u32>,
        cur_bold: &mut bool,
        cur_italic: &mut bool,
        cur_underline: &mut bool,
    ) {
        let mut style_reset = false;
        if cell.bold != *cur_bold
            || cell.italic != *cur_italic
            || cell.underline != *cur_underline
        {
            if (*cur_bold && !cell.bold)
                || (*cur_italic && !cell.italic)
                || (*cur_underline && !cell.underline)
            {
                out.extend_from_slice(b"\x1b[0m");
                *cur_fg = None;
                *cur_bg = None;
                *cur_bold = false;
                *cur_italic = false;
                *cur_underline = false;
                style_reset = true;
            }
            if cell.bold && !*cur_bold {
                out.extend_from_slice(b"\x1b[1m");
                *cur_bold = true;
            }
            if cell.italic && !*cur_italic {
                out.extend_from_slice(b"\x1b[3m");
                *cur_italic = true;
            }
            if cell.underline && !*cur_underline {
                out.extend_from_slice(b"\x1b[4m");
                *cur_underline = true;
            }
        }

        if *cur_fg != Some(cell.fg) || style_reset {
            *cur_fg = Some(cell.fg);
            let r = (cell.fg >> 16) & 0xff;
            let g = (cell.fg >> 8) & 0xff;
            let b = cell.fg & 0xff;
            let _ = write!(out, "\x1b[38;2;{};{};{}m", r, g, b);
        }

        if *cur_bg != Some(cell.bg) || style_reset {
            *cur_bg = Some(cell.bg);
            let r = (cell.bg >> 16) & 0xff;
            let g = (cell.bg >> 8) & 0xff;
            let b = cell.bg & 0xff;
            let _ = write!(out, "\x1b[48;2;{};{};{}m", r, g, b);
        }
    }

    pub fn flush<W: Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let _timer = ScopedTimer::start(&GLOBAL.flush);
        self.out_buf.clear();
        self.out_buf.extend_from_slice(b"\x1b[?25l");

        let mut cur_fg: Option<u32> = None;
        let mut cur_bg: Option<u32> = None;
        let mut cur_bold = false;
        let mut cur_italic = false;
        let mut cur_underline = false;
        let mut style_initialized = false;
        let width = self.width as usize;

        for y in 0..self.height as usize {
            let row_start = y * width;
            let mut x = 0usize;
            while x < width {
                let idx = row_start + x;
                let cell = &self.buf[idx];
                if !self.force_full_redraw && *cell == self.prev[idx] {
                    x += 1;
                    continue;
                }
                if cell.continuation && x > 0 && !self.force_full_redraw {
                    x -= 1;
                    continue;
                }

                let mut run_end = x + 1;
                while run_end < width {
                    let next_idx = row_start + run_end;
                    if self.force_full_redraw
                        || self.buf[next_idx] != self.prev[next_idx]
                        || self.buf[next_idx].continuation
                    {
                        run_end += 1;
                    } else {
                        break;
                    }
                }

                let _ = write!(self.out_buf, "\x1b[{};{}H", y + 1, x + 1);
                if !style_initialized {
                    self.out_buf.extend_from_slice(b"\x1b[0m");
                    style_initialized = true;
                }

                let mut cursor = x;
                while cursor < run_end {
                    let c_idx = row_start + cursor;
                    let c = &self.buf[c_idx];
                    self.prev[c_idx] = *c;
                    if c.continuation {
                        cursor += 1;
                        continue;
                    }
                    Self::emit_style(
                        &mut self.out_buf,
                        c,
                        &mut cur_fg,
                        &mut cur_bg,
                        &mut cur_bold,
                        &mut cur_italic,
                        &mut cur_underline,
                    );
                    if c.len == 0 {
                        self.out_buf.push(b' ');
                    } else {
                        self.out_buf.extend_from_slice(&c.ch[..c.len as usize]);
                    }
                    cursor += 1;
                }

                x = run_end;
            }
        }

        if style_initialized {
            self.out_buf.extend_from_slice(b"\x1b[0m");
        }
        if self.cursor_visible && self.cursor_x < self.width && self.cursor_y < self.height {
            let _ = write!(
                self.out_buf,
                "\x1b[{};{}H\x1b[?25h",
                self.cursor_y + 1,
                self.cursor_x + 1
            );
        }
        self.force_full_redraw = false;

        if !self.out_buf.is_empty() {
            let _wtimer = ScopedTimer::start(&GLOBAL.writer_flush);
            writer.write_all(&self.out_buf)?;
            writer.flush()?;
            if GLOBAL.enabled.load(Ordering::Relaxed) {
                GLOBAL
                    .emitted_bytes
                    .fetch_add(self.out_buf.len() as u64, Ordering::Relaxed);
            }
        }
        Ok(())
    }
}
