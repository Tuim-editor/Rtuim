# Architecture

Tuim is a native Rust terminal workspace and IDE application with an embedded
Neovim engine. For product behavior and shortcuts, see the [usage guide](usage.md).

## Processes and ownership

`src/main.rs` initializes terminal capabilities, prepares the isolated runtime
directory (`~/.local/share/tuim`), and spawns an embedded Neovim process through
`src/nvim/process.rs` using `NVIM_APPNAME=tuim` and `--clean`. Stdin and stdout
are piped for MessagePack-RPC communication, while stderr inherits the outer terminal
(`Stdio::inherit()`):

- The Neovim process loads `src/nvim/tuim_init.lua` and owns file buffers,
  text editing, treesitter syntax trees, undo history, diagnostics, language
  servers, and Neovim plugins.
- The integrated bottom drawer and vertical split terminals run as independent
  native Rust PTY sessions (`src/tui/shell_terminal.rs`) powered by `portable_pty`
  and parsed using `vt100`. They execute the user's shell directly without
  routing through Neovim or its terminal modes.

The Rust frontend owns terminal raw mode, event dispatch, workspace layout,
focus routing, native modal dialogs, and differential cell rendering. `src/tui/app.rs`
maintains application state, `src/tui/events.rs` routes input and commands, and
`src/tui/views.rs` composites the editor grids, terminal cells, and native controls.

## Event loop and multiplexing

The event loop in `src/main.rs` multiplexes asynchronous event sources using
`libc::poll`:

1. **Terminal Stdin:** Raw input bytes decoded into keys, mouse coordinates, and
   bracketed paste sequences by `src/tui/input.rs`.
2. **Signals:** Handlers set resize/quit flags and wake the loop through a
   self-pipe for `SIGWINCH`, `SIGTERM` and `SIGHUP`.
3. **Neovim RPC Transport:** Stdout and stdin pipes connected to the embedded
   Neovim instance, processed via MessagePack-RPC (`src/nvim/rpc.rs`).
4. **Shell Terminal PTY:** Master PTY file descriptors for both bottom and
   vertical terminals, tracking live child output and scrollback.
5. **Background Task Runner:** Dedicated worker thread (`src/task_runner.rs`)
   emitting Git snapshot updates across an `mpsc` channel and notification pipe.
   (Extension catalog queries via `store_search.py` run synchronously in the shop).

Each loop iteration drains pending RPC notifications, processes terminal PTY
output, dispatches user input, executes scheduled widget actions, and performs
a bounded differential paint flush.

## RPC and rendering

- `src/nvim/rpc.rs` implements asynchronous and synchronous MessagePack-RPC using
  `rmpv`. Neovim is attached via `nvim_ui_attach` with `ext_linegrid` and `rgb: true`.
- `src/nvim/ui_protocol.rs` decodes Neovim linegrid UI events (`grid_line`, `grid_cursor_goto`,
  `grid_scroll`, `default_colors_set`, `hl_attr_define`, `mode_info_set`).
- `src/tui/invalidation.rs` tracks layout, cursor and composition damage for the
  chrome, sidebar, editor, drawer and overlay regions.
- `src/tui/renderer.rs` implements double-buffered rendering. It maintains
  previous and next cell grids, skips unchanged cells, batches identical attribute
  runs, and emits optimized ANSI escapes to the outer terminal.

Native dialogs (Settings, Extension Shop, Mason, Lazy, Git Detailed, Orchestrator,
Bug Report) share geometry and hit-test primitives in `src/tui/widgets/primitives.rs`.
The Bug Report widget runs curl directly as a tracked child process without shell
interpolation, reaping background child processes and deleting private temporary files on drop.

## Configuration and plugins

Application settings are persisted in Tuim's isolated data directory as JSON.
Modes (Normal, IDE, Zen) alter input routing and footer chrome; IDE mode installs
modeless text editing keymaps onto editable Neovim file buffers.

`src/nvim/store_search.py` is embedded into the Rust executable and extracted
to Tuim's runtime directory. The Extension Shop invokes it for catalog lookups,
offline inventory scanning, and plugin lifecycle changes.

Plugin files and configuration stay completely isolated from the host system:

| File or directory in Tuim's data directory | Purpose |
| --- | --- |
| `settings.json` | Native application preferences |
| `user_plugins.json` | Marketplace additions and installed status |
| `plugin_states.json` | Desired enabled, disabled, or removed state |
| `plugin_inventory.json` | Generated bundled and runtime inventory |
| `plugin_configs/` | User Lua specification overrides |
| `lazy/` | Cloned plugin git repositories |
| `site/` | Treesitter parsers and queries |
| `tools/` | Private build and parser binaries |

Neovim applies plugin state during initialization. Disabled plugins remain on
disk; uninstalled plugins are cleaned up safely. User configuration errors do
not crash Tuim; launching with `TUIM_DISABLE_PLUGINS=1 tuim` activates recovery
mode to repair invalid configs.

## Storage and failure boundaries

Tuim isolates its state under standard XDG paths (`~/.local/share/tuim`,
`~/.config/tuim`, `~/.cache/tuim`, `~/.local/state/tuim`). It does not load the user’s normal Neovim init. The optional System theme can
inspect desktop/installed theme assets without loading that configuration.

Terminal raw mode is restored on clean exit or handled termination (`SIGTERM`,
`SIGHUP`) via `Drop` handlers; unhandled panics or forced termination (`SIGKILL`)
cannot guarantee terminal state restoration. RPC errors and background worker
failures trigger native transient notices (`tuim_native_notice`) rather than
terminating the editor session.

## Validation

- `cargo test --locked` runs unit tests covering key sequence decoding,
  SGR mouse coordinate mapping, terminal emulator scrollback and device queries,
  layout constraints, default theme contrast, and settings serialization.
- Python and shell test suites under `tests/` validate real PTY sessions, Neovim
  editor interactions, onboarding flows, plugin lifecycle, and launcher binaries.
