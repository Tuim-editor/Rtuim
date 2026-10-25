# Using Tuim

[Back to the README](../README.md)

## Workspace basics

TUIM opens with Explorer visible by default. Click **< Workspace** above it to
reach the open-files and tools list.

Normal and IDE use one compact workspace sidebar for open files and tools, with
the current file in the editor header. `F1` opens a searchable command menu;
`F6` cycles forward and `Shift+F6` cycles backward between editor, sidebar, and
integrated terminal panels with positive geometry. `Alt+Arrow` keys resize active
panel dividers (or use mouse drag). In the sidebar, use arrows or `j`/`k` and Enter,
or click an item. Escape returns from a tool to the workspace overview list, then
to the editor. At widths below 40 columns, the sidebar collapses automatically and
commands remain available through F1.

Zen hides the sidebar and terminal panel and focuses the editor. Leaving Zen
restores the previous sidebar/terminal focus and keeps the existing terminal
session alive. Its quiet footer provides a clickable return action. Normal's
footer displays Neovim's current editing mode; IDE uses the same design with
an IDE badge and modeless editing.

The bottom drawer and vertical split provide integrated shell terminals running
as native Rust PTY sessions, completely isolated from Neovim's editing modes:

- **Scrollback:** Press `Shift+PageUp`, `Shift+PageDown`, or use the mouse
  wheel to navigate scrollback history. Plain `PageUp` and `PageDown` are
  forwarded directly to the active shell or console application. Returning to
  the prompt or typing new input snaps scrollback to bottom automatically.
- **Selection:** Left-button drag directly highlights text in the terminal buffer.
- **Copy:** When text is highlighted, press `Ctrl+Shift+C`, `Ctrl+Insert`, or
  `Ctrl+C` to copy the selected text to the clipboard. Copying shares the text
  with the system clipboard when enabled.
- **Paste:** Press `Ctrl+Shift+V`, `Ctrl+V`, or `Shift+Insert` to paste text
  into the running shell.
- **Mouse forwarding:** When a running console application enables terminal mouse
  reporting (SGR mode), mouse clicks and movement are forwarded directly to that
  program. Hold `Shift` during a click or drag to override application mouse
  reporting for native Tuim selection.

To create a file, press **Ctrl+N** and start typing (press `i` first in Normal
mode). When the terminal is focused, use **F1 > New file** or focus the editor
first because `Ctrl+N` is forwarded to the active shell. On the first
**Ctrl+S**, enter a filename such as `notes.txt` and press
**Enter**. Relative paths are saved in the current working directory. **Esc**
cancels naming without discarding your text. Later saves use the same name.

In the Git sidebar, use **Up/Down** or **j/k** to select a change, **Enter** to
open it in the editor, and **Space** or **s** to toggle staging. Press **+** to
stage the selected file, **-** or **u** to unstage it, **a** to stage all changes,
and **U** to unstage all. Press **d** to discard changes in the selected file.
**Home/End** or **g/G** jumps to the first/last change. Press **c** or **i** to write
a commit message, then **Enter** to commit or **Esc** to return to the list. Press
**p** to pull, **P** to push, and **m** to open the detailed Git dialog.

## AI assistants

Open **AI assistants** from the workspace list. Select the assistant name to
change it; the chooser shows which CLIs are installed and which chats are
running. Choose an assistant, then **Open chat** and type your request in its
terminal. Install missing assistant CLIs in your terminal and restart Tuim to
refresh detection.

In the AI sidebar, **1**, **2**, and **3** switch between Agents, Context, and
Actions tabs, or use **Tab** / **Shift+Tab** / **Left** / **Right** to cycle tabs.
**Up/Down** or **j/k** navigates items and **Enter** activates the selection.
Press **f** to focus the AI terminal, **r** to restart it, and **s** to stop it.
**Escape** returns to the Workspace list. Use **F6** to move between regions.

With a chat running, **Send selection** (`Space a s` or `Ctrl+Alt+S`) and **Send file**
(`Space a f` or `Ctrl+Alt+F`) paste code into that chat for you to use in your next
request. **Send entire file** (`Space a c` or `Ctrl+Alt+C`) forwards the full buffer.
Actions apply to the assistant whose chat is currently open.

## First run

On its first launch, Tuim explains how to open, edit, and save a file, with
plain-language choices for IDE and Normal (Vim) editing. Press `i` or `n` to
choose a style and start editing. Optional language-server setup is available
with `l`, which opens Tuim's built-in package manager on the LSP tab. Close the
guide with Enter, Escape, `q`, Ctrl+C, or by clicking **Click to close guide** at
the top. Closing keeps Tuim open and preserves your current editing style. Reopen
it from the Help page by pressing `o`, or run `:TuimOnboarding` in Normal mode.
The choice and completion marker are stored only in Tuim's isolated data directory.

## Default Keybindings

### TUI Interface Controls

These keybindings are handled directly by the Tuim TUI layer:

