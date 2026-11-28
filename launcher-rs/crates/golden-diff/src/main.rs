//! golden-diff <golden.png> <ours.png> [name] [heat.png]: parity of a Rust snapshot against the C# golden.
//! Classes: text (union of both <png>.text.txt rects, grown 1 px, ignored), edge (Sobel edge in either image,
//! grown 1 px), interior (the rest). Pixels the Rust snapshot names in <png>.accept.txt (known GDI+ noise, e.g.
//! a gradient's exact wrap row) are counted apart and don't decide the verdict. Pass: interior within 2 per channel, edges within 64 with under 1% above 32,
//! edge-structure IoU at least 99.5%. Prints one line per screen plus the detail numbers.

use std::path::Path;

struct Img {
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
}

fn load(p: &Path) -> Img {
    let pm = tiny_skia::Pixmap::load_png(p).unwrap_or_else(|e| panic!("{}: {}", p.display(), e));
    let px = pm.pixels().iter().map(|c| {
        let d = c.demultiply();
        [d.red(), d.green(), d.blue(), d.alpha()]
    }).collect();
    Img { w: pm.width() as usize, h: pm.height() as usize, px }
}

#[derive(Clone)]
struct TextRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    text: String,
}

fn masks(p: &Path) -> Vec<TextRect> {
    let f = p.with_extension("text.txt");
    let Ok(s) = std::fs::read_to_string(&f) else { return Vec::new() };
    s.lines()
        .filter_map(|l| {
            let mut parts = l.split('\t');
            let r: Vec<i32> = parts.next()?.split(' ').filter_map(|v| v.parse().ok()).collect();
            let text = parts.nth(3).unwrap_or("").to_string();
            if r.len() == 4 { Some(TextRect { x: r[0], y: r[1], w: r[2], h: r[3], text }) } else { None }
        })
        .collect()
}

fn luma(c: [u8; 4]) -> f32 { 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32 }

fn sobel(img: &Img, thresh: f32) -> Vec<bool> { sobel_mag(img).iter().map(|m| *m > thresh).collect() }
fn sobel_mag(img: &Img) -> Vec<f32> {
    let (w, h) = (img.w, img.h);
    let mut out = vec![0f32; w * h];
    let l = |x: usize, y: usize| luma(img.px[y * w + x]);
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let gx = -l(x - 1, y - 1) - 2.0 * l(x - 1, y) - l(x - 1, y + 1) + l(x + 1, y - 1) + 2.0 * l(x + 1, y) + l(x + 1, y + 1);
            let gy = -l(x - 1, y - 1) - 2.0 * l(x, y - 1) - l(x + 1, y - 1) + l(x - 1, y + 1) + 2.0 * l(x, y + 1) + l(x + 1, y + 1);
            out[y * w + x] = (gx * gx + gy * gy).sqrt();
        }
    }
    out
}

fn grow(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut o = m.to_vec();
    for y in 0..h {
        for x in 0..w {
            if !m[y * w + x] { continue; }
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h { o[ny as usize * w + nx as usize] = true; }
                }
            }
        }
    }
    o
}

