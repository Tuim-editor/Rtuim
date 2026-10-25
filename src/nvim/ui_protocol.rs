use rmpv::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridCell {
    pub text: String,
    pub hl_id: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HlAttr {
    pub fg: Option<u32>,
    pub bg: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GridData {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<GridCell>,
    pub win_row: i64,
    pub win_col: i64,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub is_float: bool,
    pub zindex: i64,
    pub hidden: bool,
}

impl GridData {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![
                GridCell {
                    text: " ".to_string(),
                    hl_id: 0
                };
                width * height
            ],
            win_row: 0,
            win_col: 0,
            cursor_row: 0,
            cursor_col: 0,
            is_float: false,
            zindex: 0,
            hidden: false,
        }
    }

    pub fn resize(&mut self, new_w: usize, new_h: usize) {
        let mut new_cells = vec![
            GridCell {
                text: " ".to_string(),
                hl_id: 0
            };
            new_w * new_h
        ];
        let min_h = self.height.min(new_h);
        let min_w = self.width.min(new_w);
        for r in 0..min_h {
            for c in 0..min_w {
                new_cells[r * new_w + c] = self.cells[r * self.width + c].clone();
            }
        }
        self.cells = new_cells;
        self.width = new_w;
        self.height = new_h;
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.text.clear();
            cell.text.push(' ');
            cell.hl_id = 0;
        }
    }
}

pub struct UiState {
    pub grid: GridData,
    pub secondary_grids: HashMap<u64, GridData>,
    pub highlights: HashMap<u64, HlAttr>,
    pub default_fg: u32,
    pub default_bg: u32,
    pub cursor_grid: u64,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub mode_info: Vec<String>,
    pub current_mode_idx: usize,
    pub current_mode: String,
    pub dirty: bool,
}

