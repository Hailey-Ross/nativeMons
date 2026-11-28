//! The versions' wallpapers: LayerFor (a cached scene per size and theme), VersionFill (the scene plus its live
//! layer) and every finish's scene, ported line by line from the C# launcher.

use crate::art::*;
use crate::model::Theme;
use nm_draw::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

pub struct FillLayer {
    pub picture: std::sync::Arc<Bitmap>,
    pub sparks: Vec<PointF>,
    /// Named residuals (gradient-stack, gradient-wrap-row) found while drawing it.
    pub accepted: Vec<(&'static str, i32, i32, i32, i32)>,
}

thread_local! {
    static FILL_CACHE: RefCell<HashMap<String, Rc<FillLayer>>> = RefCell::new(HashMap::new());
}

fn p(x: f32, y: f32) -> PointF { PointF::new(x, y) }
fn rf(x: f32, y: f32, w: f32, h: f32) -> RectangleF { RectangleF::new(x, y, w, h) }
fn sin(x: f64) -> f32 { x.sin() as f32 }
fn cos(x: f64) -> f32 { x.cos() as f32 }
fn nd(r: &mut NetRandom) -> f32 { r.next_double() as f32 }

pub fn draw_bezier(g: &mut Graphics, pen: &Pen, a: PointF, b: PointF, c: PointF, d: PointF) { g.draw_bezier(pen, a, b, c, d); }
fn bez4(g: &mut Graphics, pen: &Pen, v: [f32; 8]) { draw_bezier(g, pen, p(v[0], v[1]), p(v[2], v[3]), p(v[4], v[5]), p(v[6], v[7])); }

fn ellipse_grad(r: RectangleF, center: Color, surround: Color) -> Brush { Brush::Ellipse(EllipseGradient { rect: r, center, surround }) }

/// LayerFor.
pub fn layer_for(w: i32, h: i32, th: &Theme) -> Rc<FillLayer> {
    let key = format!("{}{}{}x{}", th.main.to_argb(), th.finish, w, h);
    if let Some(f) = FILL_CACHE.with(|c| c.borrow().get(&key).cloned()) { return f; }
    let mut g = Graphics::new(Bitmap::new(w.max(1), h.max(1)));
    g.smoothing_mode = SmoothingMode::AntiAlias;
    let r = rf(0.0, 0.0, w as f32, h as f32);
    let scene = h >= 40;
    let mut sparks = Vec::new();
    match if scene { th.finish.as_str() } else { "plain" } {
        "crystal" => crystal_scene(&mut g, r, th, &mut sparks),
        "gold" => gold_scene(&mut g, r, th),
        "silver" => silver_scene(&mut g, r, th),
        "flame" => { enamel(&mut g, r, th); flame_scene(&mut g, r, th); }
        "tide" => { enamel(&mut g, r, th); tide_scene(&mut g, r, th); }
        "bolt" => { enamel(&mut g, r, th); bolt_scene(&mut g, r, th); }
        "kalos" => { enamel(&mut g, r, th); kalos_scene(&mut g, r, th); }
        "hoenn" => { enamel(&mut g, r, th); hoenn_scene(&mut g, r, th); }
        "hoenn-sea" => { enamel(&mut g, r, th); storm_scene(&mut g, r, th); }
        "alola" | "alola-moon" | "alola-ultrasun" | "alola-ultramoon" => { enamel(&mut g, r, th); alola_scene(&mut g, r, th); }
        _ => enamel(&mut g, r, th),
    }
    let accepted = std::mem::take(&mut g.accepted);
    let f = Rc::new(FillLayer { picture: std::sync::Arc::new(g.bmp), sparks, accepted });
    FILL_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 48 { c.clear(); }
        c.insert(key, f.clone());
    });
    f
}

pub fn version_fill(g: &mut Graphics, r: RectangleF, rad: f32, th: &Theme, t: f32) {
    let layer = layer_for(r.width.ceil() as i32, r.height.ceil() as i32, th);
    let st = g.save();
    g.set_clip_path(&round(r, rad));
    g.draw_image_unscaled_shared(&layer.picture, r.x as i32, r.y as i32);
    // The scene's named residuals come along with its picture (a plain offset; the C# draws it unscaled too).
    let at = g.transform_point(p(r.x as i32 as f32, r.y as i32 as f32));
    for (n, x, y, w, h) in &layer.accepted { g.accepted.push((n, x + at.x.round() as i32, y + at.y.round() as i32, *w, *h)); }
    if r.height >= 40.0 {
        let f = th.finish.as_str();
        if f == "crystal" { crystal_live(g, r, &layer, t); }
        else if f == "gold" { sheen(g, r, t, Color::rgb(255, 250, 215)); motes(g, r, t, Color::rgb(255, 180, 60), -1); }
        else if f == "silver" { sheen(g, r, t, Color::WHITE); moon_glints(g, r, t); }
        else if f == "bolt" { crackle(g, r, t); }
        else if f == "flame" { motes(g, r, t, Color::rgb(255, 210, 90), -1); }
        else if f == "hoenn" { motes(g, r, t, mix(th.light, Color::WHITE, 0.4), -1); }
        else if f == "hoenn-sea" { storm_live(g, r, t); }
        else if f.starts_with("alola") { sheen(g, r, t, Color::WHITE); sun_glints(g, r, t, th); }
        else if f == "kalos" { sheen(g, r, t, Color::WHITE); }
    } else if !th.plain() { sheen(g, r, t, Color::WHITE); }
    g.restore(st);
}

pub fn gradient(g: &mut Graphics, r: RectangleF, colors: &[Color], stops: &[f32]) {
    let mut lg = LinearGradient::new(rf(r.x, r.y - 1.0, r.width, r.height + 2.0), colors[0], colors[colors.len() - 1], 90.0);
    lg.colors = colors.iter().cloned().zip(stops.iter().cloned()).collect();
    g.fill_rectangle(&Brush::Linear(lg), r);
}
pub fn enamel(g: &mut Graphics, r: RectangleF, th: &Theme) {
    gradient(g, r, &[mix(th.main, th.light, 0.35), th.main, th.deep], &[0.0, 0.55, 1.0]);
    g.fill_ellipse(&solid(fade(Color::WHITE, 22)), rf(r.x - r.width * 0.15, r.y - r.height * 1.1, r.width * 0.9, r.height * 1.7));
}
fn metal_base(g: &mut Graphics, r: RectangleF, th: &Theme) {
    gradient(g, r, &[th.light, th.main, th.deep, th.main, th.light, th.main, th.deep], &[0.0, 0.16, 0.42, 0.56, 0.66, 0.82, 1.0]);
    let pen = Pen::new(fade(Color::WHITE, 16), 1.0);
    let mut y = r.y;
    while y < r.bottom() { g.draw_line_f(&pen, r.x, y, r.right(), y); y += 3.0; }
    let pen = Pen::new(fade(th.deep, 22), 1.0);
    let mut y = r.y + 1.5;
    while y < r.bottom() { g.draw_line_f(&pen, r.x, y, r.right(), y); y += 5.0; }
}
pub fn sheen(g: &mut Graphics, r: RectangleF, t: f32, c: Color) {
    let span = r.width + r.height * 2.0;
    let w = (r.height * 0.4).max(r.width * 0.08);
    let x = r.x - r.height + ((t * r.width * 0.22) % (span * 1.6));
    for layer in [[1.0f32, 46.0], [0.4, 120.0]] {
        let wd = w * layer[0];
        let c0 = x + w / 2.0;
        let band = [p(c0 - wd / 2.0 + r.height, r.y), p(c0 + wd / 2.0 + r.height, r.y), p(c0 + wd / 2.0, r.bottom()), p(c0 - wd / 2.0, r.bottom())];
        g.fill_polygon(&solid(fade(c, layer[1] as i32)), &band);
    }
}
fn star(g: &mut Graphics, x: f32, y: f32, s: f32, c: Color) {
    g.fill_polygon(&solid(c), &[p(x, y - s), p(x + s * 0.16, y - s * 0.16), p(x + s, y), p(x + s * 0.16, y + s * 0.16), p(x, y + s), p(x - s * 0.16, y + s * 0.16), p(x - s, y), p(x - s * 0.16, y - s * 0.16)]);
}
pub fn motes(g: &mut Graphics, r: RectangleF, t: f32, c: Color, dir: i32) {
    let u = r.height.min(r.width * 0.55);
    for i in 0..14 {
        let seed = (i as f32 * 0.618) % 1.0;
        let speed = 0.05 + seed * 0.07;
        let life = (t * speed + seed * 3.0) % 1.0;
        let y = if dir < 0 { r.bottom() - life * r.height } else { r.y + life * r.height };
        let x = r.x + ((i as f32 * 0.381 + 0.5 * sin((t * 0.3 + i as f32) as f64) * 0.04) % 1.0) * r.width;
        let size = u * (0.006 + 0.01 * seed);
        let a = sin(life as f64 * PI);
        g.fill_ellipse(&solid(fade(c, (220.0 * a) as i32)), rf(x - size, y - size, size * 2.0, size * 2.0));
    }
}

