#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dimensions {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositionDamage {
    pub chrome: bool,
    pub sidebar: bool,
    pub editor: bool,
    pub drawer: bool,
    pub overlay: bool,
}

impl CompositionDamage {
    pub fn all() -> Self {
        Self {
            chrome: true,
            sidebar: true,
            editor: true,
            drawer: true,
            overlay: true,
        }
    }

    pub fn any(&self) -> bool {
        self.chrome || self.sidebar || self.editor || self.drawer || self.overlay
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Invalidations {
    pub layout: bool,
    pub composition: CompositionDamage,
    pub cursor: bool,
}

impl Default for Invalidations {
    fn default() -> Self {
        Self::new()
    }
}

impl Invalidations {
    pub fn new() -> Self {
        Self {
            layout: true,
            composition: CompositionDamage::all(),
            cursor: true,
        }
    }

    pub fn mark_chrome(&mut self) {
        self.composition.chrome = true;
        self.composition.sidebar = true;
        self.composition.overlay = true;
        self.cursor = true;
    }

    pub fn mark_editor_grid(&mut self) {
        self.composition.editor = true;
        self.cursor = true;
    }

    pub fn mark_terminal_grid(&mut self) {
        self.composition.drawer = true;
        self.cursor = true;
    }

    pub fn mark_layout(&mut self) {
        self.layout = true;
        self.composition = CompositionDamage::all();
        self.cursor = true;
    }

    pub fn has_damage(&self) -> bool {
        self.layout || self.composition.any() || self.cursor
    }

    pub fn consume_paint(&mut self) -> bool {
        let had = self.has_damage();
        self.layout = false;
        self.composition = CompositionDamage::default();
        self.cursor = false;
        had
    }
}
