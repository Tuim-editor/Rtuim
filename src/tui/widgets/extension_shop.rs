use std::fs;
use std::process::Command;

use serde::Deserialize;

use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState, ManagerLayout, text_in};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PluginInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub stars: u64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub protected: bool,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Deserialize)]
struct StoreActionResult {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopSection {
    Installed,
    Discover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopConfirm {
    None,
    Trust,
    Uninstall,
    Restart,
}

pub const CATEGORIES: &[(&str, &str)] = &[
    ("All Categories", "all"),
    ("Colorschemes", "colorscheme"),
    ("LSP & Completion", "lsp"),
    ("Git Integrations", "git"),
    ("AI & Assistants", "ai"),
    ("Treesitter & Syntax", "treesitter"),
    ("Telescope & Pickers", "telescope"),
];

fn valid_repository(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else { return false; };
    (1..=39).contains(&owner.len()) && (1..=100).contains(&name.len())
        && owner.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && owner.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        && owner.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        && name != "." && name != ".."
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct StoreFixture(PathBuf);
    impl StoreFixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("rtuim-shop-{}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("store_search.py"), r#"import json, pathlib, sys
if sys.argv[1] == 'search':
    print('[]')
else:
    (pathlib.Path(__file__).parent / 'action.json').write_text(json.dumps(sys.argv[1:]))
    print(json.dumps({'success': True, 'message': 'Saved'}))
"#).unwrap();
            Self(path)
        }
        fn shop(&self) -> ExtensionShop {
            let mut shop = ExtensionShop::new(self.0.to_str().unwrap());
            shop.is_open = true;
            shop.detail_open = true;
            shop.plugins = vec![PluginInfo { name: "demo".into(), full_name: "owner/demo".into(), ..Default::default() }];
            shop
        }
    }
    impl Drop for StoreFixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    fn key(shop: &mut ExtensionShop, key: &str) {
        shop.handle_key(&InputEvent::Key(key.into()));
    }

    #[test]
    fn install_requires_explicit_visible_trust_and_keeps_the_selected_identity() {
        let fixture = StoreFixture::new();
        let mut shop = fixture.shop();
        key(&mut shop, "<CR>");
        assert_eq!(shop.confirm_modal, ShopConfirm::Trust);
        key(&mut shop, "y"); // Warning has not been rendered yet.
        assert!(!fixture.0.join("action.json").exists());
        let mut renderer = Renderer::new(110, 36);
        shop.draw_confirmation(&mut renderer, Rect::new(0, 0, 110, 36), &Theme::default());
        key(&mut shop, "<CR>"); // Repeated Enter must not consent.
        assert!(!fixture.0.join("action.json").exists());
        key(&mut shop, "<Esc>");
        assert_eq!(shop.confirm_modal, ShopConfirm::None);
        assert!(!fixture.0.join("action.json").exists());
        key(&mut shop, "i");
        shop.draw_confirmation(&mut renderer, Rect::new(0, 0, 35, 10), &Theme::default());
        key(&mut shop, "y");
        assert!(!fixture.0.join("action.json").exists());
        shop.draw_confirmation(&mut renderer, Rect::new(0, 0, 110, 36), &Theme::default());
        shop.plugins[0].full_name = "other/changed".into();
        key(&mut shop, "y");
        let args: Vec<String> = serde_json::from_str(&fs::read_to_string(fixture.0.join("action.json")).unwrap()).unwrap();
        assert_eq!(args, ["add", "owner/demo"]);
        assert_eq!(shop.confirm_modal, ShopConfirm::Restart);
        assert!(shop.needs_restart);
        assert!(!shop.restart_requested);
    }

