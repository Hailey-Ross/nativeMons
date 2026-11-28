//! Input and page flow, ported from LauncherForm's Poll, ProcessCmdKey, the mouse handlers and Act:
//! a pure state machine (input event + time -> state). The platform feeds it and carries out `effects`.

use crate::art::APP_NAMES;
use crate::launcher::{Launcher, OPEN_TIME};
use nm_draw::RectangleF;

pub const UP: u16 = 0x0001;
pub const DOWN: u16 = 0x0002;
pub const LEFT: u16 = 0x0004;
pub const RIGHT: u16 = 0x0008;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const LB: u16 = 0x0100;
pub const RB: u16 = 0x0200;
pub const TILE_COLS: i32 = 4;

/// The keys ProcessCmdKey handles (System.Windows.Forms.Keys); anything else is Other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key { Up, Down, Left, Right, W, A, S, D, Enter, Escape, Back, Space, Tab, X, Q, E, PageUp, PageDown, Other }

impl Key {
    pub fn parse(s: &str) -> Option<Key> {
        Some(match s.to_ascii_lowercase().as_str() {
            "up" => Key::Up, "down" => Key::Down, "left" => Key::Left, "right" => Key::Right,
            "w" => Key::W, "a" => Key::A, "s" => Key::S, "d" => Key::D, "enter" | "return" => Key::Enter,
            "escape" | "esc" => Key::Escape, "back" | "backspace" => Key::Back, "space" => Key::Space, "tab" => Key::Tab,
            "x" => Key::X, "q" => Key::Q, "e" => Key::E, "pageup" | "prior" => Key::PageUp, "pagedown" | "next" => Key::PageDown,
            _ => return None,
        })
    }
    /// A Windows virtual-key code (stable-check posts WM_KEYDOWN with these).
    pub fn from_vk(vk: u32) -> Key {
        match vk {
            0x26 => Key::Up, 0x28 => Key::Down, 0x25 => Key::Left, 0x27 => Key::Right, 0x57 => Key::W, 0x41 => Key::A,
            0x53 => Key::S, 0x44 => Key::D, 0x0D => Key::Enter, 0x1B => Key::Escape, 0x08 => Key::Back, 0x20 => Key::Space,
            0x09 => Key::Tab, 0x58 => Key::X, 0x51 => Key::Q, 0x45 => Key::E, 0x21 => Key::PageUp, 0x22 => Key::PageDown,
            _ => Key::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button { Left, Right, Middle }

/// What the window must do; the state machine never touches the platform itself.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Close,
    Minimize,
    /// Dragging the shell moves the device (ReleaseCapture + WM_NCLBUTTONDOWN).
    DragWindow,
    HandCursor(bool),
    OpenUrl(String),
    OpenFolder(String),
    /// A C# action that belongs to a later port step (play flows, installs, dialogs), by its C# name.
    Todo(String),
    /// TestHooks.Skip: something a dry-run replay only notes (logged as it is, e.g. "PlaySnaps.Watch(id)").
    Skip(String),
    /// MorphTowards' one window change per fold: SetBounds(x, y, w, h) on the window's client area.
    SetBounds { x: i32, y: i32, w: i32, h: i32 },
    /// The window's outline: Round(x, 0, w, h) with this corner radius, as SetWindowRgn (SetMorphRegion).
    SetRegion { x: i32, w: i32, h: i32, radius: f32 },
    /// A dry run's note of what would start (RunJob): title | program | args | cwd | env [| game].
    Launch(String),
    /// JobEnded: the window shows again, restored and in front.
    ShowWindow,
    /// WatchGame: the game's window is up, so the launcher hides.
    HideWindow,
    /// Clipboard.SetText (never in a replay: the C# replay's clipboard isn't the test's).
    Clipboard(String),
    /// FolderBrowserDialog / OpenFileDialog / the save-name box: the platform answers with Launcher::dialog_done.
    PickFolder { title: String },
    PickFile { title: String, filters: Vec<(String, String)> },
    AskText { title: String, prompt: String, start: String },
    /// A replay's dialog, answered from the script (the C# replay notes the same).
    Dialog(String),
    /// A replay's download, answered by the fetch stub; Gen1Recomp's command line, answered by its stub.
    Fetch(String),
    Gen1Cli(String),
    /// After an update: the launcher would start its new copy and close (noted in a replay).
    Restart,
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub prev_pad: u16,
    pub held: i32,
    pub last_action: String,
    pub last_from_pad: bool,
    pub last_action_time: f32,
    pub last_mouse: Option<(i32, i32)>,
    pub frame: i32,
    pub browse_ids: u32,
}

pub fn grid_move(sel: i32, a: &str, count: i32) -> i32 {
    match a {
        "left" => 0.max(sel - 1),
        "right" => (count - 1).min(sel + 1),
        "down" => if sel == 0 { 1.min(count - 1) } else { (count - 1).min(sel + TILE_COLS) },
        "up" => if sel <= TILE_COLS { 0 } else { sel - TILE_COLS },
        _ => sel,
    }
}
pub fn page_grid_move(sel: i32, a: &str, count: i32, cols: i32) -> i32 {
    match a {
        "left" => 0.max(sel - 1),
        "right" => (count - 1).min(sel + 1),
        "down" => if sel / cols < (count - 1) / cols { (count - 1).min(sel + cols) } else { sel },
        "up" => if sel >= cols { sel - cols } else { sel },
        _ => sel,
    }
}
pub fn gallery_move(sel: i32, a: &str, count: i32) -> i32 {
    let n = match a { "left" => sel - 1, "right" => sel + 1, "up" => sel - 2, "down" => sel + 2, _ => sel };
    0.max((count - 1).min(n))
}

fn contains(r: &RectangleF, x: f32, y: f32) -> bool { x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height }

impl Launcher {
    pub fn invalidate(&mut self) { self.dirty = true; }
    pub fn morphing(&self) -> bool { self.tablet_k > 0.0 && self.tablet_k < 1.0 }
    pub fn top_boxes(&self) -> bool { !self.job && self.pages.last().map_or(false, |p| p.boxes.is_some()) }

    /// Animating(): something on screen still moves by itself.
    pub fn animating(&self) -> bool {
        let t = self.now();
        self.power_off >= 0.0 || t - self.setup_at < 1.4 || self.pages.last().map_or(false, |p| p.step == 0) || t - self.screen_at < 0.2
            || t - self.sel_at < 0.14 || self.open_run.is_some() || t - self.open_at < OPEN_TIME + 0.3 || t - self.bounce_at < 1.4 || self.jobs.splash_shot.is_some()
            || self.setup_leaving_active()
    }

    /// One 16 ms tick (Poll), minus the platform's parts: `pad` is this tick's XInput-style buttons, `active` whether
    /// the window has focus. Returns whether a frame should be drawn now.
    pub fn poll(&mut self, pad: u16, active: bool) -> bool {
        if self.closing || self.power_off >= 0.0 {
            self.power_off_tick();
            return self.power_off >= 0.0;
        }
        if self.dirty_poll { self.dirty_poll = false; self.dirty = true; }
        self.job_tick();
        self.open_tick();
        if !self.wide_window { let boxes = self.pages.iter().any(|p| p.boxes.is_some()); self.morph_towards(boxes); }
        let t = self.now();
        if self.banner_age_peek(t) >= 0.0 { self.dirty = true; }
        if self.clash_showing() && (self.clash_at.is_nan() || t - self.clash_at < 0.4) { self.dirty = true; }
        let mut draw = false;
        if self.dirty || self.animating() { self.dirty = false; draw = true; }
        let main = self.pages.last().map_or(false, |p| p.main);
        let st = &mut self.input;
        if !self.job && main && self.pages.last().map_or(false, |p| p.sel >= 0) && st.frame % 2 == 0 && !self.light { draw = true; }
        if self.panel_intro && st.frame % 6 == 0 { draw = true; }
        st.frame += 1;
        if st.frame % 2 == 0 && (self.job || self.starting || self.scrolling || self.spinning || self.recomp_banner.is_some() || main) { draw = true; }
        if self.job && self.jobs.cancel.is_some() && active { if pad & !self.input.prev_pad & B != 0 { self.job_cancel_key(); } self.input.prev_pad = pad; return draw; }
        if self.job || self.starting || !active { self.input.prev_pad = pad; return draw; }
        let b = pad;
        let prev = self.input.prev_pad;
        let pressed = b & !prev;
        let boxes = self.top_boxes();
        if pressed & A != 0 { self.act("ok", true); }
        else if pressed & B != 0 { self.act("back", true); }
        else if pressed & UP != 0 { self.act("up", true); }
        else if pressed & DOWN != 0 { self.act("down", true); }
        else if pressed & LEFT != 0 { self.act("left", true); }
        else if pressed & RIGHT != 0 { self.act("right", true); }
        else if boxes && pressed & X != 0 { self.act("switch", true); }
        else if boxes && pressed & (LB | RB) != 0 { self.act(if pressed & LB != 0 { "boxprev" } else { "boxnext" }, true); }
        else if b & (UP | DOWN) != 0 && b == prev {
            self.input.held += 1;
            if self.input.held > 14 && self.input.held % 4 == 0 { self.act(if b & UP != 0 { "up" } else { "down" }, true); }
        }
        if b != prev { self.input.held = 0; }
        self.input.prev_pad = b;
        draw || self.dirty
    }

    /// OpenTick: the app's icon has filled the screen, so the page it opens is pushed.
    pub fn open_tick(&mut self) {
        if self.open_run.is_none() || self.now() - self.open_at < OPEN_TIME { return; }
        let run = self.open_run.take().unwrap();
        self.bounce_arm = true;
        run.call(self);
    }

    /// ProcessCmdKey. Returns whether the key was taken.
    pub fn key(&mut self, k: Key) -> bool {
        if self.job && matches!(k, Key::Escape | Key::Back) && self.job_cancel_key() { return true; }
        if self.job || self.starting { return false; }
        let a = match k {
            Key::Up | Key::W => "up",
            Key::Down | Key::S => "down",
            Key::Left | Key::A => "left",
            Key::Right | Key::D => "right",
            Key::Enter => "ok",
            Key::Escape | Key::Back | Key::Space => "back",
            _ => "",
        };
        if !a.is_empty() { self.act(a, false); return true; }
        if self.top_boxes() {
            let a = match k { Key::Tab | Key::X => "switch", Key::Q | Key::PageUp => "boxprev", Key::E | Key::PageDown => "boxnext", _ => "" };
            if !a.is_empty() { self.act(a, false); return true; }
        }
        false
    }

    pub fn hit_at(&self, x: i32, y: i32) -> i32 {
        let (ix, iy) = (x as f32 - self.screen_x, y as f32 - self.screen_y);
        self.hits.iter().find(|h| contains(&h.0, ix, iy)).map_or(-1, |h| h.1)
    }
    pub fn prompt_at(&self, x: i32, y: i32) -> Option<String> {
        self.prompt_hits.iter().find(|h| contains(&h.0, x as f32, y as f32)).map(|h| h.1.clone())
    }

    pub fn mouse_wheel(&mut self, delta: i32) {
        if self.morphing() { return; }
        if !self.job && delta != 0 { self.act(if delta > 0 { "up" } else { "down" }, false); }
    }
    pub fn mouse_down(&mut self, x: i32, y: i32, b: Button) {
        if self.morphing() { return; }
        if self.prompt_at(x, y).is_none() && self.box_mouse_down(x, y, b) { return; }
        if b == Button::Left && self.prompt_at(x, y).is_none() && self.hit_at(x, y) < 0 { self.effects.push(Effect::DragWindow); }
    }
    pub fn mouse_up(&mut self, x: i32, y: i32, b: Button) {
        if self.morphing() { return; }
        self.box_mouse_up(x, y, b);
    }
    /// OnMouseMove; `cursor` is the cursor's place on the desktop (a still cursor must not take the keys' selection).
    pub fn mouse_move(&mut self, x: i32, y: i32, cursor: (i32, i32)) {
        if self.morphing() { return; }
        let over = self.prompt_at(x, y);
        let over_s = over.clone().unwrap_or_default();
        if over_s != self.hover_prompt {
            self.hover_prompt = over_s.clone();
            self.effects.push(Effect::HandCursor(over.is_some()));
            self.dirty = true;
        }
        if self.box_mouse_move(x, y) { return; }
        if self.input.last_mouse == Some(cursor) { return; }
        self.input.last_mouse = Some(cursor);
        let i = self.hit_at(x, y);
        if !self.job && over_s != "N" && over_s != "NX" && over_s != "C" && i >= 0 {
            if let Some(p) = self.pages.last_mut() { if p.sel != i { p.sel = i; self.dirty = true; } }
        }
    }
    pub fn mouse_click(&mut self, x: i32, y: i32, b: Button) {
        if self.morphing() { return; }
        let prompt = if b == Button::Left { self.prompt_at(x, y) } else { None };
        match prompt.as_deref() {
            Some("M") => { self.effects.push(Effect::Minimize); return; }
            Some("X") if !self.job => { self.request_close(); return; }
            _ => {}
        }
        if self.job { return; }
        match prompt.as_deref() {
            Some("NX") => {
                let c = self.banner_build.to_string();
                nm_core::config::write("update-dismissed.txt", &c);
                self.banner_closed = Some(c);
                self.dirty = true;
                return;
            }
            Some("N") => { self.banner_at = -crate::launcher::BANNER_SECONDS * 2.0; self.open_updates(); return; }
            Some("C") => { self.open_clashes(); return; }
            Some(u) if u.starts_with("url:") => { self.effects.push(Effect::OpenUrl(u[4..].to_string())); return; }
            Some(p) => {
                let a = match p { "A" => "ok", "B" => "back", "U" => "up", "D" => "down", "L" => "left", "R" => "right", _ => "" };
                self.act(a, false);
                return;
            }
            None => {}
        }
        if b == Button::Right { self.act("back", false); return; }
        if self.box_click(x, y, b) { return; }
        if b != Button::Left { return; }
        let i = self.hit_at(x, y);
        if i >= 0 && !self.pages.is_empty() {
            self.pages.last_mut().unwrap().sel = i;
            self.act("ok", false);
        }
    }

    /// Close(): a user close powers the device down first (FormClosing), then closes for real.
    pub fn request_close(&mut self) {
        if self.power_off < 0.0 && !self.closing {
            self.power_off = self.now();
            return;
        }
        self.closing = true;
        self.effects.push(Effect::Close);
    }

    /// PowerOffTick without the window: the lights; the window's fall is the platform's (power_drop).
    pub fn power_off_tick(&mut self) {
        if self.power_off < 0.0 || self.powered_off { return; }
        let u = self.now() - self.power_off;
        self.power_level = crate::launcher::power_curve(u - crate::launcher::LIGHTS_AT);
        if u >= crate::launcher::LIGHTS_AT && self.frozen_t < 0.0 { self.frozen_t = self.power_off + crate::launcher::LIGHTS_AT; }
        self.dirty = true;
    }
    /// How long the device has been falling (PowerOffTick's k), or None before it starts.
    pub fn power_fall(&self) -> Option<f32> {
        if self.power_off < 0.0 { return None; }
        let k = self.now() - self.power_off - crate::launcher::LIGHTS_AT - crate::launcher::LIGHTS_TIME;
        if k > 0.0 && !self.snapshot_anim { Some(k) } else { None }
    }

    /// Act: one action from the keys, the pad, the mouse or the touch prompts.
    pub fn act(&mut self, a: &str, from_pad: bool) {
        let now = self.now();
        let st = &mut self.input;
        if a == st.last_action && from_pad != st.last_from_pad && (now - st.last_action_time) * 1000.0 < 150.0 { return; }
        st.last_action = a.to_string();
        st.last_from_pad = from_pad;
        st.last_action_time = now;
        if self.pages.is_empty() || self.open_run.is_some() || self.power_off >= 0.0 { return; }
        let top = self.pages.len() - 1;
        let holding = self.pages[top].boxes.as_ref().map_or(false, |v| v.holding());
        if a == "back" && holding {
            // B puts a held Pokemon back where it was, before it leaves the boxes.
            let v = self.pages[top].boxes.as_mut().unwrap();
            v.held_pane = -1;
            v.dragging = false;
        } else if a == "back" {
            match self.pages[top].on_back.clone() { Some(f) => f.call(self), None => { self.pages.pop(); } }
            if self.pages.is_empty() { let m = self.main_page_now(); self.pages.push(m); }
        } else if self.pages[top].boxes.is_some() {
            self.box_act(a);
        } else if self.pages[top].dex_id > 0 {
            let body_h = self.fonts.body.height() as f32;
            let shiny = self.dex_shiny_of(self.pages[top].dex_id, self.pages[top].dex_only.as_deref());
            let p = &mut self.pages[top];
            if a == "up" || a == "down" { p.body_scroll += if a == "down" { 1.0 } else { -1.0 } * body_h * 3.0; }
            else if a == "ok" { if shiny { p.dex_shiny = !p.dex_shiny; } }
            else if (a == "left" || a == "right") && !p.dex_order.is_empty() {
                // The next Pokemon you've met, as the games' Pokedex steps through them.
                let at = p.dex_order.iter().position(|&x| x == p.dex_id).map_or(-1, |i| i as i32);
                let next = at + if a == "right" { 1 } else { -1 };
                if at >= 0 && next >= 0 && (next as usize) < p.dex_order.len() {
                    let (order, only) = (p.dex_order.clone(), p.dex_only.clone());
                    self.pages.pop();
                    let e = self.dex_entry_page_now(order[next as usize], order, only);
                    self.pages.push(e);
                }
            }
        } else if !self.pages[top].items.is_empty() {
            let p = &self.pages[top];
            let n = p.items.len() as i32;
            let sel = p.sel.clamp(0, n - 1);
            let it = &p.items[sel as usize];
            if a == "ok" {
                let s = sel as usize;
                if it.on && it.run.is_some() && p.main && sel >= 1 && s < APP_NAMES.len() && APP_NAMES[s] != "Move" && APP_NAMES[s] != "Exit" && !self.app_rects[s].is_empty() {
                    self.open_run = it.run.clone();
                    self.open_app = s;
                    self.open_from = self.app_rects[s];
                    self.open_at = self.now();
                } else if it.on {
                    if let Some(f) = it.run.clone() { f.call(self); }
                }
            } else if p.main { self.pages[top].sel = grid_move(sel, a, n); }
            else if p.gallery { self.pages[top].sel = gallery_move(sel, a, n); }
            else if (a == "left" || a == "right") && it.slide.is_some() { let f = it.slide.clone().unwrap(); (f.0)(self, if a == "right" { 5 } else { -5 }); }
            else if (a == "left" && it.fold == 1) || (a == "right" && it.fold == 2) { if let Some(f) = it.run.clone() { f.call(self); } }
            else if (a == "left" || a == "right") && it.fold != 0 {}
            else if p.jump > 0 && a == "left" { self.pages[top].sel = 0.max(sel - p.jump); }
            else if p.jump > 0 && a == "right" { self.pages[top].sel = (n - 1).min(sel + p.jump); }
            else if p.body.is_none() { self.pages[top].sel = page_grid_move(sel, a, n, 1); }
            else if a == "up" || a == "left" { self.pages[top].sel = (sel - 1 + n) % n; }
            else if a == "down" || a == "right" { self.pages[top].sel = (sel + 1) % n; }
        } else if (a == "up" || a == "down") && self.pages[top].body.is_some() {
            // The drawing keeps it within the text.
            let body_h = self.fonts.body.height() as f32;
            self.pages[top].body_scroll += if a == "down" { 1.0 } else { -1.0 } * body_h * 3.0;
        } else if a == "ok" && self.pages[top].body.is_some() {
            match self.pages[top].on_back.clone() { Some(f) => f.call(self), None => { self.pages.pop(); } }
        }
        self.dirty = true;
    }
}

/// PowerOffTick's fall: the window's top after `k` seconds of falling, and whether it is out of sight.
/// rest = max(power_top, work_bottom - height); bottom = screen_bottom + 20.
pub fn power_drop(k: f32, power_top: i32, work_bottom: i32, screen_bottom: i32, height: i32) -> (i32, bool) {
    const LIFT_TIME: f32 = 0.24;
    const FALL_TIME: f32 = 0.5;
    let rest = power_top.max(work_bottom - height) as f32;
    let bottom = (screen_bottom + 20) as f32;
    let lift = height as f32 * 0.065;
    let peak = rest - lift;
    let g = 2.0 * (bottom - peak) / (FALL_TIME * FALL_TIME);
    let drop = (2.0 * (rest - power_top as f32) / g).sqrt();
    let y = if k < drop { power_top as f32 + 0.5 * g * k * k }
        else if k < drop + LIFT_TIME { let q = (k - drop) / LIFT_TIME; rest - lift * q * q * (3.0 - 2.0 * q) }
        else { let tau = k - drop - LIFT_TIME; peak + 0.5 * g * tau * tau };
    (y as i32, y >= bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn home_grid() {
        // Play (0) above a 4-wide grid of 8 apps (1..8).
        assert_eq!(grid_move(0, "down", 9), 1);
        assert_eq!(grid_move(1, "down", 9), 5);
        assert_eq!(grid_move(5, "down", 9), 8);
        assert_eq!(grid_move(4, "up", 9), 0);
        assert_eq!(grid_move(6, "up", 9), 2);
        assert_eq!(grid_move(0, "left", 9), 0);
        assert_eq!(grid_move(8, "right", 9), 8);
    }
    #[test]
    fn lists_and_gallery() {
        assert_eq!(page_grid_move(0, "up", 5, 1), 0);
        assert_eq!(page_grid_move(4, "down", 5, 1), 4);
        assert_eq!(page_grid_move(2, "down", 5, 1), 3);
        assert_eq!(gallery_move(1, "down", 5), 3);
        assert_eq!(gallery_move(3, "down", 5), 4);
        assert_eq!(gallery_move(0, "up", 5), 0);
    }
    #[test]
    fn keys() {
        assert_eq!(Key::from_vk(0x28), Key::Down);
        assert_eq!(Key::parse("Enter"), Some(Key::Enter));
        assert_eq!(Key::parse("nope"), None);
    }
    #[test]
    fn power_drop_falls_away() {
        let (y0, gone0) = power_drop(0.0, 100, 1040, 1080, 900);
        assert_eq!((y0, gone0), (100, false));
        let (_, gone) = power_drop(3.0, 100, 1040, 1080, 900);
        assert!(gone);
    }
}
