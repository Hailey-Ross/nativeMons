//! --snapshot and --snapshot-anim: screens drawn offscreen into PNGs (and their text masks), as
//! LauncherForm.Snapshot and SnapshotFrames do.

use crate::art::APP_NAMES;
use crate::launcher::{power_curve, Host, Launcher, LIGHTS_AT, OPEN_TIME};
use crate::pages::*;
use nm_core::library::Library;
use nm_draw::{Bitmap, Graphics, SmoothingMode};
use std::path::Path;

pub(crate) fn save(g: &mut Graphics, l: &mut Launcher, file: &Path) -> Result<(), String> {
    g.bmp.save_png(file).map_err(|e| e.to_string())?;
    l.hooks.save_mask(file);
    // Accepted residuals (named GDI+ noise) for golden-diff, beside the text mask.
    let acc: String = g.accepted.iter().map(|(n, x, y, w, h)| format!("{}\t{} {} {} {}\r\n", n, x, y, w, h)).collect();
    std::fs::write(file.with_extension("accept.txt"), acc).map_err(|e| e.to_string())
}

/// Builds the launcher and its pages for a screen name, as Snapshot does before it draws.
pub(crate) fn setup(screen: &str, w: i32, h: i32, host: Host, anim_t0: Option<f32>) -> Result<Launcher, String> {
    let mut l = Launcher::new(w, h, host);
    // An animation's clock starts at t0 before anything is set up, as Snapshot does.
    if let Some(t0) = anim_t0 { l.hooks.t = Some(t0); l.snapshot_anim = true; }
    if std::env::var("NATIVEMONS_SNAPSHOT_TEXTMASK").as_deref() == Ok("1") { l.hooks.mask = Some(Vec::new()); }
    let lib = Library::scan(&nm_core::engines::gen1_cli).unwrap_or_else(|e| {
        let mut l = Library::scan(&|_| None).unwrap_or_else(|_| panic!("{}", e));
        l.note = e;
        l
    });
    // PickCurrent.
    let last = nm_core::config::read("last-game.txt", "");
    let mut current = lib.games.iter().position(|g| g.id == last && g.playable()).or_else(|| lib.games.iter().position(|g| g.playable()));
    let (mut screen, mut preview_id) = (screen.to_string(), None::<String>);
    if let Some(colon) = screen.find(':') {
        if colon > 0 {
            let id = screen[colon + 1..].to_string();
            screen = screen[..colon].to_string();
            if let Some(i) = lib.games.iter().position(|g| g.id == id) { current = Some(i); }
            preview_id = Some(id);
        }
    }
    crate::game_art::set_library(&lib.games);
    crate::game_art::set_seed(l.hooks.seed);
    l.ctx = Some(Ctx { lib, current, first_setup: false, work_area: host.work_area, wide_window: l.wide_window, light_auto: host.light_auto, light_why: host.light_why });
    let c = l.ctx.as_ref().unwrap();
    l.current_theme = c.current.map(|i| theme_of(&c.lib.games[i]));
    let s = screen.as_str();
    if s == "updatebanner" {
        let build = nm_core::updater::build() + 1;
        nm_core::updater::set_latest(Some(nm_core::updater::Release { build, sha256: Some("preview".into()), ..Default::default() }));
        l.banner_build = build;
        l.banner_at = -1.0;
    }
    // clashbanner[:<game>]: the clash banner, read now; a sample when nothing clashes.
    if s == "clashbanner" {
        let c = l.ctx.as_ref().unwrap();
        let (mut first, mut count) = (None, 0);
        for (i, mg) in c.lib.games.iter().enumerate().filter(|(_, x)| x.on_gen1() && preview_id.as_ref().map_or(true, |p| &x.id == p)) {
            let n = nm_core::mods::read_clashes(mg).iter().filter(|c| c.active()).count() as i32;
            if n > 0 { count += n; if first.is_none() { first = Some(i); } }
        }
        if first.is_none() { first = c.lib.games.iter().position(|x| x.on_gen1()); count = 1; }
        l.clash_game = first;
        l.clash_count = count;
        l.clash_at = -100.0;
    }
    if s == "improvebanner" {
        l.recomp_banner = Some(crate::launcher::RecompBanner { title: "Pokemon Y".into(), verb: "Improving".into(), progress: 0.45, done_at_wall: l.hooks.wall_seconds() - 30.0, fake_running: true, paused: false });
    }
    if s == "splash" {
        l.starting = true;
        l.start_since_wall = l.hooks.wall_seconds() - 4.0;
        l.start_stage = Some("Looking for your games".into());
    } else {
        let p = main_page(l.ctx.as_ref().unwrap());
        l.pages.push(p);
    }
    if s == "folder-added" { l.ctx.as_mut().unwrap().first_setup = true; }
    let c = l.ctx.as_ref().unwrap();
    let find = |id: &Option<String>| id.as_ref().and_then(|id| c.lib.games.iter().position(|g| &g.id == id));
    let mut push: Vec<crate::model::Page> = Vec::new();
    // The snapshot's own Pokedex (the sample one too), kept for what a replay opens after it, as the C# form keeps dexBy.
    let mut keep_dex = None;
    match s {
        "games" => push.push(games_page(c, None)),
        "folders" => push.push(folders_page(c)),
        "saves" => {
            if let Some(i) = c.current {
                match saves_page(&c.lib.games[i]) {
                    Some(p) => push.push(p),
                    None => return Err(format!("{}: this save list needs the Gen1Recomp engine (the C# asks it for the slot)", screen)),
                }
            }
        }
        "credits" => push.push(credits_page()),
        "settings" => push.push(settings_page(c)),
        "controls" => push.push(controls_page()),
        "roms" | "gamepage" => {
            if let Some(i) = find(&preview_id) { push.push(if s == "roms" { roms_page(&c.lib.games[i]) } else { game_page(c, &c.lib.games[i]) }); }
        }
        "game" => {
            let sample = find(&preview_id).or_else(|| c.lib.games.iter().position(|g| g.playable())).or(if c.lib.games.is_empty() { None } else { Some(0) });
            if let Some(i) = sample { push.push(game_page(c, &c.lib.games[i])); }
        }
        "sprites" => push.push(sprites_page(c)),
        "wallpaper" => push.push(wallpaper_page(c, true)),
        // boxes|boxheld|boxchoose:<top id>,<bottom id>: the box screen, reading the saves only.
        "boxes" | "boxheld" | "boxchoose" => {
            use crate::boxes::*;
            let pid = preview_id.clone().unwrap_or_default();
            let ids: Vec<&str> = pid.split(',').collect();
            let a = c.lib.games.iter().find(|g| g.id == ids[0] && g.save_tools());
            let b = if ids.len() > 1 { c.lib.games.iter().find(|g| g.id == ids[1] && g.save_tools()) } else { a };
            if let (Some(a), Some(b)) = (a, b) {
                let mut v = BoxView::default();
                let sa = load_side(a);
                let sb = if b.id == a.id { sa.clone() } else { load_side(b) };
                let first = sa.mons.keys().filter(|k| **k / sa.slots == 0).min().copied().unwrap_or(0);
                v.cursor = first % sa.slots;
                v.side = [Some(sa), Some(sb)];
                if s == "boxheld" && v.side[0].as_ref().unwrap().at(0, v.cursor).is_some() { v.held_pane = 0; v.held_box = 0; v.held_slot = v.cursor; box_other(&mut v); }
                let mut page = crate::model::Page::new("Move Pokemon");
                page.boxes = Some(v);
                push.push(page);
                if s == "boxchoose" { let v = push.last().unwrap().boxes.clone().unwrap(); push.push(choose_box_game(c, &v, 1)); }
            }
        }
        "dex" | "dexlist" | "dexentry" | "dexother" | "dexgames" | "dexgame" => {
            use crate::dex::*;
            let mut d = load_dex(c)?;
            if (s == "dex" || s == "dexlist" || s == "dexentry") && std::env::var("NATIVEMONS_TEST_DEX").as_deref() == Ok("sample") && !c.lib.games.is_empty() { d.sample(c); }
            match s {
                "dex" => push.push(dex_home_page(c, &d)),
                "dexother" => { push.push(dex_home_page(c, &d)); push.push(dex_other_page(c, &d)); }
                "dexgames" => { push.push(dex_home_page(c, &d)); push.push(dex_games_page(c, &d)); }
                "dexgame" => {
                    if let Some(one) = find(&preview_id) { let g = &c.lib.games[one]; push.push(dex_other_page(c, &d)); push.push(dex_list_page(c, &d, &g.title, 1, 151, Some(&g.id))); }
                }
                "dexentry" => {
                    let species: i32 = preview_id.as_deref().and_then(|v| v.parse().ok()).unwrap_or(25);
                    push.push(dex_entry_page(c, &d, species, None));
                }
                _ => {
                    let species: i32 = preview_id.as_deref().and_then(|v| v.parse().ok()).unwrap_or(25);
                    let mut list = dex_list_page(c, &d, "Kanto", 1, 151, None);
                    list.sel = (species - 1).min(150);
                    push.push(list);
                }
            }
            keep_dex = Some(d);
        }
        "mods" => { if let Some(m) = c.lib.games.iter().find(|g| g.on_gen1()) { push.push(crate::mods_pages::mods_page(m)); } }
        "clashes" | "clashon" | "deps" | "dep" | "findmods" | "modentry" => {
            use crate::mods_pages::*;
            let pid = preview_id.clone().unwrap_or_default();
            let ids: Vec<&str> = pid.split(',').collect();
            if let Some(mg) = c.lib.games.iter().find(|x| x.id == ids[0] && x.on_gen1()) {
                let second = ids.get(1).copied().unwrap_or("");
                match s {
                    "clashes" | "clashon" => {
                        push.push(mods_page(mg));
                        if s == "clashes" { push.push(clashes_page(mg, None)); }
                        else if let Some(cl) = nm_core::mods::read_clashes(mg).into_iter().find(|x| x.mod_id == second || x.other_id == second) {
                            let name = if cl.mod_id == second { cl.mod_name.clone() } else { cl.other_name.clone() };
                            push.push(clash_choice_page(mg, second, &name, &cl));
                        }
                    }
                    "deps" | "dep" => {
                        push.push(mods_page(mg));
                        push.push(deps_page(mg, if s == "deps" { Some("Installed (the mod just added).") } else { None }));
                        if s == "dep" {
                            if let Some(one) = nm_core::mods::open(mg).into_iter().find(|x| x.id == second) {
                                let all = nm_core::mod_index::load().map(|x| x.0).unwrap_or_default();
                                push.push(dep_page(mg, &one, &all));
                            }
                        }
                    }
                    _ => {
                        let (all, note) = nm_core::mod_index::load()?;
                        let have = installed_mods(mg);
                        push.push(mods_page(mg));
                        push.push(find_mods_page(mg, &have, &all, note.as_deref()));
                        if s == "modentry" { if let Some(one) = all.iter().find(|x| x.id == second) { push.push(mod_entry_page(mg, one, &have)); } }
                    }
                }
            }
        }
        "import" => push.push(import_page(c)),
        "browse" => push.push(browse_page(&BrowseRequest { title: "Choose your ROM folder".into(), folders: true, kinds: ROM_KINDS.to_vec(), hint: "Open the folder that holds your ROMs, then choose Use this folder.".into(), ..Default::default() }, None)),
        "browsedir" => { if let Some(d) = &preview_id { push.push(browse_page(&BrowseRequest { title: "Choose your ROM folder".into(), folders: true, kinds: ROM_KINDS.to_vec(), ..Default::default() }, Some(d))); } }
        "browsefiles" => {
            if let Some(d) = &preview_id {
                let mut rq = BrowseRequest { title: "Choose ROMs".into(), multi: true, kinds: ROM_KINDS.to_vec(), ..Default::default() };
                let mut files: Vec<String> = std::fs::read_dir(d).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()).collect()).unwrap_or_default();
                files.retain(|f| { let e = std::path::Path::new(f).extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default(); ROM_KINDS.contains(&e.as_str()) });
                rq.picked = files.into_iter().take(2).collect();
                push.push(browse_page(&rq, Some(d)));
            }
        }
        "wallpapers" => { push.push(settings_page(c)); push.push(wallpaper_page(c, false)); }
        "ownwallpapers" => push.push(own_wallpapers_page(c)),
        "ownwallpaper" => { if let Some(i) = c.current { push.push(own_wallpaper_page(&c.lib.games[i])); } }
        "findwall" => { if let Some(i) = c.current { push.push(find_wallpaper_page(&c.lib.games[i])); } }
        _ if s.starts_with("emuask-") => {
            let f: Vec<&str> = s.split('-').collect();
            let e = nm_core::emu3ds::ALL.iter().find(|e| Some(&e.id) == f.get(1));
            let g = c.lib.games.iter().find(|g| g.ctr.is_some());
            if let (Some(e), Some(g)) = (e, g) { push.push(emu_update_ask_page(e, g, f.len() > 2)); }
        }
        "recompask" => {
            if let Some(i) = find(&preview_id) { let g = &c.lib.games[i]; push.push(recomp_ask_page(&[g], Some(g))); }
        }
        "engines" => {
            if let Some(i) = find(&preview_id) { let g = &c.lib.games[i]; push.push(game_page(c, g)); push.push(engine_page(g)); }
        }
        "main" | "splash" | "updatebanner" | "improvebanner" | "clashbanner" => {}
        _ if s.starts_with("controls-") => {
            let n: usize = s[9..].parse().map_err(|_| format!("bad screen {}", screen))?;
            push.push(controls_page());
            let sheet = CONTROL_SHEETS.get(n).ok_or_else(|| format!("bad screen {}", screen))?;
            push.push(show_text(&format!("{} controls", sheet[0]), sheet[2]));
        }
        "job" => {
            l.job = true;
            l.job_title = "Preparing Pokemon LeafGreen".into();
            l.job_start_wall = l.hooks.wall_seconds() - 14.0;
            l.progress = 0.62;
            l.log = vec!["Reading the ROM".into(), "Decoding maps".into(), "Decoding Pokemon graphics".into()];
        }
        "screenshots" => push.push(screenshots_page(c)),
        "folder-added" => {
            let first = nm_core::config::rom_folders().first().cloned().unwrap_or_default();
            push.push(folders_page(c));
            push.push(folder_added_page(c, &first));
        }
        _ if s.starts_with("emuinstalled-") => {
            let f: Vec<&str> = s.split('-').collect();
            let e = nm_core::emu3ds::ALL.iter().find(|e| Some(&e.id) == f.get(1)).ok_or_else(|| format!("no emulator in {}", s))?;
            push.push(emu_page());
            push.push(emu_detail_page(e));
            l.emu_install_name = Some(e.short.to_string());
            l.emu_install_at = l.now() - f.get(2).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        }
        "emu" => push.push(emu_page()),
        _ if s.starts_with("emu-") => {
            let e = nm_core::emu3ds::ALL.iter().find(|e| e.id == &s[4..]).ok_or_else(|| format!("no emulator {}", &s[4..]))?;
            push.push(emu_detail_page(e));
        }
        "gameoptions" => push.push(game_options_page(find(&preview_id).map(|i| &c.lib.games[i]))),
        _ if s.starts_with("optpick-") => {
            let all = game_options_all();
            let o = all.iter().find(|o| o.key == &s[8..]).ok_or_else(|| format!("no option {}", &s[8..]))?;
            push.push(game_options_page(None));
            push.push(option_pick_page(None, o));
        }
        _ if s.starts_with("setup") => {
            // setup-<step>[only][@seconds], setupslide-<from>-<to>@seconds.
            let (mut spec, mut at) = (s.to_string(), 99.0f32);
            if let Some(k) = spec.find('@') { at = spec[k + 1..].parse().map_err(|_| format!("bad screen {}", screen))?; spec.truncate(k); }
            let now = l.now();
            if let Some(rest) = spec.strip_prefix("setupslide-") {
                let f: Vec<i32> = rest.split('-').filter_map(|v| v.parse().ok()).collect();
                if f.len() < 2 { return Err(format!("bad screen {}", screen)); }
                l.setup_from = Some(crate::setup::setup_page(c, f[0], false));
                push.push(crate::setup::setup_page(c, f[1], false));
                l.setup_dir = if f[1] > f[0] { 1 } else { -1 };
            } else {
                let only = spec.ends_with("only");
                let step: i32 = spec.get(6..7).and_then(|v| v.parse().ok()).ok_or_else(|| format!("bad screen {}", screen))?;
                if step > 4 { return Err(format!("{}: the C# has 5 steps (0-4) and throws here", screen)); }
                push.push(crate::setup::setup_page(c, step, only));
            }
            l.setup_at = now - at;
        }
        _ if s.starts_with("guide") || s == "appicons" || s == "emuinstall" => {}
        _ if s.starts_with("open-") || s == "poweroff" => {}
        _ => match crate::more_pages::extra_screen(c, s, preview_id.as_deref()) { Some(v) => push.extend(v), None => return Err(format!("screen {} is not ported yet", screen)) },
    }
    l.pages.extend(push);
    if keep_dex.is_some() { l.dex = keep_dex; }
    Ok(l)
}

