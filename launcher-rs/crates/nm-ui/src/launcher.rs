//! LauncherForm's drawing, ported line by line: the device shell, the phone screen, pages, rows, the splash.

use crate::art::*;
use crate::hooks::{TestHooks, UiFont};
use crate::model::{Item, Page, Theme};
use nm_draw::text::{char_range_rects, Font, StringAlignment, StringFormat};
use nm_draw::*;

pub struct Fonts {
    pub mono: Font,
    pub title: Font,
    pub item: Font,
    pub small: Font,
    pub hero: Font,
    pub mark: Font,
    pub mark_light: Font,
    pub play: Font,
    pub icon: Font,
    pub icon_small: Font,
    pub item_bold: Font,
    pub small_bold: Font,
    pub body: Font,
    pub splash_mark: Font,
    pub splash_light: Font,
}

impl Fonts {
    /// MakeFonts.
    pub fn make(width: i32) -> Fonts {
        let h = 420f32.max(width as f32 * 1.15);
        Fonts {
            title: UiFont::get("Segoe UI Semibold", h / 24.0),
            hero: UiFont::get("Segoe UI Semibold", h / 17.0),
            item: UiFont::get("Segoe UI", h / 34.0),
            item_bold: UiFont::get("Segoe UI Semibold", h / 34.0),
            small: UiFont::get("Segoe UI", h / 52.0),
            mono: UiFont::get("Consolas", h / 50.0),
            small_bold: UiFont::get("Segoe UI Semibold", h / 52.0),
            body: UiFont::get("Segoe UI", h / 42.0),
            mark: UiFont::get("Segoe UI Black", h / 30.0),
            mark_light: UiFont::get("Segoe UI Light", h / 30.0),
            splash_mark: UiFont::get("Segoe UI Black", h / 11.0),
            splash_light: UiFont::get("Segoe UI Light", h / 11.0),
            play: UiFont::get("Segoe UI Black", h / 26.0),
            icon: UiFont::get("Segoe MDL2 Assets", h / 26.0),
            icon_small: UiFont::get("Segoe MDL2 Assets", h / 40.0),
        }
    }
}

/// What the PC around the launcher looks like (Screen.WorkingArea, PowerStatus): passed in by the platform.
#[derive(Clone, Copy)]
pub struct Host {
    pub work_area: (i32, i32),
    /// Light mode's automatic choice for this PC, and why (detected by the platform; off in snapshots).
    pub light_auto: bool,
    pub light_why: &'static str,
}
impl Host {
    pub fn new(work_area: (i32, i32)) -> Host { Host { work_area, light_auto: false, light_why: "a test run" } }
}

pub struct Launcher {
    pub hooks: TestHooks,
    pub fonts: Fonts,
    pub host: Host,
    pub pages: Vec<Page>,
    pub starting: bool,
    pub start_since_wall: f64,
    pub start_stage: Option<String>,
    pub job: bool,
    pub job_title: String,
    pub job_start_wall: f64,
    pub job_is_game: bool,
    pub progress: f32,
    pub log: Vec<String>,
    pub hover_prompt: String,
    pub screen_x: f32,
    pub screen_y: f32,
    pub phone_w: i32,
    pub tablet_k: f32,
    pub wide_window: bool,
    pub screen_key: String,
    pub screen_depth: i32,
    pub screen_dir: i32,
    pub sel_index: i32,
    pub screen_at: f32,
    pub sel_at: f32,
    pub sel_ease: f32,
    pub bounce_at: f32,
    pub open_at: f32,
    pub power_level: f32,
    pub frozen_t: f32,
    pub prompt_hits: Vec<(RectangleF, String)>,
    pub hits: Vec<(RectangleF, i32)>,
    pub scrolling: bool,
    /// Something drawn this frame keeps moving by itself (a loading ball, a dragged Pokemon): Poll redraws at half rate.
    pub spinning: bool,
    /// The side panel plays a game's intro: Poll keeps redrawing at its rate.
    pub panel_intro: bool,
    pub current_theme: Option<Theme>,
    pub ctx: Option<crate::pages::Ctx>,
    pub bounce_arm: bool,
    pub open_app: usize,
    pub open_from: RectangleF,
    pub open_run: Option<crate::model::Act>,
    pub power_off: f32,
    pub app_rects: [RectangleF; 9],
    pub banner_build: i32,
    pub banner_at: f32,
    pub banner_closed: Option<String>,
    pub hop_random: Option<NetRandom>,
    pub hops: Vec<HopMove>,
    pub hop_u: f32,
    pub shake_random: Option<NetRandom>,
    pub shake_starts: Vec<f32>,
    pub shake_u: f32,
    pub snapshot_anim: bool,
    pub emu_install_at: f32,
    pub emu_install_name: Option<String>,
    /// Find mods' view: one kind or all, most downloaded or A to Z (kept while the launcher runs).
    pub mod_kind: Option<String>,
    pub mod_alphabetical: bool,
    /// Recomp's banner state (BannerTitle, BannerVerb, QuietProgress, DoneAt, FakeRunning).
    pub recomp_banner: Option<RecompBanner>,
    pub improve_at: f32,
    pub improve_shown: f32,
    pub improve_last: f32,
    pub setup_at: f32,
    pub setup_from: Option<Page>,
    pub setup_dir: i32,
    pub clash_game: Option<usize>,
    pub clash_count: i32,
    pub clash_at: f32,
    /// Live input and page flow (input.rs).
    pub input: crate::input::InputState,
    /// What the platform must do (close, minimize, drag, open a URL, start a game...).
    pub effects: Vec<crate::input::Effect>,
    pub dirty: bool,
    /// C#'s dirty flag set beside an Invalidate: the input's own repaint doesn't clear it, the next Poll draws again.
    pub dirty_poll: bool,
    pub clock: Option<std::time::Instant>,
    pub closing: bool,
    pub powered_off: bool,
    /// Light mode (Rust only): animations at 30 fps, no idle full-rate redraws.
    pub light: bool,
    pub dex: Option<crate::dex::Dex>,
    pub morph: crate::morph::Morph,
    pub leaving: crate::setup::Leaving,
    /// RunJob, Busy and the play flows' state (play.rs).
    pub jobs: crate::play::Jobs,
}

#[derive(Clone, Debug)]
pub struct RecompBanner {
    pub title: String,
    pub verb: String,
    pub progress: f32,
    pub done_at_wall: f64,
    pub fake_running: bool,
    /// Light mode paused it while a game plays.
    pub paused: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct HopMove {
    pub start: f32,
    pub speed: f32,
    pub size: f32,
    pub kind: usize,
    pub pick: i32,
}

pub const CRT_TIME: f32 = 0.45;
pub const LIGHTS_AT: f32 = 0.1;
pub const OPEN_TIME: f32 = 0.3;
pub const LIGHTS_TIME: f32 = 0.3;
pub const BANNER_SECONDS: f32 = 8.0;
const SPUTTER: [f32; 9] = [0.15, 0.85, 0.05, 0.6, 0.2, 0.75, 0.08, 0.45, 0.3];
pub fn power_curve(x: f32) -> f32 {
    if x < 0.0 { return 1.0; }
    if x < 0.04 { return 1.25; }
    if x < 0.04 + SPUTTER.len() as f32 * 0.022 { return SPUTTER[((x - 0.04) / 0.022) as usize]; }
    0.4 * (-(x - 0.04 - SPUTTER.len() as f32 * 0.022) as f64 / 0.08).exp() as f32
}

pub fn write(hooks: &mut TestHooks, g: &mut Graphics, s: &str, f: &Font, c: Color, r: RectangleF, h: StringAlignment, v: StringAlignment, wrap: bool) {
    let sf = StringFormat { alignment: h, line_alignment: v, no_wrap: !wrap, line_limit: wrap, ellipsis: true };
    hooks.text(g, s, f, r, &sf);
    g.draw_string(s, f, c, r, &sf);
}

use StringAlignment::{Center, Far, Near};

pub fn accent(th: &Theme) -> Color {
    let c = th.main;
    if 0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64 > 150.0 { mix(c, Color::BLACK, 0.42) } else { c }
}

fn contrast(c: Color) -> f64 {
    let lin = |v: u8| {
        let x = v as f64 / 255.0;
        if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    };
    1.05 / (0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b) + 0.05)
}

fn from_hsl(hue: f32, sat: f32, light: f32) -> Color {
    let q = if light < 0.5 { light * (1.0 + sat) } else { light + sat - light * sat };
    let p = 2.0 * light - q;
    let part = |tt: f32| -> i32 {
        let tt = (tt % 1.0 + 1.0) % 1.0;
        let v = if tt < 1.0 / 6.0 { p + (q - p) * 6.0 * tt } else if tt < 0.5 { q } else if tt < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - tt) * 6.0 } else { p };
        (v.clamp(0.0, 1.0) as f64 * 255.0).round() as i32
    };
    let hh = hue / 360.0;
    Color::rgb(part(hh + 1.0 / 3.0), part(hh), part(hh - 1.0 / 3.0))
}

pub fn link_color(th: &Theme) -> Color {
    let c = th.main;
    let hue = if c.get_saturation() < 0.15 { 210.0 } else { (c.get_hue() + 180.0) % 360.0 };
    let sat = c.get_saturation().max(0.6);
    let mut light = 0.42f32;
    while light > 0.1 {
        let k = from_hsl(hue, sat, light);
        if contrast(k) >= 7.0 { return k; }
        light -= 0.01;
    }
    from_hsl(hue, sat, 0.1)
}

fn row_h_for(h: i32) -> f32 { h as f32 * 0.082 }
fn corner_radius(w: i32) -> f32 { w as f32 * 0.075 }
/// WideText: the wide window's text size, from the phone's.
pub(crate) fn wide_text(w: i32, phone: i32) -> i32 { (phone as f32 * (((w as f32 / phone as f32).sqrt() * 0.82).min(1.4)).max(1.0)) as i32 }

pub fn row_tint(it: &Item) -> Color {
    if !it.on { return TEXT3; }
    if !it.tint.is_empty() { return it.tint; }
    if let Some(c) = browse_tint(it.icon) { return c; }
    ROW_COLORS[it.icon as usize % ROW_COLORS.len()]
}

impl Launcher {
    pub fn new(w: i32, h: i32, host: Host) -> Launcher {
        let wide = w > h;
        let mut l = Launcher {
            hooks: TestHooks::from_env(),
            fonts: Fonts::make(w),
            host,
            pages: Vec::new(),
            starting: false,
            start_since_wall: 0.0,
            start_stage: None,
            job: false,
            job_title: String::new(),
            job_start_wall: 0.0,
            job_is_game: false,
            progress: -1.0,
            log: Vec::new(),
            hover_prompt: String::new(),
            screen_x: 0.0,
            screen_y: 0.0,
            phone_w: 0,
            tablet_k: 0.0,
            wide_window: false,
            screen_key: String::new(),
            screen_depth: 0,
            screen_dir: 1,
            sel_index: -1,
            screen_at: -1.0,
            sel_at: -1.0,
            sel_ease: 1.0,
            bounce_at: -10.0,
            open_at: -10.0,
            power_level: 1.0,
            frozen_t: -1.0,
            prompt_hits: Vec::new(),
            hits: Vec::new(),
            scrolling: false,
            spinning: false,
            panel_intro: false,
            current_theme: None,
            ctx: None,
            bounce_arm: false,
            open_app: 0,
            open_from: RectangleF::EMPTY,
            open_run: None,
            power_off: -1.0,
            app_rects: [RectangleF::EMPTY; 9],
            banner_build: 0,
            banner_at: 0.0,
            banner_closed: None,
            hop_random: None,
            hops: Vec::new(),
            hop_u: 0.0,
            shake_random: None,
            shake_starts: Vec::new(),
            shake_u: 0.0,
            snapshot_anim: false,
            emu_install_at: -10.0,
            emu_install_name: None,
            mod_kind: None,
            mod_alphabetical: false,
            recomp_banner: None,
            improve_at: f32::NAN,
            improve_shown: 0.0,
            improve_last: 0.0,
            setup_at: -10.0,
            setup_from: None,
            setup_dir: 1,
            clash_game: None,
            clash_count: 0,
            clash_at: f32::NAN,
            input: Default::default(),
            effects: Vec::new(),
            dirty: false,
            dirty_poll: false,
            clock: None,
            closing: false,
            powered_off: false,
            light: false,
            dex: None,
            morph: Default::default(),
            leaving: Default::default(),
            jobs: Default::default(),
        };
        if wide {
            l.phone_w = h * 60 / 100;
            l.tablet_k = 1.0;
            l.wide_window = true;
            l.fonts = Fonts::make(wide_text(w, l.phone_w));
        }
        l
    }
    /// Now(): the fixed test time, else seconds on the launcher's own clock.
    pub fn now(&self) -> f32 { self.hooks.t.unwrap_or_else(|| self.clock.map_or(0.0, |c| c.elapsed().as_secs_f32())) }
    fn tablet(&self) -> bool { self.tablet_k > 0.0 }
    fn tablet_ease(&self) -> f32 {
        let k = self.tablet_k;
        k * k * (3.0 - 2.0 * k)
    }
    fn phone_w_for(&self, h: i32) -> i32 { if self.phone_w > 0 { self.phone_w } else { h * 60 / 100 } }

