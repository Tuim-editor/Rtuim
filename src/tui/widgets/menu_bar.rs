use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives;

pub struct TopMenu {
    pub title: &'static str,
    pub items: &'static [(&'static str, &'static str)],
}

pub const MENUS: &[TopMenu] = &[
    TopMenu {
        title: "File",
        items: &[
            ("New File", "new_file"),
            ("Find File...", "find_file"),
            ("Save File", "save_file"),
            ("Close Buffer", "close_buffer"),
            ("Quit", "quit"),
        ],
    },
    TopMenu {
        title: "Edit",
        items: &[
            ("Undo", "undo"),
            ("Redo", "redo"),
            ("Find in File", "find"),
            ("Replace in File", "replace"),
            ("Search Project", "search_project"),
        ],
    },
    TopMenu {
        title: "View",
        items: &[
            ("Command Palette", "commands"),
            ("Switch Buffers", "switch_buffers"),
            ("Toggle Explorer", "toggle_explorer"),
            ("Toggle Terminal", "toggle_terminal"),
            ("Split Right", "split_right"),
            ("Split Down", "split_down"),
            ("Toggle Zen Mode", "toggle_zen"),
        ],
    },
    TopMenu {
        title: "Tools",
        items: &[
            ("Git / Changes", "changes"),
            ("AI Assistants", "ai_assistants"),
            ("Worktree Orchestrator", "orchestrator"),
            ("Extensions / Plugins", "extensions"),
            ("Language Tools (Mason)", "language_tools"),
            ("Settings", "settings"),
        ],
    },
    TopMenu {
        title: "Help",
        items: &[
            ("Help & Quickstart", "help"),
            ("Onboarding Guide", "onboarding"),
            ("Keyboard Shortcuts", "keyboard_shortcuts"),
            ("Report Bug...", "report_bug"),
        ],
    },
];

pub struct MenuBar {
    pub open_menu: Option<usize>,
    pub selected_item: usize,
    pub triggered_action: Option<&'static str>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            open_menu: None,
            selected_item: 0,
            triggered_action: None,
        }
    }

    pub fn draw_bar(&self, renderer: &mut Renderer, rect: Rect, theme: &Theme) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let c = chrome(theme);
        let mut x = rect.x;
        for (i, m) in MENUS.iter().enumerate() {
            let active = self.open_menu == Some(i);
            let bg = if active { c.row_selected_bg } else { c.surface_bg };
            let fg = if active { c.text_strong } else { c.text_muted };
            let label = format!(" {} ", m.title);
            if x + (label.len() as u16) > rect.x + rect.w {
                break;
            }
            renderer.draw_text_styled(x, rect.y, &label, fg, bg, active, false);
            x += label.len() as u16;
        }
    }

    pub fn dropdown_rect(&self, bar_rect: Rect) -> Option<Rect> {
        let idx = self.open_menu?;
        let menu = &MENUS[idx];
        let mut x = bar_rect.x;
        for m in MENUS.iter().take(idx) {
            x += m.title.len() as u16 + 2;
        }
        let w = 26u16;
        let h = menu.items.len() as u16 + 2;
        Some(Rect::new(x, bar_rect.y + 1, w, h))
    }

    pub fn draw_dropdown(&self, renderer: &mut Renderer, bar_rect: Rect, theme: &Theme) {
        let Some(idx) = self.open_menu else { return };
        let Some(dr) = self.dropdown_rect(bar_rect) else {
            return;
        };
        let c = chrome(theme);
        primitives::draw_modal_frame(renderer, dr, "", theme);
        let items = MENUS[idx].items;
        for (i, (label, _)) in items.iter().enumerate() {
            let ry = dr.y + 1 + i as u16;
            let sel = i == self.selected_item;
            let bg = if sel { c.row_selected_bg } else { c.panel_bg };
            renderer.fill_rect(Rect::new(dr.x + 1, ry, dr.w - 2, 1), bg);
            renderer.draw_text_styled(dr.x + 2, ry, label, c.text_strong, bg, sel, false);
        }
    }

    pub fn handle_key(&mut self, ev: &InputEvent) -> bool {
        let Some(idx) = self.open_menu else {
            return false;
        };
        let InputEvent::Key(k) = ev else {
            return false;
        };
        let items = MENUS[idx].items;
        match k.as_str() {
            "<Esc>" => {
                self.open_menu = None;
                true
            }
            "<Left>" | "h" => {
                self.open_menu = Some(if idx == 0 { MENUS.len() - 1 } else { idx - 1 });
                self.selected_item = 0;
                true
            }
            "<Right>" | "l" => {
                self.open_menu = Some((idx + 1) % MENUS.len());
                self.selected_item = 0;
                true
            }
            "<Up>" | "k" => {
                self.selected_item = self.selected_item.saturating_sub(1);
                true
            }
            "<Down>" | "j" => {
                if self.selected_item + 1 < items.len() {
                    self.selected_item += 1;
                }
                true
            }
            "<CR>" | " " => {
                if let Some(&(_, action)) = items.get(self.selected_item) {
                    self.triggered_action = Some(action);
                }
                self.open_menu = None;
                true
            }
            _ => false,
        }
    }

    pub fn handle_mouse(
        &mut self,
        bar_rect: Rect,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
        motion: bool,
    ) -> bool {
        if my == bar_rect.y && bar_rect.contains(mx, my) {
            let mut x = bar_rect.x;
            for (i, m) in MENUS.iter().enumerate() {
                let w = m.title.len() as u16 + 2;
                if mx >= x && mx < x + w {
                    if pressed && button == 0 {
                        if self.open_menu == Some(i) {
                            self.open_menu = None;
                        } else {
                            self.open_menu = Some(i);
                            self.selected_item = 0;
                        }
                        return true;
                    } else if motion && self.open_menu.is_some() {
                        self.open_menu = Some(i);
                        self.selected_item = 0;
                        return true;
                    }
                }
                x += w;
            }
        }

        if let Some(dr) = self.dropdown_rect(bar_rect) {
            if dr.contains(mx, my) {
                if my > dr.y && my < dr.y + dr.h - 1 {
                    let row = (my - dr.y - 1) as usize;
                    if let Some(idx) = self.open_menu {
                        if row < MENUS[idx].items.len() {
                            self.selected_item = row;
                            if pressed && button == 0 {
                                let action = MENUS[idx].items[row].1;
                                self.triggered_action = Some(action);
                                self.open_menu = None;
                            }
                        }
                    }
                }
                return true;
            } else if pressed && button == 0 && self.open_menu.is_some() {
                self.open_menu = None;
                return true;
            }
        }

        false
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}
