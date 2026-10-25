use std::process::Command;

use rmpv::Value;
use unicode_width::UnicodeWidthStr;

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

pub const AI_SESSION_ACTIONS: &[(&str, &str)] = &[
    ("[f]ocus", "_G.FocusAITerminal()"),
    ("[r]estart", "_G.RestartAITerminal()"),
    ("[s]top", "_G.StopAITerminal()"),
    ("[x]close", "_G.CloseAITerminal()"),
];

#[derive(Clone, Debug)]
pub struct AiPanelLayout {
    pub tab_rects: [(AiSection, &'static str, Rect); 3],
    pub open_chat_row: u16,
    pub list_start_y: u16,
    pub list_max_rows: usize,
    pub status_lines: Vec<(u16, String)>,
    pub action_rects: Vec<(&'static str, &'static str, Rect)>,
}

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

    pub fn layout(&self, r: Rect) -> AiPanelLayout {
        let inner_w = r.w.saturating_sub(2) as usize;
        let max_x = r.x.saturating_add(r.w.saturating_sub(1));

        let tabs = [
            (AiSection::Agents, "1 Agents"),
            (AiSection::Context, "2 Context"),
            (AiSection::Actions, "3 Actions"),
        ];
        let mut cur_x = r.x.saturating_add(1);
        let mut cur_y = r.y.saturating_add(1);
        let mut tab_rects = [(AiSection::Agents, "", Rect::default()); 3];
        for (i, (sec, label)) in tabs.into_iter().enumerate() {
            let w = (label.width() as u16).min(r.w.saturating_sub(2));
            if cur_x > r.x.saturating_add(1) && cur_x.saturating_add(w) > max_x {
                cur_y = cur_y.saturating_add(1);
                cur_x = r.x.saturating_add(1);
            }
            tab_rects[i] = (sec, label, Rect::new(cur_x, cur_y, w, 1));
            cur_x = cur_x.saturating_add(w).saturating_add(2);
        }

        let open_chat_row = cur_y.saturating_add(1);
        let list_start_y = open_chat_row.saturating_add(2);

        let mut rel_actions: Vec<(&'static str, &'static str, u16, u16, u16)> =
            Vec::with_capacity(AI_SESSION_ACTIONS.len());
        let mut act_x = r.x.saturating_add(1);
        let mut act_rel_y = 0u16;
        for &(label, lua) in AI_SESSION_ACTIONS {
            let w = (label.width() as u16).min(r.w.saturating_sub(2));
            if act_x > r.x.saturating_add(1) && act_x.saturating_add(w) > max_x {
                act_rel_y = act_rel_y.saturating_add(1);
                act_x = r.x.saturating_add(1);
            }
            rel_actions.push((label, lua, act_x, act_rel_y, w));
            act_x = act_x.saturating_add(w).saturating_add(1);
        }
        let action_rows = if rel_actions.is_empty() {
            0
        } else {
            act_rel_y + 1
        };

        let status_text = if self.active_agent.is_empty() {
            "Session: none (Enter to start)".to_string()
        } else {
            format!("Session: {} ({})", self.active_agent, self.status)
        };
        let wrapped_status = primitives::wrap_text(&status_text, inner_w);
        let status_row_count = wrapped_status.len() as u16;
        let footer_h = status_row_count.saturating_add(action_rows);
        let bottom_y = r.y.saturating_add(r.h);

        let (status_lines, action_rects, list_end_y) =
            if inner_w > 0 && bottom_y >= list_start_y.saturating_add(footer_h) {
                let footer_start_y = bottom_y.saturating_sub(footer_h);
                let status_lines: Vec<(u16, String)> = wrapped_status
                    .into_iter()
                    .enumerate()
                    .map(|(idx, line)| (footer_start_y + idx as u16, line))
                    .collect();
                let actions_start_y = footer_start_y.saturating_add(status_row_count);
                let action_rects: Vec<(&'static str, &'static str, Rect)> = rel_actions
                    .into_iter()
                    .map(|(label, lua, x, ry, w)| {
                        (label, lua, Rect::new(x, actions_start_y + ry, w, 1))
                    })
                    .collect();
                (status_lines, action_rects, footer_start_y)
            } else {
                (Vec::new(), Vec::new(), bottom_y)
            };

        let list_max_rows = list_end_y.saturating_sub(list_start_y) as usize;

        AiPanelLayout {
            tab_rects,
            open_chat_row,
            list_start_y,
            list_max_rows,
            status_lines,
            action_rects,
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
        if !r.contains(mx, my) || !pressed {
            return false;
        }
        let len = self.current_section_len();
        if button == 64 {
            self.list_state.move_up(1);
            return true;
        }
        if button == 65 {
            self.list_state.move_down(len, 1);
            return true;
        }
        if button != 0 {
            return false;
        }

        let layout = self.layout(r);
        for (sec, _, tab_r) in layout.tab_rects {
            let hit_r = Rect::new(tab_r.x.saturating_sub(1), tab_r.y, tab_r.w.saturating_add(2), 1);
            if hit_r.contains(mx, my) {
                self.set_section(sec);
                return true;
            }
        }

        if my == layout.open_chat_row {
            self.execute_selected(rpc);
            return true;
        }

        for &(label, lua, act_r) in &layout.action_rects {
            if act_r.contains(mx, my) {
                let _ = rpc.notify(
                    "nvim_exec_lua",
                    vec![Value::from(lua), Value::Array(vec![])],
                );
                if label == "[f]ocus" {
                    self.focus_editor_requested = true;
                }
                return true;
            }
        }

        if my >= layout.list_start_y {
            let rel_row = (my - layout.list_start_y) as usize;
            if rel_row < layout.list_max_rows {
                let idx = self.list_state.scroll_offset + rel_row;
                if idx < len {
                    self.list_state.selected = idx;
                    self.execute_selected(rpc);
                    return true;
                }
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

        let hdr = primitives::truncate_to_width("AI WORKSPACE", r.w.saturating_sub(2) as usize);
        renderer.draw_text_styled(
            r.x + 1,
            r.y,
            &hdr,
            c.text_strong,
            c.panel_bg,
            true,
            false,
        );

        if r.h < 4 {
            return;
        }

        let layout = self.layout(r);

        // Section tabs (flowed across rows so they never overflow r.w)
        for (sec, label, tab_r) in layout.tab_rects {
            if tab_r.y >= r.y + r.h {
                break;
            }
            let active = self.section == sec;
            let bg = if active { c.row_selected_bg } else { c.surface_bg };
            let fg = if active { c.text_strong } else { c.text_muted };
            let trunc = primitives::truncate_to_width(label, tab_r.w as usize);
            renderer.draw_text_styled(tab_r.x, tab_r.y, &trunc, fg, bg, active, false);
        }

        // Open chat action button
        if layout.open_chat_row < r.y + r.h {
            let btn_trunc =
                primitives::truncate_to_width("[ Open chat ]", r.w.saturating_sub(2) as usize);
            renderer.draw_text_styled(
                r.x + 1,
                layout.open_chat_row,
                &btn_trunc,
                c.accent,
                c.surface_bg,
                true,
                false,
            );
        }

        let start_y = layout.list_start_y;
        let max_rows = layout.list_max_rows;
        self.list_state.adjust_scroll(max_rows);

        match self.section {
            AiSection::Agents => {
                for i in 0..max_rows {
                    let idx = self.list_state.scroll_offset + i;
                    let Some(agent) = self.agents.get(idx) else {
                        break;
                    };
                    let ry = start_y + i as u16;
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
                for i in 0..max_rows {
                    let idx = self.list_state.scroll_offset + i;
                    let Some((label, _)) = AI_CONTEXT_ITEMS.get(idx) else {
                        break;
                    };
                    let ry = start_y + i as u16;
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
                for i in 0..max_rows {
                    let idx = self.list_state.scroll_offset + i;
                    let Some((label, _)) = AI_ACTION_ITEMS.get(idx) else {
                        break;
                    };
                    let ry = start_y + i as u16;
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

        for (sy, line) in &layout.status_lines {
            renderer.draw_text(r.x + 1, *sy, line, c.text_muted, c.panel_bg);
        }
        for (label, _, act_r) in &layout.action_rects {
            let trunc = primitives::truncate_to_width(label, act_r.w as usize);
            renderer.draw_text(act_r.x, act_r.y, &trunc, c.text_muted, c.panel_bg);
        }
    }
}

impl Default for AiPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(renderer: &Renderer, y: u16, x_start: u16, x_end: u16) -> String {
        let mut s = String::new();
        for x in x_start..x_end.min(renderer.width) {
            let cell = &renderer.buf[y as usize * renderer.width as usize + x as usize];
            if !cell.continuation {
                s.push_str(std::str::from_utf8(&cell.ch[..cell.len as usize]).unwrap_or(" "));
            }
        }
        s
    }

    #[test]
    fn ai_panel_wraps_without_overflow_at_default_and_narrow_widths() {
        let mut panel = AiPanel::new();
        let theme = Theme::default();

        for width in [16u16, 20, 24, 36] {
            let mut renderer = Renderer::new(80, 30);
            let sentinel_bg = 0x123456;
            renderer.fill_rect(Rect::new(0, 0, 80, 30), sentinel_bg);
            let rect = Rect::new(0, 2, width, 26);
            panel.draw(&mut renderer, rect, &theme, true);

            // Nothing may spill past rect.w into the editor region (x >= width).
            for y in 0..30u16 {
                for x in width..80u16 {
                    let cell = &renderer.buf[y as usize * 80 + x as usize];
                    assert_eq!(
                        cell.bg, sentinel_bg,
                        "AI panel overflowed at width={} x={} y={}",
                        width, x, y
                    );
                    assert_eq!(
                        &cell.ch[..cell.len as usize],
                        b" ",
                        "AI panel text spilled past border at width={} x={} y={}",
                        width,
                        x,
                        y
                    );
                }
            }

            let all_sidebar_text: String = (rect.y..rect.y + rect.h)
                .map(|y| row_text(&renderer, y, 0, width))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(all_sidebar_text.contains("1 Agents"), "width={width}\n{all_sidebar_text}");
            assert!(all_sidebar_text.contains("2 Context"), "width={width}\n{all_sidebar_text}");
            assert!(all_sidebar_text.contains("3 Actions"), "width={width}\n{all_sidebar_text}");
            assert!(all_sidebar_text.contains("[f]ocus"), "width={width}\n{all_sidebar_text}");
            assert!(all_sidebar_text.contains("[r]estart"), "width={width}\n{all_sidebar_text}");
            assert!(all_sidebar_text.contains("[s]top"), "width={width}\n{all_sidebar_text}");
            assert!(all_sidebar_text.contains("[x]close"), "width={width}\n{all_sidebar_text}");
        }

        // At default width 24, "3 Actions" wraps onto the second tab row, while at 36 all 3 share one row.
        let layout_24 = panel.layout(Rect::new(0, 2, 24, 26));
        assert_eq!(layout_24.tab_rects[0].2.y, 3);
        assert_eq!(layout_24.tab_rects[1].2.y, 3);
        assert_eq!(layout_24.tab_rects[2].2.y, 4);

        let layout_36 = panel.layout(Rect::new(0, 2, 36, 26));
        assert_eq!(layout_36.tab_rects[0].2.y, 3);
        assert_eq!(layout_36.tab_rects[1].2.y, 3);
        assert_eq!(layout_36.tab_rects[2].2.y, 3);
    }
}
