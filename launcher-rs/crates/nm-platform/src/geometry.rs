//! Window size and outline, as the C# WindowSize, WantWide, WideFits, CornerRadius and Round.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

impl Rect {
    pub fn right(&self) -> i32 { self.x + self.w }
    pub fn bottom(&self) -> i32 { self.y + self.h }
}

/// Room for the wide layout: a landscape screen at least 1000 px across.
pub fn wide_fits(area: Rect) -> bool { area.w >= 1000 && area.w as f32 > area.h as f32 * 1.2 }

/// WantWide: NATIVEMONS_LAYOUT, else layout.txt, else wide where a phone would be too small.
pub fn want_wide(area: Rect, env_layout: &str, config_layout: &str, steam_deck: bool) -> bool {
    let mut want = env_layout.to_ascii_lowercase();
    if want != "wide" && want != "phone" { want = config_layout.trim().to_ascii_lowercase(); }
    if want == "wide" { return wide_fits(area); }
    if want == "phone" { return false; }
    steam_deck || (area.w > area.h && area.h < 900)
}

pub fn window_size(area: Rect, wide: bool, steam_deck: bool) -> (i32, i32) {
    if wide && steam_deck { return (area.w, area.h); }
    if wide { return (area.w * 94 / 100, area.h * 94 / 100); }
    let h = area.h * 94 / 100;
    (h * 60 / 100, h)
}

/// Centered in the work area, as the C# constructor and ApplyLayout place it.
pub fn centered(area: Rect, size: (i32, i32)) -> (i32, i32) { (area.x + (area.w - size.0) / 2, area.y + (area.h - size.1) / 2) }

pub fn corner_radius(w: i32, h: i32) -> f32 { w.min(h * 60 / 100) as f32 * 0.075 }

/// The window's aliased outline (Round(0, 0, w, h, rad) as a Region) as rows of [x0, x1) spans.
/// A pixel is in when its center is inside the rounded rectangle (diameter min(2 rad, w, h)).
pub fn outline_spans(w: i32, h: i32, rad: f32) -> Vec<(i32, i32, i32)> {
    let d = (rad * 2.0).min(w.min(h) as f32);
    let mut out = Vec::new();
    if d <= 0.0 {
        for y in 0..h { out.push((y, 0, w)); }
        return out;
    }
    let r = d / 2.0;
    for y in 0..h {
        let cy = y as f32 + 0.5;
        // Distance into the corner band, from the nearer of the top and bottom edges.
        let dy = if cy < r { r - cy } else if cy > h as f32 - r { cy - (h as f32 - r) } else { 0.0 };
        let inset = if dy > 0.0 { r - (r * r - dy * dy).max(0.0).sqrt() } else { 0.0 };
        // Pixel centers x + 0.5 >= inset and x + 0.5 <= w - inset.
        let x0 = (inset - 0.5).ceil().max(0.0) as i32;
        let x1 = ((w as f32 - inset - 0.5).floor() as i32 + 1).min(w);
        if x1 > x0 { out.push((y, x0, x1)); }
    }
    out
}

/// The same outline as a per-pixel mask (X11's transparent window: alpha 0 outside).
pub fn outline_mask(w: i32, h: i32, rad: f32) -> Vec<bool> {
    let mut m = vec![false; (w.max(0) * h.max(0)) as usize];
    for (y, x0, x1) in outline_spans(w, h, rad) { for x in x0..x1 { m[(y * w + x) as usize] = true; } }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    const HD: Rect = Rect { x: 0, y: 0, w: 1920, h: 1040 };
    #[test]
    fn sizes() {
        assert_eq!(window_size(HD, false, false), (586, 977));
        assert_eq!(window_size(HD, true, false), (1804, 977));
        assert_eq!(window_size(Rect { x: 0, y: 0, w: 1280, h: 800 }, true, true), (1280, 800));
        assert_eq!(centered(Rect { x: 1920, y: 0, w: 1920, h: 1040 }, (586, 977)), (1920 + 667, 31));
    }
    #[test]
    fn wide_rules() {
        assert!(!want_wide(HD, "", "auto", false));
        assert!(want_wide(Rect { x: 0, y: 0, w: 1366, h: 728 }, "", "auto", false));
        assert!(want_wide(HD, "WIDE", "phone", false));
        assert!(!want_wide(HD, "phone", "wide", true));
        assert!(want_wide(HD, "", "wide\r\n", false));
        assert!(!want_wide(Rect { x: 0, y: 0, w: 900, h: 600 }, "wide", "", false));
        assert!(want_wide(Rect { x: 0, y: 0, w: 800, h: 1280 }, "", "", true));
    }
    #[test]
    fn outline() {
        let (w, h) = (586, 977);
        let rad = corner_radius(w, h);
        assert!((rad - 43.95).abs() < 0.01);
        let s = outline_spans(w, h, rad);
        assert_eq!(s.len(), h as usize);
        // Corners cut, middle rows full, symmetric left/right and top/bottom.
        assert!(s[0].1 > 10 && s[0].2 == w - s[0].1);
        assert_eq!((s[500].1, s[500].2), (0, w));
        assert_eq!((s[0].1, s[0].2), (s[h as usize - 1].1, s[h as usize - 1].2));
        let m = outline_mask(w, h, rad);
        assert!(!m[0] && m[(500 * w + 0) as usize]);
        assert_eq!(outline_spans(10, 4, 0.0).len(), 4);
    }
}
