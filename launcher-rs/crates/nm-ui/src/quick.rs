//! Quick start, line by line from the C# (QuickStartTick, StartReady, ReadyFor, UseReady, DropReady, SnapshotTick,
//! StopSnapshot): the waiting game kept at its title screen (Gen1Recomp, nativeplat) and title-screen snapshots
//! (Fulldex DS, 3DSrecomp+Zakuro). A dry-run replay notes each start (Effect::Launch) instead, as the C# hook does.

use crate::input::Effect;
use crate::launcher::Launcher;
use nm_core::game_entry::GameEntry;
use nm_core::play_prep as pp;
use nm_core::proc::RunningJob;
use nm_core::{config, saves};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
pub struct Quick {
    pub ready: Option<Box<dyn RunningJob>>,
    pub ready_for: Option<String>,
    ready_engine: String,
    ready_options: String,
    ready_save: String,
    pub ready_up: bool,
    pub ready_held: bool,
    ready_failed: Option<String>,
    /// nativeplat's slot being got ready off the UI thread: (problem, game id, key).
    starting: Option<(Receiver<Option<String>>, String, String)>,
    pub menu_since: Option<Instant>,
    snapping: Option<Snapping>,
    snap_failed: Option<String>,
    pub dropping: Option<std::thread::JoinHandle<()>>,
    ready_key_was: Option<String>,
    pub stopped_for_close: bool,
}
struct Snapping { rx: Receiver<bool>, stop: Arc<AtomicBool>, id: String, fail_key: String }

impl Launcher {
    fn quick_current(&self) -> Option<GameEntry> { self.ctx.as_ref().and_then(|c| c.current.map(|i| c.lib.games[i].clone())) }
    fn quick_on(&self) -> bool { config::read("quick-start.txt", "on").trim() != "off" }
    pub(crate) fn quick_note_line(&self, what: &str) {
        if let Ok(f) = std::env::var("NATIVEMONS_TEST_LOG") { use std::io::Write; if let Ok(mut o) = std::fs::OpenOptions::new().create(true).append(true).open(f) { let _ = write!(o, "quick-start: {}\r\n", what); } }
    }
    fn on_main(&self) -> bool { self.pages.last().map_or(false, |p| p.main) }
    fn quick_save(g: &GameEntry) -> String {
        nm_core::save_guard::hash_of(&if g.engine() == "nativeplat" { pp::nativeplat_slot(g) } else { saves::save_of(g).0.map(Into::into).unwrap_or_default() })
    }
    fn since_menu(&self) -> f64 { self.jobs.quick.menu_since.map_or(0.0, |t| t.elapsed().as_secs_f64()) }

    /// ReadyFor: the waiting game is used only if its options and save haven't changed since it loaded.
    pub fn ready_for(&mut self, g: &GameEntry) -> bool {
        let q = &mut self.jobs.quick;
        let alive = q.ready.as_mut().map_or(false, |p| p.exit_code().is_none());
        alive && q.ready_up && q.ready_for.as_deref() == Some(g.id.as_str()) && q.ready_options == pp::quick_options(g) && q.ready_save == Self::quick_save(g)
    }
    /// Play: the waiting game is kept through everything before it starts; a snapshot being made stops.
    pub fn quick_hold_for(&mut self, g: &GameEntry) {
        if self.jobs.quick.ready.is_some() && self.jobs.quick.ready_for.as_deref() == Some(g.id.as_str()) { self.jobs.quick.ready_held = true; }
        self.stop_snapshot();
    }
    /// PlayGen1 / PlayNativeplat: the waiting game if it's still good (true: it's the game now), else it goes.
    pub fn use_or_drop_ready(&mut self, g: &GameEntry, done: crate::play::Done) -> Result<(), crate::play::Done> {
        if self.ready_for(g) { self.use_ready(g, done); return Ok(()); }
        if self.jobs.quick.ready.is_some() { self.quick_note_line(&format!("not used for {}", g.id)); }
        self.drop_ready(false);
        Err(done)
    }

