//! The guided setup: SetupPage, RenderSetup, SetupArt, SetupFolders and RenderSetupFlow, ported from the C#.

use crate::art::*;
use crate::hooks::UiFont;
use crate::launcher::{accent, Launcher};
use crate::model::{Item, Page, Theme};
use crate::pages::{theme_of, wallpaper, Ctx};
use nm_core::{adding, config, emu3ds};
use nm_draw::text::StringAlignment::{Center, Near};
use nm_draw::*;

pub const SETUP_STEPS: i32 = 5;
pub const SETUP_SLIDE: f32 = 0.42;
const TITLES: [&str; 5] = ["Welcome to nativeMons", "Where are your games?", "Play 3DS games too?", "Pick a wallpaper", "You're all set"];

/// SetupPage(step, only).
pub fn setup_page(c: &Ctx, step: i32, only: bool) -> Page {
    let mut p = Page::new(if only { "New: wallpapers" } else { TITLES[step.clamp(0, 4) as usize] });
    p.step = step;
    p.step_only = only;
    let games = &c.lib.games;
    let have = games.iter().filter(|g| g.playable()).count();
    let folders = config::rom_folders().len() + config::rom_files().len();
    match step {
        0 => {
            p.body = Some("Your Pokemon games, running natively on your PC. Let's get them ready: it takes about a minute.".into());
            p.items.push(Item::new("Get started", "", true).run(|l| l.setup_go(1)));
            p = p.on_back(|_| {});
        }
        1 => {
            let with_rom: Vec<_> = games.iter().filter(|x| !x.playable() && (x.fulldex_rom.is_some() || x.ctr_rom.is_some() || x.found_unsupported.is_some())).collect();
            p.body = Some(if have > 0 {
                format!("Found {} of {} games. Add another folder if you keep some elsewhere.", have, games.len())
            } else if !with_rom.is_empty() {
                format!("Found ROMs for {}{}, but none can start yet. {}", with_rom.len(), if with_rom.len() == 1 { " game" } else { " games" }, adding::why_not_playable(&c.lib, with_rom[0]))
            } else if folders > 0 {
                "No games found there yet. Try the folder that holds your ROMs: every folder inside it is searched too.".into()
            } else {
                "Choose the folder with your ROMs. Every folder inside it is searched too, and file names don't matter. You can also drop ROMs here.".into()
            });
            p.items.push(Item::new(if folders > 0 { "Add another folder" } else { "Add a folder" }, "", true).run(|l| l.add_folder()));
            p.items.push(Item::new("Add a ROM file", "", true).run(|l| l.add_rom_file()));
            let next = have > 0 || !with_rom.is_empty();
            p.items.push(Item::new(if next { "Next" } else { "Skip for now" }, "", true).run(|l| l.setup_go(2)));
            p.sel = if next { 2 } else { 0 };
            p = p.on_back(|l| l.setup_go(0));
        }
        2 => {
            p.body = Some("X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon, Ultra Sun and Ultra Moon play through an emulator you pick later, from Emu. Nothing downloads now.".into());
            p.items.push(Item::new("Yes, show 3DS games", "", true).run(|l| { config::write("3ds-games.txt", "on"); l.replace_main(); l.setup_go(3); }));
            p.items.push(Item::new("Not now", "", true).run(|l| { config::write("3ds-games.txt", "off"); l.replace_main(); l.setup_go(3); }));
            p.sel = if emu3ds::enabled() { 0 } else { 1 };
            p = p.on_back(|l| l.setup_go(1));
        }
        3 => {
            p.body = Some("A scene drawn in each game's colors, or pictures from your own play. You can also give any game a picture of your own, in Settings.".into());
            // pick: the choice is kept, then the next step (or, for this step on its own, the end).
            for (text, drawn) in [("Drawn scenes", true), ("Pictures from your games", false)] {
                p.items.push(Item::new(text, "", true).run(move |l| {
                    config::write("wallpaper.txt", if drawn { "drawn" } else { "play" });
                    if only { l.setup_finish(); } else { l.setup_go(4); }
                }));
            }
            p.sel = if wallpaper::chosen() && !wallpaper::drawn() { 1 } else { 0 };
            p = if only { p.on_back(|l| l.setup_finish()) } else { p.on_back(|l| l.setup_go(2)) };
        }
        _ => {
            p.body = Some(format!("{} ready to play.\n3DS games: {}. Wallpapers: {}.\nAll of this can be changed in Settings.",
                if have == 1 { "1 game is".to_string() } else { format!("{} games are", have) }, if emu3ds::enabled() { "on" } else { "off" }, if wallpaper::drawn() { "drawn scenes" } else { "from your games" }));
            p.items.push(Item::new(if have > 0 { "Start playing" } else { "Go to the home screen" }, "", true).run(|l| l.setup_finish()));
            p = p.on_back(|l| l.setup_go(3));
        }
    }
    p
}

