use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;

use rmpv::Value;
use serde::{Deserialize, Serialize};

use crate::nvim::rpc::RpcClient;
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives;
use crate::tui::widgets::primitives::{ManagerLayout, text_in};

struct SettingRow {
    label: &'static str,
    value: String,
    description: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Keybindings {
    #[serde(default = "default_toggle_explorer")]
    pub toggle_explorer: String,
    #[serde(default = "default_toggle_terminal")]
    pub toggle_terminal: String,
    #[serde(default = "default_toggle_zen")]
    pub toggle_zen: String,
    #[serde(default = "default_new_file")]
    pub new_file: String,
    #[serde(default = "default_find_file")]
    pub find_file: String,
    #[serde(default = "default_search_project")]
    pub search_project: String,
    #[serde(default = "default_quit")]
    pub quit: String,
    #[serde(default = "default_save_file")]
    pub save_file: String,
    #[serde(default = "default_commands")]
    pub commands: String,
    #[serde(default = "default_focus_next")]
    pub focus_next: String,
    #[serde(default = "default_focus_prev")]
    pub focus_prev: String,
    #[serde(default = "default_resize_left")]
    pub resize_left: String,
    #[serde(default = "default_resize_right")]
    pub resize_right: String,
    #[serde(default = "default_resize_up")]
    pub resize_up: String,
    #[serde(default = "default_resize_down")]
    pub resize_down: String,
    #[serde(default)]
    pub close_buffer: String,
    #[serde(default)]
    pub switch_buffers: String,
    #[serde(default)]
    pub project_files: String,
    #[serde(default)]
    pub changes: String,
    #[serde(default)]
    pub problems: String,
    #[serde(default)]
    pub ai_assistants: String,
    #[serde(default)]
    pub extensions: String,
    #[serde(default)]
    pub language_tools: String,
    #[serde(default)]
    pub settings: String,
    #[serde(default)]
    pub keyboard_shortcuts: String,
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub split_right: String,
    #[serde(default)]
    pub terminal_right: String,
    #[serde(default)]
    pub split_down: String,
    #[serde(default = "default_report_bug")]
    pub report_bug: String,
}

fn default_toggle_explorer() -> String {
    "<C-e>".to_string()
}
fn default_toggle_terminal() -> String {
    "<C-t>".to_string()
}
fn default_toggle_zen() -> String {
    "<F11>".to_string()
}
fn default_new_file() -> String {
    "<C-n>".to_string()
}
fn default_find_file() -> String {
    "<C-p>".to_string()
}
fn default_search_project() -> String {
    "<M-g>".to_string()
}
fn default_quit() -> String {
    "<C-q>".to_string()
}
fn default_save_file() -> String {
    "<C-s>".to_string()
}
fn default_commands() -> String {
    "<F1>".to_string()
}
fn default_focus_next() -> String {
    "<F6>".to_string()
}

fn default_focus_prev() -> String { "<S-F6>".to_string() }
fn default_resize_left() -> String { "<M-Left>".to_string() }
fn default_resize_right() -> String { "<M-Right>".to_string() }
fn default_resize_up() -> String { "<M-Up>".to_string() }
fn default_resize_down() -> String { "<M-Down>".to_string() }
fn default_report_bug() -> String { "<F12>".to_string() }

impl Default for Keybindings {
    fn default() -> Self {
        Self::factory_preset()
    }
}

impl Keybindings {
    fn factory_preset() -> Self {
        Self {
            toggle_explorer: default_toggle_explorer(),
            toggle_terminal: default_toggle_terminal(),
            toggle_zen: default_toggle_zen(),
            new_file: default_new_file(),
            find_file: default_find_file(),
            search_project: default_search_project(),
            quit: default_quit(),
            save_file: default_save_file(),
            commands: default_commands(),
            focus_next: default_focus_next(),
            focus_prev: default_focus_prev(),
            resize_left: default_resize_left(),
            resize_right: default_resize_right(),
            resize_up: default_resize_up(),
            resize_down: default_resize_down(),
            close_buffer: String::new(),
            switch_buffers: String::new(),
            project_files: String::new(),
            changes: String::new(),
            problems: String::new(),
            ai_assistants: String::new(),
            extensions: String::new(),
            language_tools: String::new(),
            settings: String::new(),
            keyboard_shortcuts: String::new(),
            help: String::new(),
            split_right: String::new(),
            terminal_right: String::new(),
            split_down: String::new(),
            report_bug: default_report_bug(),
        }
    }

    pub fn vim_safe_preset() -> Self {
        let mut kb = Self::factory_preset();
        kb.toggle_explorer = "<M-e>".to_string();
        kb.toggle_terminal = "<M-t>".to_string();
        kb.new_file = "<M-n>".to_string();
        kb.find_file = "<M-p>".to_string();
        kb.quit = "<M-q>".to_string();
        kb.save_file = "<F2>".to_string();
        kb
    }

    pub fn familiar_preset() -> Self {
        let mut kb = Self::factory_preset();
        kb.toggle_explorer = "<C-b>".to_string();
        kb.focus_next = "<F6>".to_string();
        kb
    }

    pub fn get_by_field(&self, field: &str) -> &str {
        match field {
            "toggle_explorer" => &self.toggle_explorer,
            "toggle_terminal" => &self.toggle_terminal,
            "toggle_zen" => &self.toggle_zen,
            "new_file" => &self.new_file,
            "find_file" => &self.find_file,
            "search_project" => &self.search_project,
            "quit" => &self.quit,
            "save_file" => &self.save_file,
            "commands" => &self.commands,
            "focus_next" => &self.focus_next,
            "resize_down" => &self.resize_down,
            "resize_up" => &self.resize_up,
            "resize_right" => &self.resize_right,
            "resize_left" => &self.resize_left,
            "focus_prev" => &self.focus_prev,
            "close_buffer" => &self.close_buffer,
            "switch_buffers" => &self.switch_buffers,
            "project_files" => &self.project_files,
            "changes" => &self.changes,
            "problems" => &self.problems,
            "ai_assistants" => &self.ai_assistants,
            "extensions" => &self.extensions,
            "language_tools" => &self.language_tools,
            "settings" => &self.settings,
            "keyboard_shortcuts" => &self.keyboard_shortcuts,
            "help" => &self.help,
            "split_right" => &self.split_right,
            "terminal_right" => &self.terminal_right,
            "split_down" => &self.split_down,
            "report_bug" => &self.report_bug,
            _ => "",
        }
    }

