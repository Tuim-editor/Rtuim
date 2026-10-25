use rmpv::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub bg_editor: u32,
    pub bg_sidebar: u32,
    pub bg_tab_active: u32,
    pub bg_tab_inactive: u32,
    pub bg_statusbar: u32,
    pub fg_statusbar: u32,
    pub bg_terminal: u32,
    pub fg_terminal: u32,
    pub terminal_colors: [u32; 16],
    pub bg_accent: u32,
    pub fg_primary: u32,
    pub fg_secondary: u32,
    pub fg_accent: u32,
    pub border_color: u32,
    pub accent: u32,
    pub success: u32,
    pub warning: u32,
    pub error: u32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg_editor: 0x1e1e1e,
            bg_sidebar: 0x252526,
            bg_tab_active: 0x1e1e1e,
            bg_tab_inactive: 0x2d2d2d,
            bg_statusbar: 0x007acc,
            fg_statusbar: 0xffffff,
            bg_terminal: 0x1e1e1e,
            fg_terminal: 0xd4d4d4,
            terminal_colors: [
                0x1e1e1e, 0xf2495a, 0x42be65, 0xffcc00, 0x007acc, 0xc678dd, 0x56b6c2, 0xd4d4d4,
                0x858585, 0xf2495a, 0x42be65, 0xffcc00, 0x007acc, 0xc678dd, 0x56b6c2, 0xffffff,
            ],
            bg_accent: 0x094771,
            fg_primary: 0xd4d4d4,
            fg_secondary: 0x969696,
            fg_accent: 0xffffff,
            border_color: 0x454545,
            accent: 0x3794ff,
            success: 0x73c991,
            warning: 0xe2c08d,
            error: 0xf44747,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChromePalette {
    pub surface_bg: u32,
    pub panel_bg: u32,
    pub elevated_bg: u32,
    pub row_hover_bg: u32,
    pub row_selected_bg: u32,
    pub border: u32,
    pub text_strong: u32,
    pub text: u32,
    pub text_muted: u32,
    pub accent: u32,
    pub success: u32,
    pub warning: u32,
    pub error: u32,
}

pub fn chrome(theme: &Theme) -> ChromePalette {
    let panel_bg = theme.bg_sidebar;
    let surface_bg = blend_u32(panel_bg, theme.fg_primary, 6);
    let elevated_bg = blend_u32(panel_bg, theme.fg_primary, 12);
    let row_hover_bg = blend_u32(panel_bg, theme.fg_primary, 10);
    let row_selected_bg = theme.bg_accent;
    let border = readable_u32(theme.border_color, panel_bg, 3.0);
    let text_strong = readable_u32(theme.fg_accent, panel_bg, 7.0);
    let text = readable_u32(theme.fg_primary, panel_bg, 7.0);
    let text_muted = readable_u32(theme.fg_secondary, panel_bg, 4.5);

    ChromePalette {
        surface_bg,
        panel_bg,
        elevated_bg,
        row_hover_bg,
        row_selected_bg,
        border,
        text_strong,
        text,
        text_muted,
        accent: readable_u32(theme.accent, panel_bg, 4.5),
        success: theme.success,
        warning: theme.warning,
        error: theme.error,
    }
}

impl Theme {
    pub fn parse_hex_color(hex: &str) -> Option<u32> {
        let s = hex.strip_prefix('#').unwrap_or(hex);
        if s.len() != 6 {
            return None;
        }
        u32::from_str_radix(s, 16).ok()
    }

    pub fn update_from_config(&mut self, map: &[(Value, Value)]) {
        for (k, v) in map {
            if k.as_str() == Some("terminal_colors") {
                if let Some(colors) = v.as_array() {
                    for (i, color) in colors.iter().take(16).enumerate() {
                        if let Some(color) = color.as_str().and_then(Self::parse_hex_color) {
                            self.terminal_colors[i] = color;
                        }
                    }
                }
            }
            if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                if let Some(c) = Self::parse_hex_color(val) {
                    match key {
                        "bg_editor" => self.bg_editor = c,
                        "bg_sidebar" => self.bg_sidebar = c,
                        "bg_tab_active" => self.bg_tab_active = c,
                        "bg_tab_inactive" => self.bg_tab_inactive = c,
                        "bg_statusbar" => self.bg_statusbar = c,
                        "fg_statusbar" => self.fg_statusbar = c,
                        "bg_terminal" => self.bg_terminal = c,
                        "bg_accent" => self.bg_accent = c,
                        "fg_primary" => { self.fg_primary = c; self.fg_terminal = c; }
                        "fg_secondary" => self.fg_secondary = c,
                        "fg_accent" => self.fg_accent = c,
                        "border_color" => self.border_color = c,
                        _ => {}
                    }
                }
            }
        }
        self.fg_primary = readable_u32(self.fg_primary, self.bg_sidebar, 7.0);
        self.fg_secondary = readable_u32(self.fg_secondary, self.bg_sidebar, 4.5);
        self.fg_accent = readable_u32(self.fg_accent, self.bg_sidebar, 4.5);
        self.border_color = readable_u32(self.border_color, self.bg_sidebar, 3.0);
        self.fg_statusbar = readable_u32(self.fg_statusbar, self.bg_statusbar, 4.5);
    }
}

pub fn blend_u32(bg: u32, fg: u32, percent: u32) -> u32 {
    let br = (bg >> 16) & 0xff;
    let bg_g = (bg >> 8) & 0xff;
    let bb = bg & 0xff;
    let fr = (fg >> 16) & 0xff;
    let fg_g = (fg >> 8) & 0xff;
    let fb = fg & 0xff;
    let r = (br * (100 - percent) + fr * percent) / 100;
    let g = (bg_g * (100 - percent) + fg_g * percent) / 100;
    let b = (bb * (100 - percent) + fb * percent) / 100;
    (r << 16) | (g << 8) | b
}

fn linear_channel(value: u8) -> f32 {
    let channel = (value as f32) / 255.0;
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance_u32(c: u32) -> f32 {
    let r = ((c >> 16) & 0xff) as u8;
    let g = ((c >> 8) & 0xff) as u8;
    let b = (c & 0xff) as u8;
    0.2126 * linear_channel(r) + 0.7152 * linear_channel(g) + 0.0722 * linear_channel(b)
}

pub fn contrast_ratio_u32(a: u32, b: u32) -> f32 {
    let la = luminance_u32(a);
    let lb = luminance_u32(b);
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

pub fn readable_u32(fg: u32, bg: u32, min_ratio: f32) -> u32 {
    if contrast_ratio_u32(fg, bg) >= min_ratio {
        return fg;
    }
    let endpoint = if contrast_ratio_u32(0xffffff, bg) >= contrast_ratio_u32(0x000000, bg) {
        0xffffff
    } else {
        0x000000
    };
    for step in 1..=20 {
        let cand = blend_u32(fg, endpoint, step * 5);
        if contrast_ratio_u32(cand, bg) >= min_ratio {
            return cand;
        }
    }
    endpoint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_keeps_contrast_readable() {
        let t = Theme::default();
        assert!(contrast_ratio_u32(t.fg_primary, t.bg_editor) >= 7.0);
        assert!(contrast_ratio_u32(t.fg_secondary, t.bg_sidebar) >= 4.0);
    }
}
