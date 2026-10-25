# Features Guide

Tuim provides a terminal workspace combining an embedded Neovim editing engine with native Rust TUI panels and integrated shell terminals.

---

## 1. Workspaces & Layout

- **Editor Panel:** The main editing area, attached to Neovim using the `ext_linegrid` UI protocol with 24-bit TrueColor.
- **Workspace Sidebar:** Houses Open Files, File Explorer, Git Changes, AI Assistants, Search, and Extension Shop. Automatically collapses below 40 columns.
- **Bottom Terminal Drawer:** Integrated shell terminal (`Ctrl+T`), running in a collapsible bottom drawer.
- **Vertical Terminal Split:** Side-by-side integrated terminal (`Space o Shift+T`).
- **Panel Resizing:** Left-click and drag dividers to resize, or use `Alt+Arrow` keys (`Alt+Left`/`Alt+Right` for vertical dividers, `Alt+Up`/`Alt+Down` for the bottom terminal drawer; visible Zen panes can resize).
- **Editing Modes:**
  - **Normal:** Standard Vim modal editing. `F6` cycles forward; `Shift+F6` cycles in reverse. `Ctrl+E` or `Space e` toggles the Workspace Explorer.
  - **IDE:** Modeless editing with standard `Ctrl+S`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+C`, `Ctrl+V`, `Ctrl+F`, `Ctrl+H`, and `Shift+Arrows` selection.
  - **Zen (`F11`):** Hides sidebar and chrome, focusing the editor.

---

## 2. Integrated Shell Terminals

- **Native Rust PTYs:** Terminals are powered by `portable-pty` and parsed with `vt100` (`src/tui/shell_terminal.rs`), independent of Neovim.
- **Scrollback Navigation:** Scroll history with `Shift+PageUp`, `Shift+PageDown`, or mouse wheel (plain `PageUp` and `PageDown` are forwarded directly to the shell).
- **Selection & Copying:** Left-click drag highlights text in the buffer. Copying is performed by pressing `Ctrl+Shift+C`, `Ctrl+Insert`, or `Ctrl+C` while text is selected.
- **Paste:** `Ctrl+Shift+V`, `Ctrl+V`, or `Shift+Insert` writes clipboard text directly into the shell PTY.
- **Mouse Reporting:** Interactive terminal programs (e.g. `htop`) receive SGR mouse events. Hold `Shift` during click or drag to force native Tuim text selection.

---

## 3. Extension Shop & Plugin Management

- **Installed List (`1`):** View installed plugins, bundled modules, and local configs.
- **Discover List (`2`):** Browse curated marketplace extensions.
- **Pane Navigation:** Press `Tab` or `Shift+Tab` (or `Left`/`Right`) to toggle focus between the navigation sidebar and the plugin list pane.
- **Configuration Overrides (`e`):** Generates or edits Lua spec overrides under `~/.local/share/tuim/plugin_configs/<plugin>.lua`.
- **Lifecycle Actions:** Enable/disable (`d`), uninstall (`u` or `x`), install/reinstall (`Enter` or `i`), search (`/`), focus categories (`c`), refresh (`r`).

---

## 4. Language Tools & Mason

- **Project Markers:** Scans workspace root for markers (`Cargo.toml`, `build.zig`, `pyproject.toml`, `package.json`, `go.mod`, `Makefile`).
- **Recommendations:** Recommends relevant Mason packages and language servers via `F1` → `Language tools`.

---

## 5. Git Integration

- **Sidebar Panel:** Displays staged and unstaged file trees with modification status.
- **Key Actions:** Stage file (`+`), unstage file (`-` or `u`), toggle staging (`Space` or `s`), stage all (`a`), unstage all (`U`), discard changes (`d`), commit message entry (`c` or `i`), pull (`p`), push (`P`), detailed log dialog (`m`).
- **Diff:** Pressing `Enter` opens the file in the editor.

---

## 6. Themes

- **Built-in Options:** VS Code Dark Modern, Matte Black, Tokyo Night, Tokyo Night Storm, Catppuccin Mocha, Gruvbox Dark, Nord, Cyberdream, Rose Pine, Kanagawa, and Nightfox.
- **System Theme:** Select `System (follow desktop)` in Settings to follow desktop Omarchy palette files located at `~/.local/state/omarchy/current/theme/colors.toml` (or `~/.config/`).
- **Default Contrast:** Default theme contrast between primary text, background, and sidebar cards is validated in unit tests (`src/tui/theme.rs`).
