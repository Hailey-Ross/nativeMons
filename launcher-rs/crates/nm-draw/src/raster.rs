//! GDI+'s scan converter, rebuilt from probes (notes: rust-launcher-step1.md, "GDI+ internals").
//! Coordinates: nearest 1/256 px, then up to 1/16 px. Antialiased: 8x4 samples per pixel. Aliased: pixel centers.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillMode {
    Alternate,
    Winding,
}

/// Device-space polygons. Antialiased: already shifted half a pixel, so pixel j spans j .. j+1.
pub type Polys = Vec<Vec<(f64, f64)>>;

/// Per pixel sample counts, 0..=32 (antialiased) or 0..=1 (aliased), over a bounding box.
pub struct Coverage {
    pub x0: i32,
    pub y0: i32,
    pub w: i32,
    pub h: i32,
    pub data: Vec<u8>,
    pub full: u8,
}

impl Coverage {
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < self.x0 || y < self.y0 || x >= self.x0 + self.w || y >= self.y0 + self.h { return 0; }
        self.data[((y - self.y0) * self.w + (x - self.x0)) as usize]
    }
}

fn ceil_div(n: i128, d: i128) -> i128 {
    // d > 0
    let q = n.div_euclid(d);
    if n.rem_euclid(d) != 0 { q + 1 } else { q }
}

struct Edge {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    dir: i32,
}

/// An edge crossing the sample rows: its crossing ceil(num / d) kept as floor quotient and remainder, stepped exactly
/// from one sample row to the next (the same integers the division gives, without dividing each row).
struct Active { k: usize, yr: i64, q: i64, rem: i64, d: i64, sq: i64, sr: i64 }

impl Active {
    fn new(k: usize, e: &Edge, yr: i64, sx: i64, sy: i64, small: bool) -> Active {
        let mut a = Active { k, yr: i64::MIN, q: 0, rem: 0, d: 1, sq: 0, sr: 0 };
        if small {
            let dy = e.y1 - e.y0;
            a.d = dy * sx;
            let step = sy * (e.x1 - e.x0);
            a.sq = step.div_euclid(a.d);
            a.sr = step.rem_euclid(a.d);
            a.set(e, yr);
        }
        a
    }
    fn set(&mut self, e: &Edge, yr: i64) {
        let num = e.x0 * (e.y1 - e.y0) + (yr - e.y0) * (e.x1 - e.x0);
        self.q = num.div_euclid(self.d);
        self.rem = num.rem_euclid(self.d);
        self.yr = yr;
    }
    /// ceil((x0 * dy + (yr - y0) * (x1 - x0)) / (dy * sx)) at sample row yr.
    #[inline]
    fn at(&mut self, e: &Edge, yr: i64, sx: i64, sy: i64) -> i64 {
        let _ = sx;
        if yr == self.yr + sy {
            self.q += self.sq;
            self.rem += self.sr;
            if self.rem >= self.d { self.rem -= self.d; self.q += 1; }
            self.yr = yr;
        } else if yr != self.yr {
            self.set(e, yr);
        }
        self.q + if self.rem != 0 { 1 } else { 0 }
    }
}