    pub(crate) fn w(&mut self, g: &mut Graphics, s: &str, f: &Font, c: Color, r: RectangleF, h: StringAlignment, v: StringAlignment, wrap: bool) { write(&mut self.hooks, g, s, f, c, r, h, v, wrap); }

    /// Cur(): the highlighted game's colors, else the page's, else the selected game's.
    pub fn cur(&self) -> Theme {
        if !self.job {
            if let Some(p) = self.pages.last() {
                if !p.items.is_empty() {
                    if let Some(t) = &p.items[(p.sel as usize).min(p.items.len() - 1)].theme { return t.clone(); }
                }
                if let Some(t) = &p.theme { return t.clone(); }
            }
        }
        self.current_theme.clone().unwrap_or_else(Theme::default_theme)
    }

    pub fn screen_rect(&self, w: i32, h: i32) -> RectangleF {
        let pm = self.phone_w_for(h) as f32 * 0.06;
        let e = self.tablet_ease();
        RectangleF::new(pm, h as f32 * 0.115, w as f32 - 2.0 * pm, h as f32 * (0.665 + 0.18 * e))
    }

    pub fn render(&mut self, g: &mut Graphics, w: i32, h: i32) {
        g.smoothing_mode = SmoothingMode::AntiAlias;
        self.hits.clear();
        self.prompt_hits.clear();
        let th = self.cur();
        let t = self.now();
        g.clear(LINE);
        if self.tablet() { self.tablet_shell(g, w, h, &th, t); } else { self.shell(g, w, h, &th, t); }
        let s = self.screen_rect(w, h);
        self.screen_x = s.x;
        self.screen_y = s.y;
        let pw = if self.tablet() { self.phone_w_for(h) } else { w };
        let rad = pw as f32 * 0.03;
        let inset = pw as f32 * 0.016;
        let rim = inset * 0.45;
        let front = RectangleF::new(s.x - inset, s.y - inset, s.width + 2.0 * inset, s.height + 2.0 * inset);
        let recess = RectangleF::new(front.x - rim, front.y - rim, front.width + 2.0 * rim, front.height + 2.0 * rim);
        g.fill_path(&lin(recess, mix(th.deep, Color::BLACK, 0.55), mix(th.main, Color::WHITE, 0.25), 90.0), &round(recess, rad * 1.6));
        g.fill_path(&solid(Color::rgb(12, 12, 14)), &round(front, rad * 1.35));
        let outer = g.save();
        g.set_clip_path(&round(s, rad));
        g.translate_transform(s.x, s.y);
        let (sw, sh) = (s.width as i32, s.height as i32);
        let crt = if self.power_off < 0.0 { -1.0 } else { (t - self.power_off) / CRT_TIME };
        if crt >= 0.0 {
            // An old tube switching off: the picture squeezes to a bright line, the line to a dot, and the dot blips out.
            let (swf, shf) = (sw as f32, sh as f32);
            g.fill_rectangle(&solid(Color::rgb(8, 8, 10)), RectangleF::new(0.0, 0.0, swf, shf));
            let squash = ease((crt / 0.45).min(1.0));
            let narrow = ease(((crt - 0.45) / 0.3).min(1.0).max(0.0));
            let line_h = (shf * (1.0 - squash)).max(2.0);
            let line_w = (swf * (1.0 - narrow)).max(3.0);
            if crt < 0.75 {
                let kept = g.save();
                g.translate_transform(swf / 2.0, shf / 2.0);
                g.scale_transform(line_w / swf, line_h / shf);
                g.translate_transform(-swf / 2.0, -shf / 2.0);
                if squash < 0.97 { self.render_screen(g, sw, sh, &th, t); }
                g.restore(kept);
                g.fill_rectangle(&solid(Color::from_argb((255.0 * (squash * 1.1).min(1.0)) as i32, 235, 245, 255)), RectangleF::new((swf - line_w) / 2.0, (shf - line_h) / 2.0, line_w, line_h));
            }
            let dot = if crt < 0.75 { 0.0 } else { 1.0 - ((crt - 0.75) / 0.25).min(1.0) };
            if dot > 0.0 {
                let rr = (swf * 0.018 * (0.4 + 0.6 * dot)).max(2.0);
                g.fill_ellipse(&solid(Color::from_argb((90.0 * dot) as i32, 200, 225, 255)), RectangleF::new(swf / 2.0 - rr * 2.4, shf / 2.0 - rr * 2.4, rr * 4.8, rr * 4.8));
                g.fill_ellipse(&solid(Color::from_argb((255.0 * dot) as i32, 245, 250, 255)), RectangleF::new(swf / 2.0 - rr, shf / 2.0 - rr, rr * 2.0, rr * 2.0));
            }
        } else if self.starting && !self.job && self.pages.is_empty() { self.render_splash(g, sw, sh); } else { self.render_screen(g, sw, sh, &th, t); }
        g.restore(outer);
        let kept = g.save();
        g.set_clip_path(&round(s, rad));
        g.fill_polygon(&solid(Color::from_argb(9, 255, 255, 255)), &[PointF::new(s.x, s.y), PointF::new(s.x + s.width * 0.55, s.y), PointF::new(s.x, s.y + s.height * 0.42)]);
        g.restore(kept);
    }

    fn shell(&mut self, g: &mut Graphics, w: i32, h: i32, th: &Theme, t: f32) {
        let (wf, hf) = (w as f32, h as f32);
        let corner = corner_radius(w);
        let edge = (wf * 0.008).max(3.0);
        let body = RectangleF::new(0.0, 0.0, wf, hf);
        g.fill_path(&lin(body, mix(th.main, Color::WHITE, 0.1), mix(th.main, th.deep, 0.75), 78.0), &round(body, corner));
        let lip = RectangleF::new(edge, edge, wf - 2.0 * edge, hf - 2.0 * edge);
        g.draw_path(&Pen::new(fade(Color::WHITE, 70), (wf * 0.003).max(1.5)), &round(lip, corner - edge));
        g.draw_path(&Pen::new(mix(th.deep, Color::BLACK, 0.45), edge), &round(RectangleF::new(1.0, 1.0, wf - 2.0, hf - 2.0), corner));
        let fold = hf * 0.1;
        g.draw_line_f(&Pen::new(fade(Color::BLACK, 70), (hf * 0.002).max(1.5)), edge, fold, wf - edge, fold);
        let d2 = (hf * 0.002).max(2.0);
        g.draw_line_f(&Pen::new(fade(Color::WHITE, 60), (hf * 0.0012).max(1.0)), edge, fold + d2, wf - edge, fold + d2);
        let knuckle = hf * 0.032;
        let kw = wf * 0.012;
        let ky0 = fold - knuckle * 0.5;
        card(g, RectangleF::new(0.0, ky0, kw, knuckle), kw * 0.3, mix(th.deep, Color::WHITE, 0.2), mix(th.deep, Color::BLACK, 0.35), Color::EMPTY, 0.0);
        let lens_r = hf * 0.038;
        let top_y = hf * 0.056;
        self.lens(g, wf * 0.06 + lens_r * 1.14, top_y, lens_r, t, true);
        let native = g.measure_string("NATIVE", &self.fonts.mark);
        let wx = wf * 0.06 + lens_r * 2.9 + lens_r * 2.2;
        let wy = top_y - native.height * 0.1;
        let (mark, light) = (self.fonts.mark.clone(), self.fonts.mark_light.clone());
        self.w(g, "NATIVE", &mark, th.ink, RectangleF::new(wx, wy, native.width + 8.0, native.height), Near, Near, false);
        self.w(g, "MONS", &light, th.ink, RectangleF::new(wx + native.width * 0.92, wy, wf * 0.3, native.height), Near, Near, false);
        let key = hf * 0.022;
        let ky = hf * 0.02;
        let kx = wf - wf * 0.05 - key;
        self.hard_key(g, RectangleF::new(kx, ky, key, key), "X", "\u{E711}");
        self.hard_key(g, RectangleF::new(kx - key * 1.5, ky, key, key), "M", "\u{E921}");
        self.shell_controls(g, w, h, th);
    }

    /// TabletShell: the case at the wide window's size, sized by the phone's width.
    fn tablet_shell(&mut self, g: &mut Graphics, w: i32, h: i32, th: &Theme, t: f32) {
        let (wf, hf) = (w as f32, h as f32);
        let pw = self.phone_w_for(h);
        let pwf = pw as f32;
        let corner = corner_radius(pw);
        let edge = (pwf * 0.008).max(3.0);
        let body = RectangleF::new(0.0, 0.0, wf, hf);
        g.fill_path(&lin(body, mix(th.main, Color::WHITE, 0.1), mix(th.main, th.deep, 0.75), 78.0), &round(body, corner));
        let lip = RectangleF::new(edge, edge, wf - 2.0 * edge, hf - 2.0 * edge);
        g.draw_path(&Pen::new(fade(Color::WHITE, 70), (pwf * 0.003).max(1.5)), &round(lip, corner - edge));
        g.draw_path(&Pen::new(mix(th.deep, Color::BLACK, 0.45), edge), &round(RectangleF::new(1.0, 1.0, wf - 2.0, hf - 2.0), corner));
        let fold = hf * 0.1;
        g.draw_line_f(&Pen::new(fade(Color::BLACK, 70), (hf * 0.002).max(1.5)), edge, fold, wf - edge, fold);
        let d2 = (hf * 0.002).max(2.0);
        g.draw_line_f(&Pen::new(fade(Color::WHITE, 60), (hf * 0.0012).max(1.0)), edge, fold + d2, wf - edge, fold + d2);
        let lens_r = hf * 0.038;
        let top_y = hf * 0.056;
        self.lens(g, pwf * 0.06 + lens_r * 1.14, top_y, lens_r, t, true);
        let native = g.measure_string("NATIVE", &self.fonts.mark);
        let wx = pwf * 0.06 + lens_r * 2.9 + lens_r * 2.2;
        let wy = top_y - native.height * 0.1;
        let (mark, light) = (self.fonts.mark.clone(), self.fonts.mark_light.clone());
        self.w(g, "NATIVE", &mark, th.ink, RectangleF::new(wx, wy, native.width + 8.0, native.height), Near, Near, false);
        self.w(g, "MONS", &light, th.ink, RectangleF::new(wx + native.width * 0.92, wy, pwf * 0.3, native.height), Near, Near, false);
        let key = hf * 0.022;
        let ky = hf * 0.02;
        let kx = wf - pwf * 0.05 - key;
        self.hard_key(g, RectangleF::new(kx, ky, key, key), "X", "\u{E711}");
        self.hard_key(g, RectangleF::new(kx - key * 1.5, ky, key, key), "M", "\u{E921}");
        // While it unfolds, the phone's controls slide down out of the case.
        let e = self.tablet_ease();
        if e < 1.0 {
            let st = g.save();
            g.set_clip_path(&round(lip, corner - edge));
            g.translate_transform((wf - pwf) / 2.0, hf * 0.3 * e);
            self.shell_controls(g, pw, h, th);
            g.restore(st);
        }
    }

