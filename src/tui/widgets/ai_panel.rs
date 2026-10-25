use std::process::Command;

use rmpv::Value;

use crate::nvim::rpc::RpcClient;
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::renderer::Renderer;
use crate::tui::theme::{chrome, Theme};
use crate::tui::widgets::primitives::{self, ListState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiSection {
    Agents,
    Context,
    Actions,
}

#[derive(Debug, Clone)]
pub struct AiAgentItem {
    pub name: &'static str,
    pub cmd: &'static str,
    pub installed: bool,
}

pub const AI_AGENTS: &[(&str, &str)] = &[
    ("Codex CLI", "codex"),
    ("Claude Code", "claude"),
    ("Gemini CLI", "gemini"),
    ("OpenCode", "opencode"),
    ("Antigravity (agy)", "agy"),
    ("GitHub Copilot CLI", "copilot"),
];

pub const AI_CONTEXT_ITEMS: &[(&str, &str)] = &[
    ("Add Selection to AI", "_G.SendSelectionToAI()"),
    ("Add Current File Path", "_G.SendFilePathToAI()"),
    ("Add Entire Buffer Content", "_G.SendFileContentToAI()"),
    ("Add Diagnostics", "_G.SendDiagnosticsToAI()"),
    ("Add Git Diff", "_G.SendGitDiffToAI()"),
];

pub const AI_ACTION_ITEMS: &[(&str, &str)] = &[
    ("Fix Diagnostics in File", "_G.RunAIAction('fix_diagnostics')"),
    ("Explain Selection", "_G.RunAIAction('explain_selection')"),
    ("Fix / Improve Selection", "_G.RunAIAction('fix_selection')"),
    ("Write Tests for File", "_G.RunAIAction('write_tests')"),
    ("Review Working Tree Changes", "_G.RunAIAction('review_changes')"),
    ("Implement Nearby TODO", "_G.RunAIAction('implement_todo')"),
];

pub struct AiPanel {
    pub section: AiSection,
    pub agents: Vec<AiAgentItem>,
    pub list_state: ListState,
    pub active_agent: String,
    pub status: String,
    pub focus_editor_requested: bool,
}

impl AiPanel {
    pub fn new() -> Self {
        let mut panel = Self {
            section: AiSection::Agents,
            agents: Vec::new(),
            list_state: ListState::new(),
            active_agent: String::new(),
            status: "idle".to_string(),
            focus_editor_requested: false,
        };
        panel.refresh_agents();
        panel
    }

    pub fn refresh_agents(&mut self) {
        self.agents = AI_AGENTS
            .iter()
            .map(|(name, cmd)| {
                let installed = Command::new("which")
                    .arg(cmd)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                AiAgentItem {
                    name,
                    cmd,
                    installed,
                }
            })
            .collect();
        self.list_state.clamp(self.current_section_len());
    }

    pub fn current_section_len(&self) -> usize {
        match self.section {
            AiSection::Agents => self.agents.len(),
            AiSection::Context => AI_CONTEXT_ITEMS.len(),
            AiSection::Actions => AI_ACTION_ITEMS.len(),
        }
    }

    pub fn set_section(&mut self, section: AiSection) {
        self.section = section;
        self.list_state.selected = 0;
        self.list_state.scroll_offset = 0;
    }

    pub fn update_status(&mut self, cmd: &str, state: &str, is_active: bool) {
        if is_active || self.active_agent.is_empty() || self.active_agent == cmd {
            self.active_agent = cmd.to_string();
            self.status = state.to_string();
        }
    }

    pub fn execute_selected(&mut self, rpc: &mut RpcClient) {
        let idx = self.list_state.selected;
        match self.section {
            AiSection::Agents => {
                if let Some(agent) = self.agents.get(idx) {
                    let lua = format!("_G.OpenAITerminal('{}')", agent.cmd);
                    let _ = rpc.notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
                    self.focus_editor_requested = true;
                }
            }
            AiSection::Context => {
                if let Some((_, lua)) = AI_CONTEXT_ITEMS.get(idx) {
                    let _ = rpc.notify("nvim_exec_lua", vec![Value::from(*lua), Value::Array(vec![])]);
                }
            }
            AiSection::Actions => {
                if let Some((_, lua)) = AI_ACTION_ITEMS.get(idx) {
                    let _ = rpc.notify("nvim_exec_lua", vec![Value::from(*lua), Value::Array(vec![])]);
                    self.focus_editor_requested = true;
                }
            }
        }
    }

    pub fn handle_key(&mut self, ev: &InputEvent, rpc: &mut RpcClient) -> bool {
        let key = match ev {
            InputEvent::Key(k) => k.as_str(),
            _ => return false,
        };

        let len = self.current_section_len();
        match key {
            "1" => {
                self.set_section(AiSection::Agents);
                true
            }
            "2" => {
                self.set_section(AiSection::Context);
                true
            }
            "3" => {
                self.set_section(AiSection::Actions);
                true
            }
            "h" | "<Left>" => {
                self.set_section(match self.section {
                    AiSection::Agents => AiSection::Actions,
                    AiSection::Context => AiSection::Agents,
                    AiSection::Actions => AiSection::Context,
                });
                true
            }
            "l" | "<Right>" | "<Tab>" => {
                self.set_section(match self.section {
                    AiSection::Agents => AiSection::Context,
                    AiSection::Context => AiSection::Actions,
                    AiSection::Actions => AiSection::Agents,
                });
                true
            }
            "j" | "<Down>" => {
                self.list_state.move_down(len, 1);
                true
            }
            "k" | "<Up>" => {
                self.list_state.move_up(1);
                true
            }
            "<CR>" => {
                self.execute_selected(rpc);
                true
            }
            "f" => {
                let _ = rpc.notify(
                    "nvim_exec_lua",
                    vec![Value::from("_G.FocusAITerminal()"), Value::Array(vec![])],
                );
                self.focus_editor_requested = true;
                true
            }
            "r" => {
                let _ = rpc.notify(
                    "nvim_exec_lua",
                    vec![Value::from("_G.RestartAITerminal()"), Value::Array(vec![])],
                );
                true
            }
            "s" => {
                let _ = rpc.notify(
                    "nvim_exec_lua",
                    vec![Value::from("_G.StopAITerminal()"), Value::Array(vec![])],
                );
                true
            }
            "x" => {
                let _ = rpc.notify(
                    "nvim_exec_lua",
                    vec![Value::from("_G.CloseAITerminal()"), Value::Array(vec![])],
                );
                true
            }
            _ => false,
        }
    }

    pub fn handle_mouse(
        &mut self,
        r: Rect,
        mx: u16,
        my: u16,
        button: u8,
        pressed: bool,
        rpc: &mut RpcClient,
    ) -> bool {
        if !r.contains(mx, my) || !pressed || button != 0 {
            return false;
        }
        let rel_y = my - r.y;
        let rel_x = mx - r.x;

        if rel_y == 1 {
            let third = (r.w / 3).max(1);
            if rel_x < third {
                self.set_section(AiSection::Agents);
            } else if rel_x < third * 2 {
                self.set_section(AiSection::Context);
            } else {
                self.set_section(AiSection::Actions);
            }
            return true;
        }

        if rel_y == 2 {
            self.execute_selected(rpc);
            return true;
        }

        if rel_y >= 4 {
            let idx = (rel_y - 4) as usize;
            if idx < self.current_section_len() {
                self.list_state.selected = idx;
                self.execute_selected(rpc);
                return true;
            }
        }
        true
    }

    pub fn draw(&mut self, renderer: &mut Renderer, r: Rect, theme: &Theme, focused: bool) {
        if r.w == 0 || r.h == 0 {
            return;
        }
        let c = chrome(theme);
        renderer.fill_rect(r, c.panel_bg);

        renderer.draw_text_styled(
            r.x + 1,
            r.y,
            "AI WORKSPACE",
            c.text_strong,
            c.panel_bg,
            true,
            false,
        );

        if r.h < 4 {
            return;
        }

        // Section tabs on row 1
        let tabs = [
            ("1 Agents", AiSection::Agents),
            ("2 Context", AiSection::Context),
            ("3 Actions", AiSection::Actions),
        ];
        let mut x_pos = r.x + 1;
        for (label, sec) in tabs {
            let active = self.section == sec;
            let bg = if active { c.row_selected_bg } else { c.surface_bg };
            let fg = if active { c.text_strong } else { c.text_muted };
            renderer.draw_text_styled(x_pos, r.y + 1, label, fg, bg, active, false);
            x_pos += label.len() as u16 + 2;
        }

        // Row 2: Open chat action button
        renderer.draw_text_styled(
            r.x + 1,
            r.y + 2,
            "[ Open chat ]",
            c.accent,
            c.surface_bg,
            true,
            false,
        );

        let start_y = r.y + 4;
        let max_rows = r.h.saturating_sub(6) as usize;

        match self.section {
            AiSection::Agents => {
                for (idx, agent) in self.agents.iter().take(max_rows).enumerate() {
                    let ry = start_y + idx as u16;
                    let selected = idx == self.list_state.selected;
                    let bg = if selected {
                        if focused {
                            c.row_selected_bg
                        } else {
                            c.row_hover_bg
                        }
                    } else {
                        c.panel_bg
                    };
                    renderer.fill_rect(Rect::new(r.x, ry, r.w, 1), bg);
                    let badge = if self.active_agent == agent.cmd && self.status == "running" {
                        "●"
                    } else if agent.installed {
                        "○"
                    } else {
                        "×"
                    };
                    let fg = if agent.installed {
                        c.text_strong
                    } else {
                        c.text_muted
                    };
                    let line = format!("{} {}", badge, agent.name);
                    let truncated =
                        primitives::truncate_to_width(&line, r.w.saturating_sub(2) as usize);
                    renderer.draw_text(r.x + 1, ry, &truncated, fg, bg);
                }
            }
            AiSection::Context => {
                for (idx, (label, _)) in AI_CONTEXT_ITEMS.iter().take(max_rows).enumerate() {
                    let ry = start_y + idx as u16;
                    let selected = idx == self.list_state.selected;
                    let bg = if selected {
                        if focused {
                            c.row_selected_bg
                        } else {
                            c.row_hover_bg
                        }
                    } else {
                        c.panel_bg
                    };
                    renderer.fill_rect(Rect::new(r.x, ry, r.w, 1), bg);
                    let truncated =
                        primitives::truncate_to_width(label, r.w.saturating_sub(2) as usize);
                    renderer.draw_text(r.x + 1, ry, &truncated, c.text_strong, bg);
                }
            }
            AiSection::Actions => {
                for (idx, (label, _)) in AI_ACTION_ITEMS.iter().take(max_rows).enumerate() {
                    let ry = start_y + idx as u16;
                    let selected = idx == self.list_state.selected;
                    let bg = if selected {
                        if focused {
                            c.row_selected_bg
                        } else {
                            c.row_hover_bg
                        }
                    } else {
                        c.panel_bg
                    };
                    renderer.fill_rect(Rect::new(r.x, ry, r.w, 1), bg);
                    let truncated =
                        primitives::truncate_to_width(label, r.w.saturating_sub(2) as usize);
                    renderer.draw_text(r.x + 1, ry, &truncated, c.text_strong, bg);
                }
            }
        }

        if r.h >= 6 {
            let status_y = r.y + r.h - 2;
            let status_line = if self.active_agent.is_empty() {
                "Session: none (Enter to start)".to_string()
            } else {
                format!("Session: {} ({})", self.active_agent, self.status)
            };
            let truncated =
                primitives::truncate_to_width(&status_line, r.w.saturating_sub(2) as usize);
            renderer.draw_text(r.x + 1, status_y, &truncated, c.text_muted, c.panel_bg);
            renderer.draw_text(
                r.x + 1,
                status_y + 1,
                "[f]ocus [r]estart [s]top [x]close",
                c.text_muted,
                c.panel_bg,
            );
        }
    }
}

impl Default for AiPanel {
    fn default() -> Self {
        Self::new()
    }
}
