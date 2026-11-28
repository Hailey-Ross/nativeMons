//! The windowed launcher: one launcher per folder, the SDL3 window, and the tick loop that feeds nm-ui's state
//! machine and draws only when it asks (Light mode: at most 30 frames a second).

use nm_platform::instance;
use std::time::{Duration, Instant};

/// The shortcut flags of the C# Main: --game (or --play) <id>, --slot <save>, --launcher.
pub struct Flags { pub game: Option<String>, pub slot: Option<String>, pub menu: bool }

pub fn flags(argv: &[String]) -> Flags {
    let flag = |name: &str| argv.iter().position(|a| a == name).and_then(|i| argv.get(i + 1)).cloned();
    Flags { game: flag("--game").or_else(|| flag("--play")), slot: flag("--slot"), menu: argv.iter().any(|a| a == "--launcher") }
}

pub const TICK: Duration = Duration::from_millis(16);
/// Behind a game, with the launcher hidden: the game watch only.
pub const HIDDEN_TICK: Duration = Duration::from_millis(100);
/// Light mode's frame gap, from one frame's start to the next: every second 16 ms tick at most (~31 fps; a full
/// 33.3 ms gap, timed from the end of a frame, let only every third through: ~21 fps in the window test).
pub const LIGHT_FRAME: Duration = Duration::from_millis(28);

/// When the loop may draw a frame the state machine asked for.
pub fn may_draw(asked: bool, visible: bool, light: bool, since_last: Duration) -> bool {
    asked && visible && (!light || since_last >= LIGHT_FRAME)
}
pub fn tick_for(visible: bool) -> Duration { if visible { TICK } else { HIDDEN_TICK } }

/// One process scan per tick, shared by every check that asks in that tick (RunningGames and the like).
#[derive(Default)]
pub struct ProcScan { tick: u64, list: Option<Vec<nm_core::proc::ProcInfo>> }
impl ProcScan {
    pub fn get(&mut self, tick: u64) -> &[nm_core::proc::ProcInfo] {
        if self.tick != tick || self.list.is_none() { self.list = Some(nm_core::proc::processes()); self.tick = tick; }
        self.list.as_deref().unwrap()
    }
}

/// The launcher's start: Some(exit code) when another launcher of this folder took the request.
pub fn single_instance(argv: &[String]) -> Result<instance::Instance, i32> {
    let state = nm_core::config::state();
    let test_state = std::env::var_os("NATIVEMONS_STATE").map(|_| state.clone());
    let name = instance::mutex_name(&nm_core::config::here(), test_state.as_deref());
    if let Some(i) = instance::acquire(&name, &state) { return Ok(i); }
    let f = flags(argv);
    if let Some(g) = &f.game { let _ = instance::write_request(&state, f.menu, g, f.slot.as_deref()); }
    instance::ring_others(f.game.is_some(), test_state.is_none(), &state);
    Err(0)
}