    /// DetailPanel: the wide layout's right side, the chosen option large on a band of its color.
    fn detail_panel(&mut self, g: &mut Graphics, it: &Item, game_id: Option<&str>, r: RectangleF, h: i32, accent_c: Color, t: f32) {
        let hf = h as f32;
        let rad = r.width * 0.045;
        let band_h = r.height * if it.dex_no > 0 { 0.46 } else { 0.36 };
        g.fill_path(&solid(PHONE_ROW), &round(r, rad));
        let band = RectangleF::new(r.x, r.y, r.width, band_h);
        let tint = if it.region >= 0 { crate::dex::region_theme(it.region).main } else if it.theme.is_some() { it.theme.as_ref().unwrap().main } else if it.sprite.is_some() || it.fold != 0 { accent_c } else { row_tint(it) };
        {
            let st = g.save();
            g.set_clip_path(&round(r, rad));
            g.fill_rectangle(&lin(RectangleF::new(band.x, band.y - 1.0, band.width, band.height + 2.0), mix(tint, Color::WHITE, 0.3), mix(tint, Color::BLACK, 0.12), 90.0), band);
            if it.region >= 0 { crate::dex::motif(g, band, it.region, Color::from_argb(46, 255, 255, 255)); } else {
                let b = solid(Color::from_argb(34, 255, 255, 255));
                g.fill_ellipse(&b, RectangleF::new(band.right() - band_h * 0.95, band.y - band_h * 0.35, band_h * 1.25, band_h * 1.25));
                g.fill_ellipse(&b, RectangleF::new(band.x - band_h * 0.35, band.bottom() - band_h * 0.55, band_h * 0.85, band_h * 0.85));
            }
            // A game: its title screen intro in the band, in place of the icon.
            if let Some(intro) = game_id.and_then(|id| crate::game_art::frame(id, t)) {
                g.set_clip_rect(band);
                crate::game_art::draw_fitted(g, &intro, band, true);
                self.panel_intro = true;
            }
            g.restore(st);
        }
        let icon = band_h * 0.56;
        let ib = RectangleF::new(band.x + (band.width - icon) / 2.0, band.y + (band_h - icon) / 2.0, icon, icon);
        if it.region_total > 0 {
            // A region: how much of it is caught, as a ring.
            let ring = band_h * 0.62;
            let rw = (ring * 0.1).max(6.0);
            let rr = RectangleF::new(band.x + (band.width - ring) / 2.0, band.y + (band_h - ring) / 2.0, ring, ring);
            let mode = g.smoothing_mode;
            g.smoothing_mode = SmoothingMode::AntiAlias;
            g.draw_ellipse(&Pen::new(Color::from_argb(70, 0, 0, 0), rw), rr);
            let k = it.region_caught as f32 / it.region_total as f32;
            if k > 0.0 { g.draw_arc(&Pen::new(Color::WHITE, rw).round(), rr, -90.0, (360.0 * k).max(2.0)); }
            g.smoothing_mode = mode;
            let tf = self.fonts.title.clone();
            self.w(g, &format!("{}%", ((k * 100.0) as f64).round_ties_even() as i32), &tf, Color::WHITE, rr, Center, Center, false);
        } else if it.dex_no > 0 {
            // As big as the band allows, pixels kept sharp; a shiny one sparkles.
            let size = band_h * 0.98;
            let at = RectangleF::new(band.x + (band.width - size) / 2.0, band.y + (band_h - size) / 2.0, size, size);
            match it.sprite.as_deref().and_then(crate::sprites::get) {
                Some(sp) => {
                    g.fill_ellipse(&solid(Color::from_argb(40, 0, 0, 0)), RectangleF::new(at.x + size * 0.2, at.bottom() - size * 0.2, size * 0.6, size * 0.1));
                    draw_sprite(g, &sp, at);
                    if it.dex_star { sparkle(g, PointF::new(at.right() - size * 0.18, at.y + size * 0.2), size * 0.07); sparkle(g, PointF::new(at.x + size * 0.2, at.y + size * 0.34), size * 0.045); }
                }
                None if crate::sprites::loading(it.sprite.as_deref()) => { let t = self.now(); spinning_ball(g, at.x + size / 2.0, at.y + size / 2.0, size * 0.3, t, false); self.spinning = true; }
                None => {
                    // Not seen: an empty ball's outline, as the games show nothing yet.
                    let pen = Pen::new(Color::from_argb(110, 255, 255, 255), (size * 0.02).max(2.0));
                    let (br, cx, cy) = (size * 0.2, at.x + size / 2.0, at.y + size / 2.0);
                    g.draw_ellipse(&pen, RectangleF::new(cx - br, cy - br, br * 2.0, br * 2.0));
                    g.draw_line_f(&pen, cx - br, cy, cx + br, cy);
                    g.draw_ellipse(&pen, RectangleF::new(cx - br * 0.3, cy - br * 0.3, br * 0.6, br * 0.6));
                }
            }
        } else {
            if it.sprite.is_none() && it.theme.is_none() {
                g.fill_path(&solid(Color::from_argb(50, 0, 0, 0)), &round(RectangleF::new(ib.x, ib.y + icon * 0.06, icon, icon), icon * 0.24));
            }
            let glyph = UiFont::get("Segoe MDL2 Assets", icon * 0.45);
            self.row_icon(g, it, ib, &glyph);
        }
        let pad = r.width * 0.06;
        let mut y = band.bottom() + pad * 0.6;
        let tw = r.width - 2.0 * pad;
        let x = r.x + pad;
        let mut title = it.text.replace("Pokemon ", "");
        let mut value: Option<String> = None;
        if let Some(colon) = title.find(": ").filter(|&c| c > 0) {
            value = Some(title[colon + 2..].to_string());
            title.truncate(colon);
        }
        if it.dex_no > 0 { self.dex_panel_name(g, it, &title, x, y, tw, r.bottom() - pad * 0.5); return; }
        let tf = self.fonts.title.clone();
        let tfh = tf.height() as f32;
        self.w(g, &title, &tf, if it.on { PHONE_TEXT } else { TEXT3 }, RectangleF::new(x, y, tw, tfh * 1.3), Near, Center, false);
        y += tfh * 1.35;
        if it.is_slider {
            let k = if it.slider < 0 { 0.5 } else { it.slider as f32 / 100.0 };
            let th = (hf * 0.014).max(8.0);
            let knob = th * 1.5;
            let track_w = tw * 0.72;
            let cy = y + knob + 2.0;
            g.fill_path(&solid(Color::rgb(222, 222, 230)), &round(RectangleF::new(x, cy - th / 2.0, track_w, th), th / 2.0));
            if it.slider > 0 { g.fill_path(&solid(accent_c), &round(RectangleF::new(x, cy - th / 2.0, th.max(track_w * k), th), th / 2.0)); }
            let kx = x + track_w * k;
            g.fill_ellipse(&solid(Color::from_argb(40, 0, 0, 0)), RectangleF::new(kx - knob, cy - knob + knob * 0.15, knob * 2.0, knob * 2.0));
            g.fill_ellipse(&solid(Color::WHITE), RectangleF::new(kx - knob, cy - knob, knob * 2.0, knob * 2.0));
            let mut pen = Pen::new(if it.slider < 0 { Color::rgb(196, 196, 204) } else { accent_c }, (knob * 0.12).max(2.0));
            if it.slider < 0 { pen.dash_style = DashStyle::Dot; }
            g.draw_ellipse(&pen, RectangleF::new(kx - knob, cy - knob, knob * 2.0, knob * 2.0));
            let v = if it.slider < 0 { "Own".to_string() } else if it.slider == 0 { "Off".to_string() } else { format!("{}%", it.slider) };
            self.w(g, &v, &tf, accent_c, RectangleF::new(x + track_w + pad * 0.5, cy - tfh, tw - track_w - pad * 0.5, tfh * 2.0), Near, Center, false);
            y = cy + knob + pad * 0.5;
            if !it.slider_note.is_empty() {
                let sf = self.fonts.small.clone();
                let sh = sf.height() as f32;
                self.w(g, &it.slider_note, &sf, PHONE_SUB, RectangleF::new(x, y, tw, sh * 1.3), Near, Near, false);
                y += sh * 1.5;
            }
        } else if let Some(v) = value {
            let ib2 = self.fonts.item_bold.clone();
            let ih = ib2.height() as f32;
            self.w(g, &v, &ib2, accent_c, RectangleF::new(x, y, tw, ih * 1.3), Near, Center, false);
            y += ih * 1.5;
        }
        let info = if !it.info.is_empty() { it.info.clone() } else if !it.hint.is_empty() { it.hint.clone() } else { it.tile_hint.clone() };
        let room = r.bottom() - pad * 0.6 - y;
        let bf = self.fonts.body.clone();
        if !info.is_empty() && room > bf.height() as f32 {
            let ih = room.min(g.measure_string_w(&info, &bf, tw as i32).height + 2.0);
            self.w(g, &info, &bf, PHONE_SUB, RectangleF::new(x, y, tw, ih), Near, Near, true);
            y += ih + pad * 0.3;
        }
        let small = self.fonts.small.clone();
        for rec in &it.records {
            let rec = rec.replace(": ", ":  ");
            let rh = g.measure_string_w(&rec, &small, tw as i32).height;
            if y + rh > r.bottom() - pad * 0.5 { break; }
            self.w(g, &rec, &small, PHONE_SUB, RectangleF::new(x, y, tw, rh + 1.0), Near, Near, true);
            y += rh + hf * 0.002;
        }
    }

    fn shell_controls(&mut self, g: &mut Graphics, w: i32, h: i32, th: &Theme) {
        let (wf, hf) = (w as f32, h as f32);
        let cy = hf * 0.888;
        let d = hf * 0.05;
        let cx = wf * 0.2;
        g.fill_ellipse(&solid(fade(Color::BLACK, 70)), RectangleF::new(cx - d * 1.75, cy - d * 1.75, d * 3.5, d * 3.5));
        let arm = d * 0.62;
        let a = RectangleF::new(cx - arm, cy - d * 1.5, arm * 2.0, d * 3.0);
        let b = RectangleF::new(cx - d * 1.5, cy - arm, d * 3.0, arm * 2.0);
        let r = a.width * 0.18;
        g.fill_path(&solid(LINE), &round(a, r));
        g.fill_path(&solid(LINE), &round(b, r));
        self.dpad_hit(RectangleF::new(cx - arm, cy - d * 1.5, arm * 2.0, d), "U", g, cx, cy - d, 0.0);
        self.dpad_hit(RectangleF::new(cx - arm, cy + d * 0.5, arm * 2.0, d), "D", g, cx, cy + d, 180.0);
        self.dpad_hit(RectangleF::new(cx - d * 1.5, cy - arm, d, arm * 2.0), "L", g, cx - d, cy, 270.0);
        self.dpad_hit(RectangleF::new(cx + d * 0.5, cy - arm, d, arm * 2.0), "R", g, cx + d, cy, 90.0);
        let hole = (hf * 0.0034).max(2.0);
        let gap = hole * 2.3;
        let dark = solid(mix(th.deep, Color::BLACK, 0.6));
        let lightb = solid(fade(Color::WHITE, 60));
        for row in 0..5 {
            for col in 0..7 {
                let hx = wf * 0.42 + col as f32 * gap + (row % 2) as f32 * gap / 2.0;
                let hy = cy - gap * 2.0 + row as f32 * gap;
                g.fill_ellipse(&lightb, RectangleF::new(hx, hy + 1.0, hole, hole));
                g.fill_ellipse(&dark, RectangleF::new(hx, hy, hole, hole));
            }
        }
        let rb = hf * 0.038;
        self.round_key(g, wf * 0.86, cy - rb * 0.75, rb, "A", Color::rgb(108, 196, 74));
        self.round_key(g, wf * 0.70, cy + rb * 0.55, rb, "B", Color::rgb(228, 76, 70));
    }