    pub fn set_by_field(&mut self, field: &str, val: String) -> bool {
        match field {
            "toggle_explorer" => self.toggle_explorer = val,
            "toggle_terminal" => self.toggle_terminal = val,
            "toggle_zen" => self.toggle_zen = val,
            "new_file" => self.new_file = val,
            "find_file" => self.find_file = val,
            "search_project" => self.search_project = val,
            "quit" => self.quit = val,
            "save_file" => self.save_file = val,
            "commands" => self.commands = val,
            "focus_next" => self.focus_next = val,
            "resize_down" => self.resize_down = val,
            "resize_up" => self.resize_up = val,
            "resize_right" => self.resize_right = val,
            "resize_left" => self.resize_left = val,
            "focus_prev" => self.focus_prev = val,
            "close_buffer" => self.close_buffer = val,
            "switch_buffers" => self.switch_buffers = val,
            "project_files" => self.project_files = val,
            "changes" => self.changes = val,
            "problems" => self.problems = val,
            "ai_assistants" => self.ai_assistants = val,
            "extensions" => self.extensions = val,
            "language_tools" => self.language_tools = val,
            "settings" => self.settings = val,
            "keyboard_shortcuts" => self.keyboard_shortcuts = val,
            "help" => self.help = val,
            "split_right" => self.split_right = val,
            "terminal_right" => self.terminal_right = val,
            "split_down" => self.split_down = val,
            "report_bug" => self.report_bug = val,
            _ => return false,
        }
        true
    }

    pub fn all_fields(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("toggle_explorer", &self.toggle_explorer),
            ("toggle_terminal", &self.toggle_terminal),
            ("toggle_zen", &self.toggle_zen),
            ("new_file", &self.new_file),
            ("find_file", &self.find_file),
            ("search_project", &self.search_project),
            ("quit", &self.quit),
            ("save_file", &self.save_file),
            ("commands", &self.commands),
            ("focus_next", &self.focus_next),
            ("resize_down", &self.resize_down),
            ("resize_up", &self.resize_up),
            ("resize_right", &self.resize_right),
            ("resize_left", &self.resize_left),
            ("focus_prev", &self.focus_prev),
            ("close_buffer", &self.close_buffer),
            ("switch_buffers", &self.switch_buffers),
            ("project_files", &self.project_files),
            ("changes", &self.changes),
            ("problems", &self.problems),
            ("ai_assistants", &self.ai_assistants),
            ("extensions", &self.extensions),
            ("language_tools", &self.language_tools),
            ("settings", &self.settings),
            ("keyboard_shortcuts", &self.keyboard_shortcuts),
            ("help", &self.help),
            ("split_right", &self.split_right),
            ("terminal_right", &self.terminal_right),
            ("split_down", &self.split_down),
            ("report_bug", &self.report_bug),
        ]
    }

    pub fn has_conflict(&self, target_field: &str, key: &str) -> bool {
        if key.is_empty() {
            return false;
        }
        for (field, bound) in self.all_fields() {
            if field != target_field && bound.eq_ignore_ascii_case(key) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_true", alias = "clip")]
    pub clipboard: bool,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub theme_custom: String,
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default = "default_true")]
    pub nerd_fonts: bool,
    #[serde(default = "default_true")]
    pub autocomplete: bool,
    #[serde(default = "default_true")]
    pub autoindent: bool,
    #[serde(default = "default_numbers")]
    pub numbers: String,
    #[serde(default = "default_tabstop")]
    pub tabstop: u8,
    #[serde(default = "default_true")]
    pub mason_enabled: bool,
    #[serde(default = "default_true")]
    pub lazy_enabled: bool,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub keybindings: Keybindings,
}

fn default_version() -> u32 {
    1
}
fn default_true() -> bool {
    true
}
fn default_mode() -> String {
    "ide".to_string()
}
fn default_theme() -> String {
    "vscode".to_string()
}
fn default_separator() -> String {
    "thin".to_string()
}
fn default_numbers() -> String {
    "relative".to_string()
}
fn default_tabstop() -> u8 {
    4
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            version: 1,
            clipboard: true,
            mode: default_mode(),
            theme: default_theme(),
            theme_custom: String::new(),
            separator: default_separator(),
            nerd_fonts: true,
            autocomplete: true,
            autoindent: true,
            numbers: default_numbers(),
            tabstop: 4,
            mason_enabled: true,
            lazy_enabled: true,
            onboarding_completed: false,
            keybindings: Keybindings::default(),
        }
    }
}

impl SettingsConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        let mut opts = fs::OpenOptions::new();
        opts.read(true).custom_flags(libc::O_NOFOLLOW);
        let mut file = opts.open(path).map_err(|e| e.to_string())?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if meta.len() > 65536 {
            return Err("Settings file exceeds 64 KiB limit".to_string());
        }
        let mut buf = String::new();
        file.read_to_string(&mut buf).map_err(|e| e.to_string())?;

        let raw: serde_json::Value = serde_json::from_str(&buf).map_err(|e| e.to_string())?;
        if let Some(v) = raw.get("version").and_then(|v| v.as_u64()) {
            if v > 1 {
                return Err(format!("Unsupported settings schema version: {}", v));
            }
        }

        let mut cfg: SettingsConfig = serde_json::from_str(&buf).map_err(|e| e.to_string())?;
        // Only add a new default when it does not shadow a saved action.
        if let Some(saved) = raw.get("keybindings").and_then(|v| v.as_object()) {
            for field in ["focus_prev", "resize_left", "resize_right", "resize_up", "resize_down", "report_bug"] {
                let key = cfg.keybindings.get_by_field(field).to_string();
                if !saved.contains_key(field) && cfg.keybindings.has_conflict(field, &key) {
                    cfg.keybindings.set_by_field(field, String::new());
                }
            }
        }
        if cfg.version == 0 {
            cfg.version = 1;
        }
        Ok(cfg)
    }

    pub fn save(&self, path: &str) -> Result<(), String> {
        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.file_type().is_symlink() {
                return Err("Refusing to write to symlinked settings path".to_string());
            }
            let bak = format!("{}.bak", path);
            let _ = fs::copy(path, bak);
        }

        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        let tmp = format!("{}.tmp", path);
        let json_bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW);
            let mut f = opts.open(&tmp).map_err(|e| e.to_string())?;
            f.write_all(&json_bytes).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub const SETTINGS_TABS: &[&str] = &[
    "General",
    "Appearance",
    "Editor",
    "Plugins",
    "Keybindings",
    "About",
];

