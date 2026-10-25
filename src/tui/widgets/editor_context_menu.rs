use rmpv::Value;

use crate::nvim::rpc::RpcClient;
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives;

const MENU_ITEMS: &[(&str, &str, &str)] = &[
    ("Undo", "Ctrl+Z", "undo"),
    ("Redo", "Ctrl+Y", "redo"),
    ("Cut", "Ctrl+X", "cut"),
    ("Copy", "Ctrl+C", "copy"),
    ("Paste", "Ctrl+V", "paste"),
    ("Select All", "Ctrl+A", "select_all"),
    ("Explain Selection with AI", "", "explain_ai"),
    ("Fix Selection with AI", "", "fix_ai"),
    ("Add Selection to AI Context", "", "context_ai"),
];

pub struct EditorContextMenu {
    pub is_open: bool,
    pub x: u16,
    pub y: u16,
    pub selected: usize,
}

impl EditorContextMenu {
    pub fn new() -> Self {
        Self {
            is_open: false,
            x: 0,
            y: 0,
            selected: 0,
        }
    }

    pub fn open(&mut self, x: u16, y: u16) {
        self.x = x;
        self.y = y;
        self.selected = 0;
        self.is_open = true;
    }

    pub fn rect(&self, screen: Rect) -> Rect {
        let w = 34u16.min(screen.w);
        let h = (MENU_ITEMS.len() as u16 + 2).min(screen.h);
        let x = self.x.min(screen.w.saturating_sub(w));
        let y = self.y.min(screen.h.saturating_sub(h));
        Rect::new(x, y, w, h)
    }

    fn execute_action(&mut self, action: &str, rpc: &mut RpcClient) {
        self.is_open = false;
        let lua = match action {
            "undo" => "if _G.tuim_ide_action then _G.tuim_ide_action('undo') else pcall(vim.cmd, 'undo') end",
            "redo" => "if _G.tuim_ide_action then _G.tuim_ide_action('redo') else pcall(vim.cmd, 'redo') end",
            "cut" => "if _G.tuim_ide_action then _G.tuim_ide_action('cut') end",
            "copy" => "if _G.tuim_ide_action then _G.tuim_ide_action('copy') end",
            "paste" => "if _G.tuim_ide_action then _G.tuim_ide_action('paste') end",
            "select_all" => "if _G.tuim_ide_action then _G.tuim_ide_action('select_all') end",
            "explain_ai" => "if _G.RunAIAction then _G.RunAIAction('explain_selection') end",
            "fix_ai" => "if _G.RunAIAction then _G.RunAIAction('fix_selection') end",
            "context_ai" => "if _G.SendSelectionToAI then _G.SendSelectionToAI() end",
            _ => return,
        };
        let _ = rpc.notify(
            "nvim_exec_lua",
            vec![Value::from(lua), Value::Array(vec![])],
        );
    }

    pub fn handle_key(&mut self, ev: &InputEvent, rpc: &mut RpcClient) -> bool {
        if !self.is_open {
            return false;
        }
        let InputEvent::Key(k) = ev else {
            return false;
        };
        match k.as_str() {
            "<Esc>" => {
                self.is_open = false;
            }
            "<Up>" | "k" => {
                self.selected = self.selected.saturating_sub(1);
            }
            "<Down>" | "j" => {
                if self.selected + 1 < MENU_ITEMS.len() {
                    self.selected += 1;
                }
            }
            "<CR>" => {
                let act = MENU_ITEMS[self.selected].2;
                self.execute_action(act, rpc);
            }
            _ => {
                self.is_open = false;
            }
        }
        true
    }

    pub fn handle_mouse(
        &mut self,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
        rpc: &mut RpcClient,
    ) -> bool {
        if !self.is_open || !pressed {
            return false;
        }
        let r = Rect::new(self.x, self.y, 34, MENU_ITEMS.len() as u16 + 2);
        if r.contains(mx, my) && my > r.y && my < r.y + r.h - 1 && button == 0 {
            let idx = (my - r.y - 1) as usize;
            if idx < MENU_ITEMS.len() {
                self.selected = idx;
                let act = MENU_ITEMS[idx].2;
                self.execute_action(act, rpc);
                return true;
            }
        }
        self.is_open = false;
        true
    }

    pub fn draw(&self, renderer: &mut Renderer, screen: Rect, theme: &Theme) {
        if !self.is_open {
            return;
        }
        let r = self.rect(screen);
        let c = chrome(theme);
        primitives::draw_modal_frame(renderer, r, "", theme);
        for (i, (label, shortcut, _)) in MENU_ITEMS.iter().enumerate() {
            let ry = r.y + 1 + i as u16;
            let sel = i == self.selected;
            let bg = if sel { c.row_selected_bg } else { c.panel_bg };
            renderer.fill_rect(Rect::new(r.x + 1, ry, r.w - 2, 1), bg);
            renderer.draw_text_styled(r.x + 2, ry, label, c.text_strong, bg, sel, false);
            if !shortcut.is_empty() && r.w > 12 {
                let sx = r.x + r.w.saturating_sub(shortcut.len() as u16 + 2);
                renderer.draw_text(sx, ry, shortcut, c.text_muted, bg);
            }
        }
    }
}

impl Default for EditorContextMenu {
    fn default() -> Self {
        Self::new()
    }
}
