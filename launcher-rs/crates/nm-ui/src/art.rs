//! Shapes and code-drawn icons: Round, Card, GlossTile, AppArt, RowArt, Ball, Sparkle, SpinningBall.

use nm_draw::*;

pub const LINE: Color = Color::rgb(38, 42, 54);
pub const CREAM: Color = Color::rgb(250, 246, 230);
pub const INK: Color = Color::rgb(40, 36, 52);
pub const TEXT3: Color = Color::rgb(150, 148, 136);
pub const PHONE_BG: Color = Color::rgb(242, 242, 247);
pub const PHONE_ROW: Color = Color::WHITE;
pub const PHONE_LINE: Color = Color::rgb(222, 222, 228);
pub const PHONE_TEXT: Color = Color::rgb(28, 28, 30);
pub const PHONE_SUB: Color = Color::rgb(108, 108, 114);

pub const APP_NAMES: [&str; 9] = ["", "Games", "Collection", "Mods", "ROMs", "Emu", "Move", "Settings", "Exit"];
pub const APP_COLORS: [Color; 9] = [Color::EMPTY, Color::rgb(232, 62, 58), Color::rgb(240, 142, 30), Color::rgb(124, 84, 214), Color::rgb(40, 124, 232), Color::rgb(212, 56, 126), Color::rgb(24, 164, 150), Color::rgb(120, 124, 134), Color::rgb(58, 60, 68)];
pub fn app(name: &str) -> usize { APP_NAMES.iter().position(|n| *n == name).unwrap_or(0) }

pub const ROW_COLORS: [Color; 8] = [Color::rgb(232, 62, 58), Color::rgb(240, 142, 30), Color::rgb(46, 164, 79), Color::rgb(40, 124, 232), Color::rgb(124, 84, 214), Color::rgb(24, 164, 150), Color::rgb(226, 72, 140), Color::rgb(92, 104, 124)];

