use crate::git_snapshot::{collect_snapshot, GitSnapshot};
use std::os::unix::io::RawFd;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

pub struct TaskRunner {
    tx: Option<Sender<String>>,
    rx: Receiver<GitSnapshot>,
    notify_read_fd: RawFd,
    notify_write_fd: RawFd,
    worker: Option<JoinHandle<()>>,
}

impl Default for TaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRunner {
    pub fn new() -> Self {
        let mut fds = [-1i32; 2];
        let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if rc == 0 {
            unsafe {
                for &fd in &fds {
                    let flags = libc::fcntl(fd, libc::F_GETFL, 0);
                    if flags >= 0 {
                        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                    }
                }
            }
        }

        let notify_read_fd = fds[0];
        let notify_write_fd = fds[1];
        let (cmd_tx, cmd_rx) = mpsc::channel::<String>();
        let (res_tx, res_rx) = mpsc::channel::<GitSnapshot>();

        let worker = thread::spawn(move || {
            while let Ok(mut cwd) = cmd_rx.recv() {
                while let Ok(newer) = cmd_rx.try_recv() {
                    cwd = newer;
                }
                let snapshot = collect_snapshot(&cwd);
                if res_tx.send(snapshot).is_ok() && notify_write_fd >= 0 {
                    let b = [b'G'];
                    unsafe {
                        let _ = libc::write(notify_write_fd, b.as_ptr() as *const _, 1);
                    }
                }
            }
        });

        Self {
            tx: Some(cmd_tx),
            rx: res_rx,
            notify_read_fd,
            notify_write_fd,
            worker: Some(worker),
        }
    }

    pub fn notifier_fd(&self) -> RawFd {
        self.notify_read_fd
    }

    pub fn request_git_snapshot(&self, cwd: &str) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(cwd.to_string());
        }
    }

    pub fn poll_results(&self) -> Vec<GitSnapshot> {
        if self.notify_read_fd >= 0 {
            let mut buf = [0u8; 64];
            unsafe {
                while libc::read(self.notify_read_fd, buf.as_mut_ptr() as *mut _, buf.len()) > 0 {}
            }
        }
        let mut out = Vec::new();
        while let Ok(snap) = self.rx.try_recv() {
            out.push(snap);
        }
        out
    }
}

impl Drop for TaskRunner {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        unsafe {
            if self.notify_read_fd >= 0 {
                libc::close(self.notify_read_fd);
            }
            if self.notify_write_fd >= 0 {
                libc::close(self.notify_write_fd);
            }
        }
    }
}