    fn dpad_hit(&mut self, area: RectangleF, key: &str, g: &mut Graphics, cx: f32, cy: f32, angle: f32) {
        self.prompt_hits.push((area, key.to_string()));
        let s = if area.width < area.height { area.width * 0.22 } else { area.height * 0.22 };
        let st = g.save();
        g.translate_transform(cx, cy);
        g.rotate_transform(angle);
        let c = if self.hover_prompt == key { CREAM } else { fade(CREAM, 130) };
        g.fill_polygon(&solid(c), &[PointF::new(0.0, -s), PointF::new(s, s * 0.6), PointF::new(-s, s * 0.6)]);
        g.restore(st);
    }

    fn round_key(&mut self, g: &mut Graphics, cx: f32, cy: f32, r: f32, key: &str, c: Color) {
        let area = RectangleF::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.prompt_hits.push((area, key.to_string()));
        g.fill_ellipse(&solid(LINE), RectangleF::new(cx - r * 1.1, cy - r * 1.06, r * 2.2, r * 2.26));
        let hov = self.hover_prompt == key;
        g.fill_ellipse(&lin(area, mix(c, Color::WHITE, if hov { 0.45 } else { 0.2 }), mix(c, Color::BLACK, 0.18), 90.0), area);
        let f = self.fonts.item_bold.clone();
        self.w(g, key, &f, Color::rgb(16, 18, 24), area, Center, Center, false);
    }

    fn hard_key(&mut self, g: &mut Graphics, r: RectangleF, key: &str, glyph: &str) {
        self.prompt_hits.push((r, key.to_string()));
        let hov = self.hover_prompt == key;
        card(g, r, r.height * 0.3, if hov { CREAM } else { mix(LINE, Color::WHITE, 0.2) }, if hov { CREAM } else { LINE }, Color::EMPTY, 0.0);
        let f = UiFont::get("Segoe MDL2 Assets", r.height * 0.46);
        self.w(g, glyph, &f, if hov { LINE } else { CREAM }, r, Center, Center, false);
    }

    fn powered(&self, c: Color, off: Color) -> Color {
        let l = self.power_level;
        if l >= 1.0 { return mix(c, Color::WHITE, ((l - 1.0) * 2.0).min(1.0)); }
        mix(off, mix(c, Color::rgb(255, 120, 40), (1.0 - l) * 0.5), l)
    }

    fn lens(&mut self, g: &mut Graphics, cx: f32, cy: f32, r: f32, t: f32, lights: bool) {
        g.fill_ellipse(&solid(LINE), RectangleF::new(cx - r * 1.14, cy - r * 1.14, r * 2.28, r * 2.28));
        g.fill_ellipse(&solid(Color::rgb(240, 242, 246)), RectangleF::new(cx - r, cy - r, r * 2.0, r * 2.0));
        let i = r * 0.76;
        let rect = RectangleF::new(cx - i, cy - i, i * 2.0, i * 2.0);
        let t = if self.frozen_t >= 0.0 { self.frozen_t } else { t };
        let pulse = 0.5 + 0.5 * ((t * 1.8) as f64).sin() as f32;
        let c1 = self.powered(mix(Color::rgb(70, 170, 245), Color::rgb(130, 215, 255), pulse), Color::rgb(14, 30, 60));
        let c2 = self.powered(Color::rgb(16, 74, 160), Color::rgb(6, 14, 30));
        g.fill_ellipse(&lin(rect, c1, c2, 60.0), rect);
        g.fill_ellipse(&solid(fade(Color::WHITE, 200)), RectangleF::new(cx - i * 0.58, cy - i * 0.62, i * 0.55, i * 0.42));
        if !lights { return; }
        let d = r * 0.42;
        let mut x = cx + r * 1.5;
        let y = cy - r * 0.86;
        let on = [Color::rgb(240, 70, 60), Color::rgb(250, 205, 50), Color::rgb(90, 205, 90)];
        for k in 0..3 {
            let lit = ((t * 1.5) as i32) % 3;
            g.fill_ellipse(&solid(LINE), RectangleF::new(x - 1.5, y - 1.5, d + 3.0, d + 3.0));
            let off = mix(on[k], Color::BLACK, 0.7);
            let c = if k as i32 == lit { self.powered(on[k], off) } else { mix(off, mix(on[k], LINE, 0.45), self.power_level.min(1.0)) };
            g.fill_ellipse(&solid(c), RectangleF::new(x, y, d, d));
            x += d * 1.45;
        }
    }

    fn render_screen(&mut self, g: &mut Graphics, w: i32, h: i32, th: &Theme, t: f32) {
        let (wf, hf) = (w as f32, h as f32);
        let home = !self.job && self.pages.last().map_or(false, |p| p.main);
        let all = RectangleF::new(0.0, 0.0, wf, hf);
        let m = wf * 0.045;
        let top = hf * 0.055;
        let bottom = hf * 0.965;
        if home {
            crate::home::background(self, g, w, h, m, top, bottom, th, t, all);
        } else {
            g.fill_rectangle(&solid(PHONE_BG), all);
        }
        let key = if self.job { "job".to_string() } else { format!("{}|{}", self.pages.len(), self.pages.last().map_or(String::new(), |p| p.title.clone())) };
        if key != self.screen_key {
            let depth = self.pages.len() as i32 + if self.job { 1 } else { 0 };
            self.screen_dir = if depth >= self.screen_depth { 1 } else { -1 };
            self.screen_key = key;
            self.screen_depth = depth;
            self.screen_at = t;
            if self.bounce_arm {
                self.bounce_arm = false;
                self.bounce_at = t;
            }
            // Leaving the mods pages: what was switched there may have started or ended a clash.
            if !self.snapshot_anim { self.mods_left_check(); }
        }
        let setup = !self.job && self.pages.last().map_or(false, |p| p.step >= 0);
        let ease_v = ease((t - self.screen_at) / 0.18);
        let content = g.save();
        if !setup { g.translate_transform(self.screen_dir as f32 * (1.0 - ease_v) * wf * 0.04, 0.0); }
        if self.job {
            self.render_job(g, w, h, m, top, bottom, t, th);
        } else if setup {
            let p = self.pages.last().unwrap().clone();
            if p.sel != self.sel_index {
                self.sel_index = p.sel;
                self.sel_at = t;
            }
            self.sel_ease = ease((t - self.sel_at) / 0.12);
            self.render_setup_flow(g, &p, w, h, t, th);
        } else if !self.pages.is_empty() {
            let sel = self.pages.last().unwrap().sel;
            if sel != self.sel_index {
                self.sel_index = sel;
                self.sel_at = t;
            }
            self.sel_ease = ease((t - self.sel_at) / 0.12);
            let mut p = self.pages.pop().unwrap();
            if p.gallery { self.render_gallery(g, &mut p, w, h, m, top, bottom, t, th); }
            else if p.picture { self.render_preview(g, &mut p, w, h, m, top, bottom, t, th); }
            else if p.main { crate::home::render_main(self, g, &mut p, w, h, m, top, bottom, t, th); } else { self.render_page(g, &mut p, w, h, m, top, bottom, t, th); }
            self.pages.push(p);
        }
        g.restore(content);
        self.render_setup_leaving(g, w, h, t, th);
        self.render_emu_install(g, w, h, t);
        self.render_open(g, w, h, t);
        self.status_bar(g, w, h, if home { Color::WHITE } else { PHONE_TEXT });
        let below = self.clash_banner(g, w, h, t);
        self.update_banner(g, w, h, t, below);
        let top = (h as f32 * 0.052).max(below) + if self.banner_age(t) >= 0.0 { h as f32 * 0.068 } else { 0.0 };
        self.improve_banner(g, w, h, t, top);
        let bar_w = wf * 0.34;
        let bar_h = (hf * 0.0065).max(3.0);
        g.fill_path(&solid(if home { Color::WHITE } else { PHONE_TEXT }), &round(RectangleF::new((wf - bar_w) / 2.0, hf - bar_h * 2.4, bar_w, bar_h), bar_h / 2.0));
    }

    fn render_job(&mut self, g: &mut Graphics, w: i32, h: i32, m: f32, top: f32, bottom: f32, t: f32, th: &Theme) {
        let (wf, hf) = (w as f32, h as f32);
        let accent_c = accent(th);
        let mut y = top + hf * 0.1;
        let cart_len = wf * 0.22;
        spinning_ball(g, wf / 2.0, y + cart_len * 0.47, cart_len * 0.9, t, false);
        y += cart_len * 1.12;
        let title_f = self.fonts.title.clone();
        let ta = SizeF::new(wf - 2.0 * m, title_f.height() as f32 * 1.3);
        let job_title = self.job_title.clone();
        let tf = self.fit_font(g, &job_title, ta, &title_f, 0.6, false);
        self.w(g, &job_title, &tf, PHONE_TEXT, RectangleF::new(m, y, ta.width, ta.height), Center, Center, false);
        y += ta.height + hf * 0.018;
        let track = RectangleF::new(wf * 0.2, y, wf * 0.6, (hf * 0.007).max(4.0));
        card(g, track, track.height / 2.0, PHONE_LINE, PHONE_LINE, Color::EMPTY, 0.0);
        if self.progress >= 0.0 {
            card(g, RectangleF::new(track.x, track.y, track.height.max(track.width * self.progress.min(1.0)), track.height), track.height / 2.0, accent_c, accent_c, Color::EMPTY, 0.0);
        } else {
            let run = track.width * 0.3;
            let at = (t * 0.6 % 1.0) * (track.width + run) - run;
            let clip = g.save();
            g.set_clip_path(&round(track, track.height / 2.0));
            card(g, RectangleF::new(track.x + at, track.y, run, track.height), track.height / 2.0, accent_c, accent_c, Color::EMPTY, 0.0);
            g.restore(clip);
        }
        y = track.bottom() + hf * 0.018;
        let el = (self.hooks.wall_seconds() - self.job_start_wall).max(0.0);
        let lines: Vec<String> = self.log.iter().rev().take(9).cloned().collect();
        let small = self.fonts.small.clone();
        let sh = small.height() as f32;
        let first = lines.first().cloned().unwrap_or_else(|| if self.job_is_game { "Starting".into() } else { "One moment".into() });
        self.w(g, &first, &small, PHONE_SUB, RectangleF::new(m, y, wf - 2.0 * m, sh * 1.3), Center, Center, false);
        y += sh * 1.4;
        let clock = format!("{}:{:02}", (el / 60.0) as i64, (el as i64) % 60);
        self.w(g, &clock, &small, PHONE_SUB, RectangleF::new(m, y, wf - 2.0 * m, sh * 1.3), Center, Center, false);
        let lh = sh * 1.2;
        let mut yy = bottom - lh * 1.5;
        let floor_y = y + lh * 2.0;
        let mut i = 1;
        while i < lines.len() && yy >= floor_y {
            self.w(g, &lines[i], &small, fade(PHONE_SUB, 170), RectangleF::new(m, yy, wf - 2.0 * m, lh), Center, Center, false);
            yy -= lh;
            i += 1;
        }
    }