/// Samples at (sx * col, sy * row) in `unit` per px, after shifting by half a pixel.
fn scan(polys: &Polys, mode: FillMode, unit: f64, shift: f64, sx: i64, sy: i64, cols: i32, rows: i32, bound: (i32, i32, i32, i32)) -> Coverage {
    let mut edges = Vec::new();
    let (mut miny, mut maxy, mut minx, mut maxx) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for poly in polys {
        if poly.len() < 2 { continue; }
        let pts: Vec<(i64, i64)> = poly.iter().map(|&(x, y)| (crate::flatten::to_fix(x + shift, unit), crate::flatten::to_fix(y + shift, unit))).collect();
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            minx = minx.min(a.0);
            maxx = maxx.max(a.0);
            miny = miny.min(a.1);
            maxy = maxy.max(a.1);
            if a.1 == b.1 { continue; }
            if a.1 < b.1 { edges.push(Edge { x0: a.0, y0: a.1, x1: b.0, y1: b.1, dir: 1 }); } else { edges.push(Edge { x0: b.0, y0: b.1, x1: a.0, y1: a.1, dir: -1 }); }
        }
    }
    let (bx0, by0, bx1, by1) = bound;
    if edges.is_empty() {
        return Coverage { x0: 0, y0: 0, w: 0, h: 0, data: Vec::new(), full: (cols * rows) as u8 };
    }
    let px0 = ((minx.div_euclid(sx * cols as i64)) as i32).max(bx0);
    let px1 = ((maxx.div_euclid(sx * cols as i64)) as i32 + 1).min(bx1);
    let py0 = ((miny.div_euclid(sy * rows as i64)) as i32).max(by0);
    let py1 = ((maxy.div_euclid(sy * rows as i64)) as i32 + 1).min(by1);
    if px1 <= px0 || py1 <= py0 {
        return Coverage { x0: 0, y0: 0, w: 0, h: 0, data: Vec::new(), full: (cols * rows) as u8 };
    }
    let (w, h) = (px1 - px0, py1 - py0);
    let mut data = vec![0u8; (w * h) as usize];
    let mut xs: Vec<(i64, i32)> = Vec::new();
    let r_first = py0 as i64 * rows as i64;
    let r_last = py1 as i64 * rows as i64;
    // Only the edges crossing a row are visited (sorted by top); i64 is exact while coordinates stay under 2^29
    // units (|num| < 6 * 2^58), else the i128 path. Crossing order doesn't matter: ties make empty spans.
    let small = minx.abs().max(maxx.abs()).max(miny.abs()).max(maxy.abs()) < (1 << 29);
    edges.sort_by_key(|e| e.y0);
    let mut active: Vec<Active> = Vec::new();
    let mut next = 0;
    let (cols64, sx128) = (cols as i64, sx as i128);
    for r in r_first..r_last {
        let yr = r * sy;
        while next < edges.len() && edges[next].y0 <= yr { active.push(Active::new(next, &edges[next], yr, sx, sy, small)); next += 1; }
        active.retain(|a| edges[a.k].y1 > yr);
        if active.len() < 2 { continue; }
        xs.clear();
        for a in &mut active {
            let e = &edges[a.k];
            let c = if small {
                a.at(e, yr, sx, sy)
            } else {
                let dy = (e.y1 - e.y0) as i128;
                let num = e.x0 as i128 * dy + (yr - e.y0) as i128 * (e.x1 - e.x0) as i128;
                ceil_div(num, dy * sx128) as i64
            };
            xs.push((c, e.dir));
        }
        xs.sort_unstable_by_key(|v| v.0);
        let row = ((r.div_euclid(rows as i64)) as i32 - py0) as usize;
        let line = &mut data[row * w as usize..(row + 1) * w as usize];
        let mut wind = 0;
        for i in 0..xs.len() - 1 {
            wind += xs[i].1;
            let inside = match mode { FillMode::Alternate => (i + 1) % 2 == 1, FillMode::Winding => wind != 0 };
            if !inside { continue; }
            let (c0, c1) = (xs[i].0, xs[i + 1].0);
            if c1 <= c0 { continue; }
            let lo = c0.max(px0 as i64 * cols64);
            let hi = c1.min(px1 as i64 * cols64);
            if hi <= lo { continue; }
            let (p0, p1) = (lo.div_euclid(cols64), (hi - 1).div_euclid(cols64));
            let at = |p: i64| (p as i32 - px0) as usize;
            if p0 == p1 {
                line[at(p0)] += (hi - lo) as u8;
            } else {
                line[at(p0)] += ((p0 + 1) * cols64 - lo) as u8;
                for v in &mut line[at(p0 + 1)..at(p1)] { *v += cols as u8; }
                line[at(p1)] += (hi - p1 * cols64) as u8;
            }
        }
    }
    Coverage { x0: px0, y0: py0, w, h, data, full: (cols * rows) as u8 }
}

/// SmoothingMode.AntiAlias: counts 0..=32.
pub fn rasterize_aa(polys: &Polys, mode: FillMode, bound: (i32, i32, i32, i32)) -> Coverage {
    // 1/256 px units; columns every 32 units (1/8 px), rows every 64 (1/4 px).
    scan(polys, mode, 16.0, 0.0, 2, 4, 8, 4, bound)
}

/// Clip regions: like aliased fills, but coordinates round to the nearest 1/16 px (probed with rect clips).
pub fn rasterize_region(polys: &Polys, mode: FillMode, bound: (i32, i32, i32, i32)) -> Coverage {
    crate::flatten::ALIASED.with(|a| a.set(true));
    let c = rasterize_aliased(polys, mode, bound);
    crate::flatten::ALIASED.with(|a| a.set(false));
    c
}

/// Aliased fills: a pixel is in when its center is, top-left inclusive.
pub fn rasterize_aliased(polys: &Polys, mode: FillMode, bound: (i32, i32, i32, i32)) -> Coverage {
    scan(polys, mode, 16.0, 0.0, 16, 16, 1, 1, bound)
}

/// The fallback: tiny-skia's scan converter (supersampled AA) over the same polygons, as 0..=255 coverage.
pub fn rasterize_tiny_skia(polys: &Polys, mode: FillMode, aa: bool, bound: (i32, i32, i32, i32)) -> Coverage {
    let (w, h) = (bound.2 - bound.0, bound.3 - bound.1);
    let empty = Coverage { x0: 0, y0: 0, w: 0, h: 0, data: Vec::new(), full: 255 };
    let mut pb = tiny_skia::PathBuilder::new();
    for poly in polys {
        if poly.len() < 3 { continue; }
        // Ours is shifted half a pixel for antialiasing; tiny-skia samples at pixel centers already.
        let s = if aa { 0.0 } else { 0.5 };
        pb.move_to((poly[0].0 + s) as f32, (poly[0].1 + s) as f32);
        for p in &poly[1..] { pb.line_to((p.0 + s) as f32, (p.1 + s) as f32); }
        pb.close();
    }
    let (Some(path), Some(mut mask)) = (pb.finish(), tiny_skia::Mask::new(w.max(1) as u32, h.max(1) as u32)) else { return empty };
    let rule = if mode == FillMode::Winding { tiny_skia::FillRule::Winding } else { tiny_skia::FillRule::EvenOdd };
    mask.fill_path(&path, rule, aa, tiny_skia::Transform::from_translate(-bound.0 as f32, -bound.1 as f32));
    Coverage { x0: bound.0, y0: bound.1, w, h, data: mask.data().to_vec(), full: 255 }
}