| Action | Keybinding |
| :--- | :--- |
| **Toggle Zen / previous mode** | `F11` |
| **Search commands** | `F1` or click Commands in the header |
| **Cycle forward region focus** | `F6` |
| **Cycle backward region focus** | `Shift + F6` |
| **Report a bug** | `F12` (or Commands → Report bug) |
| **Toggle workspace sidebar** | `Ctrl + E` (or `Space e` in Normal; `Ctrl + B` in Familiar preset) |
| **Toggle bottom terminal panel** | `Ctrl + T` |
| **Resize panels** | `Alt + Left / Right` (sidebar/vertical terminal), `Alt + Up / Down` (bottom terminal), or mouse drag (visible Zen panes resize) |

### Native Terminal Panel Controls

These keybindings apply when either integrated shell terminal is focused:

| Action | Keybinding |
| :--- | :--- |
| **Scroll history up / down** | `Shift+PageUp` / `Shift+PageDown` or mouse wheel |
| **Copy selection** | `Ctrl + Shift + C`, `Ctrl + Insert`, or `Ctrl + C` (when text is highlighted) |
| **Paste clipboard** | `Ctrl + Shift + V`, `Ctrl + V`, or `Shift + Insert` |
| **Highlight text** | Left-click and drag |
| **Bypass app mouse reporting** | `Shift + Left-click drag` |

### Neovim / Editor Keybindings

These shipped mappings apply primarily in Normal mode (`Leader = Space`). IDE mode intentionally changes editing behavior.

| Action | Keybinding |
| :--- | :--- |
| **Save file** | `Ctrl + S` |
| **Quit Tuim (confirm unsaved changes)** | `Ctrl + Q` |
| **Open new buffer** | `Ctrl + N` |
| **Find files (Telescope / native prompt)** | `Space f f` or `Ctrl + P` |
| **Search project text with ripgrep (Telescope)** | `Space f g` or `Alt + G` |
| **Toggle Workspace Explorer** | `Ctrl + E` or `Space e` (or `Ctrl + B` in Familiar preset) |
| **Toggle bottom terminal split** | `Space o t` |
| **Toggle vertical terminal split** | `Space o Shift+T` |
| **Create horizontal editor split** | `Alt + S` (or `Ctrl+w s`) |
| **Create vertical editor split** | `Alt + V` (or `Ctrl+w v`) |
| **Move between editor splits** | `Ctrl+w`, then `h`, `j`, `k`, `l` (or `Alt+h/j/k/l`) |
| **Close current editor split** | `Alt + C` (or `Ctrl+w q`) |
| **Open editor settings** | `Space t h` |
| **Delete without yanking** | `Space d` |
| **Prepare word substitution on the current line** | `Space s` |
| **Paste over selection** | `Space p` (Visual) |
| **Scroll half page down (centered)** | `Ctrl + D` |
| **Scroll half page up (centered)** | `Ctrl + U` |
| **Move lines down** | `J` (Visual) |
| **Move lines up** | `K` (Visual) |
| **Send selection to AI** | `Ctrl + Alt + S` or `Space a s` (Visual) |
| **Send file path to AI** | `Ctrl + Alt + F` or `Space a f` |
| **Send file content to AI** | `Ctrl + Alt + C` or `Space a c` |

> [!NOTE]
> Core TUI keybindings can be customized in native Settings (`F1` → Settings → Keybindings).
> Press Enter on an action to record a shortcut. Duplicate bindings are rejected.
> Press `r` to reset to defaults, `p` to apply the Familiar preset, `v` to apply
> the Vim-safe preset, and `Ctrl+S` to save. Existing user-customized bindings in `settings.json`
> always take precedence over factory defaults.

In the F1 command menu, select any command and press `F2`, or click its
shortcut column, to assign or replace its shortcut. Press a Ctrl/Alt combination
or function key to save immediately; Escape cancels. Conflicting shortcuts are
rejected. The **Command menu** entry lets you change the F1 binding itself.
**Close buffer** closes the current buffer and asks before discarding unsaved
changes. **Switch buffers** opens a searchable list of open buffers, including
paths and current/modified indicators. Use arrows and Enter, or click a result;
Escape returns to the command menu. Both buffer commands start without a
shortcut so you can assign your preferred keys. The buffer list works without
plugins, including in Zen and narrow terminals.

The shortcut editor offers:
- Factory Default: Preserves standard Ctrl-based controls (`Ctrl+E` sidebar, `Ctrl+T` terminal, `Ctrl+N` new file, `Ctrl+P` find files, `Ctrl+S` save, `Ctrl+Q` quit, `F1` commands, `F6` next focus, `Shift+F6` prev focus, `F11` Zen, `F12` report bug).
- `v`: **Vim-safe preset** (opt-in preset replacing Ctrl bindings with `Alt+E` sidebar, `Alt+T` terminal, `Alt+N` new file, `Alt+P` find files, `Alt+Q` quit, and `F2` save, completely freeing Vim motions and completion keys).
- `p`: **Familiar preset** (applies standard bindings with `Ctrl+B` for sidebar and `F6` for focus).
- `r`: **Reset** (restores default factory configuration).