fn crystal_scene(g: &mut Graphics, r: RectangleF, th: &Theme, sparks: &mut Vec<PointF>) {
    gradient(g, r, &[th.deep, mix(th.deep, th.main, 0.6), th.main, th.light], &[0.0, 0.42, 0.78, 1.0]);
    let u = r.height.min(r.width * 0.55);
    aurora(g, r, u, 0.10, 0.20, 1.0, 0.3, Color::rgb(96, 240, 208));
    aurora(g, r, u, 0.24, 0.15, 1.4, 2.0, Color::rgb(150, 130, 255));
    aurora(g, r, u, 0.34, 0.10, 0.8, 4.1, Color::rgb(120, 225, 255));
    let mut rnd = NetRandom::new(1999);
    let n = (r.width * r.height / (u * u) * 40.0) as i32;
    for _ in 0..n {
        let x = r.x + nd(&mut rnd) * r.width;
        let y = r.y + nd(&mut rnd) * r.height * 0.55;
        let s = u * (0.003 + 0.004 * nd(&mut rnd));
        let a = 90 + rnd.next(150);
        g.fill_ellipse(&solid(fade(Color::WHITE, a)), rf(x, y, s, s));
    }
    let span = r.width;
    for row in 0..2 {
        let n = 5.max((span / (u * if row == 0 { 0.13 } else { 0.16 })) as i32);
        for i in 0..n {
            let k = (i as f32 + nd(&mut rnd) * 0.6) / n as f32;
            let height = u * if row == 0 { 0.92 } else { 0.68 } * (k as f64).powf(1.5) as f32 * (0.65 + 0.35 * nd(&mut rnd)) + u * 0.1;
            let width = u * (0.1 + 0.1 * nd(&mut rnd)) * (0.6 + 0.6 * k);
            let lean = (k - 0.55) * width * 1.3 + width * 0.5 * (nd(&mut rnd) - 0.5);
            sparks.push(shard(g, r.x + k * span - width / 2.0, r.bottom() + 2.0, width, height, lean, th, if row == 0 { 0.55 } else { 1.0 }, &mut rnd));
        }
    }
}
fn aurora(g: &mut Graphics, r: RectangleF, u: f32, at: f32, depth: f32, waves: f32, phase: f32, c: Color) {
    let steps = 48;
    let mut top = vec![PointF::default(); steps + 1];
    let mut low = vec![PointF::default(); steps + 1];
    for i in 0..=steps {
        let k = i as f32 / steps as f32;
        let x = r.x + k * r.width;
        let y = r.y + u * at + sin(k as f64 * PI * 2.0 * waves as f64 + phase as f64) * u * 0.05 + sin((k * 9.0 + phase) as f64) * u * 0.015;
        top[i] = p(x, y);
        low[i] = p(x, y + u * depth * (0.7 + 0.3 * sin((k * 7.0 + phase * 2.0) as f64)));
    }
    for i in 0..steps {
        let (a, b) = (top[i], low[i]);
        let quad = [top[i], top[i + 1], low[i + 1], low[i]];
        if b.y <= a.y + 1.0 { continue; }
        g.fill_polygon(&lin(rf(a.x, a.y - 1.0, 4.0, b.y - a.y + 2.0), fade(c, 110), fade(c, 0), 90.0), &quad);
    }
}
fn shard(g: &mut Graphics, x: f32, bottom: f32, w: f32, h: f32, lean: f32, th: &Theme, strength: f32, rnd: &mut NetRandom) -> PointF {
    let shoulder = bottom - h * 0.8;
    let tip_y = bottom - h;
    let (x1, x2, x3, x4) = (x, x + w * 0.3, x + w * 0.72, x + w);
    let tip_x = x + w * 0.5 + lean;
    let s_l = p(x1 + lean * 0.75, shoulder + h * 0.03);
    let s_m1 = p(x2 + lean * 0.75, shoulder - h * 0.02);
    let s_m2 = p(x3 + lean * 0.75, shoulder - h * 0.02);
    let s_r = p(x4 + lean * 0.75, shoulder + h * 0.03);
    let tip = p(tip_x, tip_y);
    let a = (235.0 * strength) as i32;
    let pale = mix(th.light, Color::WHITE, 0.5);
    let mid = mix(th.main, th.light, 0.45);
    let dark = mix(th.deep, th.main, 0.45);
    face(g, &[p(x1, bottom), s_l, s_m1, p(x2, bottom)], fade(pale, a), fade(mid, a));
    face(g, &[p(x2, bottom), s_m1, s_m2, p(x3, bottom)], fade(mid, a), fade(th.main, a));
    face(g, &[p(x3, bottom), s_m2, s_r, p(x4, bottom)], fade(th.main, a), fade(dark, a));
    face(g, &[s_l, tip, s_m1], fade(Color::WHITE, a), fade(pale, a));
    face(g, &[s_m1, tip, s_m2], fade(pale, a), fade(mid, a));
    face(g, &[s_m2, tip, s_r], fade(mid, a), fade(dark, a));
    let prism = [Color::rgb(255, 170, 220), Color::rgb(170, 255, 230), Color::rgb(200, 180, 255)];
    let pen = Pen::new(fade(prism[rnd.next(3) as usize], (120.0 * strength) as i32), (w * 0.05).max(1.0));
    g.draw_line_f(&pen, x2 + w * 0.1, bottom - h * 0.12, x3 + lean * 0.5 - w * 0.1, shoulder + h * 0.14);
    let pen = Pen::new(fade(Color::WHITE, (190.0 * strength) as i32), 1.0);
    g.draw_lines(&pen, &[s_l, tip, s_r]);
    g.draw_line(&pen, s_m1, tip);
    g.draw_line(&pen, s_m2, tip);
    g.draw_lines(&pen, &[s_l, s_m1, s_m2, s_r]);
    g.draw_line(&pen, s_m1, p(x2, bottom));
    g.draw_line(&pen, s_m2, p(x3, bottom));
    tip
}
fn face(g: &mut Graphics, pts: &[PointF], a: Color, b: Color) {
    let min_x = pts.iter().map(|q| q.x).fold(f32::MAX, f32::min);
    let max_x = pts.iter().map(|q| q.x).fold(f32::MIN, f32::max);
    let min_y = pts.iter().map(|q| q.y).fold(f32::MAX, f32::min);
    let max_y = pts.iter().map(|q| q.y).fold(f32::MIN, f32::max);
    g.fill_polygon(&lin(rf(min_x - 1.0, min_y - 1.0, max_x - min_x + 2.0, max_y - min_y + 2.0), a, b, 65.0), pts);
}
fn crystal_live(g: &mut Graphics, r: RectangleF, f: &FillLayer, t: f32) {
    let u = r.height.min(r.width * 0.55);
    for i in 0..16 {
        let seed = (i as f32 * 0.618) % 1.0;
        let life = (t * (0.03 + seed * 0.04) + seed * 5.0) % 1.0;
        let x = r.x + ((i as f32 * 0.381 + life * 0.08) % 1.0) * r.width;
        let y = r.y + life * r.height;
        let beat = 0.5 + 0.5 * sin((t * 3.0 + i as f32 * 2.1) as f64);
        star(g, x, y, u * (0.008 + 0.012 * seed) * (0.6 + 0.4 * beat), fade(Color::WHITE, (90.0 + 150.0 * beat) as i32));
    }
    if !f.sparks.is_empty() {
        for i in 0..3 {
            let phase = t * 0.7 + i as f32 * 0.33;
            let idx = ((phase as f64).floor() * 7.0 + (i * 13) as f64) as i32 % f.sparks.len() as i32;
            let beat = sin((phase as f64 - (phase as f64).floor()) * PI);
            let q = f.sparks[idx.unsigned_abs() as usize];
            star(g, r.x + q.x, r.y + q.y, u * 0.05 * beat, fade(Color::WHITE, (255.0 * beat) as i32));
        }
    }
}

