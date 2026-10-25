//! The bottom panel owns a shell PTY, independent of Neovim and its input modes.
use std::collections::VecDeque;
use std::io;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use vt100::{Color, MouseProtocolEncoding, MouseProtocolMode};

use super::layout::Rect;
use super::renderer::Renderer;
use super::theme::{blend_u32, chrome, Theme};

const SCROLLBACK: usize = 10_000;
const MAX_PENDING_INPUT: usize = 4 * 1024 * 1024;

struct TerminalCallbacks {
    replies: Vec<u8>,
    theme: Theme,
}

impl vt100::Callbacks for TerminalCallbacks {
    fn unhandled_csi(&mut self, screen: &mut vt100::Screen, i1: Option<u8>, _: Option<u8>, params: &[&[u16]], c: char) {
        let p = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, c, p) {
            (None, 'n', 5) => self.replies.extend_from_slice(b"\x1b[0n"),
            (None, 'n', 6) => {
                let (row, col) = screen.cursor_position();
                self.replies.extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
            }
            (None, 'c', 0) => self.replies.extend_from_slice(b"\x1b[?1;2c"),
            (Some(b'>'), 'c', 0) => self.replies.extend_from_slice(b"\x1b[>0;0;0c"),
            _ => {}
        }
    }

    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        let Some(code) = params.first().and_then(|s| std::str::from_utf8(s).ok()) else { return; };
        let reply = |key: &str, rgb: u32| format!("\x1b]{key};rgb:{:04x}/{:04x}/{:04x}\x1b\\",
            ((rgb >> 16) & 255) * 257, ((rgb >> 8) & 255) * 257, (rgb & 255) * 257);
        if params.get(1) == Some(&b"?".as_slice()) {
            let color = match code { "10" | "12" => self.theme.fg_terminal, "11" => self.theme.bg_terminal, _ => return };
            self.replies.extend_from_slice(reply(code, color).as_bytes());
        } else if code == "4" {
            for pair in params[1..].chunks_exact(2) {
                if pair[1] != b"?" { continue; }
                if let Some(index) = std::str::from_utf8(pair[0]).ok().and_then(|s| s.parse::<u8>().ok()) {
                    self.replies.extend_from_slice(reply(&format!("4;{index}"), indexed_color(index, &self.theme)).as_bytes());
                }
            }
        }
    }
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    fd: i32,
}