impl UiState {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            grid: GridData::new(cols as usize, rows as usize),
            secondary_grids: HashMap::new(),
            highlights: HashMap::new(),
            default_fg: 0,
            default_bg: 0,
            cursor_grid: 1,
            cursor_row: 0,
            cursor_col: 0,
            mode_info: Vec::new(),
            current_mode_idx: 0,
            current_mode: "normal".to_string(),
            dirty: true,
        }
    }

    fn grid_mut(&mut self, id: u64) -> &mut GridData {
        if id == 1 {
            &mut self.grid
        } else {
            self.secondary_grids
                .entry(id)
                .or_insert_with(|| GridData::new(1, 1))
        }
    }

    pub fn handle_redraw(&mut self, batches: &[Value]) {
        for batch in batches {
            let Some(arr) = batch.as_array() else {
                continue;
            };
            if arr.is_empty() {
                continue;
            }
            let Some(event_name) = arr[0].as_str() else {
                continue;
            };
            let args = &arr[1..];
            match event_name {
                "default_colors_set" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() >= 2 {
                            let fg = a[0].as_i64().unwrap_or(-1);
                            let bg = a[1].as_i64().unwrap_or(-1);
                            if fg >= 0 {
                                self.default_fg = fg as u32;
                            }
                            if bg >= 0 {
                                self.default_bg = bg as u32;
                            }
                        }
                    }
                    self.dirty = true;
                }
                "hl_attr_define" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() < 2 {
                            continue;
                        }
                        let Some(id) = a[0].as_u64() else { continue };
                        let mut hl = HlAttr::default();
                        if let Some(map) = a[1].as_map() {
                            for (k, v) in map {
                                let Some(key) = k.as_str() else { continue };
                                match key {
                                    "foreground" => {
                                        if let Some(val) = v.as_u64() {
                                            hl.fg = Some(val as u32);
                                        }
                                    }
                                    "background" => {
                                        if let Some(val) = v.as_u64() {
                                            hl.bg = Some(val as u32);
                                        }
                                    }
                                    "bold" => hl.bold = v.as_bool().unwrap_or(false),
                                    "italic" => hl.italic = v.as_bool().unwrap_or(false),
                                    "underline" | "undercurl" => {
                                        hl.underline = v.as_bool().unwrap_or(false)
                                    }
                                    "reverse" => hl.reverse = v.as_bool().unwrap_or(false),
                                    _ => {}
                                }
                            }
                        }
                        self.highlights.insert(id, hl);
                    }
                    self.dirty = true;
                }
                "grid_resize" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() >= 3 {
                            let id = a[0].as_u64().unwrap_or(1);
                            let w = a[1].as_u64().unwrap_or(1) as usize;
                            let h = a[2].as_u64().unwrap_or(1) as usize;
                            self.grid_mut(id).resize(w, h);
                        }
                    }
                    self.dirty = true;
                }
                "grid_clear" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if !a.is_empty() {
                            let id = a[0].as_u64().unwrap_or(1);
                            self.grid_mut(id).clear();
                        }
                    }
                    self.dirty = true;
                }
                "grid_destroy" | "win_close" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if !a.is_empty() {
                            let id = a[0].as_u64().unwrap_or(1);
                            if id != 1 {
                                self.secondary_grids.remove(&id);
                            }
                        }
                    }
                    self.dirty = true;
                }
                "win_hide" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if !a.is_empty() {
                            let id = a[0].as_u64().unwrap_or(1);
                            if id != 1 {
                                if let Some(g) = self.secondary_grids.get_mut(&id) {
                                    g.hidden = true;
                                }
                            }
                        }
                    }
                    self.dirty = true;
                }
                "win_pos" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() >= 4 {
                            let id = a[0].as_u64().unwrap_or(1);
                            if id != 1 {
                                let row = a[2].as_i64().unwrap_or(0);
                                let col = a[3].as_i64().unwrap_or(0);
                                let g = self.grid_mut(id);
                                g.win_row = row;
                                g.win_col = col;
                                g.is_float = false;
                                g.hidden = false;
                            }
                        }
                    }
                    self.dirty = true;
                }
                "msg_set_pos" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() >= 2 {
                            let id = a[0].as_u64().unwrap_or(1);
                            if id != 1 {
                                let row = a[1].as_i64().unwrap_or(0);
                                let g = self.grid_mut(id);
                                g.win_row = row;
                                g.win_col = 0;
                                g.is_float = true;
                                g.zindex = 200;
                                g.hidden = false;
                            }
                        }
                    }
                    self.dirty = true;
                }
                "win_float_pos" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() >= 6 {
                            let id = a[0].as_u64().unwrap_or(1);
                            if id == 1 {
                                continue;
                            }
                            let anchor_grid = a[3].as_u64().unwrap_or(1);
                            let (ar, ac) = if anchor_grid != 1 {
                                self.secondary_grids
                                    .get(&anchor_grid)
                                    .map(|ag| (ag.win_row, ag.win_col))
                                    .unwrap_or((0, 0))
                            } else {
                                (0, 0)
                            };
                            let g = self.grid_mut(id);
                            g.is_float = true;
                            g.hidden = false;
                            g.zindex = a.get(7).and_then(|v| v.as_i64()).unwrap_or(50);
                            if a.len() >= 11 && a[9].is_i64() && a[10].is_i64() {
                                g.win_row = a[9].as_i64().unwrap_or(0);
                                g.win_col = a[10].as_i64().unwrap_or(0);
                            } else {
                                let mut row = a[4].as_f64().unwrap_or(0.0) as i64 + ar;
                                let mut col = a[5].as_f64().unwrap_or(0.0) as i64 + ac;
                                if let Some(anchor) = a[2].as_str() {
                                    if anchor.contains('S') {
                                        row -= g.height as i64;
                                    }
                                    if anchor.contains('E') {
                                        col -= g.width as i64;
                                    }
                                }
                                g.win_row = row;
                                g.win_col = col;
                            }
                        }
                    }
                    self.dirty = true;
                }
                "grid_cursor_goto" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() >= 3 {
                            let id = a[0].as_u64().unwrap_or(1);
                            let row = a[1].as_u64().unwrap_or(0) as usize;
                            let col = a[2].as_u64().unwrap_or(0) as usize;
                            self.cursor_grid = id;
                            if id == 1 {
                                self.cursor_row = row;
                                self.cursor_col = col;
                            } else if let Some(g) = self.secondary_grids.get_mut(&id) {
                                g.cursor_row = row;
                                g.cursor_col = col;
                            }
                        }
                    }
                    self.dirty = true;
                }
                "grid_scroll" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() < 7 {
                            continue;
                        }
                        let id = a[0].as_u64().unwrap_or(1);
                        let top = a[1].as_u64().unwrap_or(0) as usize;
                        let bot = a[2].as_u64().unwrap_or(0) as usize;
                        let left = a[3].as_u64().unwrap_or(0) as usize;
                        let right = a[4].as_u64().unwrap_or(0) as usize;
                        let rows = a[5].as_i64().unwrap_or(0);
                        let cols = a[6].as_i64().unwrap_or(0);
                        let target = self.grid_mut(id);
                        if top >= target.height
                            || bot > target.height
                            || left >= target.width
                            || right > target.width
                            || top >= bot
                            || left >= right
                        {
                            continue;
                        }
                        let region_h = bot - top;
                        let region_w = right - left;
                        for yi in 0..region_h {
                            let y = if rows >= 0 { top + yi } else { bot - 1 - yi };
                            for xi in 0..region_w {
                                let x = if cols >= 0 { left + xi } else { right - 1 - xi };
                                let src_y = (y as i64) + rows;
                                let src_x = (x as i64) + cols;
                                let dst_idx = y * target.width + x;
                                if src_y >= (top as i64)
                                    && src_y < (bot as i64)
                                    && src_x >= (left as i64)
                                    && src_x < (right as i64)
                                {
                                    let src_idx =
                                        (src_y as usize) * target.width + (src_x as usize);
                                    target.cells[dst_idx] = target.cells[src_idx].clone();
                                } else {
                                    target.cells[dst_idx] = GridCell {
                                        text: " ".to_string(),
                                        hl_id: 0,
                                    };
                                }
                            }
                        }
                    }
                    self.dirty = true;
                }
                "grid_line" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if a.len() < 4 {
                            continue;
                        }
                        let id = a[0].as_u64().unwrap_or(1);
                        let row = a[1].as_u64().unwrap_or(0) as usize;
                        let mut col = a[2].as_u64().unwrap_or(0) as usize;
                        let Some(cells_arr) = a[3].as_array() else {
                            continue;
                        };
                        let target = self.grid_mut(id);
                        let mut current_hl_id = 0u64;
                        for c in cells_arr {
                            let Some(c_arr) = c.as_array() else { continue };
                            if c_arr.is_empty() {
                                continue;
                            }
                            let text = c_arr[0].as_str().unwrap_or(" ");
                            if c_arr.len() >= 2 {
                                current_hl_id = c_arr[1].as_u64().unwrap_or(current_hl_id);
                            }
                            let repeat = if c_arr.len() >= 3 {
                                c_arr[2].as_i64().unwrap_or(1).max(0) as usize
                            } else {
                                1
                            };
                            for _ in 0..repeat {
                                if row < target.height && col < target.width {
                                    let idx = row * target.width + col;
                                    target.cells[idx].text.clear();
                                    target.cells[idx].text.push_str(text);
                                    target.cells[idx].hl_id = current_hl_id;
                                }
                                col += 1;
                            }
                        }
                    }
                    self.dirty = true;
                }
                "mode_change" => {
                    for arg in args {
                        let Some(a) = arg.as_array() else { continue };
                        if let Some(m) = a.first().and_then(|v| v.as_str()) {
                            self.current_mode = m.to_string();
                        }
                    }
                    self.dirty = true;
                }
                _ => {}
            }
        }
    }
}