In IDE mode, Shift+Arrow extends the selection, Ctrl/Cmd+Shift+Left or Right
selects by word, and Ctrl/Cmd+A selects the buffer. Ctrl/Cmd+S, Z, C, X, and V
provide save, undo, copy, cut, and paste; Ctrl+Y or Ctrl/Cmd+Shift+Z redoes.
Ctrl+L selects the current line. Ctrl/Cmd+F opens buffer search and Ctrl+H
(Cmd+R on terminals that report it) opens replace. Home, End, Ctrl+Arrow, mouse
click, and mouse drag use Neovim's terminal-native navigation and selection.
Some terminals cannot distinguish Cmd from Alt or report shifted modifiers;
Tuim keeps the Ctrl form available and uses the terminal capability fallback.
Use the command menu and editor context menu when a terminal does not report
a shortcut distinctly.
Escape and incidental mode changes return editable file buffers to text-entry
mode. Dialogs, terminals, help pages, and plugin-owned utility buffers keep
their native keys and modes so their own escape routes continue to work.

The Editor tab in Settings controls **Line Numbers** (cycling between relative,
absolute, and hidden) and **Tab Size** (custom tabstop width). Changes apply
immediately to current and new file windows.

In Settings > Appearance > Theme, **System (follow desktop)** follows the
current Omarchy palette for editor, syntax, sidebar, and selection colors.
It reads `omarchy/current/theme/colors.toml` under `XDG_STATE_HOME` (normally
`~/.local/state`), with the older `XDG_CONFIG_HOME` location as a fallback.
Changes apply automatically while Tuim is open. The option needs no theme
plugin and keeps `system` as the saved preference. If the palette is missing
or invalid at startup, Tuim reports that it is using default colors and keeps
watching for a valid palette. Choosing another theme stops following the desktop.

### Extensions

Open **Extensions** from the workspace or the F1 command menu. It opens directly
to **Installed**, your offline library of bundled plugins, dependencies,
marketplace additions, and local plugin directories. Switch to **Discover** to
browse the marketplace; choose categories in the sidebar.

The panel shows a searchable plugin list and, on wide terminals, the selected
plugin's details and actions alongside it. Compact terminals open details with
Enter or a click. Use `1` for Installed, `2` for Discover, `Tab` / `Shift+Tab`
(or `Left` / `Right`) to switch focus between the navigation sidebar and the
plugin list pane, `c` to focus discovery categories, `/` to search, and `r` to
refresh. Up/Down select plugins. Settings > Plugins > Installed Plugins opens
this same panel.

![Extensions with installed plugins and inline controls](screenshots/extensions.webp)

| Detail action | Key |
| --- | --- |
| Edit configuration | `e` |
| Enable / disable | `d` |
| Uninstall | `u` (or `x`), then `y` to confirm or `n` to cancel |
| Install / reinstall | `Enter` (or `i`) |
| Return to list (compact), or return to editor | `Escape` (or `q`) |

Restart Tuim to apply installation, activation, and removal changes. Disabled
plugins remain on disk. Uninstall removes only the selected plugin on the next
normal startup and preserves its configuration for reinstallation. Dependencies
required by enabled plugins must be kept; disable their dependents first.
Tuim's plugin manager and unmanaged local directories are protected.

Configuration opens as a Lua file under `$XDG_DATA_HOME/tuim/plugin_configs/`
(default `~/.local/share/tuim/plugin_configs/`). Save it and restart. Return a
[lazy.nvim spec override](https://lazy.folke.io/spec), for example:

```lua
return {
  opts = { -- options passed to plugins using automatic setup
    -- plugin-specific values here
  },
}
```

For bundled plugins with a custom setup function, override `config` explicitly:

```lua
return {
  config = function(plugin, opts)
    require("plugin_module").setup(opts or {}) -- use the plugin's documented module
  end,
}
```

An explicit `config` replaces Tuim's setup for that plugin. Repository identity,
dependencies, and enable/disable state stay under the manager's control. Invalid
configuration is reported without aborting the editor. Recovery mode
(`TUIM_DISABLE_PLUGINS=1 tuim`) skips user config execution so you can repair it.
**Settings > Plugins > Plugin Manager** retains Lazy's update and sync tools;
**Mason Settings** manages language servers and formatters separately.

### Language Tools

Tuim detects common project markers for Zig, Lua, Python, Rust,
JavaScript/TypeScript, Go, and C/C++, then recommends only the matching Mason
language servers. Projects without recognized markers remain manual rather
than installing unrelated tools. Open **Workspace > Language tools**, or choose
**Language tools** from the F1 command menu, for the health summary, active
servers, recommended packages, missing executables, and installed
LSP/formatter/linter status. **Settings > Plugins > Mason Settings** opens the
same panel. Missing or failed tooling is also reported through native notices
with details in the Tuim log.

### Accessibility and Terminal Fallbacks

All primary controls are keyboard accessible; sidebar lists show their
navigation keys and focused controls use both color and a visible marker or
inverse background. Tuim supplies text-symbol alternatives when Nerd Fonts are
disabled and keeps keyboard routes available when mouse reporting is absent.
Dialogs show explicit empty, loading, success, and error messages, and compact
terminals receive a resize instruction instead of clipped or unsafe layouts.
The default theme is regression-tested for readable primary, secondary,
status-bar, and focused-control contrast.
