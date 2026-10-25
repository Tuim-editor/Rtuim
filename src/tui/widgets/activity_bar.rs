pub struct ActivityBar {
    pub active_tab: usize,
}

impl ActivityBar {
    pub fn new() -> Self {
        Self { active_tab: 0 }
    }
}

impl Default for ActivityBar {
    fn default() -> Self {
        Self::new()
    }
}
