use std::collections::HashMap;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct NvimProcess {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
}

impl NvimProcess {
    pub fn spawn(environ: &HashMap<String, String>) -> Result<Self, String> {
        Self::spawn_with_args(environ, &[])
    }

    pub fn spawn_with_args(
        environ: &HashMap<String, String>,
        extra_args: &[String],
    ) -> Result<Self, String> {
        let mut cmd = Command::new("nvim");
        cmd.args([
            "--clean",
            "--cmd",
            "set shortmess+=I",
            "--embed",
            "--headless",
        ])
        .args(extra_args)
        .env_clear()
        .envs(environ)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                "NvimNotFound".to_string()
            } else {
                format!("SpawnFailed: {}", e)
            }
        })?;

        let stdin = child.stdin.take().ok_or("StdinPipeFailed")?;
        let stdout = child.stdout.take().ok_or("StdoutPipeFailed")?;

        set_nonblocking(stdin.as_raw_fd());
        set_nonblocking(stdout.as_raw_fd());

        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    pub fn stdin_fd(&self) -> RawFd {
        self.stdin.as_raw_fd()
    }

    pub fn stdout_fd(&self) -> RawFd {
        self.stdout.as_raw_fd()
    }
}

fn set_nonblocking(fd: RawFd) {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        if flags >= 0 {
            let _ = libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
}

impl Drop for NvimProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