fn gold_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    metal_base(g, r, th);
    let u = r.height.min(r.width * 0.55);
    let wide = r.width > r.height;
    let sun = p(r.x + r.width * 0.76, if wide { r.y + r.height * 0.56 } else { r.bottom() - u * 0.14 });
    let reach = r.width.max(r.height) * 1.3;
    for i in 0..18 {
        let a0 = PI + i as f64 * PI / 18.0;
        let a1 = a0 + PI / 36.0;
        g.fill_polygon(&solid(fade(Color::rgb(255, 248, 200), 52)), &[sun, p(sun.x + cos(a0) * reach, sun.y + sin(a0) * reach), p(sun.x + cos(a1) * reach, sun.y + sin(a1) * reach)]);
    }
    let rainbow = [Color::rgb(232, 56, 48), Color::rgb(244, 128, 32), Color::rgb(250, 208, 48), Color::rgb(72, 184, 88), Color::rgb(48, 136, 216), Color::rgb(120, 84, 200)];
    let feathers = 13;
    for i in 0..feathers {
        let k = i as f32 / (feathers - 1) as f32;
        let angle = PI * (1.08 + 0.84 * k as f64);
        let len = u * (0.62 + 0.28 * sin(k as f64 * PI));
        let ki = (k * 4.999) as usize;
        let c = mix(rainbow[ki], rainbow[ki + 1], (k * 4.999) % 1.0);
        feather(g, sun, angle, len, u * 0.082, c);
    }
    let d = u * 0.21;
    let disc = rf(sun.x - d * 1.7, sun.y - d * 1.7, d * 3.4, d * 3.4);
    g.fill_ellipse(&ellipse_grad(disc, fade(Color::rgb(255, 252, 220), 245), fade(Color::rgb(255, 220, 120), 0)), disc);
    tower(g, sun.x, if wide { sun.y + u * 0.14 } else { r.bottom() + 2.0 }, u * 0.62, u * 0.25, mix(th.deep, Color::BLACK, 0.35), fade(th.light, 220));
}
fn feather(g: &mut Graphics, from: PointF, angle: f64, len: f32, wid: f32, c: Color) {
    let st = g.save();
    g.translate_transform(from.x, from.y);
    g.rotate_transform((angle * 180.0 / PI) as f32);
    let mut path = GraphicsPath::new();
    path.add_bezier(p(len * 0.12, 0.0), p(len * 0.45, -wid), p(len * 0.85, -wid * 0.75), p(len, 0.0));
    path.add_bezier(p(len, 0.0), p(len * 0.85, wid * 0.75), p(len * 0.45, wid), p(len * 0.12, 0.0));
    g.fill_path(&lin(rf(0.0, -wid, len + 1.0, wid * 2.0), fade(mix(c, Color::WHITE, 0.25), 205), fade(c, 245), 0.0), &path);
    g.draw_path(&Pen::new(fade(Color::rgb(70, 40, 10), 190), 1.0), &path);
    g.draw_line_f(&Pen::new(fade(Color::WHITE, 170), 1.0), len * 0.15, 0.0, len * 0.96, 0.0);
    g.restore(st);
}
fn tower(g: &mut Graphics, cx: f32, bottom: f32, height: f32, width: f32, fill: Color, edge: Color) {
    let tiers = 5;
    let mut y = bottom;
    let tier_h = height / (tiers as f32 + 0.7);
    let b = solid(fill);
    let pen = Pen::new(edge, 1.0);
    for i in 0..tiers {
        let k = 1.0 - i as f32 / (tiers + 1) as f32;
        let w = width * k;
        let wall_w = w * 0.62;
        let wall = rf(cx - wall_w / 2.0, y - tier_h * 0.62, wall_w, tier_h * 0.62);
        g.fill_rectangle(&b, wall);
        let lit = solid(fade(edge, 170));
        for s in 1..4 { g.fill_rectangle(&lit, rf(cx - wall_w / 2.0 + wall_w * s as f32 / 4.0 - wall_w * 0.035, wall.y + wall.height * 0.25, wall_w * 0.07, wall.height * 0.55)); }
        let ry = y - tier_h * 0.62;
        let roof = [p(cx - w * 0.68, ry - tier_h * 0.1), p(cx - w * 0.5, ry + tier_h * 0.02), p(cx + w * 0.5, ry + tier_h * 0.02), p(cx + w * 0.68, ry - tier_h * 0.1), p(cx + w * 0.34, ry - tier_h * 0.38), p(cx - w * 0.34, ry - tier_h * 0.38)];
        g.fill_polygon(&b, &roof);
        g.draw_polygon(&pen, &roof);
        y -= tier_h;
    }
    g.fill_rectangle(&b, rf(cx - width * 0.012, y - tier_h * 0.5, width * 0.024, tier_h * 0.9));
}

fn silver_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    metal_base(g, r, th);
    let u = r.height.min(r.width * 0.55);
    gradient(g, rf(r.x, r.y, r.width, r.height), &[fade(Color::rgb(20, 40, 84), 128), fade(Color::rgb(40, 70, 120), 70), fade(Color::rgb(30, 60, 110), 96)], &[0.0, 0.55, 1.0]);
    let moon = p(r.x + r.width * 0.8, r.y + u * 0.26);
    let mr = u * 0.17;
    let halo = rf(moon.x - mr * 2.4, moon.y - mr * 2.4, mr * 4.8, mr * 4.8);
    g.fill_ellipse(&ellipse_grad(halo, fade(Color::WHITE, 170), fade(Color::WHITE, 0)), halo);
    g.fill_ellipse(&solid(Color::rgb(246, 249, 255)), rf(moon.x - mr, moon.y - mr, mr * 2.0, mr * 2.0));
    let b = solid(fade(th.main, 130));
    g.fill_ellipse(&b, rf(moon.x - mr * 0.5, moon.y - mr * 0.45, mr * 0.42, mr * 0.34));
    g.fill_ellipse(&b, rf(moon.x + mr * 0.15, moon.y + mr * 0.15, mr * 0.5, mr * 0.4));
    g.fill_ellipse(&b, rf(moon.x + mr * 0.22, moon.y - mr * 0.55, mr * 0.24, mr * 0.22));
    let sea = r.bottom() - u * 0.4;
    let sea_rect = rf(r.x, sea, r.width, r.bottom() - sea);
    let path = wave_path(r, sea, u * 0.02, 0.3);
    g.fill_path(&lin(rf(r.x, sea - u * 0.03, r.width, sea_rect.height + u * 0.05), fade(Color::rgb(120, 150, 186), 230), fade(Color::rgb(28, 46, 84), 250), 90.0), &path);
    let clip = g.save();
    g.set_clip_rect(sea_rect);
    let eye = p(r.x + r.width * 0.68, sea + sea_rect.height * 0.55);
    for arm in 0..3 {
        let mut pts = Vec::new();
        let mut a = 0.0f64;
        while a < PI * 5.0 {
            let rad = (u as f64 * 0.018f32 as f64 * (0.2 * a).exp()) as f32;
            let ang = a + arm as f64 * PI * 2.0 / 3.0;
            pts.push(p(eye.x + cos(ang) * rad * 1.9, eye.y + sin(ang) * rad * 0.62));
            a += 0.14;
        }
        g.draw_lines(&Pen::new(fade(Color::rgb(226, 238, 255), 175), (u * 0.008).max(1.0)), &pts);
    }
    g.fill_ellipse(&solid(fade(Color::rgb(10, 20, 44), 200)), rf(eye.x - u * 0.05, eye.y - u * 0.018, u * 0.1, u * 0.036));
    g.restore(clip);
    for i in 0..5 {
        let k = i as f32 / 4.0;
        fin(g, r.x + r.width * (0.1 + k * 0.34), sea + u * 0.035, u * (0.3 - 0.14 * (k - 0.35).abs() * 2.0), u * 0.12, Color::rgb(224, 232, 244), Color::rgb(46, 62, 104));
    }
    g.fill_path(&solid(fade(Color::rgb(52, 78, 124), 235)), &wave_path(r, sea + u * 0.03, u * 0.014, 2.2));
    let pen = Pen::new(fade(Color::WHITE, 170), 1.0);
    for i in 0..2 { g.draw_path(&pen, &wave_line(r, sea + u * (0.03 + i as f32 * 0.09), u * 0.012, 2.2 + i as f32)); }
}
fn wave_line(r: RectangleF, y: f32, amp: f32, phase: f32) -> GraphicsPath {
    let mut pts = Vec::new();
    let step = 4f32.max(r.width / 90.0);
    let mut x = r.x - step;
    while x <= r.right() + step {
        pts.push(p(x, y + sin((x / amp * 0.12 + phase) as f64) * amp));
        x += step;
    }
    let mut path = GraphicsPath::new();
    path.add_lines(&pts);
    path
}
fn wave_path(r: RectangleF, y: f32, amp: f32, phase: f32) -> GraphicsPath {
    let mut path = wave_line(r, y, amp, phase);
    path.add_line(p(r.right() + 6.0, y), p(r.right() + 6.0, r.bottom() + 4.0));
    path.add_line(p(r.right() + 6.0, r.bottom() + 4.0), p(r.x - 6.0, r.bottom() + 4.0));
    path.close_figure();
    path
}
fn fin(g: &mut Graphics, x: f32, y: f32, h: f32, w: f32, top: Color, edge: Color) {
    let mut path = GraphicsPath::new();
    path.add_bezier(p(x, y), p(x + w * 0.15, y - h * 0.6), p(x + w * 0.5, y - h * 0.9), p(x + w * 1.15, y - h));
    path.add_bezier(p(x + w * 1.15, y - h), p(x + w * 0.85, y - h * 0.55), p(x + w * 0.95, y - h * 0.2), p(x + w, y));
    path.close_figure();
    g.fill_path(&lin(rf(x - 1.0, y - h - 1.0, w * 1.2 + 2.0, h + 2.0), top, mix(top, edge, 0.6), 80.0), &path);
    g.draw_path(&Pen::new(edge, (w * 0.03).max(1.0)), &path);
}
fn moon_glints(g: &mut Graphics, r: RectangleF, t: f32) {
    let u = r.height.min(r.width * 0.55);
    let sea = r.bottom() - u * 0.4;
    for i in 0..12 {
        let seed = (i as f32 * 0.618) % 1.0;
        let beat = 0.5 + 0.5 * sin((t * 1.6 + i as f32 * 1.9) as f64);
        let y = sea + u * 0.05 + seed * (u * 0.33);
        let x = r.x + r.width * 0.8 + (seed - 0.5) * u * 0.34 + sin((t * 0.6 + i as f32) as f64) * u * 0.02;
        g.draw_line_f(&Pen::new(fade(Color::WHITE, (200.0 * beat) as i32), (u * 0.006).max(1.0)), x - u * 0.03, y, x + u * 0.03, y);
    }
}

