use crate::color::Color;
use crate::geom::{PointF, RectangleF};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WrapMode {
    Tile,
    TileFlipX,
    TileFlipY,
    TileFlipXY,
    Clamp,
}

/// LinearGradientBrush(rect, c1, c2, angle): t runs 0..1 between the rect's corners along the angle.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    pub rect: RectangleF,
    pub c1: Color,
    pub c2: Color,
    pub angle: f32,
    pub wrap: WrapMode,
    /// Blend: factors at positions (0..1), or empty.
    pub blend: Vec<(f32, f32)>,
    /// ColorBlend (InterpolationColors): colors at positions, or empty.
    pub colors: Vec<(Color, f32)>,
}

impl LinearGradient {
    pub fn new(rect: RectangleF, c1: Color, c2: Color, angle: f32) -> LinearGradient {
        LinearGradient { rect, c1, c2, angle, wrap: WrapMode::Tile, blend: Vec::new(), colors: Vec::new() }
    }
}

/// PathGradientBrush over an ellipse: CenterColor at the center, SurroundColors[0] on the edge.
#[derive(Clone, Debug, PartialEq)]
pub struct EllipseGradient {
    pub rect: RectangleF,
    pub center: Color,
    pub surround: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Brush {
    Solid(Color),
    Linear(LinearGradient),
    Ellipse(EllipseGradient),
}

impl From<Color> for Brush {
    fn from(c: Color) -> Brush { Brush::Solid(c) }
}

/// A color premultiplied by its alpha: [a, r, g, b].
pub(crate) fn premul(c: Color) -> [f32; 4] {
    let p = |v: u8| ((v as u32 * c.a as u32 + 128 + ((v as u32 * c.a as u32 + 128) >> 8)) >> 8) as f32;
    [c.a as f32, p(c.r), p(c.g), p(c.b)]
}
/// GDI+ interpolates gradient colors premultiplied (probed, t18): [a, r, g, b] premultiplied, rounded.
pub(crate) fn lerp_pm(a: Color, b: Color, t: f32) -> [u32; 4] {
    let (pa, pb) = (premul(a), premul(b));
    let mut o = [0u32; 4];
    for i in 0..4 { o[i] = (pa[i] + (pb[i] - pa[i]) * t + 0.5).floor().clamp(0.0, 255.0) as u32; }
    for i in 1..4 { o[i] = o[i].min(o[0]); }
    o
}

impl LinearGradient {
    /// The gradient's parameter at a world point, before wrapping.
    pub fn param(&self, p: PointF) -> f64 { self.param_with(self.param_prep(), p) }
    /// The direction and the corners' projected range (dx, dy, lo, hi), for param_with.
    pub fn param_prep(&self) -> (f64, f64, f64, f64) {
        let r = self.rect;
        let a = (self.angle as f64).to_radians();
        let (dx, dy) = ((a.cos() as f32) as f64, (a.sin() as f32) as f64);
        let corners = [(r.x, r.y), (r.right(), r.y), (r.x, r.bottom()), (r.right(), r.bottom())];
        let proj = corners.map(|(x, y)| x as f64 * dx + y as f64 * dy);
        let lo = proj.iter().cloned().fold(f64::MAX, f64::min);
        let hi = proj.iter().cloned().fold(f64::MIN, f64::max);
        (dx, dy, lo, hi)
    }
    #[inline]
    pub fn param_with(&self, (dx, dy, lo, hi): (f64, f64, f64, f64), p: PointF) -> f64 {
        let v = p.x as f64 * dx + p.y as f64 * dy;
        if hi > lo { (v - lo) / (hi - lo) } else { 0.0 }
    }
    /// color_at's premultiplied end colors, once per fill (the ColorBlend case keeps color_at).
    pub fn ends_pm(&self) -> Option<([f32; 4], [f32; 4])> { if self.colors.is_empty() { Some((premul(self.c1), premul(self.c2))) } else { None } }
    /// color_at with ends_pm's colors: the same steps.
    #[inline]
    pub fn color_at_pm(&self, t: f64, (pa, pb): ([f32; 4], [f32; 4])) -> [u32; 4] {
        let t = self.wrapped(t) as f32;
        let mut k = t;
        if !self.blend.is_empty() {
            let b = &self.blend;
            k = b[b.len() - 1].0;
            for i in 0..b.len() - 1 {
                if t >= b[i].1 && t <= b[i + 1].1 {
                    let span = b[i + 1].1 - b[i].1;
                    let f = if span > 0.0 { (t - b[i].1) / span } else { 0.0 };
                    k = b[i].0 + (b[i + 1].0 - b[i].0) * f;
                    break;
                }
            }
        }
        let mut o = [0u32; 4];
        for i in 0..4 { o[i] = (pa[i] + (pb[i] - pa[i]) * k + 0.5).floor().clamp(0.0, 255.0) as u32; }
        for i in 1..4 { o[i] = o[i].min(o[0]); }
        o
    }
    #[inline]
    fn wrapped(&self, t: f64) -> f64 {
        let mut t = t;
        match self.wrap {
            WrapMode::Tile => t = t - t.floor(),
            WrapMode::TileFlipX | WrapMode::TileFlipXY => {
                let k = t.floor();
                t -= k;
                if (k as i64).rem_euclid(2) == 1 { t = 1.0 - t; }
            }
            _ => t = t.clamp(0.0, 1.0),
        }
        t
    }
    /// The premultiplied color at t: [a, r, g, b].
    pub fn color_at(&self, t: f64) -> [u32; 4] {
        let mut t = t;
        match self.wrap {
            WrapMode::Tile => t = t - t.floor(),
            WrapMode::TileFlipX | WrapMode::TileFlipXY => {
                let k = t.floor();
                t -= k;
                if (k as i64).rem_euclid(2) == 1 { t = 1.0 - t; }
            }
            _ => t = t.clamp(0.0, 1.0),
        }
        let t = t as f32;
        if !self.colors.is_empty() {
            let c = &self.colors;
            for i in 0..c.len() - 1 {
                if t >= c[i].1 && t <= c[i + 1].1 {
                    let span = c[i + 1].1 - c[i].1;
                    let k = if span > 0.0 { (t - c[i].1) / span } else { 0.0 };
                    return lerp_pm(c[i].0, c[i + 1].0, k);
                }
            }
            return premul(c[c.len() - 1].0).map(|v| v as u32);
        }
        let mut k = t;
        if !self.blend.is_empty() {
            let b = &self.blend;
            k = b[b.len() - 1].0;
            for i in 0..b.len() - 1 {
                if t >= b[i].1 && t <= b[i + 1].1 {
                    let span = b[i + 1].1 - b[i].1;
                    let f = if span > 0.0 { (t - b[i].1) / span } else { 0.0 };
                    k = b[i].0 + (b[i + 1].0 - b[i].0) * f;
                    break;
                }
            }
        }
        lerp_pm(self.c1, self.c2, k)
    }
}
