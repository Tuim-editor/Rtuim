use crate::nvim::ui_protocol::{GridData, UiState};
use crate::tui::app::{App, FocusRegion, Mode};
use crate::tui::layout::{Layout, Rect};
use crate::tui::theme::chrome;
use crate::tui::widgets::primitives;
use crate::tui::workspace;

fn resolve_grid_colors(
    ui: &UiState,
    hl_id: u64,
    fallback_fg: u32,
    fallback_bg: u32,
) -> (u32, u32, bool, bool, bool) {
    let def_fg = if ui.default_fg != 0 {
        ui.default_fg
    } else {
        fallback_fg
    };
    let def_bg = if ui.default_bg != 0 {
        ui.default_bg
    } else {
        fallback_bg
    };

    if hl_id == 0 {
        return (def_fg, def_bg, false, false, false);
    }

    if let Some(attr) = ui.highlights.get(&hl_id) {
        let mut fg = attr.fg.unwrap_or(def_fg);
        let mut bg = attr.bg.unwrap_or(def_bg);
        if attr.reverse {
            std::mem::swap(&mut fg, &mut bg);
        }
        (fg, bg, attr.bold, attr.italic, attr.underline)
    } else {
        (def_fg, def_bg, false, false, false)
    }
}

fn blit_grid(
    app: &mut App,
    ui: &UiState,
    grid: &GridData,
    dest: Rect,
    fallback_fg: u32,
    fallback_bg: u32,
) {
    let rows = (grid.height as u16).min(dest.h);
    let cols = (grid.width as u16).min(dest.w);

    for r in 0..rows {
        for c in 0..cols {
            let idx = (r as usize) * grid.width + (c as usize);
            if let Some(cell) = grid.cells.get(idx) {
                let (fg, bg, bold, italic, underline) =
                    resolve_grid_colors(ui, cell.hl_id, fallback_fg, fallback_bg);
                let text = if cell.text.is_empty() {
                    " "
                } else {
                    cell.text.as_str()
                };
                app.renderer.put_cell(
                    dest.x + c,
                    dest.y + r,
                    text,
                    fg,
                    bg,
                    bold,
                    italic,
                    underline,
                );
            }
        }
    }
}

pub fn composite_nvim_ui(
    app: &mut App,
    dest: Rect,
    has_focus: bool,
) {
    if dest.w == 0 || dest.h == 0 {
        return;
    }
    let fallback_fg = app.theme.fg_primary;
    let fallback_bg = app.theme.bg_editor;
    app.renderer.fill_rect(dest, fallback_bg);

    // Clone grid snapshots to avoid borrow conflicts with app.renderer
    let (ui_clone_fg, ui_clone_bg, main_grid, sec_grids, cur_grid, cur_row, cur_col, highlights) = {
        let ui = &app.ui_state;
        let mut secs: Vec<(u64, GridData)> = ui
            .secondary_grids
            .iter()
            .filter(|(_, g)| !g.hidden)
            .map(|(k, g)| (*k, g.clone()))
            .collect();
        secs.sort_by_key(|(id, g)| (g.is_float, g.zindex, *id));
        (
            ui.default_fg,
            ui.default_bg,
            ui.grid.clone(),
            secs,
            ui.cursor_grid,
            ui.cursor_row,
            ui.cursor_col,
            ui.highlights.clone(),
        )
    };

    let temp_ui = UiState {
        grid: main_grid,
        secondary_grids: Default::default(),
        highlights,
        default_fg: ui_clone_fg,
        default_bg: ui_clone_bg,
        cursor_grid: cur_grid,
        cursor_row: cur_row,
        cursor_col: cur_col,
        mode_info: Vec::new(),
        current_mode_idx: 0,
        current_mode: String::new(),
        dirty: false,
    };

    blit_grid(app, &temp_ui, &temp_ui.grid, dest, fallback_fg, fallback_bg);

    for (_id, sg) in &sec_grids {
        let sub_x = dest.x.saturating_add(sg.win_col.max(0) as u16);
        let sub_y = dest.y.saturating_add(sg.win_row.max(0) as u16);
        if sub_x < dest.x + dest.w && sub_y < dest.y + dest.h {
            let sub_w = (sg.width as u16).min((dest.x + dest.w).saturating_sub(sub_x));
            let sub_h = (sg.height as u16).min((dest.y + dest.h).saturating_sub(sub_y));
            blit_grid(
                app,
                &temp_ui,
                sg,
                Rect::new(sub_x, sub_y, sub_w, sub_h),
                fallback_fg,
                fallback_bg,
            );
        }
    }

    if has_focus {
        let (mut cx, mut cy) = (dest.x + cur_col as u16, dest.y + cur_row as u16);
        if cur_grid != 1 {
            if let Some((_, sg)) = sec_grids.iter().find(|(id, _)| *id == cur_grid) {
                cx = dest
                    .x
                    .saturating_add(sg.win_col.max(0) as u16)
                    .saturating_add(sg.cursor_col as u16);
                cy = dest
                    .y
                    .saturating_add(sg.win_row.max(0) as u16)
                    .saturating_add(sg.cursor_row as u16);
            }
        }
        if dest.contains(cx, cy) {
            let idx = (cy as usize) * (app.renderer.width as usize) + (cx as usize);
            if let Some(cell) = app.renderer.buf.get_mut(idx) {
                cell.fg = fallback_bg;
                cell.bg = app.theme.fg_primary;
            }
            app.renderer.set_cursor(cx, cy, true);
        }
    }
}