    #[test]
    fn enable_requires_trust_and_mouse_cancel_does_not_write() {
        let fixture = StoreFixture::new();
        let mut shop = fixture.shop();
        shop.plugins[0].installed = true;
        shop.plugins[0].enabled = false;
        key(&mut shop, "d");
        assert_eq!(shop.confirm_modal, ShopConfirm::Trust);
        let screen = Rect::new(0, 0, 110, 36);
        let mut renderer = Renderer::new(screen.w, screen.h);
        shop.draw_confirmation(&mut renderer, screen, &Theme::default());
        let rect = shop.confirmation_rect(screen).unwrap();
        shop.handle_confirmation_mouse(screen, rect.x + 21, rect.y + rect.h - 3, 0, true);
        assert_eq!(shop.confirm_modal, ShopConfirm::None);
        assert!(!fixture.0.join("action.json").exists());
        key(&mut shop, "d");
        shop.draw_confirmation(&mut renderer, screen, &Theme::default());
        let rect = shop.confirmation_rect(screen).unwrap();
        shop.handle_confirmation_mouse(screen, rect.x + 4, rect.y + rect.h - 3, 0, true);
        let args: Vec<String> = serde_json::from_str(&fs::read_to_string(fixture.0.join("action.json")).unwrap()).unwrap();
        assert_eq!(args, ["enable", "owner/demo"]);
    }

    #[test]
    fn untrusted_repository_cannot_become_a_path_or_lua_template() {
        let fixture = StoreFixture::new();
        for repo in ["../demo", "owner/..", "owner/a/b", "owner/\";error()", "owñer/demo", "local:evil", "owner/demo\n"] {
            let mut shop = fixture.shop();
            shop.plugins[0].full_name = repo.into();
            key(&mut shop, "i");
            assert_eq!(shop.confirm_modal, ShopConfirm::None);
            shop.plugins[0].installed = true;
            key(&mut shop, "e");
            assert!(shop.edit_config_path.is_none());
            assert!(!shop.run_store_action("add", repo).0);
        }
        assert!(!fixture.0.join("action.json").exists());
        assert!(!fixture.0.join("plugin_configs").exists());
    }

    #[test]
    fn trust_warning_displays_the_entire_repository_without_truncation() {
        let fixture = StoreFixture::new();
        let mut shop = fixture.shop();
        let repo = format!("{}/{}", "a".repeat(39), "b".repeat(100));
        shop.request_trust(&repo, "add");
        let lines = shop.trust_lines(34);
        let source: String = lines.iter().take_while(|line| !line.is_empty()).cloned().collect();
        assert_eq!(source, format!("https://github.com/{}", repo));
        assert!(shop.confirmation_rect(Rect::new(0, 0, 42, 12)).is_none());
        assert!(shop.confirmation_rect(Rect::new(0, 0, 110, 36)).is_some());
    }

    #[test]
    fn failed_search_clears_old_actionable_results() {
        let fixture = StoreFixture::new();
        fs::write(fixture.0.join("store_search.py"), "import sys\nprint('{\"success\":false,\"message\":\"Invalid catalog\"}')\nsys.exit(1)\n").unwrap();
        let mut shop = fixture.shop();
        shop.refresh();
        assert!(shop.plugins.is_empty());
        assert_eq!(shop.status_msg, "Invalid catalog");
    }

    #[test]
    fn one_scrolling_plugin_list_survives_resize() {
        let mut shop = ExtensionShop::new("");
        shop.is_open = true;
        shop.plugins = (0..60).map(|i| PluginInfo {
            name: format!("plugin-{i:02}"),
            full_name: format!("example/plugin-{i:02}"),
            description: "Extension description".into(),
            ..Default::default()
        }).collect();
        shop.list_state.move_end(shop.plugins.len());
        for (width, height) in [(190, 55), (80, 24), (60, 20), (35, 10)] {
            let mut renderer = Renderer::new(width, height);
            let screen = Rect::new(0, 0, width, height);
            shop.draw(&mut renderer, screen, &Theme::default());
            let text: String = renderer.buf.iter().map(|c|
                String::from_utf8_lossy(&c.ch[..c.len as usize]).into_owned()).collect();
            assert_eq!(text.matches("example/plugin-59").count(), 1,
                "Selected plugin must appear once at {width}x{height}");
            let outer = ManagerLayout::new(screen).outer;
            for y in 0..height {
                for x in 0..width {
                    if !outer.contains(x, y) {
                        assert_eq!(renderer.buf[(y * width + x) as usize].ch[0], b' ');
                    }
                }
            }
        }
    }
}

pub struct ExtensionShop {
    pub is_open: bool,
    pub section: ShopSection,
    pub category_idx: usize,
    pub search_query: String,
    pub is_searching: bool,
    pub focus_nav: bool,
    pub nav_index: usize,
    pub plugins: Vec<PluginInfo>,
    pub list_state: ListState,
    pub detail_open: bool,
    pub confirm_modal: ShopConfirm,
    pub can_confirm_modal: bool,
    pub status_msg: String,
    pub data_dir: String,
    pub needs_restart: bool,
    pub restart_requested: bool,
    pub edit_config_path: Option<String>,
    pending_trust: Option<(String, &'static str)>,
}

impl ExtensionShop {
    pub fn new(data_dir: &str) -> Self {
        Self {
            is_open: false,
            section: ShopSection::Installed,
            category_idx: 0,
            search_query: String::new(),
            is_searching: false,
            focus_nav: false,
            nav_index: 0,
            plugins: Vec::new(),
            list_state: ListState::new(),
            detail_open: false,
            confirm_modal: ShopConfirm::None,
            can_confirm_modal: true,
            status_msg: String::new(),
            data_dir: data_dir.to_string(),
            needs_restart: false,
            restart_requested: false,
            edit_config_path: None,
            pending_trust: None,
        }
    }

