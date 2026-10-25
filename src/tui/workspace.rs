use unicode_width::UnicodeWidthStr;

use crate::tui::app::{App, FocusRegion, Mode, PaletteMode};
use crate::tui::layout::{Layout, Rect};
use crate::tui::theme::chrome;
use crate::tui::widgets::primitives;

#[derive(Debug, Clone)]
pub struct CommandDef {
    pub id: &'static str,
    pub label: &'static str,
    pub key_field: &'static str,
}

pub const COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: "find_file",
        label: "Find file",
        key_field: "find_file",
    },
    CommandDef {
        id: "search_project",
        label: "Search project",
        key_field: "search_project",
    },
    CommandDef {
        id: "new_file",
        label: "New file",
        key_field: "new_file",
    },
    CommandDef {
        id: "save_file",
        label: "Save file",
        key_field: "save_file",
    },
    CommandDef {
        id: "close_buffer",
        label: "Close buffer",
        key_field: "close_buffer",
    },
    CommandDef {
        id: "switch_buffers",
        label: "Switch buffers",
        key_field: "switch_buffers",
    },
    CommandDef {
        id: "project_files",
        label: "Explorer / Project files",
        key_field: "project_files",
    },
    CommandDef {
        id: "changes",
        label: "Git / Changes",
        key_field: "changes",
    },
    CommandDef {
        id: "problems",
        label: "Problems / Diagnostics",
        key_field: "problems",
    },
    CommandDef {
        id: "terminal",
        label: "Toggle terminal",
        key_field: "toggle_terminal",
    },
    CommandDef {
        id: "toggle_explorer",
        label: "Toggle explorer sidebar",
        key_field: "toggle_explorer",
    },
    CommandDef {
        id: "focus_next",
        label: "Focus next workspace region",
        key_field: "focus_next",
    },
    CommandDef { id: "focus_prev", label: "Focus previous workspace region", key_field: "focus_prev" },
    CommandDef { id: "resize_left", label: "Resize panel left", key_field: "resize_left" },
    CommandDef { id: "resize_right", label: "Resize panel right", key_field: "resize_right" },
    CommandDef { id: "resize_up", label: "Resize panel up", key_field: "resize_up" },
    CommandDef { id: "resize_down", label: "Resize panel down", key_field: "resize_down" },
    CommandDef {
        id: "zen",
        label: "Toggle Zen mode",
        key_field: "toggle_zen",
    },
    CommandDef {
        id: "ai_assistants",
        label: "AI assistants",
        key_field: "ai_assistants",
    },
    CommandDef {
        id: "orchestrator",
        label: "Worktree & Agent Orchestrator",
        key_field: "",
    },
    CommandDef {
        id: "extensions",
        label: "Extensions / Plugins",
        key_field: "extensions",
    },
    CommandDef {
        id: "language_tools",
        label: "Language tools (Mason)",
        key_field: "language_tools",
    },
    CommandDef {
        id: "settings",
        label: "Settings",
        key_field: "settings",
    },
    CommandDef {
        id: "keyboard_shortcuts",
        label: "Keyboard shortcuts",
        key_field: "keyboard_shortcuts",
    },
    CommandDef {
        id: "help",
        label: "Help & Quickstart",
        key_field: "help",
    },
    CommandDef {
        id: "split_right",
        label: "Split editor right",
        key_field: "split_right",
    },
    CommandDef {
        id: "terminal_right",
        label: "Open terminal split right",
        key_field: "terminal_right",
    },
    CommandDef {
        id: "split_down",
        label: "Split editor down",
        key_field: "split_down",
    },
    CommandDef {
        id: "report_bug",
        label: "Report bug",
        key_field: "report_bug",
    },
    CommandDef {
        id: "quit",
        label: "Quit Tuim",
        key_field: "quit",
    },
];

