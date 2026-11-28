//! The job model and the play flows, line by line from the C# (RunJob, JobEnded, Busy, WatchGame, AddLine, Play,
//! BeforeGame, SyncThen, Import, PlayGen1, PlayFulldex, PlayGba, PlayPort, PlayNativeplat, Play3dsNow, PlayPc,
//! Finished, AfterGame). Processes start through the platform's Spawner; a replay with NATIVEMONS_TEST_DRYRUN=1
//! notes each start (Effect::Launch) instead, as the C# dry run does.

use crate::input::Effect;
use crate::launcher::Launcher;
use crate::model::Act;
use nm_core::game_entry::GameEntry;
use nm_core::play_prep as pp;
use nm_core::proc::{RunningJob, Spawner, StartInfo};
use nm_core::{config, save_guard, saves};
use std::path::Path;

pub type Done = Box<dyn FnOnce(&mut Launcher, i32)>;
type BusyPoll = Box<dyn FnMut(&mut Launcher) -> bool>;

/// RunJob's and Busy's state beside the Launcher's own job fields (job, job_title, progress, log).
#[derive(Default)]
pub struct Jobs {
    pub proc: Option<Box<dyn RunningJob>>,
    pub done: Option<Done>,
    pub cancel: Option<Act>,
    pub hid: bool,
    pub busy: Option<BusyPoll>,
    pub spawner: Option<Box<dyn Spawner>>,
    /// NATIVEMONS_TEST_DRYRUN=1 (only acted on in a replay).
    pub dry_run: bool,
    pub game_start_wall: f64,
    pub notice_after: Option<String>,
    pub notice_title: Option<String>,
    pub waiting: Option<String>,
    pub scanned_at: Option<std::time::Instant>,
    /// last-session's direct play (playDirect): closes after the game.
    pub play_direct: Option<String>,
    /// --slot (directSlot) and --launcher's game (chooseId), used once the start is done.
    pub direct_slot: Option<String>,
    pub choose: Option<String>,
    /// Sprites.Arrived when last seen: a picture that arrived redraws the window.
    pub sprites_seen: u32,
    /// Start's work off the UI thread, and the splash's last frame for the Outro.
    pub start: Option<crate::start::Starting>,
    pub splash_shot: Option<nm_draw::Bitmap>,
    pub outro_at: f32,
    /// The working area of the screen the window is on (x, y, w, h), for Zakuro's window; the live loop sets it.
    pub work_rect: Option<(i32, i32, i32, i32)>,
    /// Recomp.StopQuiet once the launcher starts closing; the 6-hourly shared-hints check and its end.
    pub quiet_stopped: bool,
    pub hints_at: Option<std::time::Instant>,
    pub hints_rx: Option<std::sync::mpsc::Receiver<()>>,
    /// PlayPc: whether the session goes on in its folder's programs after the first one exits (asked at JobEnded).
    pub folder_wait: Option<Box<dyn FnOnce() -> bool>>,
    /// That answer, for the job's done callback; paused builds resume only after that wait.
    pub in_folder_wait: bool,
    /// Quick start's waiting game and title-screen snapshots (quick.rs).
    pub quick: crate::quick::Quick,
    /// ReadClashes, WatchGba (extras.rs).
    pub extra: crate::extras::Extras,
    /// GuideTick's last reading of the guide button.
    pub guide_was: bool,
    /// The open dialog's continuation (dialogs.rs).
    pub dialog: Option<crate::dialogs::Then>,
    /// busy_reporting's lines from the worker.
    pub reports: Option<std::sync::mpsc::Receiver<(String, f32)>>,
    /// A replay's answers for the next dialogs (the replay's `answer` command); None answers Cancel.
    pub answers: std::collections::VecDeque<Option<String>>,
}

impl Launcher {
    fn dry(&self) -> bool { self.hooks.replay && self.jobs.dry_run }
    pub(crate) fn games(&self) -> &[GameEntry] { &self.ctx().lib.games }
    pub(crate) fn game(&self, id: &str) -> Option<GameEntry> { self.game_index(id).map(|i| self.ctx().lib.games[i].clone()) }
    fn show_text_back(&mut self, title: &str, body: &str, back: Option<Act>, mono: Option<String>) {
        let mut p = crate::pages::show_text(title, body);
        p.mono = mono;
        p.on_back = back;
        self.push(p);
    }
    /// ShowLog: the text with the job's last 12 lines.
    pub(crate) fn show_log(&mut self, title: &str, body: &str, back: Option<Act>) {
        let last: Vec<String> = self.log.iter().skip(self.log.len().saturating_sub(12)).cloned().collect();
        self.show_text_back(title, body, back, if last.is_empty() { None } else { Some(last.join("\n")) });
    }

