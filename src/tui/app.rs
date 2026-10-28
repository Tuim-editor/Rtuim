use std::time::{SystemTime, UNIX_EPOCH};

use crate::metrics::Metrics;
use crate::nvim::rpc::RpcClient;
use crate::nvim::ui_protocol::UiState;
use crate::task_runner::TaskRunner;
use crate::tui::capabilities::Capabilities;
use crate::tui::invalidation::{Dimensions, Invalidations};
use crate::tui::layout::{Layout, Rect};
use crate::tui::renderer::Renderer;
use crate::tui::theme::Theme;
use crate::tui::widgets::{
    activity_bar::ActivityBar,
    ai_panel::AiPanel,
    bug_report::BugReportWidget,
    editor_context_menu::EditorContextMenu,
    explorer::FileExplorer,
    extension_shop::ExtensionShop,
    git_detailed::GitDetailedWidget,
    git_panel::GitPanel,
    lazy::LazyWidget,
    mason::MasonWidget,
    menu_bar::MenuBar,
    orchestrator::WorktreeOrchestrator,
    search_panel::{ConsolePanel, SearchPanel},
    settings::{SettingsConfig, SettingsWidget},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ide,
    Zen,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    Editor,
    Sidebar,
    Drawer,
    VerticalTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    Buffers,
    LineJump,
}

#[derive(Debug, Clone, Default)]
pub struct TabInfo {
    pub bufnr: i64,
    pub name: String,
    pub relative_name: String,
    pub changed: bool,
    pub is_preview: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NvimWin {
    pub id: i64,
    pub bufnr: i64,
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub height: u16,
    pub active: bool,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub level: String,
    pub message: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub sidebar_visible: bool,
    pub drawer_visible: bool,
    pub vertical_terminal_visible: bool,
    pub drawer_tab: usize,
    pub focus: FocusRegion,
    pub prior_focus: FocusRegion,
    pub sidebar_index: usize,
    pub sidebar_scroll: usize,
    pub overview: bool,
    pub palette_open: bool,
    pub palette_mode: PaletteMode,
    pub palette_query: String,
    pub palette_index: usize,
    pub palette_scroll: usize,
    pub palette_recording_key: bool,
    pub palette_status: String,
    pub zen_return_mode: Mode,
    pub zen_return_sidebar: bool,
    pub zen_return_drawer: bool,
    pub zen_return_vertical_terminal: bool,
    pub zen_return_focus: FocusRegion,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            sidebar_visible: true,
            drawer_visible: false,
            vertical_terminal_visible: false,
            drawer_tab: 0,
            focus: FocusRegion::Editor,
            prior_focus: FocusRegion::Editor,
            sidebar_index: 0,
            sidebar_scroll: 0,
            overview: false,
            palette_open: false,
            palette_mode: PaletteMode::Commands,
            palette_query: String::new(),
            palette_index: 0,
            palette_scroll: 0,
            palette_recording_key: false,
            palette_status: String::new(),
            zen_return_mode: Mode::Ide,
            zen_return_sidebar: true,
            zen_return_drawer: false,
            zen_return_vertical_terminal: false,
            zen_return_focus: FocusRegion::Editor,
        }
    }
}

pub struct App {
    pub renderer: Renderer,
    pub editor_rpc: RpcClient,
    pub shell_terminal: crate::tui::shell_terminal::ShellTerminal,
    pub vertical_terminal: crate::tui::shell_terminal::ShellTerminal,
    pub ui_state: UiState,
    pub theme: Theme,
    pub capabilities: Capabilities,
    pub metrics: Metrics,
    pub task_runner: TaskRunner,
    pub workspace: WorkspaceState,
    pub mode: Mode,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    pub nvim_wins: Vec<NvimWin>,
    pub nvim_win_count: usize,
    pub telescope_main_rect: Option<Rect>,
    pub telescope_prev_rect: Option<Rect>,
    pub telescope_title: String,
    pub native_picker_chrome: bool,
    pub menu_bar: MenuBar,
    pub orchestrator: WorktreeOrchestrator,
    pub activity_bar: ActivityBar,
    pub explorer: FileExplorer,
    pub git_panel: GitPanel,
    pub git_detailed_widget: GitDetailedWidget,
    pub ai_panel: AiPanel,
    pub search_panel: SearchPanel,
    pub debug_console: ConsolePanel,
    pub output_panel: ConsolePanel,
    pub extension_shop: ExtensionShop,
    pub mason_widget: MasonWidget,
    pub lazy_widget: LazyWidget,
    pub settings_widget: SettingsWidget,
    pub bug_report: BugReportWidget,
    pub editor_context_menu: EditorContextMenu,
    pub show_split_menu: bool,
    pub split_menu_index: usize,
    pub dragging_sidebar: bool,
    pub dragging_terminal: bool,
    pub dragging_vertical_terminal: bool,
    pub vertical_terminal_w: u16,
    pub sidebar_w: u16,
    pub terminal_h: u16,
    pub hover_x: Option<u16>,
    pub hover_y: Option<u16>,
    pub notice: Option<Notice>,
    pub invalidations: Invalidations,
    pub last_dims: Dimensions,
    pub data_dir: String,
    pub home_dir: String,
    pub should_quit: bool,
    pub restart_requested: bool,
    pub zen_handoff_requested: bool,
    pub zen_handoff_file: String,
    pub zen_handoff_line: i64,
}

