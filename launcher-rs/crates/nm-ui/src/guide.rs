//! The guide overlay (GuideTick, OpenGuide, GuideOverlay), line by line from the C#: the guide button while a game
//! runs opens Resume, Controls and Close game over the game's screen, by controller, keys or mouse. The window is the
//! platform's; this is its state, input and drawing.

use crate::input::{Key, A, B, DOWN, UP};
use crate::launcher::Launcher;
use nm_draw::{Color, Graphics, RectangleF};

#[derive(Debug, PartialEq, Eq)]
pub enum GuideOut { Stay, Redraw, Resume, CloseGame }

/// What the live loop needs to open it: the game's window, and the overlay's contents.
pub struct GuideOpen { pub game_window: isize, pub guide: Guide }

pub struct Guide {
    pub title: String,
    pub controls: String,
    pub accent: Color,
    pub sel: i32,
    pub show_controls: bool,
    pub confirm_close: bool,
    ready: bool,
    prev: u16,
    guide_prev: bool,
    pub rows: Vec<RectangleF>,
}

impl Guide {
    pub fn new(title: &str, controls: &str, accent: Color) -> Guide {
        Guide { title: title.into(), controls: controls.into(), accent, sel: 0, show_controls: false, confirm_close: false, ready: false, prev: 0, guide_prev: true, rows: Vec::new() }
    }
    /// PollPad, every 16 ms: buttons still held from opening it don't count; the guide button again resumes.
    pub fn poll(&mut self, b: u16, gd: bool) -> GuideOut {
        if !self.ready { self.prev = b; self.guide_prev = gd; self.ready = true; return GuideOut::Stay; }
        let pressed = b & !self.prev;
        self.prev = b;
        let guide = gd && !self.guide_prev;
        self.guide_prev = gd;
        if guide { return GuideOut::Resume; }
        if pressed & UP != 0 { self.act("up") } else if pressed & DOWN != 0 { self.act("down") }
        else if pressed & A != 0 { self.act("ok") } else if pressed & B != 0 { self.act("back") } else { GuideOut::Stay }
    }
    /// ProcessCmdKey.
    pub fn key(&mut self, k: Key) -> GuideOut {
        match k { Key::Up => self.act("up"), Key::Down => self.act("down"), Key::Enter => self.act("ok"), Key::Escape | Key::Back => self.act("back"), _ => GuideOut::Stay }
    }
    pub fn mouse_move(&mut self, x: f32, y: f32) -> GuideOut {
        for i in 0..self.rows.len() {
            if self.rows[i].contains(nm_draw::PointF::new(x, y)) && self.sel != i as i32 { self.sel = i as i32; if i != 2 { self.confirm_close = false; } return GuideOut::Redraw; }
        }
        GuideOut::Stay
    }
    pub fn click(&mut self, x: f32, y: f32, right: bool) -> GuideOut {
        if right { return self.act("back"); }
        for i in 0..self.rows.len() { if self.rows[i].contains(nm_draw::PointF::new(x, y)) { self.sel = i as i32; return self.act("ok"); } }
        GuideOut::Stay
    }
    /// Do: closing asks twice, as anything after the last save in the game is lost.
    fn act(&mut self, a: &str) -> GuideOut {
        match a {
            "up" => self.sel = (self.sel + 2) % 3,
            "down" => self.sel = (self.sel + 1) % 3,
            "back" => { if self.show_controls { self.show_controls = false; } else { return GuideOut::Resume; } }
            "ok" => {
                if self.sel == 0 { return GuideOut::Resume; }
                if self.sel == 1 { self.show_controls = !self.show_controls; }
                if self.sel == 2 { if !self.confirm_close { self.confirm_close = true; } else { return GuideOut::CloseGame; } }
            }
            _ => {}
        }
        if self.sel != 2 { self.confirm_close = false; }
        GuideOut::Redraw
    }
    /// OnPaint: GuideOverlay.Draw over the whole overlay; the rows are kept for the mouse.
    pub fn draw(&mut self, hooks: &mut crate::hooks::TestHooks, g: &mut Graphics, size: (i32, i32)) {
        self.rows = crate::launcher::guide_draw(hooks, g, size, &self.title, &self.controls, self.accent, self.sel, self.show_controls, self.confirm_close);
    }
}

impl Launcher {
    /// GuideTick (never in a replay): the guide button pressed while a game runs, and no guide open yet.
    pub fn guide_tick(&mut self, down: bool, open_now: bool) -> Option<GuideOpen> {
        let was = self.jobs.guide_was;
        self.jobs.guide_was = down;
        if self.hooks.replay || !down || was || !self.job || !self.job_is_game || open_now { return None; }
        let w = self.jobs.proc.as_mut()?.main_window()?.0;
        if w == 0 { return None; }
        let cur = self.ctx.as_ref().and_then(|c| c.current.map(|i| c.lib.games[i].clone()));
        let title = cur.as_ref().map(|g| g.title.clone()).unwrap_or_else(|| self.job_title.clone());
        let controls = crate::pages::controls_for(&cur.as_ref().map(|g| g.engine()).unwrap_or_default());
        let accent = crate::launcher::accent(&self.cur());
        Some(GuideOpen { game_window: w, guide: Guide::new(&title, &controls, accent) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_asks_twice_and_back_resumes() {
        let mut g = Guide::new("Pokemon Platinum", "keys", Color::WHITE);
        // The first poll only takes what's held.
        assert_eq!(g.poll(A, true), GuideOut::Stay);
        assert_eq!(g.poll(A, true), GuideOut::Stay);
        assert_eq!(g.poll(0, false), GuideOut::Stay);
        assert_eq!(g.poll(UP, false), GuideOut::Redraw);
        assert_eq!(g.sel, 2);
        assert_eq!(g.poll(A, false), GuideOut::Redraw);
        assert!(g.confirm_close);
        assert_eq!(g.key(Key::Enter), GuideOut::CloseGame);
        let mut g = Guide::new("x", "keys", Color::WHITE);
        g.key(Key::Down);
        assert_eq!(g.key(Key::Enter), GuideOut::Redraw);
        assert!(g.show_controls);
        assert_eq!(g.key(Key::Escape), GuideOut::Redraw);
        assert!(!g.show_controls);
        assert_eq!(g.key(Key::Escape), GuideOut::Resume);
        // Moving off Close game takes the second ask back.
        let mut g = Guide::new("x", "keys", Color::WHITE);
        g.rows = vec![RectangleF::new(0.0, 0.0, 10.0, 10.0), RectangleF::new(0.0, 10.0, 10.0, 10.0), RectangleF::new(0.0, 20.0, 10.0, 10.0)];
        assert_eq!(g.click(5.0, 25.0, false), GuideOut::Redraw);
        assert!(g.confirm_close);
        assert_eq!(g.mouse_move(5.0, 5.0), GuideOut::Redraw);
        assert!(!g.confirm_close && g.sel == 0);
        assert_eq!(g.click(5.0, 5.0, true), GuideOut::Resume);
        // The guide button pressed again resumes.
        let mut g = Guide::new("x", "keys", Color::WHITE);
        g.poll(0, false);
        assert_eq!(g.poll(0, true), GuideOut::Resume);
    }
}