pub fn fade(c: Color, alpha: i32) -> Color { Color::from_argb(alpha.clamp(0, 255), c.r as i32, c.g as i32, c.b as i32) }
pub fn mix(a: Color, b: Color, k: f32) -> Color {
    let f = |x: u8, y: u8| (x as f32 + (y as i32 - x as i32) as f32 * k) as i32;
    Color::rgb(f(a.r, b.r), f(a.g, b.g), f(a.b, b.b))
}
pub fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let r = 1.0 - x;
    1.0 - r * r * r
}
pub fn back_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0) - 1.0;
    1.0 + 2.4 * x * x * x + 1.4 * x * x
}
pub fn soft(k: f32) -> f32 {
    if k <= 0.0 || k >= 1.0 { return 0.0; }
    let v = (k as f64 * std::f64::consts::PI).sin() as f32;
    v * v
}
pub fn smooth(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

pub fn round(r: RectangleF, rad: f32) -> GraphicsPath {
    let mut path = GraphicsPath::new();
    let d = (rad * 2.0).min(r.width.min(r.height));
    if d <= 0.0 {
        path.add_rectangle(r);
        return path;
    }
    path.add_arc(RectangleF::new(r.x, r.y, d, d), 180.0, 90.0);
    path.add_arc(RectangleF::new(r.right() - d, r.y, d, d), 270.0, 90.0);
    path.add_arc(RectangleF::new(r.right() - d, r.bottom() - d, d, d), 0.0, 90.0);
    path.add_arc(RectangleF::new(r.x, r.bottom() - d, d, d), 90.0, 90.0);
    path.close_figure();
    path
}

pub fn lin(r: RectangleF, a: Color, b: Color, angle: f32) -> Brush { Brush::Linear(LinearGradient::new(r, a, b, angle)) }
pub fn solid(c: Color) -> Brush { Brush::Solid(c) }

pub fn card(g: &mut Graphics, r: RectangleF, rad: f32, top: Color, bottom: Color, edge: Color, edge_w: f32) {
    let path = round(r, rad);
    g.fill_path(&lin(r, top, bottom, 90.0), &path);
    if edge_w > 0.0 { g.draw_path(&Pen::new(edge, edge_w), &path); }
}

pub fn gloss_tile(g: &mut Graphics, r: RectangleF, rad: f32, c: Color) {
    card(g, r, rad, mix(c, Color::WHITE, 0.18), mix(c, Color::BLACK, 0.14), Color::EMPTY, 0.0);
    let path = round(r, rad);
    let st = g.save();
    g.set_clip_path(&path);
    let shine = RectangleF::new(r.x - r.width * 0.2, r.y - r.height * 0.62, r.width * 1.4, r.height * 1.08);
    g.fill_ellipse(&lin(shine, Color::from_argb(80, 255, 255, 255), Color::from_argb(6, 255, 255, 255), 90.0), shine);
    g.restore(st);
    g.draw_path(&Pen::new(mix(c, Color::BLACK, 0.35), (r.width * 0.018).max(1.0)), &path);
}

pub fn ball(g: &mut Graphics, c: PointF, rad: f32, ink: &Pen) {
    let bx = RectangleF::new(c.x - rad, c.y - rad, rad * 2.0, rad * 2.0);
    g.fill_ellipse(&solid(Color::WHITE), bx);
    g.fill_pie(&lin(bx, Color::rgb(250, 84, 72), Color::rgb(196, 28, 30), 90.0), bx.x, bx.y, bx.width, bx.height, 180.0, 180.0);
    g.fill_pie(&solid(Color::from_argb(70, 150, 158, 178)), bx.x + rad * 0.12, bx.y, bx.width - rad * 0.24, bx.height, 25.0, 130.0);
    g.draw_line_f(&Pen::new(INK, (rad * 0.2).max(1.2)), c.x - rad, c.y, c.x + rad, c.y);
    g.draw_ellipse(ink, bx);
    let br = rad * 0.32;
    let btn = RectangleF::new(c.x - br, c.y - br, br * 2.0, br * 2.0);
    g.fill_ellipse(&solid(Color::WHITE), btn);
    g.draw_ellipse(ink, btn);
    g.fill_ellipse(&solid(Color::from_argb(150, 255, 255, 255)), RectangleF::new(c.x - rad * 0.62, c.y - rad * 0.72, rad * 0.4, rad * 0.26));
}

pub fn sparkle(g: &mut Graphics, c: PointF, r: f32) {
    let pts = [
        PointF::new(c.x, c.y - r),
        PointF::new(c.x + r * 0.25, c.y - r * 0.25),
        PointF::new(c.x + r, c.y),
        PointF::new(c.x + r * 0.25, c.y + r * 0.25),
        PointF::new(c.x, c.y + r),
        PointF::new(c.x - r * 0.25, c.y + r * 0.25),
        PointF::new(c.x - r, c.y),
        PointF::new(c.x - r * 0.25, c.y - r * 0.25),
    ];
    g.fill_polygon(&solid(Color::WHITE), &pts);
}

const RAINBOW: [Color; 6] = [Color::rgb(255, 0, 0), Color::rgb(255, 136, 0), Color::rgb(255, 255, 0), Color::rgb(0, 204, 0), Color::rgb(0, 102, 255), Color::rgb(136, 0, 255)];
fn rainbow_at(t: f32) -> Color {
    let k = (t / 2.0 % 1.0) * RAINBOW.len() as f32;
    let i = k as usize % RAINBOW.len();
    mix(RAINBOW[i], RAINBOW[(i + 1) % RAINBOW.len()], k - (k as i32) as f32)
}

/// dark: the page's own colors for a dark background.
pub fn spinning_ball(g: &mut Graphics, cx: f32, cy: f32, size: f32, t: f32, dark: bool) {
    let st = g.save();
    g.smoothing_mode = SmoothingMode::AntiAlias;
    g.translate_transform(cx, cy);
    g.rotate_transform(t * 180.0 % 360.0);
    g.scale_transform(size / 64.0, size / 64.0);
    let whole = RectangleF::new(-30.0, -30.0, 60.0, 60.0);
    g.fill_pie(&solid(fade(rainbow_at(t), if dark { 217 } else { 230 })), whole.x, whole.y, whole.width, whole.height, 180.0, 180.0);
    g.fill_pie(&solid(if dark { Color::from_argb(209, 255, 255, 255) } else { Color::WHITE }), whole.x, whole.y, whole.width, whole.height, 0.0, 180.0);
    g.draw_line_f(&Pen::new(if dark { Color::from_argb(51, 255, 255, 255) } else { fade(LINE, 200) }, 3.0), -30.0, 0.0, 30.0, 0.0);
    g.draw_ellipse(&Pen::new(if dark { Color::from_argb(38, 255, 255, 255) } else { fade(LINE, 220) }, 2.0), whole);
    let b = RectangleF::new(-8.0, -8.0, 16.0, 16.0);
    g.fill_ellipse(&solid(if dark { Color::from_argb(153, 0, 0, 0) } else { Color::WHITE }), b);
    g.draw_ellipse(&Pen::new(if dark { Color::from_argb(51, 255, 255, 255) } else { fade(LINE, 200) }, 2.0), b);
    if dark {
        g.fill_ellipse(&solid(Color::from_argb(38, 255, 255, 255)), RectangleF::new(-4.0, -4.0, 8.0, 8.0));
    } else {
        g.draw_ellipse(&Pen::new(fade(LINE, 70), 1.5), RectangleF::new(-4.0, -4.0, 8.0, 8.0));
    }
    g.restore(st);
}

fn round_pen(c: Color, w: f32) -> Pen { Pen::new(c, w).round() }

/// The home screen's app icons. u: seconds since highlighted (below 0: still). emu_k: the install animation, or -1.
pub fn app_art(g: &mut Graphics, app: usize, r: RectangleF, tile: Color, u: f32, emu_k: f32, shake: f32) {
    let s = r.width;
    let lw = (s * 0.045).max(1.5);
    let p = |x: f32, y: f32| PointF::new(r.x + x * s, r.y + y * s);
    let rr = |x: f32, y: f32, w2: f32, h2: f32| RectangleF::new(r.x + x * s, r.y + y * s, w2 * s, h2 * s);
    let turn = |g: &mut Graphics, c: PointF, deg: f32| {
        g.translate_transform(c.x, c.y);
        g.rotate_transform(deg);
        g.translate_transform(-c.x, -c.y);
    };
    let wave = if u < 0.0 { 0.0 } else { (u as f64 * std::f64::consts::PI * 2.0 / 1.4).sin() as f32 };
    let st = g.save();
    g.smoothing_mode = SmoothingMode::AntiAlias;
    let ink = Pen::new(INK, lw).round().round_join();
    match APP_NAMES[app] {
        "Games" => {
            let a = if u < 0.0 { 0.0 } else { shake };
            if a != 0.0 { turn(g, p(0.5, 0.83), a); }
            ball(g, p(0.5, 0.5), s * 0.33, &ink);
            if a.abs() > 0.5 { g.fill_ellipse(&solid(Color::from_argb((a.abs() * 10.0).min(200.0) as i32, 255, 70, 60)), rr(0.5 - 0.075, 0.5 - 0.075, 0.15, 0.15)); }
        }
        "Collection" => {
            let phone = rr(0.24, 0.08, 0.52, 0.84);
            let path = round(phone, s * 0.1);
            g.fill_path(&lin(phone, Color::rgb(246, 82, 72), Color::rgb(184, 28, 36), 90.0), &path);
            g.draw_path(&ink, &path);
            let ring = rr(0.28, 0.12, 0.19, 0.19);
            g.fill_ellipse(&solid(Color::WHITE), ring);
            g.draw_ellipse(&ink, ring);
            let lens = rr(0.31, 0.15, 0.13, 0.13);
            g.fill_ellipse(&lin(lens, Color::rgb(130, 210, 255), Color::rgb(24, 96, 206), 60.0), lens);
            if u >= 0.0 { g.fill_ellipse(&solid(Color::from_argb((50.0 + 50.0 * wave) as i32, 255, 255, 255)), lens); }
            g.fill_ellipse(&solid(Color::WHITE), rr(0.33, 0.17, 0.04, 0.04));
            let lights = [Color::rgb(255, 90, 80), Color::rgb(255, 214, 64), Color::rgb(90, 210, 110)];
            let lit = if u < 0.0 { -1 } else { (u * 3.0) as i32 % 3 };
            for k in 0..3 {
                let dot = rr(0.51 + k as f32 * 0.065, 0.19, 0.048, 0.048);
                g.fill_ellipse(&solid(if u < 0.0 || k == lit { lights[k as usize] } else { mix(lights[k as usize], Color::BLACK, 0.4) }), dot);
                g.draw_ellipse(&Pen::new(INK, lw * 0.5), dot);
            }
            let screen = rr(0.29, 0.35, 0.42, 0.46);
            let path = round(screen, s * 0.035);
            g.fill_path(&solid(Color::rgb(240, 244, 248)), &path);
            let clip = g.save();
            g.set_clip_path(&path);
            let row = 0.16f32;
            let mut scroll = 0.0;
            if u >= 0.0 {
                let cyc = u % 2.2;
                scroll = row * smooth((cyc - 1.4) / 0.6);
            }
            let first = if u < 0.0 { 0 } else { (u / 2.2) as i32 };
            for k in 0..4 {
                let y = 0.365 + k as f32 * row - scroll;
                if y > 0.81 { break; }
                let entry = rr(0.31, y, 0.38, row - 0.025);
                g.fill_path(&solid(Color::WHITE), &round(entry, s * 0.02));
                ball(g, p(0.375, y + (row - 0.025) / 2.0), s * 0.04, &Pen::new(INK, lw * 0.6));
                let w1 = 0.16 + 0.05 * (((first + k) * 7) % 3) as f32 / 2.0;
                let pen = round_pen(Color::rgb(150, 156, 170), lw * 0.7);
                g.draw_line(&pen, p(0.44, y + 0.045), p(0.44 + w1, y + 0.045));
                g.draw_line(&pen, p(0.44, y + 0.09), p(0.44 + 0.1, y + 0.09));
            }
            g.restore(clip);
            g.draw_path(&ink, &path);
            g.draw_line(&round_pen(Color::from_argb(150, 255, 255, 255), lw * 0.9), p(0.44, 0.86), p(0.56, 0.86));
        }
        "Mods" => {
            let disc = rr(0.15, 0.15, 0.7, 0.7);
            g.fill_ellipse(&lin(disc, Color::rgb(250, 251, 255), Color::rgb(160, 168, 186), 45.0), disc);
            g.draw_arc(&Pen::new(Color::from_argb(150, 255, 255, 255), lw * 1.2), rr(0.22, 0.22, 0.56, 0.56), 200.0, 60.0);
            let label = rr(0.31, 0.31, 0.38, 0.38);
            let spin = g.save();
            if u >= 0.0 { turn(g, p(0.5, 0.5), u * 300.0); }
            g.fill_ellipse(&lin(label, mix(tile, Color::WHITE, 0.35), mix(tile, Color::BLACK, 0.15), 90.0), label);
            if u >= 0.0 { g.draw_arc(&round_pen(Color::from_argb(170, 255, 255, 255), lw * 0.9), rr(0.36, 0.36, 0.28, 0.28), 300.0, 70.0); }
            g.restore(spin);
            g.draw_ellipse(&ink, label);
            let hole = rr(0.44, 0.44, 0.12, 0.12);
            g.fill_ellipse(&solid(mix(tile, Color::BLACK, 0.45)), hole);
            g.draw_ellipse(&ink, hole);
            g.draw_ellipse(&ink, disc);
            let k = if u < 0.0 { 1.0 } else { 0.75 + 0.4 * ((u as f64 * std::f64::consts::PI / 0.9).sin() as f32).abs() };
            sparkle(g, p(0.8, 0.2), s * 0.09 * k);
        }
        "ROMs" => {
            if u >= 0.0 { g.translate_transform(0.0, -s * 0.045 * (0.5 - 0.5 * (u as f64 * std::f64::consts::PI * 2.0 / 1.1).cos() as f32)); }
            let mut body = GraphicsPath::new();
            body.add_lines(&[p(0.24, 0.2), p(0.6, 0.2), p(0.68, 0.28), p(0.76, 0.28), p(0.76, 0.82), p(0.24, 0.82)]);
            body.close_figure();
            g.fill_path(&lin(rr(0.24, 0.2, 0.52, 0.62), Color::rgb(232, 234, 240), Color::rgb(158, 164, 178), 90.0), &body);
            let label = rr(0.31, 0.33, 0.38, 0.3);
            let path = round(label, s * 0.025);
            g.fill_path(&lin(label, mix(tile, Color::WHITE, 0.3), mix(tile, Color::BLACK, 0.12), 90.0), &path);
            g.draw_path(&ink, &path);
            ball(g, p(0.5, 0.48), s * 0.08, &Pen::new(INK, lw * 0.7));
            for k in 0..5 { g.draw_line(&ink, p(0.33 + k as f32 * 0.085, 0.72), p(0.33 + k as f32 * 0.085, 0.78)); }
            g.draw_path(&ink, &body);
        }
        "Emu" => {
            let ik = emu_k;
            let shell = Color::rgb(246, 247, 252);
            let lid = rr(0.2, 0.13, 0.6, 0.37);
            let body_r = rr(0.2, 0.52, 0.6, 0.35);
            let path = round(body_r, s * 0.06);
            g.fill_path(&lin(body_r, shell, Color::rgb(178, 184, 198), 90.0), &path);
            g.draw_path(&ink, &path);
            let screen = rr(0.37, 0.58, 0.26, 0.2);
            let path = round(screen, s * 0.02);
            g.fill_path(&solid(Color::rgb(206, 240, 228)), &path);
            g.draw_path(&ink, &path);
            let pad = round_pen(INK, lw * 1.1);
            g.draw_line(&pad, p(0.255, 0.68), p(0.325, 0.68));
            g.draw_line(&pad, p(0.29, 0.645), p(0.29, 0.715));
            let b = solid(mix(tile, Color::WHITE, 0.15));
            g.fill_ellipse(&b, rr(0.675, 0.62, 0.05, 0.05));
            g.fill_ellipse(&b, rr(0.715, 0.67, 0.05, 0.05));
            g.draw_line(&ink, p(0.24, 0.51), p(0.76, 0.51));
            let (mut open, mut power) = (1.0f32, 1.0f32);
            let mut k = if u < 0.0 { 0.0 } else { soft((u - 0.15) % 2.6 / 2.0) };
            if ik >= 0.0 {
                open = smooth(ik / 0.42) + 0.06 * soft((ik - 0.36) / 0.22);
                power = smooth((ik - 0.3) / 0.3);
                k = soft((ik - 0.5) / 0.5);
            }
            let lid_st = g.save();
            let hinge = p(0.5, 0.51);
            g.translate_transform(hinge.x, hinge.y);
            g.scale_transform(1.0, open.max(0.04));
            g.translate_transform(-hinge.x, -hinge.y);
            let path = round(lid, s * 0.06);
            g.fill_path(&lin(lid, shell, Color::rgb(178, 184, 198), 90.0), &path);
            g.draw_path(&ink, &path);
            let top = rr(0.27, 0.19, 0.46, 0.25);
            let path = round(top, s * 0.025);
            g.fill_path(&lin(top, mix(Color::rgb(40, 44, 56), Color::rgb(70, 200, 230), power), mix(Color::rgb(22, 24, 32), Color::rgb(24, 70, 130), power), 90.0), &path);
            let flash = if ik >= 0.0 { 0.8 * soft((ik - 0.3) / 0.35) } else { 0.0 };
            if k > 0.0 || flash > 0.0 { g.fill_path(&solid(Color::from_argb(((90.0 * k / 255.0 + flash).min(1.0) * 255.0) as i32, 255, 255, 255)), &path); }
            g.draw_path(&ink, &path);
            let br = s * (0.075 + 0.055 * k);
            let lift = s * 0.09 * k;
            let c0 = p(0.5, 0.315);
            if power > 0.5 {
                if k > 0.02 { g.fill_ellipse(&solid(Color::from_argb((80.0 * k) as i32, 0, 0, 0)), RectangleF::new(c0.x - br * 0.8, c0.y + br * 0.55, br * 1.6, br * 0.45)); }
                ball(g, PointF::new(c0.x, c0.y - lift), br, &Pen::new(INK, lw * (0.7 + 0.25 * k)));
            }
            g.restore(lid_st);
            if ik >= 0.55 {
                let sp = soft((ik - 0.55) / 0.45);
                sparkle(g, p(0.8, 0.17), s * 0.11 * sp);
                sparkle(g, p(0.17, 0.36), s * 0.07 * soft((ik - 0.65) / 0.35));
            }
        }
        "Move" => {
            let hop = |ph: f32| if u < 0.0 { 0.0 } else { -0.07 * (((u / 1.2 + ph) as f64 * std::f64::consts::PI * 2.0).sin() as f32).max(0.0) };
            ball(g, p(0.3, 0.62 + hop(0.0)), s * 0.16, &ink);
            ball(g, p(0.7, 0.62 + hop(0.5)), s * 0.16, &ink);
            let mut arc = GraphicsPath::new();
            arc.add_bezier(p(0.3, 0.38), p(0.36, 0.12), p(0.64, 0.12), p(0.7, 0.36));
            let head = [p(0.61, 0.3), p(0.705, 0.4), p(0.77, 0.28)];
            let back = Pen::new(INK, lw * 3.0).round().round_join();
            g.draw_path(&back, &arc);
            g.draw_lines(&back, &head);
            let front = Pen::new(Color::WHITE, lw * 1.4).round().round_join();
            g.draw_path(&front, &arc);
            g.draw_lines(&front, &head);
        }
        "Settings" => {
            let mut gear = GraphicsPath::new();
            let mut pts = Vec::new();
            let c0 = p(0.5, 0.5);
            for k in 0..8 {
                let a = k as f64 * std::f64::consts::PI / 4.0;
                for off in [-0.24f64, -0.13, 0.13, 0.24] {
                    let ang = a + off * 0.95;
                    let rr0 = if off.abs() < 0.2 { 0.35f32 } else { 0.27 };
                    pts.push(PointF::new(c0.x + ang.cos() as f32 * rr0 * s, c0.y + ang.sin() as f32 * rr0 * s));
                }
            }
            gear.add_polygon(&pts);
            if u >= 0.0 { turn(g, c0, u * 45.0); }
            g.fill_path(&lin(rr(0.15, 0.15, 0.7, 0.7), Color::rgb(252, 252, 255), Color::rgb(184, 190, 204), 90.0), &gear);
            g.draw_path(&ink, &gear);
            let hole = rr(0.39, 0.39, 0.22, 0.22);
            g.fill_ellipse(&solid(mix(tile, Color::BLACK, 0.3)), hole);
            g.draw_ellipse(&ink, hole);
        }
        "Exit" => {
            let ring = rr(0.24, 0.26, 0.52, 0.52);
            let back = round_pen(INK, lw * 3.2);
            g.draw_arc(&back, ring, -60.0, 300.0);
            g.draw_line(&back, p(0.5, 0.17), p(0.5, 0.47));
            let front = round_pen(if u < 0.0 { Color::rgb(255, 118, 110) } else { mix(Color::rgb(255, 118, 110), Color::WHITE, 0.25 + 0.25 * wave) }, lw * 1.5);
            g.draw_arc(&front, ring, -60.0, 300.0);
            g.draw_line(&front, p(0.5, 0.17), p(0.5, 0.47));
        }
        _ => {}
    }
    g.restore(st);
}

/// The list rows' common icons: white stickers with a dark outline. False: draw the glyph instead.
pub fn row_art(g: &mut Graphics, icon: char, r: RectangleF, tint: Color) -> bool {
    if browse_art(g, icon, r) { return true; }
    match icon {
        '\u{E713}' => { app_art(g, app("Settings"), r, tint, -1.0, -1.0, 0.0); return true; }
        '\u{E8F1}' => { app_art(g, app("Collection"), r, tint, -1.0, -1.0, 0.0); return true; }
        '\u{EA86}' => { app_art(g, app("Mods"), r, tint, -1.0, -1.0, 0.0); return true; }
        '\u{E7F4}' => { app_art(g, app("Emu"), r, tint, -1.0, -1.0, 0.0); return true; }
        _ => {}
    }
    let s = r.width;
    let p = |x: f32, y: f32| PointF::new(r.x + x * s, r.y + y * s);
    let mut lines: Vec<Vec<PointF>> = Vec::new();
    let mut fill: Option<GraphicsPath> = None;
    let mut fill_color = Color::WHITE;
    let mut extra: Option<Box<dyn Fn(&mut Graphics)>> = None;
    let poly = |pts: &[PointF]| {
        let mut f = GraphicsPath::new();
        f.add_polygon(pts);
        f
    };
    match icon {
        '\u{E768}' => fill = Some(poly(&[p(0.38, 0.26), p(0.76, 0.5), p(0.38, 0.74)])),
        '\u{E8D2}' => {
            let mut f = GraphicsPath::new();
            f.add_lines(&[p(0.22, 0.26), p(0.78, 0.26), p(0.78, 0.62), p(0.46, 0.62), p(0.32, 0.76), p(0.34, 0.62), p(0.22, 0.62)]);
            f.close_figure();
            fill = Some(f);
            let (a1, a2, b1, b2) = (p(0.32, 0.38), p(0.68, 0.38), p(0.32, 0.5), p(0.56, 0.5));
            extra = Some(Box::new(move |g: &mut Graphics| {
                let pen = round_pen(INK, (s * 0.05).max(1.0));
                g.draw_line(&pen, a1, a2);
                g.draw_line(&pen, b1, b2);
            }));
        }
        '\u{E765}' => {
            let mut f = GraphicsPath::new();
            f.add_rectangle(RectangleF::new(r.x + s * 0.18, r.y + s * 0.32, s * 0.64, s * 0.38));
            fill = Some(f);
            extra = Some(Box::new(move |g: &mut Graphics| {
                let b = solid(INK);
                for row in 0..2 {
                    for k in 0..4 { g.fill_rectangle(&b, RectangleF::new(r.x + s * (0.26 + k as f32 * 0.13), r.y + s * (0.39 + row as f32 * 0.1), s * 0.07, s * 0.06)); }
                }
                g.fill_rectangle(&b, RectangleF::new(r.x + s * 0.32, r.y + s * 0.6, s * 0.36, s * 0.05));
            }));
        }
        '\u{E8B9}' | '\u{E722}' => {
            let mut f = GraphicsPath::new();
            f.add_lines(&[p(0.2, 0.36), p(0.36, 0.36), p(0.41, 0.28), p(0.59, 0.28), p(0.64, 0.36), p(0.8, 0.36), p(0.8, 0.74), p(0.2, 0.74)]);
            f.close_figure();
            fill = Some(f);
            extra = Some(Box::new(move |g: &mut Graphics| {
                let lens = RectangleF::new(r.x + s * 0.37, r.y + s * 0.42, s * 0.26, s * 0.26);
                g.fill_ellipse(&solid(Color::rgb(70, 140, 230)), lens);
                g.draw_ellipse(&Pen::new(INK, (s * 0.06).max(1.2)), lens);
                g.fill_ellipse(&solid(Color::WHITE), RectangleF::new(lens.x + lens.width * 0.2, lens.y + lens.height * 0.2, lens.width * 0.28, lens.height * 0.28));
            }));
        }
        '\u{E945}' => {
            fill = Some(poly(&[p(0.56, 0.18), p(0.28, 0.56), p(0.47, 0.56), p(0.4, 0.82), p(0.72, 0.42), p(0.53, 0.42)]));
            fill_color = Color::rgb(255, 214, 52);
        }
        '\u{E711}' => {
            lines.push(vec![p(0.33, 0.33), p(0.67, 0.67)]);
            lines.push(vec![p(0.67, 0.33), p(0.33, 0.67)]);
        }
        '\u{E73E}' => lines.push(vec![p(0.27, 0.52), p(0.43, 0.68), p(0.73, 0.34)]),
        '\u{E710}' => {
            lines.push(vec![p(0.5, 0.27), p(0.5, 0.73)]);
            lines.push(vec![p(0.27, 0.5), p(0.73, 0.5)]);
        }
        '\u{E896}' | '\u{E895}' => {
            lines.push(vec![p(0.5, 0.24), p(0.5, 0.56)]);
            lines.push(vec![p(0.37, 0.45), p(0.5, 0.58), p(0.63, 0.45)]);
            lines.push(vec![p(0.27, 0.62), p(0.27, 0.74), p(0.73, 0.74), p(0.73, 0.62)]);
        }
        '\u{E898}' => {
            lines.push(vec![p(0.5, 0.58), p(0.5, 0.26)]);
            lines.push(vec![p(0.37, 0.37), p(0.5, 0.24), p(0.63, 0.37)]);
            lines.push(vec![p(0.27, 0.62), p(0.27, 0.74), p(0.73, 0.74), p(0.73, 0.62)]);
        }
        '\u{E8AB}' => {
            lines.push(vec![p(0.27, 0.39), p(0.71, 0.39)]);
            lines.push(vec![p(0.6, 0.29), p(0.72, 0.39), p(0.6, 0.49)]);
            lines.push(vec![p(0.73, 0.61), p(0.29, 0.61)]);
            lines.push(vec![p(0.4, 0.51), p(0.28, 0.61), p(0.4, 0.71)]);
        }
        '\u{E72C}' => {
            let mut arc = Vec::new();
            for k in 0..=10 {
                let a = (-20.0 + k as f64 * 25.0) * std::f64::consts::PI / 180.0;
                arc.push(p(0.5 + 0.21 * a.cos() as f32, 0.5 + 0.21 * a.sin() as f32));
            }
            lines.push(arc);
            let ea = (-20.0f64 + 250.0) * std::f64::consts::PI / 180.0;
            let (tx, ty) = (-ea.sin(), ea.cos());
            let tip = p(0.5 + 0.21 * ea.cos() as f32 + 0.13 * tx as f32, 0.5 + 0.21 * ea.sin() as f32 + 0.13 * ty as f32);
            let bl = p(0.5 + 0.33 * ea.cos() as f32, 0.5 + 0.33 * ea.sin() as f32);
            let br = p(0.5 + 0.09 * ea.cos() as f32, 0.5 + 0.09 * ea.sin() as f32);
            extra = Some(Box::new(move |g: &mut Graphics| {
                let head = [tip, bl, br];
                g.fill_polygon(&solid(Color::WHITE), &head);
                g.draw_polygon(&Pen::new(INK, (s * 0.06).max(1.2)).round_join(), &head);
            }));
        }
        '\u{E946}' => {
            lines.push(vec![p(0.5, 0.46), p(0.5, 0.72)]);
            let c = p(0.5, 0.3);
            extra = Some(Box::new(move |g: &mut Graphics| {
                let d = s * 0.13;
                g.fill_ellipse(&solid(INK), RectangleF::new(c.x - d * 0.8, c.y - d * 0.8, d * 1.6, d * 1.6));
                g.fill_ellipse(&solid(Color::WHITE), RectangleF::new(c.x - d / 2.0, c.y - d / 2.0, d, d));
            }));
        }
        '\u{E70F}' => {
            fill = Some(poly(&[p(0.62, 0.22), p(0.78, 0.38), p(0.42, 0.74), p(0.24, 0.78), p(0.28, 0.6)]));
            fill_color = Color::rgb(255, 206, 72);
            lines.push(vec![p(0.54, 0.3), p(0.7, 0.46)]);
        }
        '\u{E8B7}' | '\u{E8E5}' => {
            fill = Some(poly(&[p(0.22, 0.3), p(0.42, 0.3), p(0.48, 0.37), p(0.78, 0.37), p(0.78, 0.73), p(0.22, 0.73)]));
            fill_color = Color::rgb(255, 210, 92);
        }
        '\u{E7BA}' => {
            fill = Some(poly(&[p(0.5, 0.2), p(0.8, 0.76), p(0.2, 0.76)]));
            fill_color = Color::rgb(255, 214, 60);
            let (a, b, c) = (p(0.5, 0.4), p(0.5, 0.57), p(0.5, 0.67));
            extra = Some(Box::new(move |g: &mut Graphics| {
                g.draw_line(&round_pen(INK, (s * 0.08).max(1.5)), a, b);
                let d = s * 0.08;
                g.fill_ellipse(&solid(INK), RectangleF::new(c.x - d / 2.0, c.y - d / 2.0, d, d));
            }));
        }
        '\u{E74D}' => {
            fill = Some(poly(&[p(0.3, 0.34), p(0.7, 0.34), p(0.65, 0.76), p(0.35, 0.76)]));
            lines.push(vec![p(0.24, 0.31), p(0.76, 0.31)]);
            lines.push(vec![p(0.42, 0.24), p(0.58, 0.24)]);
            let (a1, a2, b1, b2) = (p(0.45, 0.43), p(0.45, 0.67), p(0.55, 0.43), p(0.55, 0.67));
            extra = Some(Box::new(move |g: &mut Graphics| {
                let pen = Pen::new(INK, (s * 0.045).max(1.0));
                g.draw_line(&pen, a1, a2);
                g.draw_line(&pen, b1, b2);
            }));
        }
        '\u{E7FC}' => {
            let mut f = GraphicsPath::new();
            f.add_arc(RectangleF::new(r.x + s * 0.16, r.y + s * 0.34, s * 0.3, s * 0.36), 90.0, 180.0);
            f.add_arc(RectangleF::new(r.x + s * 0.54, r.y + s * 0.34, s * 0.3, s * 0.36), 270.0, 180.0);
            f.close_figure();
            fill = Some(f);
            let (a1, a2, b1, b2) = (p(0.25, 0.52), p(0.39, 0.52), p(0.32, 0.45), p(0.32, 0.59));
            extra = Some(Box::new(move |g: &mut Graphics| {
                let pen = Pen::new(INK, (s * 0.06).max(1.2));
                g.draw_line(&pen, a1, a2);
                g.draw_line(&pen, b1, b2);
                let d = s * 0.08;
                g.fill_ellipse(&solid(Color::rgb(236, 64, 60)), RectangleF::new(r.x + s * 0.64 - d / 2.0, r.y + s * 0.47 - d / 2.0, d, d));
                g.fill_ellipse(&solid(Color::rgb(52, 120, 232)), RectangleF::new(r.x + s * 0.72 - d / 2.0, r.y + s * 0.56 - d / 2.0, d, d));
            }));
        }
        '\u{E81C}' => {
            let mut f = GraphicsPath::new();
            f.add_rectangle(RectangleF::new(r.x + s * 0.28, r.y + s * 0.22, s * 0.44, s * 0.56));
            fill = Some(f);
            lines.push(vec![p(0.36, 0.22), p(0.36, 0.78)]);
            let pts = [p(0.55, 0.22), p(0.65, 0.22), p(0.65, 0.46), p(0.6, 0.41), p(0.55, 0.46)];
            extra = Some(Box::new(move |g: &mut Graphics| g.fill_polygon(&solid(Color::rgb(236, 64, 60)), &pts)));
        }
        _ => return false,
    }
    let st = g.save();
    g.smoothing_mode = SmoothingMode::AntiAlias;
    let back = (s * 0.17).max(3.0);
    let front = (s * 0.09).max(1.5);
    let pen0 = |w: f32, c: Color| Pen::new(c, w).round().round_join();
    if let Some(f) = &fill {
        g.fill_path(&solid(fill_color), f);
        g.draw_path(&pen0((s * 0.06).max(1.2), INK), f);
    }
    if !lines.is_empty() {
        let pen = pen0(back, INK);
        for l in &lines { g.draw_lines(&pen, l); }
        let pen = pen0(front, Color::WHITE);
        for l in &lines { g.draw_lines(&pen, l); }
    }
    if let Some(e) = extra { e(g); }
    g.restore(st);
    true
}

/// A game's cartridge, in its version's colors (the row icon of a game).
pub fn cart(g: &mut Graphics, x: f32, y: f32, len: f32, th: &crate::model::Theme) {
    let st = g.save();
    g.translate_transform(x, y);
    g.scale_transform(len / 100.0, len / 100.0);
    let rect = RectangleF::new(10.0, 0.0, 80.0, 94.0);
    g.fill_path(&solid(LINE), &round(rect, 7.0));
    g.fill_path(&lin(rect, if th.plain() { th.main } else { th.light }, th.deep, 90.0), &round(RectangleF::new(13.0, 3.0, 74.0, 88.0), 5.0));
    g.fill_path(&solid(CREAM), &round(RectangleF::new(22.0, 13.0, 56.0, 44.0), 3.0));
    g.fill_ellipse(&solid(if th.plain() { th.main } else { th.deep }), RectangleF::new(39.0, 24.0, 22.0, 22.0));
    let slot = solid(fade(LINE, 200));
    for i in 0..7 { g.fill_rectangle(&slot, RectangleF::new(23.0 + i as f32 * 8.0, 68.0, 4.4, 16.0)); }
    g.restore(st);
}

/// Pixel art stays crisp when it's enlarged (NearestNeighbor, PixelOffsetMode.Half).
pub fn draw_sprite(g: &mut Graphics, b: &Bitmap, r: RectangleF) { g.draw_image_nearest(b, r); }

/// The browser's own little icons (IcoFolder..IcoAdd).
pub const ICO_FOLDER: char = '\u{F100}';
pub const ICO_USE: char = '\u{F101}';
pub const ICO_DESKTOP: char = '\u{F102}';
pub const ICO_DOCS: char = '\u{F103}';
pub const ICO_DOWN: char = '\u{F104}';
pub const ICO_PICS: char = '\u{F105}';
pub const ICO_DRIVE: char = '\u{F106}';
pub const ICO_USB: char = '\u{F107}';
pub const ICO_GB: char = '\u{F108}';
pub const ICO_GBC: char = '\u{F109}';
pub const ICO_GBA: char = '\u{F10A}';
pub const ICO_NDS: char = '\u{F10B}';
pub const ICO_3DS: char = '\u{F10C}';
pub const ICO_ZIP: char = '\u{F10D}';
pub const ICO_PIC: char = '\u{F10E}';
pub const ICO_ADD: char = '\u{F10F}';

/// BrowseTints.
pub fn browse_tint(icon: char) -> Option<Color> {
    Some(match icon {
        ICO_FOLDER => Color::rgb(245, 176, 48),
        ICO_USE => Color::rgb(46, 164, 79),
        ICO_DESKTOP => Color::rgb(40, 124, 232),
        ICO_DOCS => Color::rgb(72, 120, 210),
        ICO_DOWN => Color::rgb(124, 84, 214),
        ICO_PICS => Color::rgb(226, 72, 140),
        ICO_DRIVE => Color::rgb(92, 104, 124),
        ICO_USB => Color::rgb(24, 164, 150),
        ICO_GB => Color::rgb(118, 118, 140),
        ICO_GBC => Color::rgb(150, 90, 200),
        ICO_GBA => Color::rgb(64, 72, 180),
        ICO_NDS => Color::rgb(70, 72, 84),
        ICO_3DS => Color::rgb(210, 56, 60),
        ICO_ZIP => Color::rgb(240, 142, 30),
        ICO_PIC => Color::rgb(226, 72, 140),
        ICO_ADD => Color::rgb(46, 164, 79),
        _ => return None,
    })
}

/// FileIcon.
pub fn file_icon(file: &str) -> char {
    let ext = std::path::Path::new(file).extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default();
    match ext.as_str() { ".gb" => ICO_GB, ".gbc" => ICO_GBC, ".gba" => ICO_GBA, ".nds" => ICO_NDS, ".3ds" | ".cci" => ICO_3DS, ".zip" => ICO_ZIP, _ => ICO_PIC }
}

fn browse_pen(c: Color, w: f32) -> Pen {
    let mut p = Pen::new(c, w);
    p.line_join = LineJoin::Round;
    p.start_cap = LineCap::Round;
    p.end_cap = LineCap::Round;
    p
}

/// BrowseArt: drawn shapes on the row's colored tile, with a face on the folders.
pub fn browse_art(g: &mut Graphics, icon: char, r: RectangleF) -> bool {
    if !(ICO_FOLDER..=ICO_ADD).contains(&icon) { return false; }
    let s = r.width;
    let lw = (s * 0.045).max(1.2);
    let p = |x: f32, y: f32| PointF::new(r.x + x * s, r.y + y * s);
    let rr = |x: f32, y: f32, w: f32, h: f32| RectangleF::new(r.x + x * s, r.y + y * s, w * s, h * s);
    let ink = browse_pen(INK, lw);
    let shape = |g: &mut Graphics, pts: &[PointF], c: Color| { g.fill_polygon(&solid(c), pts); g.draw_polygon(&ink, pts); };
    let bx = |g: &mut Graphics, b: RectangleF, rad: f32, c: Color| { let path = round(b, rad * s); g.fill_path(&solid(c), &path); g.draw_path(&ink, &path); };
    let dot = |g: &mut Graphics, x: f32, y: f32, d: f32, c: Color| g.fill_ellipse(&solid(c), RectangleF::new(r.x + (x - d) * s, r.y + (y - d) * s, d * 2.0 * s, d * 2.0 * s));
    let face = |g: &mut Graphics, x: f32, y: f32| {
        dot(g, x - 0.09, y, 0.032, INK);
        dot(g, x + 0.09, y, 0.032, INK);
        dot(g, x - 0.08, y - 0.012, 0.01, Color::WHITE);
        dot(g, x + 0.1, y - 0.012, 0.01, Color::WHITE);
        g.draw_arc(&ink, RectangleF::new(r.x + (x - 0.05) * s, r.y + (y + 0.01) * s, 0.1 * s, 0.07 * s), 20.0, 140.0);
        dot(g, x - 0.17, y + 0.06, 0.03, Color::rgb(255, 150, 160));
        dot(g, x + 0.17, y + 0.06, 0.03, Color::rgb(255, 150, 160));
    };
    let folder = |g: &mut Graphics| {
        shape(g, &[p(0.18, 0.3), p(0.4, 0.3), p(0.46, 0.37), p(0.82, 0.37), p(0.82, 0.76), p(0.18, 0.76)], Color::rgb(255, 232, 170));
        shape(g, &[p(0.18, 0.44), p(0.82, 0.44), p(0.82, 0.76), p(0.18, 0.76)], Color::WHITE);
    };
    // A cartridge: body with a cut corner, a colored label, grip ridges.
    let cart = |g: &mut Graphics, c: RectangleF, body: Color, label: Color| {
        let cut = c.width * 0.22;
        shape(g, &[PointF::new(c.x, c.y), PointF::new(c.right() - cut, c.y), PointF::new(c.right(), c.y + cut), PointF::new(c.right(), c.bottom()), PointF::new(c.x, c.bottom())], body);
        let lab = RectangleF::new(c.x + c.width * 0.14, c.y + c.height * 0.2, c.width * 0.72, c.height * 0.42);
        g.fill_rectangle(&solid(label), lab);
        g.draw_rectangle(&ink, lab);
        dot(g, (lab.x + lab.width / 2.0 - r.x) / s, (lab.y + lab.height / 2.0 - r.y) / s, lab.width.min(lab.height) / s * 0.22, Color::WHITE);
        for k in 0..3 {
            let y = c.bottom() - c.height * (0.1 + k as f32 * 0.07);
            g.draw_line_f(&ink, c.x + c.width * 0.3, y, c.right() - c.width * 0.3, y);
        }
    };
    match icon {
        ICO_FOLDER => { folder(g); face(g, 0.5, 0.57); }
        ICO_USE => {
            folder(g);
            dot(g, 0.68, 0.66, 0.16, Color::rgb(46, 164, 79));
            g.draw_ellipse(&ink, RectangleF::new(r.x + 0.52 * s, r.y + 0.5 * s, 0.32 * s, 0.32 * s));
            g.draw_lines(&browse_pen(Color::WHITE, lw * 1.3), &[p(0.61, 0.66), p(0.66, 0.72), p(0.76, 0.6)]);
        }
        ICO_DESKTOP => {
            bx(g, rr(0.2, 0.26, 0.6, 0.38), 0.05, Color::WHITE);
            g.fill_rectangle(&solid(Color::rgb(160, 206, 250)), rr(0.26, 0.31, 0.48, 0.27));
            g.draw_line(&ink, p(0.5, 0.64), p(0.5, 0.73));
            g.draw_line(&ink, p(0.36, 0.74), p(0.64, 0.74));
            face(g, 0.5, 0.43);
        }
        ICO_DOCS => {
            shape(g, &[p(0.3, 0.2), p(0.6, 0.2), p(0.72, 0.32), p(0.72, 0.8), p(0.3, 0.8)], Color::WHITE);
            shape(g, &[p(0.6, 0.2), p(0.6, 0.32), p(0.72, 0.32)], Color::rgb(220, 228, 240));
            for k in 0..3 { g.draw_line(&ink, p(0.38, 0.5 + k as f32 * 0.08), p(0.64 - if k == 2 { 0.1 } else { 0.0 }, 0.5 + k as f32 * 0.08)); }
        }
        ICO_DOWN => {
            for pen in [browse_pen(INK, lw * 3.2), browse_pen(Color::WHITE, lw * 1.6)] {
                g.draw_line(&pen, p(0.5, 0.22), p(0.5, 0.56));
                g.draw_lines(&pen, &[p(0.36, 0.44), p(0.5, 0.58), p(0.64, 0.44)]);
                g.draw_lines(&pen, &[p(0.24, 0.62), p(0.24, 0.76), p(0.76, 0.76), p(0.76, 0.62)]);
            }
        }
        ICO_PICS | ICO_PIC => {
            bx(g, rr(0.2, 0.26, 0.6, 0.48), 0.05, Color::WHITE);
            g.fill_rectangle(&solid(Color::rgb(170, 214, 250)), rr(0.26, 0.32, 0.48, 0.36));
            g.fill_polygon(&solid(Color::rgb(96, 186, 96)), &[p(0.26, 0.68), p(0.4, 0.48), p(0.52, 0.6), p(0.6, 0.52), p(0.74, 0.68)]);
            dot(g, 0.62, 0.41, 0.05, Color::rgb(255, 210, 60));
        }
        ICO_DRIVE => {
            bx(g, rr(0.16, 0.34, 0.68, 0.34), 0.07, Color::WHITE);
            g.draw_line(&ink, p(0.24, 0.56), p(0.56, 0.56));
            dot(g, 0.7, 0.56, 0.04, Color::rgb(80, 220, 120));
            face(g, 0.42, 0.44);
        }
        ICO_USB => {
            bx(g, rr(0.36, 0.32, 0.28, 0.46), 0.06, Color::WHITE);
            bx(g, rr(0.41, 0.18, 0.18, 0.14), 0.01, Color::rgb(200, 204, 214));
            g.fill_rectangle(&solid(INK), rr(0.44, 0.22, 0.04, 0.04));
            g.fill_rectangle(&solid(INK), rr(0.52, 0.22, 0.04, 0.04));
            face(g, 0.5, 0.52);
        }
        ICO_GB => cart(g, rr(0.28, 0.2, 0.44, 0.6), Color::rgb(206, 206, 212), Color::rgb(120, 110, 190)),
        ICO_GBC => cart(g, rr(0.28, 0.2, 0.44, 0.6), Color::rgb(200, 170, 236), Color::rgb(250, 210, 80)),
        ICO_GBA => cart(g, rr(0.2, 0.3, 0.6, 0.42), Color::rgb(150, 156, 236), Color::rgb(240, 120, 120)),
        ICO_NDS => cart(g, rr(0.3, 0.26, 0.4, 0.48), Color::rgb(90, 92, 104), Color::rgb(120, 200, 250)),
        ICO_3DS => cart(g, rr(0.3, 0.26, 0.4, 0.48), Color::WHITE, Color::rgb(240, 90, 90)),
        ICO_ZIP => {
            bx(g, rr(0.24, 0.24, 0.52, 0.54), 0.08, Color::rgb(255, 226, 170));
            for k in 0..6 { let y = 0.28 + k as f32 * 0.07; g.draw_line(&ink, p(0.46, y), p(0.54, y)); }
            bx(g, rr(0.44, 0.66, 0.12, 0.08), 0.02, Color::rgb(200, 204, 214));
        }
        _ => {
            bx(g, rr(0.28, 0.2, 0.36, 0.46), 0.03, Color::rgb(220, 240, 226));
            bx(g, rr(0.22, 0.28, 0.36, 0.46), 0.03, Color::WHITE);
            dot(g, 0.66, 0.66, 0.15, Color::rgb(46, 164, 79));
            g.draw_ellipse(&ink, RectangleF::new(r.x + 0.51 * s, r.y + 0.51 * s, 0.3 * s, 0.3 * s));
            let mut pen = Pen::new(Color::WHITE, lw * 1.3);
            pen.start_cap = LineCap::Round;
            pen.end_cap = LineCap::Round;
            g.draw_line(&pen, p(0.66, 0.58), p(0.66, 0.74));
            g.draw_line(&pen, p(0.58, 0.66), p(0.74, 0.66));
        }
    }
    true
}