impl App {
    pub fn new(
        cols: u16,
        rows: u16,
        editor_rpc: RpcClient,
        capabilities: Capabilities,
        data_dir: String,
        home_dir: String,
    ) -> Self {
        let settings_path = format!("{}/settings.json", data_dir);
        let mut config = SettingsConfig::load(&settings_path).unwrap_or_default();
        if !capabilities.nerd_fonts_recommended {
            config.nerd_fonts = false;
        }
        let initial_mode = match config.mode.as_str() {
            "zen" => Mode::Zen,
            "normal" => Mode::Normal,
            _ => Mode::Ide,
        };

        let mut workspace = WorkspaceState::default();
        if initial_mode == Mode::Zen {
            workspace.sidebar_visible = false;
            workspace.drawer_visible = false;
        }

        let app = Self {
            renderer: Renderer::new(cols, rows),
            editor_rpc,
            shell_terminal: crate::tui::shell_terminal::ShellTerminal::default(),
            vertical_terminal: crate::tui::shell_terminal::ShellTerminal::default(),
            ui_state: UiState::new(cols.max(1), rows.max(1)),
            theme: Theme::default(),
            capabilities,
            metrics: Metrics::new(),
            task_runner: TaskRunner::new(),
            workspace,
            mode: initial_mode,
            tabs: vec![TabInfo {
                bufnr: 1,
                name: String::new(),
                relative_name: String::new(),
                changed: false,
                is_preview: false,
            }],
            active_tab: 0,
            nvim_wins: Vec::new(),
            nvim_win_count: 1,
            telescope_main_rect: None,
            telescope_prev_rect: None,
            telescope_title: String::new(),
            native_picker_chrome: false,
            menu_bar: MenuBar::new(),
            orchestrator: WorktreeOrchestrator::new(),
            activity_bar: ActivityBar::new(),
            explorer: FileExplorer::new(),
            git_panel: GitPanel::new(),
            git_detailed_widget: GitDetailedWidget::new(),
            ai_panel: AiPanel::new(),
            search_panel: SearchPanel::new(),
            debug_console: ConsolePanel::new("Debug Console"),
            output_panel: ConsolePanel::new("Output"),
            extension_shop: ExtensionShop::new(&data_dir),
            mason_widget: MasonWidget::new(),
            lazy_widget: LazyWidget::new(),
            settings_widget: SettingsWidget::new(&settings_path, config),
            bug_report: BugReportWidget::new(&data_dir, &home_dir),
            editor_context_menu: EditorContextMenu::new(),
            show_split_menu: false,
            split_menu_index: 0,
            dragging_sidebar: false,
            dragging_terminal: false,
            dragging_vertical_terminal: false,
            vertical_terminal_w: 40,
            sidebar_w: 24,
            terminal_h: 10,
            hover_x: None,
            hover_y: None,
            notice: None,
            invalidations: Invalidations::new(),
            last_dims: Dimensions { cols, rows },
            data_dir,
            home_dir,
            should_quit: false,
            restart_requested: false,
            zen_handoff_requested: false,
            zen_handoff_file: String::new(),
            zen_handoff_line: 1,
        };
        app.request_git_refresh();
        app
    }

    pub fn current_layout(&self) -> Layout {
        let sidebar_w = if self.mode == Mode::Zen || !self.workspace.sidebar_visible {
            0
        } else {
            self.sidebar_w
        };
        let term_h = if !self.workspace.drawer_visible {
            0
        } else {
            self.terminal_h
        };
        Layout::workspace(
            self.renderer.width,
            self.renderer.height,
            self.mode == Mode::Zen,
            sidebar_w,
            term_h,
        ).with_vertical_terminal(if self.workspace.vertical_terminal_visible { self.vertical_terminal_w } else { 0 })
    }

    pub fn request_git_refresh(&self) {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(s) = cwd.to_str() {
                self.task_runner.request_git_snapshot(s);
            }
        }
    }

    pub fn poll_background_tasks(&mut self) -> bool {
        let mut changed = false;
        for snap in self.task_runner.poll_results() {
            self.explorer.update_git_status(&snap);
            self.git_panel.apply_snapshot(snap.clone());
            self.git_detailed_widget.apply_snapshot(snap.clone());
            self.orchestrator.apply_snapshot(&snap);
            changed = true;
        }
        if self.bug_report.poll_submission() {
            changed = true;
        }
        if self.settings_widget.poll_software_update() {
            changed = true;
        }
        if let Some(n) = &self.notice {
            if Self::now_ms() >= n.expires_at_ms {
                self.notice = None;
                changed = true;
            }
        }
        if changed {
            self.invalidations.mark_chrome();
        }
        changed
    }

    pub fn set_notice(&mut self, level: &str, message: &str) {
        self.notice = Some(Notice {
            level: level.to_string(),
            message: message.to_string(),
            expires_at_ms: Self::now_ms() + 2000,
        });
        self.output_panel
            .push_line(format!("[{}] {}", level.to_uppercase(), message));
        self.invalidations.mark_chrome();
    }

    pub fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn active_tab_display_name(&self) -> String {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if !tab.relative_name.is_empty() {
                return tab.relative_name.clone();
            }
            if !tab.name.is_empty() {
                return std::path::Path::new(&tab.name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&tab.name)
                    .to_string();
            }
        }
        "[No Name]".to_string()
    }
}
