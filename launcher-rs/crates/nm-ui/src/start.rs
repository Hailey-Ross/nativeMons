//! Start and FinishStart from the C#: the splash while the library is read and new Gen1Recomp games are prepared (off
//! the UI thread), then the home screen, with the splash's last frame fading out over it (Outro).

use crate::launcher::Launcher;
use crate::pages::{self, Ctx};
use nm_core::library::Library;
use nm_core::{config, engines, saves};
use nm_draw::{Bitmap, Graphics, RectangleF};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

pub struct Starting {
    rx: Receiver<(Library, Option<String>)>,
    stage: Arc<Mutex<String>>,
}

// ---------- PrepareAll (Gen1Recomp's imports behind the splash) ----------
fn prepare_failed_file() -> std::path::PathBuf { config::state().join("prepare-failed.txt") }
fn prepare_key(rom: &str) -> String {
    let p = std::path::Path::new(rom);
    let len = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    format!("{}|{}|{}", rom, len, nm_core::library::last_write_ticks(p))
}
fn prepare_failed(rom: &str) -> bool {
    std::fs::read_to_string(prepare_failed_file()).map_or(false, |t| config::lines(&t).iter().any(|l| *l == prepare_key(rom)))
}
fn note_prepare_failed(rom: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(prepare_failed_file()) { let _ = write!(f, "{}\r\n", prepare_key(rom)); }
}
/// PrepareAll: every Gen1Recomp game not yet imported, three at once at most (Prepare3ds runs before it, in scan_all,
/// as the C# PrepareAll starts with it).
fn prepare_all(lib: &mut Library, stage: &Arc<Mutex<String>>) {
    if saves::running_games(&lib.games) != "" { return; }
    let todo: Vec<usize> = lib.games.iter().enumerate().filter(|(_, g)| g.gen1.is_some() && !g.imported && g.gen1_rom.as_deref().map_or(false, |r| !prepare_failed(r))).map(|(i, _)| i).collect();
    if todo.is_empty() { return; }
    let cpus = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(1);
    let lanes = (cpus / 6).clamp(1, 3);
    let workers = (cpus / (lanes * 4)).clamp(1, 2);
    let total = todo.len();
    let set = |s: String| *stage.lock().unwrap() = s;
    set(if total == 1 { format!("Preparing {}", lib.games[todo[0]].title) } else { "Preparing your games".into() });
    let done = std::sync::atomic::AtomicUsize::new(0);
    let one = |g: &nm_core::game_entry::GameEntry| -> bool {
        let ok = saves::prepare_now(g, workers);
        if !ok { note_prepare_failed(g.gen1_rom.as_deref().unwrap_or("")); }
        let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        set(if n < total { format!("Preparing your games ({} of {} ready)", n, total) } else { "Almost ready".into() });
        ok
    };
    let first_ok = one(&lib.games[todo[0]]);
    if first_ok { lib.games[todo[0]].imported = true; }
    // Biggest ROMs first: a GBA game takes several times longer than a Game Boy one, so the slow ones overlap.
    let mut rest: Vec<usize> = todo[1..].to_vec();
    let size = |i: &usize| std::fs::metadata(lib.games[*i].gen1_rom.as_deref().unwrap_or("")).map(|m| m.len()).unwrap_or(0);
    rest.sort_by(|a, b| size(b).cmp(&size(a)));
    let queue = Mutex::new(rest.into_iter());
    let results = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..lanes {
            s.spawn(|| loop {
                let Some(i) = queue.lock().unwrap().next() else { return };
                let ok = one(&lib.games[i]);
                results.lock().unwrap().push((i, ok));
            });
        }
    });
    for (i, ok) in results.into_inner().unwrap() { if ok { lib.games[i].imported = true; } }
}
/// WallpapersAtStart: the opening game's title screen and town are made once, before the menu appears.
fn wallpapers_at_start(lib: &Library, stage: &Arc<Mutex<String>>) {
    let last = config::read("last-game.txt", "");
    let first = lib.games.iter().find(|g| g.id == last.trim() && g.playable()).or_else(|| lib.games.iter().find(|g| g.playable()));
    crate::game_art::set_library(&lib.games);
    if saves::running_games(&lib.games).is_empty() {
        let (games, first_id, st) = (lib.games.clone(), first.map(|g| g.id.clone()), stage.clone());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            crate::capture::capture_at_start(&games, first_id.as_deref(), |n, total| *st.lock().unwrap() = format!("Making wallpapers ({} of {})", n + 1, total));
            let _ = tx.send(());
        });
        // At most 20 s behind the splash: a DS game's set of towns takes minutes, so it finishes behind the menu instead.
        if rx.recv_timeout(std::time::Duration::from_secs(20)).is_err() { *stage.lock().unwrap() = "Almost ready".into(); }
    }
    if let Some(g) = first { let _ = crate::game_art::frame(&g.id, 0.0); let _ = crate::game_art::town(&g.id); }
}
/// ModDefaults: mods now off by default, switched off once in older settings.
fn mod_defaults(lib: &Library) {
    if !lib.games.iter().any(|g| g.gen1.is_some()) && !config::gen1().join("main.lua").is_file() { return; }
    let mut done = config::read("mod-defaults.txt", "");
    for id in ["example_lttp_overhaul"] {
        if done.split('\n').any(|l| l.trim() == id) { continue; }
        let mut psi = engines::gen1_start("");
        psi.set_env("NATIVEMONS_CLI", &format!("default-off {}", id));
        let (_, code) = engines::run_capture(&psi, 30);
        if code != 0 { continue; }
        done = format!("{}\n{}", done.trim_end(), id).trim().to_string();
        config::write("mod-defaults.txt", &format!("{}\n", done));
    }
}
fn scan_all(stage: Arc<Mutex<String>>, since: std::time::Instant, direct: bool, live: bool) -> (Library, Option<String>) {
    let mut err = None;
    // Once: Gen1Recomp's saves come beside the launcher, before anything asks it about them.
    if engines::gen1_data_move_needed() {
        *stage.lock().unwrap() = "Moving Gen1Recomp's saves beside the launcher".into();
        err = engines::ensure_gen1_data();
        *stage.lock().unwrap() = "Looking for your games".into();
    }
    let mut lib = match Library::scan(&engines::gen1_cli) {
        Ok(l) => { saves::recover_switches(&l.games); l }
        Err(e) => { err = Some(e.clone()); let mut l = Library::scan(&|_| None).unwrap_or_else(|_| panic!("{}", e)); l.note = e; l }
    };
    mod_defaults(&lib);
    // Prepare3ds first, as PrepareAll does; a failure there never stops the start.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::recomp_ui::prepare_3ds(&lib, &|s: &str| *stage.lock().unwrap() = s.to_string())));
    prepare_all(&mut lib, &stage);
    // A shortcut straight to a game doesn't wait for pictures.
    if !direct { wallpapers_at_start(&lib, &stage); }
    // The previous launcher left by an update goes; the update check, 2 s after (only in the live launcher: no network in tests).
    if let Ok(me) = std::env::current_exe() { let _ = std::fs::remove_file(format!("{}.old", me.display())); }
    if live { std::thread::spawn(|| { std::thread::sleep(std::time::Duration::from_secs(2)); let _ = nm_core::updater::check(); }); }
    // Title screens and towns are gathered afterwards, without holding up the menu; the Pokedex is read first.
    if live {
        let games = lib.games.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let (g2, g3) = (games.clone(), games.clone());
            let busy = move || nm_core::art_jobs::player_busy() || !saves::running_games(&g2).is_empty();
            let busy2 = busy.clone();
            crate::capture::capture_missing(&games, busy, move || crate::dex::load_dex_background(&g3, &busy2));
        });
    }
    crate::sprites::load_families(|| { if !config::mon_bridge().is_file() { return None; } let (o, code) = saves::lf_bridge("families"); if code == 0 { Some(o) } else { None } });
    let left = 1.1 - since.elapsed().as_secs_f64();
    if left > 0.0 { std::thread::sleep(std::time::Duration::from_secs_f64(left)); }
    (lib, err)
}

