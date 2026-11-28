//! Small C# pieces around a game, line by line: ReadClashes (in the background, as each read starts the engine),
//! DiscordPresence's calls, and WatchGba (Fulldex's list after a GBA game; the one key press for a hack).

use crate::launcher::Launcher;
use nm_core::game_entry::GameEntry;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Extras {
    clash_rx: Option<Receiver<(Option<String>, i32)>>,
    clash_again: bool,
    pub was_in_mods: bool,
    pub watch_gba: bool,
    pub gba_playing: bool,
    pub gba_closed: bool,
    pub gba_press: bool,
    pub gba_presses: i32,
    pub gba_press_at: Option<Instant>,
    gba_watched_at: Option<Instant>,
}

/// One pass over the Gen1Recomp games: the first with an active clash and how many there are in all.
fn count_clashes(games: &[GameEntry]) -> (Option<String>, i32) {
    let (mut first, mut n) = (None, 0);
    for mg in games {
        let c = nm_core::mods::read_clashes(mg).iter().filter(|x| x.active()).count() as i32;
        if c > 0 { n += c; if first.is_none() { first = Some(mg.id.clone()); } }
    }
    (first, n)
}

impl Launcher {
    /// ReadClashes: off the UI thread; a second call while one reads reads again after. A replay never gets the result
    /// (the C# replay pumps no messages, so its BeginInvoke never runs), so it doesn't read.
    pub fn read_clashes(&mut self) {
        if self.ctx.is_none() || self.hooks.replay { return; }
        if self.jobs.extra.clash_rx.is_some() { self.jobs.extra.clash_again = true; return; }
        let games: Vec<GameEntry> = self.ctx().lib.games.iter().filter(|x| x.on_gen1()).cloned().collect();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(count_clashes(&games)); });
        self.jobs.extra.clash_rx = Some(rx);
    }
    fn clashes_read(&mut self, (first, n): (Option<String>, i32)) {
        self.clash_game = first.and_then(|id| self.game_index(&id));
        self.clash_count = n;
        self.dirty = true;
    }
    /// Each tick: a finished clash read.
    pub fn clash_poll(&mut self) {
        let Some(rx) = self.jobs.extra.clash_rx.take() else { return };
        match rx.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => { self.jobs.extra.clash_rx = Some(rx); }
            r => {
                if let Ok(r) = r { self.clashes_read(r); }
                if self.jobs.extra.clash_again { self.jobs.extra.clash_again = false; self.read_clashes(); }
            }
        }
    }
    /// The screen changed: leaving the mods pages, what was switched there may have started or ended a clash.
    pub fn mods_left_check(&mut self) {
        let in_mods = self.pages.iter().any(|x| x.mods);
        if self.jobs.extra.was_in_mods && !in_mods { self.read_clashes(); }
        self.jobs.extra.was_in_mods = in_mods;
    }

    /// DiscordPresence.Playing for a game started (not in a replay: nothing leaves the launcher there).
    pub fn discord_playing(&mut self, g: Option<&GameEntry>) { if !self.hooks.replay || std::env::var("NATIVEMONS_TEST_DISCORD").is_ok() { nm_core::discord::playing(g); } }

    /// WatchGba: Fulldex's own list showing after a GBA game means the game has saved, so Fulldex can close; a hack is
    /// started from that list with Enter, up to three times.
    pub fn watch_gba(&mut self) {
        let x = &mut self.jobs.extra;
        if !x.watch_gba || !self.job { return; }
        if x.gba_watched_at.map_or(false, |t| t.elapsed() < Duration::from_millis(50)) { return; }
        x.gba_watched_at = Some(Instant::now());
        let Some(p) = self.jobs.proc.as_mut() else { return };
        let Some((_, title)) = p.main_window() else { return };
        if title.is_empty() { return; }
        let list = title.to_lowercase().contains("fulldex");
        if !list { x.gba_playing = true; x.gba_press = false; }
        else if x.gba_playing { x.gba_closed = true; x.watch_gba = false; p.kill(); }
        else if x.gba_press {
            match x.gba_press_at {
                None => x.gba_press_at = Some(Instant::now() + Duration::from_millis(1500)),
                Some(at) if Instant::now() >= at => {
                    p.post_key(0x0D, 0x001C_0001, 0xC01C_0001u32 as i32 as isize);
                    x.gba_press_at = Some(Instant::now() + Duration::from_secs(3));
                    x.gba_presses += 1;
                    if x.gba_presses >= 3 { x.gba_press = false; }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::Host;
    use nm_core::proc::{RunningJob, Spawner, StartInfo};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Seen { titles: Vec<String>, keys: Vec<(u32, isize, isize)>, killed: bool }
    struct GbaJob(Rc<RefCell<Seen>>);
    impl RunningJob for GbaJob {
        fn id(&self) -> u32 { 1 }
        fn lines(&mut self) -> Vec<String> { Vec::new() }
        fn exit_code(&mut self) -> Option<i32> { if self.0.borrow().killed { Some(-1) } else { None } }
        fn kill(&mut self) { self.0.borrow_mut().killed = true; }
        fn window_shown(&mut self) -> bool { false }
        fn main_window(&mut self) -> Option<(isize, String)> { let s = self.0.borrow(); s.titles.first().map(|t| (5, t.clone())) }
        fn post_key(&mut self, vk: u32, d: isize, u: isize) { self.0.borrow_mut().keys.push((vk, d, u)); }
    }
    struct Sp(Rc<RefCell<Seen>>);
    impl Spawner for Sp { fn start(&mut self, _: &StartInfo) -> std::io::Result<Box<dyn RunningJob>> { Ok(Box::new(GbaJob(self.0.clone()))) } }

    #[test]
    fn gba_list_after_the_game_closes_fulldex() {
        let _turn = crate::play::tests::PAUSE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if std::env::var("NATIVEMONS_FONTS_DIR").is_err() { return; }
        let mut l = Launcher::new(600, 1000, Host::new((1920, 1040)));
        let seen = Rc::new(RefCell::new(Seen { titles: vec!["Pokemon Fulldex".into()], ..Default::default() }));
        l.jobs.spawner = Some(Box::new(Sp(seen.clone())));
        let code = Rc::new(RefCell::new(None));
        let c2 = code.clone();
        l.run_job("FireRed", StartInfo::new("fulldex.exe", ""), true, Box::new(move |_, c| *c2.borrow_mut() = Some(c)));
        l.jobs.extra.watch_gba = true;
        l.jobs.extra.gba_press = true;
        // The list first, for a hack: one Enter after 1.5 s.
        l.watch_gba();
        assert!(seen.borrow().keys.is_empty() && l.jobs.extra.gba_press_at.is_some());
        l.jobs.extra.gba_press_at = Some(Instant::now());
        l.jobs.extra.gba_watched_at = None;
        l.watch_gba();
        assert_eq!(seen.borrow().keys, vec![(0x0D, 0x001C_0001, 0xC01C_0001u32 as i32 as isize)]);
        // The game's own title, then the list again: Fulldex is closed, and the end counts as a clean one.
        seen.borrow_mut().titles = vec!["POKEMON FIRE".into()];
        l.jobs.extra.gba_watched_at = None;
        l.watch_gba();
        assert!(l.jobs.extra.gba_playing && !l.jobs.extra.gba_press);
        seen.borrow_mut().titles = vec!["Pokemon Fulldex".into()];
        l.jobs.extra.gba_watched_at = None;
        l.watch_gba();
        assert!(seen.borrow().killed && l.jobs.extra.gba_closed);
        l.job_tick();
        assert_eq!(*code.borrow(), Some(0));
        assert!(!l.jobs.extra.watch_gba && !l.jobs.extra.gba_closed);
    }
}
