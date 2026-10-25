use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicI32, Ordering};

pub static SIGWINCH_RECEIVED: AtomicBool = AtomicBool::new(false);
pub static QUIT_SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);
pub static SIGWINCH_PIPE_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
static UNGET_BYTE: AtomicI16 = AtomicI16::new(-1);

pub extern "C" fn handle_sigwinch(_sig: libc::c_int) {
    SIGWINCH_RECEIVED.store(true, Ordering::Relaxed);
    let fd = SIGWINCH_PIPE_WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let b = [b'W'];
        unsafe {
            let _ = libc::write(fd, b.as_ptr() as *const _, 1);
        }
    }
}

pub extern "C" fn handle_quit_signal(_sig: libc::c_int) {
    QUIT_SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
    let fd = SIGWINCH_PIPE_WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let b = [b'Q'];
        unsafe {
            let _ = libc::write(fd, b.as_ptr() as *const _, 1);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    Move,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    pub col: u16,
    pub row: u16,
    pub raw_button: u8,
    pub button: MouseButton,
    pub action: MouseAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub ch: u8,
    pub ctrl: bool,
    pub alt: bool,
    pub raw: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize { cols: u16, rows: u16 },
    None,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Key(String),
    Paste(String),
    Mouse {
        x: u16,
        y: u16,
        button: u8,
        pressed: bool,
        motion: bool,
    },
    Resize(u16, u16),
}

impl Event {
    pub fn to_input_event(&self) -> Option<InputEvent> {
        match self {
            Event::None | Event::Quit => None,
            Event::Resize { cols, rows } => Some(InputEvent::Resize(*cols, *rows)),
            Event::Paste(s) => Some(InputEvent::Paste(s.clone())),
            Event::Mouse(m) => Some(InputEvent::Mouse {
                x: m.col,
                y: m.row,
                button: m.raw_button,
                pressed: m.action == MouseAction::Press,
                motion: m.action == MouseAction::Move,
            }),
            Event::Key(k) => Some(InputEvent::Key(decode_key_sequence(&k.raw))),
        }
    }
}

pub fn decode_key_sequence(raw: &[u8]) -> String {
    if raw.is_empty() {
        return String::new();
    }
    match raw {
        b"\x1b" => return "<Esc>".to_string(),
        b"\r" | b"\n" => return "<CR>".to_string(),
        b"\t" => return "<Tab>".to_string(),
        b"\x1b[Z" => return "<S-Tab>".to_string(),
        b"\x7f" | b"\x08" => return "<BS>".to_string(),
        b"\x1b[A" | b"\x1bOA" => return "<Up>".to_string(),
        b"\x1b[B" | b"\x1bOB" => return "<Down>".to_string(),
        b"\x1b[C" | b"\x1bOC" => return "<Right>".to_string(),
        b"\x1b[D" | b"\x1bOD" => return "<Left>".to_string(),
        b"\x1b[H" | b"\x1bOH" | b"\x1b[1~" | b"\x1b[7~" => return "<Home>".to_string(),
        b"\x1b[F" | b"\x1bOF" | b"\x1b[4~" | b"\x1b[8~" => return "<End>".to_string(),
        b"\x1b[5~" => return "<PageUp>".to_string(),
        b"\x1b[6~" => return "<PageDown>".to_string(),
        b"\x1b[2~" => return "<Insert>".to_string(),
        b"\x1b[2;2~" => return "<S-Insert>".to_string(),
        b"\x1b[2;5~" => return "<C-Insert>".to_string(),
        b"\x1b[99;5u" | b"\x1b[27;5;99~" => return "<C-c>".to_string(),
        b"\x1b[118;5u" | b"\x1b[27;5;118~" => return "<C-v>".to_string(),
        b"\x1b[5;2~" => return "<S-PageUp>".to_string(),
        b"\x1b[6;2~" => return "<S-PageDown>".to_string(),
        b"\x1b[99;6u" | b"\x1b[67;6u" | b"\x1b[27;6;99~" => return "<C-S-c>".to_string(),
        b"\x1b[118;6u" | b"\x1b[86;6u" | b"\x1b[27;6;118~" => return "<C-S-v>".to_string(),
        b"\x1b[3~" => return "<Del>".to_string(),
        b"\x1bOP" | b"\x1b[11~" => return "<F1>".to_string(),
        b"\x1bOQ" | b"\x1b[12~" => return "<F2>".to_string(),
        b"\x1bOR" | b"\x1b[13~" => return "<F3>".to_string(),
        b"\x1bOS" | b"\x1b[14~" => return "<F4>".to_string(),
        b"\x1b[15~" => return "<F5>".to_string(),
        b"\x1b[17~" => return "<F6>".to_string(),
        b"\x1b[17;2~" => return "<S-F6>".to_string(),
        b"\x1b[1;3A" => return "<M-Up>".to_string(),
        b"\x1b[1;3B" => return "<M-Down>".to_string(),
        b"\x1b[1;3C" => return "<M-Right>".to_string(),
        b"\x1b[1;3D" => return "<M-Left>".to_string(),
        b"\x1b[18~" => return "<F7>".to_string(),
        b"\x1b[19~" => return "<F8>".to_string(),
        b"\x1b[20~" => return "<F9>".to_string(),
        b"\x1b[21~" => return "<F10>".to_string(),
        b"\x1b[23~" => return "<F11>".to_string(),
        b"\x1b[24~" => return "<F12>".to_string(),
        _ => {}
    }

    // Ctrl+Alt chords may arrive as Escape + control byte or CSI-u.
    if raw.len() == 2 && raw[0] == 0x1b && (1..=26).contains(&raw[1]) {
        return format!("<C-M-{}>", (b'a' + raw[1] - 1) as char);
    }
    if let Ok(sequence) = std::str::from_utf8(raw) {
        if let Some(body) = sequence.strip_prefix("\x1b[").and_then(|s| s.strip_suffix('u')) {
            if let Some((code, "7")) = body.split_once(';') {
                if let Some(ch) = code.parse::<u32>().ok().and_then(char::from_u32) {
                    if ch.is_ascii_alphabetic() { return format!("<C-M-{}>", ch.to_ascii_lowercase()); }
                }
            }
        }
    }

    if raw.len() == 2 && raw[0] == 0x1b && raw[1] >= 32 && raw[1] < 127 {
        return format!("<M-{}>", (raw[1] as char).to_ascii_lowercase());
    }

    if raw.len() == 1 {
        let b = raw[0];
        if (1..=26).contains(&b) {
            let ch = (b'a' + (b - 1)) as char;
            return format!("<C-{}>", ch);
        }
        if b == 0x1c {
            return "<C-\\>".to_string();
        }
    }

    String::from_utf8_lossy(raw).into_owned()
}

pub fn has_pending_event() -> bool {
    UNGET_BYTE.load(Ordering::Relaxed) >= 0
}

fn read_byte_timeout(fd: RawFd, timeout_ms: i32) -> Option<u8> {
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if rc > 0 && (pfd.revents & libc::POLLIN) != 0 {
        let mut buf = [0u8; 1];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, 1) };
        if n == 1 {
            return Some(buf[0]);
        }
    }
    None
}