#[derive(Debug, Clone)]
pub enum SidebarAction {
    Buffer(i64),
    Tool(&'static str),
}

#[derive(Debug, Clone)]
pub enum SidebarRow {
    Header(&'static str),
    Spacer,
    Item {
        item_idx: usize,
        label: String,
        action: SidebarAction,
    },
}

pub fn build_sidebar_rows(app: &App) -> (Vec<SidebarRow>, usize) {
    let mut rows = Vec::new();
    let mut item_idx = 0usize;

    rows.push(SidebarRow::Header("OPEN FILES"));
    if app.tabs.is_empty() {
        rows.push(SidebarRow::Item {
            item_idx,
            label: "[No Name]".to_string(),
            action: SidebarAction::Buffer(1),
        });
        item_idx += 1;
    } else {
        for tab in &app.tabs {
            let base = if !tab.relative_name.is_empty() {
                tab.relative_name.clone()
            } else if !tab.name.is_empty() {
                std::path::Path::new(&tab.name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&tab.name)
                    .to_string()
            } else {
                "[No Name]".to_string()
            };
            let label = if tab.changed {
                format!("{} ●", base)
            } else {
                base
            };
            rows.push(SidebarRow::Item {
                item_idx,
                label,
                action: SidebarAction::Buffer(tab.bufnr),
            });
            item_idx += 1;
        }
    }

    rows.push(SidebarRow::Spacer);
    rows.push(SidebarRow::Header("PROJECT"));
    for (label, id) in [
        ("Explorer", "explorer"),
        ("Changes", "changes"),
        ("Problems", "problems"),
    ] {
        rows.push(SidebarRow::Item {
            item_idx,
            label: label.to_string(),
            action: SidebarAction::Tool(id),
        });
        item_idx += 1;
    }

    rows.push(SidebarRow::Spacer);
    rows.push(SidebarRow::Header("TOOLS"));
    for (label, id) in [
        ("AI assistants", "ai_assistants"),
        ("Extensions", "extensions"),
        ("Language tools", "language_tools"),
        ("Settings", "settings"),
    ] {
        rows.push(SidebarRow::Item {
            item_idx,
            label: label.to_string(),
            action: SidebarAction::Tool(id),
        });
        item_idx += 1;
    }

    (rows, item_idx)
}

pub fn filtered_commands(app: &App) -> Vec<&'static CommandDef> {
    let raw = app.workspace.palette_query.trim_start_matches('>').trim();
    let q = raw.to_lowercase();
    COMMANDS
        .iter()
        .filter(|cmd| {
            q.is_empty()
                || cmd.label.to_lowercase().contains(&q)
                || cmd.id.to_lowercase().contains(&q)
        })
        .collect()
}

pub fn filtered_buffers(app: &App) -> Vec<(usize, String, i64)> {
    let raw = app.workspace.palette_query.trim_start_matches('#').trim();
    let q = raw.to_lowercase();
    let mut out = Vec::new();
    for (i, tab) in app.tabs.iter().enumerate() {
        let name = if !tab.relative_name.is_empty() {
            tab.relative_name.clone()
        } else if !tab.name.is_empty() {
            tab.name.clone()
        } else {
            "[No Name]".to_string()
        };
        if q.is_empty() || name.to_lowercase().contains(&q) {
            out.push((i, name, tab.bufnr));
        }
    }
    out
}

pub fn format_shortcut_display(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    if raw.starts_with('<') && raw.ends_with('>') {
        let inner = &raw[1..raw.len() - 1];
        let mut out = Vec::new();
        let mut rest = inner;
        while let Some(stripped) = rest
            .strip_prefix("C-")
            .or_else(|| rest.strip_prefix("c-"))
        {
            out.push("Ctrl".to_string());
            rest = stripped;
        }
        while let Some(stripped) = rest
            .strip_prefix("M-")
            .or_else(|| rest.strip_prefix("m-"))
        {
            out.push("Alt".to_string());
            rest = stripped;
        }
        while let Some(stripped) = rest
            .strip_prefix("S-")
            .or_else(|| rest.strip_prefix("s-"))
        {
            out.push("Shift".to_string());
            rest = stripped;
        }
        if rest.len() == 1 {
            out.push(rest.to_uppercase());
        } else {
            out.push(rest.to_string());
        }
        return out.join("+");
    }
    raw.to_string()
}

/// Hover follows the sidebar's click targets, including their empty padding.
/// It never changes focus or the keyboard selection.
pub fn sidebar_hover_rect(app: &App, layout: &Layout, x: u16, y: u16) -> Option<Rect> {
    let ab = layout.activity_bar;
    let sb = layout.sidebar;
    if ab.contains(x, y) && y == ab.y + 1 {
        return Some(Rect::new(ab.x, y, ab.w, 1));
    }
    if !sb.contains(x, y) { return None; }
    let row = Rect::new(sb.x, y, sb.w, 1);
    if app.workspace.overview {
        let (rows, _) = build_sidebar_rows(app);
        return matches!(rows.get(app.workspace.sidebar_scroll + (y - sb.y) as usize),
            Some(SidebarRow::Item { .. })).then_some(row);
    }
    let rel_y = y - sb.y;
    let rel_x = x - sb.x;
    match app.activity_bar.active_tab {
        1 => {
            if rel_y == 0 && rel_x >= sb.w.saturating_sub(10) {
                return Some(Rect::new(sb.x + sb.w.saturating_sub(10), y, sb.w.min(10), 1));
            }
            if rel_y == 1 { return Some(row); }
            if rel_y == 2 {
                let (left, right) = if rel_x < 10 { (0, 10) }
                    else if rel_x < 22 { (10, 22) } else { (22, sb.w) };
                return Some(Rect::new(sb.x + left, y, right.min(sb.w).saturating_sub(left), 1));
            }
            let staged_start = 5;
            let unstaged_start = 5 + if app.git_panel.staged.is_empty() { 0 }
                else { app.git_panel.staged.len() + 1 };
            let index = rel_y as usize;
            if (staged_start..staged_start + app.git_panel.staged.len()).contains(&index)
                || (unstaged_start..unstaged_start + app.git_panel.unstaged.len()).contains(&index) {
                let action_x = sb.x + sb.w.saturating_sub(3);
                return Some(if x >= action_x { Rect::new(action_x, y, sb.w.min(3), 1) }
                    else { Rect::new(sb.x, y, sb.w.saturating_sub(3), 1) });
            }
            None
        }
        2 => {
            let ai_layout = app.ai_panel.layout(sb);
            for (_, _, tab_r) in ai_layout.tab_rects {
                let hit_r = Rect::new(
                    tab_r.x.saturating_sub(1),
                    tab_r.y,
                    tab_r.w.saturating_add(1).min((sb.x + sb.w).saturating_sub(tab_r.x.saturating_sub(1))),
                    1,
                );
                if hit_r.contains(x, y) {
                    return Some(tab_r);
                }
            }
            if y == ai_layout.open_chat_row {
                return Some(row);
            }
            for &(_, _, act_r) in &ai_layout.action_rects {
                if act_r.contains(x, y) {
                    return Some(act_r);
                }
            }
            if y >= ai_layout.list_start_y {
                let rel_row = (y - ai_layout.list_start_y) as usize;
                if rel_row < ai_layout.list_max_rows
                    && app.ai_panel.list_state.scroll_offset + rel_row
                        < app.ai_panel.current_section_len()
                {
                    return Some(row);
                }
            }
            None
        }
        4 => (rel_y == 1 || (rel_y >= 3 && app.search_panel.list_state.scroll_offset
            + ((rel_y - 3) as usize) < app.search_panel.hits.len())).then_some(row),
        _ => {
            if rel_y == 0 && sb.w >= 16 && rel_x >= sb.w - 9 {
                let left = sb.w - 9 + ((rel_x - (sb.w - 9)) / 3) * 3;
                return Some(Rect::new(sb.x + left, y, 3, 1));
            }
            let editing = app.explorer.input_mode != crate::tui::widgets::explorer::InputMode::None;
            let list_y = sb.y + 1 + u16::from(editing);
            if editing && y == sb.y + 1 { return Some(row); }
            (y >= list_y && app.explorer.list_state.scroll_offset + ((y - list_y) as usize)
                < app.explorer.entries.len()).then_some(row)
        }
    }
}

pub fn draw_sidebar(app: &mut App, layout: &Layout) {
    let ab = layout.activity_bar;
    let sb = layout.sidebar;
    if ab.w == 0 || sb.w == 0 {
        return;
    }
    let c = chrome(&app.theme);
    app.renderer.fill_rect(ab, c.panel_bg);
    app.renderer.fill_rect(sb, c.panel_bg);

    // Row 0: "tuim / {branch}"
    let branch = if app.git_panel.snapshot.branch.is_empty() {
        "workspace"
    } else {
        &app.git_panel.snapshot.branch
    };
    let title = format!("tuim / {}", branch);
    let title_trunc = primitives::truncate_to_width(&title, ab.w.saturating_sub(2) as usize);
    app.renderer.draw_text_styled(
        ab.x + 1,
        ab.y,
        &title_trunc,
        c.text_strong,
        c.panel_bg,
        true,
        false,
    );

    // Row 1: "WORKSPACE" when in overview, else "< Workspace [Esc]"
    if ab.h >= 2 {
        let row1_label = if app.workspace.overview {
            "WORKSPACE"
        } else {
            "< Workspace [Esc]"
        };
        let trunc = primitives::truncate_to_width(row1_label, ab.w.saturating_sub(2) as usize);
        app.renderer.draw_text_styled(
            ab.x + 1,
            ab.y + 1,
            &trunc,
            c.text_muted,
            c.panel_bg,
            app.workspace.overview,
            false,
        );
    }

    let focused = app.workspace.focus == FocusRegion::Sidebar;

    if !app.workspace.overview {
        match app.activity_bar.active_tab {
            0 => app.explorer.draw(&mut app.renderer, sb, &app.theme, focused),
            1 => app.git_panel.draw(&mut app.renderer, sb, &app.theme, focused),
            2 => app.ai_panel.draw(&mut app.renderer, sb, &app.theme, focused),
            4 => app
                .search_panel
                .draw(&mut app.renderer, sb, &app.theme, focused),
            _ => app.explorer.draw(&mut app.renderer, sb, &app.theme, focused),
        }
        return;
    }

    // Draw Workspace Overview in sb
    let (rows, total_items) = build_sidebar_rows(app);
    if total_items > 0 && app.workspace.sidebar_index >= total_items {
        app.workspace.sidebar_index = total_items - 1;
    }

    // Find visual row index of selected item so we can scroll `sb` when short
    let selected_visual_row = rows
        .iter()
        .position(|r| matches!(r, SidebarRow::Item { item_idx, .. } if *item_idx == app.workspace.sidebar_index))
        .unwrap_or(0);

    let view_h = sb.h as usize;
    if view_h > 0 {
        let max_scroll = rows.len().saturating_sub(view_h);
        if app.workspace.sidebar_scroll > max_scroll {
            app.workspace.sidebar_scroll = max_scroll;
        }
        let target_row = if app.workspace.sidebar_index == 0 {
            0
        } else {
            selected_visual_row
        };
        if target_row < app.workspace.sidebar_scroll {
            app.workspace.sidebar_scroll = target_row;
        } else if selected_visual_row >= app.workspace.sidebar_scroll + view_h {
            app.workspace.sidebar_scroll = selected_visual_row + 1 - view_h;
        }
    }

    for i in 0..view_h {
        let r_idx = app.workspace.sidebar_scroll + i;
        if r_idx >= rows.len() {
            break;
        }
        let ry = sb.y + i as u16;
        match &rows[r_idx] {
            SidebarRow::Spacer => {}
            SidebarRow::Header(hdr) => {
                let trunc = primitives::truncate_to_width(hdr, sb.w.saturating_sub(2) as usize);
                app.renderer.draw_text_styled(
                    sb.x + 1,
                    ry,
                    &trunc,
                    c.text_muted,
                    c.panel_bg,
                    true,
                    false,
                );
            }
            SidebarRow::Item {
                item_idx, label, ..
            } => {
                let is_sel = *item_idx == app.workspace.sidebar_index;
                let bg = if is_sel && focused {
                    c.row_selected_bg
                } else {
                    c.panel_bg
                };
                app.renderer.fill_rect(Rect::new(sb.x, ry, sb.w, 1), bg);
                let text = if is_sel && focused {
                    format!("[ {} ]", label)
                } else {
                    format!("  {}", label)
                };
                let trunc = primitives::truncate_to_width(&text, sb.w.saturating_sub(1) as usize);
                let fg = if is_sel && focused {
                    c.text_strong
                } else {
                    c.text
                };
                app.renderer
                    .draw_text_styled(sb.x, ry, &trunc, fg, bg, is_sel && focused, false);
            }
        }
    }
}

pub fn picker_footer_hints(width: u16) -> Vec<(u16, &'static str, Option<&'static str>)> {
    let hints = if width >= 65 {
        vec![
            (1, "Enter Open", Some("open")),
            (14, "Up/Down Move", None),
            (29, "Tab Mark", Some("mark")),
            (40, "Alt+P Preview", Some("preview")),
            (56, "Esc Close", Some("close")),
        ]
    } else if width >= 23 {
        vec![
            (1, "Enter Open", Some("open")),
            (14, "Esc Close", Some("close")),
        ]
    } else {
        vec![(1, "Esc Close", Some("close"))]
    };
    hints
        .into_iter()
        .filter(|(x, text, _)| x + text.len() as u16 <= width)
        .collect()
}

pub fn draw_chrome(app: &mut App, layout: &Layout) {
    let c = chrome(&app.theme);

    // 1. Top Bar (Tab strip + Fresh Menu Bar + Commands affordance)
    let tb = layout.tab_bar;
    if tb.w > 0 && tb.h > 0 {
        app.renderer.fill_rect(tb, c.surface_bg);

        let mut x = tb.x + 1;
        // Draw multi-buffer tabs (active + open buffers)
        if app.tabs.is_empty() {
            let name = app.active_tab_display_name();
            app.renderer.draw_text_styled(
                x,
                tb.y,
                &name,
                c.text_strong,
                c.panel_bg,
                true,
                false,
            );
            x += name.width() as u16 + 2;
        } else {
            for (idx, tab) in app.tabs.iter().enumerate() {
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
                let is_active = idx == app.active_tab;
                let modified = if tab.changed { " ●" } else { "" };
                let label = format!(" {}{} × ", base, modified);
                let w = label.width() as u16;
                if x + w >= tb.x + tb.w.saturating_sub(18) && idx > 0 {
                    break;
                }
                let bg = if is_active { c.panel_bg } else { c.surface_bg };
                let fg = if is_active {
                    c.text_strong
                } else {
                    c.text_muted
                };
                app.renderer
                    .draw_text_styled(x, tb.y, &label, fg, bg, is_active, tab.is_preview);
                x += w + 1;
            }
        }

        // Draw Fresh Menu Bar in remaining space if wide enough
        let cmd_shortcut =
            format_shortcut_display(&app.settings_widget.config.keybindings.commands);
        let cmd_text = format!(
            "{} Commands",
            if cmd_shortcut.is_empty() {
                "F1"
            } else {
                &cmd_shortcut
            }
        );
        let right_reserved = cmd_text.width() as u16 + 3;

        if x + 32 < tb.x + tb.w.saturating_sub(right_reserved) {
            let menu_r = Rect::new(
                tb.x + tb.w.saturating_sub(right_reserved + 30),
                tb.y,
                28,
                1,
            );
            app.menu_bar.draw_bar(&mut app.renderer, menu_r, &app.theme);
        }

        if tb.w > right_reserved {
            let cx = tb.x + tb.w.saturating_sub(right_reserved);
            app.renderer
                .draw_text(cx, tb.y, &cmd_text, c.text_muted, c.surface_bg);
        }
    }

    // 2. Bottom Status Bar
    let sb = layout.status_bar;
    if sb.w > 0 && sb.h > 0 {
        app.renderer.fill_rect(sb, c.surface_bg);

        if app.native_picker_chrome && app.telescope_main_rect.is_some() {
            // Native picker footer
            for (hx, text, _) in picker_footer_hints(sb.w) {
                app.renderer
                    .draw_text(sb.x + hx, sb.y, text, c.text_strong, c.surface_bg);
            }
            return;
        }

        let mode_label = match app.mode {
            Mode::Zen => "ZEN",
            Mode::Ide => "IDE",
            Mode::Normal => {
                if app.ui_state.current_mode == "insert" {
                    "INSERT"
                } else if app.ui_state.current_mode == "visual" {
                    "VISUAL"
                } else {
                    "NORMAL"
                }
            }
        };
        app.renderer.draw_text_styled(
            sb.x + 1,
            sb.y,
            mode_label,
            c.text_strong,
            c.surface_bg,
            true,
            false,
        );

        let region_label = match app.workspace.focus {
            FocusRegion::Editor => "Editor",
            FocusRegion::Sidebar => "Workspace",
            FocusRegion::Drawer => "Terminal",
            FocusRegion::VerticalTerminal => "Vertical terminal",
        };
        let file_name = app.active_tab_display_name();
        let middle = format!("{}  ·  {}", file_name, region_label);
        let mid_trunc = primitives::truncate_to_width(&middle, sb.w.saturating_sub(42) as usize);
        if sb.w > 12 {
            app.renderer
                .draw_text(sb.x + 9, sb.y, &mid_trunc, c.text, c.surface_bg);
        }

        let cmd_key = format_shortcut_display(&app.settings_widget.config.keybindings.commands);
        let cmd_str = format!(
            "{} Commands",
            if cmd_key.is_empty() { "F1" } else { &cmd_key }
        );
        if sb.w >= 38 {
            app.renderer.draw_text(
                sb.x + sb.w.saturating_sub(36),
                sb.y,
                &cmd_str,
                c.text_muted,
                c.surface_bg,
            );
        }

        let zen_key = format_shortcut_display(&app.settings_widget.config.keybindings.toggle_zen);
        let zen_str = if app.mode == Mode::Zen {
            format!(
                "{} Return",
                if zen_key.is_empty() { "F11" } else { &zen_key }
            )
        } else {
            format!(
                "{} Zen",
                if zen_key.is_empty() { "F11" } else { &zen_key }
            )
        };
        if sb.w >= 18 {
            app.renderer.draw_text(
                sb.x + sb.w.saturating_sub(16),
                sb.y,
                &zen_str,
                c.text_muted,
                c.surface_bg,
            );
        }
    }
}

pub fn palette_rect(screen: Rect) -> Rect {
    let w = 64u16.min(screen.w.saturating_sub(2)).max(24.min(screen.w));
    let h = 14u16.min(screen.h.saturating_sub(2)).max(6.min(screen.h));
    let x = screen.x + (screen.w.saturating_sub(w)) / 2;
    let y = if screen.h > 18 {
        4
    } else {
        screen.y + (screen.h.saturating_sub(h)) / 2
    };
    Rect::new(x, y, w, h)
}

pub fn draw_palette(app: &mut App, screen: Rect) {
    if !app.workspace.palette_open {
        return;
    }
    let pr = palette_rect(screen);
    let c = chrome(&app.theme);

    let title = match app.workspace.palette_mode {
        PaletteMode::Commands => "Commands / type to filter",
        PaletteMode::Buffers => "Open buffers / type to filter",
        PaletteMode::LineJump => "Go to line / type line number",
    };

    primitives::draw_modal_frame(&mut app.renderer, pr, title, &app.theme);

    // Search input on row 1
    let input_r = Rect::new(pr.x + 2, pr.y + 1, pr.w.saturating_sub(4), 1);
    let placeholder = match app.workspace.palette_mode {
        PaletteMode::Commands => "Type a command (or # for buffers, : for line)...",
        PaletteMode::Buffers => "Type buffer name...",
        PaletteMode::LineJump => ":line",
    };
    primitives::draw_text_input(
        &mut app.renderer,
        input_r,
        &app.workspace.palette_query,
        placeholder,
        true,
        &app.theme,
    );

    let list_y = pr.y + 2;
    let list_h = pr.h.saturating_sub(4) as usize;

    match app.workspace.palette_mode {
        PaletteMode::Commands => {
            let cmds = filtered_commands(app);
            if app.workspace.palette_index >= cmds.len() && !cmds.is_empty() {
                app.workspace.palette_index = cmds.len() - 1;
            }
            if list_h > 0 {
                if app.workspace.palette_index < app.workspace.palette_scroll {
                    app.workspace.palette_scroll = app.workspace.palette_index;
                } else if app.workspace.palette_index >= app.workspace.palette_scroll + list_h {
                    app.workspace.palette_scroll = app.workspace.palette_index + 1 - list_h;
                }
            }
            if cmds.is_empty() {
                if list_h > 0 {
                    app.renderer.draw_text(
                        pr.x + 2,
                        list_y,
                        "No matching commands",
                        c.text_muted,
                        c.panel_bg,
                    );
                }
            } else {
                for i in 0..list_h {
                    let idx = app.workspace.palette_scroll + i;
                    if idx >= cmds.len() {
                        break;
                    }
                    let cmd = cmds[idx];
                    let ry = list_y + i as u16;
                    let selected = idx == app.workspace.palette_index;
                    let bg = if selected {
                        c.row_selected_bg
                    } else {
                        c.panel_bg
                    };
                    app.renderer
                        .fill_rect(Rect::new(pr.x + 1, ry, pr.w.saturating_sub(2), 1), bg);

                    let label_trunc =
                        primitives::truncate_to_width(cmd.label, pr.w.saturating_sub(20) as usize);
                    app.renderer.draw_text_styled(
                        pr.x + 2,
                        ry,
                        &label_trunc,
                        c.text_strong,
                        bg,
                        selected,
                        false,
                    );

                    if !cmd.key_field.is_empty() && pr.w >= 32 {
                        let bound = app
                            .settings_widget
                            .config
                            .keybindings
                            .get_by_field(cmd.key_field);
                        let disp = if bound.is_empty() {
                            "[Set shortcut]".to_string()
                        } else {
                            format_shortcut_display(bound)
                        };
                        let sx = pr.x + pr.w.saturating_sub(16);
                        app.renderer.draw_text(sx, ry, &disp, c.accent, bg);
                    }
                }
            }
        }
        PaletteMode::Buffers => {
            let bufs = filtered_buffers(app);
            if app.workspace.palette_index >= bufs.len() && !bufs.is_empty() {
                app.workspace.palette_index = bufs.len() - 1;
            }
            if list_h > 0 {
                if app.workspace.palette_index < app.workspace.palette_scroll {
                    app.workspace.palette_scroll = app.workspace.palette_index;
                } else if app.workspace.palette_index >= app.workspace.palette_scroll + list_h {
                    app.workspace.palette_scroll = app.workspace.palette_index + 1 - list_h;
                }
            }
            if bufs.is_empty() {
                if list_h > 0 {
                    app.renderer.draw_text(
                        pr.x + 2,
                        list_y,
                        "No matching buffers",
                        c.text_muted,
                        c.panel_bg,
                    );
                }
            } else {
                for i in 0..list_h {
                    let idx = app.workspace.palette_scroll + i;
                    if idx >= bufs.len() {
                        break;
                    }
                    let (_, ref name, _) = bufs[idx];
                    let ry = list_y + i as u16;
                    let selected = idx == app.workspace.palette_index;
                    let bg = if selected {
                        c.row_selected_bg
                    } else {
                        c.panel_bg
                    };
                    app.renderer
                        .fill_rect(Rect::new(pr.x + 1, ry, pr.w.saturating_sub(2), 1), bg);
                    let trunc =
                        primitives::truncate_to_width(name, pr.w.saturating_sub(4) as usize);
                    app.renderer.draw_text_styled(
                        pr.x + 2,
                        ry,
                        &trunc,
                        c.text_strong,
                        bg,
                        selected,
                        false,
                    );
                }
            }
        }
        PaletteMode::LineJump => {
            if list_h > 0 {
                app.renderer.draw_text(
                    pr.x + 2,
                    list_y,
                    "Press Enter to jump to line",
                    c.text_muted,
                    c.panel_bg,
                );
            }
        }
    }

    // Footer status / F2 hint
    let footer_y = pr.y + pr.h.saturating_sub(2);
    let footer_text = if !app.workspace.palette_status.is_empty() {
        &app.workspace.palette_status
    } else if app.workspace.palette_recording_key {
        "Set shortcut / press a key (Esc to cancel)"
    } else {
        "Enter: Run | F2: Set shortcut | Esc: Close"
    };
    let f_trunc = primitives::truncate_to_width(footer_text, pr.w.saturating_sub(4) as usize);
    app.renderer
        .draw_text(pr.x + 2, footer_y, &f_trunc, c.accent, c.panel_bg);
}