    // ---------- the job model ----------
    pub fn run_job(&mut self, title: &str, psi: StartInfo, is_game: bool, done: Done) {
        if self.dry() { self.effects.push(Effect::Launch(nm_core::proc::dry_line(title, &psi, is_game))); return; }
        self.jobs.cancel = None;
        self.jobs.folder_wait = None;
        self.log.clear();
        self.job_title = title.to_string();
        self.job_is_game = is_game;
        self.jobs.hid = false;
        self.job_start_wall = self.hooks.wall_seconds();
        if is_game { self.jobs.game_start_wall = self.job_start_wall; }
        self.progress = -1.0;
        self.jobs.done = Some(done);
        let started = match self.jobs.spawner.as_mut() {
            Some(s) => s.start(&psi),
            None => Err(std::io::Error::other("Programs can't be started from here.")),
        };
        match started {
            Ok(p) => {
                self.jobs.proc = Some(p);
                self.job = true;
                // Light mode: background 3DS builds pause while a game plays (user, 2026-11-25; Rust only).
                if is_game && self.light { nm_core::recomp::set_paused(true); }
                if is_game { let cur = self.ctx.as_ref().and_then(|c| c.current.map(|i| c.lib.games[i].clone())); self.discord_playing(cur.as_ref()); }
            }
            Err(e) => { self.show_text_back("Could not start", &format!("{}\n\n{}", psi.program.display(), e), None, None); return; }
        }
        self.dirty = true;
    }
    /// Gen1Recomp's import reports "nativemons: <percent> <text>".
    pub fn add_line(&mut self, raw: &str) {
        let mut c = raw.trim().to_string();
        if c.is_empty() { return; }
        if let Some(rest) = c.strip_prefix("nativemons: ") {
            c = rest.to_string();
            if let Some(sp) = c.find(' ').filter(|&sp| sp > 0) {
                if let Ok(pct) = c[..sp].parse::<i32>() { self.progress = pct as f32 / 100.0; c = c[sp + 1..].to_string(); }
                else if let Some(r) = c.strip_prefix("error ") { c = r.to_string(); }
            } else if let Some(r) = c.strip_prefix("error ") { c = r.to_string(); }
            self.log.push(c);
        } else if self.job_is_game { self.log.push(c); }
        if self.log.len() > 400 { self.log.remove(0); }
    }
    pub fn job_ended(&mut self, mut code: i32) {
        self.jobs.proc = None;
        self.job = false;
        self.jobs.cancel = None;
        // The game is over: paused background builds go on (and the queue), before the after-play steps; a PC game
        // still running from its folder resumes them after that wait.
        self.jobs.in_folder_wait = self.jobs.folder_wait.take().map_or(false, |f| f());
        if self.job_is_game && !self.jobs.in_folder_wait { nm_core::recomp::set_paused(false); }
        if self.job_is_game { nm_core::discord::stop(); self.read_clashes(); }
        if self.jobs.extra.gba_closed { code = 0; }
        self.jobs.extra.watch_gba = false;
        self.jobs.extra.gba_closed = false;
        self.effects.push(Effect::ShowWindow);
        if let Some(then) = self.jobs.done.take() { then(self, code); }
        self.dirty = true;
    }
    /// A short wait shown as a card, so the window never freezes. In a replay the work runs at once, as the C# replay does.
    pub fn busy<T: Send + 'static>(&mut self, title: &str, work: impl FnOnce() -> Result<T, String> + Send + 'static, then: impl FnOnce(&mut Launcher, Result<T, String>) + 'static) {
        if self.hooks.replay {
            let r = work();
            self.core_notes();
            then(self, r);
            self.dirty = true;
            return;
        }
        self.jobs.reports = None;
        self.job = true;
        self.job_title = title.to_string();
        self.job_is_game = false;
        self.job_start_wall = self.hooks.wall_seconds();
        self.progress = -1.0;
        self.log.clear();
        self.dirty = true;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_| Err("Something went wrong.".into())); let _ = tx.send(r); });
        let mut then = Some(then);
        self.jobs.busy = Some(Box::new(move |l: &mut Launcher| {
            let r = match rx.try_recv() {
                Ok(r) => r,
                Err(std::sync::mpsc::TryRecvError::Empty) => return false,
                Err(_) => Err("Something went wrong.".into()),
            };
            l.job = false;
            if let Some(t) = then.take() { t(l, r); }
            l.dirty = true;
            true
        }));
    }
    /// Busy with the work's own progress: each (line, share) it reports reaches the card as the C#'s BeginInvoke does
    /// (AddLine "nativemons: <line>", progress when the share isn't negative). A replay has no message pump, so the C#
    /// replay never shows them; here they're dropped too.
    pub fn busy_reporting<T: Send + 'static>(&mut self, title: &str, work: impl FnOnce(&mut dyn FnMut(&str, f32)) -> Result<T, String> + Send + 'static, then: impl FnOnce(&mut Launcher, Result<T, String>) + 'static) {
        if self.hooks.replay { self.busy(title, move || work(&mut |_, _| {}), then); return; }
        let (tx, rx) = std::sync::mpsc::channel::<(String, f32)>();
        self.busy(title, move || work(&mut |w, at| { let _ = tx.send((w.to_string(), at)); }), then);
        self.jobs.reports = Some(rx);
    }
    fn drain_reports(&mut self) {
        let Some(rx) = self.jobs.reports.as_ref() else { return };
        let got: Vec<(String, f32)> = rx.try_iter().collect();
        for (what, at) in got { if at >= 0.0 { self.progress = at; } self.add_line(&format!("nativemons: {}", what)); self.dirty = true; }
    }
    /// Each tick: a Busy card's end, the job's output and end, and (every 250 ms) WatchGame and the waiting game.
    pub fn job_tick(&mut self) {
        let arrived = crate::sprites::ARRIVED.load(std::sync::atomic::Ordering::SeqCst);
        if arrived != self.jobs.sprites_seen { self.jobs.sprites_seen = arrived; self.dirty = true; }
        self.recomp_tick();
        if self.power_off < 0.0 && !self.closing { self.watch_gba(); }
        self.clash_poll();
        self.drain_reports();
        if let Some(mut b) = self.jobs.busy.take() { if !b(self) && self.jobs.busy.is_none() { self.jobs.busy = Some(b); } }
        if let Some(p) = self.jobs.proc.as_mut() {
            let lines = p.lines();
            let code = p.exit_code();
            for l in lines { self.add_line(&l); self.dirty = true; }
            if let Some(code) = code { self.job_ended(code); }
        }
        if self.hooks.replay { return; }
        self.quick_poll();
        // Background engines (waiting game, snapshot) close with the power-off, not after it.
        if (self.power_off >= 0.0 || self.closing) && !self.jobs.quick.stopped_for_close { self.jobs.quick.stopped_for_close = true; self.quick_stop_all(false); }
        if self.power_off >= 0.0 || self.closing { return; }
        let now = std::time::Instant::now();
        if self.jobs.scanned_at.map_or(true, |at| now.duration_since(at).as_millis() >= 250) {
            self.jobs.scanned_at = Some(now);
            if let Some(id) = self.jobs.waiting.clone() {
                if !self.job && saves::running_games(self.games()) == "" { self.jobs.waiting = None; self.pages.pop(); self.play(&id); }
            }
            self.watch_game();
            self.quick_start_tick();
        }
    }
    /// Hide once the game's window shows, so it gets the foreground and its controller layout.
    fn watch_game(&mut self) {
        if !self.job || !self.job_is_game || self.jobs.hid { return; }
        let Some(p) = self.jobs.proc.as_mut() else { return };
        if !p.window_shown() { return; }
        p.front();
        self.jobs.hid = true;
        self.effects.push(Effect::HideWindow);
    }
    /// The job cancel key (B, Escape, Back) while a job that can stop runs.
    pub fn job_cancel_key(&mut self) -> bool {
        if !self.job { return false; }
        match self.jobs.cancel.clone() { Some(c) => { c.call(self); true } None => false }
    }

    // ---------- play ----------
    /// AfterCapture: a wallpaper capture runs an engine too; it stops after this picture, then `then` carries on.
    pub fn after_capture(&mut self, then: impl FnOnce(&mut Launcher) + 'static) -> bool {
        if !nm_core::art_jobs::capturing() { return false; }
        nm_core::art_jobs::stop_now();
        self.busy("Just a moment", || { while nm_core::art_jobs::capturing() { std::thread::sleep(std::time::Duration::from_millis(200)); } Ok(()) }, move |l, _: Result<(), String>| then(l));
        true
    }    pub fn play(&mut self, id: &str) {
        if !self.jobs.dry_run && self.hooks.replay { self.todo(&format!("Play {}", id)); return; }
        let Some(g) = self.game(id) else { return };
        if !g.playable() { return; }
        // The waiting game is kept through everything before it starts; a snapshot being made stops (the game gets the PC).
        self.quick_hold_for(&g);
        { let again = g.id.clone(); if self.after_capture(move |l| l.play(&again)) { return; } }
        let running = saves::running_games(self.games());
        if !running.is_empty() {
            self.jobs.waiting = Some(g.id.clone());
            self.show_text_back("Waiting", &format!("Already running: {}\nOnly one game at a time. Close it, or wait here: this continues by itself.\n\nB: cancel", running), Some(Act::new(|l| { l.jobs.waiting = None; l.pages.pop(); })), None);
            return;
        }
        if let Some(i) = self.game_index(&g.id) { if let Some(c) = self.ctx.as_mut() { c.current = Some(i); } }
        config::write("last-game.txt", &g.id);
        let id = g.id.clone();
        match g.engine().as_str() {
            "3ds" => self.play_3ds(&g),
            "recomp" => self.play_recomp(&g),
            "pc" => self.busy("Backing up your saves", { let g = g.clone(); move || { let _ = pp::pc_backup(&g); Ok(()) } }, move |l, _| l.play_pc(&id)),
            "fulldex" => {
                if nm_core::library::needs_patch(g.fulldex_rom.as_deref()) { self.push(crate::more_pages::patch_page(&g, true)); return; }
                self.before_game(&g, Box::new(move |l| l.play_fulldex(&id)));
            }
            "port" => self.before_game(&g, Box::new(move |l| l.play_port(&id))),
            "nativeplat" => self.before_game(&g, Box::new(move |l| l.play_nativeplat(&id))),
            _ if !g.imported => self.import(&g, Some(Box::new(move |l: &mut Launcher| { let id2 = id.clone(); let g = l.game(&id); if let Some(g) = g { l.before_game(&g, Box::new(move |l| l.play_gen1(&id2))); } }))),
            _ => self.before_game(&g, Box::new(move |l| l.play_gen1(&id))),
        }
    }
    /// Request: back to the home screen, then play the game (a shortcut while open) or show its page.
    pub fn request(&mut self, what: &str, id: &str, slot: Option<&str>) {
        while self.pages.len() > 1 && !self.pages.last().map_or(false, |p| p.main) { self.pages.pop(); }
        if !self.game(id).map_or(false, |g| g.playable()) {
            self.show_text(&format!("Can't start {}", id), "That game is not in the list, or its ROM was not found.");
            return;
        }
        self.effects.push(Effect::ShowWindow);
        if what == "play" { self.play_with_slot(id, slot, None); return; }
        let i = self.game_index(id).unwrap();
        self.ctx.as_mut().unwrap().current = Some(i);
        self.current_theme = Some(crate::pages::theme_of(&self.ctx().lib.games[i]));
        self.replace_main();
        self.push_with(|c| crate::pages::game_page(c, &c.lib.games[i]));
        self.dirty = true;
    }
    /// PlayWithSlot: a save named by its number or its name, switched to first as the Saves page does.
    pub fn play_with_slot(&mut self, id: &str, slot: Option<&str>, failed: Option<Act>) {
        use nm_core::save_sets;
        let Some(g) = self.game(id) else { return };
        let Some(slot) = slot else { self.play(id); return };
        if !g.save_tools() { self.show_text_back("Can't pick a save", &format!("{} keeps its saves itself.", g.title), failed, None); return; }
        let all = save_sets::all(&g);
        let Some(set) = all.iter().find(|x| x.as_str() == slot).or_else(|| all.iter().find(|x| save_sets::name(&g, x).to_lowercase() == slot.to_lowercase())).cloned() else {
            let has: Vec<String> = all.iter().map(|x| format!("{} ({})", save_sets::name(&g, x), x)).collect();
            self.show_text_back(&format!("No save called {}", slot), &format!("{} has: {}.", g.title, has.join(", ")), failed, None);
            return;
        };
        if set == save_sets::active(&g) { self.play(id); return; }
        let games = self.games().to_vec();
        let id = id.to_string();
        self.busy("Switching saves", move || Ok(saves::switch_save(&games, &g, &set)), move |l, r| {
            l.replace_main();
            match r { Err(e) | Ok(Some(e)) => l.show_text_back("Couldn't switch saves", &e, failed, None), Ok(None) => l.play(&id) }
        });
    }
    fn before_game(&mut self, g: &GameEntry, start: Box<dyn FnOnce(&mut Launcher)>) {
        let gw = g.clone();
        let id = g.id.clone();
        self.busy("Backing up your save", move || {
            let file = if gw.imported || gw.engine() != "gen1" { saves::save_of(&gw).0 } else { None };
            let _ = save_guard::backup(&gw, file.as_deref().map(Path::new), "before-play");
            Ok(())
        }, move |l, _| {
            let Some(g) = l.game(&id) else { return };
            l.sync_then(&g, Box::new(move |l| {
                let Some(g) = l.game(&id) else { return };
                l.busy("Setting game options", move || { pp::apply_options(&g); Ok(()) }, move |l, _| start(l));
            }));
        });
    }
    pub fn sync_then(&mut self, g: &GameEntry, then: Box<dyn FnOnce(&mut Launcher)>) {
        if !saves::shares(g) || !g.imported { then(self); return; }
        let gw = g.clone();
        let (id, title) = (g.id.clone(), g.title.clone());
        self.busy(&format!("Sharing your {} save", g.title), move || Ok(saves::sync_save(&gw, None)), move |l, r| {
            let then = std::rc::Rc::new(std::cell::RefCell::new(Some(then)));
            let call = { let then = then.clone(); move |l: &mut Launcher| { if let Some(t) = then.borrow_mut().take() { t(l); } } };
            if let Ok(res) = &r {
                if res.conflict {
                    let Some(g) = l.game(&id) else { return };
                    l.push(crate::save_pages::conflict_page(&g, Some(Act::new(call))));
                    return;
                }
            }
            let note = match &r { Err(e) => Some(e.clone()), Ok(res) => res.note.clone() };
            if let Some(n) = note { config::write("last-session.txt", &format!("{}: {}", title, n)); l.jobs.notice_after = Some(n); }
            call(l);
        });
    }
    pub fn import(&mut self, g: &GameEntry, then: Option<Box<dyn FnOnce(&mut Launcher)>>) {
        if !self.jobs.dry_run && self.hooks.replay { self.todo("Import"); return; }
        let rom = match nm_core::engines::PlainPath::new(g.gen1_rom.as_deref().unwrap_or("")) { Ok(r) => r, Err(e) => { self.show_text_back("Could not prepare", &e.to_string(), None, None); return; } };
        let psi = pp::gen1_import_start(&rom.path);
        let (id, title) = (g.id.clone(), g.title.clone());
        self.run_job(&format!("Preparing {}", g.title), psi, false, Box::new(move |l, code| {
            drop(rom);
            if code != 0 { l.show_log(&format!("Could not prepare {}", title), "Your ROM was not changed.", None); return; }
            if let Some(i) = l.game_index(&id) { if let Some(c) = l.ctx.as_mut() { c.lib.games[i].imported = true; } }
            match then { Some(t) => t(l), None => l.show_text_back("Ready", &format!("{} is prepared.", title), None, None) }
        }));
    }
    fn quick_start(&self) -> bool { config::read("quick-start.txt", "on").trim() != "off" }
    fn quick_note(&self, what: &str) {
        if let Ok(f) = std::env::var("NATIVEMONS_TEST_LOG") { use std::io::Write; if let Ok(mut o) = std::fs::OpenOptions::new().create(true).append(true).open(f) { let _ = write!(o, "quick-start: {}\r\n", what); } }
    }
    fn play_gen1(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let id = g.id.clone();
        // The waiting game is used only if its options and save haven't changed since it loaded.
        let Err(done) = self.use_or_drop_ready(&g, Box::new(move |l, code| l.finished(&id, code))) else { return };
        let psi = pp::gen1_play_start(&g, self.quick_start(), false);
        self.run_job(&g.title, psi, true, done);
    }
    fn play_fulldex(&mut self, id: &str) {
        let Some(mut g) = self.game(id) else { return };
        let dir = config::fulldex_dir();
        let saves_dir = dir.join("saves");
        let _ = std::fs::create_dir_all(&saves_dir);
        let _ = pp::answer_fulldex_setup();
        let mut psi;
        if g.is_ds() {
            let (p, note) = pp::fulldex_ds_start(&g, self.quick_start());
            if let Some(n) = note { self.quick_note(&n); }
            psi = p;
        } else if config::gba_player().is_file() {
            self.play_gba(&g, &saves_dir);
            return;
        } else {
            // GBA games are only found in Fulldex's roms folder. A copy goes there, the original stays put.
            let rom = g.fulldex_rom.clone().unwrap_or_default();
            let roms = dir.join("roms");
            let copy = roms.join(Path::new(&rom).file_name().unwrap_or_default());
            let _ = std::fs::create_dir_all(&roms);
            let same = |a: &Path, b: &Path| a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy());
            if !same(Path::new(&rom), &copy) && !copy.is_file() { let _ = std::fs::copy(&rom, &copy); }
            // Another dump of this game in its folder could be run instead: only copies go, whose original is still in place.
            let others: Vec<usize> = g.roms.iter().enumerate().filter(|(_, r)| r.engine_copy && !same(Path::new(&r.file), &copy)).map(|(i, _)| i).collect();
            for i in others.into_iter().rev() { if std::fs::remove_file(&g.roms[i].file).is_ok() { g.roms.remove(i); } }
            let chosen_sha = g.chosen_rom().map(|r| r.sha1.clone()).unwrap_or_default();
            for mine in g.roms.iter().filter(|r| !r.engine_copy && r.sha1 != chosen_sha) {
                let stale = roms.join(Path::new(&mine.file).file_name().unwrap_or_default());
                if stale.is_file() && !same(&stale, &copy) && nm_core::library::sha1_of(&stale.to_string_lossy()).ok().as_deref() == Some(mine.sha1.as_str()) { let _ = std::fs::remove_file(&stale); }
            }
            // A hack shares its base game's code, so --game would start the clean game: its list starts the hack instead.
            let modified = g.chosen_rom().map_or(false, |r| g.is_modified(r));
            let args = if g.is_hack() || modified { String::new() } else { format!("--game {}", g.fulldex_code.as_deref().unwrap_or("")) };
            let mut press = g.is_hack() || modified;
            if press && std::fs::write(dir.join("last-game.txt"), format!("{}/roms/{}\r\n", dir.display(), Path::new(&rom).file_name().unwrap_or_default().to_string_lossy())).is_err() { press = false; }
            psi = StartInfo::new(config::fulldex(), &args);
            psi.cwd = Some(dir.clone());
            // That key press must never answer an update prompt, so update checks go nowhere.
            if press { psi.set_env("FULLDEX_UPDATE_URL", "http://127.0.0.1:9/"); }
            self.jobs.extra.watch_gba = true;
            self.jobs.extra.gba_playing = false;
            self.jobs.extra.gba_closed = false;
            // A romhack is made Fulldex's last game played, then started with one key press.
            self.jobs.extra.gba_press = press;
            self.jobs.extra.gba_presses = 0;
            self.jobs.extra.gba_press_at = None;
        }
        if let Some(i) = self.game_index(&g.id) { if let Some(c) = self.ctx.as_mut() { c.lib.games[i].roms = g.roms.clone(); } }
        let id = g.id.clone();
        self.run_job(&g.title, psi, true, Box::new(move |l, code| l.finished(&id, code)));
    }
    fn play_gba(&mut self, g: &GameEntry, saves_dir: &Path) {
        // The player can't open a path with accents ("PokÃƒÂ©mon - LeafGreen Version"): it gets a plain one.
        let rom = match nm_core::engines::PlainPath::new(g.fulldex_rom.as_deref().unwrap_or("")) { Ok(r) => r, Err(e) => { self.show_text_back("Could not start", &e.to_string(), None, None); return; } };
        let mut base_rom: Option<nm_core::engines::PlainPath> = None;
        let games = self.games().to_vec();
        let psi = pp::gba_player_start(g, &games, saves_dir, &rom.path, &mut |f| { base_rom = nm_core::engines::PlainPath::new(f).ok(); base_rom.as_ref().map(|b| b.path.clone()) });
        let id = g.id.clone();
        self.run_job(&g.title, psi, true, Box::new(move |l, code| { drop(rom); drop(base_rom); l.finished(&id, code); }));
    }
    fn play_port(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let (gw, id, title) = (g.clone(), g.id.clone(), g.title.clone());
        self.busy(&format!("Getting {} ready", g.title), move || pp::prepare_port(&gw), move |l, r| {
            if let Err(e) = r { l.show_text_back(&format!("Could not start {}", title), &format!("Nothing was changed.\n\n{}", e), None, None); return; }
            l.run_job(&title, pp::port_start(), true, Box::new(move |l, code| {
                let Some(g) = l.game(&id) else { return };
                let title = g.title.clone();
                l.busy(&format!("Saving {}", title), move || Ok(pp::save_from_port(&g)), move |l, r| {
                    match r { Err(e) => l.jobs.notice_after = Some(e), Ok(Some(n)) => l.jobs.notice_after = Some(n), Ok(None) => {} }
                    l.finished(&id, code);
                });
            }));
        });
    }
    fn play_nativeplat(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let (gw, id, title) = (g.clone(), g.id.clone(), g.title.clone());
        self.busy(&format!("Getting {} ready", g.title), move || pp::prepare_nativeplat(&gw), move |l, r| {
            if let Err(e) = r { l.show_text_back(&format!("Could not start {}", title), &format!("Nothing was changed.\n\n{}", e), None, None); return; }
            let Some(g) = l.game(&id) else { return };
            let done: Done = Box::new(move |l, code| {
                let Some(g) = l.game(&id) else { return };
                l.busy(&format!("Saving {}", g.title), { let g = g.clone(); move || Ok(pp::save_from_nativeplat(&g)) }, move |l, r| {
                    match r { Err(e) => l.jobs.notice_after = Some(e), Ok(Some(n)) => l.jobs.notice_after = Some(n), Ok(None) => {} }
                    l.finished(&g.id, code);
                });
            });
            // The game kept waiting is used only if its slot and options are still what it read while starting.
            let Err(done) = l.use_or_drop_ready(&g, done) else { return };
            let psi = pp::nativeplat_start(&g, if l.quick_start() { Some("title") } else { None });
            l.run_job(&g.title, psi, true, done);
        });
    }
    pub(crate) fn play_3ds(&mut self, g: &GameEntry) {
        let Some(e) = nm_core::emu3ds::for_game(Some(g)) else { return };
        if saves::emu_running(&nm_core::proc::processes(), &e) { self.show_text_back(&format!("Close {} first", e.short), &format!("{} is already open. Close it, then press Play.", e.short), None, None); return; }
        // An update is asked about before every play, until the player stops reminders for that one update.
        let ready = e.installed() && e.tag.is_some() && e.installed_tag().as_deref() != e.tag;
        if ready && config::read(&format!("emu-update-skipped-{}.txt", e.id), "") != e.tag.unwrap_or("") { self.push(crate::pages::emu_update_ask_page(&e, g, false)); return; }
        self.play_3ds_now(g, e);
    }
    pub(crate) fn play_3ds_now(&mut self, g: &GameEntry, e: nm_core::emu3ds::Emu3ds) {
        let games = self.games().to_vec();
        let (gw, id, title) = (g.clone(), g.id.clone(), g.title.clone());
        self.busy("Backing up your save", move || {
            nm_core::ctr3ds_saves::prepare(&games, &e, &gw).map_err(|x| { let _ = nm_core::ctr3ds_saves::unlink(&e); x.to_string() })
        }, move |l, r| {
            if let Err(x) = r { l.show_text_back(&format!("Could not start {}", title), &format!("Its save couldn't be handed to {}, so nothing was started. {}", e.short, x), None, None); return; }
            let Some(g) = l.game(&id) else { return };
            let psi = if e.native {
                let (psi, note) = pp::zakuro_start(&g, &e, l.work_rect(), l.quick_start());
                if let Some(n) = note { l.quick_note(&n); }
                psi
            } else { pp::emu3ds_start(&g, &e) };
            l.run_job(&g.title, psi, true, Box::new(move |l, code| {
                // The emulator's own output, kept for when something didn't work (one file per emulator, last session only).
                let _ = config::write_all_lines(&nm_core::ctr3ds_saves::emu_data(&e).join("nativemons-last-output.txt"), &l.log);
                let Some(g) = l.game(&id) else { return };
                l.busy("Collecting your save", { let g = g.clone(); move || Ok(nm_core::ctr3ds_saves::collect(&e, &g)) }, move |l, r| {
                    use nm_core::recomp;
                    match r {
                        Err(y) => l.jobs.notice_after = Some(y),
                        Ok(Some(k)) => l.jobs.notice_after = Some(k),
                        Ok(None) if e.native && code == 0 && recomp::state(&g) == "old" && !l.closing => {
                            l.improve_quietly(&g);
                            let told = format!("recomp-improve-told-{}.txt", g.id);
                            if recomp::improving(&g) && config::read(&told, "") != recomp::hint_count(&g).to_string() {
                                config::write(&told, &recomp::hint_count(&g).to_string());
                                l.jobs.notice_title = Some("Improving in the background".into());
                                l.jobs.notice_after = Some(format!("{} is being improved in the background: this session found code its build didn't cover, and a quiet rebuild covers it. Your next play uses it.", g.title));
                            }
                        }
                        Ok(None) => {}
                    }
                    l.finished(&g.id, code);
                    if e.native && code == 0 && recomp::should_ask_share(&g) { l.push(crate::more_pages::share_hints_page(&g, true)); }
                });
            }));
            // Wallpapers from this session's play, while it runs.
            if !crate::game_art::wallpaper_drawn() {
                if l.job && l.job_is_game { if let Some(p) = l.jobs.proc.as_ref() { crate::play_snaps::watch(p.id(), g.id.clone()); } }
                else if l.dry() { l.effects.push(Effect::Skip(format!("PlaySnaps.Watch({})", g.id))); }
            }
        });
    }
    /// The working area of the window's screen: the C# replay's fixed one, else what the live loop set.
    fn work_rect(&self) -> (i32, i32, i32, i32) {
        if self.hooks.replay { return (0, 0, 1920, 1040); }
        self.jobs.work_rect.unwrap_or((0, 0, self.host.work_area.0, self.host.work_area.1))
    }
    fn play_pc(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let mut psi = pp::pc_start(&g);
        if let Err(x) = pp::pc_prepare_saves(&g, &mut psi) { self.show_text_back(&format!("Could not start {}", g.title), &format!("Its save folder couldn't be set up: {}", x), None, None); return; }
        let started = std::time::Instant::now();
        let id = g.id.clone();
        self.run_job(&g.title, psi, true, Box::new(move |l, code| {
            // Some games start through a program that quits at once: then the session lasts while their folder's programs run.
            let Some(g) = l.game(&id) else { return };
            if !l.jobs.in_folder_wait { l.finished(&id, code); return; }
            let dir = nm_core::pc_games::dir(&g).unwrap_or_default().to_lowercase();
            let probe = move || nm_core::proc::processes().iter().any(|p| p.exe.as_deref().map_or(false, |x| x.to_lowercase().starts_with(&format!("{}{}", dir, std::path::MAIN_SEPARATOR))));
            l.folder_wait_busy(&format!("Playing {}", g.title), 3.0, probe, Box::new(move |l| l.finished(&id, 0)));
        }));
        if self.job {
            let gw = g.clone();
            self.jobs.folder_wait = Some(Box::new(move || nm_core::pc_games::is_custom(&gw) && started.elapsed().as_secs_f64() <= 15.0));
        }
    }
    /// The quiet wait: until the folder's programs have been gone `quiet` seconds; paused builds resume at its end.
    pub(crate) fn folder_wait_busy(&mut self, title: &str, quiet: f64, mut running: impl FnMut() -> bool + Send + 'static, then: Box<dyn FnOnce(&mut Launcher)>) {
        self.busy(title, move || {
            let mut since = std::time::Instant::now();
            while since.elapsed().as_secs_f64() < quiet {
                if running() { since = std::time::Instant::now(); }
                std::thread::sleep(std::time::Duration::from_millis(if quiet < 1.0 { 10 } else { 500 }));
            }
            Ok(())
        }, move |l, _| {
            l.jobs.in_folder_wait = false;
            nm_core::recomp::set_paused(false);
            then(l);
        });
    }
    /// Finished: the session's line, then AfterGame, then what the player should hear.
    pub fn finished(&mut self, id: &str, code: i32) {
        // The Pokedex read again 5 s after, on the capture thread (live launcher only: replays read it when they open it).
        if !self.hooks.replay {
            let games = self.ctx().lib.games.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let g2 = games.clone();
                nm_core::art_jobs::background(|| crate::dex::load_dex_background(&games, &|| nm_core::art_jobs::player_busy() || !nm_core::saves::running_games(&g2).is_empty()));
            });
        }
        let Some(g) = self.game(id) else { return };
        let secs = (self.hooks.wall_seconds() - self.jobs.game_start_wall).max(0.0) as i64;
        let mut text = format!("{} ran for {}:{:02}.", g.title, secs / 60, secs % 60);
        if code != 0 { text += &format!(" It ended with code {}.", code); }
        let detail = if code != 0 { format!("\n\n{}", self.log.iter().skip(self.log.len().saturating_sub(12)).cloned().collect::<Vec<_>>().join("\n")) } else { String::new() };
        config::write("last-session.txt", &format!("{}{}", text, detail));
        let kept = self.log.clone();
        self.after_game(&g, code, Box::new(move |l| {
            l.log = kept;
            let back = Act::new(|l| { if l.jobs.play_direct.is_some() { l.request_close(); } else { l.pages.pop(); } });
            if code != 0 { l.show_log("Finished, check this", &text, Some(back)); }
            else if let Some(n) = l.jobs.notice_after.clone() { let t = l.jobs.notice_title.clone().unwrap_or_else(|| "About your save".into()); l.show_text_back(&t, &n, Some(back), None); }
            else if l.jobs.play_direct.is_some() { l.request_close(); return; }
            l.jobs.notice_after = None;
            l.jobs.notice_title = None;
            l.replace_main();
        }));
    }
    /// After a game: back up, check for damage, share. The player hears only when there's something to decide.
    fn after_game(&mut self, g: &GameEntry, _code: i32, done: Box<dyn FnOnce(&mut Launcher)>) {
        // A PC game's saves are its own business: backed up before the session, nothing to share or check after.
        if g.pc.is_some() { done(self); return; }
        let (gw, id) = (g.clone(), g.id.clone());
        self.busy("Checking your save", move || {
            let mut damaged = None;
            let mut result = None;
            if let Some(file) = saves::save_of(&gw).0 {
                let _ = save_guard::backup(&gw, Some(Path::new(&file)), "played");
                damaged = save_guard::problem_for(Some(&gw), Path::new(&file));
            }
            if damaged.is_none() { result = Some(saves::sync_save(&gw, None)); }
            Ok((damaged, result))
        }, move |l, r| {
            let Some(g) = l.game(&id) else { return };
            let done = std::rc::Rc::new(std::cell::RefCell::new(Some(done)));
            let call = move |l: &mut Launcher| { if let Some(d) = done.borrow_mut().take() { d(l); } };
            match r {
                Ok((Some(why), _)) => { l.push(crate::save_pages::damaged_page(&g, &why, Some(Act::new(call)))); }
                Ok((None, Some(res))) if res.conflict => { l.push(crate::save_pages::conflict_page(&g, Some(Act::new(call)))); }
                Ok((None, res)) => { if let Some(n) = res.and_then(|x| x.note) { l.jobs.notice_after = Some(n); } call(l); }
                Err(e) => { l.jobs.notice_after = Some(e); call(l); }
            }
        });
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::launcher::Host;
    use std::cell::RefCell;
    use std::rc::Rc;

    // The pause flag is one per process: the tests that read it take turns.
    pub(crate) static PAUSE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // Launcher::new needs the bundled fonts (NATIVEMONS_FONTS_DIR), as in morph.rs.
    fn launcher() -> Option<Launcher> {
        if std::env::var("NATIVEMONS_FONTS_DIR").is_err() { eprintln!("skipped: NATIVEMONS_FONTS_DIR not set"); return None; }
        Some(Launcher::new(600, 1000, Host::new((1920, 1040))))
    }
    struct FakeJob { lines: Vec<String>, code: Option<i32>, shown: bool }
    impl RunningJob for FakeJob {
        fn id(&self) -> u32 { 7 }
        fn lines(&mut self) -> Vec<String> { std::mem::take(&mut self.lines) }
        fn exit_code(&mut self) -> Option<i32> { self.code }
        fn kill(&mut self) { self.code = Some(-1); }
        fn window_shown(&mut self) -> bool { self.shown }
    }
    struct FakeSpawner(Rc<RefCell<Vec<String>>>);
    impl Spawner for FakeSpawner {
        fn start(&mut self, psi: &StartInfo) -> std::io::Result<Box<dyn RunningJob>> {
            self.0.borrow_mut().push(nm_core::proc::dry_line("t", psi, true));
            Ok(Box::new(FakeJob { lines: vec!["nativemons: 40 Reading the ROM".into(), "nativemons: error It broke".into(), "noise".into()], code: Some(3), shown: false }))
        }
    }

    #[test]
    fn job_runs_reads_and_ends() {
        let _turn = PAUSE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut l) = launcher() else { return };
        let started = Rc::new(RefCell::new(Vec::new()));
        l.jobs.spawner = Some(Box::new(FakeSpawner(started.clone())));
        let ended = Rc::new(RefCell::new(None));
        let e2 = ended.clone();
        let mut psi = StartInfo::new("C:/x/game.exe", "--a \"b c\"");
        psi.set_env("NM_TEST_ONLY_VAR", "1");
        l.run_job("Preparing", psi, false, Box::new(move |_, code| *e2.borrow_mut() = Some(code)));
        assert!(l.job);
        assert_eq!(started.borrow()[0], "t | C:/x/game.exe | --a \"b c\" |  | NM_TEST_ONLY_VAR=1 | game");
        l.hooks.replay = true;
        l.job_tick();
        // Not a game: only the "nativemons:" lines are kept, the percent read off the first.
        assert_eq!(l.log, vec!["Reading the ROM".to_string(), "It broke".to_string()]);
        assert!((l.progress - 0.4).abs() < 1e-6);
        assert_eq!(*ended.borrow(), Some(3));
        assert!(!l.job && l.effects.contains(&Effect::ShowWindow));
    }

    #[test]
    fn light_mode_pauses_background_builds_while_a_game_plays() {
        let _turn = PAUSE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut l) = launcher() else { return };
        let started = Rc::new(RefCell::new(Vec::new()));
        l.jobs.spawner = Some(Box::new(FakeSpawner(started.clone())));
        l.hooks.replay = true;
        // Light off: as the C#, builds go on beside the game.
        l.light = false;
        l.run_job("Pokemon X", StartInfo::new("zakuro.exe", ""), true, Box::new(|_, _| {}));
        assert!(l.job && !nm_core::recomp::paused());
        l.job_tick();
        assert!(!l.job && !nm_core::recomp::paused());
        // Light on: paused from the game's start to its end; a tool job (not a game) never pauses them.
        l.light = true;
        l.run_job("Building Pokemon X", StartInfo::new("nativemons-recomp.exe", ""), false, Box::new(|_, _| {}));
        assert!(!nm_core::recomp::paused());
        l.job_tick();
        let paused_in_game = Rc::new(RefCell::new(None));
        let p2 = paused_in_game.clone();
        l.run_job("Pokemon X", StartInfo::new("zakuro.exe", ""), true, Box::new(move |_, _| *p2.borrow_mut() = Some(nm_core::recomp::paused())));
        assert!(l.job && nm_core::recomp::paused());
        l.job_tick();
        // Resumed before the after-play steps run.
        assert_eq!(*paused_in_game.borrow(), Some(false));
        assert!(!nm_core::recomp::paused());
    }

    #[test]
    fn light_mode_pause_lasts_through_a_pc_folder_wait() {
        let _turn = PAUSE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut l) = launcher() else { return };
        l.jobs.spawner = Some(Box::new(FakeSpawner(Rc::new(RefCell::new(Vec::new())))));
        l.light = true;
        l.run_job("Fan game", StartInfo::new("start.exe", ""), true, Box::new(|l, _| {
            assert!(l.jobs.in_folder_wait && nm_core::recomp::paused());
            let left = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(5));
            l.folder_wait_busy("Playing", 0.05, move || left.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) > 0, Box::new(|l| l.log.push("finished".into())));
        }));
        l.jobs.folder_wait = Some(Box::new(|| true));
        l.job_tick();
        // The launcher program has exited, but the game still runs from its folder: still paused.
        assert!(nm_core::recomp::paused() && l.job);
        let t = std::time::Instant::now();
        while l.job && t.elapsed().as_secs() < 5 { assert!(nm_core::recomp::paused()); l.job_tick(); std::thread::sleep(std::time::Duration::from_millis(5)); }
        assert!(!l.job && !nm_core::recomp::paused());
        assert_eq!(l.log.last().map(|s| s.as_str()), Some("finished"));
    }

    #[test]
    fn dry_run_notes_instead() {
        let Some(mut l) = launcher() else { return };
        l.hooks.replay = true;
        l.jobs.dry_run = true;
        l.run_job("Pokemon Red", StartInfo::new("love.exe", "\"gen1\" --game=red"), true, Box::new(|_, _| panic!("a dry run never ends a job")));
        assert!(!l.job);
        assert_eq!(l.effects, vec![Effect::Launch("Pokemon Red | love.exe | \"gen1\" --game=red |  |  | game".into())]);
    }

    #[test]
    fn busy_in_a_replay_runs_at_once_and_cancel_key() {
        let Some(mut l) = launcher() else { return };
        l.hooks.replay = true;
        let got = Rc::new(RefCell::new(None));
        let g2 = got.clone();
        l.busy("Checking", || Ok(5), move |_, r: Result<i32, String>| *g2.borrow_mut() = Some(r));
        assert_eq!(*got.borrow(), Some(Ok(5)));
        l.job = true;
        let stopped = Rc::new(RefCell::new(false));
        let s2 = stopped.clone();
        l.jobs.cancel = Some(Act::new(move |_| *s2.borrow_mut() = true));
        assert!(l.key(crate::input::Key::Escape));
        assert!(*stopped.borrow());
    }

    #[test]
    fn busy_off_the_ui_thread() {
        let Some(mut l) = launcher() else { return };
        let got = Rc::new(RefCell::new(None));
        let g2 = got.clone();
        l.busy("Waiting", || { std::thread::sleep(std::time::Duration::from_millis(30)); Err::<(), String>("no".into()) }, move |_, r| *g2.borrow_mut() = Some(r));
        assert!(l.job && l.job_title == "Waiting");
        let t = std::time::Instant::now();
        while got.borrow().is_none() && t.elapsed().as_secs() < 5 { l.job_tick(); std::thread::sleep(std::time::Duration::from_millis(5)); }
        assert_eq!(*got.borrow(), Some(Err("no".to_string())));
        assert!(!l.job);
    }
}