pub const THEME_OPTIONS: &[(&str, &str)] = &[
    ("System (follow desktop)", "system"),
    ("VS Code Dark Modern", "vscode"),
    ("Matte Black", "matteblack"),
    ("Tokyo Night", "tokyonight"),
    ("Tokyo Night Storm", "tokyonight-storm"),
    ("Catppuccin Mocha", "catppuccin"),
    ("Gruvbox Dark", "gruvbox"),
    ("Nord", "nord"),
    ("Cyberdream", "cyberdream"),
    ("Rose Pine", "rose-pine"),
    ("Kanagawa", "kanagawa"),
    ("Nightfox", "nightfox"),
];

pub const KEYBINDING_ROWS: &[(&str, &str)] = &[
    ("Toggle Terminal", "toggle_terminal"),
    ("Toggle Explorer", "toggle_explorer"),
    ("Toggle Zen", "toggle_zen"),
    ("New File", "new_file"),
    ("Find File", "find_file"),
    ("Search Project", "search_project"),
    ("Quit", "quit"),
    ("Save File", "save_file"),
    ("Commands", "commands"),
    ("Next Region", "focus_next"),
    ("Previous Region", "focus_prev"),
    ("Resize Left", "resize_left"),
    ("Resize Right", "resize_right"),
    ("Resize Up", "resize_up"),
    ("Resize Down", "resize_down"),
    ("Report Bug", "report_bug"),
];

pub struct SettingsWidget {
    pub is_open: bool,
    pub active_tab: usize,
    pub selected_row: usize,
    pub focus_sidebar: bool,
    pub search_query: String,
    pub is_searching: bool,
    pub config: SettingsConfig,
    pub saved_config: SettingsConfig,
    pub settings_path: String,
    pub dropdown_open: bool,
    pub dropdown_idx: usize,
    pub recording_key: bool,
    pub confirm_discard: bool,
    pub status_msg: String,
    pub open_mason_requested: bool,
    pub open_lazy_requested: bool,
    pub open_shop_requested: bool,
}

impl SettingsWidget {
    pub fn new(settings_path: &str, config: SettingsConfig) -> Self {
        Self {
            is_open: false,
            active_tab: 0,
            selected_row: 0,
            focus_sidebar: true,
            search_query: String::new(),
            is_searching: false,
            saved_config: config.clone(),
            config,
            settings_path: settings_path.to_string(),
            dropdown_open: false,
            dropdown_idx: 0,
            recording_key: false,
            confirm_discard: false,
            status_msg: String::new(),
            open_mason_requested: false,
            open_lazy_requested: false,
            open_shop_requested: false,
        }
    }

    pub fn modal_rect(screen: Rect) -> Rect {
        ManagerLayout::new(screen).outer
    }

    pub fn is_dirty(&self) -> bool {
        self.config != self.saved_config
    }

    pub fn save_and_sync(&mut self, rpc: &mut RpcClient) {
        if let Err(e) = self.config.save(&self.settings_path) {
            self.status_msg = format!("Save error: {}", e);
            return;
        }
        self.saved_config = self.config.clone();
        self.status_msg = "Settings saved.".to_string();
        self.is_open = false;

        // Sync with Neovim
        let clip_val = if self.config.clipboard {
            "unnamedplus"
        } else {
            ""
        };
        let nf_val = if self.config.nerd_fonts {
            "true"
        } else {
            "false"
        };
        let ac_val = if self.config.autocomplete {
            "true"
        } else {
            "false"
        };
        let ai_val = if self.config.autoindent {
            "true"
        } else {
            "false"
        };
        let mode_lua = match self.config.mode.as_str() {
            "ide" => "vim.g.tuim_zen_mode = false; vim.g.tuim_ide_mode = true; if _G.tuim_enable_ide_mode then _G.tuim_enable_ide_mode() end",
            "zen" => "vim.g.tuim_zen_mode = true; vim.g.tuim_ide_mode = false; if _G.tuim_disable_ide_mode then _G.tuim_disable_ide_mode() end",
            _ => "vim.g.tuim_zen_mode = false; vim.g.tuim_ide_mode = false; if _G.tuim_disable_ide_mode then _G.tuim_disable_ide_mode() end",
        };
        let lua_cmd = format!(
            "vim.o.clipboard = '{}'; vim.g.tuim_nerd_fonts = {}; vim.g.tuim_autocomplete_enabled = {}; vim.o.autoindent = {}; {}; if _G.tuim_apply_theme then pcall(_G.tuim_apply_theme, '{}') end",
            clip_val, nf_val, ac_val, ai_val, mode_lua, self.config.theme
        );
        let _ = rpc.notify(
            "nvim_exec_lua",
            vec![Value::from(lua_cmd), Value::Array(vec![])],
        );
    }

