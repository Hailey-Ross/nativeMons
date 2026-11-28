//! The home screen: the version's wallpaper, the Play widget (with PlayHop) and the app grid.

use crate::art::*;
use crate::launcher::{accent, HopMove, Launcher};
use crate::model::{Page, Theme};
use nm_draw::text::StringAlignment::{Center, Far, Near};
use nm_draw::text::{char_range_rects, StringFormat};
use nm_draw::*;

const TILE_COLS: i32 = 4;

pub fn background(l: &mut Launcher, g: &mut Graphics, w: i32, h: i32, m: f32, top: f32, bottom: f32, th: &Theme, t: f32, all: RectangleF) {
    let band_top = top + (bottom - top) * 0.31;
    let band_bottom = match l.pages.last() { Some(p) => apps_top(l, p, w, h, m, bottom), None => bottom };
    let town = th.game_id.as_deref().map_or(false, |id| crate::game_art::draw_own(g, id, all)
        || (!crate::game_art::wallpaper_drawn() && crate::game_art::draw_town(g, id, all, RectangleF::new(0.0, band_top, w as f32, (band_bottom - band_top).max(1.0)))));
    if !town { crate::scenes::version_fill(g, all, 0.0, th, t); }
    g.fill_rectangle(&solid(Color::from_argb(if town { 30 } else { 80 }, 0, 0, 0)), all);
}

fn app_grid(l: &Launcher, p: &Page, w: i32, h: i32, m: f32) -> (f32, f32, f32) {
    let (wf, hf) = (w as f32, h as f32);
    let full = (wf - 2.0 * m) / TILE_COLS as f32;
    let label_h = l.fonts.small.height() as f32 * 1.35;
    let row_gap = hf * 0.018;
    let app_rows = (((p.items.len().min(APP_NAMES.len()) as i32 - 1 + TILE_COLS - 1) / TILE_COLS).max(1)) as f32;
    let icon = (full * 0.68).min((hf * 0.42 - app_rows * label_h - (app_rows - 1.0) * row_gap) / app_rows);
    let cell = full.min(icon / 0.68 * 1.3);
    let x0 = m + (wf - 2.0 * m - cell * TILE_COLS as f32) / 2.0;
    (cell, icon, x0)
}

pub fn apps_top(l: &Launcher, p: &Page, w: i32, h: i32, m: f32, bottom: f32) -> f32 {
    let (_, icon, _) = app_grid(l, p, w, h, m);
    let hf = h as f32;
    let label_h = l.fonts.small.height() as f32 * 1.35;
    let row_gap = hf * 0.018;
    let app_rows = ((p.items.len().min(APP_NAMES.len()) as i32 - 1 + TILE_COLS - 1) / TILE_COLS) as f32;
    bottom - hf * 0.012 - app_rows * (icon + label_h) - (app_rows - 1.0) * row_gap
}

// wave, wave back, float, nod, peek, catch, skip, dropped ball, bops, flip
const HOP_LENGTH: [f32; 10] = [1.05, 1.05, 1.35, 0.95, 1.1, 2.0, 0.85, 0.8, 0.72, 0.75];
const HOP_WEIGHT: [i32; 10] = [3, 2, 3, 3, 2, 2, 2, 1, 1, 1];
fn next_hop(r: &mut NetRandom, start: f32, last: i32) -> HopMove {
    let mut next;
    loop {
        let mut roll = r.next(HOP_WEIGHT.iter().sum());
        next = 0usize;
        while roll >= HOP_WEIGHT[next] {
            roll -= HOP_WEIGHT[next];
            next += 1;
        }
        if next as i32 != last { break; }
    }
    let pick = r.next(4);
    let speed = 1.25 + r.next_double() as f32 * 0.5;
    let size = 0.7 + r.next_double() as f32 * 0.45;
    HopMove { start, kind: next, pick, speed, size }
}
fn hop(k: f32) -> f32 { if k <= 0.0 || k >= 1.0 { 0.0 } else { (k as f64 * std::f64::consts::PI).sin() as f32 } }

