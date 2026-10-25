# Keybindings Reference

Default shortcuts and their contexts in Rtuim. Saved custom bindings take
precedence; terminal support determines which modified keys can be transmitted.

---

## 1. TUI Interface Controls

Handled directly by the Tuim Rust TUI layer:

| Action | Shortcut | Notes |
| :--- | :--- | :--- |
| **Search Commands** | `F1` | Opens the native fuzzy command palette |
| **Toggle Zen Mode** | `F11` | Hides chrome; focuses editor; press again to restore |
| **Cycle Focus Forward** | `F6` | Cycles: Editor → Sidebar → Terminal → Vertical Terminal |
| **Cycle Focus Backward** | `Shift + F6` | Reverse cycle across visible regions |
| **Toggle Sidebar** | `Ctrl + E` / `Space e` (Normal mode) | Toggles the active workspace sidebar (`Ctrl+B` in Familiar preset) |
| **Toggle Terminal Drawer** | `Ctrl + T` | Opens or collapses the bottom shell terminal |
| **Resize Panels** | `Alt + Left / Right` | Resizes vertical terminal divider (if focused or sidebar collapsed) or sidebar divider |
| **Resize Drawer** | `Alt + Up / Down` | Resizes bottom terminal drawer divider (visible Zen panes can resize) |
| **Resize via Mouse** | Mouse drag | Left-click and drag border dividers between panels |
| **Report a Bug** | `F12` | Opens the in-app bug report form (or `F1` → Report bug) |

---

## 2. Integrated Shell Terminal Controls

Applies when focus is inside either the bottom drawer or vertical split terminal:

| Action | Shortcut | Description |
| :--- | :--- | :--- |
| **Scroll History** | `Shift + PageUp` / `Shift + PageDown` | Scrolls terminal scrollback buffer (plain `PageUp`/`PageDown` are forwarded to shell) |
| **Mouse Wheel** | Scroll Up / Down | Scrolls scrollback buffer |
| **Highlight Text** | Left-click and drag | Highlights text in the terminal buffer |
| **Copy Text** | `Ctrl + Shift + C` or `Ctrl + Insert` | Copies highlighted text to clipboard |
| **Copy Active Selection** | `Ctrl + C` | Copies text when a selection is active |
| **Paste Clipboard** | `Ctrl + Shift + V`, `Ctrl + V`, or `Shift + Insert` | Pastes clipboard text directly into PTY |
| **Bypass App Mouse** | `Shift + Left-click drag` | Forces native Tuim selection when app handles mouse |

> [!NOTE]
> Releasing a mouse drag leaves text highlighted in the terminal buffer without auto-copying. Press `Ctrl+Shift+C` or `Ctrl+C` to copy the highlighted selection. Neovim's `Ctrl+\ Ctrl+N` escape mode does not apply to Tuim's independent PTY sessions.

---

## 3. Editor Controls (Normal Mode)

When editing in **Normal Mode** (`Leader = Space`):

### File & Buffer Management
| Action | Shortcut |
| :--- | :--- |
| **Save Current Buffer** | `Ctrl + S` |
| **Quit Tuim** | `Ctrl + Q` (prompts to confirm unsaved changes) |
| **New Empty Buffer** | `Ctrl + N` |
| **Find Files** | `Ctrl + P` (TUI) or `Space f f` (Telescope) |
| **Search Project Text** | `Alt + G` (TUI) or `Space f g` (Telescope) |
| **Switch Buffers** | `Space f b` (Telescope) or `F1` → `Switch buffers` |
| **Toggle Workspace Explorer** | `Ctrl + E` or `Space e` (or `Ctrl + B` in Familiar preset) |

### Window & Split Navigation
| Action | Shortcut |
| :--- | :--- |
| **Horizontal Editor Split** | `Alt + S` or `Ctrl + W s` |
| **Vertical Editor Split** | `Alt + V` or `Ctrl + W v` |
| **Move Focus to Split** | `Ctrl + W h/j/k/l` (or `Alt+h/j/k/l`) |
| **Cycle Window Focus** | `Alt + O` |
| **Close Current Split** | `Alt + C` or `Ctrl + W q` |
| **Toggle Bottom Terminal** | `Space o t` |
| **Toggle Vertical Terminal** | `Space o Shift+T` |

### Editing Motions & Text Manipulation
| Action | Shortcut |
| :--- | :--- |
| **Scroll Half Page Down** | `Ctrl + D` (centered) |
| **Scroll Half Page Up** | `Ctrl + U` (centered) |
| **Move Line(s) Down** | `J` (Visual mode) |
| **Move Line(s) Up** | `K` (Visual mode) |
| **Delete Without Yanking** | `Space d` (Normal and Visual) |
| **Paste Over Selection** | `Space p` (Visual mode; preserves clipboard) |
| **Substitute Word on Current Line** | `Space s` (pre-populates `:s/\<<word>\>//gI`) |