pub fn draw_workspace(app: &mut App) {
    let layout = app.current_layout();
    let screen = Rect::new(0, 0, app.renderer.width, app.renderer.height);
    let c = chrome(&app.theme);

    app.renderer.set_cursor(0, 0, false);

    // 1. Editor Grid
    let editor_focused = app.workspace.focus == FocusRegion::Editor
        && !app.workspace.palette_open
        && !app.settings_widget.is_open
        && !app.mason_widget.is_open
        && !app.lazy_widget.is_open
        && !app.git_detailed_widget.is_open
        && !app.orchestrator.is_open
        && !app.bug_report.is_open
        && !app.extension_shop.is_open;
    composite_nvim_ui(app, layout.editor, editor_focused);

    // Draw split window header close affordances if multiple splits exist
    if app.nvim_wins.len() > 1 && app.mode != Mode::Zen {
        for win in &app.nvim_wins {
            let wx = layout.editor.x + win.col;
            let wy = layout.editor.y + win.row;
            if wx + win.width <= layout.editor.x + layout.editor.w
                && wy < layout.editor.y + layout.editor.h
                && win.width >= 10
            {
                let close_x = wx + win.width.saturating_sub(3);
                app.renderer
                    .draw_text(close_x, wy, "×", c.text_muted, c.surface_bg);
            }
        }
    }

    // 2. Sidebar + Activity Header + Vertical Divider
    if app.mode != Mode::Zen && layout.sidebar.w > 0 {
        workspace::draw_sidebar(app, &layout);
        let div_x = layout.sidebar.x + layout.sidebar.w;
        if div_x < screen.w {
            let sep_ch = match app.settings_widget.config.separator.as_str() {
                "thick" => "┃",
                "double" => "║",
                _ => "│",
            };
            app.renderer.draw_vline(
                div_x,
                0,
                screen.h.saturating_sub(1),
                sep_ch,
                c.border,
                c.panel_bg,
            );
        }
    }

    if layout.vertical_terminal.w > 0 {
        let rect = layout.vertical_terminal;
        app.renderer.draw_vline(rect.x - 1, rect.y - 1, rect.h + 1, "│", c.border, c.panel_bg);
        app.renderer.fill_rect(Rect::new(rect.x, rect.y - 1, rect.w, 1), c.surface_bg);
        super::widgets::primitives::text_in(&mut app.renderer,
            Rect::new(rect.x + 1, rect.y - 1, rect.w.saturating_sub(4), 1),
            "[Terminal]", c.text_strong, c.surface_bg, true);
        app.renderer.draw_text(rect.x + rect.w.saturating_sub(3), rect.y - 1, "×", c.text_muted, c.surface_bg);
        let focused = app.workspace.focus == FocusRegion::VerticalTerminal && !has_native_overlay(app)
            && app.telescope_main_rect.is_none();
        app.vertical_terminal.draw(&mut app.renderer, rect, &app.theme, focused);
    }

    // 3. Bottom Drawer (Terminal / Debug console / Output)
    if layout.terminal.h > 0 {
        let header_y = layout.terminal.y.saturating_sub(1);
        let tx = layout.terminal.x;
        let tw = layout.terminal.w;
        app.renderer
            .fill_rect(Rect::new(tx, header_y, tw, 1), c.surface_bg);

        let t0_fg = if app.workspace.drawer_tab == 0 {
            c.text_strong
        } else {
            c.text_muted
        };
        let t1_fg = if app.workspace.drawer_tab == 1 {
            c.text_strong
        } else {
            c.text_muted
        };
        let t2_fg = if app.workspace.drawer_tab == 2 {
            c.text_strong
        } else {
            c.text_muted
        };

        if tw >= 46 {
            // Exact offsets required by terminal_resize_ui.py: 'Terminal' at tx + 3
            app.renderer.draw_text_styled(
                tx + 2,
                header_y,
                "[Terminal]",
                t0_fg,
                c.surface_bg,
                app.workspace.drawer_tab == 0,
                false,
            );
            app.renderer.draw_text_styled(
                tx + 14,
                header_y,
                "[Debug console]",
                t1_fg,
                c.surface_bg,
                app.workspace.drawer_tab == 1,
                false,
            );
            app.renderer.draw_text_styled(
                tx + 31,
                header_y,
                "[Output]",
                t2_fg,
                c.surface_bg,
                app.workspace.drawer_tab == 2,
                false,
            );
        } else {
            // Exact offsets required by terminal_resize_ui.py when narrow: 'Term' at tx + 2
            app.renderer.draw_text_styled(
                tx + 1,
                header_y,
                "[Term]",
                t0_fg,
                c.surface_bg,
                app.workspace.drawer_tab == 0,
                false,
            );
            app.renderer.draw_text_styled(
                tx + 8,
                header_y,
                "[Debug]",
                t1_fg,
                c.surface_bg,
                app.workspace.drawer_tab == 1,
                false,
            );
            app.renderer.draw_text_styled(
                tx + 16,
                header_y,
                "[Out]",
                t2_fg,
                c.surface_bg,
                app.workspace.drawer_tab == 2,
                false,
            );
        }

        let drawer_focused = app.workspace.focus == FocusRegion::Drawer && !has_native_overlay(app);

        match app.workspace.drawer_tab {
            0 => app.shell_terminal.draw(&mut app.renderer, layout.terminal, &app.theme, drawer_focused),
            1 => app
                .debug_console
                .draw(&mut app.renderer, layout.terminal, &app.theme),
            2 => app
                .output_panel
                .draw(&mut app.renderer, layout.terminal, &app.theme),
            _ => app.shell_terminal.draw(&mut app.renderer, layout.terminal, &app.theme, drawer_focused),
        }
    }

    // 4. Top & Bottom Chrome
    workspace::draw_chrome(app, &layout);

    // 5. Non-native floating window chrome (e.g. Tuim Help Reference when not in Zen mode)
    if !app.native_picker_chrome {
        if let Some(mt) = app.telescope_main_rect {
            let fx = layout.editor.x + mt.x;
            let fy = layout.editor.y + mt.y;
            if fx > 0 && fy > 0 {
                let border_r = Rect::new(fx - 1, fy - 1, mt.w + 2, mt.h + 2);
                primitives::draw_rounded_border(
                    &mut app.renderer,
                    border_r,
                    Some(&app.telescope_title),
                    c.border,
                    c.panel_bg,
                );
            }
        }
    }

    // 6. Extension Shop Main Panel
    if app.extension_shop.is_open {
        app.extension_shop
            .draw(&mut app.renderer, screen, &app.theme);
    }

    // 7. Modals & Overlays
    if app.workspace.palette_open {
        workspace::draw_palette(app, screen);
    }
    if app.settings_widget.is_open {
        app.settings_widget
            .draw(&mut app.renderer, screen, &app.theme);
    }
    if app.mason_widget.is_open {
        app.mason_widget.draw(&mut app.renderer, screen, &app.theme);
    }
    if app.lazy_widget.is_open {
        app.lazy_widget.draw(&mut app.renderer, screen, &app.theme);
    }
    if app.git_detailed_widget.is_open {
        app.git_detailed_widget
            .draw(&mut app.renderer, screen, &app.theme);
    }
    if app.orchestrator.is_open {
        app.orchestrator.draw(&mut app.renderer, screen, &app.theme);
    }
    if app.bug_report.is_open {
        app.bug_report.draw(&mut app.renderer, screen, &app.theme);
    }
    if app.menu_bar.open_menu.is_some() {
        let cmd_shortcut =
            workspace::format_shortcut_display(&app.settings_widget.config.keybindings.commands);
        let right_reserved = cmd_shortcut.len() as u16 + 12;
        let menu_r = Rect::new(
            layout.tab_bar.x + layout.tab_bar.w.saturating_sub(right_reserved + 30),
            layout.tab_bar.y,
            28,
            1,
        );
        app.menu_bar
            .draw_dropdown(&mut app.renderer, menu_r, &app.theme);
    }
    if app.Mode_allows_explorer_overlay(&layout) {
        app.explorer
            .draw_overlay(&mut app.renderer, layout.sidebar, &app.theme);
    }
    if app.editor_context_menu.is_open {
        app.editor_context_menu
            .draw(&mut app.renderer, screen, &app.theme);
    }

    // 8. Toast Notice Banner
    if let Some(notice) = &app.notice {
        let msg = format!(" {} ", notice.message);
        let nw = (msg.chars().count() as u16 + 2).min(screen.w.saturating_sub(4));
        let nx = screen.w.saturating_sub(nw + 2);
        let ny = screen.h.saturating_sub(3);
        let fg = match notice.level.as_str() {
            "error" => app.theme.error,
            "warning" => app.theme.warning,
            _ => c.text_strong,
        };
        let nr = Rect::new(nx, ny, nw, 1);
        app.renderer.fill_rect(nr, c.elevated_bg);
        let trunc = primitives::truncate_to_width(&msg, nw as usize);
        app.renderer
            .draw_text_styled(nx, ny, &trunc, fg, c.elevated_bg, true, false);
    }

    // 9. Passive Mouse Hover Highlight
    if let (Some(hx), Some(hy)) = (app.hover_x, app.hover_y) {
        if app.settings_widget.is_open {
            app.settings_widget.draw_hover(&mut app.renderer, screen, hx, hy, &app.theme);
        } else if !has_native_overlay(app) && !app.explorer.context_menu_open
            && (layout.sidebar.contains(hx, hy) || layout.activity_bar.contains(hx, hy)) {
            if let Some(rect) = workspace::sidebar_hover_rect(app, &layout, hx, hy) {
                app.renderer.highlight_rect(rect, c.row_hover_bg);
            } else {
                app.renderer.apply_hover(hx, hy, c.row_hover_bg);
            }
        } else if !editor_contains_non_chrome(app, &layout, hx, hy) {
            app.renderer.apply_hover(hx, hy, c.row_hover_bg);
        }
    }
}

fn has_native_overlay(app: &App) -> bool {
    app.workspace.palette_open
        || app.settings_widget.is_open
        || app.mason_widget.is_open
        || app.lazy_widget.is_open
        || app.git_detailed_widget.is_open
        || app.orchestrator.is_open
        || app.bug_report.is_open
        || app.extension_shop.is_open
        || app.editor_context_menu.is_open
        || app.menu_bar.open_menu.is_some()
}

fn editor_contains_non_chrome(app: &App, layout: &Layout, x: u16, y: u16) -> bool {
    !has_native_overlay(app) && (layout.editor.contains(x, y)
        || layout.vertical_terminal.contains(x, y)
        || (app.workspace.drawer_tab == 0 && layout.terminal.contains(x, y)))
}

impl App {
    #[allow(non_snake_case)]
    pub fn Mode_allows_explorer_overlay(&self, layout: &Layout) -> bool {
        self.mode != Mode::Zen
            && layout.sidebar.w > 0
            && !self.workspace.overview
            && self.activity_bar.active_tab == 0
    }
}