/// PlayHop: now and then the letters of a highlighted Play greet you with a soft move, picked at random.
fn play_hop(l: &mut Launcher, g: &mut Graphics, r: RectangleF, c: Color, pill_h: f32, u: f32) {
    let s = "Play";
    let ib = l.fonts.item_bold.clone();
    let sf = StringFormat { alignment: Near, line_alignment: Center, no_wrap: true, line_limit: false, ellipsis: false };
    let at: Vec<RectangleF> = (0..4).map(|i| char_range_rects(s, &ib, r, &sf, i, 1)[0]).collect();
    if l.hop_random.is_none() || u < l.hop_u {
        l.hop_random = Some(l.hooks.new_random());
        l.hops.clear();
        let first = next_hop(l.hop_random.as_mut().unwrap(), 1.2, -1);
        l.hops.push(first);
    }
    l.hop_u = u;
    loop {
        let h = l.hops[l.hops.len() - 1];
        if !(u > h.start + HOP_LENGTH[h.kind] * h.speed) { break; }
        let rnd = l.hop_random.as_mut().unwrap();
        let start = h.start + HOP_LENGTH[h.kind] * h.speed + 7.0 + rnd.next_double() as f32 * 13.0;
        let n = next_hop(rnd, start, h.kind as i32);
        l.hops.push(n);
    }
    let mv = l.hops[l.hops.len() - 1];
    let tau = (u - mv.start) / mv.speed;
    let lead = at[0].x - r.x;
    let pick = mv.pick;
    for i in 0..4usize {
        let fi = i as f32;
        let (mut lift, mut turn, mut land, mut pivot) = (0f32, 0f32, -1f32, false);
        match mv.kind {
            0 => lift = 0.15 * soft((tau - fi * 0.12) / 0.6),
            1 => lift = 0.15 * soft((tau - (3.0 - fi) * 0.12) / 0.6),
            2 => {
                let x = tau - fi * 0.04;
                let e = if x < 0.5 { smooth(x / 0.5) } else if x < 0.7 { 1.0 } else { 1.0 - smooth((x - 0.7) / 0.5) - 0.12 * soft((x - 1.05) / 0.25) };
                lift = 0.13 * e;
                turn = (fi - 1.5) * 1.6 * e.max(0.0);
            }
            3 => lift = -0.07 * soft((tau - fi * 0.08) / 0.5) + 0.03 * soft((tau - fi * 0.08 - 0.42) / 0.3),
            4 => {
                let e = soft(tau / 1.1);
                if i as i32 == pick {
                    lift = 0.1 * e;
                    turn = (if pick < 2 { -11.0 } else { 11.0 }) * e;
                } else if (i as i32 - pick).abs() == 1 { lift = 0.025 * e; }
            }
            5 => {
                let rock = (tau / 0.6) as i32;
                let q = (tau - rock as f32 * 0.6) / 0.4;
                if rock < 3 && q < 1.0 {
                    turn = (if rock % 2 == 0 { -6.0 } else { 6.0 }) * soft(q);
                    pivot = true;
                }
                land = (tau - 1.8) / 0.16;
                lift = 0.05 * hop(land);
            }
            6 => {
                land = (tau - fi * 0.1) / 0.42;
                lift = 0.13 * hop(land);
                turn = 5.0 * hop(land);
            }
            7 => {
                if i as i32 == pick {
                    if tau < 0.34 { land = tau / 0.34; lift = 0.18 * hop(land); } else if tau < 0.58 { land = (tau - 0.34) / 0.24; lift = 0.08 * hop(land); } else { land = (tau - 0.58) / 0.16; lift = 0.03 * hop(land); }
                }
            }
            8 => {
                if tau < 0.34 { land = tau / 0.34; lift = 0.09 * hop(land); } else { land = (tau - 0.4) / 0.3; lift = 0.07 * hop(land); }
            }
            _ => {
                if i as i32 == pick {
                    land = tau / 0.7;
                    lift = 0.2 * hop(land);
                    turn = 360.0 * smooth(land);
                }
            }
        }
        if mv.kind != 9 { turn *= mv.size; }
        lift *= pill_h * mv.size;
        let squash = if land > 1.0 && land < 1.3 { 0.1 * ((land - 1.0) as f64 / 0.3 * std::f64::consts::PI).sin() as f32 } else { 0.0 };
        let bx = RectangleF::new(at[i].x - lead, r.y - lift, r.right() - at[i].x + lead, r.height);
        let st = g.save();
        if turn != 0.0 || squash != 0.0 {
            let fx = at[i].x + at[i].width / 2.0;
            let o = if pivot { PointF::new((at[0].x + at[3].right()) / 2.0, at[0].bottom()) } else if turn != 0.0 { PointF::new(fx, at[i].y + at[i].height / 2.0 - lift) } else { PointF::new(fx, at[i].bottom() - lift) };
            g.translate_transform(o.x, o.y);
            g.rotate_transform(turn);
            g.scale_transform(1.0 + squash, 1.0 - squash);
            g.translate_transform(-o.x, -o.y);
        }
        crate::launcher::write(&mut l.hooks, g, &s[i..i + 1], &ib, c, bx, Near, Center, false);
        g.restore(st);
    }
}