    pub fn handle_key(&mut self, ev: &InputEvent, rpc: &mut RpcClient) -> bool {
        if !self.is_open {
            return false;
        }
        let key = match ev {
            InputEvent::Key(k) => k.as_str(),
            InputEvent::Paste(text) => {
                if self.is_searching {
                    self.search_query.extend(text.chars().filter(|ch| !ch.is_control()));
                    self.find_search_match();
                }
                return true;
            }
            _ => return false,
        };

        if self.is_searching {
            match key {
                "<Esc>" => { self.is_searching = false; self.search_query.clear(); }
                "<CR>" => { self.is_searching = false; self.focus_sidebar = false; }
                "<C-s>" => { self.is_searching = false; self.save_and_sync(rpc); }
                "<BS>" => { self.search_query.pop(); self.find_search_match(); }
                k if k.chars().count() == 1 => { self.search_query.push_str(k); self.find_search_match(); }
                _ => {}
            }
            return true;
        }

        if self.confirm_discard {
            match key {
                "y" | "Y" | "<CR>" => {
                    self.config = self.saved_config.clone();
                    self.confirm_discard = false;
                    self.is_open = false;
                    return true;
                }
                "s" | "S" => {
                    self.save_and_sync(rpc);
                    self.confirm_discard = false;
                    return true;
                }
                "n" | "N" | "<Esc>" => {
                    self.confirm_discard = false;
                    return true;
                }
                _ => return true,
            }
        }

        if self.recording_key {
            if key == "<Esc>" {
                self.recording_key = false;
                self.status_msg = "Canceled shortcut recording".to_string();
                return true;
            }
            if let Some((_, field)) = KEYBINDING_ROWS.get(self.selected_row) {
                if self.config.keybindings.has_conflict(field, key) {
                    self.status_msg = format!("Conflict: {} is already bound", key);
                } else {
                    self.config.keybindings.set_by_field(field, key.to_string());
                    self.status_msg = format!("Bound {} to {}", field, key);
                }
            }
            self.recording_key = false;
            return true;
        }

        if self.dropdown_open {
            match key {
                "<Home>" => { self.dropdown_idx = 0; return true; }
                "<End>" => { self.dropdown_idx = self.dropdown_options().len().saturating_sub(1); return true; }
                "<Esc>" => {
                    self.dropdown_open = false;
                    return true;
                }
                "j" | "<Down>" => {
                    let max_len = if self.active_tab == 0 {
                        3
                    } else if self.active_tab == 1 && self.selected_row == 0 {
                        THEME_OPTIONS.len()
                    } else {
                        3
                    };
                    if self.dropdown_idx + 1 < max_len {
                        self.dropdown_idx += 1;
                    }
                    return true;
                }
                "k" | "<Up>" => {
                    self.dropdown_idx = self.dropdown_idx.saturating_sub(1);
                    return true;
                }
                "<CR>" => {
                    if self.active_tab == 0 && self.selected_row == 1 {
                        let modes = ["normal", "ide", "zen"];
                        if let Some(&m) = modes.get(self.dropdown_idx) {
                            self.config.mode = m.to_string();
                        }
                    } else if self.active_tab == 1 && self.selected_row == 0 {
                        if let Some(&(_, id)) = THEME_OPTIONS.get(self.dropdown_idx) {
                            self.config.theme = id.to_string();
                        }
                    } else if self.active_tab == 1 && self.selected_row == 1 {
                        let seps = ["thin", "thick", "double"];
                        if let Some(&s) = seps.get(self.dropdown_idx) {
                            self.config.separator = s.to_string();
                        }
                    }
                    self.dropdown_open = false;
                    return true;
                }
                _ => return true,
            }
        }

        match key {
            "/" => { self.is_searching = true; true }
            "<Esc>" | "q" => {
                if self.is_dirty() {
                    self.confirm_discard = true;
                } else {
                    self.is_open = false;
                }
                true
            }
            "<C-s>" => {
                self.save_and_sync(rpc);
                true
            }
            "1" | "2" | "3" | "4" | "5" | "6" => {
                self.active_tab = (key.as_bytes()[0] - b'1') as usize;
                self.selected_row = 0;
                self.search_query.clear();
                true
            }
            "<Tab>" | "<S-Tab>" => {
                self.focus_sidebar = !self.focus_sidebar;
                true
            }
            "l" | "<Right>" => {
                self.focus_sidebar = false;
                true
            }
            "h" | "<Left>" => {
                self.focus_sidebar = true;
                true
            }
            "j" | "<Down>" => {
                if self.focus_sidebar {
                    self.search_query.clear();
                    if self.active_tab + 1 < SETTINGS_TABS.len() {
                        self.active_tab += 1;
                        self.selected_row = 0;
                    }
                } else {
                    if let Some(next) = self.visible_rows().into_iter().find(|&i| i > self.selected_row) {
                        self.selected_row = next;
                    }
                }
                true
            }
            "k" | "<Up>" => {
                if self.focus_sidebar {
                    self.search_query.clear();
                    self.active_tab = self.active_tab.saturating_sub(1);
                    self.selected_row = 0;
                } else {
                    if let Some(prev) = self.visible_rows().into_iter().rev().find(|&i| i < self.selected_row) {
                        self.selected_row = prev;
                    }
                }
                true
            }
            "p" if self.active_tab == 4 => {
                self.config.keybindings = Keybindings::familiar_preset();
                self.status_msg = "Applied Familiar preset".to_string();
                true
            }
            "v" if self.active_tab == 4 => {
                self.config.keybindings = Keybindings::vim_safe_preset();
                self.status_msg = "Applied Vim-safe preset".to_string();
                true
            }
            "r" if self.active_tab == 4 => {
                self.config.keybindings = Keybindings::default();
                self.status_msg = "Reset keybindings to defaults".to_string();
                true
            }
            "<CR>" | " " => {
                if self.focus_sidebar {
                    self.focus_sidebar = false;
                } else {
                    self.activate_selected_row();
                }
                true
            }
            _ => true,
        }
    }

    pub fn activate_selected_row(&mut self) {
        if !self.visible_rows().contains(&self.selected_row) {
            return;
        }
        match self.active_tab {
            0 => match self.selected_row {
                0 => self.config.clipboard = !self.config.clipboard,
                1 => {
                    self.dropdown_open = true;
                    self.dropdown_idx = match self.config.mode.as_str() {
                        "normal" => 0,
                        "ide" => 1,
                        "zen" => 2,
                        _ => 1,
                    };
                }
                2 => self.config.autocomplete = !self.config.autocomplete,
                3 => self.config.autoindent = !self.config.autoindent,
                _ => {}
            },
            1 => match self.selected_row {
                0 => {
                    self.dropdown_open = true;
                    self.dropdown_idx = THEME_OPTIONS
                        .iter()
                        .position(|(_, id)| *id == self.config.theme)
                        .unwrap_or(1);
                }
                1 => {
                    self.dropdown_open = true;
                    self.dropdown_idx = match self.config.separator.as_str() {
                        "thin" => 0,
                        "thick" => 1,
                        "double" => 2,
                        _ => 0,
                    };
                }
                2 => self.config.nerd_fonts = !self.config.nerd_fonts,
                _ => {}
            },
            2 => match self.selected_row {
                0 => {
                    self.config.numbers = match self.config.numbers.as_str() {
                        "relative" => "absolute".to_string(),
                        "absolute" => "none".to_string(),
                        _ => "relative".to_string(),
                    };
                }
                1 => {
                    self.config.tabstop = match self.config.tabstop {
                        2 => 4,
                        4 => 8,
                        _ => 2,
                    };
                }
                _ => {}
            },
            3 => match self.selected_row {
                0 => self.config.mason_enabled = !self.config.mason_enabled,
                1 => self.config.lazy_enabled = !self.config.lazy_enabled,
                2 => {
                    self.is_open = false;
                    self.open_mason_requested = true;
                }
                3 => {
                    self.is_open = false;
                    self.open_lazy_requested = true;
                }
                4 => {
                    self.is_open = false;
                    self.open_shop_requested = true;
                }
                _ => {}
            },
            4 => {
                self.recording_key = true;
                self.status_msg = "Press new shortcut key (Esc to cancel)...".to_string();
            }
            _ => {}
        }
    }

