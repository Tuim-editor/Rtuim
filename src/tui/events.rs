use rmpv::Value;
use unicode_width::UnicodeWidthStr;

use crate::nvim::rpc::AsyncTag;
use crate::tui::app::{App, FocusRegion, Mode, PaletteMode};
use crate::tui::input::InputEvent;
use crate::tui::layout::Rect;
use crate::tui::widgets::explorer::InputMode;
use self::shop_types_helper::has_active_submodal;
use crate::tui::workspace::{self, SidebarAction, SidebarRow};

pub mod shop_types_helper {
    use crate::tui::widgets::extension_shop::{ExtensionShop, ShopConfirm};
    pub fn has_active_submodal(shop: &ExtensionShop) -> bool {
        shop.confirm_modal != ShopConfirm::None
            || shop.detail_open
            || shop.is_searching
    }
}

fn ensure_shell_terminal(app: &mut App) {
    let rect = app.current_layout().terminal;
    app.shell_terminal.set_theme(app.theme);
    app.shell_terminal.ensure_running(rect.w.max(1), rect.h.max(1));
    app.invalidations.mark_terminal_grid();
}

fn process_terminal_clipboard(app: &mut App) {
    let vertical = app.workspace.focus == FocusRegion::VerticalTerminal;
    let terminal = if vertical { &mut app.vertical_terminal } else { &mut app.shell_terminal };
    let copy = terminal.copy_requested.take();
    let paste = std::mem::take(&mut terminal.paste_requested);
    if let Some(text) = copy {
        let _ = app.editor_rpc.notify("nvim_exec_lua", vec![Value::from(
            r#"local text, system = ...; vim.fn.setreg('"', text); if system then pcall(vim.fn.setreg, '+', text) end"#),
            Value::Array(vec![Value::from(text), Value::from(app.settings_widget.config.clipboard)])]);
    }
    if paste {
        let _ = app.editor_rpc.request_async("nvim_exec_lua", vec![Value::from(
            r#"local system = ...; if system then local ok, text = pcall(vim.fn.getreg, '+'); if ok and text ~= '' then return text end end; return vim.fn.getreg('"')"#),
            Value::Array(vec![Value::from(app.settings_widget.config.clipboard)])], if vertical { AsyncTag::VerticalTerminalPaste } else { AsyncTag::TerminalPaste });
    }
}

pub fn sync_layout(app: &mut App) {
    let layout = app.current_layout();
    if layout.editor.w > 0 && layout.editor.h > 0 {
        let ew = layout.editor.w as usize;
        let eh = layout.editor.h as usize;
        if app.ui_state.grid.width != ew || app.ui_state.grid.height != eh {
            app.ui_state.grid.resize(ew, eh);
            let _ = app.editor_rpc.notify(
                "nvim_ui_try_resize",
                vec![
                    Value::from(layout.editor.w as i64),
                    Value::from(layout.editor.h as i64),
                ],
            );
        }
    }

    if app.workspace.drawer_visible
        && layout.terminal.w > 0
        && layout.terminal.h > 0
    {
        app.shell_terminal.resize(layout.terminal.w, layout.terminal.h);
    }

    app.vertical_terminal.resize(layout.vertical_terminal.w, layout.vertical_terminal.h);
    app.invalidations.mark_layout();
}

pub fn toggle_zen_mode(app: &mut App) {
    let _ = app.editor_rpc.notify(
        "nvim_exec_lua",
        vec![
            Value::from("if _G.tuim_close_floating_windows then _G.tuim_close_floating_windows() end"),
            Value::Array(vec![]),
        ],
    );
    app.telescope_main_rect = None;
    app.telescope_prev_rect = None;
    app.native_picker_chrome = false;

    if app.mode != Mode::Zen {
        app.workspace.zen_return_mode = app.mode;
        app.workspace.zen_return_sidebar = app.workspace.sidebar_visible;
        app.workspace.zen_return_drawer = app.workspace.drawer_visible;
        app.workspace.zen_return_vertical_terminal = app.workspace.vertical_terminal_visible;
        app.workspace.drawer_visible = false;
        app.workspace.vertical_terminal_visible = false;
        app.workspace.zen_return_focus = app.workspace.focus;
        app.mode = Mode::Zen;
        app.workspace.focus = FocusRegion::Editor;
        let _ = app.editor_rpc.notify(
            "nvim_exec_lua",
            vec![
                Value::from("vim.g.tuim_zen_mode = true"),
                Value::Array(vec![]),
            ],
        );
    } else {
        app.mode = app.workspace.zen_return_mode;
        app.workspace.sidebar_visible = app.workspace.zen_return_sidebar;
        app.workspace.drawer_visible = app.workspace.zen_return_drawer;
        app.workspace.vertical_terminal_visible = app.workspace.zen_return_vertical_terminal;
        app.workspace.focus = app.workspace.zen_return_focus;
        let _ = app.editor_rpc.notify(
            "nvim_exec_lua",
            vec![
                Value::from("vim.g.tuim_zen_mode = false"),
                Value::Array(vec![]),
            ],
        );
    }
    sync_layout(app);
}

pub fn trigger_quit(app: &mut App) {
    if app.editor_rpc.has_pending_tag(AsyncTag::QuitConfirmation) {
        return;
    }
    app.workspace.focus = FocusRegion::Editor;
    let _ = app.editor_rpc.request_async(
        "nvim_exec_lua",
        vec![
            Value::from(
                "if _G.tuim_confirm_quit then _G.tuim_confirm_quit() else vim.cmd('confirm qall') end",
            ),
            Value::Array(vec![]),
        ],
        AsyncTag::QuitConfirmation,
    );
}

pub fn open_palette(app: &mut App, mode: PaletteMode) {
    let _ = app.editor_rpc.notify(
        "nvim_exec_lua",
        vec![
            Value::from("if _G.tuim_close_floating_windows then _G.tuim_close_floating_windows() end"),
            Value::Array(vec![]),
        ],
    );
    app.telescope_main_rect = None;
    app.telescope_prev_rect = None;
    app.native_picker_chrome = false;
    app.workspace.palette_open = true;
    app.workspace.palette_mode = mode;
    app.workspace.palette_query.clear();
    app.workspace.palette_index = 0;
    app.workspace.palette_scroll = 0;
    app.workspace.palette_recording_key = false;
    app.workspace.palette_status.clear();
    app.invalidations.mark_chrome();
}

pub fn execute_command(app: &mut App, cmd_id: &str) {
    match cmd_id {
        "find_file" => {
            app.extension_shop.is_open = false;
            app.workspace.focus = FocusRegion::Editor;
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from(
                        "if _G.tuim_close_floating_windows then _G.tuim_close_floating_windows() end; if vim.fn.exists(':Telescope') == 2 then vim.cmd('Telescope find_files') else vim.api.nvim_feedkeys(':edit ', 'n', false) end",
                    ),
                    Value::Array(vec![]),
                ],
            );
        }
        "search_project" => {
            app.extension_shop.is_open = false;
            app.workspace.focus = FocusRegion::Editor;
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from(
                        "if _G.tuim_close_floating_windows then _G.tuim_close_floating_windows() end; if vim.fn.exists(':Telescope') == 2 then vim.cmd('Telescope live_grep') else vim.api.nvim_feedkeys(':vimgrep // **/*', 'n', false) end",
                    ),
                    Value::Array(vec![]),
                ],
            );
        }
        "new_file" => {
            app.extension_shop.is_open = false;
            app.workspace.focus = FocusRegion::Editor;
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from(
                        "if _G.tuim_close_floating_windows then _G.tuim_close_floating_windows() end; if _G.tuim_new_primary_buffer then _G.tuim_new_primary_buffer() else vim.cmd('enew') end",
                    ),
                    Value::Array(vec![]),
                ],
            );
        }
        "save_file" => {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from(
                        "if _G.tuim_save_file then _G.tuim_save_file() else vim.cmd('silent! write') end",
                    ),
                    Value::Array(vec![]),
                ],
            );
        }
        "close_buffer" => {
            app.workspace.focus = FocusRegion::Editor;
            let _ = app.editor_rpc.request_async(
                "nvim_exec_lua",
                vec![
                    Value::from(
                        "if _G.tuim_close_buffer then _G.tuim_close_buffer(vim.api.nvim_get_current_buf()) else vim.cmd('confirm bdelete') end",
                    ),
                    Value::Array(vec![]),
                ],
                AsyncTag::QuitConfirmation,
            );
        }
        "switch_buffers" => {
            open_palette(app, PaletteMode::Buffers);
        }
        "project_files" | "explorer" => {
            app.extension_shop.is_open = false;
            app.workspace.sidebar_visible = true;
            app.workspace.overview = false;
            app.activity_bar.active_tab = 0;
            app.workspace.focus = FocusRegion::Sidebar;
            sync_layout(app);
        }
        "changes" => {
            app.extension_shop.is_open = false;
            app.workspace.sidebar_visible = true;
            app.workspace.overview = false;
            app.activity_bar.active_tab = 1;
            app.git_panel.refresh_sync();
            app.workspace.focus = FocusRegion::Sidebar;
            sync_layout(app);
        }
        "problems" => {
            app.extension_shop.is_open = false;
            app.workspace.focus = FocusRegion::Editor;
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from(
                        "if _G.tuim_close_floating_windows then _G.tuim_close_floating_windows() end; if vim.fn.exists(':Telescope') == 2 then vim.cmd('Telescope diagnostics') else vim.diagnostic.setqflist() end",
                    ),
                    Value::Array(vec![]),
                ],
            );
        }
        "toggle_vertical_terminal" | "terminal_right" => {
            app.workspace.vertical_terminal_visible = cmd_id == "terminal_right" || !app.workspace.vertical_terminal_visible;
            if app.workspace.vertical_terminal_visible {
                app.workspace.focus = FocusRegion::VerticalTerminal;
                let rect = app.current_layout().vertical_terminal;
                app.vertical_terminal.set_theme(app.theme);
                app.vertical_terminal.ensure_running(rect.w.max(1), rect.h.max(1));
            } else if app.workspace.focus == FocusRegion::VerticalTerminal {
                app.workspace.focus = FocusRegion::Editor;
            }
            sync_layout(app);
        }
        "terminal" | "toggle_terminal" | "toggle_bottom_terminal" => {
            if !app.workspace.drawer_visible {
                app.workspace.drawer_visible = true;
                app.workspace.drawer_tab = 0;
                app.workspace.focus = FocusRegion::Drawer;
                ensure_shell_terminal(app);
            } else if (cmd_id != "toggle_bottom_terminal" && app.workspace.focus != FocusRegion::Drawer) || app.workspace.drawer_tab != 0 {
                app.workspace.drawer_tab = 0;
                app.workspace.focus = FocusRegion::Drawer;
                ensure_shell_terminal(app);
            } else {
                app.workspace.drawer_visible = false;
                app.workspace.focus = FocusRegion::Editor;
            }
            sync_layout(app);
        }
        "toggle_explorer" => {
            if !app.workspace.sidebar_visible {
                app.workspace.sidebar_visible = true;
                app.workspace.overview = false;
                app.activity_bar.active_tab = 0;
                app.workspace.focus = FocusRegion::Sidebar;
            } else if app.workspace.focus != FocusRegion::Sidebar {
                app.workspace.focus = FocusRegion::Sidebar;
            } else {
                app.workspace.sidebar_visible = false;
                app.workspace.focus = FocusRegion::Editor;
            }
            sync_layout(app);
        }
        "focus_next" | "focus_prev" => {
            let layout = app.current_layout();
            let mut regions = vec![FocusRegion::Editor];
            if layout.sidebar.w > 0 && layout.sidebar.h > 0 { regions.push(FocusRegion::Sidebar); }
            if layout.terminal.w > 0 && layout.terminal.h > 0 { regions.push(FocusRegion::Drawer); }
            if layout.vertical_terminal.w > 0 && layout.vertical_terminal.h > 0 { regions.push(FocusRegion::VerticalTerminal); }
            let current = regions.iter().position(|r| *r == app.workspace.focus).unwrap_or(0);
            let offset = if cmd_id == "focus_prev" { regions.len() - 1 } else { 1 };
            let next = regions[(current + offset) % regions.len()];
            app.workspace.focus = next;
            if next == FocusRegion::Drawer && app.workspace.drawer_tab == 0 {
                ensure_shell_terminal(app);
            }
            app.invalidations.mark_chrome();
        }
        "resize_left" | "resize_right" | "resize_up" | "resize_down" => {
            let layout = app.current_layout();
            match cmd_id {
                "resize_left" | "resize_right" => {
                    let left = cmd_id == "resize_left";
                    if layout.vertical_terminal.w > 0 && (app.workspace.focus == FocusRegion::VerticalTerminal || layout.sidebar.w == 0) {
                        let width = layout.vertical_terminal.w;
                        let max = width.saturating_add(layout.editor.w).saturating_sub(5).max(6);
                        app.vertical_terminal_w = if left { width.saturating_add(2).min(max) } else { width.saturating_sub(2).max(6).min(max) };
                    } else if layout.sidebar.w > 0 && layout.sidebar.h > 0 {
                        let width = layout.sidebar.w;
                        let max = layout.total.w.saturating_sub(24).max(16);
                        app.sidebar_w = if left { width.saturating_sub(2).max(16) } else { width.saturating_add(2).min(max) };
                    }
                }
                _ if layout.terminal.h > 0 => {
                    let max = layout.terminal.h.saturating_add(layout.editor.h).saturating_sub(2).max(2);
                    app.terminal_h = if cmd_id == "resize_up" { layout.terminal.h.saturating_add(1).min(max) } else { layout.terminal.h.saturating_sub(1).max(2) };
                }
                _ => {}
            }
            sync_layout(app);
        }
        "zen" | "toggle_zen" => {
            toggle_zen_mode(app);
        }
        "ai_assistants" => {
            app.extension_shop.is_open = false;
            app.workspace.sidebar_visible = true;
            app.workspace.overview = false;
            app.activity_bar.active_tab = 2;
            app.ai_panel.refresh_agents();
            app.workspace.focus = FocusRegion::Sidebar;
            sync_layout(app);
        }
        "orchestrator" => {
            app.orchestrator.is_open = true;
            app.request_git_refresh();
            app.invalidations.mark_chrome();
        }
        "extensions" => {
            app.workspace.prior_focus = app.workspace.focus;
            app.extension_shop.is_open = true;
            app.extension_shop.refresh();
            app.invalidations.mark_chrome();
        }
        "language_tools" => {
            app.workspace.prior_focus = app.workspace.focus;
            app.mason_widget.is_open = true;
            app.mason_widget.refresh(&mut app.editor_rpc);
            app.invalidations.mark_chrome();
        }
        "settings" => {
            app.workspace.prior_focus = app.workspace.focus;
            app.settings_widget.is_open = true;
            app.settings_widget.search_query.clear();
            app.settings_widget.is_searching = false;
            app.settings_widget.status_msg.clear();
            app.settings_widget.active_tab = 0;
            app.settings_widget.selected_row = 0;
            app.settings_widget.focus_sidebar = true;
            app.invalidations.mark_chrome();
        }
        "keyboard_shortcuts" => {
            app.workspace.prior_focus = app.workspace.focus;
            app.settings_widget.is_open = true;
            app.settings_widget.search_query.clear();
            app.settings_widget.is_searching = false;
            app.settings_widget.status_msg.clear();
            app.settings_widget.active_tab = 4;
            app.settings_widget.selected_row = 0;
            app.settings_widget.focus_sidebar = true;
            app.invalidations.mark_chrome();
        }
        "help" => {
            app.extension_shop.is_open = false;
            app.workspace.focus = FocusRegion::Editor;
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.open_help_menu then _G.open_help_menu() end"),
                    Value::Array(vec![]),
                ],
            );
        }
        "split_right" => {
            app.workspace.focus = FocusRegion::Editor;
            let _ = app
                .editor_rpc
                .notify("nvim_command", vec![Value::from("vsplit")]);
        }
        "split_down" => {
            app.workspace.focus = FocusRegion::Editor;
            let _ = app
                .editor_rpc
                .notify("nvim_command", vec![Value::from("split")]);
        }
        "report_bug" => {
            app.bug_report.open();
            app.invalidations.mark_chrome();
        }
        "undo" => {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.tuim_ide_action then _G.tuim_ide_action('undo') else pcall(vim.cmd, 'undo') end"),
                    Value::Array(vec![]),
                ],
            );
        }
        "redo" => {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.tuim_ide_action then _G.tuim_ide_action('redo') else pcall(vim.cmd, 'redo') end"),
                    Value::Array(vec![]),
                ],
            );
        }
        "find" => {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.tuim_ide_action then _G.tuim_ide_action('find') end"),
                    Value::Array(vec![]),
                ],
            );
        }
        "replace" => {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.tuim_ide_action then _G.tuim_ide_action('replace') end"),
                    Value::Array(vec![]),
                ],
            );
        }
        "onboarding" => {
            app.workspace.focus = FocusRegion::Editor;
            let _ = app
                .editor_rpc
                .notify("nvim_command", vec![Value::from("TuimOnboarding")]);
        }
        "quit" => {
            trigger_quit(app);
        }
        _ => {}
    }
    app.invalidations.mark_chrome();
}