pub fn render_main(l: &mut Launcher, g: &mut Graphics, p: &mut Page, w: i32, h: i32, m: f32, top: f32, bottom: f32, t: f32, th: &Theme) {
    let (wf, hf) = (w as f32, h as f32);
    let widget = RectangleF::new(m, top, wf - 2.0 * m, (bottom - top) * 0.31);
    let rad = wf * 0.06;
    if !p.items.is_empty() { l.hits.push((widget, 0)); }
    g.fill_path(&solid(Color::from_argb(70, 0, 0, 0)), &round(RectangleF::new(widget.x, widget.y + hf * 0.006, widget.width, widget.height), rad));
    {
        let st = g.save();
        g.set_clip_path(&round(widget, rad));
        let cur = l.ctx.as_ref().and_then(|c| c.current.map(|i| c.lib.games[i].id.clone()));
        match cur.as_deref().and_then(|id| crate::game_art::frame(id, t)) {
            Some(intro) => crate::game_art::draw_fitted(g, &intro, widget, false),
            None => {
                g.fill_rectangle(&solid(th.deep), widget);
                // Its title screen is being made right now.
                if cur.is_some() && crate::capture::making() == cur {
                    crate::art::spinning_ball(g, widget.x + widget.width / 2.0, widget.y + widget.height * 0.36, widget.height * 0.34, t, true);
                    l.spinning = true;
                }
            }
        }
        let low = RectangleF::new(widget.x, widget.y + widget.height * 0.45, widget.width, widget.height * 0.55);
        g.fill_rectangle(&lin(RectangleF::new(low.x, low.y - 1.0, low.width, low.height + 2.0), Color::from_argb(0, 0, 0, 0), Color::from_argb(205, 0, 0, 0), 90.0), low);
        g.restore(st);
    }
    if p.sel == 0 { g.draw_path(&Pen::new(Color::WHITE, (wf * 0.009).max(3.0)), &round(widget, rad)); }
    let pad = widget.width * 0.05;
    let pill_h = widget.height * 0.23;
    let pill_w = pill_h * 2.6;
    let pill = RectangleF::new(widget.right() - pad - pill_w, widget.bottom() - pad - pill_h, pill_w, pill_h);
    let on = !p.items.is_empty() && p.items[0].on;
    g.fill_path(&solid(if on { Color::WHITE } else { Color::from_argb(160, 255, 255, 255) }), &round(pill, pill_h / 2.0));
    let pink = if on { accent(th) } else { PHONE_SUB };
    let tri = pill_h * 0.19;
    let tx = pill.x + pill_w * 0.2;
    let ty = pill.y + pill_h / 2.0;
    g.fill_polygon(&solid(pink), &[PointF::new(tx, ty - tri), PointF::new(tx + tri * 1.6, ty), PointF::new(tx, ty + tri)]);
    let word = RectangleF::new(tx + tri * 2.1, pill.y, pill_w * 0.7, pill_h);
    let fb = l.fonts.item_bold.clone();
    if p.sel == 0 && on {
        let u = t - l.sel_at;
        play_hop(l, g, word, pink, pill_h, u);
    } else {
        crate::launcher::write(&mut l.hooks, g, "Play", &fb, pink, word, Near, Center, false);
    }
    let text_w = pill.x - widget.x - pad * 1.4;
    let small = l.fonts.small.clone();
    let sub_h = small.height() as f32 * 1.25;
    let (name, sub) = match l.ctx.as_ref().and_then(|c| c.current.map(|i| &c.lib.games[i])) {
        Some(g0) => (g0.title.replace("Pokemon ", ""), crate::pages::engine_short(&g0.engine(), Some(g0))),
        None => ("No games yet".to_string(), "Add a ROM folder to start".to_string()),
    };
    crate::launcher::write(&mut l.hooks, g, &sub, &small, Color::from_argb(225, 255, 255, 255), RectangleF::new(widget.x + pad, widget.bottom() - pad - sub_h, text_w, sub_h), Near, Far, false);
    let name_area = SizeF::new(text_w, l.fonts.title.height() as f32 * 1.25);
    let title = l.fonts.title.clone();
    let nf = l.fit_font(g, &name, name_area, &title, 0.65, false);
    crate::launcher::write(&mut l.hooks, g, &name, &nf, Color::WHITE, RectangleF::new(widget.x + pad, widget.bottom() - pad - sub_h - name_area.height, text_w, name_area.height), Near, Far, false);

    let (cell, icon, x0) = app_grid(l, p, w, h, m);
    let label_h = small.height() as f32 * 1.35;
    let row_gap = hf * 0.018;
    let gy = apps_top(l, p, w, h, m, bottom);
    if p.sel < 1 { l.ball_shake(-1.0); }
    let sb = l.fonts.small_bold.clone();
    let ready = nm_core::updater::ready();
    for i in 1..p.items.len().min(APP_NAMES.len()) {
        let it = p.items[i].clone();
        let k = i as i32 - 1;
        let cx = x0 + (k % TILE_COLS) as f32 * cell + cell / 2.0;
        let cy = gy + (k / TILE_COLS) as f32 * (icon + label_h + row_gap);
        let sel = i as i32 == p.sel;
        let size = if sel { icon * (1.0 + 0.07 * l.sel_ease) } else { icon };
        let r = RectangleF::new(cx - size / 2.0, cy + (icon - size) / 2.0, size, size);
        l.hits.push((RectangleF::new(cx - cell / 2.0, cy, cell, icon + label_h), i as i32));
        g.fill_path(&solid(Color::from_argb(70, 0, 0, 0)), &round(RectangleF::new(r.x, r.y + hf * 0.004, r.width, r.height), size * 0.24));
        let c = if it.on { APP_COLORS[i] } else { mix(APP_COLORS[i], Color::rgb(128, 128, 128), 0.7) };
        gloss_tile(g, r, size * 0.24, c);
        if sel { g.draw_path(&Pen::new(Color::WHITE, (wf * 0.007).max(2.5)), &round(RectangleF::new(r.x - 3.0, r.y - 3.0, r.width + 6.0, r.height + 6.0), size * 0.27)); }
        l.app_rects[i] = r;
        if l.open_run.is_some() && i == l.open_app { continue; }
        // Just installed: the Emulators tile plays its install animation (as over the emulator's page).
        let emu_k = if APP_NAMES[i] == "Emu" && t - l.emu_install_at < 2.4 { l.spinning = true; (t - l.emu_install_at) / 2.4 } else { -1.0 };
        let u = if sel && it.on && emu_k < 0.0 { t - l.sel_at } else { -1.0 };
        let shake = if APP_NAMES[i] == "Games" && u >= 0.0 { l.ball_shake(u) } else { 0.0 };
        app_art(g, i, r, c, u, emu_k, shake);
        if !it.on { g.fill_path(&solid(Color::with_alpha(110, c)), &round(r, size * 0.24)); }
        if APP_NAMES[i] == "Settings" && ready {
            let dot = size * 0.26;
            g.fill_ellipse(&solid(Color::rgb(240, 58, 50)), RectangleF::new(r.right() - dot * 0.72, r.y - dot * 0.28, dot, dot));
        }
        let label = RectangleF::new(cx - cell / 2.0, cy + icon + label_h * 0.1, cell, label_h);
        let label_w = g.measure_string(APP_NAMES[i], &sb).width + sb.height() as f32 * 0.5;
        g.fill_path(&solid(Color::from_argb(150, 0, 0, 0)), &round(RectangleF::new(cx - label_w / 2.0, label.y, label_w, label.height), label.height / 2.0));
        crate::launcher::write(&mut l.hooks, g, APP_NAMES[i], &sb, Color::WHITE, label, Center, Center, false);
    }
}