    /// Over the emulator's page after an install: the Emu tile pops in, plays, then shrinks away.
    fn render_emu_install(&mut self, g: &mut Graphics, w: i32, h: i32, t: f32) {
        const INSTALL_TIME: f32 = 2.4;
        let a = t - self.emu_install_at;
        let out_time = 0.45;
        if a < 0.0 || a > INSTALL_TIME + out_time || self.pages.last().map_or(true, |p| p.main) { return; }
        self.spinning = true;
        let (wf, hf) = (w as f32, h as f32);
        let come = smooth(a / 0.3);
        let go = 1.0 - smooth((a - INSTALL_TIME) / out_time);
        g.fill_rectangle(&solid(Color::from_argb((140.0 * come * go) as i32, 0, 0, 0)), RectangleF::new(0.0, 0.0, wf, hf));
        let size = wf * 0.36 * (0.82 + 0.18 * come) * go;
        if size < 2.0 { return; }
        let r = RectangleF::new((wf - size) / 2.0, hf * 0.42 - size / 2.0, size, size);
        let c = APP_COLORS[app("Emu")];
        g.fill_path(&solid(Color::from_argb((70.0 * go) as i32, 0, 0, 0)), &round(RectangleF::new(r.x, r.y + hf * 0.008, r.width, r.height), size * 0.24));
        gloss_tile(g, r, size * 0.24, c);
        app_art(g, app("Emu"), r, c, -1.0, (a / INSTALL_TIME).min(1.0), 0.0);
        let words = smooth((a - 0.9) / 0.4) * go;
        if words > 0.01 {
            if let Some(name) = self.emu_install_name.clone() {
                let ib = self.fonts.item_bold.clone();
                let ih = ib.height() as f32;
                self.w(g, &format!("{} installed", name), &ib, Color::from_argb((255.0 * words) as i32, 255, 255, 255), RectangleF::new(0.0, r.bottom() + hf * 0.02, wf, ih * 1.6), Center, Near, false);
            }
        }
    }

    fn render_open(&mut self, g: &mut Graphics, w: i32, h: i32, t: f32) {
        let k = (t - self.open_at) / OPEN_TIME;
        if k < 0.0 || self.open_app == 0 || k > 1.0 + 0.25 / OPEN_TIME { return; }
        let all = RectangleF::new(0.0, 0.0, w as f32, h as f32);
        let e = ease(k.min(1.0));
        let fd = if k <= 1.0 { 1.0 } else { 1.0 - (k - 1.0) * OPEN_TIME / 0.25 };
        let of = self.open_from;
        let r = RectangleF::new(of.x + (all.x - of.x) * e, of.y + (all.y - of.y) * e, of.width + (all.width - of.width) * e, of.height + (all.height - of.height) * e);
        let c = APP_COLORS[self.open_app];
        if fd >= 1.0 { gloss_tile(g, r, of.width * 0.24 * (1.0 - e), c); } else { g.fill_rectangle(&solid(Color::with_alpha((255.0 * fd.max(0.0)) as i32, c)), r); }
        if fd >= 1.0 {
            let size = of.width + ((w.min(h) as f32) * 0.32 - of.width) * e;
            app_art(g, self.open_app, RectangleF::new(r.x + (r.width - size) / 2.0, r.y + (r.height - size) / 2.0, size, size), c, -1.0, -1.0, 0.0);
        }
    }

    /// BallShake: a highlighted app's ball shakes as in a catch, on its own random rhythm.
    pub fn ball_shake(&mut self, u: f32) -> f32 {
        if u < 0.0 {
            self.shake_random = None;
            return 0.0;
        }
        if self.shake_random.is_none() || u < self.shake_u {
            self.shake_random = Some(self.hooks.new_random());
            self.shake_starts.clear();
            self.shake_starts.push(0.15);
        }
        self.shake_u = u;
        const SHAKE_LENGTH: f32 = 0.62;
        while self.shake_starts[self.shake_starts.len() - 1] + SHAKE_LENGTH < u {
            let last = self.shake_starts[self.shake_starts.len() - 1];
            let r = self.shake_random.as_mut().unwrap().next_double() as f32;
            self.shake_starts.push(last + SHAKE_LENGTH + 0.3 + r * 0.7);
        }
        let k = (u - self.shake_starts[self.shake_starts.len() - 1]) / SHAKE_LENGTH;
        if k <= 0.0 || k >= 1.0 { return 0.0; }
        24.0 * ((k as f64 * std::f64::consts::PI * 2.0).sin() * (k as f64 * std::f64::consts::PI).sin()) as f32
    }

    /// BannerAge: the update notice slides in and leaves after 8 s.
    fn banner_age(&mut self, t: f32) -> f32 {
        if !nm_core::updater::ready() || self.job || self.starting { return -1.0; }
        if self.banner_closed.is_none() { self.banner_closed = Some(nm_core::config::read("update-dismissed.txt", "").trim().to_string()); }
        let build = nm_core::updater::latest().map(|r| r.build).unwrap_or(0);
        if self.banner_closed.as_deref() == Some(build.to_string().as_str()) { return -1.0; }
        if self.banner_build != build {
            self.banner_build = build;
            self.banner_at = t;
        }
        if (self.hover_prompt == "N" || self.hover_prompt == "NX") && t - self.banner_at > 0.3 { self.banner_at = t - 0.3; }
        let age = t - self.banner_at;
        if age <= 8.0 { age } else { -1.0 }
    }
    fn update_banner(&mut self, g: &mut Graphics, w: i32, h: i32, t: f32, below: f32) {
        let age = self.banner_age(t);
        if age < 0.0 { return; }
        let (wf, hf) = (w as f32, h as f32);
        let show = ease(age / 0.28).min(ease((8.0 - age) / 0.28));
        let text = format!("{} is ready", nm_core::updater::upper(&nm_core::updater::latest_label()));
        let bh = hf * 0.056;
        let y = (hf * 0.052).max(below) - (1.0 - show) * (bh + hf * 0.06);
        let rad = bh / 2.0;
        let pad = bh * 0.16;
        let size = bh - 2.0 * pad;
        let close = bh * 0.8;
        let sb = self.fonts.small_bold.clone();
        let text_w = g.measure_string(&text, &sb).width + pad;
        let bw = (wf * 0.9).min(pad + size + pad * 1.4 + text_w + close);
        let r = RectangleF::new((wf - bw) / 2.0, y, bw, bh);
        g.fill_path(&solid(Color::from_argb((55.0 * show) as i32, 0, 0, 0)), &round(RectangleF::new(r.x, r.y + hf * 0.004, r.width, r.height), rad));
        g.fill_path(&solid(Color::from_argb((248.0 * show) as i32, 252, 252, 254)), &round(r, rad));
        let icon = RectangleF::new(r.x + pad, r.y + pad, size, size);
        card(g, icon, size / 2.0, Color::rgb(52, 134, 240), Color::rgb(30, 104, 214), Color::EMPTY, 0.0);
        let isf = self.fonts.icon_small.clone();
        self.w(g, "\u{E895}", &isf, Color::WHITE, icon, Center, Center, false);
        let x = icon.right() + pad * 1.4;
        self.w(g, &text, &sb, PHONE_TEXT, RectangleF::new(x, r.y, r.right() - close - x, bh), Near, Center, false);
        let x1 = RectangleF::new(r.right() - close, r.y + (bh - close) / 2.0, close, close);
        let cross = close * 0.14;
        let (cx, cy) = (x1.x + close / 2.0, x1.y + close / 2.0);
        let pen = Pen::new(if self.hover_prompt == "NX" { PHONE_TEXT } else { PHONE_SUB }, (hf * 0.0024).max(1.5));
        g.draw_line_f(&pen, cx - cross, cy - cross, cx + cross, cy + cross);
        g.draw_line_f(&pen, cx - cross, cy + cross, cx + cross, cy - cross);
        self.prompt_hits.push((RectangleF::new(x1.x + self.screen_x, x1.y + self.screen_y, x1.width, x1.height), "NX".into()));
        self.prompt_hits.push((RectangleF::new(r.x + self.screen_x, r.y + self.screen_y, r.width, r.height), "N".into()));
    }

    /// ClashBanner: red and sticky until the clash is settled, hidden on the Mods pages. How far down it reaches, or 0.
    fn clash_banner(&mut self, g: &mut Graphics, w: i32, h: i32, t: f32) -> f32 {
        let showing = self.clash_game.is_some() && !self.starting && !self.job && !self.pages.iter().any(|x| x.mods);
        if !showing { self.clash_at = f32::NAN; return 0.0; }
        if self.clash_at.is_nan() { self.clash_at = t; }
        let (wf, hf) = (w as f32, h as f32);
        let show = ease((t - self.clash_at) / 0.28);
        let text = "Mods are clashing and may cause issues";
        let bh = hf * 0.056;
        let y = hf * 0.052 - (1.0 - show) * (bh + hf * 0.06);
        let rad = bh / 2.0;
        let pad = bh * 0.16;
        let size = bh - 2.0 * pad;
        let sb = self.fonts.small_bold.clone();
        let text_w = g.measure_string(text, &sb).width + pad;
        let bw = (wf * 0.9).min(pad + size + pad * 1.4 + text_w + pad * 1.6);
        let r = RectangleF::new((wf - bw) / 2.0, y, bw, bh);
        let over = self.hover_prompt == "C";
        g.fill_path(&solid(Color::from_argb((70.0 * show) as i32, 90, 0, 0)), &round(RectangleF::new(r.x, r.y + hf * 0.004, r.width, r.height), rad));
        g.fill_path(&solid(Color::from_argb((250.0 * show) as i32, if over { 236 } else { 214 }, if over { 58 } else { 40 }, if over { 58 } else { 44 })), &round(r, rad));
        let icon = RectangleF::new(r.x + pad, r.y + pad, size, size);
        card(g, icon, size / 2.0, Color::WHITE, Color::rgb(255, 226, 226), Color::EMPTY, 0.0);
        let isf = self.fonts.icon_small.clone();
        self.w(g, "\u{E7BA}", &isf, Color::rgb(200, 32, 36), icon, Center, Center, false);
        let x = icon.right() + pad * 1.4;
        self.w(g, text, &sb, Color::WHITE, RectangleF::new(x, r.y, r.right() - pad - x, bh), Near, Center, false);
        self.prompt_hits.push((RectangleF::new(r.x + self.screen_x, r.y + self.screen_y, r.width, r.height), "C".into()));
        r.bottom() + hf * 0.012
    }

