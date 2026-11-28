//! 3DSrecomp+Zakuro in the launcher, line by line from the C#: PlayRecomp, AskAboutRecomp, AskAboutEncrypted,
//! Prepare3ds, RefreshSharedHints, ImproveFromShared, BuildRecomp, and the background builds' banner.

use crate::input::Effect;
use crate::launcher::{Launcher, RecompBanner};
use crate::model::{Act, Item, Page};
use crate::pages::{engine_short, theme_of};
use nm_core::game_entry::GameEntry;
use nm_core::library::{self, Library};
use nm_core::{config, ctr_header_fix, recomp, saves};

fn asked_file() -> std::path::PathBuf { config::state().join("recomp-asked.txt") }
fn asked() -> Vec<String> { std::fs::read_to_string(asked_file()).map(|t| config::lines(&t).into_iter().map(String::from).collect()).unwrap_or_default() }
/// Library.Games' 3DS games that 3DSrecomp+Zakuro plays and that haven't been asked about or built.
fn new_3ds(games: &[GameEntry]) -> Vec<GameEntry> {
    let asked = asked();
    games.iter().filter(|g| g.on_recomp() && g.engine() == "recomp" && !asked.iter().any(|a| Some(a) == g.ctr.as_ref()) && recomp::state(g) != "ready" && recomp::state(g) != "old").cloned().collect()
}
fn append_lines(f: &std::path::Path, lines: &[String]) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(config::state());
    if let Ok(mut o) = std::fs::OpenOptions::new().create(true).append(true).open(f) { for l in lines { let _ = write!(o, "{}\r\n", l); } }
}

/// Prepare3ds, behind the splash: only what's quick (Zig, the checks); the builds run after one question.
pub fn prepare_3ds(lib: &Library, stage: &dyn Fn(&str)) {
    if !recomp::installed() { return; }
    refresh_shared_hints(&lib.games);
    let found = new_3ds(&lib.games);
    if found.is_empty() { return; }
    stage("Getting your 3DS games ready");
    if !recomp::zig_exe().is_file() { stage("Getting your 3DS games ready (downloading Zig)"); let _ = recomp::ensure_zig(); }
    for g in &found { let _ = ctr_header_fix::needs(g.ctr_plain_rom.as_deref().unwrap_or("")); }
}
pub fn refresh_shared_hints(games: &[GameEntry]) {
    let _ = recomp::check_shared_hints();
    for g in games.iter().filter(|x| x.on_recomp() && x.engine() == "recomp" && recomp::built(x)) { recomp::learn_code_id(g); }
}

impl Launcher {
    fn dry_recomp(&self) -> bool { self.hooks.replay && self.jobs.dry_run }
    /// Background starts in a dry run are noted, as RunJob's are.
    fn quiet<R>(&mut self, f: impl FnOnce() -> R) -> R {
        let dry = self.dry_recomp();
        if dry { recomp::DRY.lock().unwrap().get_or_insert_with(Vec::new); }
        let r = f();
        if dry { if let Some(d) = recomp::DRY.lock().unwrap().take() { for x in d { self.effects.push(Effect::Launch(x)); } } }
        r
    }
    pub fn build_quietly(&mut self, games: &[GameEntry]) { self.quiet(|| recomp::build_quietly(games)); }
    pub fn improve_quietly(&mut self, g: &GameEntry) { self.quiet(|| recomp::improve_quietly(g)); }

    /// PlayRecomp: a built game plays on Zakuro; one still building, or not built, asks first.
    pub(crate) fn play_recomp(&mut self, g: &GameEntry) {
        let e = nm_core::emu3ds::ZAKURO;
        recomp::swap_in(g);
        if saves::emu_running(&nm_core::proc::processes(), &e) { self.show_text(&format!("Close {} first", e.short), &format!("{} is already open. Close it, then press Play.", e.short)); return; }
        let st = recomp::state(g);
        let unbuilt = st == "missing" || st == "outdated";
        if unbuilt && (recomp::improving(g) || recomp::queued(g)) {
            let mut page = Page::new(&format!("{} is building", g.title));
            page.theme = Some(theme_of(g));
            page.fit_body = true;
            page.body = Some(if recomp::improving(g) { format!("It's being built in the background: {}% so far. It's ready in a few minutes.", (recomp::quiet_progress() as f64 * 100.0).round_ties_even() as i32) } else { "It's waiting its turn to be built in the background, after the game building now.".into() });
            if g.on_3ds() {
                let id = g.id.clone();
                page.items.push(Item::new(&format!("Play with {} this time", engine_short("3ds", Some(g))), "", true).icon('\u{E768}').info("The emulator plays it now; the build carries on.")
                    .run(move |l| { l.pages.pop(); if let Some(g) = l.game(&id) { l.play_3ds(&g); } }));
            }
            page.items.push(Item::new("OK", "", true).icon('\u{E711}').info("The banner shows when it's ready.").run(|l| { l.pages.pop(); }));
            self.push(page);
            return;
        }
        if unbuilt { self.push(crate::pages::recomp_ask_page(&[g], Some(g))); return; }
        self.play_3ds_now(g, e);
    }