/// NATIVEMONS_TEST_LOG for the window test (Rust only): window, frames, pages, input and effects, by ms since start;
/// NATIVEMONS_TEST_FRAMES=<folder>: the frame on screen once each page settles, and the last one, as PNG.
pub struct LiveLog { file: Option<std::fs::File>, t0: Instant, frames: Option<std::path::PathBuf>, key: String, changed: Option<Instant>, n: u32, drawn: u32, items: Vec<String> }
impl LiveLog {
    pub fn from_env() -> LiveLog {
        let file = std::env::var("NATIVEMONS_TEST_LOG").ok().and_then(|f| std::fs::OpenOptions::new().create(true).append(true).open(f).ok());
        let frames = std::env::var("NATIVEMONS_TEST_FRAMES").ok().map(std::path::PathBuf::from);
        if let Some(d) = &frames { let _ = std::fs::create_dir_all(d); }
        LiveLog { file, t0: Instant::now(), frames, key: String::new(), changed: None, n: 0, drawn: 0, items: Vec::new() }
    }
    pub fn line(&mut self, s: &str) {
        use std::io::Write;
        let ms = self.t0.elapsed().as_millis();
        if let Some(f) = self.file.as_mut() { let _ = write!(f, "{} {}
", ms, s); }
    }
    /// After a frame: a page change is noted, and its frame saved 0.6 s after it settles. A frame over 16 ms says why it
    /// may have been slow (`redrawn`: the share of the window redrawn; `items`: the top page's items, logged when they change).
    pub fn frame(&mut self, key: &str, bmp: &nm_draw::Bitmap, took: Duration, redrawn: f64, items: &[String]) {
        if self.file.is_none() { return; }
        let ms = took.as_secs_f64() * 1000.0;
        self.drawn += 1;
        self.line(&format!("frame {:.2} ms", ms));
        if ms > 16.0 {
            let mut why = vec![format!("redrawn {:.0}%", redrawn * 100.0)];
            if self.drawn == 1 { why.push("first frame".into()); }
            if key != self.key { why.push(format!("page change {:?} -> {:?}", self.key, key)); }
            for (k, w) in [(" (starting)", "splash"), ("(job: ", "job running"), (" (capturing)", "capture running")] { if key.contains(k) { why.push(w.into()); } }
            self.line(&format!("slow frame {:.2} ms (frame {}): {}", ms, self.drawn, why.join(", ")));
        }
        if key != self.key {
            self.key = key.to_string(); self.changed = Some(Instant::now()); self.line(&format!("page {}", key));
        }
        if items != self.items.as_slice() {
            self.items = items.to_vec();
            self.line(&format!("items {}", items.iter().enumerate().map(|(i, t)| format!("{}: {}", i, t)).collect::<Vec<_>>().join(" | ")));
        }
        if self.changed.map_or(false, |c| c.elapsed() >= Duration::from_millis(600)) { self.changed = None; self.save(bmp, "page"); }
    }
    pub fn on(&self) -> bool { self.file.is_some() }
    pub fn save(&mut self, bmp: &nm_draw::Bitmap, what: &str) {
        let Some(d) = self.frames.clone() else { return };
        self.n += 1;
        let f = d.join(format!("{:03}-{}.png", self.n, what));
        let _ = bmp.save_png(&f);
        self.line(&format!("saved {}", f.display()));
    }
}

/// What the tick loop needs from the window: SDL's (the `window` feature), or a fake in the headless tests.
pub trait Surface {
    fn events(&mut self) -> Vec<nm_platform::input::Event>;
    fn pad(&self) -> u16;
    fn pad_count(&self) -> usize;
    fn visible(&self) -> bool;
    fn focused(&self) -> bool;
    fn position(&self) -> (i32, i32);
    fn set_position(&mut self, x: i32, y: i32);
    fn set_bounds(&mut self, x: i32, y: i32, w: i32, h: i32);
    fn set_outline(&mut self, x: i32, w: i32, h: i32, radius: f32);
    fn present(&mut self, pixels: &[u32], w: i32, h: i32);
    fn show(&mut self);
    fn hide(&mut self);
    fn minimize(&mut self);
    fn begin_drag(&mut self);
    fn set_hand(&mut self, on: bool);
    fn open_url(&mut self, url: &str);
    /// Clipboard.SetText.
    fn set_clipboard(&mut self, _text: &str) {}
    /// The guide overlay: the guide button, its window (size), its events, a frame, and its close.
    /// Dialogs: a picker (answered later), the save-name box (modal), and a picker's answer once it came.
    fn pick(&mut self, _folder: bool, _title: &str, _filters: &[(String, String)]) {}
    fn ask_text(&mut self, _title: &str, _prompt: &str, _start: &str) -> Option<String> { None }
    fn dialog_answer(&mut self) -> Option<Option<String>> { None }
    fn guide_down(&self) -> bool { false }
    /// The launcher's own window (the window test opens the guide over it).
    fn own_window(&self) -> isize { 0 }
    fn guide_open(&mut self, _game: isize) -> Option<(i32, i32)> { None }
    fn guide_events(&mut self) -> Vec<nm_platform::input::Event> { Vec::new() }
    fn guide_present(&mut self, _pixels: &[u32], _w: i32, _h: i32) {}
    fn guide_close(&mut self, _game: isize, _close_game: bool) {}
    /// The tick's wait; a fake moves its own clock instead.
    fn delay(&mut self, seconds: f32);
    /// A fake clock (seconds) for the launcher, or None for the real one.
    fn clock(&self) -> Option<f32> { None }
}

/// The screen the window is on, for the power-off fall and the morph.
/// `headless`: the window test's dry run (nothing opened or restarted).
pub struct Place { pub area: nm_platform::geometry::Rect, pub screen: nm_platform::geometry::Rect, pub light_auto: bool, pub headless: bool }

/// Why the loop ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Exit { PoweredOff, Close }

/// NATIVEMONS_TEST_INPUT=<file>: scripted input for the window test, fed into the same event queue as SDL's, so no
/// real or posted mouse is needed. Lines `<ms> key <vk hex|name>`, `<ms> move|down|up|click <x> <y> [left|right|middle]`,
/// `<ms> clickitem <i>` (the middle of item i's hit rect, as the replays), `<ms> clicktext <text>` (the top page's first
/// item whose text starts with it), `<ms> waitpage <title>` (later steps wait until the top page's title starts with it,
/// at most 10 s, then run as if it had come at <ms>), `<ms> wheel <delta>`, `<ms> close`; ms since the window showed.
pub struct ScriptedInput { steps: std::collections::VecDeque<(u64, Step)>, shift: u64, waiting: Option<(String, u64, u64)> }

#[derive(Clone, Debug, PartialEq)]
pub enum Step { Ev(nm_platform::input::Event), ClickItem(i32), ClickText(String), WaitPage(String), PageCame(String, u64), PageTimedOut(String, String) }
impl ScriptedInput {
    pub fn from_env() -> Option<ScriptedInput> {
        let f = std::env::var("NATIVEMONS_TEST_INPUT").ok()?;
        Some(ScriptedInput::parse(&std::fs::read_to_string(f).ok()?))
    }
    pub fn parse(text: &str) -> ScriptedInput {
        use nm_platform::input::{Event, MouseButton};
        let mut steps = std::collections::VecDeque::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
            if f.len() < 2 { continue; }
            let Ok(ms) = f[0].parse::<u64>() else { continue };
            let n = |i: usize| f.get(i).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let b = match f.get(4).map(|s| s.to_ascii_lowercase()) { Some(s) if s == "right" => MouseButton::Right, Some(s) if s == "middle" => MouseButton::Middle, _ => MouseButton::Left };
            let e = match f[1] {
                "key" => {
                    let k = f.get(2).copied().unwrap_or("");
                    let vk = u32::from_str_radix(k.trim_start_matches("0x"), 16).ok().filter(|_| k.starts_with("0x")).or_else(|| vk_of(k));
                    match vk { Some(v) => Event::Key(v), None => continue }
                }
                "move" => Event::MouseMove { x: n(2), y: n(3), gx: n(2), gy: n(3) },
                "down" => Event::MouseDown { x: n(2), y: n(3), b },
                "up" => Event::MouseUp { x: n(2), y: n(3), b },
                "click" => Event::Click { x: n(2), y: n(3), b },
                "wheel" => Event::Wheel(n(2)),
                "close" => Event::Quit,
                "clickitem" => { steps.push_back((ms, Step::ClickItem(n(2)))); continue; }
                "clicktext" | "waitpage" if f.len() > 2 => {
                    let t = f[2..].join(" ");
                    steps.push_back((ms, if f[1] == "clicktext" { Step::ClickText(t) } else { Step::WaitPage(t) }));
                    continue;
                }
                _ => continue,
            };
            steps.push_back((ms, Step::Ev(e)));
        }
        ScriptedInput { steps, shift: 0, waiting: None }
    }
    /// The steps due by `ms` (no page waits).
    #[cfg(test)]
    pub fn due(&mut self, ms: u64) -> Vec<Step> { self.due_on(ms, "") }
    /// The steps due by `ms` with `top` the top page's title; a met or timed-out wait is returned as a step for the log.
    pub fn due_on(&mut self, ms: u64, top: &str) -> Vec<Step> {
        let mut out = Vec::new();
        loop {
            if let Some((t, at, since)) = self.waiting.clone() {
                if top.to_lowercase().starts_with(&t.to_lowercase()) { self.shift += ms.saturating_sub(at); out.push(Step::PageCame(t, ms.saturating_sub(since))); }
                else if ms.saturating_sub(since) >= 10_000 { self.shift += ms.saturating_sub(at); out.push(Step::PageTimedOut(t, top.to_string())); }
                else { break; }
                self.waiting = None;
            }
            if !self.steps.front().map_or(false, |s| s.0 + self.shift <= ms) { break; }
            let (at, st) = self.steps.pop_front().unwrap();
            match st {
                Step::WaitPage(t) => self.waiting = Some((t, at + self.shift, ms)),
                st => out.push(st),
            }
        }
        out
    }
}

/// A key name as the replay scripts write it, as its virtual key.
fn vk_of(name: &str) -> Option<u32> {
    Some(match name.to_ascii_lowercase().as_str() {
        "up" => 0x26, "down" => 0x28, "left" => 0x25, "right" => 0x27, "enter" | "return" => 0x0D, "escape" | "esc" => 0x1B,
        "back" | "backspace" => 0x08, "space" => 0x20, "tab" => 0x09, "pageup" => 0x21, "pagedown" => 0x22,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => s.to_ascii_uppercase().as_bytes()[0] as u32,
        _ => return None,
    })
}

#[cfg(feature = "window")]
impl Surface for nm_platform::window::Window {
    fn events(&mut self) -> Vec<nm_platform::input::Event> { nm_platform::window::Window::events(self) }
    fn pad(&self) -> u16 { nm_platform::window::Window::pad(self) }
    fn pad_count(&self) -> usize { nm_platform::window::Window::pad_count(self) }
    fn visible(&self) -> bool { nm_platform::window::Window::visible(self) }
    fn focused(&self) -> bool { nm_platform::window::Window::focused(self) }
    fn position(&self) -> (i32, i32) { nm_platform::window::Window::position(self) }
    fn set_position(&mut self, x: i32, y: i32) { nm_platform::window::Window::set_position(self, x, y) }
    fn set_bounds(&mut self, x: i32, y: i32, w: i32, h: i32) { nm_platform::window::Window::set_bounds(self, x, y, w, h) }
    fn set_outline(&mut self, x: i32, w: i32, h: i32, radius: f32) { nm_platform::window::Window::set_outline(self, x, w, h, radius) }
    fn present(&mut self, pixels: &[u32], w: i32, h: i32) { nm_platform::window::Window::present(self, pixels, w, h) }
    fn show(&mut self) { nm_platform::window::Window::show(self) }
    fn hide(&mut self) { nm_platform::window::Window::hide(self) }
    fn minimize(&mut self) { nm_platform::window::Window::minimize(self) }
    fn begin_drag(&mut self) { nm_platform::window::Window::begin_drag(self) }
    fn set_hand(&mut self, on: bool) { nm_platform::window::Window::set_hand(self, on) }
    fn open_url(&mut self, url: &str) { nm_platform::window::Window::open_url(url) }
    fn set_clipboard(&mut self, text: &str) { nm_platform::window::Window::set_clipboard(text) }
    fn pick(&mut self, folder: bool, title: &str, filters: &[(String, String)]) { nm_platform::window::Window::pick(self, folder, title, filters) }
    fn ask_text(&mut self, title: &str, prompt: &str, start: &str) -> Option<String> { nm_platform::window::Window::ask_text(self, title, prompt, start) }
    fn dialog_answer(&mut self) -> Option<Option<String>> { nm_platform::dialogs::take_answer() }
    fn guide_down(&self) -> bool { nm_platform::window::Window::guide_down(self) }
    #[cfg(windows)]
    fn own_window(&self) -> isize { nm_platform::window::Window::hwnd(self) }
    fn guide_open(&mut self, game: isize) -> Option<(i32, i32)> { nm_platform::window::Window::guide_open(self, game) }
    fn guide_events(&mut self) -> Vec<nm_platform::input::Event> { nm_platform::window::Window::guide_events(self) }
    fn guide_present(&mut self, pixels: &[u32], w: i32, h: i32) { nm_platform::window::Window::guide_present(self, pixels, w, h) }
    fn guide_close(&mut self, game: isize, close_game: bool) { nm_platform::window::Window::guide_close(self, game, close_game) }
    fn delay(&mut self, seconds: f32) { nm_platform::window::Window::delay(seconds) }
}

#[cfg(feature = "window")]
pub fn run(argv: &[String]) -> i32 {
    use nm_platform::geometry::{self, Rect};
    use nm_platform::window::{self, Window};
    let instance = match single_instance(argv) { Ok(i) => i, Err(code) => return code };
    if let Err(e) = window::init() { eprintln!("nativemons: {}", e); return 1; }
    let displays = window::displays();
    let Some(di) = window::pick_display(&displays, std::env::var("NATIVEMONS_TEST_SCREEN").ok().as_deref()) else { eprintln!("nativemons: no display"); return 1 };
    let (area, screen): (Rect, Rect) = (displays[di].usable, displays[di].bounds);
    let deck = std::env::var("SteamDeck").as_deref() == Ok("1");
    let wide = geometry::want_wide(area, &std::env::var("NATIVEMONS_LAYOUT").unwrap_or_default(), &nm_core::config::read("layout.txt", "auto"), deck);
    let (w, h) = geometry::window_size(area, wide, deck);
    let pos = geometry::centered(area, (w, h));
    let (light_auto, light_why) = nm_platform::light::detect();
    let mut host = nm_ui::launcher::Host::new((area.w, area.h));
    host.light_auto = light_auto;
    host.light_why = light_why;
    let mut l = start(w, h, host);
    // A shortcut straight to a game (--play/--game without --launcher): it starts once the start is done.
    let f = flags(argv);
    if !f.menu { l.jobs.play_direct = f.game.clone(); }
    let mut win = match Window::create((w, h), pos) { Ok(x) => x, Err(e) => { eprintln!("nativemons: {}", e); return 1 } };
    let mut tlog = LiveLog::from_env();
    tlog.line(&format!("hwnd {}", win.hwnd()));
    tlog.line(&format!("display {} {} usable {} {} {} {} window {} {} {} {} light {} ({})", displays[di].device, displays[di].name, area.x, area.y, area.w, area.h, pos.0, pos.1, w, h, light_auto, light_why));
    Surface::set_outline(&mut win, 0, w, h, geometry::corner_radius(w, h));
    let place = Place { area, screen, light_auto, headless: false };
    let exit = run_loop(&mut l, &mut win, &place, &mut tlog, Some(&instance), (w, h), ScriptedInput::from_env());
    tlog.line(&format!("exit {:?}", exit));
    0
}

/// The tick loop: events (SDL's, the scripted ones), the state machine, effects, the power-off fall, and a frame
/// whenever Poll asks (Light mode: at most 30 a second). Returns once the launcher powered off or closed.
/// The guide overlay while it's open: the game's window, its state, its size and whether it needs a new frame.
pub struct GuideShown { game: isize, guide: nm_ui::guide::Guide, size: (i32, i32), dirty: bool }
/// GuideTick, then the open overlay's input (pad every tick, keys and mouse from its window) and its frame.
fn guide_tick<S: Surface>(l: &mut nm_ui::launcher::Launcher, win: &mut S, guide: &mut Option<GuideShown>, tlog: &mut LiveLog) {
    use nm_ui::guide::GuideOut;
    let gd = win.guide_down();
    // Window test only (NATIVEMONS_TEST_GUIDE=<ms>): the overlay over the launcher's own screen once, as if a game ran.
    if guide.is_none() && !l.starting {
        if let Some(ms) = std::env::var("NATIVEMONS_TEST_GUIDE").ok().and_then(|v| v.parse::<f32>().ok()) {
            static OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if l.now() * 1000.0 >= ms && !OPENED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                let own = win.own_window();
                let th = l.cur();
                let g = nm_ui::guide::Guide::new("Pokemon Platinum", &nm_ui::pages::controls_for("port"), nm_ui::launcher::accent(&th));
                if let Some(size) = win.guide_open(own) { tlog.line(&format!("guide open {}x{} (test)", size.0, size.1)); *guide = Some(GuideShown { game: own, guide: g, size, dirty: true }); }
            }
        }
    }
    if let Some(open) = l.guide_tick(gd, guide.is_some()) {
        if let Some(size) = win.guide_open(open.game_window) {
            tlog.line(&format!("guide open {}x{}", size.0, size.1));
            *guide = Some(GuideShown { game: open.game_window, guide: open.guide, size, dirty: true });
        }
    }
    let Some(gs) = guide.as_mut() else { return };
    let mut outs = vec![gs.guide.poll(win.pad(), gd)];
    for e in win.guide_events() {
        use nm_platform::input::{Event, MouseButton};
        tlog.line(&format!("guide event {:?}", e));
        outs.push(match e {
            Event::Key(vk) => gs.guide.key(nm_ui::input::Key::from_vk(vk)),
            Event::MouseMove { x, y, .. } => gs.guide.mouse_move(x as f32, y as f32),
            Event::Click { x, y, b } => gs.guide.click(x as f32, y as f32, b == MouseButton::Right),
            Event::Quit => GuideOut::Resume,
            _ => GuideOut::Stay,
        });
    }
    for o in outs {
        match o {
            GuideOut::Stay => {}
            GuideOut::Redraw => gs.dirty = true,
            GuideOut::Resume | GuideOut::CloseGame => {
                let close = o == GuideOut::CloseGame;
                tlog.line(if close { "guide close game" } else { "guide resume" });
                win.guide_close(gs.game, close);
                *guide = None;
                return;
            }
        }
    }
    if gs.dirty {
        gs.dirty = false;
        let mut g = nm_draw::Graphics::new(nm_draw::Bitmap::new(gs.size.0, gs.size.1));
        gs.guide.draw(&mut l.hooks, &mut g, gs.size);
        win.guide_present(&g.bmp.data, gs.size.0, gs.size.1);
    }
}