fn process_widget_side_effects(app: &mut App) {
    if let Some(path) = app.explorer.open_file_requested.take() {
        app.extension_shop.is_open = false;
        app.workspace.focus = FocusRegion::Editor;
        let escaped = path.replace('\'', "''");
        let lua = format!(
            "vim.cmd('edit ' .. vim.fn.fnameescape('{}'))",
            escaped
        );
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
        app.request_git_refresh();
    }
    if let Some(_preview) = app.explorer.preview_file_requested.take() {
        // Keep focus in sidebar during preview navigation
    }
    if let Some(del_path) = app.explorer.delete_buffer_requested.take() {
        let escaped = del_path.replace('\'', "''");
        let lua = format!(
            "for _, b in ipairs(vim.api.nvim_list_bufs()) do if vim.api.nvim_buf_is_valid(b) and vim.api.nvim_buf_get_name(b):find('{}', 1, true) then pcall(vim.api.nvim_buf_delete, b, {{force=true}}) end end",
            escaped
        );
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
        app.request_git_refresh();
    }
    if let Some((old_p, new_p)) = app.explorer.rename_buffer_requested.take() {
        let old_e = old_p.replace('\'', "''");
        let new_e = new_p.replace('\'', "''");
        let lua = format!(
            "for _, b in ipairs(vim.api.nvim_list_bufs()) do if vim.api.nvim_buf_is_valid(b) and vim.api.nvim_buf_get_name(b) == '{}' then pcall(vim.api.nvim_buf_set_name, b, '{}') end end",
            old_e, new_e
        );
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
        app.request_git_refresh();
    }

    if let Some(path) = app.git_panel.open_file_requested.take() {
        app.extension_shop.is_open = false;
        app.workspace.focus = FocusRegion::Editor;
        let escaped = path.replace('\'', "''");
        let lua = format!(
            "vim.cmd('edit ' .. vim.fn.fnameescape('{}'))",
            escaped
        );
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
    }
    if app.git_panel.open_detailed_requested {
        app.git_panel.open_detailed_requested = false;
        app.git_detailed_widget.is_open = true;
        app.git_detailed_widget
            .apply_snapshot(app.git_panel.snapshot.clone());
    }
    if app.git_panel.refresh_requested || app.git_detailed_widget.refresh_requested {
        app.git_panel.refresh_requested = false;
        app.git_detailed_widget.refresh_requested = false;
        app.explorer.update_git_status(&app.git_panel.snapshot);
        app.request_git_refresh();
    }

    if app.ai_panel.focus_editor_requested {
        app.ai_panel.focus_editor_requested = false;
        app.workspace.focus = FocusRegion::Editor;
    }

    if let Some((path, line, col)) = app.search_panel.open_hit_requested.take() {
        app.extension_shop.is_open = false;
        app.workspace.focus = FocusRegion::Editor;
        let escaped = path.replace('\'', "''");
        let lua = format!(
            "vim.cmd('edit ' .. vim.fn.fnameescape('{}')); pcall(vim.api.nvim_win_set_cursor, 0, {{{}, {}}})",
            escaped,
            line,
            col.saturating_sub(1)
        );
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
    }

    if let Some(cfg_path) = app.extension_shop.edit_config_path.take() {
        app.workspace.focus = FocusRegion::Editor;
        let escaped = cfg_path.replace('\'', "''");
        let lua = format!(
            "vim.cmd('edit ' .. vim.fn.fnameescape('{}'))",
            escaped
        );
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
    }
    if app.extension_shop.sync_requested {
        app.extension_shop.sync_requested = false;
        let _ = app.editor_rpc.notify(
            "nvim_exec_lua",
            vec![
                Value::from("if not vim.g.tuim_plugins_disabled then pcall(vim.cmd, 'Lazy sync') end"),
                Value::Array(vec![]),
            ],
        );
    }
    if app.extension_shop.restart_requested {
        app.extension_shop.restart_requested = false;
        app.restart_requested = true;
    }

    if app.settings_widget.open_mason_requested {
        app.settings_widget.open_mason_requested = false;
        app.mason_widget.is_open = true;
        app.mason_widget.refresh(&mut app.editor_rpc);
    }
    if app.settings_widget.open_lazy_requested {
        app.settings_widget.open_lazy_requested = false;
        app.lazy_widget.is_open = true;
        app.lazy_widget.refresh(&mut app.editor_rpc);
    }
    if app.settings_widget.open_shop_requested {
        app.settings_widget.open_shop_requested = false;
        execute_command(app, "extensions");
    }

    if let Some((agent_cmd, _wt_path)) = app.orchestrator.open_agent_cmd.take() {
        app.workspace.focus = FocusRegion::Editor;
        let lua = format!("if _G.OpenAITerminal then _G.OpenAITerminal('{}') end", agent_cmd);
        let _ = app
            .editor_rpc
            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
    }
    if app.orchestrator.refresh_requested {
        app.orchestrator.refresh_requested = false;
        app.request_git_refresh();
    }
}

