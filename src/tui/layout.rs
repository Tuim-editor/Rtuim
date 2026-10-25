#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.w > 0
            && self.h > 0
            && x >= self.x
            && x < self.x.saturating_add(self.w)
            && y >= self.y
            && y < self.y.saturating_add(self.h)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    pub total: Rect,
    pub tab_bar: Rect,
    pub activity_bar: Rect,
    pub sidebar: Rect,
    pub editor: Rect,
    pub terminal: Rect,
    pub vertical_terminal: Rect,
    pub status_bar: Rect,
}

impl Layout {
    /// Carve a native right-hand shell out of the editor, above the bottom drawer.
    pub fn with_vertical_terminal(mut self, width: u16) -> Self {
        if width > 0 && self.editor.w >= 12 && self.editor.h >= 3 {
            let w = width.min(self.editor.w.saturating_sub(6));
            self.editor.w -= w + 1;
            self.vertical_terminal = Rect::new(
                self.editor.x + self.editor.w + 1, self.editor.y + 1, w, self.editor.h - 1,
            );
        }
        self
    }

    pub fn workspace(
        cols: u16,
        rows: u16,
        is_zen: bool,
        sidebar_width: u16,
        terminal_height: u16,
    ) -> Self {
        let total = Rect::new(0, 0, cols, rows);
        let sb_w = if !is_zen && sidebar_width > 0 && cols >= 40 {
            sidebar_width.min(cols.saturating_sub(24))
        } else {
            0
        };

        let right_x = if sb_w > 0 {
            (sb_w + 1).min(cols)
        } else {
            0
        };
        let right_w = cols.saturating_sub(right_x);

        let top_h = if is_zen { 0 } else { rows.min(1) };
        let status_h = if rows > 1 { 1 } else { 0 };
        let body_h = rows.saturating_sub(top_h + status_h);

        let (editor_h, term_h) = if terminal_height > 0 && body_h >= 6 {
            let th = terminal_height.min(body_h.saturating_sub(3)).max(2);
            // Reserve 1 row above terminal for `[Terminal] [Debug console] [Output]` header
            let eh = body_h.saturating_sub(th + 1);
            (eh, th)
        } else {
            (body_h, 0)
        };

        let activity_bar = if sb_w > 0 {
            Rect::new(0, 0, sb_w, rows.saturating_sub(status_h).min(2))
        } else {
            Rect::default()
        };

        let sidebar = if sb_w > 0 && rows > 3 {
            Rect::new(0, 2, sb_w, rows.saturating_sub(2 + status_h))
        } else {
            Rect::default()
        };

        let tab_bar = Rect::new(right_x, 0, right_w, top_h);
        let editor = Rect::new(right_x, top_h, right_w, editor_h);
        let terminal = if term_h > 0 {
            Rect::new(right_x, top_h + editor_h + 1, right_w, term_h)
        } else {
            Rect::default()
        };
        let status_bar = Rect::new(0, rows.saturating_sub(status_h), cols, status_h);

        Self {
            total,
            tab_bar,
            activity_bar,
            sidebar,
            editor,
            terminal,
            vertical_terminal: Rect::default(),
            status_bar,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_sidebar_and_zen_geometry() {
        let normal = Layout::workspace(100, 30, false, 24, 0);
        assert_eq!(normal.sidebar.w, 24);
        assert_eq!(normal.editor.x, 25);
        assert_eq!(normal.editor.w, 75);

        let narrow = Layout::workspace(30, 12, false, 24, 0);
        assert_eq!(narrow.sidebar.w, 0);
        assert_eq!(narrow.editor.w, 30);

        let zen = Layout::workspace(120, 40, true, 24, 0);
        assert_eq!(zen.editor, Rect::new(0, 0, 120, 39));
        assert_eq!(zen.status_bar, Rect::new(0, 39, 120, 1));
    }
    #[test]
    fn native_terminal_panes_fit_in_normal_and_zen_layouts() {
        for zen in [false, true] {
            for (cols, rows) in [(120, 36), (45, 16), (12, 8), (1, 1), (0, 0)] {
                let layout = Layout::workspace(cols, rows, zen, 24, 10).with_vertical_terminal(40);
                let rects = [layout.editor, layout.terminal, layout.vertical_terminal, layout.status_bar];
                for rect in rects {
                    assert!(rect.x + rect.w <= cols);
                    assert!(rect.y + rect.h <= rows);
                }
                for (i, a) in rects.iter().enumerate() {
                    for b in &rects[i + 1..] {
                        if a.w > 0 && a.h > 0 && b.w > 0 && b.h > 0 {
                            assert!(a.x + a.w <= b.x || b.x + b.w <= a.x
                                || a.y + a.h <= b.y || b.y + b.h <= a.y);
                        }
                    }
                }
                if cols == 120 {
                    assert!(layout.terminal.h > 0 && layout.vertical_terminal.w > 0);
                }
            }
        }
    }

}