    fn script_path(&self) -> String {
        format!("{}/store_search.py", self.data_dir)
    }

    pub fn refresh(&mut self) {
        let cat = match self.section {
            ShopSection::Installed => "installed",
            ShopSection::Discover => CATEGORIES
                .get(self.category_idx)
                .map(|(_, id)| *id)
                .unwrap_or("all"),
        };

        let out = Command::new("python3")
            .args([&self.script_path(), "search", &self.search_query, cat])
            .output();

        self.plugins.clear();
        match out {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<Vec<PluginInfo>>(&output.stdout) {
                    Ok(parsed) => self.plugins = parsed,
                    Err(_) => self.status_msg = "Invalid extension search response".into(),
                }
            }
            Ok(output) => self.status_msg = serde_json::from_slice::<StoreActionResult>(&output.stdout)
                .map(|r| r.message).unwrap_or_else(|_| "Extension search failed".into()),
            Err(_) => self.status_msg = "Unable to run extension search".into(),
        }
        self.list_state.clamp(self.plugins.len());
    }

    pub fn run_store_action(&mut self, action: &str, full_name: &str) -> (bool, String) {
        if !valid_repository(full_name) {
            return (false, "Invalid GitHub repository identity".into());
        }
        let out = Command::new("python3")
            .args([&self.script_path(), action, full_name])
            .output();

        if let Ok(output) = out {
            if let Ok(res) = serde_json::from_slice::<StoreActionResult>(&output.stdout) {
                return (res.success, res.message);
            }
        }
        (false, "Store action failed".to_string())
    }

    pub fn selected_plugin(&self) -> Option<&PluginInfo> {
        self.plugins.get(self.list_state.selected)
    }