fn vector(a: PointF, b: PointF) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dx * dx + dy * dy).sqrt()
}

impl Launcher {
    /// RenderSetupFlow: the old step leaves one way as the new one comes in from the other.
    pub fn render_setup_flow(&mut self, g: &mut Graphics, p: &Page, w: i32, h: i32, t: f32, th: &Theme) {
        let k = ease((t - self.setup_at) / SETUP_SLIDE);
        let wf = w as f32;
        match self.setup_from.clone() {
            Some(from) if k < 1.0 => {
                let st = g.save();
                g.translate_transform(-self.setup_dir as f32 * k * wf * 0.6, 0.0);
                self.render_setup(g, &from, w, h, t, th, 99.0, false);
                g.restore(st);
                g.fill_rectangle(&solid(fade(PHONE_BG, (255.0 * k) as i32)), RectangleF::new(0.0, 0.0, wf, h as f32));
                let st = g.save();
                g.translate_transform(self.setup_dir as f32 * (1.0 - k) * wf, 0.0);
                self.render_setup(g, p, w, h, t, th, t - self.setup_at, true);
                g.restore(st);
            }
            from => {
                if from.is_some() && k >= 1.0 { self.setup_from = None; }
                self.render_setup(g, p, w, h, t, th, t - self.setup_at, true);
            }
        }
    }