### AI Assistant Integration
| Action | Shortcut |
| :--- | :--- |
| **Send Selection to AI** | `Ctrl + Alt + S` or `Space a s` (Visual mode) |
| **Send File Path to AI** | `Ctrl + Alt + F` or `Space a f` |
| **Send Full File Content to AI** | `Ctrl + Alt + C` or `Space a c` |

---

## 4. Editor Controls (IDE Mode)

In **IDE Mode**, Tuim provides modeless editing:

| Action | Shortcut | Description |
| :--- | :--- | :--- |
| **Save** | `Ctrl + S` / `Cmd + S` | Saves active buffer |
| **Undo** | `Ctrl + Z` / `Cmd + Z` | Reverts last edit |
| **Redo** | `Ctrl + Y` or `Ctrl + Shift + Z` | Redoes reverted edit |
| **Select All** | `Ctrl + A` / `Cmd + A` | Selects entire file content |
| **Select Line** | `Ctrl + L` | Selects current line |
| **Copy** | `Ctrl + C` / `Cmd + C` | Copies selection |
| **Cut** | `Ctrl + X` / `Cmd + X` | Cuts selection |
| **Paste** | `Ctrl + V` / `Cmd + V` | Pastes clipboard content |
| **Find in Buffer** | `Ctrl + F` / `Cmd + F` | Opens fuzzy search for active buffer |
| **Replace in Buffer** | `Ctrl + H` / `Cmd + R` | Opens search-and-replace prompt |
| **Extend Selection** | `Shift + Arrows` | Highlights text character or line-wise |
| **Select by Word** | `Ctrl + Shift + Left / Right` | Highlights whole words |

---

## 5. Sidebar Tool Shortcuts

### Git Sidebar Panel
| Action | Shortcut |
| :--- | :--- |
| **Navigate Changes** | `Up` / `Down` or `j` / `k` |
| **First / Last Item** | `Home` / `End` or `g` / `G` |
| **Open Diff in Editor** | `Enter` |
| **Toggle Staging** | `Space` or `s` |
| **Stage Selected File** | `+` |
| **Unstage Selected File** | `-` or `u` |
| **Stage All Changes** | `a` |
| **Unstage All Changes** | `U` |
| **Discard Changes** | `d` |
| **Type Commit Message** | `c` or `i` (`Enter` to commit, `Esc` to cancel) |
| **Git Pull** | `p` |
| **Git Push** | `P` |
| **Detailed Git Dialog** | `m` |

### Extension Shop
| Action | Shortcut |
| :--- | :--- |
| **Switch to Installed View** | `1` |
| **Switch to Discover View** | `2` |
| **Switch Focus Between Panes** | `Tab` / `Shift + Tab` or `Left` / `Right` (toggles nav sidebar vs plugin list) |
| **Focus Discovery Categories** | `c` |
| **Search Plugins** | `/` |
| **Refresh Marketplace** | `r` |
| **Edit Configuration** | `e` (creates/opens config in `~/.local/share/tuim/plugin_configs/`) |
| **Enable / Disable Plugin** | `d` |
| **Uninstall Plugin** | `u` or `x` (prompts `y`/`n` confirmation) |
| **Install Plugin** | `Enter` or `i` |
| **Close Shop** | `Esc` or `q` |

### AI Assistants Panel
| Action | Shortcut |
| :--- | :--- |
| **Switch Tabs** | `1` (Agents), `2` (Context), `3` (Actions) or `Tab` / `Shift+Tab` |
| **Navigate List** | `Up` / `Down` or `j` / `k` |
| **Activate Action** | `Enter` |
| **Focus AI Terminal** | `f` |
| **Restart Session** | `r` |
| **Stop Session** | `s` |

---

## 6. Keybinding Presets & Customization

1. Open the Command Palette with `F1`.
2. Highlight any command and press `F2` (or click its shortcut column) to rebind.
3. Open **Settings (`F1` → Settings) → Keybindings**:
   - Factory defaults retain standard Ctrl-based controls (`Ctrl+E`, `Ctrl+T`, `Ctrl+N`, `Ctrl+P`, `Ctrl+S`, `Ctrl+Q`, `F1`, `F6`, `Shift+F6`, `F12`).
   - Press `v` to apply the **Vim-safe preset** (`Alt+E` sidebar, `Alt+T` terminal, `Alt+N` new file, `Alt+P` find files, `Alt+Q` quit, `F2` save file), which frees Vim motions and completion keys.
   - Press `p` to apply the **Familiar preset** (`Ctrl+B` sidebar, `F6` focus).
   - Press `r` to reset to factory defaults.
   - Press `Ctrl + S` to save changes.
   - Existing saved shortcuts in `settings.json` take precedence over factory defaults.
