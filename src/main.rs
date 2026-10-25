use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rmpv::Value;

use tuim::metrics::GLOBAL;
use tuim::nvim::helpers::handle_rpc_message;
use tuim::nvim::process::NvimProcess;
use tuim::nvim::rpc::{RpcClient, RpcMessage};
use tuim::tui::app::App;
use tuim::tui::capabilities::Capabilities;
use tuim::tui::events;
use tuim::tui::input::{
    self, handle_quit_signal, handle_sigwinch, QUIT_SIGNAL_RECEIVED, SIGWINCH_PIPE_WRITE_FD,
    SIGWINCH_RECEIVED,
};
use tuim::tui::terminal::Terminal;
use tuim::tui::views;

const EMBED_TUIM_INIT_LUA: &str = include_str!("nvim/tuim_init.lua");
const EMBED_TUIM_SETTINGS_LUA: &str = include_str!("nvim/tuim_settings.lua");
const EMBED_DASHBOARD_LUA: &str = include_str!("nvim/dashboard.lua");
const EMBED_STORE_SEARCH_PY: &str = include_str!("nvim/store_search.py");
const EMBED_SOFTWARE_UPDATE_SH: &str = include_str!("software_update.sh");

fn resolve_dirs() -> (String, String) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/.local/share", home));
    let data_dir = format!("{}/tuim", data_home);
    (data_dir, home)
}

fn write_embedded_runtime(data_dir: &str) -> Result<String, String> {
    fs::create_dir_all(data_dir).map_err(|e| format!("Failed to create {}: {}", data_dir, e))?;
    let lua_dir = format!("{}/lua", data_dir);
    fs::create_dir_all(&lua_dir).map_err(|e| format!("Failed to create {}: {}", lua_dir, e))?;

    let files = [
        (format!("{}/tuim_init.lua", data_dir), EMBED_TUIM_INIT_LUA, false),
        (
            format!("{}/lua/tuim_settings.lua", data_dir),
            EMBED_TUIM_SETTINGS_LUA,
            false,
        ),
        (
            format!("{}/tuim_settings.lua", data_dir),
            EMBED_TUIM_SETTINGS_LUA,
            false,
        ),
        (
            format!("{}/lua/dashboard.lua", data_dir),
            EMBED_DASHBOARD_LUA,
            false,
        ),
        (
            format!("{}/dashboard.lua", data_dir),
            EMBED_DASHBOARD_LUA,
            false,
        ),
        (
            format!("{}/store_search.py", data_dir),
            EMBED_STORE_SEARCH_PY,
            true,
        ),
        (
            format!("{}/software_update.sh", data_dir),
            EMBED_SOFTWARE_UPDATE_SH,
            true,
        ),
    ];

    for (path, content, executable) in files {
        let needs_write = match fs::read_to_string(&path) {
            Ok(existing) => existing != content,
            Err(_) => true,
        };
        if needs_write {
            fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", path, e))?;
        }
        if executable {
            if let Ok(meta) = fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&path, perms);
            }
        }
    }

    Ok(format!("{}/tuim_init.lua", data_dir))
}

fn print_help() {
    println!(
        "Tuim {} — Terminal Workspace & Modal/Modeless IDE Editor\n\n\
         USAGE:\n  \
         tuim [OPTIONS] [FILES...]\n\n\
         OPTIONS:\n  \
         -h, --help          Print this help information\n  \
         -V, --version       Print version information\n  \
         --diagnostics       Emit runtime performance diagnostics JSON on exit",
        env!("CARGO_PKG_VERSION")
    );
}

