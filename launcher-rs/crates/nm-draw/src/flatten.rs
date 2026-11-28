//! Bezier flattening the way GDI+ does it: forward differencing in fixed point (GDI's "HFD" cracker).

use crate::path::{GraphicsPath, PtType};
use crate::matrix::Matrix;

#[derive(Clone, Copy, Debug)]
pub struct FlattenCfg {
    /// Fixed-point units per pixel the curve is cracked in.
    pub unit: f64,
    /// 6x the error allowed, in those units.
    pub tol: i64,
    /// Halve and double the step along the curve (GDI) or keep it uniform.
    pub adaptive: bool,
}

/// GDI+'s: 28.4 fixed point, error 1/4 px (24 = 6 x 4), adaptive steps (probed: ellipses match exactly).
pub fn cfg() -> FlattenCfg { FlattenCfg { unit: 16.0, tol: 24, adaptive: true } }

/// GDI+'s float to fixed point: to 1/256 px (nearest), then up to the 1/16 grid.
pub fn to_fix(v: f64, unit: f64) -> i64 {
    if unit == 16.0 && ALIASED.with(|a| a.get()) { return (v * 16.0 + 0.5).floor() as i64; }
    let q = (v * 256.0 + 0.5).floor() as i64;
    let k = (256.0 / unit) as i64;
    q.div_euclid(k) + if q.rem_euclid(k) != 0 { 1 } else { 0 }
}