    /// AskAboutRecomp: asked once per game, at start and after a scan finds one; after that the game page and Play offer it.
    pub fn ask_about_recomp(&mut self) {
        if self.hooks.replay && !self.jobs.dry_run { self.todo("AskAboutRecomp"); return; }
        if !recomp::installed() { return; }
        if !recomp::busy() { recomp::clear_leftovers(); }
        self.improve_from_shared();
        // For tests: NATIVEMONS_TEST_IMPROVE=<game> starts its quiet rebuild now, to see the banner.
        let want = std::env::var("NATIVEMONS_TEST_IMPROVE").ok();
        if let Some(t) = self.ctx().lib.games.iter().find(|x| Some(&x.id) == want.as_ref()).cloned() { if t.on_recomp() { self.improve_quietly(&t); } }
        let found = new_3ds(&self.ctx().lib.games);
        if found.is_empty() { return; }
        append_lines(&asked_file(), &found.iter().map(|g| g.ctr.clone().unwrap_or_default()).collect::<Vec<_>>());
        let refs: Vec<&GameEntry> = found.iter().collect();
        self.push(crate::more_pages::setup_3ds_page(&refs));
    }
    /// ImproveFromShared: built games whose shared list grew since their build, rebuilt quietly when the launcher is idle.
    pub fn improve_from_shared(&mut self) {
        if !recomp::installed() || self.job || saves::running_games(&self.ctx().lib.games) != "" { return; }
        let old: Vec<GameEntry> = self.ctx().lib.games.iter().filter(|g| g.on_recomp() && g.engine() == "recomp" && recomp::built(g) && recomp::state(g) == "old").cloned().collect();
        if !old.is_empty() { self.build_quietly(&old); }
    }

