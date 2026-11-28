//! MorphTowards: the phone grows into the tablet while Move Pokemon's boxes are open, and folds back after.
//! The window changes once per fold (Effect::SetBounds); in between only the drawn device (draw_x, draw_w) moves.

use crate::input::Effect;
use crate::launcher::Launcher;
use nm_draw::Graphics;

/// The morph's own state, and the window as the platform last reported it.
#[derive(Clone, Debug, Default)]
pub struct Morph {
    pub tablet_w: i32,
    pub tablet_last: f32,
    pub morph_centre: i32,
    pub draw_x: i32,
    pub draw_w: i32,
    /// The window: left, top, client width, client height (the platform keeps it current).
    pub window: (i32, i32, i32, i32),
    /// The screen's working area: left, top, width, height.
    pub area: (i32, i32, i32, i32),
}

fn corner_radius(w: i32) -> f32 { w as f32 * 0.075 }

impl Launcher {
    fn morph_bounds(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.morph.window = (x, y, w, h);
        self.effects.push(Effect::SetBounds { x, y, w, h });
    }
    fn morph_region(&mut self, x: i32, w: i32, h: i32) { self.effects.push(Effect::SetRegion { x, w, h, radius: corner_radius(self.phone_w) }); }

    /// ApplyLayout: the Settings switch; the window becomes the other shape where it is, right away.
    pub fn apply_layout(&mut self, wide: bool) {
        let (ax, ay, aw, ah) = self.morph.area;
        let (w, h) = if wide && std::env::var("SteamDeck").as_deref() == Ok("1") { (aw, ah) }
            else if wide { (aw * 94 / 100, ah * 94 / 100) } else { let h = ah * 94 / 100; (h * 60 / 100, h) };
        self.morph.draw_x = 0;
        self.morph.draw_w = 0;
        self.wide_window = wide;
        if let Some(c) = self.ctx.as_mut() { c.wide_window = wide; }
        if wide { self.phone_w = h * 60 / 100; self.tablet_k = 1.0; self.morph.tablet_w = w; } else { self.phone_w = 0; self.tablet_k = 0.0; }
        self.morph_bounds(ax + (aw - w) / 2, ay + (ah - h) / 2, w, h);
        self.effects.push(Effect::SetRegion { x: 0, w, h, radius: corner_radius(w.min(h * 60 / 100)) });
        self.fonts = crate::launcher::Fonts::make(if wide { crate::launcher::wide_text(w, self.phone_w) } else { w });
        self.dirty = true;
    }

    /// MorphTowards(on), once per tick.
    pub fn morph_towards(&mut self, on: bool) {
        let target = if on { 1.0 } else { 0.0 };
        let now = self.now();
        if self.tablet_k == target { self.morph.tablet_last = now; return; }
        let (left0, top, cw, h) = self.morph.window;
        let area = self.morph.area;
        if self.phone_w == 0 { self.phone_w = cw; }
        if self.tablet_k == 0.0 || self.tablet_k == 1.0 {
            // Starting out: where it opens from, and how wide it gets on this screen.
            self.morph.morph_centre = left0 + cw / 2;
            if self.tablet_k == 0.0 {
                self.morph.tablet_w = self.phone_w.max((area.2 * 94 / 100).min(h * 3 / 2));
                let left = area.0.max((area.0 + area.2 - self.morph.tablet_w).min(self.morph.morph_centre - self.morph.tablet_w / 2));
                self.morph.draw_w = self.phone_w;
                self.morph.draw_x = left0 - left;
                // One change of the window, before anything moves: its final place and size, showing only the phone.
                let tw = self.morph.tablet_w;
                self.morph_bounds(left, top, tw, h);
                let (dx, dw) = (self.morph.draw_x, self.morph.draw_w);
                self.morph_region(dx, dw, h);
            }
            // The first step counts one tick, so it gets going.
            self.morph.tablet_last = now - 0.016;
        }
        let step = (now - self.morph.tablet_last).max(0.0).min(0.05) / 0.35;
        self.morph.tablet_last = now;
        self.tablet_k = if on { (self.tablet_k + step).min(1.0) } else { (self.tablet_k - step).max(0.0) };
        let k = self.tablet_k;
        let ease = k * k * (3.0 - 2.0 * k);
        let w = (self.phone_w as f32 + (self.morph.tablet_w - self.phone_w) as f32 * ease).round() as i32;
        // It widens from (and narrows back to) the phone's center, kept inside the window.
        let (left, _, cw, _) = self.morph.window;
        self.morph.draw_w = w;
        self.morph.draw_x = 0.max((cw - w).min(self.morph.morph_centre - left - w / 2));
        if self.tablet_k == 0.0 {
            // Folded: the window becomes the phone again, where the phone now is.
            let l = left + self.morph.draw_x;
            self.morph.draw_x = 0;
            self.morph.draw_w = 0;
            let pw = self.phone_w;
            self.morph_bounds(l, top, pw, h);
            self.morph_region(0, pw, h);
            self.dirty = true;
            return;
        } else if self.tablet_k == 1.0 {
            self.morph.draw_x = 0;
            self.morph.draw_w = 0;
            self.morph_region(0, cw, h);
        } else {
            let dx = self.morph.draw_x;
            self.morph_region(dx, w, h);
        }
        self.dirty = true;
    }

    /// OnPaint: mid-morph only the drawn device's part of the window is the device.
    pub fn paint(&mut self, g: &mut Graphics, w: i32, h: i32) {
        self.spinning = false;
        self.panel_intro = false;
        if self.morph.draw_w > 0 {
            let kept = g.save();
            g.translate_transform(self.morph.draw_x as f32, 0.0);
            let dw = self.morph.draw_w;
            self.render(g, dw, h);
            g.restore(kept);
        } else {
            self.render(g, w, h);
            self.outro(g, w, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::{Host, Launcher};

    #[test]
    fn morph_opens_and_folds() {
        // Launcher::new needs the bundled fonts (NATIVEMONS_FONTS_DIR); without them there's nothing to test.
        if std::env::var("NATIVEMONS_FONTS_DIR").is_err() { eprintln!("skipped: NATIVEMONS_FONTS_DIR not set"); return; }
        let mut l = Launcher::new(600, 1000, Host::new((1920, 1040)));
        l.hooks.t = Some(10.0);
        l.morph.window = (660, 20, 600, 1000);
        l.morph.area = (0, 0, 1920, 1040);
        l.morph_towards(true);
        // The window takes its final place at once: 1500 wide (h * 3 / 2), centered on the phone.
        assert_eq!(l.effects[0], Effect::SetBounds { x: 210, y: 20, w: 1500, h: 1000 });
        assert!(l.tablet_k > 0.0 && l.tablet_k < 0.1);
        let mut t = 10.0;
        while l.tablet_k < 1.0 { t += 0.016; l.hooks.t = Some(t); l.morph_towards(true); }
        // 0.35 s at one step a tick (plus the first tick).
        assert!((t - 10.0 - 0.336).abs() < 0.02, "{}", t);
        assert_eq!(l.morph.draw_w, 0);
        l.effects.clear();
        while l.tablet_k > 0.0 { t += 0.016; l.hooks.t = Some(t); l.morph_towards(false); }
        assert!(l.effects.contains(&Effect::SetBounds { x: 660, y: 20, w: 600, h: 1000 }), "{:?}", l.effects.last());
    }
}