    fn request_trust(&mut self, repo: &str, action: &'static str) {
        if !valid_repository(repo) {
            self.status_msg = "Invalid GitHub repository identity".into();
            return;
        }
        self.pending_trust = Some((repo.to_string(), action));
        self.confirm_modal = ShopConfirm::Trust;
        // A confirmation is only actionable after the full warning is drawn.
        self.can_confirm_modal = false;
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        let key = match ev {
            InputEvent::Key(k) => k.as_str(),
            InputEvent::Paste(text) => {
                if self.is_searching && self.confirm_modal == ShopConfirm::None && !self.detail_open {
                    let remaining = 256usize.saturating_sub(self.search_query.chars().count());
                    self.search_query.extend(text.chars()
                        .filter(|ch| !crate::tui::renderer::unsafe_display_char(*ch)).take(remaining));
                    self.list_state.move_home();
                    self.refresh();
                }
                return true;
            }
            _ => return false,
        };

        if self.confirm_modal != ShopConfirm::None {
            match self.confirm_modal {
                ShopConfirm::Trust => match key {
                    "y" | "Y" if self.can_confirm_modal => {
                        if let Some((repo, action)) = self.pending_trust.take() {
                            let (ok, msg) = self.run_store_action(action, &repo);
                            self.status_msg = msg;
                            self.confirm_modal = if ok { ShopConfirm::Restart } else { ShopConfirm::None };
                            if ok { self.needs_restart = true; }
                            self.refresh();
                        }
                        return true;
                    }
                    "n" | "N" | "<Esc>" => {
                        self.pending_trust = None;
                        self.confirm_modal = ShopConfirm::None;
                        return true;
                    }
                    _ => return true, // Enter must not accept a repeated install key.
                },
                ShopConfirm::Uninstall => match key {
                    "y" | "Y" | "<CR>" => {
                        if !self.can_confirm_modal {
                            return true;
                        }
                        if let Some(p) = self.selected_plugin().cloned() {
                            let (ok, msg) = self.run_store_action("remove", &p.full_name);
                            self.status_msg = msg;
                            if ok {
                                self.needs_restart = true;
                                self.confirm_modal = ShopConfirm::Restart;
                            } else {
                                self.confirm_modal = ShopConfirm::None;
                            }
                            self.refresh();
                        } else {
                            self.confirm_modal = ShopConfirm::None;
                        }
                        return true;
                    }
                    "n" | "N" | "<Esc>" => {
                        self.confirm_modal = ShopConfirm::None;
                        return true;
                    }
                    _ => return true,
                },
                ShopConfirm::Restart => match key {
                    "y" | "Y" | "<CR>" => {
                        if !self.can_confirm_modal {
                            return true;
                        }
                        self.confirm_modal = ShopConfirm::None;
                        self.restart_requested = true;
                        return true;
                    }
                    "n" | "N" | "<Esc>" => {
                        self.confirm_modal = ShopConfirm::None;
                        return true;
                    }
                    _ => return true,
                },
                ShopConfirm::None => {}
            }
        }

        if self.detail_open {
            match key {
                "<Esc>" | "q" => {
                    self.detail_open = false;
                    return true;
                }
                "<CR>" | "i" | "I" => {
                    if let Some(p) = self.selected_plugin().cloned() {
                        if !p.installed || p.status == "pending_removal" {
                            self.request_trust(&p.full_name, "add");
                        }
                    }
                    return true;
                }
                "e" | "E" => {
                    if let Some(p) = self.selected_plugin().cloned() {
                        if p.installed && p.status != "pending_removal" {
                            if !valid_repository(&p.full_name) {
                                self.status_msg = "Only valid GitHub repositories can be configured here".into();
                                return true;
                            }
                            let sanitized = p.full_name.replace('/', "_");
                            let dir = format!("{}/plugin_configs", self.data_dir);
                            let _ = fs::create_dir_all(&dir);
                            let path = format!("{}/{}.lua", dir, sanitized);
                            if !std::path::Path::new(&path).exists() {
                                let template = format!(
                                    "-- lazy.nvim spec override for {}\nreturn {{\n  \"{}\",\n  opts = {{}},\n}}\n",
                                    p.full_name, p.full_name
                                );
                                let _ = fs::write(&path, template);
                            }
                            self.edit_config_path = Some(path);
                            self.detail_open = false;
                            self.is_open = false;
                        }
                    }
                    return true;
                }
                "d" | "D" => {
                    if let Some(p) = self.selected_plugin().cloned() {
                        if p.installed && !p.protected && p.status != "pending_removal" {
                            if !p.enabled {
                                self.request_trust(&p.full_name, "enable");
                                return true;
                            }
                            let (ok, msg) = self.run_store_action("disable", &p.full_name);
                            self.status_msg = msg;
                            if ok {
                                self.needs_restart = true;
                                self.confirm_modal = ShopConfirm::Restart;
                            }
                            self.refresh();
                        }
                    }
                    return true;
                }
                "u" | "U" | "x" => {
                    if let Some(p) = self.selected_plugin().cloned() {
                        if p.installed && !p.protected && p.status != "pending_removal" {
                            self.confirm_modal = ShopConfirm::Uninstall;
                        }
                    }
                    return true;
                }
                _ => return true,
            }
        }

        if self.is_searching {
            match key {
                "<Esc>" | "<CR>" => {
                    self.is_searching = false;
                    return true;
                }
                "<BS>" => {
                    self.search_query.pop();
                    self.list_state.move_home();
                    self.refresh();
                    return true;
                }
                k if k.chars().count() == 1 && !k.starts_with('<') => {
                    if self.search_query.chars().count() < 256
                        && !k.chars().any(crate::tui::renderer::unsafe_display_char) {
                        self.search_query.push_str(k);
                    }
                    self.list_state.move_home();
                    self.refresh();
                    return true;
                }
                _ => return true,
            }
        }


        match key {
            "<Tab>" | "<S-Tab>" => {
                self.focus_nav = !self.focus_nav;
                true
            }
            "<Left>" => { self.focus_nav = true; true }
            "<Right>" => { self.focus_nav = false; true }
            "<Home>" => { self.focus_nav = false; self.list_state.move_home(); true }
            "<End>" => { self.focus_nav = false; self.list_state.move_end(self.plugins.len()); true }
            "<PageDown>" => { self.list_state.move_down(self.plugins.len(), 8); true }
            "<PageUp>" => { self.list_state.move_up(8); true }
            "1" => {
                self.section = ShopSection::Installed;
                self.nav_index = 0;
                self.focus_nav = false;
                self.list_state.move_home();
                self.search_query.clear();
                self.refresh();
                true
            }
            "2" => {
                self.section = ShopSection::Discover;
                self.nav_index = 1;
                self.focus_nav = false;
                self.list_state.move_home();
                self.refresh();
                true
            }
            "c" => {
                self.section = ShopSection::Discover;
                self.focus_nav = true;
                self.nav_index = 2 + self.category_idx;
                self.refresh();
                true
            }
            "/" | "f" => {
                self.is_searching = true;
                true
            }
            "j" | "<Down>" => {
                if self.focus_nav {
                    self.select_nav((self.nav_index + 1).min(CATEGORIES.len() + 1));
                } else {
                    self.list_state.move_down(self.plugins.len(), 1);
                }
                true
            }
            "k" | "<Up>" => {
                if self.focus_nav {
                    self.select_nav(self.nav_index.saturating_sub(1));
                } else {
                    self.list_state.move_up(1);
                }
                true
            }
            "<CR>" => {
                if self.focus_nav {
                    self.focus_nav = false;
                } else if !self.plugins.is_empty() {
                    self.detail_open = true;
                }
                true
            }
            "r" | "R" => {
                let _ = Command::new("python3")
                    .args([&self.script_path(), "download"])
                    .output();
                self.refresh();
                true
            }
            _ => false,
        }
    }

