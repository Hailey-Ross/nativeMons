#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PointF {
    pub x: f32,
    pub y: f32,
}
impl PointF {
    pub const fn new(x: f32, y: f32) -> PointF { PointF { x, y } }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SizeF {
    pub width: f32,
    pub height: f32,
}
impl SizeF {
    pub const fn new(width: f32, height: f32) -> SizeF { SizeF { width, height } }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectangleF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RectangleF {
    pub const EMPTY: RectangleF = RectangleF { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> RectangleF { RectangleF { x, y, width, height } }
    pub fn left(&self) -> f32 { self.x }
    pub fn top(&self) -> f32 { self.y }
    pub fn right(&self) -> f32 { self.x + self.width }
    pub fn bottom(&self) -> f32 { self.y + self.height }
    pub fn is_empty(&self) -> bool { self.width <= 0.0 || self.height <= 0.0 }
    /// RectangleF.Intersect: Empty when they don't overlap.
    pub fn intersect(a: RectangleF, b: RectangleF) -> RectangleF {
        let x1 = a.x.max(b.x);
        let x2 = a.right().min(b.right());
        let y1 = a.y.max(b.y);
        let y2 = a.bottom().min(b.bottom());
        if x2 >= x1 && y2 >= y1 { RectangleF::new(x1, y1, x2 - x1, y2 - y1) } else { RectangleF::EMPTY }
    }
    pub fn intersects_with(&self, r: RectangleF) -> bool {
        r.x < self.right() && self.x < r.right() && r.y < self.bottom() && self.y < r.bottom()
    }
    pub fn contains(&self, p: PointF) -> bool { self.x <= p.x && p.x < self.right() && self.y <= p.y && p.y < self.bottom() }
}
