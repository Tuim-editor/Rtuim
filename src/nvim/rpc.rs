use crate::metrics::{ScopedTimer, GLOBAL};
use crate::nvim::process::NvimProcess;
use rmpv::Value;
use std::collections::{HashMap, VecDeque};
use std::io::{self, Cursor};
use std::os::unix::io::RawFd;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncTag {
    ZenHandoff,
    Reload,
    QuitConfirmation,
    TerminalAdded,
    TerminalRemoved,
    TerminalPaste,
    VerticalTerminalPaste,
}

#[derive(Clone, Debug)]
pub enum RpcMessage {
    Notification {
        method: String,
        params: Vec<Value>,
    },
    Response {
        msgid: u32,
        tag: Option<AsyncTag>,
        error: Value,
        result: Value,
    },
}

pub struct RpcClient {
    pub process: NvimProcess,
    next_msg_id: u32,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    pub incoming_queue: VecDeque<RpcMessage>,
    pending_async: HashMap<u32, AsyncTag>,
    eof_reached: bool,
}

impl RpcClient {
    pub fn new(process: NvimProcess) -> Self {
        Self {
            process,
            next_msg_id: 1,
            read_buf: Vec::with_capacity(65536),
            write_buf: Vec::with_capacity(16384),
            incoming_queue: VecDeque::new(),
            pending_async: HashMap::new(),
            eof_reached: false,
        }
    }

    pub fn stdout_fd(&self) -> RawFd {
        self.process.stdout_fd()
    }

    pub fn stdin_fd(&self) -> RawFd {
        self.process.stdin_fd()
    }

    pub fn wants_write(&self) -> bool {
        !self.write_buf.is_empty()
    }

    pub fn has_pending_work(&self) -> bool {
        !self.incoming_queue.is_empty()
    }

    pub fn next_message(&mut self) -> Option<RpcMessage> {
        self.incoming_queue.pop_front()
    }

