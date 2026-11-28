//! Pen widening: every segment, join and cap as its own positively wound polygon, filled Winding (their union).

use crate::flatten::{crack_f64, FlattenCfg};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineCap {
    Flat,
    Square,
    Round,
    Triangle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineJoin {
    Miter,
    Bevel,
    Round,
    MiterClipped,
}

#[derive(Clone, Copy)]
pub struct StrokeStyle {
    pub width: f64,
    pub start_cap: LineCap,
    pub end_cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f64,
}

type P = (f64, f64);

fn area(p: &[P]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (x0, y0) = p[i];
        let (x1, y1) = p[(i + 1) % p.len()];
        a += x0 * y1 - x1 * y0;
    }
    a
}

fn push(out: &mut Vec<Vec<P>>, mut p: Vec<P>) {
    if p.len() < 3 { return; }
    let a = area(&p);
    if a.abs() < 1e-12 { return; }
    if a < 0.0 { p.reverse(); }
    out.push(p);
}

fn circle(c: P, r: f64, cfg: &FlattenCfg) -> Vec<P> {
    const K: f64 = 0.5522847498;
    let (x, y) = c;
    let k = r * K;
    let segs = [
        [(x + r, y), (x + r, y + k), (x + k, y + r), (x, y + r)],
        [(x, y + r), (x - k, y + r), (x - r, y + k), (x - r, y)],
        [(x - r, y), (x - r, y - k), (x - k, y - r), (x, y - r)],
        [(x, y - r), (x + k, y - r), (x + r, y - k), (x + r, y)],
    ];
    let mut pts = vec![(x + r, y)];
    for s in segs { pts.extend(crack_f64(s, cfg)); }
    pts.pop();
    pts
}

/// GDI+'s nominal-width pen (device width up to 1.5): each segment swept by a diamond of radius 0.5.
pub fn nominal(line: &[P], closed: bool, out: &mut Vec<Vec<P>>) {
    let mut pts: Vec<P> = line.to_vec();
    if closed && pts.len() > 1 { pts.push(pts[0]); }
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[i], pts[i + 1]);
        let mut cloud = Vec::with_capacity(8);
        for c in [a, b] {
            cloud.extend_from_slice(&[(c.0 - 0.5, c.1), (c.0, c.1 - 0.5), (c.0 + 0.5, c.1), (c.0, c.1 + 0.5)]);
        }
        push(out, hull(cloud));
    }
}

fn hull(mut p: Vec<P>) -> Vec<P> {
    p.sort_by(|a, b| a.partial_cmp(b).unwrap());
    p.dedup();
    if p.len() < 3 { return p; }
    let cross = |o: P, a: P, b: P| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let build = |it: &mut dyn Iterator<Item = P>| {
        let mut h: Vec<P> = Vec::new();
        for q in it {
            while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 { h.pop(); }
            h.push(q);
        }
        h.pop();
        h
    };
    let mut lower = build(&mut p.iter().copied());
    let upper = build(&mut p.iter().rev().copied());
    lower.extend(upper);
    lower
}

fn raw_push(out: &mut Vec<Vec<P>>, p: Vec<P>) {
    if p.len() >= 3 { out.push(p); }
}

/// Points of a circular arc around `c` from angle a0 to a1 (radians, sweep under 180 degrees), after the first.
fn arc_points(c: P, r: f64, a0: f64, a1: f64, cfg: &FlattenCfg) -> Vec<P> {
    let mut out = Vec::new();
    let sweep = a1 - a0;
    let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = sweep / n as f64;
    let mut a = a0;
    for _ in 0..n {
        let b = a + step;
        let half = (b - a) / 2.0;
        let k = 4.0 / 3.0 * (1.0 - half.cos()) / half.sin();
        let (cs, ss, ce, se) = (a.cos(), a.sin(), b.cos(), b.sin());
        let p0 = (c.0 + r * cs, c.1 + r * ss);
        let p1 = (c.0 + r * (cs - k * ss), c.1 + r * (ss + k * cs));
        let p2 = (c.0 + r * (ce + k * se), c.1 + r * (se - k * ce));
        let p3 = (c.0 + r * ce, c.1 + r * se);
        out.extend(crack_f64([p0, p1, p2, p3], cfg));
        a = b;
    }
    out
}

fn line_x(a: P, da: P, b: P, db: P) -> Option<P> {
    let den = da.0 * db.1 - da.1 * db.0;
    if den.abs() < 1e-12 { return None; }
    let t = ((b.0 - a.0) * db.1 - (b.1 - a.1) * db.0) / den;
    Some((a.0 + da.0 * t, a.1 + da.1 * t))
}

