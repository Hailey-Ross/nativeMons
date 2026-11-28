//! GDI+'s InterpolationMode.HighQualityBicubic, rebuilt from probes (tools/gdiplus-probe, t49-t53).
//! Scales and moves are separable and bit-exact; rotations use a 4x4 Keys model (close, not exact).

use std::sync::OnceLock;

/// The Keys cubic with a = -1 (GDI+'s high-quality kernel).
fn keys(x: f64, a: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 { (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0 } else if x < 2.0 { a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a } else { 0.0 }
}

/// The kernel's running integral in 16.16, 128 bins a unit: the sum of the samples at i/128 up to bin i, minus a half.
fn table() -> &'static [i64; 513] {
    static T: OnceLock<[i64; 513]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0i64; 513];
        let mut sum = 0f64;
        for (n, v) in t.iter_mut().enumerate() {
            sum += keys((n as f64 - 256.0) / 128.0, -1.0);
            *v = ((sum / 128.0 - 0.5) * 65536.0).round_ties_even() as i64;
        }
        t
    })
}
fn t_at(i: i64) -> i64 { if i < -256 { -32768 } else if i >= 256 { 32768 } else { table()[(i + 256) as usize] } }

/// GDI+ moves a column's phase the wrong way within a pixel: x is reflected about the pixel's middle (within 1/32 of a
/// whole pixel, about that pixel). Rows use y as it is.
fn mirror(o: f64) -> f64 {
    let n = o.floor();
    if o - n <= 1.0 / 32.0 { 2.0 * n - o } else if o - n >= 31.0 / 32.0 { 2.0 * n + 2.0 - o } else { 2.0 * n + 1.0 - o }
}
/// Where a same-size draw is a plain copy: within 1/64 past a whole pixel, or less than 1/64 before one (probed).
pub fn snap(v: f64) -> Option<i32> {
    let n = v.floor();
    if v - n <= 1.0 / 64.0 { Some(n as i32) } else if v - n > 63.0 / 64.0 { Some(n as i32 + 1) } else { None }
}
/// A premultiplied pixel onto a transparent one, back to straight alpha as GDI+'s image draw does it.
pub fn unpremul(p: [u8; 4]) -> u32 {
    let a = p[0] as u32;
    let r = (255u32 << 16) / a;
    let f = |c: u8| ((c as u32 * r) >> 16).min(255);
    a << 24 | f(p[1]) << 16 | f(p[2]) << 8 | f(p[3])
}

/// One output line's taps: for device pixels d0..d1, the first source texel and its 16.16 weights (inside the image).
pub fn taps(n_src: i32, d0: i32, d1: i32, o: f64, k: f64, mirrored: bool) -> Vec<(i32, Vec<i64>)> {
    let s = k.max(1.0);
    let oo = if mirrored { mirror(o) } else { o };
    // The source position steps along in 16.16 from the first pixel.
    let kf = (k * 65536.0).round_ties_even() as i64;
    let c0 = ((d0 as f64 - oo) * k * 65536.0).round_ties_even() as i64;
    let bin = |e: i64| -> i64 { if s == 1.0 { e.div_euclid(512) } else { (e as f64 / s / 512.0).floor() as i64 } };
    (0..(d1 - d0).max(0) as i64).map(|i| {
        let c = c0 + i * kf;
        let cr = c as f64 / 65536.0;
        let lo = ((cr - 2.5 * s).floor() as i64 - 1).max(0);
        let hi = ((cr + 2.5 * s).ceil() as i64 + 1).min(n_src as i64 - 1);
        if lo > hi { return (0, Vec::new()); }
        let w = (lo..=hi).map(|j| t_at(bin(j * 65536 + 32768 - c)) - t_at(bin(j * 65536 - 32768 - c))).collect();
        (lo as i32, w)
    }).collect()
}

/// Premultiplied A, R, G, B of a straight ARGB pixel (rounded as GDI+ does).
pub fn premul(v: u32) -> [i64; 4] {
    let a = v >> 24;
    let d = |c: u32| { let x = c * a; ((x + 128 + ((x + 128) >> 8)) >> 8) as i64 };
    [a as i64, d(v >> 16 & 255), d(v >> 8 & 255), d(v & 255)]
}

fn settle(acc: [i64; 4]) -> [i64; 4] {
    let mut o = acc.map(|v| ((v + 32768) >> 16).clamp(0, 255));
    for c in 1..4 { o[c] = o[c].min(o[0]); }
    o
}

