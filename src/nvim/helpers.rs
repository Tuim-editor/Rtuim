use rmpv::Value;

use crate::nvim::rpc::{AsyncTag, RpcMessage};
use crate::tui::app::{App, FocusRegion, Mode, NvimWin, PaletteMode, TabInfo};
use crate::tui::events;
use crate::tui::layout::Rect;
use crate::tui::widgets::settings::SettingsConfig;

pub fn handle_rpc_message(app: &mut App, msg: RpcMessage) {
    match msg {
        RpcMessage::Notification { method, params } => {
            handle_editor_notification(app, &method, &params);
        }
        RpcMessage::Response {
            tag: Some(tag),
            error: _,
            result,
            ..
        } => {
            handle_async_response(app, tag, &result);
        }
        _ => {}
    }
}

fn handle_editor_notification(app: &mut App, method: &str, params: &[Value]) {
    match method {
        "redraw" => {
            app.ui_state.handle_redraw(params);
            app.invalidations.mark_editor_grid();
        }
        "tuim_buffers" => {
            if let Some(Value::Array(list)) = params.first() {
                let current_buf = params.get(1).and_then(|v| v.as_i64()).unwrap_or(1);
                let mut new_tabs = Vec::new();
                let mut active_idx = 0usize;
                for (idx, item) in list.iter().enumerate() {
                    if let Value::Map(entries) = item {
                        let mut tab = TabInfo::default();
                        for (k, v) in entries {
                            if let Some(key) = k.as_str() {
                                match key {
                                    "bufnr" => tab.bufnr = v.as_i64().unwrap_or(1),
                                    "name" => {
                                        tab.name = v.as_str().unwrap_or("").to_string();
                                    }
                                    "relative_name" => {
                                        tab.relative_name = v.as_str().unwrap_or("").to_string();
                                    }
                                    "changed" => tab.changed = v.as_bool().unwrap_or(false),
                                    _ => {}
                                }
                            }
                        }
                        if tab.bufnr == current_buf {
                            active_idx = idx;
                        }
                        new_tabs.push(tab);
                    }
                }
                if !new_tabs.is_empty() {
                    app.tabs = new_tabs;
                    app.active_tab = active_idx;
                }
                app.invalidations.mark_chrome();
            }
        }
        "tuim_telescope_rect" => {
            app.telescope_main_rect = parse_lua_rect(params.first());
            app.telescope_prev_rect = parse_lua_rect(params.get(1));
            app.telescope_title = params
                .get(2)
                .and_then(|v| v.as_str())
                .unwrap_or("Search")
                .to_string();
            app.native_picker_chrome = params.get(3).and_then(|v| v.as_bool()).unwrap_or(false);
            app.invalidations.mark_chrome();
        }
        "tuim_open_commands" => {
            events::open_palette(app, PaletteMode::Commands);
        }
        "tuim_open_language_tools" => {
            events::execute_command(app, "language_tools");
        }
        "tuim_open_terminal_right" => {
            events::execute_command(app, "terminal_right");
        }
        "tuim_toggle_explorer" => {
            events::execute_command(app, "toggle_explorer");
        }
        "tuim_toggle_terminal" => {
            let vertical = params.first().and_then(Value::as_str) == Some("vertical");
            events::execute_command(app, if vertical { "toggle_vertical_terminal" } else { "toggle_bottom_terminal" });
        }
        "tuim_toggle_zen" => {
            events::toggle_zen_mode(app);
        }
        "tuim_toggle_ide" => {
            app.mode = Mode::Ide;
            app.workspace.sidebar_visible = true;
            events::sync_layout(app);
        }
        "tuim_notice" => {
            let level = params.first().and_then(|v| v.as_str()).unwrap_or("info");
            let msg = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
            if !msg.is_empty() {
                app.set_notice(level, msg);
            }
        }
        "tuim_ai_status" => {
            let cmd = params.first().and_then(|v| v.as_str()).unwrap_or("");
            let state = params.get(1).and_then(|v| v.as_str()).unwrap_or("stopped");
            let active = params.get(2).and_then(|v| v.as_bool()).unwrap_or(false);
            app.ai_panel.update_status(cmd, state, active);
            app.invalidations.mark_chrome();
        }
        "tuim_settings_changed" => {
            if let Ok(cfg) = SettingsConfig::load(&app.settings_widget.settings_path) {
                app.mode = match cfg.mode.as_str() {
                    "zen" => Mode::Zen,
                    "normal" => Mode::Normal,
                    _ => Mode::Ide,
                };
                app.settings_widget.config = cfg.clone();
                app.settings_widget.saved_config = cfg;
                events::sync_layout(app);
            }
        }
        "tuim_win_count" => {
            if let Some(c) = params.first().and_then(|v| v.as_u64()) {
                app.nvim_win_count = c as usize;
                app.invalidations.mark_chrome();
            }
        }
        "tuim_win_positions" => {
            if let Some(Value::Array(list)) = params.first() {
                let mut wins = Vec::new();
                for item in list {
                    if let Value::Map(entries) = item {
                        let mut win = NvimWin::default();
                        for (k, v) in entries {
                            if let Some(key) = k.as_str() {
                                match key {
                                    "id" => win.id = v.as_i64().unwrap_or(0),
                                    "bufnr" => win.bufnr = v.as_i64().unwrap_or(1),
                                    "row" => win.row = v.as_u64().unwrap_or(0) as u16,
                                    "col" => win.col = v.as_u64().unwrap_or(0) as u16,
                                    "width" => win.width = v.as_u64().unwrap_or(0) as u16,
                                    "height" => win.height = v.as_u64().unwrap_or(0) as u16,
                                    "active" => win.active = v.as_bool().unwrap_or(false),
                                    "name" => win.name = v.as_str().unwrap_or("").to_string(),
                                    _ => {}
                                }
                            }
                        }
                        wins.push(win);
                    }
                }
                app.nvim_wins = wins;
                app.invalidations.mark_chrome();
            }
        }
        "tuim_boundary_hit" => {
            if let Some(dir) = params.first().and_then(|v| v.as_str()) {
                match dir {
                    "h" if app.workspace.sidebar_visible && app.mode != Mode::Zen => {
                        app.workspace.focus = FocusRegion::Sidebar;
                        app.invalidations.mark_chrome();
                    }
                    "l" if app.current_layout().vertical_terminal.w > 0 => {
                        app.workspace.focus = FocusRegion::VerticalTerminal;
                        app.invalidations.mark_chrome();
                    }
                    "j" if app.current_layout().terminal.h > 0 => {
                        app.workspace.focus = FocusRegion::Drawer;
                        app.invalidations.mark_chrome();
                    }
                    _ => {}
                }
            }
        }
        "tuim_theme_changed" => {
            if let Some(Value::Map(entries)) = params.first() {
                app.theme.update_from_config(entries);
                app.shell_terminal.set_theme(app.theme);
                app.vertical_terminal.set_theme(app.theme);
                app.invalidations.mark_terminal_grid();
                app.invalidations.mark_chrome();
            }
        }
        _ => {}
    }
}