    /// RenderSetup: a step's picture, title, words and buttons, each easing in a moment after the last.
    pub fn render_setup(&mut self, g: &mut Graphics, p: &Page, w: i32, h: i32, t: f32, th: &Theme, appear: f32, live: bool) {
        let (wf, hf) = (w as f32, h as f32);
        let m = wf * 0.07;
        let top = hf * 0.055;
        let acc = accent(th);
        let ez = |d: f32| ease((appear - d) / 0.4);
        let alpha = |d: f32| (255.0 * ez(d)) as i32;
        let size = wf * if p.items.len() > 2 { 0.27 } else { 0.32 };
        let cx = wf / 2.0;
        let ay = hf * if p.step_only { 0.11 } else { 0.14 };
        let pop = back_out((appear - 0.02) / 0.5);
        let k = pop.max(0.0);
        if k > 0.0 {
            let st = g.save();
            g.translate_transform(cx, ay + size / 2.0);
            g.scale_transform(0.6 + 0.4 * k, 0.6 + 0.4 * k);
            g.translate_transform(-cx, -(ay + size / 2.0));
            self.setup_art(g, p, RectangleF::new(cx - size / 2.0, ay, size, size), t, appear, th, live);
            g.restore(st);
        }
        let mut y = ay + size + hf * 0.045;
        let mut rise = (1.0 - ez(0.1)) * hf * 0.02;
        let title = self.fonts.title.clone();
        let tr = RectangleF::new(m, y + rise, wf - 2.0 * m, title.height() as f32 * 1.5);
        let hero = self.fonts.hero.clone();
        let tf = self.fit_font(g, &p.title, SizeF::new(tr.width, tr.height), &hero, 0.6, false);
        self.w(g, &p.title, &tf, fade(PHONE_TEXT, alpha(0.1)), tr, Center, Center, false);
        y += tr.height + hf * 0.012;
        let mut body = p.body.clone().unwrap_or_default();
        if p.step == 1 {
            // The count climbs as the step eases in.
            let have = self.ctx.as_ref().map_or(0, |c| c.lib.games.iter().filter(|x| x.playable()).count());
            let shown = ((have as f32 * ease((appear - 0.2) / 0.9)) as f64).round_ties_even() as usize;
            if have > 0 { body = body.replace(&format!("Found {} of", have), &format!("Found {} of", shown)); }
        }
        let bf = self.fonts.body.clone();
        let bh = g.measure_string_w(&body, &bf, (wf - 2.0 * m) as i32).height + 2.0;
        rise = (1.0 - ez(0.18)) * hf * 0.02;
        self.w(g, &body, &bf, fade(PHONE_SUB, alpha(0.18)), RectangleF::new(m, y + rise, wf - 2.0 * m, bh), Center, Near, true);
        y += bh + hf * 0.02;
        if p.step == 1 { self.setup_folders(g, w, m, y, alpha(0.26)); }
        // Buttons, stacked from the bottom: the selected one filled in the game's color.
        let b_h = hf * 0.068;
        let gap = hf * 0.014;
        let by = hf * 0.93 - p.items.len() as f32 * (b_h + gap);
        for (i, it) in p.items.iter().enumerate() {
            let d = 0.3 + i as f32 * 0.06;
            let a = ez(d);
            let shift = (1.0 - a) * hf * 0.03;
            let r = RectangleF::new(m, by + i as f32 * (b_h + gap) + shift, wf - 2.0 * m, b_h);
            let sel = i as i32 == p.sel;
            let fill = if sel { if live { self.sel_ease } else { 1.0 } } else { 0.0 };
            let path = round(r, b_h / 2.0);
            g.fill_path(&solid(fade(mix(PHONE_ROW, acc, fill), (255.0 * a) as i32)), &path);
            g.draw_path(&Pen::new(fade(mix(PHONE_LINE, acc, fill), (255.0 * a) as i32), (hf * 0.002).max(1.5)), &path);
            let f = if sel { self.fonts.item_bold.clone() } else { self.fonts.item.clone() };
            self.w(g, &it.text, &f, fade(mix(PHONE_TEXT, Color::WHITE, fill), (255.0 * a) as i32), r, Center, Center, false);
            if live && a > 0.5 { self.hits.push((r, i as i32)); }
        }
        // Where you are: a dot per step, the current one stretched into a pill.
        if !p.step_only {
            let dot = (wf * 0.014).max(5.0);
            let gap_d = dot * 1.1;
            let pill = dot * 3.2;
            let move_k = if live && self.setup_from.is_some() { ease((t - self.setup_at) / SETUP_SLIDE) } else { 1.0 };
            let from = match (&self.setup_from, live) { (Some(f), true) => f.step, _ => p.step };
            let total = (SETUP_STEPS - 1) as f32 * (dot + gap_d) + pill;
            let mut dx = (wf - total) / 2.0;
            let dy = top + hf * 0.03;
            for i in 0..SETUP_STEPS {
                let wi = dot + (pill - dot) * if i == p.step { move_k } else if i == from { 1.0 - move_k } else { 0.0 };
                let c = if i <= p.step { fade(acc, if i == p.step { 255 } else { 150 }) } else { PHONE_LINE };
                g.fill_path(&solid(c), &round(RectangleF::new(dx, dy, wi, dot), dot / 2.0));
                dx += wi + gap_d;
            }
        }
    }

    fn setup_folders(&mut self, g: &mut Graphics, w: i32, m: f32, y: f32, a: i32) {
        let name = |f: &str| f.trim_end_matches('\\').rsplit(['\\', '/']).next().unwrap_or("").to_string();
        let names: Vec<String> = config::rom_folders().iter().map(|f| name(f)).chain(config::rom_files().iter().map(|f| name(f))).collect();
        if names.is_empty() || a <= 0 { return; }
        let line = format!("{}{}", names.iter().take(3).cloned().collect::<Vec<_>>().join("   "), if names.len() > 3 { format!("   +{} more", names.len() - 3) } else { String::new() });
        let small = self.fonts.small.clone();
        let r = RectangleF::new(m, y, w as f32 - 2.0 * m, small.height() as f32 * 2.0);
        g.fill_path(&solid(fade(PHONE_ROW, a)), &round(r, r.height / 2.0));
        self.w(g, &format!("\u{E8B7}  {}", line), &small, fade(PHONE_SUB, a), r, Center, Center, false);
    }