    fn rows_for_tab(&self, tab: usize) -> Vec<SettingRow> {
        let checked = |value| if value { "[x]".to_string() } else { "[ ]".to_string() };
        let row = |label, value, description| SettingRow { label, value, description };
        match tab {
            0 => vec![
                row("System Clipboard", checked(self.config.clipboard), "Share copied text with other applications."),
                row("Mode:", format!("{} ▾", self.config.mode), "Choose Normal, IDE, or a distraction-free Zen workspace."),
                row("Autocomplete", checked(self.config.autocomplete), "Suggest completions as you type."),
                row("Autoindent", checked(self.config.autoindent), "Carry indentation onto new lines."),
            ],
            1 => {
                let label = THEME_OPTIONS.iter().find(|(_, id)| *id == self.config.theme)
                    .map(|(label, _)| *label).unwrap_or(&self.config.theme);
                vec![
                    row("Theme:", format!("{} ▾", label), "Colors for the editor, panels, selections, and dialogs."),
                    row("Split Separator:", format!("{} ▾", self.config.separator), "Choose how editor and terminal splits are separated."),
                    row("Use Nerd Fonts (Icons)", checked(self.config.nerd_fonts), "Enable icons when your terminal font supports them."),
                ]
            }
            2 => vec![
                row("Line Numbers:", self.config.numbers.clone(), "Cycle between relative, absolute, and hidden line numbers."),
                row("Tab Size:", self.config.tabstop.to_string(), "Number of columns used to display a tab."),
            ],
            3 => vec![
                row("Enable Mason (LSP/Tools)", checked(self.config.mason_enabled), "Manage language servers, formatters, linters, and debuggers."),
                row("Enable Lazy.nvim", checked(self.config.lazy_enabled), "Load your configured Neovim plugins at startup."),
                row("Mason Settings...", "Open →".into(), "Choose and install language tools for your projects."),
                row("Plugin Manager...", "Open →".into(), "Inspect loaded plugins, update them, or run a sync."),
                row("Installed Plugins...", "Open →".into(), "Configure, enable, or remove extensions; discover new ones."),
            ],
            4 => KEYBINDING_ROWS.iter().map(|(label, field)| row(*label,
                self.config.keybindings.get_by_field(field).to_string(), "Press Enter to record a shortcut.")).collect(),
            _ => vec![row("Tuim", env!("CARGO_PKG_VERSION").into(), "A terminal workspace for editing, projects, and tools.")],
        }
    }

    fn visible_rows(&self) -> Vec<usize> {
        let query = self.search_query.to_lowercase();
        self.rows_for_tab(self.active_tab).iter().enumerate().filter(|(_, row)| {
            query.is_empty() || format!("{} {} {}", row.label, row.description, SETTINGS_TABS[self.active_tab])
                .to_lowercase().contains(&query)
        }).map(|(i, _)| i).collect()
    }

    fn find_search_match(&mut self) {
        let query = self.search_query.to_lowercase();
        for tab in 0..SETTINGS_TABS.len() {
            if let Some((i, _)) = self.rows_for_tab(tab).iter().enumerate().find(|(_, row)| {
                format!("{} {} {}", row.label, row.description, SETTINGS_TABS[tab]).to_lowercase().contains(&query)
            }) {
                self.active_tab = tab;
                self.selected_row = i;
                break;
            }
        }
    }

    fn row_geometry(&self, layout: &ManagerLayout) -> (Rect, u16, usize) {
        let content = layout.content;
        let header_h = if content.h >= 5 { 2 } else { 1 };
        let list = Rect::new(content.x, content.y + header_h, content.w, content.h.saturating_sub(header_h));
        let count = self.visible_rows().len().max(1) as u16;
        let stride = if self.active_tab == 4 { 1 }
            else if list.h >= count * 5 && list.w >= 48 { 5 }
            else if list.h >= count * 3 && list.w >= 30 { 3 }
            else { 1 };
        let visible = (list.h / stride).max(1) as usize;
        let position = self.visible_rows().iter().position(|&i| i == self.selected_row).unwrap_or(0);
        let start = position.saturating_sub(visible - 1);
        (list, stride, start)
    }