fn parse_lua_rect(val: Option<&Value>) -> Option<Rect> {
    let arr = val?.as_array()?;
    if arr.len() < 4 {
        return None;
    }
    let row = arr[0].as_i64()?.max(0) as u16;
    let col = arr[1].as_i64()?.max(0) as u16;
    let w = arr[2].as_i64()?.max(0) as u16;
    let h = arr[3].as_i64()?.max(0) as u16;
    if w == 0 || h == 0 {
        return None;
    }
    Some(Rect { x: col, y: row, w, h })
}

fn handle_async_response(
    app: &mut App,
    tag: AsyncTag,
    result: &Value,
) {
    match tag {
        AsyncTag::ZenHandoff => {
            if let Some(arr) = result.as_array() {
                if let Some(file) = arr.first().and_then(|v| v.as_str()) {
                    app.zen_handoff_file = file.to_string();
                    app.zen_handoff_line = arr.get(1).and_then(|v| v.as_i64()).unwrap_or(1);
                    app.zen_handoff_requested = true;
                }
            }
        }
        AsyncTag::VerticalTerminalPaste => {
            if let Some(text) = result.as_str() { app.vertical_terminal.paste(text); }
            app.invalidations.mark_terminal_grid();
        }
        AsyncTag::TerminalPaste => {
            if let Some(text) = result.as_str() { app.shell_terminal.paste(text); }
            app.invalidations.mark_terminal_grid();
        }
        AsyncTag::QuitConfirmation
        | AsyncTag::Reload
        | AsyncTag::TerminalAdded
        | AsyncTag::TerminalRemoved => {
            app.invalidations.mark_chrome();
        }
    }
}