    fn setup_art(&mut self, g: &mut Graphics, p: &Page, r: RectangleF, t: f32, appear: f32, th: &Theme, live: bool) {
        match p.step {
            0 => spinning_ball(g, r.x + r.width / 2.0, r.y + r.height / 2.0, r.width * 0.82, t, false),
            1 | 2 => {
                let a = app(if p.step == 1 { "ROMs" } else { "Emu" });
                gloss_tile(g, r, r.width * 0.24, APP_COLORS[a]);
                app_art(g, a, r, APP_COLORS[a], -1.0, -1.0, 0.0);
            }
            3 => {
                // The two choices, side by side, the selected one raised and ringed.
                let sample = if th.finish != "plain" { th.clone() } else {
                    let c = self.ctx.as_ref().unwrap();
                    c.lib.games.iter().find(|x| x.id == "crystal").or(c.lib.games.first()).map(theme_of).unwrap_or_else(Theme::default_theme)
                };
                let cw = r.width * 0.8;
                let ch = r.height * 1.05;
                let gap_c = r.width * 0.12;
                for i in 0..2 {
                    let sel = p.sel == i;
                    let lift = if sel { (if live { self.sel_ease } else { 1.0 }) * r.height * 0.06 } else { 0.0 };
                    let c = RectangleF::new(r.x + r.width / 2.0 + if i == 0 { -gap_c / 2.0 - cw } else { gap_c / 2.0 }, r.y - lift, cw, ch);
                    let st = g.save();
                    let path = round(c, cw * 0.12);
                    g.set_clip_path(&path);
                    if i == 0 {
                        let layer = crate::scenes::layer_for(c.width as i32, c.height as i32, &sample);
                        g.draw_image_bilinear_shared(&layer.picture, c);
                    } else if !sample.game_id.as_deref().map_or(false, |id| crate::game_art::draw_town(g, id, c, c)) {
                        g.fill_rectangle(&solid(Color::rgb(72, 76, 88)), c);
                        let big = UiFont::get("Segoe MDL2 Assets", c.width * 0.3);
                        self.w(g, "\u{E91B}", &big, Color::from_argb(220, 255, 255, 255), c, Center, Center, false);
                    }
                    g.restore(st);
                    g.draw_path(&Pen::new(if sel { accent(th) } else { PHONE_LINE }, (cw * if sel { 0.03 } else { 0.012 }).max(2.0)), &path);
                    if live { self.hits.push((c, i)); }
                }
            }
            _ => {
                // A check mark drawing itself in a green disc.
                let green = Color::rgb(46, 164, 79);
                let d = r.width * 0.9;
                let k = ease((appear - 0.15) / 0.5);
                let c = RectangleF::new(r.x + (r.width - d) / 2.0, r.y + (r.height - d) / 2.0, d, d);
                g.fill_ellipse(&solid(green), c);
                let pts = [PointF::new(c.x + d * 0.28, c.y + d * 0.52), PointF::new(c.x + d * 0.44, c.y + d * 0.67), PointF::new(c.x + d * 0.73, c.y + d * 0.36)];
                let (l1, l2) = (vector(pts[0], pts[1]), vector(pts[1], pts[2]));
                let done = k * (l1 + l2);
                let mut path = vec![pts[0]];
                if done <= l1 {
                    path.push(PointF::new(pts[0].x + (pts[1].x - pts[0].x) * done / l1, pts[0].y + (pts[1].y - pts[0].y) * done / l1));
                } else {
                    path.push(pts[1]);
                    let f = (done - l1) / l2;
                    path.push(PointF::new(pts[1].x + (pts[2].x - pts[1].x) * f, pts[1].y + (pts[2].y - pts[1].y) * f));
                }
                if k > 0.0 {
                    let mut pen = Pen::new(Color::WHITE, d * 0.09);
                    pen.start_cap = LineCap::Round;
                    pen.end_cap = LineCap::Round;
                    pen.line_join = LineJoin::Round;
                    g.draw_lines(&pen, &path);
                }
                // A ring of little sparks bursting out as it completes.
                let burst = ease((appear - 0.55) / 0.6);
                if burst > 0.0 && burst < 1.0 {
                    for i in 0..10 {
                        let a = i as f64 * std::f64::consts::PI / 5.0;
                        let rr = d * (0.55 + 0.35 * burst);
                        let s = d * 0.035 * (1.0 - burst);
                        let col = fade(if i % 2 == 0 { green } else { Color::rgb(240, 180, 40) }, (255.0 * (1.0 - burst)) as i32);
                        g.fill_ellipse(&solid(col), RectangleF::new(c.x + d / 2.0 + a.cos() as f32 * rr - s, c.y + d / 2.0 + a.sin() as f32 * rr - s, s * 2.0, s * 2.0));
                    }
                }
            }
        }
    }
}