impl Drop for Session {
    fn drop(&mut self) {
        // Stop the foreground job as well as the shell, and always reap our child.
        if let Some(group) = self.master.process_group_leader().filter(|&pid| pid > 1) {
            unsafe { libc::kill(-group, libc::SIGHUP); }
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

pub struct ShellTerminal {
    session: Option<Session>,
    parser: vt100::Parser<TerminalCallbacks>,
    pending: VecDeque<u8>,
    selection: Option<((u16, u16), (u16, u16))>,
    selecting: bool,
    pub copy_requested: Option<String>,
    pub paste_requested: bool,
    status: String,
    error: Option<String>,
}

impl Default for ShellTerminal {
    fn default() -> Self {
        Self {
            session: None,
            parser: vt100::Parser::new_with_callbacks(10, 80, SCROLLBACK,
                TerminalCallbacks { replies: Vec::new(), theme: Theme::default() }),
            pending: VecDeque::new(), selection: None, selecting: false,
            copy_requested: None, paste_requested: false, status: String::new(), error: None,
        }
    }
}

impl ShellTerminal {
    pub fn fd(&self) -> i32 { self.session.as_ref().map_or(-1, |s| s.fd) }
    pub fn wants_write(&self) -> bool { !self.pending.is_empty() && self.session.is_some() }
    pub fn selecting(&self) -> bool { self.selecting }
    pub fn set_theme(&mut self, theme: Theme) { self.parser.callbacks_mut().theme = theme; }
    pub fn take_error(&mut self) -> Option<String> { self.error.take() }

    pub fn ensure_running(&mut self, cols: u16, rows: u16) {
        self.pump();
        if self.session.is_some() { self.resize(cols, rows); return; }
        match Self::spawn(cols.max(1), rows.max(1)) {
            Ok(session) => {
                let theme = self.parser.callbacks().theme;
                self.parser = vt100::Parser::new_with_callbacks(rows.max(1), cols.max(1), SCROLLBACK,
                    TerminalCallbacks { replies: Vec::new(), theme });
                self.session = Some(session);
                self.pending.clear();
                self.selection = None;
                self.selecting = false;
                self.status.clear();
            }
            Err(error) => {
                self.status = format!("Could not start terminal: {error}");
                self.error = Some(self.status.clone());
            }
        }
    }

    fn spawn(cols: u16, rows: u16) -> Result<Session, String> {
        let pair = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
        let fd = pair.master.as_raw_fd().ok_or("PTY has no pollable descriptor")?;
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
        let mut command = CommandBuilder::new(shell);
        command.arg("-i");
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env_remove("NVIM");
        command.env_remove("NVIM_LISTEN_ADDRESS");
        command.env_remove("COLUMNS");
        command.env_remove("LINES");
        if let Ok(cwd) = std::env::current_dir() { command.cwd(cwd); }
        let child = pair.slave.spawn_command(command).map_err(|e| e.to_string())?;
        drop(pair.slave);
        Ok(Session { master: pair.master, child, fd })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 || self.parser.screen().size() == (rows, cols) { return; }
        if let Some(session) = &self.session {
            if let Err(e) = session.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }) {
                self.error = Some(format!("Terminal resize failed: {e}"));
                return;
            }
        }
        self.parser.screen_mut().set_size(rows, cols);
        self.selection = None;
        self.selecting = false;
    }

    fn queue(&mut self, bytes: &[u8]) {
        if self.session.is_none() { return; }
        if self.pending.len() + bytes.len() > MAX_PENDING_INPUT {
            self.error = Some("Terminal input queue is full; wait for the program to read input.".into());
            return;
        }
        self.pending.extend(bytes);
        self.flush_input();
    }

    pub fn flush_input(&mut self) {
        let fd = self.fd();
        if fd < 0 { return; }
        while !self.pending.is_empty() {
            let bytes = self.pending.as_slices().0;
            let n = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
            if n > 0 { self.pending.drain(..n as usize); }
            else {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted { continue; }
                if error.kind() != io::ErrorKind::WouldBlock {
                    self.error = Some(format!("Terminal input failed: {error}"));
                    self.pending.clear();
                }
                break;
            }
        }
    }

    pub fn pump(&mut self) -> bool {
        let fd = self.fd();
        if fd < 0 { return false; }
        let mut changed = false;
        let mut closed = false;
        let mut bytes = [0u8; 16384];
        // Bound each pass so a noisy child cannot starve keyboard input.
        for _ in 0..64 {
            let n = unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
            if n > 0 {
                self.parser.process(&bytes[..n as usize]);
                let replies = std::mem::take(&mut self.parser.callbacks_mut().replies);
                self.queue(&replies);
                changed = true;
            } else if n == 0 { closed = true; break; }
            else {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted { continue; }
                if error.kind() != io::ErrorKind::WouldBlock {
                    closed = true;
                    if error.raw_os_error() != Some(libc::EIO) { self.error = Some(format!("Terminal output failed: {error}")); }
                }
                break;
            }
        }
        if closed {
            self.session.take();
            self.pending.clear();
            self.status = "Shell exited — Enter to start a new shell".into();
            self.selection = None;
            self.selecting = false;
            changed = true;
        } else if changed {
            // A selection refers to displayed cells; discard it when output moves them.
            self.selection = None;
            self.selecting = false;
        }
        changed
    }

    pub fn paste(&mut self, text: &str) {
        self.selection = None;
        self.parser.screen_mut().set_scrollback(0);
        let bytes = if self.parser.screen().bracketed_paste() {
            format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', ""))
        } else { text.replace("\r\n", "\n").replace('\n', "\r") };
        self.queue(bytes.as_bytes());
    }

    pub fn handle_key(&mut self, key: &str) {
        if matches!(key, "<C-S-c>" | "<C-Insert>") || (key == "<C-c>" && self.selection.is_some()) {
            self.copy_requested = self.selected_text();
            self.selection = None;
            return;
        }
        if matches!(key, "<C-v>" | "<C-S-v>" | "<S-Insert>") { self.paste_requested = true; return; }
        if matches!(key, "<S-PageUp>" | "<S-PageDown>") {
            let delta = self.parser.screen().size().0 as usize;
            self.scroll(key == "<S-PageUp>", delta);
            return;
        }
        if self.session.is_none() {
            if key == "<CR>" {
                let (rows, cols) = self.parser.screen().size();
                self.ensure_running(cols, rows);
            }
            return;
        }
        self.selection = None;
        self.parser.screen_mut().set_scrollback(0);
        let bytes = encode_key(key, self.parser.screen().application_cursor());
        self.queue(&bytes);
    }

    fn scroll(&mut self, up: bool, count: usize) {
        let current = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(if up { current.saturating_add(count) } else { current.saturating_sub(count) });
        self.selection = None;
        self.selecting = false;
    }

    pub fn handle_mouse(&mut self, x: u16, y: u16, button: u8, pressed: bool, motion: bool) {
        let (rows, cols) = self.parser.screen().size();
        let x = x.min(cols.saturating_sub(1));
        let y = y.min(rows.saturating_sub(1));
        let mode = self.parser.screen().mouse_protocol_mode();
        let shift = button & 4 != 0;
        if mode != MouseProtocolMode::None && !shift && self.parser.screen().scrollback() == 0 {
            let report = if motion {
                mode == MouseProtocolMode::AnyMotion || (mode == MouseProtocolMode::ButtonMotion && button & 3 != 3)
            } else { pressed || mode != MouseProtocolMode::Press };
            if report {
                let release = !pressed && !motion;
                let bytes = match self.parser.screen().mouse_protocol_encoding() {
                    MouseProtocolEncoding::Sgr => format!("\x1b[<{};{};{}{}", button, x + 1, y + 1, if release { 'm' } else { 'M' }).into_bytes(),
                    encoding => {
                        let code = if release { (button & 28) | 3 } else { button };
                        if encoding == MouseProtocolEncoding::Utf8 {
                            format!("\x1b[M{}{}{}", char::from_u32(code as u32 + 32).unwrap(),
                                char::from_u32(x as u32 + 33).unwrap_or(' '), char::from_u32(y as u32 + 33).unwrap_or(' ')).into_bytes()
                        } else if x < 223 && y < 223 { vec![27, b'[', b'M', code + 32, x as u8 + 33, y as u8 + 33] }
                        else { Vec::new() }
                    }
                };
                self.queue(&bytes);
            }
            return;
        }
        if button & 64 != 0 && pressed {
            if self.parser.screen().alternate_screen() && !shift {
                let key = if button & 1 == 0 { "<Up>" } else { "<Down>" };
                let bytes = encode_key(key, self.parser.screen().application_cursor()).repeat(3);
                self.queue(&bytes);
            } else { self.scroll(button & 1 == 0, 3); }
            return;
        }
        if button & 3 == 0 {
            if motion && self.selecting {
                if let Some((_, end)) = &mut self.selection { *end = (y, x); }
            } else if pressed {
                self.selection = Some(((y, x), (y, x)));
                self.selecting = true;
            } else if !motion {
                self.selecting = false;
                if self.selection.is_some_and(|(start, end)| start == end) { self.selection = None; }
            }
        }
    }

    fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection?;
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        Some(self.parser.screen().contents_between(start.0, start.1, end.0, end.1.saturating_add(1)))
    }

