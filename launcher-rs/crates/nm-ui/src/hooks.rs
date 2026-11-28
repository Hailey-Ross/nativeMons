//! TestHooks and UiFont from the C# launcher: fixed time, clock, battery, seed, bundled fonts, text mask.

use nm_draw::text::{Font, FontFace, StringAlignment, StringFormat};
use nm_draw::{Graphics, NetRandom, PointF, RectangleF};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

fn var(n: &str) -> Option<String> { std::env::var(n).ok() }

pub struct TestHooks {
    pub t: Option<f32>,
    pub battery: Option<String>,
    pub seed: Option<i32>,
    pub mask: Option<Vec<String>>,
    /// --replay is driving (TestHooks.Replay): Busy runs its work at once.
    pub replay: bool,
}

impl TestHooks {
    pub fn from_env() -> TestHooks {
        TestHooks {
            t: var("NATIVEMONS_TEST_T").and_then(|v| v.parse().ok()),
            battery: var("NATIVEMONS_TEST_BATTERY"),
            seed: var("NATIVEMONS_TEST_SEED").and_then(|v| v.parse().ok()),
            mask: None,
            replay: false,
        }
    }
    pub fn fixed_time(&self) -> bool { self.t.is_some() }
    /// Seconds since midnight of the wall clock: 09:41:00 + T when fixed.
    pub fn wall_seconds(&self) -> f64 {
        match self.t {
            Some(t) => 9.0 * 3600.0 + 41.0 * 60.0 + t as f64,
            None => {
                let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
                s % 86400.0
            }
        }
    }
    /// DateTime.ToString("h:mm").
    pub fn wall_hmm(&self) -> String {
        let s = self.wall_seconds() as i64;
        let (h, m) = ((s / 3600) % 24, (s / 60) % 60);
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{}:{:02}", h12, m)
    }
    pub fn new_random(&self) -> NetRandom { NetRandom::new(self.seed.unwrap_or(0)) }

    /// TestHooks.Text with no StringFormat ("Near Near None").
    pub fn text_plain(&mut self, g: &Graphics, s: &str, f: &Font, r: RectangleF) { self.text_line(g, s, f, r, "Near Near None".into()); }
    /// The text mask line for one Write: the layout rect's device box inside the visible clip.
    pub fn text(&mut self, g: &Graphics, s: &str, f: &Font, r: RectangleF, sf: &StringFormat) {
        let al = |a: StringAlignment| match a { StringAlignment::Near => "Near", StringAlignment::Center => "Center", StringAlignment::Far => "Far" };
        let flags = if sf.line_limit { "LineLimit" } else if sf.no_wrap { "NoWrap" } else { "0" };
        self.text_line(g, s, f, r, format!("{} {} {}", al(sf.alignment), al(sf.line_alignment), flags));
    }
    fn text_line(&mut self, g: &Graphics, s: &str, f: &Font, r: RectangleF, align: String) {
        let Some(mask) = self.mask.as_mut() else { return };
        if s.is_empty() { return; }
        let b = RectangleF::intersect(r, g.visible_clip_bounds());
        if b.width <= 0.0 || b.height <= 0.0 { return; }
        let pts = [PointF::new(b.x, b.y), PointF::new(b.right(), b.y), PointF::new(b.right(), b.bottom()), PointF::new(b.x, b.bottom())].map(|p| g.transform_point(p));
        let x0 = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min).floor() as i32;
        let y0 = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min).floor() as i32;
        let x1 = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max).ceil() as i32;
        let y1 = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max).ceil() as i32;
        let mut px = format!("{:.2}", f.size);
        while px.contains('.') && (px.ends_with('0') || px.ends_with('.')) { px.pop(); }
        mask.push(format!("{} {} {} {}\t{}\t{}\t{}\t{}", x0, y0, x1 - x0, y1 - y0, f.name(), px, align, s.replace('\t', " ").replace('\r', "").replace('\n', "\\n")));
    }
    pub fn save_mask(&mut self, picture: &std::path::Path) {
        let Some(mask) = self.mask.as_mut() else { return };
        let body: String = mask.iter().map(|l| format!("{}\r\n", l)).collect();
        let _ = std::fs::write(picture.with_extension("text.txt"), body);
        mask.clear();
    }
}

/// UiFont: the bundled Noto faces by the Windows names the launcher asks for (decided: Noto everywhere).
/// Segoe MDL2 Assets is our code-drawn icon set (nm_draw::icons): no Microsoft font on any platform.
pub struct UiFont {
    dir: PathBuf,
    faces: Mutex<HashMap<String, Option<Arc<FontFace>>>>,
}

impl UiFont {
    pub fn global() -> &'static UiFont {
        static U: OnceLock<UiFont> = OnceLock::new();
        U.get_or_init(|| {
            let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default();
            let dir = var("NATIVEMONS_FONTS_DIR").map(PathBuf::from).unwrap_or_else(|| exe.join("fonts"));
            // Noto Sans Symbols 2 fills in what Noto Sans lacks (▶ ◀ ★), as GDI+'s font linking does on Windows.
            if let Some(f) = FontFace::load(&dir.join("NotoSansSymbols2-Regular.ttf")) { nm_draw::text::add_fallback(f); }
            UiFont { dir, faces: Mutex::new(HashMap::new()) }
        })
    }
    fn file(name: &str) -> Option<&'static str> {
        Some(match name {
            "Segoe UI" => "NotoSans-Regular.ttf",
            "Segoe UI Semibold" => "NotoSans-SemiBold.ttf",
            "Segoe UI Black" => "NotoSans-Black.ttf",
            "Segoe UI Light" => "NotoSans-Light.ttf",
            "Consolas" => "NotoSansMono-Regular.ttf",
            _ => return None,
        })
    }
    fn face(&self, name: &str) -> Arc<FontFace> {
        let mut m = self.faces.lock().unwrap();
        if let Some(Some(f)) = m.get(name) { return f.clone(); }
        let f = match UiFont::file(name) {
            Some(file) => FontFace::load(&self.dir.join(file)).or_else(|| FontFace::load(&self.dir.join("NotoSans-Regular.ttf"))).expect("no usable font"),
            // Segoe MDL2 Assets: our code-drawn icons (nm_draw::icons), no font file.
            None => FontFace::icon_face(),
        };
        m.insert(name.to_string(), Some(f.clone()));
        f
    }
    pub fn get(name: &str, size: f32) -> Font { Font::new(&UiFont::global().face(name), size) }
}
