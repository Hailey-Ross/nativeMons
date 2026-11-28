//! Theme, Item and Page: the menu model, as in the C# launcher.

use nm_draw::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    pub main: Color,
    pub deep: Color,
    pub light: Color,
    pub ink: Color,
    pub finish: String,
    pub game_id: Option<String>,
}

impl Theme {
    pub fn plain(&self) -> bool { self.finish != "gold" && self.finish != "silver" && self.finish != "crystal" }
    pub fn on_cream(&self) -> Color {
        let d = self.deep;
        let lum = 0.299 * d.r as f64 + 0.587 * d.g as f64 + 0.114 * d.b as f64;
        if self.plain() && lum < 110.0 { d } else { Color::rgb(38, 40, 50) }
    }
    pub fn make(main: &str, deep: &str, light: &str, finish: Option<&str>) -> Theme {
        let mut t = Theme { main: Color::from_html(main), deep: Color::from_html(deep), light: Color::from_html(light), ink: Color::BLACK, finish: finish.unwrap_or("plain").to_string(), game_id: None };
        let lum = 0.299 * t.main.r as f64 + 0.587 * t.main.g as f64 + 0.114 * t.main.b as f64;
        t.ink = if lum > 150.0 { Color::rgb(28, 28, 34) } else { Color::rgb(252, 250, 244) };
        t
    }
    pub fn default_theme() -> Theme { Theme::make("#D6302B", "#8E1712", "#F2665C", Some("plain")) }
}

/// Item.Run and Page.OnBack: what selecting it does (a C# Action closure).
pub type RunFn = std::rc::Rc<dyn Fn(&mut crate::launcher::Launcher)>;
#[derive(Clone)]
pub struct Act(pub RunFn);
impl std::fmt::Debug for Act {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Act") }
}
impl Act {
    pub fn new(f: impl Fn(&mut crate::launcher::Launcher) + 'static) -> Act { Act(std::rc::Rc::new(f)) }
    pub fn call(&self, l: &mut crate::launcher::Launcher) { (self.0)(l) }
}
/// Item.Slide: left and right on a slider row, with the step to take.
#[derive(Clone)]
pub struct Slide(pub std::rc::Rc<dyn Fn(&mut crate::launcher::Launcher, i32)>);
impl std::fmt::Debug for Slide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Slide") }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub text: String,
    pub hint: String,
    pub on: bool,
    pub info: String,
    pub records: Vec<String>,
    pub icon: char,
    pub theme: Option<Theme>,
    pub game_id: Option<String>,
    pub dex_no: i32,
    pub dex_state: i32,
    pub dex_star: bool,
    pub region: i32,
    pub region_caught: i32,
    pub region_total: i32,
    pub sprite: Option<String>,
    pub tile_hint: String,
    pub fold: i32,
    pub tint: Color,
    pub is_slider: bool,
    pub slider: i32,
    pub slider_note: String,
    pub dex_entry: Option<std::rc::Rc<crate::dex::DexEntry>>,
    pub run: Option<Act>,
    pub slide: Option<Slide>,
}

impl Item {
    pub fn new(text: &str, hint: &str, on: bool) -> Item {
        Item {
            text: text.to_string(),
            hint: hint.to_string(),
            on,
            info: String::new(),
            records: Vec::new(),
            icon: '\u{E8FD}',
            theme: None,
            game_id: None,
            dex_no: 0,
            dex_state: 0,
            dex_star: false,
            region: -1,
            region_caught: 0,
            region_total: 0,
            sprite: None,
            tile_hint: String::new(),
            fold: 0,
            tint: Color::EMPTY,
            is_slider: false,
            slider: -1,
            slider_note: String::new(),
            dex_entry: None,
            run: None,
            slide: None,
        }
    }
    pub fn icon(mut self, c: char) -> Item {
        self.icon = c;
        self
    }
    pub fn info(mut self, s: &str) -> Item {
        self.info = s.to_string();
        self
    }
    pub fn tint(mut self, c: Color) -> Item {
        self.tint = c;
        self
    }
    pub fn run(mut self, f: impl Fn(&mut crate::launcher::Launcher) + 'static) -> Item {
        self.run = Some(Act::new(f));
        self
    }
    pub fn slide(mut self, f: impl Fn(&mut crate::launcher::Launcher, i32) + 'static) -> Item {
        self.slide = Some(Slide(std::rc::Rc::new(f)));
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub title: String,
    pub items: Vec<Item>,
    pub sel: i32,
    pub top_row: i32,
    pub scroll_row: f32,
    pub body: Option<String>,
    pub mono: Option<String>,
    pub body_scroll: f32,
    pub jump: i32,
    pub dex_id: i32,
    pub links: bool,
    pub fit_body: bool,
    pub mods: bool,
    pub main: bool,
    pub theme: Option<Theme>,
    pub game_id: Option<String>,
    pub step: i32,
    pub step_only: bool,
    pub gallery: bool,
    pub picture: bool,
    pub boxes: Option<crate::boxes::BoxView>,
    pub dex_shiny: bool,
    pub dex_height: f32,
    pub dex_entry: Option<std::rc::Rc<crate::dex::DexEntry>>,
    pub on_back: Option<Act>,
    pub dex_order: Vec<i32>,
    pub dex_only: Option<String>,
    /// The browser request this page belongs to (0: none).
    pub browse: u32,
    /// Find a wallpaper's results (gallery) or a preview's picture (picture).
    pub wall: Wall,
}

/// Page.Gallery and Page.Picture: the results shown as a grid, or the downloaded picture above the buttons.
#[derive(Clone, Default)]
pub struct Wall {
    pub results: std::rc::Rc<Vec<std::sync::Arc<crate::flows::WallResult>>>,
    pub picture: Option<std::sync::Arc<nm_draw::Bitmap>>,
}
impl std::fmt::Debug for Wall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Wall({} results, picture {})", self.results.len(), self.picture.is_some()) }
}

impl Page {
    pub fn new(title: &str) -> Page { Page { title: title.to_string(), scroll_row: -1.0, step: -1, ..Default::default() } }
    pub fn on_back(mut self, f: impl Fn(&mut crate::launcher::Launcher) + 'static) -> Page {
        self.on_back = Some(Act::new(f));
        self
    }
}