    pub fn draw(&self, renderer: &mut Renderer, rect: Rect, theme: &Theme, focused: bool) {
        renderer.fill_rect(rect, theme.bg_terminal);
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        for y in 0..rows.min(rect.h) {
            for x in 0..cols.min(rect.w) {
                let Some(cell) = screen.cell(y, x) else { continue; };
                let mut fg = resolve_color(cell.fgcolor(), theme.fg_terminal, theme);
                let mut bg = resolve_color(cell.bgcolor(), theme.bg_terminal, theme);
                if cell.dim() { fg = blend_u32(bg, fg, 60); }
                if cell.inverse() { std::mem::swap(&mut fg, &mut bg); }
                if let Some((a, b)) = self.selection {
                    let (start, end) = if a <= b { (a, b) } else { (b, a) };
                    if (start..=end).contains(&(y, x)) { bg = chrome(theme).row_selected_bg; fg = chrome(theme).text_strong; }
                }
                let text = if cell.is_wide_continuation() { "" } else if cell.has_contents() { cell.contents() } else { " " };
                renderer.put_cell(rect.x + x, rect.y + y, text, fg, bg, cell.bold(), cell.italic(), cell.underline());
            }
        }
        if !self.status.is_empty() && rect.h > 0 {
            super::widgets::primitives::text_in(renderer, Rect::new(rect.x, rect.y + rect.h - 1, rect.w, 1),
                &self.status, theme.fg_terminal, theme.bg_terminal, false);
        }
        if focused && self.session.is_some() && screen.scrollback() == 0 && !screen.hide_cursor() && self.selection.is_none() {
            let (y, x) = screen.cursor_position();
            if x < rect.w && y < rect.h { renderer.set_cursor(rect.x + x, rect.y + y, true); }
        }
    }
}