    pub fn flush_writes(&mut self) -> Result<(), String> {
        let fd = self.stdin_fd();
        while !self.write_buf.is_empty() {
            let rc = unsafe {
                libc::write(
                    fd,
                    self.write_buf.as_ptr() as *const libc::c_void,
                    self.write_buf.len(),
                )
            };
            if rc > 0 {
                self.write_buf.drain(..rc as usize);
            } else if rc < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if err.kind() == io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(format!("RPC write failed: {}", err));
            } else {
                break;
            }
        }
        Ok(())
    }

    fn flush_writes_blocking(&mut self) -> Result<(), String> {
        let fd = self.stdin_fd();
        while !self.write_buf.is_empty() {
            self.flush_writes()?;
            if !self.write_buf.is_empty() {
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLOUT,
                    revents: 0,
                };
                let _ = unsafe { libc::poll(&mut pfd, 1, 50) };
            }
        }
        Ok(())
    }

    fn queue_message(&mut self, msg: &Value) -> Result<(), String> {
        rmpv::encode::write_value(&mut self.write_buf, msg)
            .map_err(|e| format!("Msgpack encode error: {}", e))?;
        self.flush_writes()
    }

    pub fn notify(&mut self, method: &str, params: Vec<Value>) -> Result<(), String> {
        let msg = Value::Array(vec![
            Value::from(2),
            Value::from(method),
            Value::Array(params),
        ]);
        self.queue_message(&msg)
    }

    pub fn has_pending_tag(&self, tag: AsyncTag) -> bool {
        self.pending_async.values().any(|t| *t == tag)
    }

    pub fn request_async(
        &mut self,
        method: &str,
        params: Vec<Value>,
        tag: AsyncTag,
    ) -> Result<u32, String> {
        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        self.pending_async.insert(msg_id, tag);
        let msg = Value::Array(vec![
            Value::from(0),
            Value::from(msg_id),
            Value::from(method),
            Value::Array(params),
        ]);
        self.queue_message(&msg)?;
        Ok(msg_id)
    }

    pub fn call(&mut self, method: &str, params: Vec<Value>) -> Result<Value, String> {
        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        let msg = Value::Array(vec![
            Value::from(0),
            Value::from(msg_id),
            Value::from(method),
            Value::Array(params),
        ]);
        rmpv::encode::write_value(&mut self.write_buf, &msg)
            .map_err(|e| format!("Msgpack encode error: {}", e))?;
        self.flush_writes_blocking()?;

        let fd = self.stdout_fd();
        loop {
            while let Some(frame) = self.try_decode_frame() {
                if let Some(res) = self.dispatch_frame_for_sync(frame, msg_id)? {
                    return Ok(res);
                }
            }
            if self.eof_reached {
                return Err("EndOfStream".to_string());
            }
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pfd, 1, 30000) };
            if rc < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(format!("Poll error: {}", err));
            }
            if rc == 0 {
                return Err(format!("Timeout waiting for RPC response to {}", method));
            }
            self.read_available_bytes()?;
        }
    }

    fn read_available_bytes(&mut self) -> Result<(), String> {
        let fd = self.stdout_fd();
        let mut temp = [0u8; 32768];
        loop {
            let n = unsafe { libc::read(fd, temp.as_mut_ptr() as *mut libc::c_void, temp.len()) };
            if n > 0 {
                self.read_buf.extend_from_slice(&temp[..n as usize]);
            } else if n == 0 {
                self.eof_reached = true;
                break;
            } else {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if err.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                return Err(format!("Read error: {}", err));
            }
        }
        Ok(())
    }

    fn try_decode_frame(&mut self) -> Option<Value> {
        if self.read_buf.is_empty() {
            return None;
        }
        let _timer = ScopedTimer::start(&GLOBAL.rpc_decode);
        let mut cursor = Cursor::new(&self.read_buf[..]);
        match rmpv::decode::read_value(&mut cursor) {
            Ok(val) => {
                let pos = cursor.position() as usize;
                self.read_buf.drain(..pos);
                Some(val)
            }
            Err(_) => None,
        }
    }

    fn dispatch_frame_for_sync(
        &mut self,
        frame: Value,
        wait_msg_id: u32,
    ) -> Result<Option<Value>, String> {
        let Value::Array(mut arr) = frame else {
            return Ok(None);
        };
        if arr.is_empty() {
            return Ok(None);
        }
        let msg_type = arr[0].as_u64().unwrap_or(99);
        match msg_type {
            0 if arr.len() >= 4 => {
                let req_id = arr[1].clone();
                let resp = Value::Array(vec![Value::from(1), req_id, Value::Nil, Value::Nil]);
                let _ = self.queue_message(&resp);
                Ok(None)
            }
            1 if arr.len() >= 4 => {
                let resp_id = arr[1].as_u64().unwrap_or(0) as u32;
                let result = arr.pop().unwrap_or(Value::Nil);
                let error = arr.pop().unwrap_or(Value::Nil);
                if resp_id == wait_msg_id {
                    if !error.is_nil() {
                        return Err(format!("RPC error: {:?}", error));
                    }
                    return Ok(Some(result));
                }
                let tag = self.pending_async.remove(&resp_id);
                self.incoming_queue.push_back(RpcMessage::Response {
                    msgid: resp_id,
                    tag,
                    error,
                    result,
                });
                Ok(None)
            }
            2 if arr.len() >= 3 => {
                let params_val = arr.pop().unwrap_or(Value::Nil);
                let params = match params_val {
                    Value::Array(v) => v,
                    other => vec![other],
                };
                let method = arr[1].as_str().unwrap_or("").to_string();
                self.incoming_queue
                    .push_back(RpcMessage::Notification { method, params });
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub fn pump_incoming(&mut self) -> Result<bool, String> {
        self.read_available_bytes()?;
        while let Some(frame) = self.try_decode_frame() {
            let _ = self.dispatch_frame_for_sync(frame, u32::MAX)?;
        }
        if self.eof_reached && self.read_buf.is_empty() && self.incoming_queue.is_empty() {
            return Ok(false);
        }
        Ok(true)
    }
}