fn install_signal_handlers() -> (i32, i32) {
    let mut pipe_fds = [-1i32; 2];
    unsafe {
        if libc::pipe(pipe_fds.as_mut_ptr()) == 0 {
            for &fd in &pipe_fds {
                let flags = libc::fcntl(fd, libc::F_GETFL, 0);
                if flags >= 0 {
                    libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
            SIGWINCH_PIPE_WRITE_FD.store(pipe_fds[1], Ordering::Relaxed);
        }

        let mut sa_winch: libc::sigaction = std::mem::zeroed();
        sa_winch.sa_sigaction = handle_sigwinch as *const () as usize;
        libc::sigemptyset(&mut sa_winch.sa_mask);
        sa_winch.sa_flags = libc::SA_RESTART;
        libc::sigaction(libc::SIGWINCH, &sa_winch, std::ptr::null_mut());

        let mut sa_quit: libc::sigaction = std::mem::zeroed();
        sa_quit.sa_sigaction = handle_quit_signal as *const () as usize;
        libc::sigemptyset(&mut sa_quit.sa_mask);
        sa_quit.sa_flags = 0;
        libc::sigaction(libc::SIGTERM, &sa_quit, std::ptr::null_mut());
        libc::sigaction(libc::SIGHUP, &sa_quit, std::ptr::null_mut());
    }
    (pipe_fds[0], pipe_fds[1])
}

fn attach_nvim_ui(rpc: &mut RpcClient, width: u16, height: u16) -> Result<(), String> {
    let opts = Value::Map(vec![
        (Value::from("rgb"), Value::from(true)),
        (Value::from("ext_linegrid"), Value::from(true)),
    ]);
    rpc.call(
        "nvim_ui_attach",
        vec![
            Value::from(width.max(1) as i64),
            Value::from(height.max(1) as i64),
            opts,
        ],
    )?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut files_to_open: Vec<String> = Vec::new();
    let mut diagnostics_enabled = false;
    let mut passthrough = false;

    for arg in &args {
        if passthrough {
            files_to_open.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            "--" => passthrough = true,
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-V" | "--version" => {
                println!("tuim {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--diagnostics" => {
                diagnostics_enabled = true;
            }
            s if s.starts_with('-') => {
                eprintln!("Tuim could not start: unknown option '{}'\n", s);
                print_help();
                std::process::exit(2);
            }
            _ => {
                files_to_open.push(arg.clone());
            }
        }
    }

    if diagnostics_enabled || std::env::var("TUIM_DIAGNOSTICS").ok().as_deref() == Some("1") {
        GLOBAL.enabled.store(true, Ordering::Relaxed);
    }

    let env_map: HashMap<String, String> = std::env::vars().collect();
    let capabilities = Capabilities::detect(&env_map);
    let mut nvim_env_map = env_map.clone();
    nvim_env_map.insert("NVIM_APPNAME".to_string(), "tuim".to_string());

    let mut term = match Terminal::init(capabilities) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Tuim could not start: {}", e);
            std::process::exit(1);
        }
    };

    let (sig_read_fd, sig_write_fd) = install_signal_handlers();
    let (data_dir, home_dir) = resolve_dirs();

    let init_lua_path = match write_embedded_runtime(&data_dir) {
        Ok(p) => p,
        Err(e) => {
            term.cleanup();
            let _ = writeln!(term.writer, "Tuim could not start: {}", e);
            eprintln!("Tuim could not start: {}", e);
            std::process::exit(1);
        }
    };

    loop {
        let (cols, rows) = term.get_size().unwrap_or((100, 30));
        let cols = cols.max(20);
        let rows = rows.max(6);

        let editor_proc = match NvimProcess::spawn(&nvim_env_map) {
            Ok(p) => p,
            Err(e) => {
                term.cleanup();
                let _ = writeln!(
                    term.writer,
                    "Tuim could not start: failed to launch embedded Neovim ({}). Ensure `nvim` is installed and in PATH.",
                    e
                );
                eprintln!(
                    "Tuim could not start: failed to launch embedded Neovim ({}). Ensure `nvim` is installed and in PATH.",
                    e
                );
                std::process::exit(1);
            }
        };

        let editor_rpc = RpcClient::new(editor_proc);

        let mut app = App::new(
            cols,
            rows,
            editor_rpc,
            capabilities,
            data_dir.clone(),
            home_dir.clone(),
        );

        let layout = app.current_layout();
        if let Err(e) = attach_nvim_ui(&mut app.editor_rpc, layout.editor.w, layout.editor.h) {
            term.cleanup();
            let _ = writeln!(term.writer, "Tuim could not start: {}", e);
            eprintln!("Tuim could not start: {}", e);
            std::process::exit(1);
        }
        let _ = app
            .editor_rpc
            .call("nvim_command", vec![Value::from("set laststatus=0")]);
        let _ = app.editor_rpc.call(
            "nvim_command",
            vec![Value::from(
                "autocmd BufWritePost * let b:tuim_session_saved = 1",
            )],
        );

        // Source the editor runtime synchronously.
        let escaped_data = data_dir.replace('\'', "''");
        let file_vals: Vec<Value> = files_to_open
            .iter()
            .map(|s| Value::from(s.as_str()))
            .collect();
        let editor_boot_lua = format!(
            "local files = select(1, ...)\n\
             package.path = '{0}/lua/?.lua;' .. package.path\n\
             dofile('{0}/tuim_init.lua')\n\
             if #files > 0 then\n\
                 local orig_alpha = _G.tuim_alpha_start\n\
                 _G.tuim_alpha_start = function()\n\
                     _G.tuim_alpha_start = orig_alpha\n\
                 end\n\
                 while #vim.api.nvim_win_get_config(0).relative > 0 do vim.cmd('close') end\n\
                 vim.cmd('edit ' .. vim.fn.fnameescape(files[1]))\n\
                 for i = 2, #files do\n\
                     vim.cmd('badd ' .. vim.fn.fnameescape(files[i]))\n\
                 end\n\
             end\n\
             vim.schedule(function()\n\
                 vim.schedule(function()\n\
                     vim.schedule(function()\n\
                         pcall(vim.rpcnotify, 1, 'tuim_startup_done')\n\
                     end)\n\
                 end)\n\
             end)",
            escaped_data
        );
        let _ = app.editor_rpc.call(
            "nvim_exec_lua",
            vec![
                Value::from(editor_boot_lua),
                Value::Array(vec![Value::Array(file_vals)]),
            ],
        );

        events::sync_layout(&mut app);

        let startup_deadline = Instant::now() + Duration::from_millis(500);
        let mut startup_done = false;
        while !startup_done && Instant::now() < startup_deadline {
            let _ = app.editor_rpc.pump_incoming();
            while let Some(msg) = app.editor_rpc.next_message() {
                if let RpcMessage::Notification { ref method, .. } = msg {
                    if method == "tuim_startup_done" {
                        startup_done = true;
                    }
                }
                handle_rpc_message(&mut app, msg);
            }
            if !startup_done {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        // Initial render
        views::draw_workspace(&mut app);
        let _ = app.renderer.flush(&mut term.writer);
        let _ = app.invalidations.consume_paint();

        // Main Poll Loop
        loop {
            if QUIT_SIGNAL_RECEIVED.load(Ordering::Relaxed) || app.should_quit {
                break;
            }

            if SIGWINCH_RECEIVED.swap(false, Ordering::Relaxed) {
                if let Some((w, h)) = term.get_size() {
                    if w > 0 && h > 0 {
                        events::handle_event(&mut app, input::InputEvent::Resize(w, h));
                    }
                }
            }

            // Drain any already-buffered RPC notifications
            let mut had_rpc = false;
            while let Some(msg) = app.editor_rpc.next_message() {
                handle_rpc_message(&mut app, msg);
                had_rpc = true;
            }
            if app.shell_terminal.pump() {
                app.invalidations.mark_terminal_grid();
            }
            if app.vertical_terminal.pump() {
                app.invalidations.mark_terminal_grid();
            }
            if let Some(error) = app.vertical_terminal.take_error() {
                app.set_notice("error", &error);
            }
            if let Some(error) = app.shell_terminal.take_error() {
                app.set_notice("error", &error);
            }

            if app.poll_background_tasks() {
                had_rpc = true;
            }

            if app.restart_requested || app.zen_handoff_requested {
                break;
            }

            let _ = app.editor_rpc.flush_writes();
            app.shell_terminal.flush_input();
            app.vertical_terminal.flush_input();

            if app.invalidations.has_damage() || had_rpc {
                views::draw_workspace(&mut app);
                let _ = app.renderer.flush(&mut term.writer);
                let _ = app.invalidations.consume_paint();
            }

            let timeout_ms = if input::has_pending_event()
                || app.editor_rpc.has_pending_work()
            {
                0
            } else if app.notice.is_some() || app.bug_report.is_open {
                100
            } else {
                -1
            };

            let mut poll_fds = [
                libc::pollfd {
                    fd: term.tty_fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: app.editor_rpc.stdout_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: app.shell_terminal.fd(),
                    events: libc::POLLIN | if app.shell_terminal.wants_write() { libc::POLLOUT } else { 0 },
                    revents: 0,
                },
                libc::pollfd {
                    fd: app.task_runner.notifier_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: sig_read_fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: app.vertical_terminal.fd(),
                    events: libc::POLLIN | if app.vertical_terminal.wants_write() { libc::POLLOUT } else { 0 },
                    revents: 0,
                },
            ];

            let rc = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as _, timeout_ms) };
            GLOBAL.poll_wakeup.record(1);

            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }

            // Signal pipe wakeup
            if (poll_fds[4].revents & libc::POLLIN) != 0 {
                let mut sig_buf = [0u8; 32];
                unsafe {
                    while libc::read(sig_read_fd, sig_buf.as_mut_ptr() as *mut _, sig_buf.len()) > 0
                    {
                    }
                }
            }

            // Editor RPC stdout
            if (poll_fds[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0 {
                match app.editor_rpc.pump_incoming() {
                    Ok(true) => {}
                    _ => {
                        app.should_quit = true;
                        break;
                    }
                }
            }

            // Shell output and queued input share the PTY descriptor.
            if (poll_fds[2].revents & libc::POLLOUT) != 0 {
                app.shell_terminal.flush_input();
            }
            if (poll_fds[2].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0 {
                if app.shell_terminal.pump() { app.invalidations.mark_terminal_grid(); }
            }

            if (poll_fds[5].revents & libc::POLLOUT) != 0 {
                app.vertical_terminal.flush_input();
            }
            if (poll_fds[5].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0 {
                if app.vertical_terminal.pump() { app.invalidations.mark_terminal_grid(); }
            }

            // TTY input
            if (poll_fds[0].revents & libc::POLLIN) != 0 || input::has_pending_event() {
                loop {
                    let raw_ev = input::read_event(term.tty_fd);
                    if raw_ev == input::Event::None {
                        break;
                    }
                    if let Some(iev) = raw_ev.to_input_event() {
                        events::handle_event(&mut app, iev);
                    }
                    if !input::has_pending_event() {
                        break;
                    }
                }
            }
        }

        if app.zen_handoff_requested {
            let handoff_file = app.zen_handoff_file.clone();
            let handoff_line = app.zen_handoff_line.max(1);
            term.cleanup();
            let mut cmd = Command::new("nvim");
            cmd.arg("-u").arg(&init_lua_path);
            cmd.arg(format!("+{}", handoff_line));
            if !handoff_file.is_empty() {
                cmd.arg(&handoff_file);
            }
            let _ = cmd.status();
            term = match Terminal::init(capabilities) {
                Ok(t) => t,
                Err(_) => break,
            };
            continue;
        }

        if app.restart_requested {
            continue;
        }

        break;
    }

    unsafe {
        if sig_read_fd >= 0 {
            libc::close(sig_read_fd);
        }
        if sig_write_fd >= 0 {
            libc::close(sig_write_fd);
        }
    }

    term.cleanup();

    if GLOBAL.enabled.load(Ordering::Relaxed) {
        let json = GLOBAL.export_json();
        let diag_path = format!("{}/diagnostics.json", data_dir);
        let _ = fs::write(&diag_path, &json);
    }
}