fn resolve_color(color: Color, default: u32, theme: &Theme) -> u32 {
    match color { Color::Default => default, Color::Idx(i) => indexed_color(i, theme),
        Color::Rgb(r, g, b) => (r as u32) << 16 | (g as u32) << 8 | b as u32 }
}

fn indexed_color(index: u8, theme: &Theme) -> u32 {
    if index < 16 { theme.terminal_colors[index as usize] }
    else if index < 232 {
        let n = (index - 16) as u32;
        let channel = |v| if v == 0 { 0 } else { 55 + 40 * v };
        channel(n / 36) << 16 | channel(n / 6 % 6) << 8 | channel(n % 6)
    } else { let value = 8 + (index as u32 - 232) * 10; value * 0x010101 }
}

fn encode_key(key: &str, application_cursor: bool) -> Vec<u8> {
    let sequence = match key {
        "<Esc>" => "\x1b", "<CR>" => "\r", "<BS>" => "\x7f", "<Tab>" => "\t", "<S-Tab>" => "\x1b[Z",
        "<Up>" => if application_cursor { "\x1bOA" } else { "\x1b[A" },
        "<Down>" => if application_cursor { "\x1bOB" } else { "\x1b[B" },
        "<Right>" => if application_cursor { "\x1bOC" } else { "\x1b[C" },
        "<Left>" => if application_cursor { "\x1bOD" } else { "\x1b[D" },
        "<Home>" => if application_cursor { "\x1bOH" } else { "\x1b[H" },
        "<End>" => if application_cursor { "\x1bOF" } else { "\x1b[F" },
        "<Insert>" => "\x1b[2~", "<Del>" => "\x1b[3~", "<PageUp>" => "\x1b[5~", "<PageDown>" => "\x1b[6~",
        "<S-F6>" => "\x1b[17;2~",
        "<M-Up>" => "\x1b[1;3A", "<M-Down>" => "\x1b[1;3B",
        "<M-Right>" => "\x1b[1;3C", "<M-Left>" => "\x1b[1;3D",
        "<F1>" => "\x1bOP", "<F2>" => "\x1bOQ", "<F3>" => "\x1bOR", "<F4>" => "\x1bOS",
        "<F5>" => "\x1b[15~", "<F6>" => "\x1b[17~", "<F7>" => "\x1b[18~", "<F8>" => "\x1b[19~",
        "<F9>" => "\x1b[20~", "<F10>" => "\x1b[21~", "<F11>" => "\x1b[23~", "<F12>" => "\x1b[24~",
        _ => {
            if let Some(ch) = key.strip_prefix("<C-M-").and_then(|k| k.strip_suffix('>')) {
                if ch.len() == 1 && ch.as_bytes()[0].is_ascii_alphabetic() {
                    return vec![0x1b, ch.as_bytes()[0].to_ascii_uppercase() & 31];
                }
            }
            if key.starts_with("<C-") && key.ends_with('>') && key.len() == 5 {
                let ch = key.as_bytes()[3].to_ascii_uppercase();
                if (b'@'..=b'_').contains(&ch) { return vec![ch & 31]; }
            }
            if let Some(ch) = key.strip_prefix("<M-").and_then(|k| k.strip_suffix('>')) {
                return [b"\x1b".as_slice(), ch.as_bytes()].concat();
            }
            return key.as_bytes().to_vec();
        }
    };
    sequence.as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbound_modified_keys_roundtrip_to_shell() {
        for key in ["<S-F6>", "<M-Up>", "<M-Down>", "<M-Left>", "<M-Right>", "<C-M-s>", "<C-M-f>", "<C-M-c>"] {
            assert_eq!(crate::tui::input::decode_key_sequence(&encode_key(key, false)), key);
        }
    }

    #[test]
    fn default_and_indexed_colors_follow_theme_but_truecolor_is_preserved() {
        let mut terminal = ShellTerminal::default();
        terminal.parser.process(b"plain\x1b[31m red\x1b[38;2;1;2;3m rgb\x1b[0m");
        let mut renderer = Renderer::new(80, 10);
        let rect = Rect::new(0, 0, 80, 10);
        let mut theme = Theme::default();
        for (bg, fg, red) in [(0x101010, 0xeeeeee, 0xcc3333), (0xeeeeee, 0x222222, 0x992222)] {
            theme.bg_terminal = bg;
            theme.fg_terminal = fg;
            theme.terminal_colors[1] = red;
            terminal.draw(&mut renderer, rect, &theme, false);
            assert_eq!(renderer.buf[0].bg, bg);
            assert_eq!(renderer.buf[0].fg, fg);
            assert_eq!(renderer.buf[6].fg, red);
            assert_eq!(renderer.buf[10].fg, 0x010203);
            assert_eq!(renderer.buf[79].bg, bg);
        }
    }

    #[test]
    fn selection_is_direct_and_keyboard_escape_is_a_single_byte() {
        let mut terminal = ShellTerminal::default();
        terminal.parser.process("hello 界!".as_bytes());
        terminal.handle_mouse(0, 0, 0, true, false);
        terminal.handle_mouse(4, 0, 32, false, true);
        terminal.handle_mouse(4, 0, 0, false, false);
        assert_eq!(terminal.selected_text().as_deref(), Some("hello"));
        terminal.handle_key("<C-c>");
        assert_eq!(terminal.copy_requested.as_deref(), Some("hello"));
        assert!(terminal.selection.is_none());
        assert_eq!(encode_key("<Esc>", false), b"\x1b");
        assert_eq!(encode_key("<C-c>", false), b"\x03");
        assert_eq!(encode_key("<C-e>", false), b"\x05");
        assert_eq!(encode_key("<Up>", true), b"\x1bOA");
        assert_eq!(encode_key("\x1b[1;5D", false), b"\x1b[1;5D");
        let mut renderer = Renderer::new(80, 10);
        terminal.draw(&mut renderer, Rect::new(0, 0, 80, 10), &Theme::default(), false);
        assert!(renderer.buf[7].continuation);
    }

    #[test]
    fn scrollback_alternate_screen_and_split_device_queries() {
        let mut terminal = ShellTerminal::default();
        for i in 0..25 { terminal.parser.process(format!("line {i}\r\n").as_bytes()); }
        terminal.handle_mouse(1, 1, 64, true, false);
        assert_eq!(terminal.parser.screen().scrollback(), 3);
        terminal.handle_mouse(1, 1, 65, true, false);
        assert_eq!(terminal.parser.screen().scrollback(), 0);
        let contents = terminal.parser.screen().contents();
        terminal.parser.process(b"\x1b[?1049h\x1b[2J\x1b[Happ");
        assert!(terminal.parser.screen().alternate_screen());
        terminal.parser.process(b"\x1b[?1049l");
        assert_eq!(terminal.parser.screen().contents(), contents);
        terminal.parser.process(b"\x1b[2;4H\x1b[");
        terminal.parser.process(b"6n\x1b[5n\x1b[c\x1b]11;?");
        terminal.parser.process(b"\x1b\\");
        let replies = &terminal.parser.callbacks().replies;
        assert!(replies.starts_with(b"\x1b[2;4R\x1b[0n\x1b[?1;2c"));
        assert!(replies.windows(5).any(|w| w == b"]11;r"));
    }
}
