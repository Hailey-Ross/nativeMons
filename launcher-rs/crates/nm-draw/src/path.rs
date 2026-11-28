use crate::geom::{PointF, RectangleF};
use crate::raster::FillMode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PtType {
    Start,
    Line,
    Bezier,
}

/// One point of a GraphicsPath: its type, and whether it closes its figure.
#[derive(Clone, Copy, Debug)]
pub struct PathPoint {
    pub p: PointF,
    pub t: PtType,
    pub close: bool,
}

/// System.Drawing.Drawing2D.GraphicsPath, world coordinates.
#[derive(Clone, Debug)]
pub struct GraphicsPath {
    pub pts: Vec<PathPoint>,
    pub fill_mode: FillMode,
    new_figure: bool,
}

impl Default for GraphicsPath {
    fn default() -> Self { GraphicsPath::new() }
}

const KAPPA: f32 = 0.5522847498;

impl GraphicsPath {
    pub fn new() -> GraphicsPath { GraphicsPath { pts: Vec::new(), fill_mode: FillMode::Alternate, new_figure: true } }

    fn push(&mut self, p: PointF, t: PtType) {
        let t = if self.new_figure { PtType::Start } else { t };
        self.pts.push(PathPoint { p, t, close: false });
        self.new_figure = false;
    }
    /// Lines and arcs join the open figure: its last point connects to their first.
    fn push_joined(&mut self, p: PointF) {
        if self.new_figure {
            self.push(p, PtType::Start);
        } else {
            let last = self.pts.last().unwrap().p;
            if last != p { self.push(p, PtType::Line); }
        }
    }
    pub fn start_figure(&mut self) { self.new_figure = true; }
    pub fn close_figure(&mut self) {
        if let Some(l) = self.pts.last_mut() {
            if !self.new_figure { l.close = true; }
        }
        self.new_figure = true;
    }
    pub fn add_line(&mut self, a: PointF, b: PointF) {
        self.push_joined(a);
        self.push(b, PtType::Line);
    }
    pub fn add_lines(&mut self, pts: &[PointF]) {
        if pts.is_empty() { return; }
        self.push_joined(pts[0]);
        for &p in &pts[1..] { self.push(p, PtType::Line); }
    }
    pub fn add_polygon(&mut self, pts: &[PointF]) {
        self.start_figure();
        self.add_lines(pts);
        self.close_figure();
    }
    pub fn add_rectangle(&mut self, r: RectangleF) {
        self.add_polygon(&[PointF::new(r.x, r.y), PointF::new(r.right(), r.y), PointF::new(r.right(), r.bottom()), PointF::new(r.x, r.bottom())]);
    }
    pub fn add_bezier(&mut self, p1: PointF, p2: PointF, p3: PointF, p4: PointF) {
        self.push_joined(p1);
        self.push(p2, PtType::Bezier);
        self.push(p3, PtType::Bezier);
        self.push(p4, PtType::Bezier);
    }
    pub fn add_ellipse(&mut self, r: RectangleF) {
        let (rx, ry) = (r.width / 2.0, r.height / 2.0);
        let (cx, cy) = (r.x + rx, r.y + ry);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        self.start_figure();
        let p = |x: f32, y: f32| PointF::new(x, y);
        self.push(p(cx + rx, cy), PtType::Start);
        for c in [
            [p(cx + rx, cy + ky), p(cx + kx, cy + ry), p(cx, cy + ry)],
            [p(cx - kx, cy + ry), p(cx - rx, cy + ky), p(cx - rx, cy)],
            [p(cx - rx, cy - ky), p(cx - kx, cy - ry), p(cx, cy - ry)],
            [p(cx + kx, cy - ry), p(cx + rx, cy - ky), p(cx + rx, cy)],
        ] {
            for q in c { self.push(q, PtType::Bezier); }
        }
        self.close_figure();
    }
    /// AddArc: angles in degrees, clockwise, measured on the ellipse as GDI+ does.
    pub fn add_arc(&mut self, r: RectangleF, start: f32, sweep: f32) {
        let b = arc_beziers(r, start, sweep);
        if b.is_empty() { return; }
        self.push_joined(b[0]);
        for q in &b[1..] { self.push(*q, PtType::Bezier); }
    }
    pub fn add_pie(&mut self, r: RectangleF, start: f32, sweep: f32) {
        self.start_figure();
        let c = PointF::new(r.x + r.width / 2.0, r.y + r.height / 2.0);
        self.push(c, PtType::Start);
        let b = arc_beziers(r, start, sweep);
        if !b.is_empty() {
            self.push(b[0], PtType::Line);
            for q in &b[1..] { self.push(*q, PtType::Bezier); }
        }
        self.close_figure();
    }

    /// Figures as (points with types, closed).
    pub fn figures(&self) -> Vec<(Vec<PathPoint>, bool)> {
        let mut out: Vec<(Vec<PathPoint>, bool)> = Vec::new();
        for pp in &self.pts {
            if pp.t == PtType::Start || out.is_empty() { out.push((Vec::new(), false)); }
            let f = out.last_mut().unwrap();
            f.0.push(*pp);
            if pp.close { f.1 = true; }
        }
        out
    }
}

fn unstretch(angle_deg: f64, rx: f64, ry: f64) -> f64 {
    let a = angle_deg.to_radians();
    if a.cos().abs() < 0.00001 || a.sin().abs() < 0.00001 { return a; }
    let s = (a.sin() / ry.abs()).atan2(a.cos() / rx.abs());
    let revs = (a / (2.0 * std::f64::consts::PI)).round() - (s / (2.0 * std::f64::consts::PI)).round();
    s + revs * std::f64::consts::PI * 2.0
}

/// The arc as cubic bezier points (first point, then 3 per piece), at most 90 degrees of the ellipse's own angle each.
pub fn arc_beziers(r: RectangleF, start: f32, sweep: f32) -> Vec<PointF> {
    let (x, y, w, h) = (r.x as f64, r.y as f64, r.width as f64, r.height as f64);
    let (rx, ry) = (w / 2.0, h / 2.0);
    let (cx, cy) = (x + rx, y + ry);
    let mut sweep = sweep as f64;
    if sweep > 360.0 { sweep = 360.0; }
    if sweep < -360.0 { sweep = -360.0; }
    let end = unstretch(start as f64 + sweep, rx, ry);
    let mut a = unstretch(start as f64, rx, ry);
    let half_pi = std::f64::consts::FRAC_PI_2;
    let mut out: Vec<PointF> = Vec::new();
    for _ in 0..8 {
        let b;
        if sweep > 0.0 {
            if a >= end - 1e-12 { break; }
            b = (a + half_pi).min(end);
        } else {
            if a <= end + 1e-12 { break; }
            b = (a - half_pi).max(end);
        }
        let half = (b - a) / 2.0;
        let k = 4.0 / 3.0 * (1.0 - half.cos()) / half.sin();
        let (cs, ss, ce, se) = (a.cos(), a.sin(), b.cos(), b.sin());
        let pt = |ux: f64, uy: f64| PointF::new((ux * rx + cx) as f32, (uy * ry + cy) as f32);
        if out.is_empty() { out.push(pt(cs, ss)); }
        out.push(pt(cs - k * ss, ss + k * cs));
        out.push(pt(ce + k * se, se - k * ce));
        out.push(pt(ce, se));
        a += if sweep < 0.0 { -half_pi } else { half_pi };
    }
    out
}