fn save_png(p: &Path, w: usize, h: usize, rgba: &[u8]) {
    let mut pm = tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
    for (d, s) in pm.pixels_mut().iter_mut().zip(rgba.chunks(4)) { *d = tiny_skia::ColorU8::from_rgba(s[0], s[1], s[2], s[3]).premultiply(); }
    pm.save_png(p).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("golden-diff <golden.png> <ours.png> [name] [heat.png]");
        std::process::exit(1);
    }
    let (gp, op) = (Path::new(&a[1]), Path::new(&a[2]));
    let name = a.get(3).cloned().unwrap_or_else(|| gp.file_stem().unwrap().to_string_lossy().into());
    let (g, o) = (load(gp), load(op));
    if g.w != o.w || g.h != o.h {
        println!("{}: FAIL size {}x{} vs {}x{}", name, g.w, g.h, o.w, o.h);
        std::process::exit(2);
    }
    let (w, h) = (g.w, g.h);
    let (tg, to) = (masks(gp), masks(op));
    let mut text = vec![false; w * h];
    for r in tg.iter().chain(to.iter()) {
        for y in r.y.max(0)..(r.y + r.h).min(h as i32) {
            for x in r.x.max(0)..(r.x + r.w).min(w as i32) { text[y as usize * w + x as usize] = true; }
        }
    }
    let text = grow(&text, w, h);
    // Accepted residuals, named by the Rust snapshot (<png>.accept.txt): known GDI+ noise, counted on their own.
    // How far each named residual may differ: a wrap row any amount, a gradient stack up to 3 (user, 2026-11-25).
    let mut acc = vec![0i32; w * h];
    let mut acc_names: Vec<String> = Vec::new();
    if let Ok(s) = std::fs::read_to_string(op.with_extension("accept.txt")) {
        for l in s.lines() {
            let mut p = l.split('\t');
            let name = p.next().unwrap_or("").to_string();
            let r: Vec<i32> = p.next().unwrap_or("").split(' ').filter_map(|v| v.parse().ok()).collect();
            if r.len() != 4 { continue; }
            // morph-stack: single pixels listed by hand in tools/replays/residuals.txt (user, 2026-11-25), also up to 3.
            let lim = if name == "gradient-stack" || name == "morph-stack" { 3 } else { i32::MAX };
            if !acc_names.contains(&name) { acc_names.push(name); }
            for y in r[1].max(0)..(r[1] + r[3]).min(h as i32) {
                for x in r[0].max(0)..(r[0] + r[2]).min(w as i32) { let k = y as usize * w + x as usize; acc[k] = acc[k].max(lim); }
            }
        }
    }
    let (mut a_px, mut a_n, mut a_max) = (0usize, 0usize, 0i32);
    let (eg, eo) = (sobel(&g, 48.0), sobel(&o, 48.0));
    // Threshold flicker: an edge in only one image whose strength is near the threshold in both (48 +- 8).
    let (mg, mo) = (sobel_mag(&g), sobel_mag(&o));
    let mut flicker = 0usize;
    let edge: Vec<bool> = grow(&eg.iter().zip(eo.iter()).map(|(a, b)| *a || *b).collect::<Vec<_>>(), w, h);
    let (mut i_n, mut i_over, mut i_max, mut i_px) = (0usize, 0usize, 0i32, 0usize);
    let (mut e_n, mut e_over32, mut e_over64, mut e_max, mut e_px) = (0usize, 0usize, 0usize, 0i32, 0usize);
    let (mut t_px, mut inter, mut uni) = (0usize, 0usize, 0usize);
    let mut heat = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let d = (0..4).map(|c| (g.px[i][c] as i32 - o.px[i][c] as i32).abs()).max().unwrap();
        let base = (luma(g.px[i]) * 0.3) as u8;
        let mut col = [base, base, base, 255];
        if acc[i] > 0 && !text[i] { a_px += 1; }
        // Only the accepted noise itself steps out of the classes: over 2 in a named area, within that name's limit.
        if acc[i] > 0 && !text[i] && d > 2 && d <= acc[i] {
            a_n += 1;
            a_max = a_max.max(d);
            col = [180, 80, 255, 255];
        } else if text[i] {
            t_px += 1;
            col = [base / 2, base / 2, (base as u16 + 60).min(255) as u8, 255];
        } else {
            if eg[i] || eo[i] {
                uni += 1;
                if eg[i] && eo[i] { inter += 1; } else if (mg[i] - 48.0).abs() <= 8.0 && (mo[i] - 48.0).abs() <= 8.0 { flicker += 1; }
            }
            if edge[i] {
                e_px += 1;
                if d > 0 { e_n += 1; }
                if d > 32 { e_over32 += 1; }
                if d > 64 { e_over64 += 1; }
                e_max = e_max.max(d);
                if d > 0 { col = if d > 32 { [255, 140, 0, 255] } else { [0, (90 + d * 5).min(255) as u8, 60, 255] }; }
            } else {
                i_px += 1;
                if d > 0 { i_n += 1; }
                if d > 2 { i_over += 1; if std::env::var("GOLDEN_DIFF_LIST").is_ok() { eprintln!("interior over 2: {} {} by {}", i % w, i / w, d); } }
                i_max = i_max.max(d);
                if d > 0 { col = if d > 2 { [255, 30, 30, 255] } else { [200, 200, 40, 255] }; }
            }
        }
        heat[i * 4..i * 4 + 4].copy_from_slice(&col);
    }
    let iou = if uni == 0 { 1.0 } else { inter as f64 / uni as f64 };
    // Text rects: each of ours against the golden's with the same text, within 2 px.
    let mut matched = 0;
    for r in &to {
        if tg.iter().any(|q| q.text == r.text && (q.x - r.x).abs() <= 2 && (q.y - r.y).abs() <= 2 && (q.w - r.w).abs() <= 2 && (q.h - r.h).abs() <= 2) { matched += 1; }
    }
    let e_frac = if e_px == 0 { 0.0 } else { e_over32 as f64 / e_px as f64 };
    // The verdict uses the IoU without threshold flicker (user, 2026-11-25); the raw IoU is printed for information.
    let iou_ff = if uni == 0 { 1.0 } else { (inter + flicker) as f64 / uni as f64 };
    let pass = i_over == 0 && e_over64 == 0 && e_frac < 0.01 && iou_ff >= 0.995;
    println!(
        "{}: {} | interior {} px, {} differ, {} over 2, max {} | edge {} px, {} differ, {} over 32 ({:.3}%), max {} | IoU {:.4}% ({:.4}% without threshold flicker) | text {} px masked, rects {}/{} ours match ({} golden) | accepted {}: {} px named, {} over 2, max {}",
        name,
        if pass { "PASS" } else { "FAIL" },
        i_px,
        i_n,
        i_over,
        i_max,
        e_px,
        e_n,
        e_over32,
        e_frac * 100.0,
        e_max,
        iou * 100.0,
        if uni == 0 { 100.0 } else { (inter + flicker) as f64 / uni as f64 * 100.0 },
        t_px,
        matched,
        to.len(),
        tg.len(),
        if acc_names.is_empty() { "-".to_string() } else { acc_names.join(",") },
        a_px,
        a_n,
        a_max
    );
    if let Some(hp) = a.get(4) { save_png(Path::new(hp), w, h, &heat); }
    std::process::exit(if pass { 0 } else { 2 });
}