pub fn snapshot(file: &Path, screen: &str, w: i32, h: i32, host: Host) -> Result<(), String> {
    let mut l = setup(screen, w, h, host, None)?;
    let base = screen.split(':').next().unwrap_or(screen);
    if base.starts_with("guide") {
        // The guide overlay over a 1280x720 game screen.
        let mut g = Graphics::new(Bitmap::new(1280, 720));
        g.smoothing_mode = SmoothingMode::AntiAlias;
        let c = l.ctx.as_ref().unwrap();
        let cur = c.current.map(|i| &c.lib.games[i]);
        let th = l.current_theme.clone().unwrap_or_else(crate::model::Theme::default_theme);
        let title = cur.map(|g| g.title.clone()).unwrap_or_else(|| "Pokemon Platinum".into());
        let controls = controls_for(&cur.map(|g| g.engine()).unwrap_or_else(|| "port".into()));
        let sel = if base == "guide-close" { 2 } else if base == "guide-controls" { 1 } else { 0 };
        crate::launcher::guide_draw(&mut l.hooks, &mut g, (1280, 720), &title, &controls, crate::launcher::accent(&th), sel, base == "guide-controls", base == "guide-close");
        return save(&mut g, &mut l, file);
    }
    if base == "emuinstall" || base == "appicons" {
        let times = [-1.0f32, 0.25, 0.35, 0.45, 0.55, 0.65, 1.0, 1.6, 2.2];
        let (cols, rows) = if base == "emuinstall" { (10, 1) } else { (times.len(), APP_NAMES.len() - 1) };
        let mut g = Graphics::new(Bitmap::new(cols as i32 * 90, rows as i32 * 90));
        g.clear(nm_draw::Color::rgb(60, 60, 70));
        for a in 0..rows {
            for k in 0..cols {
                let app = if base == "emuinstall" { crate::art::app("Emu") } else { a + 1 };
                let r = nm_draw::RectangleF::new(k as f32 * 90.0 + 10.0, a as f32 * 90.0 + 10.0, 70.0, 70.0);
                crate::art::gloss_tile(&mut g, r, 70.0 * 0.24, crate::art::APP_COLORS[app]);
                if base == "emuinstall" {
                    crate::art::app_art(&mut g, app, r, crate::art::APP_COLORS[app], -1.0, k as f32 / (cols - 1) as f32, 0.0);
                } else {
                    l.ball_shake(-1.0);
                    let u = times[k];
                    let shake = if APP_NAMES[app] == "Games" && u >= 0.0 { l.ball_shake(u) } else { 0.0 };
                    crate::art::app_art(&mut g, app, r, crate::art::APP_COLORS[app], u, -1.0, shake);
                }
            }
        }
        return g.bmp.save_png(file).map_err(|e| e.to_string());
    }
    let mut g = Graphics::new(Bitmap::new(w, h));
    l.render(&mut g, w, h);
    l.screen_at = -10.0;
    l.sel_at = -10.0;
    if let Some(m) = l.hooks.mask.as_mut() { m.clear(); }
    g.clear_accepted();
    // NATIVEMONS_TEST_DAMAGE=1: drawn from its recorded ops (the live path), to check it against the direct one.
    let rec = std::env::var("NATIVEMONS_TEST_DAMAGE").as_deref() == Ok("1");
    if rec { g.begin_record(); }
    l.render(&mut g, w, h);
    if rec { g.flush(&[]); }
    save(&mut g, &mut l, file)
}

