use crate::tui::capabilities::Capabilities;
use std::ffi::CString;
use std::io::{self, Write};
use std::os::unix::io::RawFd;

pub struct FdWriter {
    pub fd: RawFd,
}

impl Write for FdWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut total = 0usize;
        while total < buf.len() {
            let rc = unsafe {
                libc::write(
                    self.fd,
                    buf[total..].as_ptr() as *const libc::c_void,
                    buf.len() - total,
                )
            };
            if rc > 0 {
                total += rc as usize;
            } else if rc < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            } else {
                break;
            }
        }
        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn is_pollable(fd: RawFd) -> bool {
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let rc = unsafe { libc::poll(&mut pfd, 1, 0) };
    if rc < 0 {
        return false;
    }
    (pfd.revents & libc::POLLNVAL) == 0
}

pub fn has_usable_terminal() -> bool {
    let c_path = CString::new("/dev/tty").unwrap();
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR) };
    if fd >= 0 {
        unsafe {
            libc::close(fd);
        }
        return true;
    }
    unsafe { libc::isatty(0) == 1 }
}

pub struct Terminal {
    pub orig_termios: libc::termios,
    pub tty_fd: RawFd,
    pub owns_tty_fd: bool,
    pub writer: FdWriter,
    pub mouse_enabled: bool,
    pub hover_mouse_enabled: bool,
    pub paste_enabled: bool,
    cleaned_up: bool,
}

impl Terminal {
    pub fn init(capabilities: Capabilities) -> Result<Self, String> {
        let c_path = CString::new("/dev/tty").unwrap();
        let mut opened_tty: Option<RawFd> = {
            let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR) };
            if fd >= 0 {
                Some(fd)
            } else {
                None
            }
        };

        if let Some(fd) = opened_tty {
            if !is_pollable(fd) {
                unsafe {
                    libc::close(fd);
                }
                opened_tty = None;
            }
        }

        if opened_tty.is_none() {
            let mut dummy: libc::termios = unsafe { std::mem::zeroed() };
            if unsafe { libc::tcgetattr(0, &mut dummy) } != 0
                || unsafe { libc::tcgetattr(1, &mut dummy) } != 0
                || !is_pollable(0)
            {
                return Err("TerminalUnavailable".to_string());
            }
        }

        let tty_fd = opened_tty.unwrap_or(0);
        let output_fd = opened_tty.unwrap_or(1);

        let mut orig: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(tty_fd, &mut orig) } != 0 {
            if let Some(fd) = opened_tty {
                unsafe {
                    libc::close(fd);
                }
            }
            return Err("TerminalUnavailable".to_string());
        }

        let mut raw = orig;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_iflag &= !(libc::IXON | libc::ICRNL);
        raw.c_oflag &= !(libc::OPOST);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;

        if unsafe { libc::tcsetattr(tty_fd, libc::TCSAFLUSH, &raw) } != 0 {
            if let Some(fd) = opened_tty {
                unsafe {
                    libc::close(fd);
                }
            }
            return Err("TerminalUnavailable".to_string());
        }

        let mut term = Self {
            orig_termios: orig,
            tty_fd,
            owns_tty_fd: opened_tty.is_some(),
            writer: FdWriter { fd: output_fd },
            mouse_enabled: capabilities.mouse,
            hover_mouse_enabled: false,
            paste_enabled: capabilities.bracketed_paste,
            cleaned_up: false,
        };

        let _ = term.writer.write_all(b"\x1b[?1049h\x1b[?25l");
        if term.mouse_enabled {
            let _ = term.writer.write_all(b"\x1b[?1002h\x1b[?1006h");
        }
        term.set_hover_mouse(true);
        if term.paste_enabled {
            let _ = term.writer.write_all(b"\x1b[?2004h");
        }

        Ok(term)
    }

    pub fn set_hover_mouse(&mut self, enabled: bool) {
        if !self.mouse_enabled || self.hover_mouse_enabled == enabled {
            return;
        }
        if enabled {
            let _ = self.writer.write_all(b"\x1b[?1003h");
        } else {
            let _ = self.writer.write_all(b"\x1b[?1003l\x1b[?1002h");
        }
        self.hover_mouse_enabled = enabled;
    }

    pub fn get_size(&self) -> Option<(u16, u16)> {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::ioctl(self.tty_fd, libc::TIOCGWINSZ, &mut ws) };
        if rc != 0 {
            return None;
        }
        Some((ws.ws_col, ws.ws_row))
    }

    pub fn cleanup(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;
        if self.paste_enabled {
            let _ = self.writer.write_all(b"\x1b[?2004l");
        }
        if self.mouse_enabled {
            let _ = self.writer.write_all(b"\x1b[?1003l\x1b[?1002l\x1b[?1006l");
        }
        let _ = self.writer.write_all(b"\x1b[?25h\x1b[?1049l");
        unsafe {
            let _ = libc::tcsetattr(self.tty_fd, libc::TCSAFLUSH, &self.orig_termios);
            if self.owns_tty_fd {
                let _ = libc::close(self.tty_fd);
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pollable_checks_valid_pipe() {
        let mut pipe = [0i32; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
        assert!(is_pollable(pipe[0]));
        unsafe {
            libc::close(pipe[0]);
            libc::close(pipe[1]);
        }
        assert!(!is_pollable(pipe[0]));
    }
}