pub fn parse_sgr_mouse(seq: &[u8]) -> Option<MouseEvent> {
    if !seq.starts_with(b"\x1b[<") || seq.len() < 6 {
        return None;
    }
    let last_char = *seq.last()?;
    if last_char != b'M' && last_char != b'm' {
        return None;
    }
    let params_str = std::str::from_utf8(&seq[3..seq.len() - 1]).ok()?;
    let mut iter = params_str.split(';');
    let b: u8 = iter.next()?.parse().ok()?;
    let col: u16 = iter.next()?.parse().ok()?;
    let row: u16 = iter.next()?.parse().ok()?;

    let action = if last_char == b'M' {
        if (b & 32) != 0 {
            MouseAction::Move
        } else {
            MouseAction::Press
        }
    } else {
        MouseAction::Release
    };

    let button = if (b & 64) != 0 {
        if (b & 3) == 0 {
            MouseButton::WheelUp
        } else {
            MouseButton::WheelDown
        }
    } else {
        match b & 3 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::None,
        }
    };

    Some(MouseEvent {
        col: col.saturating_sub(1),
        row: row.saturating_sub(1),
        raw_button: b,
        button,
        action,
    })
}

pub fn read_event(fd: RawFd) -> Event {
    if SIGWINCH_RECEIVED.swap(false, Ordering::Relaxed) {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0 {
            return Event::Resize {
                cols: ws.ws_col,
                rows: ws.ws_row,
            };
        }
    }

    let first_byte = {
        let unget = UNGET_BYTE.swap(-1, Ordering::Relaxed);
        if unget >= 0 {
            unget as u8
        } else {
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pfd, 1, 0) };
            if rc <= 0 || (pfd.revents & libc::POLLIN) == 0 {
                return Event::None;
            }
            let mut buf = [0u8; 1];
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, 1) };
            if n <= 0 {
                return Event::None;
            }
            buf[0]
        }
    };

    let mut seq = Vec::with_capacity(64);
    seq.push(first_byte);

    if first_byte == 0x1b {
        while seq.len() < 4096 {
            if let Some(b) = read_byte_timeout(fd, 20) {
                // A second Escape starts another event (often passive mouse
                // motion). Preserve it instead of swallowing the Escape key.
                if b == 0x1b {
                    UNGET_BYTE.store(b as i16, Ordering::Relaxed);
                    break;
                }
                seq.push(b);
                let len = seq.len();
                if len == 2 {
                    if b != b'[' && b != b'O' {
                        break;
                    }
                } else if len >= 3 {
                    if seq[1] == b'[' && seq[2] == b'<' {
                        if b == b'M' || b == b'm' {
                            break;
                        }
                    } else if (64..=126).contains(&b) {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        if seq.len() == 1 {
            return Event::Key(KeyEvent {
                ch: 0x1b,
                ctrl: false,
                alt: false,
                raw: seq,
            });
        }

        if seq.starts_with(b"\x1b[<") {
            if let Some(me) = parse_sgr_mouse(&seq) {
                return Event::Mouse(me);
            }
        }

        if seq == b"\x1b[200~" {
            let mut paste_buf = Vec::new();
            let end_seq = b"\x1b[201~";
            let mut end_idx = 0usize;
            const MAX_PASTE_SIZE: usize = 2 * 1024 * 1024;
            while let Some(b) = read_byte_timeout(fd, 200) {
                if paste_buf.len() < MAX_PASTE_SIZE {
                    paste_buf.push(b);
                }
                if b == end_seq[end_idx] {
                    end_idx += 1;
                    if end_idx == end_seq.len() {
                        if paste_buf.len() >= end_seq.len() {
                            paste_buf.truncate(paste_buf.len() - end_seq.len());
                        }
                        break;
                    }
                } else if b == end_seq[0] {
                    end_idx = 1;
                } else {
                    end_idx = 0;
                }
            }
            return Event::Paste(String::from_utf8_lossy(&paste_buf).into_owned());
        }

        return Event::Key(KeyEvent {
            ch: 0,
            ctrl: false,
            alt: false,
            raw: seq,
        });
    }

    if first_byte < 32 && first_byte != 0x1b {
        return Event::Key(KeyEvent {
            ch: first_byte,
            ctrl: true,
            alt: false,
            raw: seq,
        });
    }

    // Read multi-byte UTF-8 character if first_byte >= 0x80
    let expected_utf8_len = if first_byte < 0x80 {
        1
    } else if (first_byte & 0xe0) == 0xc0 {
        2
    } else if (first_byte & 0xf0) == 0xe0 {
        3
    } else if (first_byte & 0xf8) == 0xf0 {
        4
    } else {
        1
    };

    while seq.len() < expected_utf8_len {
        if let Some(b) = read_byte_timeout(fd, 10) {
            seq.push(b);
        } else {
            break;
        }
    }

    // Check if another byte is already buffered in the kernel so the main loop drains all events
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    if unsafe { libc::poll(&mut pfd, 1, 0) } > 0 && (pfd.revents & libc::POLLIN) != 0 {
        let mut next_b = [0u8; 1];
        if unsafe { libc::read(fd, next_b.as_mut_ptr() as *mut _, 1) } == 1 {
            UNGET_BYTE.store(next_b[0] as i16, Ordering::Relaxed);
        }
    }

    Event::Key(KeyEvent {
        ch: first_byte,
        ctrl: false,
        alt: false,
        raw: seq,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn workspace_and_ai_modified_keys_decode() {
        for (raw, key) in [
            (b"\x1b[17;2~".as_slice(), "<S-F6>"),
            (b"\x1b[1;3A", "<M-Up>"), (b"\x1b[1;3B", "<M-Down>"),
            (b"\x1b[1;3C", "<M-Right>"), (b"\x1b[1;3D", "<M-Left>"),
            (b"\x1b\x13", "<C-M-s>"), (b"\x1b[102;7u", "<C-M-f>"),
            (b"\x1b\x03", "<C-M-c>"),
        ] { assert_eq!(decode_key_sequence(raw), key); }
    }

    #[test]
    fn escape_preserves_the_following_input_event() {
        let (mut writer, reader) = UnixStream::pair().unwrap();
        for following in [b"\x1b[<35;12;7M".as_slice(), b"\x1b[B", b"\x1b"] {
            writer.write_all(b"\x1b").unwrap();
            writer.write_all(following).unwrap();
            assert_eq!(read_event(reader.as_raw_fd()).to_input_event(),
                Some(InputEvent::Key("<Esc>".into())));
            assert!(has_pending_event());
            let next = read_event(reader.as_raw_fd()).to_input_event().unwrap();
            if following.starts_with(b"\x1b[<") {
                assert_eq!(next, InputEvent::Mouse {
                    x: 11, y: 6, button: 35, pressed: false, motion: true,
                });
            } else {
                assert_eq!(next, InputEvent::Key(decode_key_sequence(following)));
            }
            assert!(!has_pending_event());
        }
    }

    #[test]
    fn sgr_mouse_press_uses_zero_based_coordinates() {
        let ev = parse_sgr_mouse(b"\x1b[<0;12;7M").unwrap();
        assert_eq!(ev.button, MouseButton::Left);
        assert_eq!(ev.action, MouseAction::Press);
        assert_eq!(ev.col, 11);
        assert_eq!(ev.row, 6);
    }

    #[test]
    fn key_decoder_maps_special_sequences() {
        assert_eq!(decode_key_sequence(b"\x1b[17~"), "<F6>");
        assert_eq!(decode_key_sequence(b"\x1b[23~"), "<F11>");
        assert_eq!(decode_key_sequence(b"\x13"), "<C-s>");
        assert_eq!(decode_key_sequence(b"\x1b[B"), "<Down>");
    }
}