/// A launcher set up on a screen as --snapshot sets it up (the live loop's headless tests).
pub fn launcher_for(screen: &str, w: i32, h: i32, host: Host) -> Result<Launcher, String> { setup(screen, w, h, host, None) }

/// What --bench measured: ms per full frame, ms per live frame (recorded, only the changed parts redrawn), the share
/// of the window those redrew, and live frames that differed from the full one (must be 0).
pub struct Bench { pub full: Vec<f64>, pub live: Vec<f64>, pub redrawn: f64, pub differ: usize }

/// --bench <screen> <WxH> <frames> [still]: each frame drawn both ways at the same time, the clock stepping 1/60 s a
/// frame unless `still`; one warm-up frame first.
pub fn bench(screen: &str, w: i32, h: i32, n: usize, still: bool, host: Host) -> Result<Bench, String> {
    let mut l = setup(screen, w, h, host, None)?;
    l.hooks.mask = None;
    let t0 = l.now();
    let (mut full_bmp, mut live_bmp) = (Bitmap::new(w, h), Bitmap::new(w, h));
    let mut ops = Vec::new();
    let mut kept: Vec<(&'static str, f64)> = Vec::new();
    let mut b = Bench { full: Vec::with_capacity(n), live: Vec::with_capacity(n), redrawn: 0.0, differ: 0 };
    for i in 0..=n {
        if !still { l.hooks.t = Some(t0 + i as f32 / 60.0); }
        let st = std::time::Instant::now();
        let mut g = Graphics::new(std::mem::replace(&mut live_bmp, Bitmap::new(0, 0)));
        g.begin_record();
        l.paint(&mut g, w, h);
        let (o, dirty) = g.flush(&ops);
        ops = o;
        live_bmp = g.bmp;
        let live_ms = st.elapsed().as_secs_f64() * 1000.0;
        let live_prof = nm_draw::prof::take();
        let st = std::time::Instant::now();
        let mut g = Graphics::new(std::mem::replace(&mut full_bmp, Bitmap::new(0, 0)));
        g.track = false;
        l.paint(&mut g, w, h);
        full_bmp = g.bmp;
        let full_ms = st.elapsed().as_secs_f64() * 1000.0;
        let full_prof = nm_draw::prof::take();
        if i > 0 {
            b.full.push(full_ms);
            b.live.push(live_ms);
            b.redrawn += dirty.iter().map(|d| ((d.2 - d.0) as f64) * ((d.3 - d.1) as f64)).sum::<f64>() / (w as f64 * h as f64) / n as f64;
            if live_bmp.data != full_bmp.data { b.differ += 1; }
            // The stage timers keep the full frames' (taken by the caller), with the live frames' under "live".
            kept.push(("live frame", live_ms));
            for (name, ms, _) in live_prof { if name == "flush" { kept.push(("live flush", ms)); } }
            for (name, ms, _) in full_prof { kept.push((name, ms)); }
        } else {
            nm_draw::prof::take();
            // Settled, as --snapshot draws it: no slide-in or selection animation left.
            l.screen_at = -10.0;
            l.sel_at = -10.0;
        }
    }
    for (name, ms) in kept { nm_draw::prof::add(name, ms); }
    Ok(b)
}
/// --snapshot-anim <folder> <screen> <WxH> <t0> <t1> <fps>: frame-NNNN.png per frame (SnapshotFrames).
pub fn snapshot_anim(folder: &Path, screen: &str, w: i32, h: i32, t0: f32, t1: f32, fps: f32, host: Host) -> Result<(), String> {
    let mut l = setup(screen, w, h, host, Some(t0))?;
    l.hooks.t = Some(t0);
    l.snapshot_anim = true;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let app = screen.strip_prefix("open-").and_then(|a| APP_NAMES.iter().position(|n| *n == a)).filter(|&a| a >= 1);
    if app.is_some() || screen == "poweroff" {
        if let Some(a) = app {
            if let Some(p) = l.pages.last_mut() { if (a as usize) < p.items.len() { p.sel = a as i32; } }
        }
        l.hooks.t = Some(t0 - 1.0);
        let mut g = Graphics::new(Bitmap::new(w, h));
        l.render(&mut g, w, h);
        l.screen_at = -10.0;
        l.sel_at = -10.0;
        l.render(&mut g, w, h);
        if let Some(m) = l.hooks.mask.as_mut() { m.clear(); }
        if let Some(a) = app {
            let ok = l.pages.last().map_or(false, |p| p.main && a < p.items.len() && APP_NAMES[a] != "Move" && APP_NAMES[a] != "Exit");
            if ok && !l.app_rects[a].is_empty() {
                l.open_run = l.pages.last().and_then(|p| p.items[a].run.clone());
                l.open_app = a;
                l.open_from = l.app_rects[a];
                l.open_at = t0;
            }
        }
        if screen == "poweroff" { l.power_off = t0; }
    }
    let n = ((t1 - t0) * fps + 0.001).floor() as i32 + 1;
    for i in 0..n {
        let t = t0 + i as f32 / fps;
        l.hooks.t = Some(t);
        // OpenTick.
        if l.open_run.is_some() && t - l.open_at >= OPEN_TIME {
            let run = l.open_run.take().unwrap();
            l.bounce_arm = true;
            run.call(&mut l);
        }
        // PowerOffTick (the window itself stays put).
        if l.power_off >= 0.0 {
            let u = t - l.power_off;
            l.power_level = power_curve(u - LIGHTS_AT);
            if u >= LIGHTS_AT && l.frozen_t < 0.0 { l.frozen_t = l.power_off + LIGHTS_AT; }
        }
        let png = folder.join(format!("frame-{:04}.png", i));
        let mut g = Graphics::new(Bitmap::new(w, h));
        l.render(&mut g, w, h);
        save(&mut g, &mut l, &png)?;
    }
    Ok(())
}