    fn select_nav(&mut self, index: usize) {
        self.nav_index = index;
        self.section = if index == 0 { ShopSection::Installed } else { ShopSection::Discover };
        if index == 0 { self.search_query.clear(); }
        if index >= 2 { self.category_idx = index - 2; }
        self.detail_open = false;
        self.list_state.move_home();
        self.refresh();
    }

    fn list_geometry(layout: &ManagerLayout) -> (Rect, u16) {
        let r = layout.content;
        let header_h = if r.h >= 5 { 3 } else { 1 };
        let list = Rect::new(r.x, r.y + header_h, r.w, r.h.saturating_sub(header_h));
        let stride = if list.h >= 12 && r.w >= 44 { 3 } else { 1 };
        (list, stride)
    }

    fn nav_start(&self, layout: &ManagerLayout) -> usize {
        self.nav_index.saturating_sub(layout.nav.h.saturating_sub(1) as usize)
    }

    pub fn handle_mouse(&mut self, screen: Rect, mx: u16, my: u16, button: u8, pressed: bool) -> bool {
        if !pressed { return false; }
        if self.handle_confirmation_mouse(screen, mx, my, button, pressed) { return true; }
        let layout = ManagerLayout::new(screen);
        if !layout.outer.contains(mx, my) {
            if button == 0 { self.is_open = false; }
            return true;
        }
        if button == 64 || button == 65 {
            if layout.nav.contains(mx, my) {
                let index = if button == 64 { self.nav_index.saturating_sub(1) }
                    else { (self.nav_index + 1).min(CATEGORIES.len() + 1) };
                self.focus_nav = true;
                self.select_nav(index);
            } else if !self.detail_open {
                if button == 64 { self.list_state.move_up(1); }
                else { self.list_state.move_down(self.plugins.len(), 1); }
            }
            return true;
        }
        if button != 0 { return true; }
        let actions = if self.detail_open { ["[ Refresh ]", "[ Back ]"] } else { ["[ Refresh ]", "[ Close ]"] };
        if layout.action_rect(&actions, 1).contains(mx, my) {
            if self.detail_open { self.detail_open = false; } else { self.is_open = false; }
            return true;
        }
        if layout.action_rect(&actions, 0).contains(mx, my) {
            self.refresh();
            return true;
        }
        if layout.search.contains(mx, my) {
            self.is_searching = true;
            self.detail_open = false;
            return true;
        }
        if layout.nav.contains(mx, my) {
            let index = self.nav_start(&layout) + (my - layout.nav.y) as usize;
            if index < CATEGORIES.len() + 2 {
                self.focus_nav = true;
                self.select_nav(index);
            }
            return true;
        }
        if layout.content.contains(mx, my) {
            self.focus_nav = false;
            if self.detail_open {
                let r = layout.content;
                let action_y = r.y + r.h.saturating_sub(3);
                if my == action_y { self.handle_key(&InputEvent::Key("e".into())); }
                if my == action_y + 1 {
                    let key = if self.selected_plugin().is_some_and(|p| !p.installed || p.status == "pending_removal") { "<CR>" } else { "d" };
                    self.handle_key(&InputEvent::Key(key.into()));
                }
                if my == action_y + 2 { self.handle_key(&InputEvent::Key("u".into())); }
            } else {
                let (list, stride) = Self::list_geometry(&layout);
                if list.contains(mx, my) {
                    let index = self.list_state.scroll_offset + ((my - list.y) / stride) as usize;
                    if index < self.plugins.len() {
                        self.list_state.selected = index;
                        self.detail_open = true;
                    }
                }
            }
        }
        true
    }

