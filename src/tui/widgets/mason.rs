use std::collections::HashSet;

use rmpv::Value;
use serde::Deserialize;

use crate::nvim::rpc::RpcClient;
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MasonPkg {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Deserialize)]
struct MasonPayload {
    #[serde(default)]
    #[allow(dead_code)]
    available: bool,
    #[serde(default)]
    health: String,
    #[serde(default)]
    packages: Vec<MasonPkg>,
}

pub const MASON_TABS: &[(&str, &str)] = &[
    (" LSP ", "LSP"),
    (" DAP ", "DAP"),
    (" Linter ", "Linter"),
    (" Formatter ", "Formatter"),
    (" Runtime ", "Runtime"),
    (" Compiler ", "Compiler"),
];

pub struct MasonWidget {
    pub is_open: bool,
    pub active_tab: usize,
    pub search_query: String,
    pub is_searching: bool,
    pub packages: Vec<MasonPkg>,
    pub filtered: Vec<usize>,
    pub list_state: ListState,
    pub pending_install: HashSet<String>,
    pub pending_uninstall: HashSet<String>,
    pub health_summary: String,
}

impl MasonWidget {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: 0,
            search_query: String::new(),
            is_searching: false,
            packages: Vec::new(),
            filtered: Vec::new(),
            list_state: ListState::new(),
            pending_install: HashSet::new(),
            pending_uninstall: HashSet::new(),
            health_summary: "Active: none | Recommended: rust-analyzer, lua-language-server | Missing: none".to_string(),
        }
    }

    pub fn refresh(&mut self, rpc: &mut RpcClient) {
        let lua_code = r#"
            local ok, reg = pcall(require, 'mason-registry')
            if not ok then
                return vim.fn.json_encode({
                    available = false,
                    health = "Language tools unavailable. Enable Mason in Settings > Plugins.",
                    packages = {}
                })
            end
            local pkgs = {}
            local specs_ok, all_specs = pcall(reg.get_all_package_specs)
            if specs_ok and type(all_specs) == 'table' then
                for _, spec in ipairs(all_specs) do
                    local name = spec.name or ''
                    local is_inst = false
                    pcall(function() is_inst = reg.is_installed(name) end)
                    local cats = spec.categories or {}
                    table.insert(pkgs, {
                        name = name,
                        desc = spec.description or '',
                        categories = cats,
                        installed = is_inst,
                    })
                end
            else
                local p_ok, all_pkgs = pcall(reg.get_all_packages)
                if p_ok and type(all_pkgs) == 'table' then
                    for _, pkg in ipairs(all_pkgs) do
                        table.insert(pkgs, {
                            name = pkg.name or '',
                            desc = (pkg.spec and pkg.spec.description) or '',
                            categories = (pkg.spec and pkg.spec.categories) or {},
                            installed = pkg:is_installed(),
                        })
                    end
                end
            end
            local clients = vim.lsp.get_clients and vim.lsp.get_clients() or {}
            local active_names = {}
            for _, c in ipairs(clients) do table.insert(active_names, c.name) end
            local active_str = #active_names > 0 and table.concat(active_names, ', ') or 'none'
            local health_str = string.format("Active: %s | Recommended: lua-language-server | Missing: none", active_str)
            return vim.fn.json_encode({
                available = true,
                health = health_str,
                packages = pkgs
            })
        "#;

        if let Ok(Value::String(s)) = rpc.call(
            "nvim_exec_lua",
            vec![Value::from(lua_code), Value::Array(vec![])],
        ) {
            if let Some(json_str) = s.as_str() {
                if let Ok(payload) = serde_json::from_str::<MasonPayload>(json_str) {
                    self.health_summary = payload.health;
                    self.packages = payload.packages;
                    self.packages.sort_by(|a, b| a.name.cmp(&b.name));
                }
            }
        }
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        let (_, target_cat) = MASON_TABS
            .get(self.active_tab)
            .copied()
            .unwrap_or(MASON_TABS[0]);
        let q = self.search_query.to_lowercase();
        self.filtered.clear();
        for (idx, pkg) in self.packages.iter().enumerate() {
            let cat_match = pkg.categories.is_empty()
                || pkg
                    .categories
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(target_cat));
            if !cat_match {
                continue;
            }
            if !q.is_empty()
                && !pkg.name.to_lowercase().contains(&q)
                && !pkg.desc.to_lowercase().contains(&q)
            {
                continue;
            }
            self.filtered.push(idx);
        }
        self.list_state.clamp(self.filtered.len());
    }

    pub fn toggle_selected(&mut self) {
        if let Some(&pkg_idx) = self.filtered.get(self.list_state.selected) {
            if let Some(pkg) = self.packages.get(pkg_idx) {
                let name = pkg.name.clone();
                if pkg.installed {
                    if !self.pending_uninstall.remove(&name) {
                        self.pending_uninstall.insert(name);
                    }
                } else if !self.pending_install.remove(&name) {
                    self.pending_install.insert(name);
                }
            }
        }
    }

    pub fn apply_and_close(&mut self, rpc: &mut RpcClient) {
        let mut cmds = Vec::new();
        if !self.pending_install.is_empty() {
            let list: Vec<String> = self.pending_install.drain().collect();
            cmds.push(format!("MasonInstall {}", list.join(" ")));
        }
        if !self.pending_uninstall.is_empty() {
            let list: Vec<String> = self.pending_uninstall.drain().collect();
            cmds.push(format!("MasonUninstall {}", list.join(" ")));
        }
        for cmd in cmds {
            let _ = rpc.notify("nvim_command", vec![Value::from(cmd)]);
        }
        self.is_open = false;
    }

    pub fn handle_key(&mut self, ev: &InputEvent, rpc: &mut RpcClient) -> bool {
        if !self.is_open {
            return false;
        }
        let key = match ev {
            InputEvent::Key(k) => k.as_str(),
            _ => return false,
        };

        if self.is_searching {
            match key {
                "<Esc>" | "<CR>" => {
                    self.is_searching = false;
                    return true;
                }
                "<BS>" => {
                    self.search_query.pop();
                    self.apply_filter();
                    return true;
                }
                k if k.chars().count() == 1 && !k.starts_with('<') => {
                    self.search_query.push_str(k);
                    self.apply_filter();
                    return true;
                }
                _ => return true,
            }
        }

        match key {
            "<Esc>" | "q" => {
                self.is_open = false;
                true
            }
            "/" => {
                self.is_searching = true;
                true
            }
            "1" | "2" | "3" | "4" | "5" | "6" => {
                let idx = (key.as_bytes()[0] - b'1') as usize;
                if idx < MASON_TABS.len() {
                    self.active_tab = idx;
                    self.apply_filter();
                }
                true
            }
            "<Tab>" | "l" | "<Right>" => {
                self.active_tab = (self.active_tab + 1) % MASON_TABS.len();
                self.apply_filter();
                true
            }
            "h" | "<Left>" => {
                self.active_tab = (self.active_tab + MASON_TABS.len() - 1) % MASON_TABS.len();
                self.apply_filter();
                true
            }
            "j" | "<Down>" => {
                self.list_state.move_down(self.filtered.len(), 1);
                true
            }
            "k" | "<Up>" => {
                self.list_state.move_up(1);
                true
            }
            " " | "i" => {
                self.toggle_selected();
                true
            }
            "x" => {
                self.toggle_selected();
                true
            }
            "<CR>" => {
                self.apply_and_close(rpc);
                true
            }
            _ => true,
        }
    }

    pub fn handle_mouse(
        &mut self, screen: Rect, mx: u16, my: u16, button: u8, pressed: bool, rpc: &mut RpcClient,
    ) -> bool {
        if !self.is_open || !pressed { return false; }
        let layout = primitives::ManagerLayout::new(screen);
        if !layout.outer.contains(mx, my) {
            if button == 0 { self.is_open = false; }
            return true;
        }
        if button == 64 { self.list_state.move_up(1); return true; }
        if button == 65 { self.list_state.move_down(self.filtered.len(), 1); return true; }
        if button != 0 { return true; }
        if layout.search.contains(mx, my) { self.is_searching = true; return true; }
        if layout.nav.contains(mx, my) {
            let index = (my - layout.nav.y) as usize;
            if index < MASON_TABS.len() { self.active_tab = index; self.apply_filter(); }
            return true;
        }
        let label = self.install_label();
        let actions = [label.as_str(), "[ Cancel ]"];
        if layout.action_rect(&actions, 0).contains(mx, my) { self.apply_and_close(rpc); return true; }
        if layout.action_rect(&actions, 1).contains(mx, my) { self.is_open = false; return true; }
        let list_y = layout.content.y + 4;
        if layout.content.contains(mx, my) && my >= list_y {
            let index = self.list_state.scroll_offset + (my - list_y) as usize;
            if index < self.filtered.len() { self.list_state.selected = index; self.toggle_selected(); }
        }
        true
    }

    fn install_label(&self) -> String {
        let pending = self.pending_install.len() + self.pending_uninstall.len();
        if pending > 0 { format!("[ Install ({}) ]", pending) } else { "[ Install & Close ]".into() }
    }

    pub fn draw(&mut self, renderer: &mut Renderer, screen: Rect, theme: &Theme) {
        if !self.is_open { return; }
        let layout = primitives::ManagerLayout::new(screen);
        let c = chrome(theme);
        layout.draw(renderer, "Mason Package Manager", theme);
        if layout.outer.w < 16 || layout.outer.h < 8 { return; }
        primitives::draw_text_input(renderer, layout.search, &self.search_query,
            "/ Search language tools...", self.is_searching, theme);
        for (i, (label, _)) in MASON_TABS.iter().enumerate().take(layout.nav.h as usize) {
            if layout.nav.w == 0 { break; }
            let active = i == self.active_tab;
            let bg = if active { c.row_selected_bg } else { c.panel_bg };
            let row = Rect::new(layout.nav.x, layout.nav.y + i as u16, layout.nav.w.saturating_sub(1), 1);
            renderer.fill_rect(row, bg);
            primitives::text_in(renderer, row, &format!("{}{}", i + 1, label), c.text_strong, bg, active);
        }
        let r = layout.content;
        primitives::text_in(renderer, Rect::new(r.x, r.y, r.w, 1),
            MASON_TABS[self.active_tab].1, c.text_strong, c.panel_bg, true);
        for (i, line) in primitives::wrap_text(&self.health_summary, r.w as usize).iter().take(2).enumerate() {
            primitives::text_in(renderer, Rect::new(r.x, r.y + 1 + i as u16, r.w, 1), line, c.text_muted, c.panel_bg, false);
        }
        let list_y = r.y + 4;
        let list_h = r.h.saturating_sub(4) as usize;
        self.list_state.adjust_scroll(list_h);
        if self.filtered.is_empty() && list_h > 0 {
            primitives::text_in(renderer, Rect::new(r.x, list_y, r.w, 1),
                "No packages in this category", c.text_muted, c.panel_bg, false);
        }
        for (i, &pkg_idx) in self.filtered.iter().skip(self.list_state.scroll_offset).take(list_h).enumerate() {
            let pkg = &self.packages[pkg_idx];
            let selected = self.list_state.scroll_offset + i == self.list_state.selected;
            let bg = if selected { c.row_selected_bg } else { c.panel_bg };
            let row = Rect::new(r.x, list_y + i as u16, r.w, 1);
            renderer.fill_rect(row, bg);
            let removing = self.pending_uninstall.contains(&pkg.name);
            let pending = self.pending_install.contains(&pkg.name);
            let checked = (pkg.installed && !removing) || pending;
            let status = if removing { "removing" } else if pending { "pending" } else if pkg.installed { "installed" } else { "available" };
            primitives::text_in(renderer, row,
                &format!("{} {}  [{}]  {}", if checked { "[x]" } else { "[ ]" }, pkg.name, status, pkg.desc),
                c.text_strong, bg, selected);
        }
        let label = self.install_label();
        layout.draw_footer(renderer, "Tab Category   ↑↓ Navigate   Space Toggle",
            &[label.as_str(), "[ Cancel ]"], theme);
    }

}

impl Default for MasonWidget {
    fn default() -> Self {
        Self::new()
    }
}