thread_local! {
    /// Set while a clip region's points are converted.
    pub static ALIASED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const ISH: u32 = 10;
const ADD: u32 = 3;

#[derive(Clone, Copy)]
struct Hfd {
    e0: i64,
    e1: i64,
    e2: i64,
    e3: i64,
}

impl Hfd {
    fn new(p1: i64, p2: i64, p3: i64, p4: i64) -> Hfd {
        Hfd { e0: p1 << ISH, e1: (p4 - p1) << ISH, e2: (6 * (p2 - p3 - p3 + p4)) << ISH, e3: (6 * (p1 - p2 - p2 + p3)) << ISH }
    }
    fn err(&self) -> i64 { self.e2.abs().max(self.e3.abs()) }
    fn parent_err(&self) -> i64 { self.e3.abs().max((self.e2 + self.e2 - self.e3).abs()) }
    fn lazy_halve(&mut self, c: u32) {
        self.e2 = (self.e2 + self.e3) >> 1;
        self.e1 = (self.e1 - (self.e2 >> c)) >> 1;
    }
    fn steady(&mut self, c: u32) {
        self.e0 <<= ADD;
        self.e1 <<= ADD;
        let l = c as i64 - ADD as i64;
        if l < 0 {
            self.e2 <<= -l;
            self.e3 <<= -l;
        } else {
            self.e2 >>= l;
            self.e3 >>= l;
        }
    }
    fn halve(&mut self) {
        self.e2 = (self.e2 + self.e3) >> 3;
        self.e1 = (self.e1 - self.e2) >> 1;
        self.e3 >>= 2;
    }
    fn double(&mut self) {
        self.e1 += self.e1 + self.e2;
        self.e3 <<= 2;
        self.e2 = (self.e2 << 3) - self.e3;
    }
    fn step(&mut self) {
        self.e0 += self.e1;
        let t = self.e2;
        self.e1 += t;
        self.e2 += t - self.e3;
        self.e3 = t;
    }
    fn value(&self) -> i64 {
        let sh = ISH + ADD;
        (self.e0 + (1 << (sh - 1))) >> sh
    }
}

/// Points after the first, in fixed units.
fn crack(p: [(i64, i64); 4], c: &FlattenCfg, out: &mut Vec<(i64, i64)>) {
    let left = p.iter().map(|q| q.0).min().unwrap() - 16;
    let top = p.iter().map(|q| q.1).min().unwrap() - 16;
    let mut x = Hfd::new(p[0].0 - left, p[1].0 - left, p[2].0 - left, p[3].0 - left);
    let mut y = Hfd::new(p[0].1 - top, p[1].1 - top, p[2].1 - top, p[3].1 - top);
    let mut steps: i64 = 1;
    let mut sh = 0u32;
    loop {
        let m = (c.tol << ISH) << sh;
        if x.err() <= m && y.err() <= m { break; }
        if sh > 40 { break; }
        sh += 2;
        x.lazy_halve(sh);
        y.lazy_halve(sh);
        steps <<= 1;
    }
    x.steady(sh);
    y.steady(sh);
    let tm = (c.tol << ISH) << ADD;
    if !c.adaptive {
        for _ in 0..steps {
            x.step();
            y.step();
            out.push((x.value() + left, y.value() + top));
        }
        return;
    }
    x.step();
    y.step();
    steps -= 1;
    loop {
        out.push((x.value() + left, y.value() + top));
        if steps == 0 { return; }
        if x.err().max(y.err()) > tm {
            x.halve();
            y.halve();
            steps <<= 1;
        }
        while steps & 1 == 0 && x.parent_err() <= tm >> 2 && y.parent_err() <= tm >> 2 {
            x.double();
            y.double();
            steps >>= 1;
        }
        steps -= 1;
        x.step();
        y.step();
    }
}

/// The path in device space as polygons (each figure closed for filling), plus whether each figure was closed.
pub fn flatten(path: &GraphicsPath, m: &Matrix, dx: f32, c: &FlattenCfg) -> Vec<(Vec<(f64, f64)>, bool)> { flatten_ex(path, m, dx, c, false) }

/// `float_lines`: line vertices stay unsnapped (pens widen from them); curve points are always on the 1/16 grid.
pub fn flatten_ex(path: &GraphicsPath, m: &Matrix, dx: f32, c: &FlattenCfg, float_lines: bool) -> Vec<(Vec<(f64, f64)>, bool)> {
    let dev = |p: crate::geom::PointF| {
        let d = m.apply(p);
        ((d.x + dx) as f64, (d.y + dx) as f64)
    };
    let fix = |q: (f64, f64)| (to_fix(q.0, c.unit), to_fix(q.1, c.unit));
    let unfix = |q: (i64, i64)| (q.0 as f64 / c.unit, q.1 as f64 / c.unit);
    let mut out = Vec::new();
    for (fig, closed) in path.figures() {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let mut i = 0;
        while i < fig.len() {
            let pp = fig[i];
            match pp.t {
                PtType::Start | PtType::Line => {
                    let d = dev(pp.p);
                    let curve_next = i + 1 < fig.len() && fig[i + 1].t == PtType::Bezier;
                    pts.push(if float_lines && !curve_next { d } else { unfix(fix(d)) });
                    i += 1;
                }
                PtType::Bezier => {
                    if i + 2 >= fig.len() || pts.is_empty() {
                        i += 1;
                        continue;
                    }
                    let p0 = fix(*pts.last().unwrap());
                    let mut cracked = Vec::new();
                    crack([p0, fix(dev(fig[i].p)), fix(dev(fig[i + 1].p)), fix(dev(fig[i + 2].p))], c, &mut cracked);
                    pts.extend(cracked.into_iter().map(unfix));
                    i += 3;
                }
            }
        }
        out.push((pts, closed));
    }
    out
}

/// One device-space bezier as points after the first.
pub fn crack_f64(p: [(f64, f64); 4], c: &FlattenCfg) -> Vec<(f64, f64)> {
    let fix = |q: (f64, f64)| (to_fix(q.0, c.unit), to_fix(q.1, c.unit));
    let mut out = Vec::new();
    crack([fix(p[0]), fix(p[1]), fix(p[2]), fix(p[3])], c, &mut out);
    out.iter().map(|&(x, y)| (x as f64 / c.unit, y as f64 / c.unit)).collect()
}