pub fn run_loop<S: Surface>(l: &mut nm_ui::launcher::Launcher, win: &mut S, place: &Place, tlog: &mut LiveLog, instance: Option<&instance::Instance>, size: (i32, i32), mut script: Option<ScriptedInput>) -> Exit {
    use nm_platform::input::{Event, MouseButton};
    use nm_ui::input::{Button, Effect, Key};
    let (area, screen) = (place.area, place.screen);
    let (mut cw, mut ch) = size;
    l.morph.area = (area.x, area.y, area.w, area.h);
    // The display's working area for windows the launcher places (Zakuro's).
    l.jobs.work_rect = Some((area.x, area.y, area.w, area.h));
    let state = nm_core::config::state();
    let mut bmp = nm_draw::Bitmap::new(cw, ch);
    // OnPaint: mid-morph only the drawn device's part of the window is drawn (Launcher::paint). The frame is recorded
    // and only what changed since the last one is redrawn (pixels as a full redraw; NATIVEMONS_TEST_FULL_FRAMES=1: all).
    let full_frames = std::env::var("NATIVEMONS_TEST_FULL_FRAMES").as_deref() == Ok("1");
    let mut ops: Vec<nm_draw::graphics::Op> = Vec::new();
    let mut draw = |l: &mut nm_ui::launcher::Launcher, win: &mut S, bmp: &mut nm_draw::Bitmap, w: i32, h: i32, force: bool| -> f64 {
        let old = std::mem::replace(bmp, nm_draw::Bitmap::new(0, 0));
        if (old.width, old.height) != (w, h) || full_frames { ops.clear(); }
        let mut g = nm_draw::Graphics::new(if (old.width, old.height) == (w, h) { old } else { nm_draw::Bitmap::new(w, h) });
        g.begin_record();
        l.paint(&mut g, w, h);
        let (o, dirty) = g.flush(&ops);
        ops = o;
        if !dirty.is_empty() || force { win.present(&g.bmp.data, w, h); }
        *bmp = g.bmp;
        dirty.iter().map(|d| ((d.2 - d.0).max(0) as f64) * ((d.3 - d.1).max(0) as f64)).sum::<f64>() / ((w as f64) * (h as f64)).max(1.0)
    };
    if let Some(t) = win.clock() { l.hooks.t = Some(t); }
    draw(l, win, &mut bmp, cw, ch, true);
    win.show();
    let mut exposed = false;
    let (shown, shown_t) = (Instant::now(), win.clock().unwrap_or(0.0));
    // Frame pacing on the surface's clock (the fake's in tests), from each frame's start.
    let clock = |win: &S| win.clock().map_or(shown.elapsed().as_secs_f64(), |t| (t - shown_t) as f64);
    let (mut last_frame, mut light_read, mut tick) = (clock(win), Instant::now(), 0u64);
    let mut power_top: Option<i32> = None;
    let mut want = false;
    let mut scan = ProcScan::default();
    let mut guide: Option<GuideShown> = None;
    let b = |b: MouseButton| match b { MouseButton::Left => Button::Left, MouseButton::Right => Button::Right, MouseButton::Middle => Button::Middle };
    loop {
        let started = Instant::now();
        tick += 1;
        if let Some(t) = win.clock() { l.hooks.t = Some(t); }
        if tick == 1 { tlog.line(&format!("pads {}", win.pad_count())); }
        let mut events = win.events();
        if let Some(s) = script.as_mut() {
            let ms = win.clock().map_or(shown.elapsed().as_millis() as u64, |t| ((t - shown_t).max(0.0) * 1000.0).round() as u64);
            let top = l.pages.last().map_or(String::new(), |p| p.title.clone());
            for st in s.due_on(ms, &top) {
                tlog.line(&format!("scripted {:?}", st));
                let st = match st {
                    Step::ClickText(t) => match l.pages.last().and_then(|p| p.items.iter().position(|it| it.text.to_lowercase().starts_with(&t.to_lowercase()))) {
                        Some(i) => Step::ClickItem(i as i32),
                        None => { tlog.line(&format!("scripted: no item {:?} on {:?}", t, top)); continue; }
                    },
                    st => st,
                };
                match st {
                    Step::Ev(e) => events.push(e),
                    Step::ClickText(_) | Step::WaitPage(_) | Step::PageCame(..) | Step::PageTimedOut(..) => {}
                    Step::ClickItem(i) => match l.hits.iter().find(|h| h.1 == i).map(|h| h.0) {
                        Some(r) => {
                            let (x, y) = ((r.x + r.width / 2.0 + l.screen_x) as i32, (r.y + r.height / 2.0 + l.screen_y) as i32);
                            let (wx, wy) = win.position();
                            let b = MouseButton::Left;
                            events.extend([Event::MouseMove { x, y, gx: wx + x, gy: wy + y }, Event::MouseDown { x, y, b }, Event::MouseUp { x, y, b }, Event::Click { x, y, b }]);
                        }
                        None => tlog.line(&format!("scripted: no hit rect for item {}", i)),
                    },
                }
            }
        }
        // One close per tick: SDL reports a close request and its quit together.
        let mut closed = false;
        for e in events {
            tlog.line(&format!("event {:?}", e));
            match e {
                Event::Quit => {
                    if closed { continue; }
                    closed = true;
                    // FormClosing: only a shown, normal window powers down first; otherwise it closes now.
                    if win.visible() { l.request_close(); } else { l.closing = true; l.effects.push(Effect::Close); }
                }
                Event::Key(vk) => { l.key(Key::from_vk(vk)); }
                Event::MouseMove { x, y, gx, gy } => l.mouse_move(x, y, (gx, gy)),
                Event::MouseDown { x, y, b: k } => l.mouse_down(x, y, b(k)),
                Event::MouseUp { x, y, b: k } => l.mouse_up(x, y, b(k)),
                Event::Click { x, y, b: k } => l.mouse_click(x, y, b(k)),
                Event::Wheel(d) => l.mouse_wheel(d),
                Event::Request => { if let Some(r) = instance::take_request(&state) { request(l, &r); } }
                Event::Focus(_) => l.dirty = true,
                Event::Exposed => { l.dirty = true; exposed = true; }
                Event::Drop(paths) => l.take_drop(paths),
            }
        }
        if instance.map_or(false, |i| i.rang()) { if let Some(r) = instance::take_request(&state) { request(l, &r); } }
        // Light mode follows the Settings switch, read every 2 s instead of every tick.
        if light_read.elapsed() >= Duration::from_secs(2) {
            light_read = Instant::now();
            l.light = nm_ui::light::effective(nm_ui::light::read(), place.light_auto);
        }
        let visible = win.visible();
        let _running = scan.get(tick);
        let (wx, wy) = win.position();
        l.morph.window = (wx, wy, cw, ch);
        l.start_tick(cw, ch);
        want |= l.poll(if visible { win.pad() } else { 0 }, win.focused());
        guide_tick(l, win, &mut guide, tlog);
        // Window test only (NATIVEMONS_TEST_DIALOG="<ms> folder|file|text"): one dialog, its answer logged.
        if !l.starting {
            if let Some((ms, kind)) = std::env::var("NATIVEMONS_TEST_DIALOG").ok().and_then(|v| { let mut p = v.split_whitespace(); Some((p.next()?.parse::<f32>().ok()?, p.next()?.to_string())) }) {
                static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if l.now() * 1000.0 >= ms && !ASKED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    match kind.as_str() {
                        "folder" => l.pick_folder("Window test: pick a folder", Box::new(|_, _| {})),
                        "file" => l.pick_file("Window test: pick a save", "Cartridge save (*.sav)|*.sav|All files (*.*)|*.*", Box::new(|_, _| {})),
                        _ => l.ask_text("Window test", "Name for this save:", "Red", Box::new(|_, _| {})),
                    }
                }
            }
        }
        if let Some(a) = win.dialog_answer() { tlog.line(&match &a { Some(p) => format!("dialog picked {:?}", p), None => "dialog canceled".to_string() }); l.dialog_done(a); }
        if l.power_off >= 0.0 && power_top.is_none() { power_top = Some(win.position().1); tlog.line("power-off"); }
        if let (Some(k), Some(top)) = (l.power_fall(), power_top) {
            let (y, gone) = nm_ui::input::power_drop(k, top, area.bottom(), screen.bottom(), ch);
            let x = win.position().0;
            win.set_position(x, y);
            if gone { l.powered_off = true; tlog.line("exit powered off"); return Exit::PoweredOff; }
        }
        for e in std::mem::take(&mut l.effects) {
            if !matches!(e, Effect::HandCursor(_)) { tlog.line(&format!("effect {:?}", e)); }
            match e {
                Effect::Close => { tlog.save(&bmp, "last"); tlog.line("exit close"); return Exit::Close; }
                Effect::Minimize => win.minimize(),
                Effect::DragWindow => win.begin_drag(),
                Effect::HandCursor(on) => win.set_hand(on),
                Effect::OpenUrl(u) => win.open_url(&u),
                Effect::OpenFolder(p) => if !place.headless { open_folder(&p) },
                Effect::Todo(what) => eprintln!("nativemons: not ported yet: {}", what),
                Effect::Skip(_) => {}
                Effect::Dialog(_) => {}
                Effect::SetBounds { x, y, w, h } => { win.set_bounds(x, y, w, h); cw = w; ch = h; l.morph.window = (x, y, w, h); }
                Effect::SetRegion { x, w, h, radius } => win.set_outline(x, w, h, radius),
                Effect::Launch(what) => eprintln!("nativemons: would start {}", what),
                Effect::ShowWindow => win.show(),
                Effect::HideWindow => win.hide(),
                Effect::Clipboard(text) => win.set_clipboard(&text),
                Effect::PickFolder { title } => { tlog.line(&format!("dialog open folder: {}", title)); win.pick(true, &title, &[]) }
                Effect::PickFile { title, filters } => { tlog.line(&format!("dialog open file: {}", title)); win.pick(false, &title, &filters) }
                Effect::AskText { title, prompt, start } => { let a = win.ask_text(&title, &prompt, &start); tlog.line(&format!("asktext {:?}", a)); l.dialog_done(a); }
                Effect::Fetch(_) | Effect::Gen1Cli(_) => {}
                // The updated copy starts, then this one closes (the C#'s Process.Start, then Close).
                Effect::Restart => { if let (false, Ok(exe)) = (place.headless, std::env::current_exe()) { let _ = std::process::Command::new(exe).spawn(); } tlog.line("exit restart"); return Exit::Close; }
            }
        }
        if may_draw(want, visible, l.light, Duration::from_secs_f64((clock(win) - last_frame).max(0.0))) {
            let t0 = Instant::now();
            last_frame = clock(win);
            let redrawn = draw(l, win, &mut bmp, cw, ch, std::mem::take(&mut exposed));
            let took = t0.elapsed();
            let key = format!("{}|{}{}{}", l.pages.len(), l.pages.last().map_or(String::new(), |p| p.title.clone()), if l.job { format!(" (job: {})", l.job_title) } else if l.starting { " (starting)".into() } else { String::new() },
                if nm_core::art_jobs::capturing() { " (capturing)" } else { "" });
            let items: Vec<String> = if tlog.on() { l.pages.last().map_or(Vec::new(), |p| p.items.iter().map(|it| it.text.clone()).collect()) } else { Vec::new() };
            tlog.frame(&key, &bmp, took, redrawn, &items);
            want = false;
        }
        if !visible { want = false; }
        win.delay(tick_for(visible).saturating_sub(started.elapsed()).as_secs_f32());
    }
}

