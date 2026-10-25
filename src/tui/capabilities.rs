use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub true_color: bool,
    pub mouse: bool,
    pub bracketed_paste: bool,
    pub nerd_fonts_recommended: bool,
    pub distinct_modifiers: bool,
}

impl Capabilities {
    pub fn detect_from_env() -> Self {
        let map: HashMap<String, String> = std::env::vars().collect();
        Self::detect(&map)
    }

    pub fn detect(environ: &HashMap<String, String>) -> Self {
        let term = environ.get("TERM").map(|s| s.as_str()).unwrap_or("");
        let colorterm = environ.get("COLORTERM").map(|s| s.as_str()).unwrap_or("");
        let term_program = environ.get("TERM_PROGRAM").map(|s| s.as_str()).unwrap_or("");

        let dumb = term == "dumb" || term.is_empty();
        let linux_console = term == "linux";
        let tmux = environ.contains_key("TMUX") || term.starts_with("screen");
        let ssh = environ.contains_key("SSH_CONNECTION") || environ.contains_key("SSH_TTY");
        let wsl = environ.contains_key("WSL_DISTRO_NAME") || environ.contains_key("WSL_INTEROP");

        let modern_program = term_program == "WezTerm"
            || term_program.eq_ignore_ascii_case("ghostty")
            || term_program == "iTerm.app"
            || term_program.eq_ignore_ascii_case("kitty")
            || term_program.eq_ignore_ascii_case("Alacritty")
            || term_program == "Apple_Terminal"
            || environ.contains_key("KONSOLE_VERSION")
            || environ.contains_key("VTE_VERSION")
            || environ.contains_key("WT_SESSION");

        let true_color = !dumb
            && !linux_console
            && (colorterm.eq_ignore_ascii_case("truecolor")
                || colorterm.eq_ignore_ascii_case("24bit")
                || term.contains("direct")
                || modern_program);

        Self {
            true_color,
            mouse: !dumb && !linux_console,
            bracketed_paste: !dumb,
            nerd_fonts_recommended: !dumb && !linux_console && !tmux,
            distinct_modifiers: !dumb && !linux_console && !tmux && !ssh && !wsl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumb_and_linux_console_use_conservative_capabilities() {
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "dumb".to_string());
        let caps = Capabilities::detect(&env);
        assert!(!caps.true_color);
        assert!(!caps.mouse);
        assert!(!caps.bracketed_paste);

        env.insert("TERM".to_string(), "linux".to_string());
        let caps = Capabilities::detect(&env);
        assert!(!caps.true_color);
        assert!(!caps.mouse);
        assert!(caps.bracketed_paste);
    }

    #[test]
    fn modern_profiles_enable_color() {
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("TERM_PROGRAM".to_string(), "ghostty".to_string());
        let caps = Capabilities::detect(&env);
        assert!(caps.true_color);
        assert!(caps.mouse);
        assert!(caps.bracketed_paste);
    }
}