    fn footer_actions(layout: &ManagerLayout) -> &'static [&'static str] {
        if layout.footer.w >= 46 { &["[ Revert ]", "[ Save Ctrl+S ]", "[ Cancel ]"] }
        else { &["[Save]", "[Cancel]"] }
    }

    fn dropdown_options(&self) -> Vec<&str> {
        if self.active_tab == 0 { vec!["Normal", "IDE", "Zen"] }
        else if self.selected_row == 0 { THEME_OPTIONS.iter().map(|(label, _)| *label).collect() }
        else { vec!["Thin", "Thick", "Double"] }
    }

    fn dropdown_rect(&self, screen: Rect) -> Rect {
        let layout = ManagerLayout::new(screen);
        let (list, stride, start) = self.row_geometry(&layout);
        let position = self.visible_rows().iter().position(|&i| i == self.selected_row).unwrap_or(start);
        let card = Self::row_rect(list, stride, position.saturating_sub(start));
        let inset = if stride == 5 { 2 } else { 1 };
        let anchor_y = card.y + u16::from(stride == 5);
        let bounds = layout.outer;
        let top = bounds.y;
        let bottom = bounds.y + bounds.h;
        let below = bottom.saturating_sub(anchor_y + 1);
        let above = anchor_y.saturating_sub(top);
        let options = self.dropdown_options();
        let wanted_h = options.len() as u16 + 2;
        let open_below = below >= wanted_h || below >= above;
        let h = wanted_h.min(if open_below { below } else { above });
        let w = (options.iter().map(|label| label.chars().count() as u16).max().unwrap_or(0) + 4)
            .max(16).min(bounds.w);
        let right = card.x + card.w.saturating_sub(inset);
        let x = right.saturating_sub(w).max(bounds.x).min(bounds.x + bounds.w - w);
        let y = if open_below { anchor_y + 1 } else { anchor_y.saturating_sub(h) };
        Rect::new(x, y.min(bottom), w, h)
    }

    fn row_rect(list: Rect, stride: u16, position: usize) -> Rect {
        Rect::new(list.x, list.y + position as u16 * stride, list.w,
            if stride > 1 { stride - 1 } else { 1 })
    }

    fn dropdown_rows(&self, screen: Rect) -> (Rect, usize) {
        let r = self.dropdown_rect(screen);
        let rows = Rect::new(r.x + 1, r.y + 1, r.w.saturating_sub(2), r.h.saturating_sub(2));
        let start = self.dropdown_idx.saturating_sub((rows.h as usize).saturating_sub(1));
        (rows, start)
    }

    /// Match hover to clickable geometry without moving keyboard focus or
    /// changing settings as the pointer crosses a control.
    pub fn draw_hover(&self, renderer: &mut Renderer, screen: Rect, mx: u16, my: u16, theme: &Theme) {
        let layout = ManagerLayout::new(screen);
        if !self.is_open || layout.outer.w < 16 || layout.outer.h < 8 { return; }
        let c = chrome(theme);
        if self.confirm_discard {
            let r = Self::discard_rect(screen);
            for (offset, width) in [(2, 11), (16, 8), (27, 10)] {
                let action = Rect::new(r.x + offset, r.y + r.h.saturating_sub(3), width, 1);
                if action.contains(mx, my) { renderer.highlight_rect(action, c.row_hover_bg); }
            }
            return;
        }
        if self.dropdown_open {
            let (rows, start) = self.dropdown_rows(screen);
            if rows.contains(mx, my) && start + ((my - rows.y) as usize) < self.dropdown_options().len() {
                renderer.highlight_rect(Rect::new(rows.x, my, rows.w, 1), c.row_hover_bg);
            }
            return;
        }
        let actions = Self::footer_actions(&layout);
        for i in 0..actions.len() {
            let action = layout.action_rect(actions, i);
            if action.contains(mx, my) {
                renderer.highlight_rect(action, c.row_hover_bg);
                return;
            }
        }
        if layout.search.contains(mx, my) {
            renderer.highlight_rect(layout.search, c.row_hover_bg);
            return;
        }
        if layout.nav.contains(mx, my) {
            let start = self.active_tab.saturating_sub(layout.nav.h.saturating_sub(1) as usize);
            if start + ((my - layout.nav.y) as usize) < SETTINGS_TABS.len() {
                renderer.highlight_rect(Rect::new(layout.nav.x, my, layout.nav.w.saturating_sub(1), 1), c.row_hover_bg);
            }
            return;
        }
        let (list, stride, start) = self.row_geometry(&layout);
        if list.contains(mx, my) {
            let position = ((my - list.y) / stride) as usize;
            let card = Self::row_rect(list, stride, position);
            if position < (list.h / stride) as usize && card.contains(mx, my)
                && start + position < self.visible_rows().len() {
                renderer.highlight_rect(card, c.row_hover_bg);
            }
        }
    }

    fn discard_rect(screen: Rect) -> Rect { primitives::centered_rect(screen, 48, 8) }

    pub fn handle_mouse(
        &mut self, screen: Rect, mx: u16, my: u16, button: u8, pressed: bool, rpc: &mut RpcClient,
    ) -> bool {
        if !self.is_open || !pressed { return false; }
        let layout = ManagerLayout::new(screen);
        if self.confirm_discard {
            if button == 0 {
                let r = Self::discard_rect(screen);
                if my == r.y + r.h.saturating_sub(3) {
                    let x = mx.saturating_sub(r.x);
                    let key = if (2..13).contains(&x) { Some("y") }
                        else if (16..24).contains(&x) { Some("s") }
                        else if (27..37).contains(&x) { Some("n") } else { None };
                    if let Some(key) = key { self.handle_key(&InputEvent::Key(key.into()), rpc); }
                }
            }
            return true;
        }
        if self.dropdown_open {
            let (rows, start) = self.dropdown_rows(screen);
            if button == 64 || button == 65 {
                self.handle_key(&InputEvent::Key(if button == 64 { "<Up>" } else { "<Down>" }.into()), rpc);
            } else if button == 0 {
                if rows.contains(mx, my) {
                    let index = start + (my - rows.y) as usize;
                    if index < self.dropdown_options().len() {
                        self.dropdown_idx = index;
                        self.handle_key(&InputEvent::Key("<CR>".into()), rpc);
                    }
                } else { self.dropdown_open = false; }
            }
            return true;
        }
        if button == 64 || button == 65 {
            self.focus_sidebar = layout.nav.contains(mx, my);
            self.handle_key(&InputEvent::Key(if button == 64 { "<Up>" } else { "<Down>" }.into()), rpc);
            return true;
        }
        if button != 0 { return true; }
        if !layout.outer.contains(mx, my) {
            self.handle_key(&InputEvent::Key("<Esc>".into()), rpc);
            return true;
        }
        let actions = Self::footer_actions(&layout);
        for i in 0..actions.len() {
            if layout.action_rect(actions, i).contains(mx, my) {
                if i == actions.len() - 1 { self.handle_key(&InputEvent::Key("<Esc>".into()), rpc); }
                else if i == actions.len() - 2 { self.save_and_sync(rpc); }
                else { self.config = self.saved_config.clone(); self.status_msg = "Unsaved changes reverted.".into(); }
                return true;
            }
        }
        if layout.search.contains(mx, my) { self.is_searching = true; return true; }
        if layout.nav.contains(mx, my) {
            let start = self.active_tab.saturating_sub(layout.nav.h.saturating_sub(1) as usize);
            let index = start + (my - layout.nav.y) as usize;
            if index < SETTINGS_TABS.len() {
                self.active_tab = index;
                self.selected_row = 0;
                self.search_query.clear();
                self.is_searching = false;
                self.focus_sidebar = true;
            }
            return true;
        }
        let (list, stride, start) = self.row_geometry(&layout);
        if list.contains(mx, my) {
            let position = ((my - list.y) / stride) as usize;
            if position >= (list.h / stride) as usize
                || !Self::row_rect(list, stride, position).contains(mx, my) { return true; }
            if let Some(&row) = self.visible_rows().get(start + position) {
                self.selected_row = row;
                self.focus_sidebar = false;
                self.is_searching = false;
                self.activate_selected_row();
            }
        }
        true
    }

    pub fn draw(&self, renderer: &mut Renderer, screen: Rect, theme: &Theme) {
        if !self.is_open { return; }
        let layout = ManagerLayout::new(screen);
        let c = chrome(theme);
        layout.draw(renderer, if self.is_dirty() { "Settings [User] · Unsaved" } else { "Settings [User]" }, theme);
        if layout.outer.w < 16 || layout.outer.h < 8 { return; }
        primitives::draw_text_input(renderer, layout.search, &self.search_query,
            "/ Search settings...", self.is_searching, theme);
        if layout.nav.w > 0 {
            let start = self.active_tab.saturating_sub(layout.nav.h.saturating_sub(1) as usize);
            for (i, label) in SETTINGS_TABS.iter().enumerate().skip(start).take(layout.nav.h as usize) {
                let active = i == self.active_tab;
                let bg = if active { c.row_selected_bg } else { c.panel_bg };
                let row = Rect::new(layout.nav.x, layout.nav.y + (i - start) as u16, layout.nav.w.saturating_sub(1), 1);
                renderer.fill_rect(row, bg);
                text_in(renderer, row, &format!("{} {}", if active { ">" } else { " " }, label),
                    c.text_strong, bg, active && self.focus_sidebar);
            }
        }
        let title = if self.active_tab == 4 { "Keybindings / Enter to record" } else { SETTINGS_TABS[self.active_tab] };
        text_in(renderer, Rect::new(layout.content.x, layout.content.y, layout.content.w, 1), title, c.text_strong, c.panel_bg, true);
        let rows = self.rows_for_tab(self.active_tab);
        let indices = self.visible_rows();
        let (list, stride, start) = self.row_geometry(&layout);
        if indices.is_empty() {
            text_in(renderer, list, "No matching settings", c.text_muted, c.panel_bg, false);
        }
        for (position, &index) in indices.iter().skip(start).take((list.h / stride) as usize).enumerate() {
            let row = &rows[index];
            let selected = index == self.selected_row && !self.focus_sidebar;
            let bg = if selected { c.row_selected_bg } else { c.surface_bg };
            let card = Self::row_rect(list, stride, position);
            renderer.fill_rect(card, bg);
            let inset = if stride == 5 { 2 } else { 1 };
            if stride == 5 {
                primitives::draw_rounded_border(renderer, card, None, c.border, bg);
            }
            let y = card.y + u16::from(stride == 5);
            let inner_w = card.w.saturating_sub(inset * 2);
            let value_w = (row.value.chars().count() as u16).min(inner_w / 2);
            let label_w = if value_w > 0 { inner_w.saturating_sub(value_w + 2) } else { inner_w };
            text_in(renderer, Rect::new(card.x + inset, y, label_w, 1), row.label, c.text_strong, bg, selected);
            text_in(renderer, Rect::new(card.x + card.w.saturating_sub(inset + value_w), y, value_w, 1),
                &row.value, if selected { c.text_strong } else { c.accent }, bg, selected);
            if stride > 1 {
                text_in(renderer, Rect::new(card.x + inset, y + 1, inner_w, 1), row.description, c.text_muted, bg, false);
            }
        }
        let hint = if !self.status_msg.is_empty() { self.status_msg.as_str() }
            else if self.active_tab == 4 { "p Familiar  v Vim-safe  r Reset" }
            else { "↑↓ Navigate   Tab Next pane   Enter Edit   Esc Close" };
        layout.draw_footer(renderer, hint, Self::footer_actions(&layout), theme);
        if self.dropdown_open {
            let r = self.dropdown_rect(screen);
            primitives::draw_modal_frame(renderer, r, "", theme);
            let (rows, start) = self.dropdown_rows(screen);
            for (i, label) in self.dropdown_options().iter().enumerate().skip(start).take(rows.h as usize) {
                let selected = i == self.dropdown_idx;
                let bg = if selected { c.row_selected_bg } else { c.panel_bg };
                let row = Rect::new(rows.x, rows.y + (i - start) as u16, rows.w, 1);
                renderer.fill_rect(row, bg);
                text_in(renderer, Rect::new(row.x + 1, row.y, row.w.saturating_sub(2), 1), label, c.text_strong, bg, selected);
            }
        }
        if self.confirm_discard {
            let r = Self::discard_rect(screen);
            primitives::draw_modal_frame(renderer, r, "Unsaved Settings", theme);
            text_in(renderer, Rect::new(r.x + 2, r.y + 2, r.w.saturating_sub(4), 1),
                "Discard unsaved settings changes?", c.text_strong, c.panel_bg, false);
            text_in(renderer, Rect::new(r.x + 2, r.y + r.h.saturating_sub(3), r.w.saturating_sub(4), 1),
                "[Y] Discard    [S] Save   [N] Cancel", c.accent, c.panel_bg, false);
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_preserves_saved_bindings_and_avoids_new_conflicts() {
        let dir = std::env::temp_dir().join(format!("tuim-settings-migration-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"keybindings":{"close_buffer":"<S-F6>","save_file":"<M-Left>","report_bug":"","focus_next":"<F8>"}}"#).unwrap();
        let cfg = SettingsConfig::load(path.to_str().unwrap()).unwrap();
        assert_eq!(cfg.keybindings.close_buffer, "<S-F6>");
        assert_eq!(cfg.keybindings.save_file, "<M-Left>");
        assert_eq!(cfg.keybindings.focus_next, "<F8>");
        assert!(cfg.keybindings.focus_prev.is_empty());
        assert!(cfg.keybindings.resize_left.is_empty());
        assert!(cfg.keybindings.report_bug.is_empty());
        assert_eq!(cfg.keybindings.resize_right, "<M-Right>");
        cfg.save(path.to_str().unwrap()).unwrap();
        assert_eq!(SettingsConfig::load(path.to_str().unwrap()).unwrap().keybindings.close_buffer, "<S-F6>");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn presets_are_conflict_free_and_vim_safe_frees_control_keys() {
        let default = Keybindings::default();
        assert_eq!(default.toggle_explorer, "<C-e>");
        assert_eq!(default.report_bug, "<F12>");
        for kb in [default, Keybindings::vim_safe_preset(), Keybindings::familiar_preset()] {
            for (field, key) in kb.all_fields() { assert!(!kb.has_conflict(field, key), "{field}: {key}"); }
        }
        for (_, key) in Keybindings::vim_safe_preset().all_fields() { assert!(!key.starts_with("<C-")); }
    }

    #[test]
    fn dropdown_tracks_its_field_and_stays_inside_resized_dialog() {
        let mut widget = SettingsWidget::new("", SettingsConfig::default());
        widget.is_open = true;
        widget.selected_row = 1;
        for (width, height) in [(190, 55), (100, 30), (60, 20), (35, 10), (18, 8)] {
            let screen = Rect::new(0, 0, width, height);
            let layout = ManagerLayout::new(screen);
            let popup = widget.dropdown_rect(screen);
            assert!(layout.outer.contains(popup.x, popup.y));
            assert!(layout.outer.contains(popup.x + popup.w - 1, popup.y + popup.h - 1));
            if width >= 60 {
                let (list, stride, start) = widget.row_geometry(&layout);
                let card = SettingsWidget::row_rect(list, stride, 1 - start);
                let inset = if stride == 5 { 2 } else { 1 };
                assert_eq!(popup.x + popup.w, card.x + card.w - inset);
                assert_eq!(popup.y, card.y + u16::from(stride == 5) + 1);
            }
        }
        // In a short terminal, use the extra space above the mode field.
        let screen = Rect::new(0, 0, 35, 11);
        let layout = ManagerLayout::new(screen);
        let (list, stride, start) = widget.row_geometry(&layout);
        let card = SettingsWidget::row_rect(list, stride, 1 - start);
        let popup = widget.dropdown_rect(screen);
        assert!(popup.y < card.y);
        assert_eq!(popup.y + popup.h, card.y);
    }

    #[test]
    fn hover_highlights_card_padding_without_changing_selection() {
        let mut widget = SettingsWidget::new("", SettingsConfig::default());
        widget.is_open = true;
        let screen = Rect::new(0, 0, 190, 55);
        let theme = Theme::default();
        let mut renderer = Renderer::new(screen.w, screen.h);
        widget.draw(&mut renderer, screen, &theme);
        let original = renderer.buf.clone();
        let (list, stride, _) = widget.row_geometry(&ManagerLayout::new(screen));
        let card = SettingsWidget::row_rect(list, stride, 1);
        widget.draw_hover(&mut renderer, screen, card.x + card.w / 2, card.y + 2, &theme);
        for y in card.y..card.y + card.h {
            for x in card.x..card.x + card.w {
                let index = y as usize * screen.w as usize + x as usize;
                assert_eq!(renderer.buf[index].bg, chrome(&theme).row_hover_bg);
                assert_eq!(renderer.buf[index].ch, original[index].ch);
            }
        }
        assert_eq!(widget.selected_row, 0);
        assert!(widget.focus_sidebar);
        assert!(!widget.is_dirty());
        assert!(!widget.dropdown_open);
        widget.draw(&mut renderer, screen, &theme);
        widget.draw_hover(&mut renderer, screen, card.x, card.y + card.h, &theme);
        // The gap below a card is not interactive.
        for y in card.y..card.y + card.h {
            let index = y as usize * screen.w as usize + card.x as usize;
            assert_eq!(renderer.buf[index].bg, original[index].bg);
        }
    }

    #[test]
    fn settings_content_stays_inside_dialog_when_resized() {
        for (width, height) in [(190, 55), (100, 30), (60, 20), (35, 10), (18, 8)] {
            for tab in 0..SETTINGS_TABS.len() {
                let mut widget = SettingsWidget::new("", SettingsConfig::default());
                widget.is_open = true;
                widget.active_tab = tab;
                widget.focus_sidebar = false;
                widget.selected_row = widget.rows_for_tab(tab).len() - 1;
                let mut renderer = Renderer::new(width, height);
                let screen = Rect::new(0, 0, width, height);
                widget.draw(&mut renderer, screen, &Theme::default());
                let outer = ManagerLayout::new(screen).outer;
                for y in 0..height {
                    for x in 0..width {
                        if !outer.contains(x, y) {
                            assert_eq!(renderer.buf[(y * width + x) as usize].ch[0], b' ',
                                "Settings tab {tab} escaped its frame at {width}x{height}: {x},{y}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_settings_roundtrip_and_version_validation() {
        let dir = std::env::temp_dir().join(format!("tuim_settings_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let path_str = path.to_string_lossy().to_string();

        let mut cfg = SettingsConfig::default();
        cfg.theme = "tokyonight".to_string();
        cfg.keybindings.close_buffer = "<C-k>".to_string();
        cfg.save(&path_str).unwrap();

        let loaded = SettingsConfig::load(&path_str).unwrap();
        assert_eq!(loaded.theme, "tokyonight");
        assert_eq!(loaded.keybindings.close_buffer, "<C-k>");

        // Version > 1 should fail
        fs::write(&path, r#"{"version": 99}"#).unwrap();
        assert!(SettingsConfig::load(&path_str).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_keybinding_conflict_detection() {
        let kb = Keybindings::default();
        assert!(kb.has_conflict("close_buffer", "<C-s>"));
        assert!(!kb.has_conflict("save_file", "<C-s>"));
        assert!(!kb.has_conflict("close_buffer", "<C-k>"));
    }
}