    fn plugin_status(p: &PluginInfo) -> &'static str {
        if p.status == "pending_removal" { "Removal pending restart" }
        else if p.installed && !p.enabled { "Disabled" }
        else if p.installed { "Installed" }
        else { "Available" }
    }

    pub fn draw(&mut self, renderer: &mut Renderer, screen: Rect, theme: &Theme) {
        let layout = ManagerLayout::new(screen);
        let c = chrome(theme);
        layout.draw(renderer, "Extensions", theme);
        if layout.outer.w < 16 || layout.outer.h < 8 { return; }
        primitives::draw_text_input(renderer, layout.search, &self.search_query,
            "/ Search plugins by name or description", self.is_searching, theme);

        if layout.nav.w > 0 {
            let start = self.nav_start(&layout);
            for index in start..CATEGORIES.len() + 2 {
                let y = layout.nav.y + (index - start) as u16;
                if y >= layout.nav.y + layout.nav.h { break; }
                let label = match index { 0 => "1 Installed", 1 => "2 Discover", _ => CATEGORIES[index - 2].0 };
                let active = index == self.nav_index;
                let bg = if active { c.row_selected_bg } else { c.panel_bg };
                let row = Rect::new(layout.nav.x, y, layout.nav.w.saturating_sub(1), 1);
                renderer.fill_rect(row, bg);
                let prefix = if active { "> " } else { "  " };
                text_in(renderer, row, &format!("{}{}", prefix, label), c.text_strong, bg, active && self.focus_nav);
            }
        }

        let r = layout.content;
        if self.detail_open {
            self.draw_detail(renderer, r, theme);
        } else {
            let title = if self.section == ShopSection::Installed { "Your plugins" } else { CATEGORIES[self.category_idx].0 };
            text_in(renderer, Rect::new(r.x, r.y, r.w, 1), title, c.text_strong, c.panel_bg, true);
            let subtitle = format!("{} plugins  ·  Enter Details & Configure", self.plugins.len());
            if r.h >= 5 {
                text_in(renderer, Rect::new(r.x, r.y + 1, r.w, 1), &subtitle, c.text_muted, c.panel_bg, false);
            }
            let (list, stride) = Self::list_geometry(&layout);
            let visible = (list.h / stride) as usize;
            self.list_state.adjust_scroll(visible);
            if self.plugins.is_empty() {
                text_in(renderer, list, "No matching plugins", c.text_muted, c.panel_bg, false);
            }
            for (offset, p) in self.plugins.iter().skip(self.list_state.scroll_offset).take(visible).enumerate() {
                let index = self.list_state.scroll_offset + offset;
                let y = list.y + offset as u16 * stride;
                let selected = index == self.list_state.selected;
                let bg = if selected && !self.focus_nav { c.row_selected_bg } else if selected { c.row_hover_bg } else { c.panel_bg };
                let row = Rect::new(list.x, y, list.w, stride.saturating_sub(1).max(1));
                renderer.fill_rect(row, bg);
                let marker = if selected { "> " } else { "  " };
                let label = if stride > 1 { &p.name } else { &p.full_name };
                let status = format!("[{}]", Self::plugin_status(p));
                let status_w = status.len() as u16;
                let show_status = list.w >= 42;
                let name_w = if show_status { list.w.saturating_sub(status_w + 2) } else { list.w };
                text_in(renderer, Rect::new(list.x, y, name_w, 1), &format!("{}{}", marker, label), c.text_strong, bg, selected);
                if show_status && name_w > 4 {
                    text_in(renderer, Rect::new(list.x + name_w + 1, y, status_w, 1), &status, c.text, bg, false);
                }
                if stride > 1 {
                    let description = format!("{}  ·  {}", p.full_name, p.description);
                    text_in(renderer, Rect::new(list.x + 2, y + 1, list.w.saturating_sub(3), 1), &description, c.text_muted, bg, false);
                }
            }
        }
        let actions = if self.detail_open { ["[ Refresh ]", "[ Back ]"] } else { ["[ Refresh ]", "[ Close ]"] };
        let hint = if !self.status_msg.is_empty() { self.status_msg.as_str() }
            else if self.detail_open { "Esc Back   E Configure   D Toggle" }
            else { "Tab Next pane   ↑↓ Navigate   Enter Open   Esc Close" };
        layout.draw_footer(renderer, hint, &actions, theme);
        self.draw_confirmation(renderer, screen, theme);
    }