/// Start() without its splash and background jobs yet: the library scanned, the current game picked, the home screen.
#[cfg(feature = "window")]
fn start(w: i32, h: i32, host: nm_ui::launcher::Host) -> nm_ui::launcher::Launcher {
    let mut l = nm_ui::launcher::Launcher::new(w, h, host);
    l.clock = Some(Instant::now());
    l.jobs.spawner = Some(Box::new(nm_platform::process::SystemSpawner));
    // Sprites on worker threads with a loading ball (their evolution lines are read in start_up).
    nm_ui::sprites::set_live(true);
    l.light = nm_ui::light::effective(nm_ui::light::read(), host.light_auto);
    l.start_up();
    l
}

/// Request(): back to the home screen, then play the game or show its page.
fn request(l: &mut nm_ui::launcher::Launcher, r: &instance::Request) {
    if l.starting || l.job { return; }
    while l.pages.len() > 1 && !l.pages.last().map_or(false, |p| p.main) { l.pages.pop(); }
    let Some(i) = l.game_index(&r.id).filter(|&i| l.ctx().lib.games[i].playable()) else {
        l.show_text(&format!("Can't start {}", r.id), "That game is not in the list, or its ROM was not found.");
        return;
    };
    if r.what == "play" { l.todo(&format!("PlayWithSlot {} {}", r.id, r.slot.clone().unwrap_or_default())); return; }
    l.ctx.as_mut().unwrap().current = Some(i);
    l.current_theme = Some(nm_ui::pages::theme_of(&l.ctx().lib.games[i]));
    l.replace_main();
    l.push_with(|c| nm_ui::pages::game_page(c, &c.lib.games[i]));
}