/// One side of the outline: offset by s * half width, with joins.
fn side(pts: &[P], closed: bool, s: f64, st: &StrokeStyle, cfg: &FlattenCfg) -> Vec<P> {
    let n = pts.len();
    let hw = st.width / 2.0;
    let segs = if closed { n } else { n - 1 };
    let dir: Vec<P> = (0..segs).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        ((b.0 - a.0) / l, (b.1 - a.1) / l)
    }).collect();
    let nrm = |d: P| (-d.1 * hw * s, d.0 * hw * s);
    let mut out = Vec::new();
    let joins: Vec<usize> = if closed { (0..n).collect() } else { (1..n - 1).collect() };
    if !closed {
        let o = nrm(dir[0]);
        out.push((pts[0].0 + o.0, pts[0].1 + o.1));
    }
    for k in joins {
        let p = pts[k];
        let d1 = dir[(k + segs - 1) % segs];
        let d2 = dir[k % segs];
        let (o1, o2) = (nrm(d1), nrm(d2));
        let a = (p.0 + o1.0, p.1 + o1.1);
        let b = (p.0 + o2.0, p.1 + o2.1);
        let cross = d1.0 * d2.1 - d1.1 * d2.0;
        if cross.abs() < 1e-12 {
            out.push(a);
            continue;
        }
        let outer = s * cross < 0.0;
        if !outer {
            // The inner side keeps both offset points (probed: intersecting them is off by a sample).
            out.push(a);
            out.push(b);
            continue;
        }
        match st.join {
            LineJoin::Round => {
                out.push(a);
                let a0 = (a.1 - p.1).atan2(a.0 - p.0);
                let mut a1 = (b.1 - p.1).atan2(b.0 - p.0);
                let pi = std::f64::consts::PI;
                if s * cross < 0.0 {
                    // the short way round, the side the path turns away from
                    while a1 - a0 > pi { a1 -= 2.0 * pi; }
                    while a0 - a1 > pi { a1 += 2.0 * pi; }
                }
                out.extend(arc_points(p, hw, a0, a1, cfg));
            }
            LineJoin::Bevel => {
                out.push(a);
                out.push(b);
            }
            _ => {
                let tip = line_x(a, d1, b, d2).unwrap_or(a);
                let ml = ((tip.0 - p.0).powi(2) + (tip.1 - p.1).powi(2)).sqrt();
                let lim = st.miter_limit * hw;
                if ml <= lim {
                    out.push(tip);
                } else {
                    // GDI+'s Miter (not MiterClipped): the miter cut square at the limit, not a bevel.
                    let m = ((tip.0 - p.0) / ml, (tip.1 - p.1) / ml);
                    let dot = |v: P, w: P| v.0 * w.0 + v.1 * w.1;
                    let (ra, rb) = ((a.0 - p.0, a.1 - p.1), (b.0 - p.0, b.1 - p.1));
                    let (k1, k2) = (dot(d1, m), dot(d2, m));
                    if k1.abs() < 1e-12 || k2.abs() < 1e-12 {
                        out.push(a);
                        out.push(b);
                    } else {
                        let t1 = (lim - dot(ra, m)) / k1;
                        let t2 = (dot(rb, m) - lim) / k2;
                        out.push(a);
                        out.push((a.0 + d1.0 * t1, a.1 + d1.1 * t1));
                        out.push((b.0 - d2.0 * t2, b.1 - d2.1 * t2));
                        out.push(b);
                    }
                }
            }
        }
    }
    if !closed {
        let o = nrm(dir[segs - 1]);
        out.push((pts[n - 1].0 + o.0, pts[n - 1].1 + o.1));
    }
    out
}

/// The end cap's points from side +1 to side -1 at `p`, going out along `d`.
fn cap_points(p: P, d: P, c: LineCap, hw: f64, cfg: &FlattenCfg) -> Vec<P> {
    let n = (-d.1 * hw, d.0 * hw);
    let u = (d.0 * hw, d.1 * hw);
    match c {
        LineCap::Round => {
            let a0 = n.1.atan2(n.0);
            let mut v = arc_points(p, hw, a0, a0 - std::f64::consts::PI, cfg);
            v.pop();
            v
        }
        LineCap::Square => vec![(p.0 + n.0 + u.0, p.1 + n.1 + u.1), (p.0 - n.0 + u.0, p.1 - n.1 + u.1)],
        LineCap::Triangle => vec![(p.0 + u.0, p.1 + u.1)],
        LineCap::Flat => vec![],
    }
}

/// The stroke of one polyline as GDI+ widens it: one outline (two rings when closed), filled Winding.
pub fn widen_outline(line: &[P], closed: bool, st: &StrokeStyle, cfg: &FlattenCfg, out: &mut Vec<Vec<P>>) {
    let mut pts: Vec<P> = Vec::new();
    for &p in line {
        if pts.last().map_or(true, |&q: &P| (q.0 - p.0).abs() > 1e-9 || (q.1 - p.1).abs() > 1e-9) { pts.push(p); }
    }
    if closed && pts.len() > 1 && pts[0] == *pts.last().unwrap() { pts.pop(); }
    let hw = st.width / 2.0;
    if pts.len() < 2 {
        if pts.len() == 1 && st.start_cap == LineCap::Round { push(out, circle(pts[0], hw, cfg)); }
        return;
    }
    if closed && pts.len() >= 3 {
        let a = side(&pts, true, 1.0, st, cfg);
        let mut b = side(&pts, true, -1.0, st, cfg);
        b.reverse();
        raw_push(out, a);
        raw_push(out, b);
        return;
    }
    let n = pts.len();
    let unit = |a: P, b: P| {
        let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        ((b.0 - a.0) / l, (b.1 - a.1) / l)
    };
    let a = side(&pts, false, 1.0, st, cfg);
    let b = side(&pts, false, -1.0, st, cfg);
    let mut poly = a;
    poly.extend(cap_points(pts[n - 1], unit(pts[n - 2], pts[n - 1]), st.end_cap, hw, cfg));
    poly.extend(b.into_iter().rev());
    poly.extend(cap_points(pts[0], unit(pts[1], pts[0]), st.start_cap, hw, cfg));
    raw_push(out, poly);
}