fn flame_scene(g: &mut Graphics, r: RectangleF, _th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let layers = [fade(Color::rgb(120, 16, 8), 170), fade(Color::rgb(244, 110, 24), 215), fade(Color::rgb(255, 204, 64), 225)];
    let tall = [0.62f32, 0.42, 0.24];
    let mut rnd = NetRandom::new(1996);
    for l in 0..3 {
        let n = 6.max((r.width / (u * (0.2 - l as f32 * 0.03))) as i32);
        let cw = r.width / n as f32;
        for i in -1..=n {
            let k = (i as f32 + 0.5) / n as f32;
            let h = u * tall[l] * (0.35 + 0.65 * k * k) * (0.7 + 0.3 * nd(&mut rnd));
            let x = r.x + i as f32 * cw;
            let b = r.bottom() + 2.0;
            let mut path = GraphicsPath::new();
            path.add_bezier(p(x - cw * 0.3, b), p(x, b - h * 0.5), p(x + cw * 0.05, b - h * 0.7), p(x + cw * 0.75, b - h));
            path.add_bezier(p(x + cw * 0.75, b - h), p(x + cw * 0.7, b - h * 0.55), p(x + cw * 1.2, b - h * 0.35), p(x + cw * 1.3, b));
            path.close_figure();
            g.fill_path(&solid(layers[l]), &path);
        }
    }
}
fn tide_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let s = u * 0.15;
    let rows = (u * 0.6 / (s * 0.55)) as i32;
    for row in 0..rows {
        let y = r.bottom() - u * 0.6 + row as f32 * s * 0.55;
        let a = 0.25 + 0.6 * row as f32 / 1.max(rows - 1) as f32;
        let mut x = r.x - s + (row % 2) as f32 * s / 2.0;
        while x < r.right() + s {
            g.fill_ellipse(&solid(fade(mix(th.main, th.deep, a), 235)), rf(x, y, s, s * 1.3));
            g.draw_arc(&Pen::new(fade(th.light, 170), (u * 0.005).max(1.0)), rf(x, y, s, s * 1.3), 195.0, 150.0);
            g.draw_arc(&Pen::new(fade(Color::WHITE, 56), (u * 0.004).max(1.0)), rf(x + s * 0.2, y + s * 0.2, s * 0.6, s * 0.8), 200.0, 140.0);
            x += s;
        }
    }
}
fn kalos_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let mut rnd = NetRandom::new(2013);
    let ink = mix(th.deep, Color::BLACK, 0.35);
    let glow = mix(th.light, Color::WHITE, 0.45);
    soft_glow(g, p(r.x + r.width * 0.5, r.bottom() - u * 0.15), r.width * 0.8, fade(glow, 120));
    for i in 0..3 { cloud(g, r.x + r.width * (0.06 + i as f32 * 0.34), r.y + r.height * (0.1 + 0.12 * (i % 2) as f32), u * (0.36 - i as f32 * 0.05), fade(Color::WHITE, 54)); }
    let t_h = r.height * 0.64;
    let tx = r.right() - r.width * 0.27;
    let bw = (t_h * 0.5).min(r.width * 0.42);
    let mut x = r.x - u * 0.05;
    while x < r.right() {
        let w = u * (0.08 + 0.1 * nd(&mut rnd));
        let h = r.height * (0.32 + 0.14 * nd(&mut rnd));
        g.fill_rectangle(&solid(fade(mix(th.deep, th.main, 0.45), 215)), rf(x, r.bottom() - h, w + 1.0, h + 1.0));
        x += w;
    }
    lattice_tower(g, tx, r.bottom(), t_h, bw, ink, fade(glow, 150));
    let lit = [Color::rgb(255, 214, 120), Color::rgb(255, 172, 196), Color::rgb(255, 238, 176)];
    let mut n = 0;
    let mut x = r.x - u * 0.03;
    while x < r.right() {
        let w = u * (0.11 + 0.08 * nd(&mut rnd));
        let h = r.height * (0.18 + 0.12 * nd(&mut rnd));
        if x + w > tx - bw * 0.62 && x < tx + bw * 0.62 { x = tx + bw * 0.62; continue; }
        let top = r.bottom() - h;
        let roof = u * 0.035;
        let b = solid(ink);
        g.fill_polygon(&b, &[p(x, r.bottom() + 1.0), p(x, top), p(x + roof * 0.6, top - roof), p(x + w - roof * 0.6, top - roof), p(x + w, top), p(x + w, r.bottom() + 1.0)]);
        n += 1;
        if n == 2 {
            let dw = w * 0.62;
            let dx = x + (w - dw) / 2.0;
            let dy = top - roof;
            g.fill_pie(&b, dx, dy - dw * 0.7, dw, dw * 1.4, 180.0, 180.0);
            g.fill_rectangle(&b, rf(x + w / 2.0 - dw * 0.07, dy - dw * 0.92, dw * 0.14, dw * 0.25));
        } else {
            g.fill_rectangle(&b, rf(x + w * 0.7, top - roof - u * 0.03, u * 0.018, u * 0.04));
        }
        let ws = (u * 0.014).max(2.0);
        let mut wy = top + u * 0.025;
        while wy < r.bottom() - u * 0.02 {
            let mut wx = x + u * 0.018;
            while wx < x + w - u * 0.018 {
                let on = rnd.next_double() < 0.45;
                let c = if on { fade(lit[rnd.next(3) as usize], 215) } else { fade(glow, 26) };
                g.fill_rectangle(&solid(c), rf(wx, wy, ws, ws * 1.4));
                wx += u * 0.03;
            }
            wy += u * 0.036;
        }
        x += w + u * 0.004;
    }
}
fn lattice_tower(g: &mut Graphics, cx: f32, bottom: f32, h: f32, bw: f32, fill: Color, line: Color) {
    let y1 = bottom - h * 0.3;
    let y2 = bottom - h * 0.56;
    let y3 = bottom - h * 0.93;
    let top = bottom - h;
    let left = [
        p(cx - bw * 0.5, bottom), p(cx - bw * 0.4, bottom - h * 0.12), p(cx - bw * 0.32, bottom - h * 0.22), p(cx - bw * 0.27, y1),
        p(cx - bw * 0.17, bottom - h * 0.42), p(cx - bw * 0.12, y2 + h * 0.04), p(cx - bw * 0.1, y2),
        p(cx - bw * 0.05, bottom - h * 0.72), p(cx - bw * 0.025, y3 + h * 0.06), p(cx - bw * 0.02, y3),
    ];
    let right: Vec<PointF> = left.iter().rev().map(|q| p(2.0 * cx - q.x, q.y)).collect();
    let mut path = GraphicsPath::new();
    add_beziers(&mut path, &left);
    path.add_line(p(cx - bw * 0.02, y3), p(cx, top));
    path.add_line(p(cx, top), p(cx + bw * 0.02, y3));
    add_beziers(&mut path, &right);
    path.add_line(p(cx + bw * 0.5, bottom), p(cx + bw * 0.3, bottom));
    path.add_bezier(p(cx + bw * 0.3, bottom), p(cx + bw * 0.26, bottom - h * 0.17), p(cx - bw * 0.26, bottom - h * 0.17), p(cx - bw * 0.3, bottom));
    path.close_figure();
    g.fill_path(&solid(fill), &path);
    let st = g.save();
    g.set_clip_path(&path);
    let step = 3f32.max(bw * 0.055);
    let pen = Pen::new(fade(line, line.a as i32 / 2), 1.0);
    let mut d = -h;
    while d < bw + h {
        g.draw_line_f(&pen, cx - bw / 2.0 + d, bottom, cx - bw / 2.0 + d - h, top);
        g.draw_line_f(&pen, cx - bw / 2.0 + d - h, bottom, cx - bw / 2.0 + d, top);
        d += step;
    }
    g.restore(st);
    g.draw_path(&Pen::new(line, (bw * 0.008).max(1.0)), &path);
    for q in [[y1, 0.64f32], [y2, 0.3]] {
        let deck = rf(cx - bw * q[1] / 2.0, q[0] - h * 0.022, bw * q[1], h * 0.03);
        g.fill_rectangle(&solid(fill), deck);
        g.draw_rectangle(&Pen::new(line, (bw * 0.008).max(1.0)), deck);
    }
    let tip = 2f32.max(bw * 0.025);
    soft_glow(g, p(cx, top), tip * 5.0, fade(Color::WHITE, 150));
    g.fill_ellipse(&solid(Color::WHITE), rf(cx - tip / 2.0, top - tip / 2.0, tip, tip));
}
/// AddBeziers: the first point, then three per curve.
fn add_beziers(path: &mut GraphicsPath, pts: &[PointF]) {
    let mut i = 0;
    while i + 3 < pts.len() {
        path.add_bezier(pts[i], pts[i + 1], pts[i + 2], pts[i + 3]);
        i += 3;
    }
}
fn soft_glow(g: &mut Graphics, c: PointF, rad: f32, color: Color) {
    let r = rf(c.x - rad, c.y - rad, rad * 2.0, rad * 2.0);
    g.fill_ellipse(&ellipse_grad(r, color, fade(color, 0)), r);
}
fn cloud(g: &mut Graphics, x: f32, y: f32, w: f32, c: Color) {
    let mut path = GraphicsPath::new();
    path.fill_mode = FillMode::Winding;
    path.add_ellipse(rf(x, y - w * 0.04, w * 0.42, w * 0.24));
    path.add_ellipse(rf(x + w * 0.2, y - w * 0.2, w * 0.42, w * 0.4));
    path.add_ellipse(rf(x + w * 0.48, y - w * 0.08, w * 0.42, w * 0.28));
    path.add_rectangle(rf(x + w * 0.16, y + w * 0.06, w * 0.6, w * 0.14));
    g.fill_path(&solid(c), &path);
}
fn bez(a: PointF, b: PointF, c: PointF, d: PointF, t: f32) -> PointF {
    let s = 1.0 - t;
    p(s * s * s * a.x + 3.0 * s * s * t * b.x + 3.0 * s * t * t * c.x + t * t * t * d.x, s * s * s * a.y + 3.0 * s * s * t * b.y + 3.0 * s * t * t * c.y + t * t * t * d.y)
}
fn rays(g: &mut Graphics, o: PointF, reach: f32, n: i32, c: Color) {
    let b = solid(c);
    let mut i = 0;
    while i < n {
        let a0 = i as f64 * PI * 2.0 / n as f64;
        let a1 = (i + 1) as f64 * PI * 2.0 / n as f64;
        g.fill_polygon(&b, &[o, p(o.x + cos(a0) * reach, o.y + sin(a0) * reach), p(o.x + cos(a1) * reach, o.y + sin(a1) * reach)]);
        i += 2;
    }
}
fn hoenn_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let horizon = r.bottom() - (u * 0.42).max(r.height * 0.3);
    let sea = mix(Color::rgb(40, 92, 140), th.main, 0.35);
    let ink = mix(th.deep, Color::BLACK, 0.4);
    let foam = Color::rgb(246, 242, 228);
    let glow = mix(th.light, Color::WHITE, 0.45);
    let sun = p(r.x + r.width * 0.3, horizon - u * 0.62);
    let st = g.save();
    g.set_clip_rect(rf(r.x, r.y, r.width, horizon - r.y));
    rays(g, sun, r.width.max(r.height) * 1.4, 48, fade(glow, 34));
    g.restore(st);
    let sr = u * 0.13;
    soft_glow(g, sun, sr * 2.2, fade(glow, 120));
    g.fill_ellipse(&solid(fade(glow, 235)), rf(sun.x - sr, sun.y - sr, sr * 2.0, sr * 2.0));
    let pen = Pen::new(fade(ink, 160), (u * 0.006).max(1.0));
    g.draw_ellipse(&pen, rf(sun.x - sr, sun.y - sr, sr * 2.0, sr * 2.0));
    g.draw_ellipse(&pen, rf(sun.x - sr * 1.15, sun.y - sr * 1.15, sr * 2.3, sr * 2.3));
    let vx = r.x + r.width * 0.64;
    let vh = u * 0.55;
    let vw = u * 1.05;
    let crater = horizon - vh;
    let mut rnd = NetRandom::new(2014);
    for i in (0..=9).rev() {
        let k = i as f32 / 9.0;
        let pr = u * (0.05 + 0.07 * k);
        let c = p(vx + u * 0.12 * k * k + sin((i as f32 * 1.7) as f64) * u * 0.03, crater - u * 0.06 - u * 0.5 * k);
        g.fill_ellipse(&solid(fade(mix(Color::rgb(108, 94, 82), th.deep, 0.3), 235)), rf(c.x - pr, c.y - pr, pr * 2.0, pr * 2.0));
        g.fill_ellipse(&solid(fade(mix(Color::rgb(214, 200, 176), th.light, 0.2), 120)), rf(c.x - pr * 0.8, c.y - pr * 0.9, pr * 1.1, pr * 0.9));
    }
    let mut path = GraphicsPath::new();
    path.add_bezier(p(vx - vw * 0.62, horizon + 2.0), p(vx - vw * 0.3, horizon - vh * 0.25), p(vx - vw * 0.16, crater + vh * 0.2), p(vx - vw * 0.09, crater));
    path.add_line(p(vx - vw * 0.09, crater), p(vx - vw * 0.03, crater + vh * 0.05));
    path.add_line(p(vx - vw * 0.03, crater + vh * 0.05), p(vx + vw * 0.04, crater + vh * 0.02));
    path.add_line(p(vx + vw * 0.04, crater + vh * 0.02), p(vx + vw * 0.1, crater));
    path.add_bezier(p(vx + vw * 0.1, crater), p(vx + vw * 0.18, crater + vh * 0.2), p(vx + vw * 0.32, horizon - vh * 0.25), p(vx + vw * 0.62, horizon + 2.0));
    path.close_figure();
    g.fill_path(&lin(rf(vx - vw, crater - 1.0, vw * 2.0, vh + 4.0), mix(ink, th.main, 0.35), ink, 90.0), &path);
    let clip = g.save();
    g.set_clip_path(&path);
    let mut pen = Pen::new(fade(glow, 60), (u * 0.012).max(1.5));
    pen.end_cap = LineCap::Round;
    for i in -4..=4 {
        let i = i as f32;
        bez4(g, &pen, [vx + i * vw * 0.02, crater + vh * 0.06, vx + i * vw * 0.05, crater + vh * 0.4, vx + i * vw * 0.1, horizon - vh * 0.3, vx + i * vw * 0.16, horizon + 2.0]);
    }
    for side in [-1.0f32, 1.0, -1.0] {
        let off = side * vw * (0.05 + 0.04 * nd(&mut rnd));
        let mut pen = Pen::new(Color::rgb(214, 70, 30), (u * 0.024).max(2.0));
        pen.end_cap = LineCap::Round;
        let end_y = crater + vh * (0.62 + 0.2 * nd(&mut rnd));
        bez4(g, &pen, [vx, crater + vh * 0.03, vx + off * 0.5, crater + vh * 0.3, vx + off * 1.2, crater + vh * 0.5, vx + off * 1.9, end_y]);
        let mut pen = Pen::new(Color::rgb(255, 196, 80), (u * 0.009).max(1.0));
        pen.end_cap = LineCap::Round;
        bez4(g, &pen, [vx, crater + vh * 0.03, vx + off * 0.5, crater + vh * 0.3, vx + off * 1.2, crater + vh * 0.5, vx + off * 1.6, crater + vh * 0.55]);
    }
    g.restore(clip);
    g.draw_path(&Pen::new(fade(ink, 220), (u * 0.006).max(1.0)), &path);
    soft_glow(g, p(vx, crater), u * 0.09, fade(Color::rgb(255, 150, 60), 170));
    let sea_rect = rf(r.x, horizon, r.width, r.bottom() - horizon + 1.0);
    gradient(g, sea_rect, &[mix(sea, Color::WHITE, 0.25), sea, mix(sea, ink, 0.6)], &[0.0, 0.4, 1.0]);
    for row in 0..7 {
        let y = horizon + (r.bottom() - horizon) * (row as f32 + 0.5) / 7.0;
        let amp = u * (0.006 + row as f32 * 0.003);
        let pen = Pen::new(fade(foam, 90 + row * 12), (u * (0.004 + row as f32 * 0.0012)).max(1.0));
        g.draw_path(&pen, &wave_line(r, y, amp, row as f32 * 1.3));
    }
    let rx = r.right() - r.width * 0.16;
    let ry = r.bottom() - (r.bottom() - horizon) * 0.35;
    for i in 0..2 {
        let w = u * (0.22 - i as f32 * 0.08);
        let h = u * (0.11 - i as f32 * 0.03);
        let x = rx - u * 0.12 * i as f32;
        g.fill_polygon(&solid(mix(ink, th.deep, 0.3)), &[p(x - w / 2.0, ry + h * 0.4), p(x - w * 0.3, ry - h * 0.5), p(x + w * 0.05, ry - h * 0.7), p(x + w * 0.4, ry - h * 0.2), p(x + w / 2.0, ry + h * 0.4)]);
    }
    for _ in 0..18 {
        let a = (rnd.next_double() * PI) as f32;
        let d = u * (0.05 + 0.12 * nd(&mut rnd));
        let s = u * (0.01 + 0.02 * nd(&mut rnd));
        let fx = rx - u * 0.06 + cos(a as f64) * d * 1.4;
        let fy = ry - sin(a as f64) * d * 0.7;
        let al = 120 + rnd.next(110);
        g.fill_ellipse(&solid(fade(foam, al)), rf(fx - s, fy - s, s * 2.0, s * 2.0));
    }
    great_wave(g, r.x - u * 0.12, r.bottom() + 2.0, (u * 1.0).min(r.width * 0.7), u * 0.5, sea, foam, ink);
}
fn storm_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let horizon = r.bottom() - (u * 0.5).max(r.height * 0.5);
    let storm = Color::rgb(70, 80, 100);
    let ink = mix(th.deep, Color::BLACK, 0.5);
    let foam = Color::rgb(236, 242, 248);
    let sea = mix(Color::rgb(24, 64, 100), th.main, 0.3);
    gradient(g, rf(r.x, r.y, r.width, horizon - r.y + 1.0), &[fade(mix(th.deep, Color::BLACK, 0.45), 235), fade(mix(th.deep, storm, 0.5), 220), fade(mix(th.main, storm, 0.55), 225)], &[0.0, 0.55, 1.0]);
    soft_glow(g, p(r.x + r.width * 0.28, horizon), u * 0.45, fade(mix(th.light, Color::WHITE, 0.5), 110));
    for layer in 0..3 {
        let y = r.y + (horizon - r.y) * (0.15 + 0.3 * layer as f32);
        let size = u * (0.2 + 0.05 * layer as f32);
        let body = mix(mix(storm, th.deep, 0.4), Color::BLACK, 0.15 + 0.12 * layer as f32);
        let rim = mix(body, Color::WHITE, 0.3);
        for pass in 0..2 {
            let mut x = r.x - size * if layer % 2 == 0 { 0.5 } else { 1.1 };
            while x < r.right() + size {
                let d = size * (0.8 + 0.5 * ((x * 0.37 + layer as f32) as f64).sin().abs() as f32);
                g.fill_ellipse(&solid(fade(if pass == 0 { rim } else { body }, 235)), rf(x - d / 2.0, y - d / 2.0 + if pass == 0 { -u * 0.012 } else { 0.0 }, d, d * 0.7));
                x += size * 0.9;
            }
        }
    }
    let mut rnd = NetRandom::new(2015);
    let mut bolt = vec![p(r.x + r.width * 0.66, r.y + (horizon - r.y) * 0.4)];
    while bolt[bolt.len() - 1].y < horizon {
        let q = bolt[bolt.len() - 1];
        let dx = (rnd.next_double() - 0.5) as f32 * u * 0.1;
        let dy = u * (0.04 + 0.04 * nd(&mut rnd));
        bolt.push(p(q.x + dx, q.y + dy));
    }
    let mut fork = vec![bolt[bolt.len() / 3]];
    let mut fork2 = vec![bolt[bolt.len() / 2]];
    for _ in 0..4 {
        let q = fork[fork.len() - 1];
        let dx = u * (0.02 + 0.04 * nd(&mut rnd));
        let dy = u * (0.03 + 0.03 * nd(&mut rnd));
        fork.push(p(q.x + dx, q.y + dy));
    }
    for _ in 0..3 {
        let q = fork2[fork2.len() - 1];
        let dx = u * (0.02 + 0.04 * nd(&mut rnd));
        let dy = u * (0.03 + 0.03 * nd(&mut rnd));
        fork2.push(p(q.x - dx, q.y + dy));
    }
    soft_glow(g, bolt[bolt.len() / 2], u * 0.45, fade(Color::rgb(200, 220, 255), 140));
    soft_glow(g, bolt[0], u * 0.25, fade(Color::rgb(230, 240, 255), 170));
    for line in [&bolt, &fork, &fork2] {
        let mut pen = Pen::new(fade(Color::rgb(180, 210, 255), 130), (u * 0.04).max(4.0));
        pen.line_join = LineJoin::Round;
        g.draw_lines(&pen, line);
        let mut pen = Pen::new(Color::rgb(252, 253, 255), (u * 0.013).max(2.0));
        pen.line_join = LineJoin::Round;
        g.draw_lines(&pen, line);
    }
    gradient(g, rf(r.x, horizon, r.width, r.bottom() - horizon + 1.0), &[mix(sea, storm, 0.3), sea, mix(sea, Color::BLACK, 0.45)], &[0.0, 0.4, 1.0]);
    for i in 0..10 {
        let y = horizon + (r.bottom() - horizon) * (i as f32 + 0.5) / 10.0;
        let pen = Pen::new(fade(foam, 70 + i * 6), (u * 0.003).max(1.0));
        g.draw_path(&pen, &wave_line(r, y, u * (0.004 + i as f32 * 0.0015), i as f32 * 1.9));
    }
    for row in 0..4 {
        let k = row as f32 / 3.0;
        let hh = u * (0.05 + 0.09 * k);
        let ww = hh * 1.7;
        let y = horizon + (r.bottom() - horizon) * (0.12 + 0.3 * row as f32) + hh * 0.3;
        let mut x = r.x - ww * if row % 2 == 0 { 0.2 } else { 0.9 };
        while x < r.right() {
            great_wave(g, x, y, ww, hh, mix(sea, Color::rgb(120, 200, 220), 0.2 + 0.1 * k), foam, ink);
            x += ww * 1.45;
        }
    }
    let gh = (u * 0.7).max((r.bottom() - horizon) * 0.9);
    great_wave(g, r.x - u * 0.08, r.bottom() + 2.0, (gh * 1.9).min(r.width * 0.85), gh, mix(sea, Color::rgb(120, 200, 220), 0.35), foam, ink);
}
fn storm_live(g: &mut Graphics, r: RectangleF, t: f32) {
    let u = r.height.min(r.width * 0.55);
    let len = u * 0.06;
    let pen = Pen::new(fade(Color::rgb(220, 230, 245), 70), (u * 0.003).max(1.0));
    for i in 0..50 {
        let seed = (i as f32 * 0.618) % 1.0;
        let fall = (t * (0.5 + 0.3 * seed) + seed * 7.0) % 1.0;
        let x = r.x + (((i as f32 * 0.381) % 1.0) * 1.3 - fall * 0.3) * r.width;
        let y = r.y + fall * (r.height + len) - len;
        g.draw_line_f(&pen, x, y, x - len * 0.35, y + len);
    }
    let pp = t % 9.0;
    let a = if pp < 0.2 { 1.0 - pp / 0.2 } else if pp > 0.32 && pp < 0.5 { 0.6 * (1.0 - (pp - 0.32) / 0.18) } else { 0.0 };
    if a > 0.0 { g.fill_rectangle(&solid(fade(Color::rgb(225, 235, 255), (70.0 * a) as i32)), r); }
}
fn round_pen(c: Color, w: f32) -> Pen { Pen::new(c, w).round() }
fn great_wave(g: &mut Graphics, x: f32, y: f32, w: f32, h: f32, body: Color, foam: Color, ink: Color) {
    let back = [p(x, y), p(x + w * 0.08, y - h * 0.55), p(x + w * 0.2, y - h), p(x + w * 0.45, y - h)];
    let curl = [back[3], p(x + w * 0.66, y - h), p(x + w * 0.84, y - h * 0.82), p(x + w * 0.8, y - h * 0.56)];
    let under = [curl[3], p(x + w * 0.74, y - h * 0.44), p(x + w * 0.6, y - h * 0.5), p(x + w * 0.58, y - h * 0.64)];
    let front = [under[3], p(x + w * 0.56, y - h * 0.3), p(x + w * 0.78, y - h * 0.06), p(x + w, y)];
    let line = (h * 0.012).max(1.0);
    let mut path = GraphicsPath::new();
    for c in [back, curl, under, front] { path.add_bezier(c[0], c[1], c[2], c[3]); }
    path.close_figure();
    g.fill_path(&lin(rf(x, y - h - 2.0, w, h + 4.0), mix(body, Color::WHITE, 0.12), mix(body, ink, 0.6), 90.0), &path);
    let st = g.save();
    g.set_clip_path(&path);
    let pen = Pen::new(fade(mix(body, Color::WHITE, 0.4), 210), h * 0.14);
    bez4(g, &pen, [back[0].x + w * 0.05, back[0].y, back[1].x + w * 0.05, back[1].y + h * 0.04, back[2].x + w * 0.04, back[2].y + h * 0.08, curl[1].x, curl[1].y + h * 0.1]);
    let pen = Pen::new(fade(foam, 190), (h * 0.009).max(1.0));
    for k in 1..=8 {
        let dx = k as f32 * w * 0.032;
        let dy = k as f32 * h * 0.045;
        bez4(g, &pen, [back[0].x + dx, back[0].y, back[1].x + dx, back[1].y + dy, back[2].x + dx, back[2].y + dy, curl[1].x - dx * 0.2, curl[1].y + dy]);
    }
    g.restore(st);
    g.draw_path(&Pen::new(ink, line), &path);
    bez4(g, &round_pen(foam, (h * 0.05).max(1.5)), [curl[0].x - w * 0.05, curl[0].y + h * 0.03, curl[1].x, curl[1].y + h * 0.03, curl[2].x - w * 0.02, curl[2].y + h * 0.02, curl[3].x - w * 0.02, curl[3].y]);
    for i in 0..=16 {
        let t = i as f32 / 16.0;
        let (q0, q1) = if t < 0.75 {
            let tt = 0.2 + t / 0.75 * 0.8;
            (bez(curl[0], curl[1], curl[2], curl[3], tt), bez(curl[0], curl[1], curl[2], curl[3], (tt + 0.02).min(1.0)))
        } else {
            let tt = (t - 0.75) / 0.25 * 0.45;
            (bez(under[0], under[1], under[2], under[3], tt), bez(under[0], under[1], under[2], under[3], tt + 0.02))
        };
        let tan = ((q1.y - q0.y) as f64).atan2((q1.x - q0.x) as f64);
        let base_a = tan + 0.9;
        let sz = h * (0.07 + 0.06 * sin(t as f64 * PI));
        for f in -1..=1 {
            let a = base_a + f as f64 * 0.45;
            let tip = p(q0.x + cos(a) * sz, q0.y + sin(a) * sz);
            let c1 = p(q0.x + cos(a) * sz * 0.5, q0.y + sin(a) * sz * 0.5);
            let c2 = p(tip.x + cos(a + 1.6) * sz * 0.35, tip.y + sin(a + 1.6) * sz * 0.35);
            let end = p(tip.x + cos(a + 2.4) * sz * 0.3, tip.y + sin(a + 2.4) * sz * 0.3);
            draw_bezier(g, &round_pen(ink, (sz * 0.3).max(1.5)), q0, c1, c2, end);
            draw_bezier(g, &round_pen(foam, (sz * 0.18).max(1.0)), q0, c1, c2, end);
        }
    }
    let mut rnd = NetRandom::new(1831 + h as i32);
    for _ in 0..18 {
        let s = h * (0.01 + 0.018 * nd(&mut rnd));
        let fx = x + w * (0.62 + 0.32 * nd(&mut rnd));
        let fy = y - h * (0.3 + 0.55 * nd(&mut rnd));
        g.fill_ellipse(&solid(fade(foam, 235)), rf(fx - s, fy - s, s * 2.0, s * 2.0));
    }
}
fn alola_horizon(r: RectangleF) -> f32 {
    let u = r.height.min(r.width * 0.55);
    r.bottom() - (u * 0.4).max(r.height * 0.42)
}
fn alola_sky(r: RectangleF, th: &Theme) -> PointF {
    let u = r.height.min(r.width * 0.55);
    let sea = alola_horizon(r);
    if th.finish.contains("moon") { p(r.x + r.width * 0.66, sea - (u * 0.4).min((sea - r.y) * 0.5)) } else { p(r.x + r.width * 0.64, sea) }
}
fn complement(c: Color) -> Color {
    let m = c.r.max(c.g.max(c.b)) as i32 + c.r.min(c.g.min(c.b)) as i32;
    Color::rgb(m - c.r as i32, m - c.g as i32, m - c.b as i32)
}
fn alola_scene(g: &mut Graphics, r: RectangleF, th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let sea = alola_horizon(r);
    let night = th.finish.contains("moon");
    let ultra = th.finish.contains("ultra");
    let warm = if night { Color::rgb(150, 170, 230) } else { Color::rgb(255, 168, 72) };
    let glow = mix(th.light, Color::WHITE, 0.45);
    let ink = mix(th.deep, Color::BLACK, 0.45);
    let halo = if ultra { mix(complement(th.light), Color::WHITE, 0.15) } else if night { Color::rgb(220, 228, 255) } else { mix(glow, warm, 0.3) };
    let trail = if ultra { mix(halo, Color::WHITE, 0.3) } else if night { Color::rgb(226, 234, 255) } else { Color::rgb(255, 226, 160) };
    if night {
        gradient(g, rf(r.x, r.y, r.width, sea - r.y + 1.0), &[fade(mix(th.deep, Color::BLACK, 0.3), 200), fade(th.deep, 170), fade(mix(th.main, warm, 0.3), 200)], &[0.0, 0.6, 1.0]);
    } else {
        gradient(g, rf(r.x, r.y, r.width, sea - r.y + 1.0), &[fade(th.deep, 0), fade(mix(th.main, warm, 0.35), 140), fade(mix(th.light, warm, 0.6), 225)], &[0.0, 0.6, 1.0]);
    }
    let sun = alola_sky(r, th);
    let sr = u * if night { 0.13 } else { 0.2 };
    let st = g.save();
    g.set_clip_rect(rf(r.x, r.y, r.width, sea - r.y));
    let mut rnd = NetRandom::new(2016);
    if night {
        for _ in 0..40 {
            let x = r.x + nd(&mut rnd) * r.width;
            let y = r.y + nd(&mut rnd) * (sea - r.y) * 0.9;
            let d = u * (0.004 + 0.006 * nd(&mut rnd));
            let a = 120 + rnd.next(120);
            g.fill_ellipse(&solid(fade(Color::WHITE, a)), rf(x - d, y - d, d * 2.0, d * 2.0));
        }
        if ultra { rays(g, sun, r.width.max(r.height) * 1.4, 56, fade(halo, 30)); }
        soft_glow(g, sun, sr * if ultra { 4.2 } else { 3.0 }, fade(halo, if ultra { 200 } else { 110 }));
        if ultra { soft_glow(g, sun, sr * 1.8, fade(halo, 230)); }
        g.fill_ellipse(&solid(Color::rgb(240, 242, 250)), rf(sun.x - sr, sun.y - sr, sr * 2.0, sr * 2.0));
        let b = solid(fade(mix(th.light, Color::rgb(150, 156, 176), 0.6), 120));
        g.fill_ellipse(&b, rf(sun.x - sr * 0.5, sun.y - sr * 0.45, sr * 0.42, sr * 0.34));
        g.fill_ellipse(&b, rf(sun.x + sr * 0.12, sun.y + sr * 0.1, sr * 0.5, sr * 0.4));
        g.fill_ellipse(&b, rf(sun.x + sr * 0.22, sun.y - sr * 0.58, sr * 0.24, sr * 0.22));
        g.fill_ellipse(&b, rf(sun.x - sr * 0.55, sun.y + sr * 0.3, sr * 0.22, sr * 0.2));
        if ultra { g.draw_ellipse(&Pen::new(fade(halo, 230), (u * 0.008).max(1.5)), rf(sun.x - sr, sun.y - sr, sr * 2.0, sr * 2.0)); }
    } else {
        rays(g, sun, r.width.max(r.height) * 1.4, 40, fade(if ultra { halo } else { glow }, if ultra { 56 } else { 40 }));
        soft_glow(g, sun, sr * if ultra { 3.2 } else { 2.4 }, fade(halo, if ultra { 210 } else { 150 }));
        g.fill_ellipse(&solid(mix(glow, Color::rgb(255, 220, 120), 0.4)), rf(sun.x - sr, sun.y - sr, sr * 2.0, sr * 2.0));
        if ultra { g.draw_ellipse(&Pen::new(fade(halo, 240), (u * 0.01).max(1.5)), rf(sun.x - sr, sun.y - sr, sr * 2.0, sr * 2.0)); }
    }
    g.restore(st);
    for i in 0..4 {
        let cw = u * (0.42 - i as f32 * 0.06);
        let cx = r.x + r.width * (0.02 + i as f32 * 0.26);
        let cy = r.y + (sea - r.y) * (0.22 + 0.18 * (i % 2) as f32);
        cloud(g, cx, cy + u * 0.012, cw, fade(if night { halo } else { mix(glow, warm, 0.5) }, if night { 90 } else { 150 }));
        cloud(g, cx, cy, cw, fade(mix(th.deep, th.main, 0.4), if night { 190 } else { 210 }));
    }
    let sea_colors = if night { [mix(Color::rgb(30, 60, 110), th.main, 0.3), mix(Color::rgb(8, 16, 40), th.deep, 0.4)] } else { [mix(Color::rgb(40, 120, 168), th.main, 0.3), mix(Color::rgb(20, 60, 110), th.deep, 0.4)] };
    gradient(g, rf(r.x, sea, r.width, r.bottom() - sea + 1.0), &sea_colors, &[0.0, 1.0]);
    if night { soft_glow(g, p(sun.x, sea), sr * 1.6, fade(halo, if ultra { 120 } else { 70 })); }
    for i in 0..9 {
        let k = i as f32 / 8.0;
        let y = sea + u * 0.02 + k * (r.bottom() - sea) * 0.6;
        let half = (if night { u * 0.2 } else { sr }) * (0.9 - 0.6 * k);
        g.draw_line_f(&Pen::new(fade(trail, 170 - i * 14), (u * 0.01).max(1.0)), sun.x - half, y, sun.x + half, y);
    }
    {
        let b = solid(ink);
        let ix = r.x + r.width * 0.08;
        let iw = r.width * 0.36;
        g.fill_pie(&b, ix, sea - u * 0.05, iw, u * 0.1, 180.0, 180.0);
        for i in 0..3 {
            let px = ix + iw * (0.3 + i as f32 * 0.2);
            let ph = u * (0.13 + 0.03 * (i % 2) as f32);
            bez4(g, &Pen::new(ink, (u * 0.008).max(1.5)), [px, sea - u * 0.03, px + u * 0.01, sea - ph * 0.5, px + u * 0.03, sea - ph * 0.8, px + u * 0.025, sea - ph]);
            for f in 0..6 { frond(g, p(px + u * 0.025, sea - ph), -PI / 2.0 + (f as f64 - 2.5) * 0.55, u * 0.07, u * 0.03, ink, (u * 0.006).max(1.0)); }
        }
    }
    let pen = Pen::new(fade(Color::WHITE, 90), (u * 0.004).max(1.0));
    for i in 0..3 { g.draw_path(&pen, &wave_line(r, sea + (r.bottom() - sea) * (0.55 + i as f32 * 0.15), u * 0.008, i as f32 * 2.1)); }
    let sand = mix(Color::rgb(236, 206, 150), if night { th.deep } else { th.light }, if night { 0.45 } else { 0.2 });
    let mut path = GraphicsPath::new();
    path.add_bezier(p(r.x - 4.0, r.bottom() - u * 0.2), p(r.x + r.width * 0.2, r.bottom() - u * 0.2), p(r.x + r.width * 0.4, r.bottom() - u * 0.06), p(r.x + r.width * 0.62, r.bottom() + 2.0));
    path.add_line(p(r.x + r.width * 0.62, r.bottom() + 2.0), p(r.x - 4.0, r.bottom() + 2.0));
    path.close_figure();
    g.fill_path(&lin(rf(r.x, r.bottom() - u * 0.21, r.width, u * 0.22), sand, mix(sand, th.deep, 0.35), 90.0), &path);
    bez4(g, &Pen::new(fade(Color::WHITE, 200), (u * 0.008).max(1.5)), [r.x - 4.0, r.bottom() - u * 0.2, r.x + r.width * 0.2, r.bottom() - u * 0.2, r.x + r.width * 0.4, r.bottom() - u * 0.06, r.x + r.width * 0.62, r.bottom() + 2.0]);
    let foot = p(r.x + r.width * 0.14, r.bottom() - u * 0.1);
    let crown = p(r.x + r.width * 0.34, (r.y + u * 0.2).max(sea - u * 0.45));
    let c1 = p(foot.x + (crown.x - foot.x) * 0.05, foot.y - (foot.y - crown.y) * 0.4);
    let c2 = p(foot.x + (crown.x - foot.x) * 0.45, crown.y + (foot.y - crown.y) * 0.15);
    let bark = mix(ink, Color::rgb(110, 80, 50), 0.35);
    let tw = (u * 0.045).max(3.0);
    let mut lside = Vec::new();
    let mut rside = Vec::new();
    for i in 0..=30 {
        let t = i as f32 / 30.0;
        let w = tw * (1.0 - 0.45 * t) / 2.0;
        let q = bez(foot, c1, c2, crown, t);
        let a = bez(foot, c1, c2, crown, (t + 0.01).min(1.0));
        let o = bez(foot, c1, c2, crown, (t - 0.01).max(0.0));
        let (dx, dy) = (a.x - o.x, a.y - o.y);
        let d = (dx * dx + dy * dy).sqrt() + 0.001;
        lside.push(p(q.x - dy / d * w, q.y + dx / d * w));
        rside.push(p(q.x + dy / d * w, q.y - dx / d * w));
    }
    rside.reverse();
    lside.extend(rside);
    g.fill_polygon(&solid(bark), &lside);
    let pen = Pen::new(fade(mix(bark, Color::WHITE, 0.3), 150), (u * 0.004).max(1.0));
    for i in 1..14 {
        let t = i as f32 / 14.0;
        let w = tw * (1.0 - 0.45 * t) / 2.0;
        let q = bez(foot, c1, c2, crown, t);
        g.draw_line_f(&pen, q.x - w * 0.9, q.y + w * 0.25, q.x + w * 0.9, q.y - w * 0.25);
    }
    let leaf = mix(ink, Color::rgb(40, 90, 50), 0.4);
    for f in 0..9 {
        let a = -PI + f as f64 * PI / 8.0 + (f % 2) as f64 * 0.12;
        frond(g, crown, a, u * (0.36 + 0.05 * (f % 3) as f32), u * 0.16, leaf, (u * 0.012).max(1.5));
    }
    let b = solid(mix(bark, warm, 0.2));
    for i in 0..3 {
        let s = u * 0.028;
        g.fill_ellipse(&b, rf(crown.x - s * 1.6 + i as f32 * s * 1.05, crown.y + s * 0.1 + (i % 2) as f32 * s * 0.5, s, s));
    }
}
fn frond(g: &mut Graphics, o: PointF, a: f64, len: f32, droop: f32, c: Color, wid: f32) {
    let mut spine = [PointF::default(); 9];
    for k in 0..9 {
        let t = k as f32 / 8.0;
        spine[k] = p(o.x + cos(a) * len * t, o.y + sin(a) * len * t + droop * t * t);
    }
    let b = solid(c);
    g.draw_curve(&round_pen(c, wid), &spine);
    for k in 1..8 {
        let t = k as f32 / 8.0;
        let leaf_len = len * (0.08 + 0.26 * sin(t as f64 * PI));
        let bw = (len * 0.05 * (1.0 - t * 0.5)).max(1.5);
        let dir = ((spine[k + 1].y - spine[k - 1].y) as f64).atan2((spine[k + 1].x - spine[k - 1].x) as f64);
        for side in [-1.0f64, 1.0] {
            let la = dir + side * 0.75;
            let tip = p(spine[k].x + cos(la) * leaf_len, spine[k].y + sin(la) * leaf_len + leaf_len * 0.5);
            let nx = cos(dir) * bw;
            let ny = sin(dir) * bw;
            g.fill_polygon(&b, &[p(spine[k].x - nx, spine[k].y - ny), p(spine[k].x + nx, spine[k].y + ny), tip]);
        }
    }
}
fn sun_glints(g: &mut Graphics, r: RectangleF, t: f32, th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let sea = alola_horizon(r);
    let sx = alola_sky(r, th).x;
    let c = if th.finish.contains("ultra") { mix(complement(th.light), Color::WHITE, 0.5) } else if th.finish.contains("moon") { Color::rgb(232, 238, 255) } else { Color::rgb(255, 240, 200) };
    for i in 0..14 {
        let depth = (i as f32 * 0.618) % 1.0;
        let beat = 0.5 + 0.5 * sin((t * 1.6 + i as f32 * 1.9) as f64);
        let y = sea + u * 0.03 + depth * (r.bottom() - sea) * 0.6;
        let spread = u * (0.05 + 0.2 * depth);
        let x = sx + (((i as f32 * 0.381) % 1.0) - 0.5) * 2.0 * spread + sin((t * 0.6 + i as f32) as f64) * u * 0.015;
        let len = u * (0.02 + 0.04 * depth);
        g.draw_line_f(&Pen::new(fade(c, (210.0 * beat) as i32), (u * 0.006).max(1.0)), x - len, y, x + len, y);
    }
}
const BOLT_SHAPE: [[f32; 2]; 7] = [[0.55, 0.0], [0.1, 0.55], [0.42, 0.55], [0.22, 1.0], [0.9, 0.38], [0.55, 0.38], [0.82, 0.0]];
fn bolt(g: &mut Graphics, x: f32, y: f32, h: f32, fill: Color, edge: Color) {
    let pts: Vec<PointF> = BOLT_SHAPE.iter().map(|q| p(x + q[0] * h * 0.62, y + q[1] * h)).collect();
    g.fill_polygon(&solid(fill), &pts);
    g.draw_polygon(&Pen::new(edge, (h * 0.018).max(1.5)), &pts);
}
fn bolt_scene(g: &mut Graphics, r: RectangleF, _th: &Theme) {
    let u = r.height.min(r.width * 0.55);
    let o = p(r.x + r.width * 0.8, r.y + r.height * 0.5);
    let reach = r.width.max(r.height) * 1.3;
    for i in 0..12 {
        let a0 = i as f64 * PI / 6.0;
        let a1 = a0 + PI / 12.0;
        g.fill_polygon(&solid(fade(Color::WHITE, 44)), &[o, p(o.x + cos(a0) * reach, o.y + sin(a0) * reach), p(o.x + cos(a1) * reach, o.y + sin(a1) * reach)]);
    }
    bolt(g, o.x - u * 0.28, o.y - u * 0.45, u * 0.9, Color::rgb(255, 251, 212), Color::rgb(70, 50, 16));
    let b = solid(Color::rgb(232, 72, 60));
    let pen = Pen::new(Color::rgb(120, 40, 24), 1.0);
    for c in [p(r.x + r.width * 0.58, r.bottom() - u * 0.16), p(r.x + r.width * 0.95, r.bottom() - u * 0.16)] {
        let d = u * 0.09;
        g.fill_ellipse(&b, rf(c.x - d, c.y - d, d * 2.0, d * 2.0));
        g.draw_ellipse(&pen, rf(c.x - d, c.y - d, d * 2.0, d * 2.0));
    }
}
fn crackle(g: &mut Graphics, r: RectangleF, t: f32) {
    let u = r.height.min(r.width * 0.55);
    for i in 0..4 {
        let phase = t * 1.3 + i as f32 * 0.31;
        let seed = (phase as f64).floor() as i32 * 31 + i * 17;
        let beat = sin((phase as f64 - (phase as f64).floor()) * PI);
        let mut rnd = NetRandom::new(seed);
        let mut x = r.x + nd(&mut rnd) * r.width;
        let mut y = r.y + nd(&mut rnd) * r.height;
        let mut pts = [PointF::default(); 5];
        for k in 0..5 {
            pts[k] = p(x, y);
            x += u * (0.03 * (rnd.next_double() * 2.0 - 1.0) as f32) + u * 0.012;
            y += u * 0.024;
        }
        g.draw_lines(&Pen::new(fade(Color::WHITE, (230.0 * beat) as i32), (u * 0.006).max(1.2)), &pts);
    }
}