    /// ImproveBanner: a quiet 3DSrecomp+Zakuro rebuild's pill, with its progress along the bottom.
    fn improve_banner(&mut self, g: &mut Graphics, w: i32, h: i32, t: f32, top: f32) {
        let wall = self.hooks.wall_seconds();
        let rb = match self.recomp_banner.clone() {
            Some(b) if (b.fake_running || wall - b.done_at_wall < 5.0) && !self.starting && !self.job => b,
            _ => { self.improve_at = f32::NAN; self.improve_shown = 0.0; return; }
        };
        let (wf, hf) = (w as f32, h as f32);
        if self.improve_at.is_nan() { self.improve_at = t; self.improve_last = t; }
        let done = wall - rb.done_at_wall < 5.0;
        let target = if done { 1.0 } else { rb.progress };
        let dt = (t - self.improve_last).min(0.1).max(0.0);
        self.improve_last = t;
        self.improve_shown += (target - self.improve_shown) * (dt * 3.5).min(1.0);
        let mut show = ease((t - self.improve_at) / 0.28);
        if done { show = show.min(ease((5.0 - (wall - rb.done_at_wall) as f32) / 0.28)); }
        let pct = (self.improve_shown as f64 * 100.0).round_ties_even() as i32;
        // Light mode pauses it while a game plays (Rust only): the banner says so, and the arrows stand still.
        let text = if done { format!("{}{}", rb.title, if rb.verb == "Building" { " is ready" } else { " improved" }) } else if rb.paused { format!("{} {} paused \u{b7} {}%", rb.verb, rb.title, pct) } else { format!("{} {} \u{b7} {}%", rb.verb, rb.title, pct) };
        let bh = hf * 0.056;
        let y = top - (1.0 - show) * (bh + hf * 0.06);
        let rad = bh / 2.0;
        let pad = bh * 0.16;
        let size = bh - 2.0 * pad;
        let sb = self.fonts.small_bold.clone();
        let text_w = g.measure_string(&text, &sb).width + pad;
        let bw = (wf * 0.9).min(pad + size + pad * 1.4 + text_w + pad * 1.6);
        let r = RectangleF::new((wf - bw) / 2.0, y, bw, bh);
        g.fill_path(&solid(Color::from_argb((55.0 * show) as i32, 0, 0, 0)), &round(RectangleF::new(r.x, r.y + hf * 0.004, r.width, r.height), rad));
        g.fill_path(&solid(Color::from_argb((248.0 * show) as i32, 252, 252, 254)), &round(r, rad));
        let icon = RectangleF::new(r.x + pad, r.y + pad, size, size);
        card(g, icon, size / 2.0, Color::rgb(60, 190, 110), Color::rgb(34, 150, 80), Color::EMPTY, 0.0);
        let isf = self.fonts.icon_small.clone();
        if done {
            self.w(g, "\u{E73E}", &isf, Color::WHITE, icon, Center, Center, false);
        } else {
            // The arrows turn around their own drawn center: our glyph's ink fills its em box.
            self.hooks.text_plain(g, "(glyph)", &isf, icon);
            let em = isf.size;
            let st = g.save();
            let mode = g.smoothing_mode;
            g.smoothing_mode = SmoothingMode::AntiAlias;
            g.translate_transform(icon.x + size / 2.0, icon.y + size / 2.0);
            g.rotate_transform(if rb.paused { 0.0 } else { (t - self.improve_at) * 140.0 % 360.0 });
            g.translate_transform(-em / 2.0, -em / 2.0);
            nm_draw::icons::draw_ex(g, '\u{E895}', RectangleF::new(0.0, 0.0, em, em), Color::from_argb((255.0 * show) as i32, 255, 255, 255), false);
            g.smoothing_mode = mode;
            g.restore(st);
        }
        let x = icon.right() + pad * 1.4;
        self.w(g, &text, &sb, PHONE_TEXT, RectangleF::new(x, r.y, r.right() - pad - x, bh * 0.92), Near, Center, false);
        let bar_h = (bh * 0.07).max(2.0);
        let bar_w = r.right() - pad * 1.6 - x;
        let bar_y = r.bottom() - pad - bar_h;
        g.fill_path(&solid(Color::from_argb((40.0 * show) as i32, 0, 0, 0)), &round(RectangleF::new(x, bar_y, bar_w, bar_h), bar_h / 2.0));
        let fill = self.improve_shown.min(1.0).max(0.03);
        let filled = RectangleF::new(x, bar_y, bar_w * fill, bar_h);
        let bar = round(filled, bar_h / 2.0);
        g.fill_path(&solid(Color::from_argb((255.0 * show) as i32, 52, 168, 90)), &bar);
        if !done && filled.width > bar_h * 2.0 {
            let band = (bar_h * 6.0).max(bar_w * 0.22);
            let sweep = ((t - self.improve_at) % 1.4) / 1.4;
            let bx = filled.x - band + (filled.width + band) * sweep;
            let st = g.save();
            g.set_clip_path(&bar);
            let clear = Color::from_argb(0, 255, 255, 255);
            let mut lg = LinearGradient::new(RectangleF::new(bx, bar_y, band, bar_h), clear, clear, 0.0);
            lg.colors = vec![(clear, 0.0), (Color::from_argb((150.0 * show) as i32, 255, 255, 255), 0.5), (clear, 1.0)];
            g.fill_rectangle(&Brush::Linear(lg), RectangleF::new(bx, bar_y, band, bar_h));
            g.restore(st);
        }
    }

    fn status_bar(&mut self, g: &mut Graphics, w: i32, h: i32, ink: Color) {
        let (wf, hf) = (w as f32, h as f32);
        let bar = hf * 0.05;
        let x = wf * 0.06;
        let f = self.fonts.small_bold.clone();
        let clock = self.hooks.wall_hmm();
        self.w(g, &clock, &f, ink, RectangleF::new(x, 0.0, wf * 0.4, bar), Near, Center, false);
        let level = match self.hooks.battery.as_deref() {
            Some("none") | None => return,
            Some(v) => v.parse::<i32>().unwrap_or(0) as f32 / 100.0,
        };
        let bw = wf * 0.055;
        let bh = bw * 0.5;
        let bx = wf - x - bw;
        let by = (bar - bh) / 2.0;
        g.draw_path(&Pen::new(fade(ink, 150), 1.0), &round(RectangleF::new(bx, by, bw, bh), bh * 0.28));
        let fill = (bw - 4.0) * level.clamp(0.05, 1.0);
        g.fill_path(&solid(ink), &round(RectangleF::new(bx + 2.0, by + 2.0, fill, bh - 4.0), (bh - 4.0) * 0.25));
    }

    pub fn fit_font(&self, g: &Graphics, s: &str, area: SizeF, max: &Font, min_scale: f32, wrap: bool) -> Font {
        if s.is_empty() { return max.clone(); }
        let mut f = max.clone();
        let mut tenths = (max.size * 10.0) as i32;
        let end = (max.size * 10.0 * min_scale) as i32;
        while tenths >= end {
            f = Font::new(&max.face, tenths as f32 / 10.0);
            let l = nm_draw::text::layout(s, &f, area.width, wrap);
            let lines = l.lines.len() as f32;
            let fits_h = (lines - 1.0) * l.line_height + f.get_height() <= area.height + 0.01;
            let fits_w = wrap || l.lines.iter().all(|ln| ln.chars.iter().map(|c| f.advance_f(c.1)).sum::<f32>() <= area.width - f.size / 3.0);
            if fits_h && fits_w { break; }
            let _ = g;
            tenths -= 5;
        }
        f
    }

    pub(crate) fn back_button_pub(&mut self, g: &mut Graphics, w: i32, h: i32, m: f32, top: f32, accent_c: Color) -> f32 { self.back_button(g, w, h, m, top, accent_c) }
    fn back_button(&mut self, g: &mut Graphics, w: i32, h: i32, m: f32, top: f32, accent_c: Color) -> f32 {
        let hf = h as f32;
        let back = RectangleF::new(m * 0.6, top - hf * 0.004, w as f32 * 0.3, self.fonts.small_bold.height() as f32 * 2.0);
        self.prompt_hits.push((RectangleF::new(back.x + self.screen_x, back.y + self.screen_y, back.width, back.height), "B".into()));
        let ch = back.height * 0.2;
        let cmid = back.y + back.height / 2.0;
        let cx0 = back.x + m * 0.4;
        let pen = Pen::new(accent_c, (hf * 0.0035).max(2.0)).round();
        g.draw_lines(&pen, &[PointF::new(cx0 + ch * 0.8, cmid - ch), PointF::new(cx0, cmid), PointF::new(cx0 + ch * 0.8, cmid + ch)]);
        let f = self.fonts.item.clone();
        self.w(g, "Back", &f, accent_c, RectangleF::new(cx0 + ch * 1.4, back.y, back.width, back.height), Near, Center, false);
        back.bottom()
    }

    fn render_page(&mut self, g: &mut Graphics, p: &mut Page, w: i32, h: i32, m: f32, top: f32, bottom: f32, t: f32, th: &Theme) {
        let (wf, hf) = (w as f32, h as f32);
        let accent_c = accent(th);
        let mut y = self.back_button(g, w, h, m, top, accent_c);
        // The boxes take the whole screen under Back: their headers already say what they are.
        if let Some(mut v) = p.boxes.take() { self.render_boxes(g, &mut v, w, h, m, y + hf * 0.004, bottom, t, th); p.boxes = Some(v); return; }
        let title_area = SizeF::new(wf - 2.0 * m, self.fonts.title.height() as f32 * 1.3);
        let title_font = self.fonts.title.clone();
        let tf = self.fit_font(g, &p.title, title_area, &title_font, 0.6, false);
        self.w(g, &p.title, &tf, PHONE_TEXT, RectangleF::new(m, y, title_area.width, title_area.height), Near, Center, false);
        y += title_area.height + hf * 0.012;
        if p.dex_id > 0 { self.render_dex_entry(g, p, w, h, m, y, bottom, t, th); return; }
        let rad = wf * 0.04;
        let body_f = self.fonts.body.clone();
        if let Some(body) = p.body.clone() {
            let mut body_h = if !p.items.is_empty() { (bottom - y) * 0.6 } else { bottom - y };
            if p.fit_body && !p.items.is_empty() {
                body_h = body_h.min(g.measure_string_w(&body, &body_f, (wf - 2.0 * m - 2.0 * hf * 0.024) as i32).height + 2.0 * hf * 0.024 + body_f.height() as f32 * 0.3);
            }
            let br = RectangleF::new(m, y, wf - 2.0 * m, body_h);
            g.fill_path(&solid(PHONE_ROW), &round(br, rad));
            let pad = hf * 0.024;
            let mut body_text = br.height - 2.0 * pad;
            if p.mono.is_some() {
                let small_h = self.fonts.small.height() as f32;
                body_text = body_text.min(g.measure_string_w(&body, &body_f, (br.width - 2.0 * pad) as i32).height + small_h * 0.5);
            }
            let whole = if p.items.is_empty() && p.mono.is_none() { g.measure_string_w(&body, &body_f, (br.width - 2.0 * pad) as i32).height + body_f.height() as f32 } else { 0.0 };
            if whole > body_text {
                p.body_scroll = p.body_scroll.min(whole - body_text).max(0.0);
                let st = g.save();
                g.set_clip_rect(RectangleF::new(br.x, br.y + pad, br.width, body_text));
                let r = RectangleF::new(br.x + pad, br.y + pad - p.body_scroll, br.width - 2.0 * pad, whole);
                self.w(g, &body, &body_f, PHONE_TEXT, r, Near, Near, true);
                if p.links { self.body_links(g, &body, &body_f, r, RectangleF::new(br.x, br.y + pad, br.width, body_text), link_color(th)); }
                g.restore(st);
                let max_scroll = whole - body_text;
                let bar_w = (wf * 0.008).max(3.0);
                let bar_h = body_text * body_text / whole;
                g.fill_path(&solid(Color::from_argb(120, 120, 120, 128)), &round(RectangleF::new(br.right() - bar_w * 2.0, br.y + pad + (body_text - bar_h) * p.body_scroll / max_scroll, bar_w, bar_h), bar_w / 2.0));
            } else {
                let r = RectangleF::new(br.x + pad, br.y + pad, br.width - 2.0 * pad, body_text);
                self.w(g, &body, &body_f, PHONE_TEXT, r, Near, Near, true);
                if p.links { self.body_links(g, &body, &body_f, r, r, link_color(th)); }
            }
            if let Some(mono) = p.mono.clone() {
                let eb = RectangleF::new(br.x + pad, br.y + pad + body_text, br.width - 2.0 * pad, br.height - 2.0 * pad - body_text);
                card(g, eb, hf * 0.012, Color::rgb(28, 28, 30), Color::rgb(28, 28, 30), Color::EMPTY, 0.0);
                let mf = self.fonts.mono.clone();
                self.w(g, &mono, &mf, Color::rgb(235, 235, 240), RectangleF::new(eb.x + pad * 0.5, eb.y + pad * 0.5, eb.width - pad, eb.height - pad), Near, Near, true);
            }
            y = br.bottom() + hf * 0.016;
        }
        if p.items.is_empty() { return; }
        let small = self.fonts.small.clone();
        let mut foot_h = 0.0;
        let chosen = p.items[(p.sel as usize).min(p.items.len() - 1)].clone();
        let note = if p.body.is_some() { String::new() } else if !chosen.info.is_empty() { chosen.info.clone() } else { chosen.hint.clone() };
        let mut width = wf - 2.0 * m;
        let note_x = m + wf * 0.02;
        let note_w = width - wf * 0.04;
        // Wide: the list a column on the left, the chosen option shown large on the right.
        let side = p.body.is_none() && wf > hf * 1.25;
        let mut panel = RectangleF::EMPTY;
        if side {
            let gap_x = wf * 0.025;
            width = (wf - 2.0 * m) * 0.46;
            panel = RectangleF::new(m + width + gap_x, y + row_h_for(h) * 0.06, wf - 2.0 * m - width - gap_x, bottom - y - row_h_for(h) * 0.12);
        }
        if p.body.is_none() && !side {
            if !note.is_empty() { foot_h += g.measure_string_w(&note, &small, note_w as i32).height + hf * 0.006; }
            for rec in &chosen.records { foot_h += g.measure_string_w(&rec.replace(": ", ":  "), &small, note_w as i32).height + hf * 0.002; }
            foot_h = (foot_h + hf * 0.012).min((bottom - y) * 0.34);
        }
        let list_bottom = bottom - foot_h;
        let row_h = row_h_for(h);
        let fit = (((list_bottom - y) / row_h) as i32).max(1);
        if p.sel < p.top_row { p.top_row = p.sel; } else if p.sel >= p.top_row + fit { p.top_row = p.sel - fit + 1; }
        p.top_row = p.top_row.min(p.items.len() as i32 - fit).max(0);
        if p.scroll_row < 0.0 { p.scroll_row = p.top_row as f32; }
        p.scroll_row += (p.top_row as f32 - p.scroll_row) * 0.3;
        if (p.top_row as f32 - p.scroll_row).abs() < 0.01 { p.scroll_row = p.top_row as f32; }
        self.scrolling = p.scroll_row != p.top_row as f32;
        let overflow = p.items.len() as i32 > fit;
        let group_h = if overflow { list_bottom - y } else { p.items.len() as f32 * row_h };
        let group = RectangleF::new(m, y, width, group_h);
        let gap = row_h * 0.12;
        {
            let kept = g.save();
            g.set_clip_rect(group);
            for i in 0..p.items.len() {
                let slot = RectangleF::new(m, y + (i as f32 - p.scroll_row) * row_h, width - if overflow { (wf * 0.008).max(3.0) * 3.0 } else { 0.0 }, row_h);
                let r = RectangleF::new(slot.x, slot.y + gap / 2.0, slot.width, slot.height - gap);
                if !r.intersects_with(group) { continue; }
                let pop = back_out((t - self.bounce_at - 0.05 * (i as f32 - p.scroll_row).max(0.0)) / 0.36);
                if pop <= 0.01 { continue; }
                let popped = g.save();
                if t - self.bounce_at < 1.4 {
                    let (cx, cy) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
                    g.translate_transform(cx, cy + (1.0 - pop) * row_h * 0.5);
                    g.scale_transform(0.85 + 0.15 * pop, 0.85 + 0.15 * pop);
                    g.translate_transform(-cx, -cy);
                }
                g.fill_path(&solid(PHONE_ROW), &round(r, r.height / 2.0));
                let it = p.items[i].clone();
                self.list_row(g, &it, i as i32 == p.sel, r, false, h, accent_c, th);
                g.restore(popped);
                self.hits.push((RectangleF::intersect(r, group), i as i32));
            }
            g.restore(kept);
        }
        if overflow {
            let bar_w = (wf * 0.008).max(3.0);
            let bar_h = group.height * fit as f32 / p.items.len() as f32;
            let bar_y = group.y + (group.height - bar_h) * p.scroll_row / (p.items.len() as i32 - fit) as f32;
            g.fill_path(&solid(Color::from_argb(120, 120, 120, 128)), &round(RectangleF::new(group.right() - bar_w * 1.2, bar_y, bar_w, bar_h), bar_w / 2.0));
        }
        if side { let gid = chosen.game_id.clone().or_else(|| p.game_id.clone()); self.detail_panel(g, &chosen, gid.as_deref(), panel, h, accent_c, t); return; }
        if p.body.is_some() { return; }
        let mut fy = list_bottom + hf * 0.01;
        if !note.is_empty() {
            let nh = (g.measure_string_w(&note, &small, note_w as i32).height + 2.0).min(bottom - fy);
            self.w(g, &note, &small, PHONE_SUB, RectangleF::new(note_x, fy, note_w, nh), Near, Near, true);
            fy += nh + hf * 0.006;
        }
        for rec in &chosen.records {
            let rec = rec.replace(": ", ":  ");
            let rh = g.measure_string_w(&rec, &small, note_w as i32).height;
            if fy + rh > bottom { break; }
            self.w(g, &rec, &small, PHONE_SUB, RectangleF::new(note_x, fy, note_w, rh + 1.0), Near, Near, true);
            fy += rh + hf * 0.002;
        }
    }