    fn draw_detail(&self, renderer: &mut Renderer, r: Rect, theme: &Theme) {
        let Some(p) = self.selected_plugin() else { return; };
        let c = chrome(theme);
        if r.h < 6 {
            text_in(renderer, r, "Enlarge to view plugin details", c.text_muted, c.panel_bg, false);
            return;
        }
        text_in(renderer, Rect::new(r.x, r.y, r.w, 1), &p.name, c.text_strong, c.panel_bg, true);
        text_in(renderer, Rect::new(r.x, r.y + 1, r.w, 1), &p.full_name, c.text_muted, c.panel_bg, false);
        text_in(renderer, Rect::new(r.x, r.y + 2, r.w, 1), Self::plugin_status(p), c.accent, c.panel_bg, true);
        if r.h > 8 {
            for (i, line) in primitives::wrap_text(&p.description, r.w as usize).iter().take(r.h.saturating_sub(8) as usize).enumerate() {
                text_in(renderer, Rect::new(r.x, r.y + 4 + i as u16, r.w, 1), line, c.text, c.panel_bg, false);
            }
        }
        let action_y = r.y + r.h.saturating_sub(3);
        if !p.installed || p.status == "pending_removal" {
            text_in(renderer, Rect::new(r.x, action_y + 1, r.w, 1), "Enter  Install plugin", c.text_strong, c.surface_bg, true);
        } else {
            text_in(renderer, Rect::new(r.x, action_y, r.w, 1), "E  Configure plugin", c.text_strong, c.surface_bg, false);
            if !p.protected {
                let toggle = if p.enabled { "D  Disable plugin" } else { "D  Enable plugin" };
                text_in(renderer, Rect::new(r.x, action_y + 1, r.w, 1), toggle, c.text_strong, c.surface_bg, false);
                text_in(renderer, Rect::new(r.x, action_y + 2, r.w, 1), "U  Uninstall plugin", c.error, c.surface_bg, false);
            }
        }
    }

    fn trust_lines(&self, width: usize) -> Vec<String> {
        let Some((repo, _)) = &self.pending_trust else { return vec![]; };
        let source = format!("https://github.com/{}", repo);
        // Repository identifiers are validated ASCII. Never truncate the source
        // identity: long names wrap in full before confirmation becomes available.
        let mut lines: Vec<String> = source.as_bytes().chunks(width.max(1))
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect();
        lines.push(String::new());
        lines.extend(primitives::wrap_text(
            "Plugin code and dependencies can read/write files, run commands and access the network as your user. Install and update hooks can also run code. No sandbox or security review. Only proceed if you trust this source.", width));
        lines
    }

