use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

#[derive(Default)]
pub struct Counter {
    pub count: AtomicU64,
    pub total_ns: AtomicU64,
    pub max_ns: AtomicU64,
}

impl Counter {
    pub fn record(&self, elapsed_ns: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        let mut current = self.max_ns.load(Ordering::Relaxed);
        while elapsed_ns > current {
            match self.max_ns.compare_exchange_weak(
                current,
                elapsed_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }
}

#[derive(Serialize)]
struct CounterJson {
    count: u64,
    total_ns: u64,
    max_ns: u64,
}

impl From<&Counter> for CounterJson {
    fn from(c: &Counter) -> Self {
        Self {
            count: c.count.load(Ordering::Relaxed),
            total_ns: c.total_ns.load(Ordering::Relaxed),
            max_ns: c.max_ns.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub struct Metrics {
    pub enabled: AtomicBool,
    pub poll_wakeup: Counter,
    pub input_decode: Counter,
    pub rpc_decode: Counter,
    pub state_update: Counter,
    pub layout: Counter,
    pub composition: Counter,
    pub flush: Counter,
    pub writer_flush: Counter,
    pub blocking_io_log: Counter,
    pub blocking_io_settings: Counter,
    pub emitted_bytes: AtomicU64,
    pub editor_resize_requests: AtomicU64,
    pub terminal_resize_requests: AtomicU64,
}

pub static GLOBAL: Metrics = Metrics {
    enabled: AtomicBool::new(false),
    poll_wakeup: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    input_decode: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    rpc_decode: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    state_update: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    layout: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    composition: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    flush: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    writer_flush: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    blocking_io_log: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    blocking_io_settings: Counter {
        count: AtomicU64::new(0),
        total_ns: AtomicU64::new(0),
        max_ns: AtomicU64::new(0),
    },
    emitted_bytes: AtomicU64::new(0),
    editor_resize_requests: AtomicU64::new(0),
    terminal_resize_requests: AtomicU64::new(0),
};

pub struct ScopedTimer<'a> {
    counter: Option<&'a Counter>,
    start: Instant,
}

impl<'a> ScopedTimer<'a> {
    pub fn start(counter: &'a Counter) -> Self {
        if GLOBAL.enabled.load(Ordering::Relaxed) {
            Self {
                counter: Some(counter),
                start: Instant::now(),
            }
        } else {
            Self {
                counter: None,
                start: Instant::now(),
            }
        }
    }

    pub fn stop(mut self) {
        if let Some(c) = self.counter.take() {
            c.record(self.start.elapsed().as_nanos() as u64);
        }
    }
}

impl<'a> Drop for ScopedTimer<'a> {
    fn drop(&mut self) {
        if let Some(c) = self.counter.take() {
            c.record(self.start.elapsed().as_nanos() as u64);
        }
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc_input_events(&self) {
        self.input_decode.record(1);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn to_diagnostics_json(
        &self,
        version: &str,
        os: &str,
        arch: &str,
        mode: &str,
        theme: &str,
        true_color: bool,
        mouse: bool,
        bracketed_paste: bool,
        nerd_fonts: bool,
        cols: u16,
        rows: u16,
    ) -> String {
        serde_json::json!({
            "version": version,
            "os": os,
            "arch": arch,
            "mode": mode,
            "theme": theme,
            "capabilities": {
                "true_color": true_color,
                "mouse": mouse,
                "bracketed_paste": bracketed_paste,
                "nerd_fonts": nerd_fonts,
            },
            "dimensions": {
                "cols": cols,
                "rows": rows,
            },
            "emitted_bytes": self.emitted_bytes.load(Ordering::Relaxed),
        })
        .to_string()
    }

    pub fn export_json(&self) -> String {
        #[derive(Serialize)]
        struct DiagnosticsExport {
            poll_wakeup: CounterJson,
            input_decode: CounterJson,
            rpc_decode: CounterJson,
            state_update: CounterJson,
            layout: CounterJson,
            composition: CounterJson,
            flush: CounterJson,
            writer_flush: CounterJson,
            blocking_io_log: CounterJson,
            blocking_io_settings: CounterJson,
            emitted_bytes: u64,
            editor_resize_requests: u64,
            terminal_resize_requests: u64,
        }

        let export = DiagnosticsExport {
            poll_wakeup: (&self.poll_wakeup).into(),
            input_decode: (&self.input_decode).into(),
            rpc_decode: (&self.rpc_decode).into(),
            state_update: (&self.state_update).into(),
            layout: (&self.layout).into(),
            composition: (&self.composition).into(),
            flush: (&self.flush).into(),
            writer_flush: (&self.writer_flush).into(),
            blocking_io_log: (&self.blocking_io_log).into(),
            blocking_io_settings: (&self.blocking_io_settings).into(),
            emitted_bytes: self.emitted_bytes.load(Ordering::Relaxed),
            editor_resize_requests: self.editor_resize_requests.load(Ordering::Relaxed),
            terminal_resize_requests: self.terminal_resize_requests.load(Ordering::Relaxed),
        };
        serde_json::to_string_pretty(&export).unwrap_or_else(|_| "{}".to_string())
    }
}