    /// QuickStartTick, every 250 ms (never in a replay but by its `quick` command).
    pub fn quick_start_tick(&mut self) {
        self.quick_poll();
        let menu = !self.job && !self.starting && self.jobs.waiting.is_none() && self.jobs.play_direct.is_none() && self.on_main();
        if menu { self.jobs.quick.ready_held = false; }
        if !menu { self.jobs.quick.menu_since = None; } else if self.jobs.quick.menu_since.is_none() { self.jobs.quick.menu_since = Some(Instant::now()); }
        let idle = menu && self.since_menu() >= 1.5;
        self.snapshot_tick(idle);
        let cur = self.quick_current();
        let wanted = menu && self.quick_on() && cur.as_ref().map_or(false, pp::can_quick_start);
        if self.jobs.quick.ready.is_some() {
            let exited = self.jobs.quick.ready.as_mut().map_or(true, |p| p.exit_code().is_some());
            let stale = match &cur { None => true, Some(c) => self.jobs.quick.ready_for.as_deref() != Some(c.id.as_str()) || self.jobs.quick.ready_options != pp::quick_options(c) || exited };
            if (!wanted && !self.jobs.quick.ready_held) || stale { self.drop_ready(false); }
            return;
        }
        if self.jobs.quick.starting.is_some() || !wanted || self.since_menu() < 1.5 || !saves::running_games(&self.ctx().lib.games).is_empty() { return; }
        let g = cur.unwrap();
        let key = format!("{}|{}|{}|{}", g.id, g.engine(), pp::quick_options(&g), g.gen1_rom.clone().or_else(|| g.fulldex_rom.clone()).unwrap_or_default());
        if self.jobs.quick.ready_failed.as_deref() == Some(key.as_str()) { return; }
        if g.engine() != "nativeplat" { self.start_ready(&g, &key); return; }
        // nativeplat plays a copy of the save in its own slot: put it there first, as Play does, off the window's thread.
        if self.hooks.replay {
            if let Err(x) = pp::prepare_nativeplat(&g) { self.quick_note_line(&format!("failed {}: {}", g.id, x)); self.jobs.quick.ready_failed = Some(key); return; }
            self.start_ready(&g, &key);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let gw = g.clone();
        std::thread::spawn(move || { let _ = tx.send(pp::prepare_nativeplat(&gw).err()); });
        self.jobs.quick.starting = Some((rx, g.id.clone(), key));
    }
    /// Each tick: the waiting game's lines and end, nativeplat's slot being ready, the snapshot's end.
    pub fn quick_poll(&mut self) {
        if let Some((rx, id, key)) = self.jobs.quick.starting.take() {
            match rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => { self.jobs.quick.starting = Some((rx, id, key)); }
                r => {
                    let problem = r.unwrap_or(Some("stopped".into()));
                    if let Some(p) = problem { self.quick_note_line(&format!("failed {}: {}", id, p)); self.jobs.quick.ready_failed = Some(key); }
                    else {
                        let cur = self.quick_current();
                        let still = !self.job && self.on_main() && cur.as_ref().map_or(false, |c| c.id == id) && self.quick_on() && saves::running_games(&self.ctx().lib.games).is_empty();
                        if still && self.jobs.quick.ready.is_none() { let g = cur.unwrap(); self.start_ready(&g, &key); }
                    }
                }
            }
        }
        if let Some(p) = self.jobs.quick.ready.as_mut() {
            let lines = p.lines();
            let ended = p.exit_code().is_some();
            for text in lines {
                if text.contains("nativemons: quick-start ready") || text.contains("nativeplat: quick-start ready") {
                    self.jobs.quick.ready_up = true;
                    let id = self.jobs.quick.ready_for.clone().unwrap_or_default();
                    self.quick_note_line(&format!("ready {}", id));
                } else if text.contains("nativemons: quick-start failed") {
                    let id = self.jobs.quick.ready_for.clone().unwrap_or_default();
                    self.quick_note_line(&format!("failed {}: {}", id, text));
                    self.jobs.quick.ready_failed = self.jobs.quick.ready_key();
                    self.drop_ready(false);
                    break;
                }
            }
            if ended && self.jobs.quick.ready.is_some() { self.drop_ready(false); }
        }
        if let Some(s) = self.jobs.quick.snapping.take() {
            match s.rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => { self.jobs.quick.snapping = Some(s); }
                r => {
                    let ok = r.unwrap_or(false);
                    // Stopped for a game isn't a failure: it's made again later.
                    let stopped = s.stop.load(SeqCst);
                    saves::SNAP_PID.store(0, SeqCst);
                    if !ok && !stopped { self.jobs.quick.snap_failed = Some(s.fail_key.clone()); }
                    self.quick_note_line(&format!("{} {}", if ok { "snapshot ready" } else { "snapshot failed" }, s.id));
                }
            }
        }
    }
    fn start_ready(&mut self, g: &GameEntry, key: &str) {
        let options = pp::quick_options(g);
        let mut psi = if g.engine() == "nativeplat" { pp::nativeplat_start(g, Some("wait")) } else { pp::gen1_play_start(g, false, true) };
        psi.stdin = true;
        // Behind whatever the player has open while it gets ready.
        psi.below_normal = true;
        if self.hooks.replay && self.jobs.dry_run {
            self.effects.push(Effect::Launch(nm_core::proc::dry_line(&format!("Quick start {}", g.title), &psi, false)));
            self.jobs.quick.ready_failed = Some(key.to_string());
            self.quick_note_line(&format!("starting {}", g.id));
            return;
        }
        let started = match self.jobs.spawner.as_mut() { Some(s) => s.start(&psi), None => Err(std::io::Error::other("no spawner")) };
        let p = match started { Ok(p) => p, Err(x) => { self.jobs.quick.ready_failed = Some(key.to_string()); self.quick_note_line(&format!("could not start: {}", x)); return; } };
        let pid = p.id();
        saves::READY_PID.store(pid, SeqCst);
        let q = &mut self.jobs.quick;
        q.ready = Some(p);
        q.ready_for = Some(g.id.clone());
        q.ready_engine = g.engine();
        q.ready_options = options;
        q.ready_up = false;
        q.ready_save = Self::quick_save(g);
        q.ready_key_was = Some(key.to_string());
        self.quick_note_line(&format!("starting {} pid {}", g.id, pid));
    }
    /// DropReady: closing its input tells it to quit, and it's stopped if it doesn't.
    pub fn drop_ready(&mut self, wait: bool) {
        let Some(p) = self.jobs.quick.ready.take() else { return };
        saves::READY_PID.store(0, SeqCst);
        let q = &mut self.jobs.quick;
        q.ready_up = false;
        q.ready_for = None;
        // nativeplat writes its save slot as it closes: it gets longer, so it's never stopped part way through.
        let grace = if q.ready_engine == "nativeplat" { 15000 } else { 2000 };
        self.quick_note_line("dropped");
        let t = p.end_gently(grace);
        if wait { if let Some(t) = t { let _ = t.join(); } } else { self.jobs.quick.dropping = t; }
    }
    /// UseReady: the waiting game becomes the game, told to go on with "play".
    fn use_ready(&mut self, g: &GameEntry, done: crate::play::Done) {
        let Some(mut p) = self.jobs.quick.ready.take() else { return };
        saves::READY_PID.store(0, SeqCst);
        self.jobs.quick.ready_up = false;
        self.jobs.quick.ready_held = false;
        p.normal_priority();
        self.log.clear();
        self.job_title = g.title.clone();
        self.job_is_game = true;
        self.jobs.hid = false;
        self.job_start_wall = self.hooks.wall_seconds();
        self.jobs.game_start_wall = self.job_start_wall;
        self.progress = -1.0;
        self.jobs.done = Some(done);
        self.jobs.cancel = None;
        self.jobs.folder_wait = None;
        self.quick_note_line(&format!("used {}", g.id));
        p.write_line("play");
        self.jobs.proc = Some(p);
        self.job = true;
        self.discord_playing(Some(g));
        if self.light { nm_core::recomp::set_paused(true); }
        self.dirty = true;
    }

    // ---------- title-screen snapshots ----------
    /// StopSnapshot: a snapshot being made stops (the game gets the PC).
    pub fn stop_snapshot(&mut self) { if let Some(s) = &self.jobs.quick.snapping { s.stop.store(true, SeqCst); } }
    fn snapshot_tick(&mut self, menu_idle: bool) {
        // Never beside a game: the player's game gets the PC.
        if self.jobs.quick.snapping.is_some() { if self.job && self.job_is_game { self.stop_snapshot(); } return; }
        let Some(g) = self.quick_current() else { return };
        if !menu_idle || !self.quick_on() || !pp::can_snapshot(&g) || pp::snapshot_good(&g) || !saves::running_games(&self.ctx().lib.games).is_empty() { return; }
        let key = pp::snap_key(&g);
        let save_hash = pp::save_hash(&pp::snap_save(&g));
        let fail_key = format!("{}{}", key, save_hash);
        if self.jobs.quick.snap_failed.as_deref() == Some(fail_key.as_str()) { return; }
        let Some(m) = pp::snapshot_making(&g, &save_hash) else { return };
        if self.hooks.replay && self.jobs.dry_run {
            self.effects.push(Effect::Launch(nm_core::proc::dry_line(&format!("Snapshot {}", g.title), &m.psi, false)));
            self.jobs.quick.snap_failed = Some(fail_key);
            let _ = std::fs::remove_dir_all(&m.work);
            return;
        }
        let child = nm_core::proc::command_for(&m.psi).spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(x) => { self.jobs.quick.snap_failed = Some(fail_key); self.quick_note_line(&format!("snapshot failed {}: {}", g.id, x)); let _ = std::fs::remove_dir_all(&m.work); return; }
        };
        nm_core::proc::apply_priority(&child, &m.psi);
        saves::SNAP_PID.store(child.id(), SeqCst);
        self.quick_note_line(&format!("snapshot making {}", g.id));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let (stop2, gw) = (stop.clone(), g.clone());
        std::thread::spawn(move || {
            use std::io::Read;
            if let Some(mut o) = child.stdout.take() { std::thread::spawn(move || { let mut b = Vec::new(); let _ = o.read_to_end(&mut b); }); }
            if let Some(mut e) = child.stderr.take() { std::thread::spawn(move || { let mut b = Vec::new(); let _ = e.read_to_end(&mut b); }); }
            let end = Instant::now() + std::time::Duration::from_secs(180);
            let code = loop {
                if let Ok(Some(s)) = child.try_wait() { break s.code(); }
                if stop2.load(SeqCst) || Instant::now() >= end { let _ = child.kill(); let _ = child.wait(); break None; }
                std::thread::sleep(std::time::Duration::from_millis(50));
            };
            let ok = pp::snapshot_keep(&gw, &m, code == Some(0), &key, &save_hash);
            let _ = tx.send(ok);
        });
        self.jobs.quick.snapping = Some(Snapping { rx, stop, id: g.id.clone(), fail_key });
    }
    /// Closing: the waiting game and the snapshot go with the launcher (FormClosing).
    pub fn quick_stop_all(&mut self, wait: bool) {
        self.drop_ready(false);
        self.stop_snapshot();
        if wait { if let Some(d) = self.jobs.quick.dropping.take() { let _ = d.join(); } }
    }
}

impl Quick {
    fn ready_key(&self) -> Option<String> { self.ready_key_was.clone() }
}