impl Launcher {
    /// Start: the splash at once, the library read off the UI thread (at once in a replay).
    pub fn start_up(&mut self) {
        // One random town per start (TestHooks.NewRandom), seeded before WallpapersAtStart picks.
        crate::game_art::set_seed(self.hooks.seed);
        self.starting = true;
        self.start_since_wall = self.hooks.wall_seconds();
        self.start_stage = Some("Looking for your games".into());
        self.dirty = true;
        let stage = Arc::new(Mutex::new("Looking for your games".to_string()));
        let since = std::time::Instant::now();
        let direct = self.jobs.play_direct.is_some();
        if self.hooks.replay { let (lib, err) = scan_all(stage, since, direct, false); self.finish_start(lib, err, 0, 0); return; }
        let live = crate::sprites::is_live();
        let (tx, rx) = channel();
        let s2 = stage.clone();
        std::thread::spawn(move || { let _ = tx.send(scan_all(s2, since, direct, live)); });
        self.jobs.start = Some(Starting { rx, stage });
    }
    /// Each tick while starting: the splash's stage line, and FinishStart once the work is done.
    pub fn start_tick(&mut self, w: i32, h: i32) {
        let Some(s) = self.jobs.start.as_ref() else { return };
        let stage = s.stage.lock().unwrap().clone();
        if self.start_stage.as_deref() != Some(&stage) { self.start_stage = Some(stage); self.dirty = true; }
        if let Ok((lib, err)) = s.rx.try_recv() { self.jobs.start = None; self.finish_start(lib, err, w, h); }
    }
    pub fn finish_start(&mut self, lib: Library, err: Option<String>, w: i32, h: i32) {
        if w > 0 && h > 0 {
            let mut g = Graphics::new(Bitmap::new(w, h));
            self.render(&mut g, w, h);
            self.jobs.splash_shot = Some(g.bmp);
            self.jobs.outro_at = self.now();
        }
        self.starting = false;
        let last = config::read("last-game.txt", "");
        let current = lib.games.iter().position(|g| g.id == last.trim() && g.playable()).or_else(|| lib.games.iter().position(|g| g.playable()));
        let any_playable = lib.games.iter().any(|g| g.playable());
        crate::game_art::set_library(&lib.games);
        self.ctx = Some(Ctx { lib, current, first_setup: false, work_area: self.host.work_area, wide_window: self.wide_window, light_auto: self.host.light_auto, light_why: self.host.light_why });
        self.current_theme = current.map(|i| pages::theme_of(&self.ctx().lib.games[i]));
        let m = self.main_page_now();
        self.push(m);
        self.read_clashes();
        if let Some(id) = self.jobs.play_direct.clone() {
            match self.game(&id) {
                Some(g) if g.playable() => { self.play(&id); return; }
                _ => { self.show_text(&format!("Can't start {}", id), "That game is not in the list, or its ROM was not found."); if let Some(p) = self.pages.last_mut() { p.on_back = Some(crate::model::Act::new(|l| l.request_close())); } return; }
            }
        }
        // The Rust-only test pages open as on a set-up install: no guided setup or start questions over them.
        let test_open = std::env::var("NATIVEMONS_TEST_OPEN").map_or(false, |v| ["emu", "settings", "wallpapers"].contains(&v.as_str()) || v.starts_with("game:") || v.starts_with("boxes:"));
        match std::env::var("NATIVEMONS_TEST_OPEN").as_deref() {
            Ok("move") => self.open_box_view(),
            Ok("credits") => self.push(pages::credits_page()),
            // Rust only, for the window tests: a page as its --snapshot screen opens it.
            Ok("emu") => self.push(pages::emu_page()),
            Ok("settings") => { let p = pages::settings_page(self.ctx()); self.push(p); }
            Ok("wallpapers") => { let p = pages::settings_page(self.ctx()); self.push(p); let w = pages::wallpaper_page(self.ctx(), false); self.push(w); }
            Ok(v) if v.starts_with("game:") => {
                let p = { let c = self.ctx(); c.lib.games.iter().find(|g| g.id == v[5..]).map(|g| pages::game_page(c, g)) };
                if let Some(p) = p { self.push(p); }
            }
            // Rust only, for the window test: Move Pokemon's boxes on two games' saves (OpenBoxView isn't ported yet).
            Ok(v) if v.starts_with("boxes:") => {
                use crate::boxes::*;
                let ids: Vec<String> = v[6..].split(',').map(|x| x.to_string()).collect();
                let c = self.ctx();
                let a = c.lib.games.iter().find(|g| g.id == ids[0] && g.save_tools()).cloned();
                let b = ids.get(1).and_then(|i| c.lib.games.iter().find(|g| &g.id == i && g.save_tools()).cloned()).or(a.clone());
                if let (Some(a), Some(b)) = (a, b) {
                    let mut bv = BoxView::default();
                    let sa = load_side(&a);
                    let sb = if b.id == a.id { sa.clone() } else { load_side(&b) };
                    bv.side = [Some(sa), Some(sb)];
                    let mut page = crate::model::Page::new("Move Pokemon");
                    page.boxes = Some(bv);
                    self.push(page);
                }
            }
            _ => {}
        }
        if let Some(e) = &err { self.show_text("Something went wrong", e); }
        else if test_open {}
        else if (config::rom_folders().is_empty() && !any_playable) || std::env::var("NATIVEMONS_TEST_SETUP").as_deref() == Ok("1") {
            if let Some(c) = self.ctx.as_mut() { c.first_setup = true; }
            self.setup_from = None;
            self.setup_at = self.now();
            let p = crate::setup::setup_page(self.ctx(), 0, false);
            self.push(p);
        } else {
            self.ask_about_encrypted();
            // The one-time wallpaper question below ends at the main screen: the build question follows it instead.
            if pages::wallpaper::chosen() { self.ask_about_recomp(); }
        }
        // Asked once, also on the first start of the version that brought the choice.
        if err.is_none() && !test_open && !self.ctx().first_setup && !pages::wallpaper::chosen() {
            self.setup_from = None;
            self.setup_at = self.now();
            let p = crate::setup::setup_page(self.ctx(), 3, true);
            self.push(p);
        }
        self.dirty = true;
    }
    /// Outro: the splash's last frame fading out over the home screen (0.6 s).
    pub fn outro(&mut self, g: &mut Graphics, w: i32, h: i32) {
        let Some(shot) = self.jobs.splash_shot.as_ref() else { return };
        let k = crate::art::ease((self.now() - self.jobs.outro_at) / 0.6);
        if k >= 1.0 { self.jobs.splash_shot = None; return; }
        let shot = shot.clone();
        g.draw_image_alpha(&shot, RectangleF::new(0.0, 0.0, w as f32, h as f32), 1.0 - k);
    }
}