    fn list_row(&mut self, g: &mut Graphics, it: &Item, sel: bool, r: RectangleF, line: bool, h: i32, accent_c: Color, _th: &Theme) {
        if sel { g.fill_path(&solid(fade(accent_c, 38)), &round(r, r.height / 2.0)); }
        if it.fold != 0 {
            self.section_row(g, it, sel, r, line, h, accent_c);
            return;
        }
        if it.dex_no > 0 { self.dex_row(g, it, r); return; }
        if it.region >= 0 && it.region_total > 0 { self.region_row(g, it, r); return; }
        let icon = r.height * 0.62;
        let pad = r.height * 0.2;
        let ix = r.x + pad;
        let iy = r.y + (r.height - icon) / 2.0;
        let ib = RectangleF::new(ix, iy, icon, icon);
        let glyph = self.fonts.icon_small.clone();
        self.row_icon(g, it, ib, &glyph);
        self.list_row_text(g, it, sel, r, line, h, accent_c, ib.right() + pad, r.right() - pad);
    }

    fn row_icon(&mut self, g: &mut Graphics, it: &Item, ib: RectangleF, glyph: &Font) {
        let icon = ib.width;
        let pic = it.sprite.as_deref().and_then(crate::sprites::get);
        if let Some(pic) = pic {
            draw_sprite(g, &pic, RectangleF::new(ib.x - icon * 0.15, ib.y - icon * 0.15, icon * 1.3, icon * 1.3));
        } else if crate::sprites::loading(it.sprite.as_deref()) {
            let t = self.now();
            spinning_ball(g, ib.x + icon / 2.0, ib.y + icon / 2.0, icon * 0.8, t, false);
            self.spinning = true;
        } else if let Some(th) = &it.theme {
            cart(g, ib.x + icon * 0.1, ib.y, icon, th);
        } else {
            let tint = row_tint(it);
            gloss_tile(g, ib, icon * 0.24, tint);
            if !row_art(g, it.icon, ib, tint) {
                let s = it.icon.to_string();
                self.w(g, &s, glyph, Color::with_alpha(90, INK), RectangleF::new(ib.x, ib.y + icon * 0.04, ib.width, ib.height), Center, Center, false);
                self.w(g, &s, glyph, Color::WHITE, ib, Center, Center, false);
            }
        }
    }

    /// A section header: an arrow pointing down when open, right when folded, then its name and how many it holds.
    fn section_row(&mut self, g: &mut Graphics, it: &Item, sel: bool, r: RectangleF, line: bool, h: i32, accent_c: Color) {
        let pad = r.height * 0.2;
        let c = r.height * 0.11;
        let cx = r.x + pad + r.height * 0.31;
        let cy = r.y + r.height / 2.0;
        let arrow = if it.fold == 1 {
            [PointF::new(cx - c, cy - c * 0.5), PointF::new(cx, cy + c * 0.5), PointF::new(cx + c, cy - c * 0.5)]
        } else {
            [PointF::new(cx - c * 0.5, cy - c), PointF::new(cx + c * 0.5, cy), PointF::new(cx - c * 0.5, cy + c)]
        };
        g.draw_lines(&Pen::new(accent_c, (h as f32 * 0.0035).max(2.0)).round().round_join(), &arrow);
        let mut title = it.text.clone();
        let mut count = String::new();
        if let Some(colon) = title.rfind(": ") {
            if colon > 0 {
                count = title[colon + 2..].to_string();
                title = title[..colon].to_string();
            }
        }
        let tx = r.x + pad * 2.0 + r.height * 0.62;
        let right = r.right() - pad;
        let item_f = self.fonts.item.clone();
        let cw = if !count.is_empty() { g.measure_string(&count, &item_f).width + 4.0 } else { 0.0 };
        if !count.is_empty() { self.w(g, &count, &item_f, if sel { accent_c } else { PHONE_SUB }, RectangleF::new(right - cw, r.y, cw, r.height), Far, Center, false); }
        let ib = self.fonts.item_bold.clone();
        self.w(g, &title, &ib, if sel { accent_c } else { PHONE_TEXT }, RectangleF::new(tx, r.y, right - cw - tx, r.height), Near, Center, false);
        if line { g.draw_line_f(&Pen::new(PHONE_LINE, 1.0), r.x, r.bottom() - 1.0, r.right(), r.bottom() - 1.0); }
    }

    /// A slider row: its name (and where the value comes from) on the left, a track with a knob and the value on the right.
    fn slider_row(&mut self, g: &mut Graphics, it: &Item, sel: bool, r: RectangleF, tx: f32, right: f32, accent_c: Color) {
        let item_f = self.fonts.item.clone();
        let small = self.fonts.small.clone();
        let value_w = g.measure_string("100%", &item_f).width + 4.0;
        let track_w = ((right - tx) * 0.42).min(r.height * 4.2);
        let track_x = right - value_w - track_w - r.height * 0.3;
        let name_w = track_x - tx - r.height * 0.2;
        if !it.slider_note.is_empty() {
            let th1 = item_f.height() as f32 * 1.05;
            let th2 = small.height() as f32 * 1.1;
            let ty = r.y + (r.height - th1 - th2) / 2.0;
            self.w(g, &it.text, &item_f, PHONE_TEXT, RectangleF::new(tx, ty, name_w, th1), Near, Center, false);
            self.w(g, &it.slider_note, &small, PHONE_SUB, RectangleF::new(tx, ty + th1, name_w, th2), Near, Center, false);
        } else {
            self.w(g, &it.text, &item_f, PHONE_TEXT, RectangleF::new(tx, r.y, name_w, r.height), Near, Center, false);
        }
        let cy = r.y + r.height / 2.0;
        let th = (r.height * 0.14).max(5.0);
        let k = if it.slider < 0 { 0.5 } else { it.slider as f32 / 100.0 };
        let knob = r.height * 0.22;
        let track = RectangleF::new(track_x, cy - th / 2.0, track_w, th);
        g.fill_path(&solid(Color::rgb(222, 222, 230)), &round(track, th / 2.0));
        if it.slider > 0 { g.fill_path(&solid(accent_c), &round(RectangleF::new(track_x, cy - th / 2.0, th.max(track_w * k), th), th / 2.0)); }
        let kx = track_x + track_w * k;
        let own = it.slider < 0;
        g.fill_ellipse(&solid(Color::from_argb(if own { 18 } else { 45 }, 0, 0, 0)), RectangleF::new(kx - knob, cy - knob + knob * 0.18, knob * 2.0, knob * 2.0));
        g.fill_ellipse(&solid(if own { Color::rgb(246, 246, 250) } else { Color::WHITE }), RectangleF::new(kx - knob, cy - knob, knob * 2.0, knob * 2.0));
        let mut pen = Pen::new(if own { Color::rgb(196, 196, 204) } else if sel { accent_c } else { Color::rgb(200, 200, 206) }, (knob * 0.14).max(1.5));
        if own { pen.dash_style = DashStyle::Dot; }
        g.draw_ellipse(&pen, RectangleF::new(kx - knob, cy - knob, knob * 2.0, knob * 2.0));
        let value = if it.slider < 0 { "Own".to_string() } else if it.slider == 0 { "Off".to_string() } else { format!("{}%", it.slider) };
        self.w(g, &value, &item_f, if sel { accent_c } else { PHONE_SUB }, RectangleF::new(right - value_w, r.y, value_w, r.height), Far, Center, false);
    }