/// setupLeaving: the last setup step, kept as a picture that grows and fades over the home screen.
#[derive(Clone, Default)]
pub struct Leaving {
    pub page: Option<Page>,
    pub shot: Option<Bitmap>,
    pub at: f32,
    /// The text mask lines of the picture (test runs), moved with it each frame so its text stays masked.
    pub text: Vec<String>,
}
pub const SETUP_FADE: f32 = 0.5;

impl Launcher {
    pub fn setup_leaving_active(&self) -> bool { self.leaving.page.is_some() }
    /// SetupGo(step): the step leaves one way as the next comes in from the other.
    pub fn setup_go(&mut self, step: i32) {
        let Some(from) = self.pages.last().cloned() else { return };
        self.setup_dir = if step > from.step { 1 } else { -1 };
        self.setup_at = self.now();
        self.setup_from = Some(from);
        self.pages.pop();
        let p = setup_page(self.ctx(), step, false);
        self.pages.push(p);
    }
    /// SetupFinish: the last step fades out over the home screen, then AskAboutRecomp.
    pub fn setup_finish(&mut self) {
        self.leaving.page = self.pages.last().cloned();
        self.leaving.at = self.now();
        self.ctx.as_mut().unwrap().first_setup = false;
        self.setup_from = None;
        while self.pages.len() > 1 && !self.pages.last().unwrap().main { self.pages.pop(); }
        self.replace_main();
        self.ask_about_recomp();
    }
    /// RenderSetupLeaving.
    pub fn render_setup_leaving(&mut self, g: &mut Graphics, w: i32, h: i32, t: f32, th: &Theme) {
        let Some(page) = self.leaving.page.clone() else { return };
        if self.leaving.shot.as_ref().map_or(true, |s| s.width != w || s.height != h) {
            let mut sg = Graphics::new(Bitmap::new(w.max(1), h.max(1)));
            sg.smoothing_mode = SmoothingMode::AntiAlias;
            sg.clear(PHONE_BG);
            // The picture's own text: kept apart, then masked where the picture puts it.
            let mask = self.hooks.mask.replace(Vec::new());
            self.render_setup(&mut sg, &page, w, h, t, th, 99.0, false);
            self.leaving.text = std::mem::replace(&mut self.hooks.mask, mask).unwrap_or_default();
            self.leaving.shot = Some(sg.bmp);
        }
        let k = ease((t - self.leaving.at) / SETUP_FADE);
        if k >= 1.0 { self.leaving = Leaving::default(); return; }
        let s = 1.0 + 0.08 * k;
        let (wf, hf) = (w as f32, h as f32);
        let dest = RectangleF::new(wf / 2.0 - wf * s / 2.0, hf / 2.0 - hf * s / 2.0, wf * s, hf * s);
        if let Some(m) = self.hooks.mask.as_mut() {
            for l in &self.leaving.text {
                let (rect, tail) = l.split_once('\t').unwrap_or((l.as_str(), ""));
                let v: Vec<f32> = rect.split(' ').filter_map(|x| x.parse().ok()).collect();
                if v.len() != 4 { continue; }
                // Through the screen's own transform too (the C# hook leaves the picture's text in picture space).
                let a = g.transform_point(PointF::new(dest.x + v[0] * s, dest.y + v[1] * s));
                let b = g.transform_point(PointF::new(dest.x + (v[0] + v[2]) * s, dest.y + (v[1] + v[3]) * s));
                let (x0, y0, x1, y1) = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                let (ix, iy) = (x0.floor() as i32, y0.floor() as i32);
                m.push(format!("{} {} {} {}\t{}", ix, iy, x1.ceil() as i32 - ix, y1.ceil() as i32 - iy, tail));
            }
        }
        // Rectangle.Round(dest), drawn with ColorMatrix Matrix33 = 1 - k.
        // Rectangle.Round: Math.Round, ties to even.
        let rd = RectangleF::new(dest.x.round_ties_even(), dest.y.round_ties_even(), dest.width.round_ties_even(), dest.height.round_ties_even());
        let shot = self.leaving.shot.clone().unwrap();
        g.draw_image_alpha(&shot, rd, 1.0 - k);
    }
}