    fn confirmation_rect(&self, screen: Rect) -> Option<Rect> {
        if screen.w < 42 || screen.h < 12 {
            return None;
        }
        if self.confirm_modal == ShopConfirm::Trust {
            let width = 76.min(screen.w.saturating_sub(4));
            let height = self.trust_lines(width.saturating_sub(4) as usize).len() as u16 + 6;
            if self.pending_trust.is_none() || screen.h < height + 2 { return None; }
            return Some(Rect::new(screen.x + (screen.w - width) / 2,
                screen.y + (screen.h - height) / 2, width, height));
        }
        Some(Rect::new(
            screen.x + (screen.w - 40) / 2,
            screen.y + (screen.h - 9) / 2,
            40,
            9,
        ))
    }

    pub fn handle_confirmation_mouse(
        &mut self,
        screen: Rect,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
    ) -> bool {
        if self.confirm_modal == ShopConfirm::None {
            return false;
        }
        if pressed && button == 0 {
            if let Some(mr) = self.confirmation_rect(screen) {
                if !self.can_confirm_modal { return true; }
                if my == mr.y + mr.h - 3 {
                    let (yes_width, no_width) = match self.confirm_modal {
                        ShopConfirm::Trust => (9, 10),
                        ShopConfirm::Restart => (11, 9),
                        _ => (10, 10),
                    };
                    if (mr.x + 3..mr.x + 3 + yes_width).contains(&mx) {
                        self.handle_key(&InputEvent::Key("y".to_string()));
                    } else if (mr.x + 20..mr.x + 20 + no_width).contains(&mx) {
                        self.handle_key(&InputEvent::Key("n".to_string()));
                    }
                }
            }
        }
        true
    }

    pub fn draw_confirmation(&mut self, renderer: &mut Renderer, r: Rect, theme: &Theme) {
        let c = chrome(theme);
        if self.confirm_modal != ShopConfirm::None {
            self.can_confirm_modal = self.confirmation_rect(r).is_some();
            if !self.can_confirm_modal {
                renderer.draw_text(
                    r.x + 1,
                    r.y + r.h / 2,
                    "Enlarge to confirm",
                    theme.warning,
                    c.panel_bg,
                );
            } else {
                let mr = self.confirmation_rect(r).unwrap();
                let mh = mr.h;

                match self.confirm_modal {
                    ShopConfirm::Trust => {
                        primitives::draw_modal_frame(renderer, mr, "Trust this plugin?", theme);
                        for (i, line) in self.trust_lines(mr.w.saturating_sub(4) as usize).iter().enumerate() {
                            renderer.draw_text(mr.x + 2, mr.y + 2 + i as u16, line, c.text, c.panel_bg);
                        }
                        renderer.draw_text(mr.x + 3, mr.y + mh - 3, "[Y] Trust", theme.warning, c.panel_bg);
                        renderer.draw_text(mr.x + 20, mr.y + mh - 3, "[N] Cancel", c.text_muted, c.panel_bg);
                    }
                    ShopConfirm::Uninstall => {
                        primitives::draw_modal_frame(renderer, mr, "Uninstall?", theme);
                        renderer.draw_text(
                            mr.x + 2,
                            mr.y + 3,
                            "Configuration will be kept.",
                            c.text,
                            c.panel_bg,
                        );
                        renderer.draw_text(
                            mr.x + 3,
                            mr.y + mh.saturating_sub(3),
                            "[Y] Remove",
                            theme.error,
                            c.panel_bg,
                        );
                        renderer.draw_text(
                            mr.x + 20,
                            mr.y + mh.saturating_sub(3),
                            "[N] Cancel",
                            c.text_muted,
                            c.panel_bg,
                        );
                    }
                    ShopConfirm::Restart => {
                        primitives::draw_modal_frame(renderer, mr, "Restart Tuim?", theme);
                        renderer.draw_text(
                            mr.x + 2,
                            mr.y + 3,
                            "Plugin changes saved.",
                            c.text,
                            c.panel_bg,
                        );
                        renderer.draw_text(
                            mr.x + 3,
                            mr.y + mh.saturating_sub(3),
                            "[Y] Restart",
                            theme.success,
                            c.panel_bg,
                        );
                        renderer.draw_text(
                            mr.x + 20,
                            mr.y + mh.saturating_sub(3),
                            "[N] Later",
                            c.text_muted,
                            c.panel_bg,
                        );
                    }
                    ShopConfirm::None => {}
                }
            }
        }
    }
}