    fn list_row_text(&mut self, g: &mut Graphics, it: &Item, sel: bool, r: RectangleF, line: bool, h: i32, accent_c: Color, tx: f32, right: f32) {
        let hf = h as f32;
        if it.is_slider {
            self.slider_row(g, it, sel, r, tx, right, accent_c);
            if line { g.draw_line_f(&Pen::new(PHONE_LINE, 1.0), tx, r.bottom() - 1.0, r.right(), r.bottom() - 1.0); }
            return;
        }
        let mut right = right;
        let mut title = it.text.replace("Pokemon ", "");
        let mut value: Option<String> = None;
        if let Some(colon) = title.find(": ") {
            if colon > 0 {
                value = Some(title[colon + 2..].to_string());
                title = title[..colon].to_string();
            }
        }
        let ink = if it.on { PHONE_TEXT } else { TEXT3 };
        if value.is_none() && it.on {
            let c = r.height * 0.1;
            let cy = r.y + r.height / 2.0;
            let cx = right - c * 0.3;
            let pen = Pen::new(Color::rgb(190, 190, 196), (hf * 0.0032).max(2.0)).round();
            g.draw_lines(&pen, &[PointF::new(cx - c * 0.7, cy - c), PointF::new(cx, cy), PointF::new(cx - c * 0.7, cy + c)]);
            right -= c * 1.6;
        }
        let item_f = self.fonts.item.clone();
        let small = self.fonts.small.clone();
        let mut value_w = 0.0;
        if let Some(v) = &value {
            value_w = (g.measure_string(v, &item_f).width + 4.0).min((right - tx) * 0.55);
            let c = if v == "\u{2605}" { Color::rgb(226, 164, 16) } else if sel { accent_c } else { PHONE_SUB };
            self.w(g, v, &item_f, c, RectangleF::new(right - value_w, r.y, value_w, r.height), Far, Center, false);
        }
        let tw = right - tx - value_w;
        let sub = if !it.tile_hint.is_empty() { it.tile_hint.clone() } else if !it.on { it.hint.clone() } else { String::new() };
        if !sub.is_empty() {
            let th1 = item_f.height() as f32 * 1.05;
            let th2 = small.height() as f32 * 1.1;
            let ty = r.y + (r.height - th1 - th2) / 2.0;
            self.w(g, &title, &item_f, ink, RectangleF::new(tx, ty, tw, th1), Near, Center, false);
            self.w(g, &sub, &small, PHONE_SUB, RectangleF::new(tx, ty + th1, tw, th2), Near, Center, false);
        } else {
            self.w(g, &title, &item_f, ink, RectangleF::new(tx, r.y, tw, r.height), Near, Center, false);
        }
        if line { g.draw_line_f(&Pen::new(PHONE_LINE, 1.0), tx, r.bottom() - 1.0, r.right(), r.bottom() - 1.0); }
    }

    /// Web addresses in a body: underlined in the link color, the text redrawn in it.
    fn body_links(&mut self, g: &mut Graphics, text: &str, f: &Font, r: RectangleF, visible: RectangleF, link: Color) {
        let sf = StringFormat { alignment: Near, line_alignment: Near, no_wrap: false, line_limit: true, ellipsis: true };
        for (start, len) in crate::links::find(text) {
            let rects = char_range_rects(text, f, r, &sf, start, len);
            for reg in &rects {
                let st = g.save();
                g.set_clip_rect(*reg);
                g.fill_rectangle(&solid(PHONE_ROW), *reg);
                self.w(g, text, f, link, r, Near, Near, true);
                g.restore(st);
            }
            for line in &rects {
                if line.bottom() < visible.top() || line.top() > visible.bottom() { continue; }
                let uy = line.bottom() - f.height() as f32 * 0.12;
                g.draw_line_f(&Pen::new(link, (f.height() as f32 * 0.07).max(1.0)), line.left(), uy, line.right(), uy);
            }
        }
    }

    fn render_splash(&mut self, g: &mut Graphics, w: i32, h: i32) {
        let (wf, hf) = (w as f32, h as f32);
        g.smoothing_mode = SmoothingMode::AntiAlias;
        let t = self.now();
        let since = (self.hooks.wall_seconds() - self.start_since_wall) as f32;
        let all = RectangleF::new(0.0, 0.0, wf, hf);
        g.fill_rectangle(&solid(Color::BLACK), all);
        let cx = wf / 2.0;
        let cy = hf * 0.42;
        let rise = (1.0 - ease(since / 0.6)) * hf * 0.012;
        spinning_ball(g, cx, cy - hf * 0.1 + rise, hf * 0.104, t, true);
        let size = hf * 0.062;
        let bold = UiFont::get("Segoe UI Semibold", size);
        let light = UiFont::get("Segoe UI Light", size);
        let a1 = g.measure_string("Native", &bold);
        let a2 = g.measure_string("Mons", &light);
        let nudge = size * 0.28;
        let mark_w = a1.width + a2.width - nudge;
        let mx = cx - mark_w / 2.0;
        let my = cy + rise;
        self.w(g, "Native", &bold, Color::WHITE, RectangleF::new(mx, my, a1.width + 8.0, a1.height), Near, Near, false);
        self.w(g, "Mons", &light, Color::from_argb(200, 255, 255, 255), RectangleF::new(mx + a1.width - nudge, my, a2.width + 8.0, a2.height), Near, Near, false);
        let track = RectangleF::new(wf * 0.32, cy + hf * 0.115, wf * 0.36, (hf * 0.0048).max(3.0));
        g.fill_path(&solid(Color::from_argb(60, 255, 255, 255)), &round(track, track.height / 2.0));
        let run = track.width * 0.3;
        let at = (t * 0.55 % 1.0) * (track.width + run) - run;
        let clip = g.save();
        g.set_clip_path(&round(track, track.height / 2.0));
        g.fill_rectangle(&solid(Color::WHITE), RectangleF::new(track.x + at, track.y, run, track.height));
        g.restore(clip);
        let stage = self.start_stage.clone().unwrap_or_default();
        let small = self.fonts.small.clone();
        self.w(g, &stage, &small, Color::from_argb(170, 255, 255, 255), RectangleF::new(0.0, track.bottom() + hf * 0.018, wf, small.height() as f32 * 1.4), Center, Near, false);
        let fd = ease(since / 0.45);
        if fd < 1.0 { g.fill_rectangle(&solid(Color::from_argb((255.0 * (1.0 - fd)) as i32, 8, 10, 14)), all); }
    }
}

/// GuideOverlay.Draw: Resume, Controls and Close game over the game's screen.
#[allow(clippy::too_many_arguments)]
pub fn guide_draw(hooks: &mut TestHooks, g: &mut Graphics, size: (i32, i32), title: &str, controls: &str, accent_c: Color, sel: i32, show_controls: bool, confirm_close: bool) -> Vec<RectangleF> {
    let mut hits = Vec::new();
    let (sw, sh) = (size.0 as f32, size.1 as f32);
    g.fill_rectangle(&solid(Color::rgb(10, 12, 18)), RectangleF::new(0.0, 0.0, sw, sh));
    let cw = 640f32.min(sw * 0.46);
    let pad = cw * 0.06;
    let row_h = cw * 0.12;
    let gap = cw * 0.02;
    let inner = cw - 2.0 * pad;
    let f1 = UiFont::get("Segoe UI Semibold", cw * 0.058);
    let f0 = UiFont::get("Segoe UI", cw * 0.034);
    let f2 = UiFont::get("Segoe UI Semibold", cw * 0.046);
    let f3 = UiFont::get("Segoe UI", cw * 0.034);
    let title_h = f1.height() as f32 * 1.3;
    let label_h = f0.height() as f32 * 1.4;
    let text_h = if show_controls { g.measure_string_w(controls, &f3, (inner - pad) as i32).height + pad } else { 0.0 };
    let hint_h = f3.height() as f32 * 2.2;
    let ch = pad + label_h + title_h + gap * 2.0 + 3.0 * row_h + 2.0 * gap + if show_controls { text_h + gap * 2.0 } else { 0.0 } + if confirm_close { f3.height() as f32 * 1.4 } else { 0.0 } + hint_h + pad * 0.5;
    let card_r = RectangleF::new((sw - cw) / 2.0, pad.max((sh - ch) / 2.0), cw, ch);
    let path = round(card_r, cw * 0.05);
    g.fill_path(&solid(Color::rgb(30, 32, 40)), &path);
    g.draw_path(&Pen::new(Color::with_alpha(120, accent_c), 2.0), &path);
    let mut y = card_r.y + pad;
    let x = card_r.x + pad;
    let plain = StringFormat { alignment: Near, line_alignment: Near, no_wrap: false, line_limit: false, ellipsis: false };
    let ms = g.measure_string("NATIVEMONS", &f0);
    let at = RectangleF::new(x, y, 100000.0, 100000.0);
    g.draw_string("NATIVEMONS", &f0, Color::from_argb(150, 255, 255, 255), at, &plain);
    hooks.text(g, "NATIVEMONS", &f0, RectangleF::new(x, y, ms.width, ms.height), &plain);
    y += label_h;
    g.draw_string(title, &f1, Color::WHITE, RectangleF::new(x, y, inner, title_h), &plain);
    hooks.text(g, title, &f1, RectangleF::new(x, y, inner, title_h), &plain);
    y += title_h + gap * 2.0;
    let names = ["Resume", if show_controls { "Hide controls" } else { "Controls" }, if confirm_close { "Press again to close" } else { "Close game" }];
    let mid = StringFormat { alignment: Near, line_alignment: Center, no_wrap: true, line_limit: false, ellipsis: true };
    for i in 0..3 {
        let r = RectangleF::new(x, y, inner, row_h);
        let on = i as i32 == sel;
        let fill = if on { if i == 2 && confirm_close { Color::rgb(214, 64, 52) } else { accent_c } } else { Color::rgb(44, 47, 58) };
        g.fill_path(&solid(fill), &round(r, row_h / 2.0));
        let nr = RectangleF::new(r.x + row_h * 0.45, r.y, r.width - row_h * 0.9, r.height);
        hooks.text(g, names[i], &f2, nr, &mid);
        g.draw_string(names[i], &f2, Color::WHITE, nr, &mid);
        hits.push(r);
        y += row_h + gap;
        if i == 1 && show_controls {
            let bx = RectangleF::new(x, y, inner, text_h);
            g.fill_path(&solid(Color::rgb(20, 22, 28)), &round(bx, pad * 0.4));
            let tr = RectangleF::new(bx.x + pad / 2.0, bx.y + pad / 2.0, bx.width - pad, bx.height - pad);
            g.draw_string(controls, &f3, Color::rgb(214, 218, 228), tr, &plain);
            hooks.text(g, controls, &f3, tr, &plain);
            y += text_h + gap;
        }
    }
    y += gap;
    if confirm_close {
        let lost = RectangleF::new(x, y, inner, f3.height() as f32 * 1.4);
        let lf = StringFormat { alignment: Center, line_alignment: Near, no_wrap: false, line_limit: false, ellipsis: false };
        hooks.text(g, "Anything since your last save in the game is lost.", &f3, lost, &lf);
        g.draw_string("Anything since your last save in the game is lost.", &f3, Color::rgb(255, 176, 166), lost, &lf);
        y += f3.height() as f32 * 1.4;
    }
    let hint_r = RectangleF::new(x, y, inner, hint_h);
    let hf = StringFormat { alignment: Center, line_alignment: Center, no_wrap: false, line_limit: false, ellipsis: false };
    hooks.text(g, "A  choose      B  back      Guide  resume", &f3, hint_r, &hf);
    g.draw_string("A  choose      B  back      Guide  resume", &f3, Color::from_argb(140, 255, 255, 255), hint_r, &hf);
    hits
}