/// The device box [x0, x1) x [y0, y1) of a draw at (dx, dy, dw, dh), and each pixel's premultiplied ARGB there:
/// columns first, every line rounded and kept within its alpha, then rows (probed).
pub fn stretch(src: &[u32], sw: i32, sh: i32, dx: f64, dy: f64, dw: f64, dh: f64, bounds: (i32, i32, i32, i32)) -> (i32, i32, i32, i32, Vec<[u8; 4]>) {
    let x0 = (dx.ceil() as i32).max(bounds.0);
    let y0 = (dy.ceil() as i32).max(bounds.1);
    let x1 = ((dx + dw).ceil() as i32).min(bounds.2);
    let y1 = ((dy + dh).ceil() as i32).min(bounds.3);
    if x0 >= x1 || y0 >= y1 || sw <= 0 || sh <= 0 { return (0, 0, 0, 0, Vec::new()); }
    let cols = taps(sw, x0, x1, dx, sw as f64 / dw, true);
    let rows = taps(sh, y0, y1, dy, sh as f64 / dh, false);
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    // The source lines the rows reach, each filtered across once.
    let (r0, r1) = rows.iter().filter(|r| !r.1.is_empty()).fold((sh, 0), |a, r| (a.0.min(r.0), a.1.max(r.0 + r.1.len() as i32)));
    let mut lines: Vec<Vec<[i64; 4]>> = vec![Vec::new(); sh as usize];
    for sy in r0..r1 {
        let row: Vec<[i64; 4]> = src[(sy * sw) as usize..((sy + 1) * sw) as usize].iter().map(|v| premul(*v)).collect();
        lines[sy as usize] = cols.iter().map(|(j0, cw)| {
            let mut acc = [0i64; 4];
            for (k, wt) in cw.iter().enumerate() { let p = row[*j0 as usize + k]; for c in 0..4 { acc[c] += p[c] * wt; } }
            settle(acc)
        }).collect();
    }
    let mut out = vec![[0u8; 4]; w * h];
    for (yi, (j0, rw)) in rows.iter().enumerate() {
        for xi in 0..w {
            let mut acc = [0i64; 4];
            for (k, wt) in rw.iter().enumerate() { let p = lines[*j0 as usize + k][xi]; for c in 0..4 { acc[c] += p[c] * wt; } }
            out[yi * w + xi] = settle(acc).map(|v| v as u8);
        }
    }
    (x0, y0, x1, y1, out)
}

/// A rotated or skewed draw: the picture is first stretched to the rect's size (as above), then each device pixel's
/// corner is mapped back into it and a 4x4 Keys (a = -0.5) taken there, transparent beyond the edges. A model of GDI+'s
/// affine path (probed, t53): mean error about 1, not bit-exact.
pub fn prescale(src: &[u32], sw: i32, sh: i32, pw: i32, ph: i32) -> Vec<[u8; 4]> {
    if pw == sw && ph == sh { return src.iter().map(|v| premul(*v).map(|c| c as u8)).collect(); }
    stretch(src, sw, sh, 0.0, 0.0, pw as f64, ph as f64, (0, 0, pw, ph)).4
}
pub fn sample_affine(pre: &[[u8; 4]], pw: i32, ph: i32, u: f64, v: f64) -> [u8; 4] {
    let (iu, iv) = (u.floor() as i64, v.floor() as i64);
    let mut acc = [0f64; 4];
    for jy in iv - 1..=iv + 2 {
        if jy < 0 || jy >= ph as i64 { continue; }
        let wy = keys(jy as f64 - v, -0.5);
        for jx in iu - 1..=iu + 2 {
            if jx < 0 || jx >= pw as i64 { continue; }
            let wt = wy * keys(jx as f64 - u, -0.5);
            let p = pre[(jy * pw as i64 + jx) as usize];
            for c in 0..4 { acc[c] += p[c] as f64 * wt; }
        }
    }
    let mut o = acc.map(|x| x.round().clamp(0.0, 255.0) as u8);
    for c in 1..4 { o[c] = o[c].min(o[0]); }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_ends_and_weights_sum() {
        assert_eq!(t_at(-257), -32768);
        assert_eq!(t_at(256), 32768);
        // Values measured from GDI+ (vertical probe, phase 1/4): the weights differ from round(integral) here.
        let t = taps(1000, 60, 61, 5.25, 1.0, false);
        assert_eq!(t[0].1.iter().copied().filter(|w| *w != 0).collect::<Vec<_>>(), vec![-4068, 21299, 51517, -2947, -265]);
        for k in [0.5, 1.0, 1.7, 3.0] {
            for (_, w) in taps(400, 10, 60, 3.3, k, true) { assert_eq!(w.iter().sum::<i64>(), 65536); }
        }
    }
}