fn handle_palette_key(app: &mut App, ev: &InputEvent) -> bool {
    let key = match ev {
        InputEvent::Key(k) => k.as_str(),
        InputEvent::Paste(text) => {
            for ch in text.chars() {
                if ch != '\n' && ch != '\r' {
                    app.workspace.palette_query.push(ch);
                }
            }
            app.workspace.palette_index = 0;
            app.workspace.palette_scroll = 0;
            return true;
        }
        _ => return false,
    };

    if app.workspace.palette_recording_key {
        if key == "<Esc>" {
            app.workspace.palette_recording_key = false;
            app.workspace.palette_status.clear();
            return true;
        }
        let cmds = workspace::filtered_commands(app);
        if let Some(cmd) = cmds.get(app.workspace.palette_index) {
            if !cmd.key_field.is_empty() {
                if app
                    .settings_widget
                    .config
                    .keybindings
                    .has_conflict(cmd.key_field, key)
                {
                    app.workspace.palette_status = "Shortcut already in use".to_string();
                } else {
                    app.settings_widget
                        .config
                        .keybindings
                        .set_by_field(cmd.key_field, key.to_string());
                    app.settings_widget
                        .saved_config
                        .keybindings
                        .set_by_field(cmd.key_field, key.to_string());
                    let _ = app
                        .settings_widget
                        .config
                        .save(&app.settings_widget.settings_path);
                    app.workspace.palette_status = "Shortcut saved".to_string();
                    app.workspace.palette_recording_key = false;
                }
            }
        }
        return true;
    }

    match key {
        "<Esc>" => {
            app.workspace.palette_open = false;
            app.workspace.palette_query.clear();
            app.workspace.palette_status.clear();
            true
        }
        "<F2>" => {
            if app.workspace.palette_mode == PaletteMode::Commands {
                let cmds = workspace::filtered_commands(app);
                if let Some(cmd) = cmds.get(app.workspace.palette_index) {
                    if !cmd.key_field.is_empty() {
                        app.workspace.palette_recording_key = true;
                        app.workspace.palette_status.clear();
                    }
                }
            }
            true
        }
        "<Up>" | "<C-p>" => {
            app.workspace.palette_index = app.workspace.palette_index.saturating_sub(1);
            true
        }
        "<Down>" | "<C-n>" => {
            let max = match app.workspace.palette_mode {
                PaletteMode::Commands => workspace::filtered_commands(app).len(),
                PaletteMode::Buffers => workspace::filtered_buffers(app).len(),
                PaletteMode::LineJump => 1,
            };
            if app.workspace.palette_index + 1 < max {
                app.workspace.palette_index += 1;
            }
            true
        }
        "<CR>" => {
            match app.workspace.palette_mode {
                PaletteMode::Commands => {
                    let cmds = workspace::filtered_commands(app);
                    if let Some(cmd) = cmds.get(app.workspace.palette_index) {
                        let id = cmd.id;
                        app.workspace.palette_open = false;
                        execute_command(app, id);
                    }
                }
                PaletteMode::Buffers => {
                    let bufs = workspace::filtered_buffers(app);
                    if let Some(&(_, _, bufnr)) = bufs.get(app.workspace.palette_index) {
                        app.workspace.palette_open = false;
                        app.workspace.focus = FocusRegion::Editor;
                        let lua = format!(
                            "if _G.tuim_select_buffer then _G.tuim_select_buffer({}) else vim.api.nvim_set_current_buf({}) end",
                            bufnr, bufnr
                        );
                        let _ = app
                            .editor_rpc
                            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
                    }
                }
                PaletteMode::LineJump => {
                    let line_str = app.workspace.palette_query.trim_start_matches(':').trim();
                    if let Ok(line_no) = line_str.parse::<u32>() {
                        app.workspace.palette_open = false;
                        app.workspace.focus = FocusRegion::Editor;
                        let _ = app
                            .editor_rpc
                            .notify("nvim_command", vec![Value::from(line_no.to_string())]);
                    }
                }
            }
            true
        }
        "<BS>" => {
            app.workspace.palette_query.pop();
            app.workspace.palette_index = 0;
            app.workspace.palette_scroll = 0;
            if app.workspace.palette_query.starts_with('#') {
                app.workspace.palette_mode = PaletteMode::Buffers;
            } else if app.workspace.palette_query.starts_with(':') {
                app.workspace.palette_mode = PaletteMode::LineJump;
            } else if app.workspace.palette_query.starts_with('>') {
                app.workspace.palette_mode = PaletteMode::Commands;
            }
            true
        }
        k if k.chars().count() == 1 && !k.starts_with('<') => {
            app.workspace.palette_query.push_str(k);
            app.workspace.palette_index = 0;
            app.workspace.palette_scroll = 0;
            if app.workspace.palette_query.starts_with('#') {
                app.workspace.palette_mode = PaletteMode::Buffers;
            } else if app.workspace.palette_query.starts_with(':') {
                app.workspace.palette_mode = PaletteMode::LineJump;
            } else if app.workspace.palette_query.starts_with('>') {
                app.workspace.palette_mode = PaletteMode::Commands;
            }
            true
        }
        _ => true,
    }
}