fn open_folder(p: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer.exe").arg(p).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

/// `--live-headless <seconds>`: the window test's run without a window (its dry run). The real start, the same env
/// (NATIVEMONS_TEST_OPEN, _INPUT, _LOG, stubs) and tick loop on a surface that shows nothing; nothing is started (a
/// game or tool start fails as "headless"), opened or restarted. Ends at the script's close or after the seconds.
pub fn run_headless(seconds: f32) -> i32 {
    use nm_platform::geometry::{self, Rect};
    struct Refuse;
    impl nm_core::proc::Spawner for Refuse {
        fn start(&mut self, psi: &nm_core::proc::StartInfo) -> std::io::Result<Box<dyn nm_core::proc::RunningJob>> { Err(std::io::Error::other(format!("headless: {} not started", psi.program.display()))) }
    }
    struct Headless { t0: Instant, limit: f32, shown: bool, pos: (i32, i32), quit: bool }
    impl Surface for Headless {
        fn events(&mut self) -> Vec<nm_platform::input::Event> { if !self.quit && self.t0.elapsed().as_secs_f32() >= self.limit { self.quit = true; return vec![nm_platform::input::Event::Quit]; } Vec::new() }
        fn pad(&self) -> u16 { 0 }
        fn pad_count(&self) -> usize { 0 }
        fn visible(&self) -> bool { self.shown }
        fn focused(&self) -> bool { true }
        fn position(&self) -> (i32, i32) { self.pos }
        fn set_position(&mut self, x: i32, y: i32) { self.pos = (x, y); }
        fn set_bounds(&mut self, x: i32, y: i32, _w: i32, _h: i32) { self.pos = (x, y); }
        fn set_outline(&mut self, _x: i32, _w: i32, _h: i32, _r: f32) {}
        fn present(&mut self, _p: &[u32], _w: i32, _h: i32) {}
        fn show(&mut self) { self.shown = true; }
        fn hide(&mut self) { self.shown = false; }
        fn minimize(&mut self) {}
        fn begin_drag(&mut self) {}
        fn set_hand(&mut self, _on: bool) {}
        fn open_url(&mut self, _u: &str) {}
        fn delay(&mut self, s: f32) { std::thread::sleep(Duration::from_secs_f32(s.max(0.0))); }
    }
    // Monitor 3's place as the window test uses it (1920x1080 right of the primary, taskbar below).
    let (area, screen) = (Rect { x: 1920, y: 0, w: 1920, h: 1040 }, Rect { x: 1920, y: 0, w: 1920, h: 1080 });
    let wide = geometry::want_wide(area, &std::env::var("NATIVEMONS_LAYOUT").unwrap_or_default(), &nm_core::config::read("layout.txt", "auto"), false);
    let (w, h) = geometry::window_size(area, wide, false);
    let pos = geometry::centered(area, (w, h));
    let mut l = nm_ui::launcher::Launcher::new(w, h, nm_ui::launcher::Host::new((area.w, area.h)));
    l.clock = Some(Instant::now());
    l.jobs.spawner = Some(Box::new(Refuse));
    nm_ui::sprites::set_live(true);
    l.light = nm_ui::light::effective(nm_ui::light::read(), false);
    l.start_up();
    let mut win = Headless { t0: Instant::now(), limit: seconds, shown: false, pos, quit: false };
    let mut tlog = LiveLog::from_env();
    tlog.line(&format!("headless window {} {} {} {}", pos.0, pos.1, w, h));
    let place = Place { area, screen, light_auto: false, headless: true };
    let exit = run_loop(&mut l, &mut win, &place, &mut tlog, None, (w, h), ScriptedInput::from_env());
    tlog.line(&format!("exit {:?}", exit));
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use nm_platform::geometry::Rect;
    use nm_platform::input::Event;

    /// A window that only records: its clock moves one 16 ms tick per wait; events come from a list by time.
    struct Fake { guide: (bool, Vec<String>, Vec<Event>), t: f32, t0: f32, pos: (i32, i32), shown: bool, hidden: bool, queue: Vec<(f32, Event)>, bounds: Vec<(i32, i32, i32, i32)>, outlines: Vec<(i32, i32, i32)>, frames: u32, ticks: u32, minimized: bool, falls: Vec<i32> }
    impl Fake {
        fn new(queue: Vec<(f32, Event)>) -> Fake { Fake { guide: (false, Vec::new(), Vec::new()), t: 10.0, t0: 10.0, pos: (660, 20), shown: false, hidden: false, queue, bounds: Vec::new(), outlines: Vec::new(), frames: 0, ticks: 0, minimized: false, falls: Vec::new() } }
    }
    impl Surface for Fake {
        fn events(&mut self) -> Vec<Event> {
            let t = self.t - self.t0;
            let (due, rest): (Vec<_>, Vec<_>) = self.queue.drain(..).partition(|e| e.0 <= t + 1e-4);
            self.queue = rest;
            due.into_iter().map(|e| e.1).collect()
        }
        fn pad(&self) -> u16 { 0 }
        fn pad_count(&self) -> usize { 0 }
        fn visible(&self) -> bool { self.shown && !self.hidden }
        fn focused(&self) -> bool { true }
        fn position(&self) -> (i32, i32) { self.pos }
        fn set_position(&mut self, x: i32, y: i32) { self.pos = (x, y); self.falls.push(y); }
        fn set_bounds(&mut self, x: i32, y: i32, w: i32, h: i32) { self.pos = (x, y); self.bounds.push((x, y, w, h)); }
        fn set_outline(&mut self, x: i32, w: i32, h: i32, _r: f32) { self.outlines.push((x, w, h)); }
        fn present(&mut self, _p: &[u32], _w: i32, _h: i32) { self.frames += 1; }
        fn show(&mut self) { self.shown = true; }
        fn hide(&mut self) { self.hidden = true; }
        fn minimize(&mut self) { self.minimized = true; }
        fn begin_drag(&mut self) {}
        fn set_hand(&mut self, _on: bool) {}
        fn open_url(&mut self, _u: &str) {}
        fn delay(&mut self, _s: f32) { self.t += 0.016; self.ticks += 1; assert!(self.ticks < 1500, "the loop never ended"); }
        fn clock(&self) -> Option<f32> { Some(self.t) }
        fn guide_down(&self) -> bool { self.guide.0 }
        fn guide_open(&mut self, game: isize) -> Option<(i32, i32)> { self.guide.1.push(format!("open {}", game)); Some((1280, 720)) }
        fn guide_events(&mut self) -> Vec<Event> { std::mem::take(&mut self.guide.2) }
        fn guide_present(&mut self, _p: &[u32], w: i32, h: i32) { self.guide.1.push(format!("frame {}x{}", w, h)); }
        fn guide_close(&mut self, game: isize, close: bool) { self.guide.1.push(format!("close {} {}", game, close)); }
    }
    fn place() -> Place { Place { area: Rect { x: 0, y: 0, w: 1920, h: 1040 }, screen: Rect { x: 0, y: 0, w: 1920, h: 1080 }, light_auto: false, headless: false } }
    /// The home screen as --snapshot sets it up; None (skipped) without the bundled fonts.
    fn home() -> Option<nm_ui::launcher::Launcher> {
        if std::env::var("NATIVEMONS_FONTS_DIR").is_err() { eprintln!("skipped: NATIVEMONS_FONTS_DIR not set"); return None; }
        // The launcher reads games.json beside its exe: the repo's, copied next to the test binary once.
        let beside = nm_core::config::here().join("games.json");
        if !beside.exists() { let _ = std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../launcher/games.json"), &beside); }
        Some(nm_ui::snapshot::launcher_for("main", 600, 1000, nm_ui::launcher::Host::new((1920, 1040))).unwrap())
    }
    fn run(l: &mut nm_ui::launcher::Launcher, f: &mut Fake, script: Option<ScriptedInput>) -> Exit {
        l.morph.window = (660, 20, 600, 1000);
        run_loop(l, f, &place(), &mut LiveLog::from_env(), None, (600, 1000), script)
    }

    struct GameJob;
    impl nm_core::proc::RunningJob for GameJob {
        fn id(&self) -> u32 { 9 }
        fn lines(&mut self) -> Vec<String> { Vec::new() }
        fn exit_code(&mut self) -> Option<i32> { None }
        fn kill(&mut self) {}
        fn window_shown(&mut self) -> bool { false }
        fn main_window(&mut self) -> Option<(isize, String)> { Some((77, "Pokemon Platinum".into())) }
    }
    #[test]
    fn guide_opens_over_a_game_and_closes_it_on_the_second_ask() {
        let Some(mut l) = home() else { return };
        let mut f = Fake::new(Vec::new());
        let mut log = LiveLog::from_env();
        let mut guide = None;
        // No game: the guide button does nothing.
        f.guide.0 = true;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        assert!(guide.is_none());
        f.guide.0 = false;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        l.job = true;
        l.job_is_game = true;
        l.jobs.proc = Some(Box::new(GameJob));
        f.guide.0 = true;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        assert!(guide.is_some());
        assert_eq!(f.guide.1, vec!["open 77".to_string(), "frame 1280x720".into()]);
        // Held from opening: not a second press. Up to Close game, Enter twice.
        f.guide.2 = vec![Event::Key(0x26), Event::Key(0x0D)];
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        assert!(guide.as_ref().unwrap().guide.confirm_close);
        f.guide.2 = vec![Event::Key(0x0D)];
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        assert!(guide.is_none());
        assert_eq!(f.guide.1.last().unwrap(), "close 77 true");
        // Open again (a fresh press), then the guide button resumes the game.
        f.guide.0 = false;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        f.guide.0 = true;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        f.guide.0 = false;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        f.guide.0 = true;
        guide_tick(&mut l, &mut f, &mut guide, &mut log);
        assert!(guide.is_none());
        assert_eq!(f.guide.1.last().unwrap(), "close 77 false");
    }

    #[test]
    fn close_powers_off_then_exits() {
        let Some(mut l) = home() else { return };
        // One posted WM_CLOSE reaches SDL as a close request plus a quit in the same tick: one power-off, not two closes.
        let mut f = Fake::new(vec![(0.5, Event::Quit), (0.5, Event::Quit)]);
        assert_eq!(run(&mut l, &mut f, None), Exit::PoweredOff);
        assert!(l.powered_off && (l.power_off - 10.5).abs() < 0.02, "{}", l.power_off);
        let took = f.t - 10.5;
        assert!(took > nm_ui::launcher::LIGHTS_AT + nm_ui::launcher::LIGHTS_TIME && took < 3.0, "{}", took);
        // The device hops, then falls below the working area.
        assert!(f.falls.iter().any(|&y| y < 20) && *f.falls.last().unwrap() > 1040, "{:?}", f.falls);
        assert!(f.frames > 10);
    }

    #[test]
    fn close_again_while_powering_off_closes_now() {
        let Some(mut l) = home() else { return };
        let mut f = Fake::new(vec![(0.5, Event::Quit), (0.55, Event::Quit)]);
        assert_eq!(run(&mut l, &mut f, None), Exit::Close);
        assert!(f.t - 10.5 < 0.1);
    }

    #[test]
    fn hidden_window_closes_at_once() {
        let Some(mut l) = home() else { return };
        let mut f = Fake::new(vec![(0.2, Event::Quit)]);
        f.hidden = true;
        assert_eq!(run(&mut l, &mut f, None), Exit::Close);
        assert!(l.power_off < 0.0 && f.t - 10.2 < 0.05);
    }

    #[test]
    fn posted_and_scripted_input_reach_the_launcher() {
        let Some(mut l) = home() else { return };
        // A posted WM_KEYDOWN (as the message hook queues it), then scripted mouse: a click on the M key minimizes.
        let mut f = Fake::new(vec![(0.2, Event::Key(0x28))]);
        let script = ScriptedInput::parse("# window test input\n300 move 526 31\n320 down 526 31 left\n340 up 526 31 left\n340 click 526 31 left\n600 close\n");
        assert_eq!(run(&mut l, &mut f, Some(script)), Exit::PoweredOff);
        assert_ne!(l.pages.last().unwrap().sel, 0);
        assert!(f.minimized);
    }

    #[test]
    fn scripted_clickitem_opens_settings() {
        let Some(mut l) = home() else { return };
        let mut f = Fake::new(Vec::new());
        let script = ScriptedInput::parse("500 wheel -120\n600 wheel 120\n700 clickitem 7\n2000 close\n");
        assert_eq!(run(&mut l, &mut f, Some(script)), Exit::PoweredOff);
        assert_eq!(l.pages.last().map(|p| p.title.as_str()), Some("Settings"));
    }

    #[test]
    fn light_mode_draws_every_second_tick() {
        let Some(mut l) = home() else { return };
        let mut f = Fake::new(vec![(2.0, Event::Quit)]);
        l.light = true;
        // ~3.2 s with the power-off: every second tick is ~100 frames (full rate ~200, every third tick ~67).
        assert_eq!(run(&mut l, &mut f, None), Exit::PoweredOff);
        assert!((80..=130).contains(&f.frames), "{} frames in {:.2} s", f.frames, f.t - 10.0);
    }

    #[test]
    fn scripted_input_parses() {
        let mut s = ScriptedInput::parse("100 key 0x28\n100 key enter\n200 wheel -120\n250 click 5 6 right\n270 clickitem 7\nbad line\n300 close # done\n");
        let ev = |v: Vec<Event>| v.into_iter().map(Step::Ev).collect::<Vec<_>>();
        assert_eq!(s.due(99), vec![]);
        assert_eq!(s.due(100), ev(vec![Event::Key(0x28), Event::Key(0x0D)]));
        assert_eq!(s.due(260), ev(vec![Event::Wheel(-120), Event::Click { x: 5, y: 6, b: nm_platform::input::MouseButton::Right }]));
        assert_eq!(s.due(280), vec![Step::ClickItem(7)]);
        assert_eq!(s.due(1000), ev(vec![Event::Quit]));
        assert!(s.steps.is_empty());
    }

    #[test]
    fn scripted_input_waits_for_pages() {
        let mut s = ScriptedInput::parse("100 waitpage Settings\n300 clicktext Import saves\n500 waitpage Nowhere\n600 close\n");
        assert_eq!(s.due_on(150, "Welcome"), vec![]);
        // Settings came at 1100 (950 ms after the wait began): the click (200 ms after the wait) is due at 1300.
        assert_eq!(s.due_on(1100, "Settings"), vec![Step::PageCame("Settings".into(), 950)]);
        assert_eq!(s.due_on(1299, "Settings"), vec![]);
        assert_eq!(s.due_on(1300, "Settings"), vec![Step::ClickText("Import saves".into())]);
        // A page that never comes: the wait gives up after 10 s and the rest runs on.
        assert_eq!(s.due_on(1500, "Settings"), vec![]);
        assert_eq!(s.due_on(11500, "Settings"), vec![Step::PageTimedOut("Nowhere".into(), "Settings".into())]);
        assert_eq!(s.due_on(11600, "Settings"), vec![Step::Ev(Event::Quit)]);
    }

    #[test]
    fn tablet_morph_window_steps() {
        let Some(mut l) = home() else { return };
        let mut p = nm_ui::model::Page::new("Move Pokemon");
        p.boxes = Some(Default::default());
        l.pages.push(p);
        // Open (the boxes page is up), Escape back at 1 s, close at 2 s.
        let mut f = Fake::new(vec![(1.0, Event::Key(0x1B)), (2.0, Event::Quit)]);
        assert_eq!(run(&mut l, &mut f, None), Exit::PoweredOff);
        // One window change each way: the tablet's final place and size first, the phone where it ends up after.
        assert_eq!(f.bounds.first(), Some(&(210, 20, 1500, 1000)), "{:?}", f.bounds);
        assert_eq!(f.bounds.len(), 2, "{:?}", f.bounds);
        assert_eq!((f.bounds[1].1, f.bounds[1].2, f.bounds[1].3), (20, 600, 1000));
        // In between only the outline moves: wider each tick while it opens, ~0.35 s at 16 ms ticks.
        let open: Vec<i32> = f.outlines.iter().take_while(|o| o.1 < 1500).map(|o| o.1).collect();
        assert!(open.windows(2).all(|w| w[1] >= w[0]) && (18..=26).contains(&open.len()), "{:?}", open);
        assert!(f.outlines.iter().any(|o| *o == (0, 1500, 1000)));
        assert_eq!(f.outlines.last(), Some(&(0, 600, 1000)));
    }
    #[test]
    fn shortcut_flags() {
        let a: Vec<String> = ["--game", "red", "--slot", "2"].iter().map(|s| s.to_string()).collect();
        let f = flags(&a);
        assert_eq!((f.game.as_deref(), f.slot.as_deref(), f.menu), (Some("red"), Some("2"), false));
        let a: Vec<String> = ["--launcher", "--play", "crystal"].iter().map(|s| s.to_string()).collect();
        let f = flags(&a);
        assert_eq!((f.game.as_deref(), f.slot, f.menu), (Some("crystal"), None, true));
        assert!(flags(&["--game".to_string()]).game.is_none());
    }
    #[test]
    fn frame_pacing() {
        assert!(may_draw(true, true, false, Duration::ZERO));
        assert!(!may_draw(true, true, true, Duration::from_millis(16)));
        assert!(may_draw(true, true, true, Duration::from_millis(32)));
        assert!(!may_draw(true, true, true, Duration::from_millis(27)));
        assert!(!may_draw(true, false, false, Duration::from_secs(1)));
        assert!(!may_draw(false, true, false, Duration::from_secs(1)));
        assert_eq!(tick_for(false), HIDDEN_TICK);
    }
    #[test]
    fn one_scan_per_tick() {
        let mut s = ProcScan::default();
        let n = s.get(1).len();
        assert!(n > 0);
        let p = s.get(1).as_ptr();
        assert_eq!(p, s.get(1).as_ptr());
    }
}