    /// AskAboutEncrypted: encrypted ROMs are raised once, when found. After that they stay flagged on their game's page.
    pub fn ask_about_encrypted(&mut self) {
        let f = config::state().join("patch-asked.txt");
        let asked: Vec<String> = std::fs::read_to_string(&f).map(|t| config::lines(&t).into_iter().map(String::from).collect()).unwrap_or_default();
        let found: Vec<GameEntry> = self.ctx().lib.games.iter().filter(|g| g.fulldex_rom.as_ref().map_or(false, |r| library::needs_patch(Some(r.as_str())) && !asked.contains(r))).cloned().collect();
        if found.is_empty() { return; }
        append_lines(&f, &found.iter().map(|g| g.fulldex_rom.clone().unwrap_or_default()).collect::<Vec<_>>());
        let mut p = Page::new(&if found.len() == 1 { format!("Patch {}?", found[0].title) } else { format!("Patch {} games?", found.len()) });
        p.body = Some(format!("Found encrypted: {}.\n\nFulldex can't load these without a DS BIOS until their secure area is decrypted. hails.nds-Patcher does that. It changes 2 KB of each ROM file and keeps the original bytes in a backup, so it can be undone.", found.iter().map(|g| g.title.as_str()).collect::<Vec<_>>().join(", ")));
        p.items.push(Item::new("Patch now", "Changes the ROM files, with backups.", true).icon('\u{E72E}').run({ let roms: Vec<String> = found.iter().map(|g| g.fulldex_rom.clone().unwrap_or_default()).collect(); move |l| l.patch_found(roms.clone()) }));
        p.items.push(Item::new("Not now", "Nothing is changed. You can patch from the game's page.", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
        self.push(p);
    }

    /// BuildRecomp: headers fixed and Zig fetched first if needed, then each game built in turn as a job.
    pub fn build_recomp(&mut self, ids: Vec<String>, play: Option<String>) {
        let games: Vec<GameEntry> = ids.iter().filter_map(|i| self.game(i)).collect();
        let unfixed: Vec<&GameEntry> = games.iter().filter(|x| ctr_header_fix::needs(x.ctr_plain_rom.as_deref().unwrap_or(""))).collect();
        if !unfixed.is_empty() {
            let (i2, p2) = (ids.clone(), play.clone());
            // A scan makes new game entries: the build goes on with those.
            let then = Act::new(move |l| l.build_recomp(i2.clone(), p2.clone()));
            self.push(crate::more_pages::header_fix_page(&unfixed, Some(then)));
            return;
        }
        if !recomp::zig_exe().is_file() {
            self.busy("Getting Zig", || Ok(recomp::ensure_zig()), move |l, r| {
                if let Err(x) | Ok(Some(x)) = r { l.show_text("Couldn't get Zig", &format!("{}\n\nNothing was built. Your games still play with an emulator.", x)); return; }
                l.build_recomp(ids, play);
            });
            return;
        }
        if games.is_empty() { self.replace_main(); if let Some(p) = play { self.play(&p); } return; }
        let g = games[0].clone();
        let rest: Vec<String> = games[1..].iter().map(|x| x.id.clone()).collect();
        let mut psi = match recomp::build_start(&g) { Ok(p) => p, Err(e) => { self.show_text(&format!("Couldn't build {}", g.title), &e.to_string()); return; } };
        psi.idle = Some(recomp::mask());
        let stopped = std::rc::Rc::new(std::cell::Cell::new(false));
        let s2 = stopped.clone();
        let (gid, title) = (g.id.clone(), g.title.clone());
        self.run_job(&format!("Building {}", g.title), psi, false, Box::new(move |l, code| {
            let ok = recomp::finish(&g, code);
            if l.pages.len() > 1 && !l.pages.last().unwrap().main && l.pages.last().unwrap().title == title { l.rebuild_game(&gid, true, crate::pages::game_page); }
            if s2.get() { l.show_text("Build stopped", &format!("{} wasn't built. It starts over next time.", title)); return; }
            if !ok { l.show_log(&format!("Couldn't build {}", title), "Your dump wasn't changed, and an emulator can still play it.", None); return; }
            l.build_recomp(rest, play);
        }));
        if !self.job { return; }
        let Some(pid) = self.jobs.proc.as_ref().map(|p| p.id()) else { return };
        self.jobs.cancel = Some(Act::new(move |l| { stopped.set(true); l.jobs.cancel = None; recomp::stop(pid); }));
        self.add_line("nativemons: 0 Esc or B stops the build");
    }

    /// The quiet builds' banner, read from Recomp each tick (the snapshot's fixed one is kept while nothing runs).
    pub fn recomp_tick(&mut self) {
        // Background engines close with the power-off, not after it.
        if (self.power_off >= 0.0 || self.closing) && !self.jobs.quiet_stopped { self.jobs.quiet_stopped = true; nm_core::art_jobs::stop_now(); recomp::stop_quiet(); }
        nm_core::art_jobs::set_player_busy(self.job);
        if !self.hooks.replay && self.ctx.is_some() {
            // A launcher left open still finds out (the C# does it with the 6-hourly update check).
            let now = std::time::Instant::now();
            let due = self.jobs.hints_at.map_or(false, |at| now.duration_since(at).as_secs() >= 6 * 3600);
            if self.jobs.hints_at.is_none() { self.jobs.hints_at = Some(now); }
            if due && !self.job && recomp::installed() && self.jobs.hints_rx.is_none() {
                self.jobs.hints_at = Some(now);
                let games = self.ctx().lib.games.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || { refresh_shared_hints(&games); let _ = tx.send(()); });
                self.jobs.hints_rx = Some(rx);
            }
            if self.jobs.hints_rx.as_ref().map_or(false, |r| r.try_recv().is_ok()) { self.jobs.hints_rx = None; self.improve_from_shared(); }
        }
        let b = recomp::banner();
        let Some(title) = b.title.clone() else { return };
        let wall = self.hooks.wall_seconds();
        self.recomp_banner = Some(RecompBanner { title, verb: b.verb.clone(), progress: b.progress, done_at_wall: b.done_ago.map_or(f64::NEG_INFINITY, |a| wall - a), fake_running: b.running, paused: b.paused });
        if recomp::banner_showing() { self.dirty = true; }
    }

    /// ShareHintsPage's Share: the list goes to the submission service over HTTPS. A replay sends nothing: it notes it
    /// and gets the C# replay's receipt.
    pub fn share_send(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let replay = self.hooks.replay;
        let (title, gw) = (g.title.clone(), g.clone());
        self.busy("Sending your list", move || {
            let Some(code) = recomp::code_id(&gw) else { return Ok((false, Err("The game's code version couldn't be read.".to_string()))) };
            if replay { return Ok((true, Ok("a replay".to_string()))); }
            let lines: Vec<String> = std::fs::read_to_string(recomp::hints_file(&gw)).map(|t| config::lines(&t).into_iter().map(String::from).collect()).map_err(|e| e.to_string())?;
            Ok((true, nm_core::submit::zakuro_hints(&code, &lines)))
        }, move |l, r| {
            if replay && r.as_ref().map_or(false, |x| x.0) { l.todo("Submit.ZakuroHints"); }
            match r.and_then(|x| x.1) {
                Err(x) => l.show_text("Not sent", &format!("Your list wasn't sent: {}.\n\nIt's kept on this PC; you can share it again from the game's page, or through GitHub.", x)),
                Ok(receipt) => l.show_text("Sent, thank you!", &format!("Your findings for {} are on their way. They're checked, then go into the shared lists every launcher downloads.\n\nReceipt: {}", title, receipt)),
            }
        });
    }
    /// ShareHintsPage's GitHub way: the list as a file, Explorer on it, and the issue form in the browser.
    pub fn share_github(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let replay = self.hooks.replay;
        let gw = g.clone();
        self.busy("Preparing the list", move || {
            let id = recomp::code_id(&gw);
            let file = match &id { Some(c) if !replay => Some(recomp::share_file(&gw, c).map_err(|e| e.to_string())?), _ => None };
            Ok((id, file))
        }, move |l, r| {
            if replay && r.as_ref().map_or(false, |x| x.0.is_some()) { l.todo("Recomp.ShareFile"); }
            let (id, file) = match r { Ok((Some(i), Some(f))) => (i, f), Ok(_) => { l.show_text("Couldn't prepare it", "The game's code version couldn't be read."); return; } Err(x) => { l.show_text("Couldn't prepare it", &x); return; } };
            l.effects.push(Effect::OpenFolder(file.display().to_string()));
            let url = format!("https://github.com/Hailey-Ross/nativeMons/issues/new?template=zakuro-hints.yml&title={}&game={}&code={}&versions={}",
                escape_data(&format!("Zakuro hints: {} {}", g.title, id)), escape_data(&g.title), escape_data(&id), escape_data(&recomp::share_versions()));
            l.effects.push(Effect::OpenUrl(url.clone()));
            // The browser opening is the platform's; a replay notes it instead, as the C#'s Skip (opened stays false).
            let opened = !l.hooks.replay;
            // The list goes on the clipboard to paste into the form; one too long for a GitHub text box is attached as the file.
            let list = std::fs::read_to_string(&file).unwrap_or_default();
            let pasted = opened && !list.is_empty() && list.encode_utf16().count() < 60000;
            if !l.hooks.replay { l.effects.push(Effect::Clipboard(if pasted { list } else { url })); }
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let how = if pasted { format!("Your list is on your clipboard: click the form's \".hints file\" box and press Ctrl+V. (It's also in the file Explorer shows, {}, if you'd rather drag it in.)", name) }
                else { format!("Drag the file Explorer shows, {}, into the form's \".hints file\" box.", name) };
            l.show_text("Almost done", &format!("{}Sign in to GitHub if it asks. {} Then submit it. Thank you!", if opened { "Your browser shows the form. " } else { "Your browser didn't open, so the form's link is on your clipboard: paste it into your browser's address bar. " }, how));
        });
    }
}

/// Uri.EscapeDataString: everything but the unreserved characters, as UTF-8 percent escapes.
fn escape_data(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}