pub fn handle_event(app: &mut App, ev: InputEvent) {
    app.metrics.inc_input_events();
    let screen = Rect::new(0, 0, app.renderer.width, app.renderer.height);

    match &ev {
        InputEvent::Resize(cols, rows) => {
            app.renderer.resize(*cols, *rows);
            app.last_dims.cols = *cols;
            app.last_dims.rows = *rows;
            sync_layout(app);
            return;
        }
        InputEvent::Mouse {
            x,
            y,
            button,
            pressed,
            motion,
        } => {
            handle_mouse_event(app, *x, *y, *button, *pressed, *motion);
            process_widget_side_effects(app);
            return;
        }
        _ => {}
    }

    // 1. Modal keyboard handlers
    if app.bug_report.is_open {
        let diag = app.metrics.to_diagnostics_json(
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            match app.mode {
                Mode::Ide => "ide",
                Mode::Zen => "zen",
                Mode::Normal => "normal",
            },
            &app.settings_widget.config.theme,
            app.capabilities.true_color,
            app.capabilities.mouse,
            app.capabilities.bracketed_paste,
            app.capabilities.nerd_fonts_recommended,
            app.renderer.width,
            app.renderer.height,
        );
        app.bug_report.handle_key(&ev, &diag);
        app.invalidations.mark_chrome();
        return;
    }

    if app.settings_widget.is_open {
        let was_open = app.settings_widget.is_open;
        app.settings_widget.handle_key(&ev, &mut app.editor_rpc);
        if was_open && !app.settings_widget.is_open {
            app.workspace.focus = app.workspace.prior_focus;
        }
        process_widget_side_effects(app);
        app.invalidations.mark_chrome();
        return;
    }

    if app.mason_widget.is_open {
        let was_open = app.mason_widget.is_open;
        app.mason_widget.handle_key(&ev, &mut app.editor_rpc);
        if was_open && !app.mason_widget.is_open {
            app.workspace.focus = app.workspace.prior_focus;
        }
        app.invalidations.mark_chrome();
        return;
    }

    if app.lazy_widget.is_open {
        let was_open = app.lazy_widget.is_open;
        app.lazy_widget.handle_key(&ev, &mut app.editor_rpc);
        if was_open && !app.lazy_widget.is_open {
            app.workspace.focus = app.workspace.prior_focus;
        }
        app.invalidations.mark_chrome();
        return;
    }

    if app.git_detailed_widget.is_open {
        app.git_detailed_widget.handle_key(&ev);
        process_widget_side_effects(app);
        app.invalidations.mark_chrome();
        return;
    }

    if app.orchestrator.is_open {
        app.orchestrator.handle_key(&ev);
        process_widget_side_effects(app);
        app.invalidations.mark_chrome();
        return;
    }

    if app.menu_bar.open_menu.is_some() {
        let handled = app.menu_bar.handle_key(&ev);
        if let Some(action) = app.menu_bar.triggered_action.take() {
            execute_command(app, action);
        }
        if handled {
            app.invalidations.mark_chrome();
            return;
        }
    }

    if app.editor_context_menu.is_open {
        app.editor_context_menu.handle_key(&ev, &mut app.editor_rpc);
        app.invalidations.mark_chrome();
        return;
    }

    if app.workspace.palette_open {
        handle_palette_key(app, &ev);
        app.invalidations.mark_chrome();
        return;
    }

    let key = match &ev {
        InputEvent::Key(k) => k.as_str(),
        InputEvent::Paste(text) => {
            if app.extension_shop.is_open {
                app.extension_shop.handle_key(&ev);
            } else if app.workspace.focus == FocusRegion::VerticalTerminal {
                app.vertical_terminal.paste(text);
            } else if app.workspace.focus == FocusRegion::Drawer && app.workspace.drawer_tab == 0 {
                app.shell_terminal.paste(text);
            } else if app.workspace.focus == FocusRegion::Sidebar && !app.workspace.overview {
                match app.activity_bar.active_tab {
                    0 => {
                        app.explorer.handle_key(&ev);
                    }
                    1 => {
                        app.git_panel.handle_key(&ev);
                    }
                    4 => {
                        app.search_panel.handle_key(&ev);
                    }
                    _ => {}
                }
                process_widget_side_effects(app);
            } else {
                let _ = app.editor_rpc.notify(
                    "nvim_paste",
                    vec![Value::from(text.as_str()), Value::from(true), Value::from(-1)],
                );
            }
            app.invalidations.mark_chrome();
            return;
        }
        _ => return,
    };

    // If QuitConfirmation is pending in Neovim, forward confirmation keys directly to Neovim
    if app.editor_rpc.has_pending_tag(AsyncTag::QuitConfirmation) {
        if key == app.settings_widget.config.keybindings.quit {
            return;
        }
        let mapped = if key == "<Esc>" { "c" } else { key };
        let _ = app
            .editor_rpc
            .notify("nvim_input", vec![Value::from(mapped)]);
        return;
    }

    // Native Telescope picker interception
    if app.native_picker_chrome && app.telescope_main_rect.is_some() {
        if key == "<Esc>" {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.tuim_picker_action then _G.tuim_picker_action('close') end"),
                    Value::Array(vec![]),
                ],
            );
            return;
        }
        if key == "<M-p>" {
            let _ = app.editor_rpc.notify(
                "nvim_exec_lua",
                vec![
                    Value::from("if _G.tuim_picker_action then _G.tuim_picker_action('preview') end"),
                    Value::Array(vec![]),
                ],
            );
            return;
        }
    }

    // Global keybindings check
    let in_terminal =
        (app.workspace.focus == FocusRegion::Drawer && app.workspace.drawer_tab == 0)
        || app.workspace.focus == FocusRegion::VerticalTerminal;
    let in_picker = app.native_picker_chrome && app.telescope_main_rect.is_some();
    let matched_cmd = {
        let kb = &app.settings_widget.config.keybindings;
        if !kb.commands.is_empty() && key.eq_ignore_ascii_case(&kb.commands) {
            Some("commands")
        } else if !kb.quit.is_empty() && key.eq_ignore_ascii_case(&kb.quit) {
            Some("quit")
        } else if !kb.toggle_zen.is_empty() && key.eq_ignore_ascii_case(&kb.toggle_zen) {
            Some("toggle_zen")
        } else if !kb.toggle_terminal.is_empty() && key.eq_ignore_ascii_case(&kb.toggle_terminal) {
            Some("toggle_terminal")
        } else if !in_terminal && !kb.toggle_explorer.is_empty() && key.eq_ignore_ascii_case(&kb.toggle_explorer) {
            Some("toggle_explorer")
        } else if !kb.focus_next.is_empty() && key.eq_ignore_ascii_case(&kb.focus_next) {
            Some("focus_next")
        } else if !kb.focus_prev.is_empty() && key.eq_ignore_ascii_case(&kb.focus_prev) {
            Some("focus_prev")
        } else if !kb.resize_left.is_empty() && key.eq_ignore_ascii_case(&kb.resize_left) {
            Some("resize_left")
        } else if !kb.resize_right.is_empty() && key.eq_ignore_ascii_case(&kb.resize_right) {
            Some("resize_right")
        } else if !kb.resize_up.is_empty() && key.eq_ignore_ascii_case(&kb.resize_up) {
            Some("resize_up")
        } else if !kb.resize_down.is_empty() && key.eq_ignore_ascii_case(&kb.resize_down) {
            Some("resize_down")
        } else if !kb.report_bug.is_empty() && key.eq_ignore_ascii_case(&kb.report_bug) {
            Some("report_bug")
        } else if !in_terminal {
            if !kb.save_file.is_empty() && key.eq_ignore_ascii_case(&kb.save_file) {
                Some("save_file")
            } else if !kb.new_file.is_empty() && key.eq_ignore_ascii_case(&kb.new_file) {
                if in_picker {
                    None
                } else {
                    Some("new_file")
                }
            } else if !kb.find_file.is_empty() && key.eq_ignore_ascii_case(&kb.find_file) {
                if in_picker {
                    None
                } else {
                    Some("find_file")
                }
            } else if !kb.search_project.is_empty() && key.eq_ignore_ascii_case(&kb.search_project)
            {
                Some("search_project")
            } else if !kb.close_buffer.is_empty() && key.eq_ignore_ascii_case(&kb.close_buffer) {
                Some("close_buffer")
            } else if !kb.switch_buffers.is_empty() && key.eq_ignore_ascii_case(&kb.switch_buffers)
            {
                Some("switch_buffers")
            } else if !kb.project_files.is_empty() && key.eq_ignore_ascii_case(&kb.project_files) {
                Some("project_files")
            } else if !kb.changes.is_empty() && key.eq_ignore_ascii_case(&kb.changes) {
                Some("changes")
            } else if !kb.problems.is_empty() && key.eq_ignore_ascii_case(&kb.problems) {
                Some("problems")
            } else if !kb.ai_assistants.is_empty() && key.eq_ignore_ascii_case(&kb.ai_assistants) {
                Some("ai_assistants")
            } else if !kb.extensions.is_empty() && key.eq_ignore_ascii_case(&kb.extensions) {
                Some("extensions")
            } else if !kb.language_tools.is_empty() && key.eq_ignore_ascii_case(&kb.language_tools)
            {
                Some("language_tools")
            } else if !kb.settings.is_empty() && key.eq_ignore_ascii_case(&kb.settings) {
                Some("settings")
            } else if !kb.keyboard_shortcuts.is_empty()
                && key.eq_ignore_ascii_case(&kb.keyboard_shortcuts)
            {
                Some("keyboard_shortcuts")
            } else if !kb.help.is_empty() && key.eq_ignore_ascii_case(&kb.help) {
                Some("help")
            } else if !kb.split_right.is_empty() && key.eq_ignore_ascii_case(&kb.split_right) {
                Some("split_right")
            } else if !kb.terminal_right.is_empty() && key.eq_ignore_ascii_case(&kb.terminal_right)
            {
                Some("terminal_right")
            } else if !kb.split_down.is_empty() && key.eq_ignore_ascii_case(&kb.split_down) {
                Some("split_down")
            } else if !kb.report_bug.is_empty() && key.eq_ignore_ascii_case(&kb.report_bug) {
                Some("report_bug")
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(cmd) = matched_cmd {
        if cmd == "commands" {
            open_palette(app, PaletteMode::Commands);
        } else {
            execute_command(app, cmd);
        }
        return;
    }

    if app.extension_shop.is_open {
        if key == "<Esc>" && !has_active_submodal(&app.extension_shop) {
            app.extension_shop.is_open = false;
        } else {
            app.extension_shop.handle_key(&ev);
        }
        process_widget_side_effects(app);
        app.invalidations.mark_chrome();
        return;
    }

    let nvim_key = if key == "<" { "<lt>" } else { key };

    // Region-specific routing
    match app.workspace.focus {
        FocusRegion::Sidebar => {
            if app.workspace.overview {
                let (rows, total_items) = workspace::build_sidebar_rows(app);
                match key {
                    "<Esc>" => {
                        app.workspace.focus = FocusRegion::Editor;
                    }
                    "j" | "<Down>" => {
                        if app.workspace.sidebar_index + 1 < total_items {
                            app.workspace.sidebar_index += 1;
                        }
                    }
                    "k" | "<Up>" => {
                        app.workspace.sidebar_index =
                            app.workspace.sidebar_index.saturating_sub(1);
                    }
                    "<Home>" => {
                        app.workspace.sidebar_index = 0;
                    }
                    "<End>" => {
                        if total_items > 0 {
                            app.workspace.sidebar_index = total_items - 1;
                        }
                    }
                    "<CR>" | " " => {
                        if let Some(SidebarRow::Item { action, .. }) = rows.into_iter().find(|r| {
                            matches!(r, SidebarRow::Item { item_idx, .. } if *item_idx == app.workspace.sidebar_index)
                        }) {
                            match action {
                                SidebarAction::Buffer(bufnr) => {
                                    app.extension_shop.is_open = false;
                                    app.workspace.focus = FocusRegion::Editor;
                                    let lua = format!(
                                        "if _G.tuim_select_buffer then _G.tuim_select_buffer({}) else vim.api.nvim_set_current_buf({}) end",
                                        bufnr, bufnr
                                    );
                                    let _ = app.editor_rpc.notify(
                                        "nvim_exec_lua",
                                        vec![Value::from(lua), Value::Array(vec![])],
                                    );
                                }
                                SidebarAction::Tool(tool_id) => {
                                    execute_command(app, tool_id);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                app.invalidations.mark_chrome();
                return;
            }

            // Active sidebar widget
            match app.activity_bar.active_tab {
                0 => {
                    if key == "<Esc>" && app.explorer.input_mode == InputMode::None && !app.explorer.context_menu_open {
                        app.workspace.overview = true;
                    } else {
                        app.explorer.handle_key(&ev);
                    }
                }
                1 => {
                    if key == "<Esc>" && !app.git_panel.is_typing_commit {
                        app.workspace.overview = true;
                    } else {
                        app.git_panel.handle_key(&ev);
                    }
                }
                2 => {
                    if key == "<Esc>" {
                        app.workspace.overview = true;
                    } else {
                        app.ai_panel.handle_key(&ev, &mut app.editor_rpc);
                    }
                }
                4 => {
                    if key == "<Esc>" && app.search_panel.focus_field == 0 {
                        app.workspace.overview = true;
                    } else {
                        app.search_panel.handle_key(&ev);
                    }
                }
                _ => {
                    if key == "<Esc>" {
                        app.workspace.overview = true;
                    }
                }
            }
            process_widget_side_effects(app);
            app.invalidations.mark_chrome();
        }
        FocusRegion::VerticalTerminal => {
            app.vertical_terminal.handle_key(key);
            process_terminal_clipboard(app);
            app.invalidations.mark_terminal_grid();
        }
        FocusRegion::Drawer => {
            if app.workspace.drawer_tab == 0 {
                app.shell_terminal.handle_key(key);
                process_terminal_clipboard(app);
                app.invalidations.mark_terminal_grid();
            } else if key == "<Esc>" {
                app.workspace.focus = FocusRegion::Editor;
                app.invalidations.mark_chrome();
            } else {
                let panel = if app.workspace.drawer_tab == 1 {
                    &mut app.debug_console
                } else {
                    &mut app.output_panel
                };
                match key {
                    "j" | "<Down>" => panel.list_state.move_down(panel.lines.len(), 1),
                    "k" | "<Up>" => panel.list_state.move_up(1),
                    _ => {}
                }
                app.invalidations.mark_chrome();
            }
        }
        FocusRegion::Editor => {
            let _ = app
                .editor_rpc
                .notify("nvim_input", vec![Value::from(nvim_key)]);
        }
    }

    let _ = screen;
}

fn handle_mouse_event(
    app: &mut App,
    mx: u16,
    my: u16,
    button: u8,
    pressed: bool,
    motion: bool,
) {
    let layout = app.current_layout();
    let screen = Rect::new(0, 0, app.renderer.width, app.renderer.height);

    let terminal_mouse = !app.settings_widget.is_open && !app.mason_widget.is_open
        && !app.lazy_widget.is_open && !app.git_detailed_widget.is_open
        && !app.orchestrator.is_open && !app.bug_report.is_open
        && !app.workspace.palette_open && !app.extension_shop.is_open
        && !app.editor_context_menu.is_open && app.menu_bar.open_menu.is_none();
    // Selection drags and terminal application mouse motion must not be
    // swallowed by the workspace's passive hover handler.
    if terminal_mouse && app.workspace.drawer_tab == 0 && layout.terminal.h > 0 && !app.dragging_sidebar && !app.dragging_terminal && !app.dragging_vertical_terminal
        && ((motion && layout.terminal.contains(mx, my)) || app.shell_terminal.selecting()) {
        app.shell_terminal.handle_mouse(mx.saturating_sub(layout.terminal.x),
            my.saturating_sub(layout.terminal.y), button, pressed, motion);
        app.hover_x = Some(mx);
        app.hover_y = Some(my);
        app.invalidations.mark_chrome();
        return;
    }

    if terminal_mouse && layout.vertical_terminal.w > 0
        && !app.dragging_sidebar && !app.dragging_terminal && !app.dragging_vertical_terminal
        && ((motion && layout.vertical_terminal.contains(mx, my)) || app.vertical_terminal.selecting()) {
        app.vertical_terminal.handle_mouse(mx.saturating_sub(layout.vertical_terminal.x),
            my.saturating_sub(layout.vertical_terminal.y), button, pressed, motion);
        app.hover_x = Some(mx);
        app.hover_y = Some(my);
        app.invalidations.mark_chrome();
        return;
    }

    // SGR motion is passive only when the low button bits indicate no button held.
    // Button-held motion must reach Neovim so selections update during the drag.
    if motion && (button & 3) == 3 && !app.dragging_sidebar && !app.dragging_terminal && !app.dragging_vertical_terminal {
        app.hover_x = Some(mx);
        app.hover_y = Some(my);
        if app.menu_bar.open_menu.is_some() {
            let cmd_shortcut = workspace::format_shortcut_display(
                &app.settings_widget.config.keybindings.commands,
            );
            let right_reserved = cmd_shortcut.len() as u16 + 12;
            let menu_r = Rect::new(
                layout.tab_bar.x + layout.tab_bar.w.saturating_sub(right_reserved + 30),
                layout.tab_bar.y,
                28,
                1,
            );
            app.menu_bar.handle_mouse(menu_r, mx, my, button, false, true);
        }
        app.invalidations.mark_chrome();
        return;
    }

    // Handle active border dragging
    let is_active_drag = pressed || (motion && (button & 3) != 3);
    if app.dragging_sidebar {
        if !is_active_drag {
            app.dragging_sidebar = false;
        } else {
            let new_w = mx.clamp(16, screen.w.saturating_sub(24).max(16));
            if new_w != app.sidebar_w {
                app.sidebar_w = new_w;
                sync_layout(app);
            }
        }
        return;
    }
    if app.dragging_terminal {
        if !is_active_drag {
            app.dragging_terminal = false;
        } else {
            let bottom = screen.h.saturating_sub(layout.status_bar.h);
            let max_h = layout
                .terminal
                .h
                .saturating_add(layout.editor.h)
                .saturating_sub(2)
                .max(2);
            let new_h = bottom.saturating_sub(my + 1).clamp(2, max_h);
            if new_h != app.terminal_h {
                app.terminal_h = new_h;
                sync_layout(app);
            }
        }
        return;
    }

    if app.dragging_vertical_terminal {
        if !is_active_drag {
            app.dragging_vertical_terminal = false;
        } else {
            app.vertical_terminal_w = screen.w.saturating_sub(mx + 1).max(6);
            sync_layout(app);
        }
        return;
    }

    // Modal mouse routing
    if app.settings_widget.is_open {
        let was_open = app.settings_widget.is_open;
        app.settings_widget
            .handle_mouse(screen, mx, my, button, pressed, &mut app.editor_rpc);
        if was_open && !app.settings_widget.is_open {
            app.workspace.focus = app.workspace.prior_focus;
        }
        app.invalidations.mark_chrome();
        return;
    }
    if app.mason_widget.is_open {
        let was_open = app.mason_widget.is_open;
        app.mason_widget
            .handle_mouse(screen, mx, my, button, pressed, &mut app.editor_rpc);
        if was_open && !app.mason_widget.is_open {
            app.workspace.focus = app.workspace.prior_focus;
        }
        app.invalidations.mark_chrome();
        return;
    }
    if app.lazy_widget.is_open {
        let was_open = app.lazy_widget.is_open;
        app.lazy_widget
            .handle_mouse(screen, mx, my, button, pressed, &mut app.editor_rpc);
        if was_open && !app.lazy_widget.is_open {
            app.workspace.focus = app.workspace.prior_focus;
        }
        app.invalidations.mark_chrome();
        return;
    }
    if app.git_detailed_widget.is_open {
        app.git_detailed_widget
            .handle_mouse(screen, mx, my, button, pressed);
        app.invalidations.mark_chrome();
        return;
    }
    if app.orchestrator.is_open {
        app.orchestrator
            .handle_mouse(screen, mx, my, button, pressed);
        app.invalidations.mark_chrome();
        return;
    }
    if app.editor_context_menu.is_open {
        app.editor_context_menu
            .handle_mouse(mx, my, button, pressed, &mut app.editor_rpc);
        app.invalidations.mark_chrome();
        return;
    }
    if app.workspace.palette_open {
        if !pressed || button != 0 {
            return;
        }
        let pr = workspace::palette_rect(screen);
        if !pr.contains(mx, my) {
            app.workspace.palette_open = false;
            app.invalidations.mark_chrome();
            return;
        }
        let rel_y = my - pr.y;
        let rel_x = mx - pr.x;
        let list_h = pr.h.saturating_sub(4) as usize;
        if rel_y >= 2 && (rel_y as usize) < 2 + list_h {
            let idx = app.workspace.palette_scroll + (rel_y - 2) as usize;
            match app.workspace.palette_mode {
                PaletteMode::Commands => {
                    let cmds = workspace::filtered_commands(app);
                    if idx < cmds.len() {
                        app.workspace.palette_index = idx;
                        if rel_x >= pr.w.saturating_sub(18) && !cmds[idx].key_field.is_empty() {
                            app.workspace.palette_recording_key = true;
                            app.workspace.palette_status.clear();
                        } else {
                            let id = cmds[idx].id;
                            app.workspace.palette_open = false;
                            execute_command(app, id);
                        }
                    }
                }
                PaletteMode::Buffers => {
                    let bufs = workspace::filtered_buffers(app);
                    if let Some(&(_, _, bufnr)) = bufs.get(idx) {
                        app.workspace.palette_open = false;
                        app.workspace.focus = FocusRegion::Editor;
                        let lua = format!(
                            "if _G.tuim_select_buffer then _G.tuim_select_buffer({}) else vim.api.nvim_set_current_buf({}) end",
                            bufnr, bufnr
                        );
                        let _ = app
                            .editor_rpc
                            .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
                    }
                }
                PaletteMode::LineJump => {}
            }
        }
        app.invalidations.mark_chrome();
        return;
    }

    if app.extension_shop.is_open {
        app.extension_shop.handle_mouse(screen, mx, my, button, pressed);
        process_widget_side_effects(app);
        app.invalidations.mark_chrome();
        return;
    }

    // Menu bar dropdown or header click
    {
        let cmd_shortcut =
            workspace::format_shortcut_display(&app.settings_widget.config.keybindings.commands);
        let right_reserved = cmd_shortcut.len() as u16 + 12;
        let menu_r = Rect::new(
            layout.tab_bar.x + layout.tab_bar.w.saturating_sub(right_reserved + 30),
            layout.tab_bar.y,
            28,
            1,
        );
        if (app.menu_bar.open_menu.is_some() || (my == layout.tab_bar.y && menu_r.contains(mx, my)))
            && app.menu_bar.handle_mouse(menu_r, mx, my, button, pressed, motion)
        {
            if let Some(action) = app.menu_bar.triggered_action.take() {
                execute_command(app, action);
            }
            app.invalidations.mark_chrome();
            return;
        }
    }

    // Top tab bar clicks
    if layout.tab_bar.contains(mx, my) && pressed && button == 0 {
        let cmd_shortcut =
            workspace::format_shortcut_display(&app.settings_widget.config.keybindings.commands);
        let right_reserved = cmd_shortcut.len() as u16 + 12;
        if mx >= layout.tab_bar.x + layout.tab_bar.w.saturating_sub(right_reserved) {
            open_palette(app, PaletteMode::Commands);
            return;
        }
        // Check multi-buffer tab clicks
        let mut x = layout.tab_bar.x + 1;
        for (idx, tab) in app.tabs.clone().iter().enumerate() {
            let base = if !tab.relative_name.is_empty() {
                tab.relative_name.as_str()
            } else if !tab.name.is_empty() {
                std::path::Path::new(&tab.name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&tab.name)
            } else {
                "[No Name]"
            };
            let modified = if tab.changed { " ●" } else { "" };
            let label = format!(" {}{} × ", base, modified);
            let w = label.width() as u16;
            if mx >= x && mx < x + w {
                if mx >= x + w.saturating_sub(3) {
                    app.workspace.focus = FocusRegion::Editor;
                    let lua = format!(
                        "if _G.tuim_close_buffer then _G.tuim_close_buffer({}) end",
                        tab.bufnr
                    );
                    let _ = app.editor_rpc.request_async(
                        "nvim_exec_lua",
                        vec![Value::from(lua), Value::Array(vec![])],
                        AsyncTag::QuitConfirmation,
                    );
                } else {
                    app.active_tab = idx;
                    app.workspace.focus = FocusRegion::Editor;
                    let lua = format!(
                        "if _G.tuim_select_buffer then _G.tuim_select_buffer({}) end",
                        tab.bufnr
                    );
                    let _ = app
                        .editor_rpc
                        .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
                }
                app.invalidations.mark_chrome();
                return;
            }
            x += w + 1;
        }
        return;
    }

    // Bottom status bar clicks
    if layout.status_bar.contains(mx, my) && pressed && button == 0 {
        if app.native_picker_chrome && app.telescope_main_rect.is_some() {
            let rel_x = mx - layout.status_bar.x;
            let action = workspace::picker_footer_hints(layout.status_bar.w)
                .into_iter()
                .find(|(x, text, _)| (*x..*x + text.len() as u16).contains(&rel_x))
                .and_then(|(_, _, action)| action);
            if let Some(act) = action {
                let lua = format!(
                    "if _G.tuim_picker_action then _G.tuim_picker_action('{}') end",
                    act
                );
                let _ = app
                    .editor_rpc
                    .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
            }
            return;
        }
        if mx >= layout.status_bar.w.saturating_sub(16) {
            toggle_zen_mode(app);
        } else if mx >= layout.status_bar.w.saturating_sub(36) {
            open_palette(app, PaletteMode::Commands);
        }
        return;
    }

    // Divider drag check
    if app.mode != Mode::Zen && layout.sidebar.w > 0 && pressed && button == 0 {
        let div_x = layout.sidebar.x + layout.sidebar.w;
        if mx == div_x {
            app.dragging_sidebar = true;
            return;
        }
    }

    // Activity bar / Sidebar top header (rows 0..2)
    if app.mode != Mode::Zen && layout.activity_bar.contains(mx, my) {
        if pressed && button == 0 && my == layout.activity_bar.y + 1 {
            app.workspace.overview = !app.workspace.overview;
            app.workspace.focus = FocusRegion::Sidebar;
            app.extension_shop.is_open = false;
            app.invalidations.mark_chrome();
        }
        return;
    }

    // Sidebar body clicks
    if app.mode != Mode::Zen && layout.sidebar.contains(mx, my) {
        if app.workspace.overview {
            if !pressed {
                return;
            }
            app.workspace.focus = FocusRegion::Sidebar;
            if button == 64 {
                app.workspace.sidebar_index = app.workspace.sidebar_index.saturating_sub(1);
                app.invalidations.mark_chrome();
                return;
            }
            if button == 65 {
                let (_, total) = workspace::build_sidebar_rows(app);
                if app.workspace.sidebar_index + 1 < total {
                    app.workspace.sidebar_index += 1;
                }
                app.invalidations.mark_chrome();
                return;
            }
            if button == 0 {
                let (rows, _) = workspace::build_sidebar_rows(app);
                let r_idx = app.workspace.sidebar_scroll + (my - layout.sidebar.y) as usize;
                if let Some(SidebarRow::Item {
                    item_idx, action, ..
                }) = rows.get(r_idx).cloned()
                {
                    app.workspace.sidebar_index = item_idx;
                    match action {
                        SidebarAction::Buffer(bufnr) => {
                            app.extension_shop.is_open = false;
                            app.workspace.focus = FocusRegion::Editor;
                            let lua = format!(
                                "if _G.tuim_select_buffer then _G.tuim_select_buffer({}) else vim.api.nvim_set_current_buf({}) end",
                                bufnr, bufnr
                            );
                            let _ = app.editor_rpc.notify(
                                "nvim_exec_lua",
                                vec![Value::from(lua), Value::Array(vec![])],
                            );
                        }
                        SidebarAction::Tool(tool_id) => {
                            execute_command(app, tool_id);
                        }
                    }
                }
                app.invalidations.mark_chrome();
            }
            return;
        }

        if pressed {
            app.workspace.focus = FocusRegion::Sidebar;
        }
        match app.activity_bar.active_tab {
            0 => {
                app.explorer
                    .handle_mouse(layout.sidebar, mx, my, button, pressed);
            }
            1 => {
                app.git_panel
                    .handle_mouse(layout.sidebar, mx, my, button, pressed);
            }
            2 => {
                app.ai_panel.handle_mouse(
                    layout.sidebar,
                    mx,
                    my,
                    button,
                    pressed,
                    &mut app.editor_rpc,
                );
            }
            4 => {
                app.search_panel
                    .handle_mouse(layout.sidebar, mx, my, button, pressed);
            }
            _ => {}
        }
        app.invalidations.mark_chrome();
        return;
    }

    if layout.vertical_terminal.w > 0 {
        let rect = layout.vertical_terminal;
        if mx == rect.x - 1 && my >= rect.y - 1 && my < rect.y + rect.h && pressed && button == 0 {
            app.dragging_vertical_terminal = true;
            return;
        }
        if my == rect.y - 1 && mx >= rect.x && mx < rect.x + rect.w {
            if pressed && button == 0 {
                if mx >= rect.x + rect.w.saturating_sub(3) {
                    execute_command(app, "toggle_vertical_terminal");
                } else {
                    app.workspace.focus = FocusRegion::VerticalTerminal;
                    app.invalidations.mark_chrome();
                }
            }
            return;
        }
        if rect.contains(mx, my) {
            if pressed { app.workspace.focus = FocusRegion::VerticalTerminal; }
            app.vertical_terminal.handle_mouse(mx - rect.x, my - rect.y, button, pressed, motion);
            app.invalidations.mark_chrome();
            return;
        }
    }

    // Drawer header row & Drawer body
    if layout.terminal.h > 0 {
        let header_y = layout.terminal.y.saturating_sub(1);
        if my == header_y && mx >= layout.terminal.x && mx < layout.terminal.x + layout.terminal.w {
            if pressed && button == 0 {
                let rel_x = mx - layout.terminal.x;
                let mut switched_tab = false;
                if layout.terminal.w >= 46 {
                    if (2..12).contains(&rel_x) {
                        app.workspace.drawer_tab = 0;
                        app.workspace.focus = FocusRegion::Drawer;
                        switched_tab = true;
                    } else if (14..29).contains(&rel_x) {
                        app.workspace.drawer_tab = 1;
                        app.workspace.focus = FocusRegion::Drawer;
                        switched_tab = true;
                    } else if (31..40).contains(&rel_x) {
                        app.workspace.drawer_tab = 2;
                        app.workspace.focus = FocusRegion::Drawer;
                        switched_tab = true;
                    }
                } else if (1..7).contains(&rel_x) {
                    app.workspace.drawer_tab = 0;
                    app.workspace.focus = FocusRegion::Drawer;
                    switched_tab = true;
                } else if (8..15).contains(&rel_x) {
                    app.workspace.drawer_tab = 1;
                    app.workspace.focus = FocusRegion::Drawer;
                    switched_tab = true;
                } else if (16..22).contains(&rel_x) {
                    app.workspace.drawer_tab = 2;
                    app.workspace.focus = FocusRegion::Drawer;
                    switched_tab = true;
                }
                app.dragging_terminal = true;
                if switched_tab && app.workspace.drawer_tab == 0 {
                    ensure_shell_terminal(app);
                }
                app.invalidations.mark_chrome();
            }
            return;
        }

        if layout.terminal.contains(mx, my) {
            if pressed {
                app.workspace.focus = FocusRegion::Drawer;
            }
            if app.workspace.drawer_tab == 0 {
                app.shell_terminal.handle_mouse(mx - layout.terminal.x,
                    my - layout.terminal.y, button, pressed, motion);
            }
            app.invalidations.mark_chrome();
            return;
        }
    }

    // Editor area
    if layout.editor.contains(mx, my) {

        if pressed {
            app.workspace.focus = FocusRegion::Editor;
        }

        // Right-click opens native editor context menu
        if button == 2 && pressed && !motion {
            app.editor_context_menu.open(mx, my);
            app.invalidations.mark_chrome();
            return;
        }

        // Check split window close button click
        if pressed && button == 0 && app.nvim_wins.len() > 1 && app.mode != Mode::Zen {
            for win in &app.nvim_wins {
                let wx = layout.editor.x + win.col;
                let wy = layout.editor.y + win.row;
                let close_x = wx + win.width.saturating_sub(3);
                if my == wy && mx >= close_x && mx <= close_x + 1 {
                    let lua = format!(
                        "if _G.tuim_close_split then _G.tuim_close_split({}, {}) end",
                        win.id, win.bufnr
                    );
                    let _ = app
                        .editor_rpc
                        .notify("nvim_exec_lua", vec![Value::from(lua), Value::Array(vec![])]);
                    return;
                }
            }
        }

        forward_mouse_to_nvim(
            &mut app.editor_rpc,
            mx - layout.editor.x,
            my - layout.editor.y,
            button,
            pressed,
            motion,
        );
        app.invalidations.mark_chrome();
    }
}

fn forward_mouse_to_nvim(
    rpc: &mut crate::nvim::rpc::RpcClient,
    col: u16,
    row: u16,
    button: u8,
    pressed: bool,
    motion: bool,
) {
    let base_btn = button & 0b0000_0011;
    let is_wheel = (button & 64) != 0;

    let (nvim_btn, action) = if is_wheel {
        let dir = match base_btn {
            0 => "up",
            1 => "down",
            2 => "left",
            _ => "right",
        };
        ("wheel", dir)
    } else {
        let b_name = match base_btn {
            0 => "left",
            1 => "middle",
            2 => "right",
            _ => return,
        };
        let act = if motion {
            "drag"
        } else if pressed {
            "press"
        } else {
            "release"
        };
        (b_name, act)
    };

    let mut mods = String::new();
    if (button & 4) != 0 {
        mods.push('s');
    }
    if (button & 8) != 0 {
        mods.push('a');
    }
    if (button & 16) != 0 {
        mods.push('c');
    }

    let _ = rpc.notify(
        "nvim_input_mouse",
        vec![
            Value::from(nvim_btn),
            Value::from(action),
            Value::from(mods),
            Value::from(0),
            Value::from(row as i64),
            Value::from(col as i64),
        ],
    );
}
