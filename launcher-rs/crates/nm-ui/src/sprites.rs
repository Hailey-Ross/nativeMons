//! Sprites: Pokemon pictures from pogo.hails.app, cached in state\sprite-cache (as the C# Sprites class). The live
//! launcher fetches them on up to 3 worker threads, the whole evolution line together, and shows a loading ball
//! meanwhile; snapshots and replays (the default, `set_live(false)`) fetch at once so every run draws the same.

use nm_draw::Bitmap;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const SITE: &str = "https://pogo.hails.app/api/pokemon-sprite/";

#[derive(Default)]
struct State {
    loaded: HashMap<String, Option<Arc<Bitmap>>>,
    asked: HashSet<String>,
    tried: HashSet<String>,
    queue: VecDeque<String>,
    workers: u32,
    families: HashMap<String, Vec<String>>,
}
static STATE: Mutex<Option<State>> = Mutex::new(None);
static LIVE: AtomicBool = AtomicBool::new(false);
/// Goes up when a picture arrives, so the window can redraw.
pub static ARRIVED: AtomicU32 = AtomicU32::new(0);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R { f(STATE.lock().unwrap().get_or_insert_with(State::default)) }
/// The live launcher's worker threads (on), or fetched at once (off: snapshots, replays, tests).
pub fn set_live(on: bool) { LIVE.store(on, Ordering::SeqCst); }
/// The live launcher (not a snapshot, replay or test).
pub fn is_live() -> bool { LIVE.load(Ordering::SeqCst) }

fn folder() -> PathBuf { nm_core::config::state().join("sprite-cache") }
fn file_of(key: &str) -> PathBuf { folder().join(format!("{}.png", key.replace('/', "-"))) }

/// Download: a copy on disk under 30 days old is left alone.
fn download(key: &str) -> bool {
    let file = file_of(key);
    if let Ok(m) = std::fs::metadata(&file).and_then(|m| m.modified()) {
        if std::time::SystemTime::now().duration_since(m).map_or(true, |d| d.as_secs_f64() / 86400.0 <= 30.0) { return false; }
    }
    let _ = std::fs::create_dir_all(folder());
    let temp = file.with_extension(format!("png.{}.part", nm_core::proc::new_guid()));
    let mut ok = false;
    if nm_core::net::download_file_unnoted(&format!("{}{}.png", SITE, key), &temp).is_ok() && Bitmap::load_png(&temp).is_ok() {
        ok = std::fs::copy(&temp, &file).is_ok();
    }
    let _ = std::fs::remove_file(&temp);
    ok
}
fn fetch(key: &str) -> Option<Arc<Bitmap>> {
    download(key);
    Bitmap::load_png(&file_of(key)).ok().map(Arc::new)
}

/// Sprites.Get: the picture, or None until it's here (the first ask starts the fetch, with its evolution line).
pub fn get(key: &str) -> Option<Arc<Bitmap>> {
    if !LIVE.load(Ordering::SeqCst) {
        if let Some(b) = with(|s| s.loaded.get(key).cloned()) { return b; }
        let b = fetch(key);
        with(|s| { s.loaded.insert(key.to_string(), b.clone()); });
        return b;
    }
    with(|s| {
        if let Some(Some(b)) = s.loaded.get(key) { return Some(b.clone()); }
        if !s.asked.contains(key) {
            let (prefix, id) = match key.strip_prefix("shiny/") { Some(rest) => ("shiny/", rest), None => ("", key) };
            enqueue(s, key.to_string());
            let line = s.families.get(id).cloned().unwrap_or_default();
            for other in line { enqueue(s, format!("{}{}", prefix, other)); }
        }
        None
    })
}
fn enqueue(s: &mut State, key: String) {
    if !s.asked.insert(key.clone()) { return; }
    s.queue.push_back(key);
    if s.workers < 3 { s.workers += 1; std::thread::spawn(work); }
}
fn work() {
    loop {
        let Some(key) = with(|s| { let k = s.queue.pop_front(); if k.is_none() { s.workers -= 1; } k }) else { return };
        let b = fetch(&key);
        with(|s| { if b.is_some() { s.loaded.insert(key.clone(), b); } s.tried.insert(key); });
        ARRIVED.fetch_add(1, Ordering::SeqCst);
    }
}
/// Asked for and still on its way (not here, not given up on): the window shows the loading Poke Ball meanwhile.
pub fn loading(key: Option<&str>) -> bool {
    let Some(key) = key else { return false };
    LIVE.load(Ordering::SeqCst) && with(|s| s.asked.contains(key) && !s.loaded.contains_key(key) && !s.tried.contains(key))
}
/// LoadFamilies: evolution lines (one per row, ids by commas), so asking for one picture brings its whole line.
pub fn load_families(ask: impl FnOnce() -> Option<String>) {
    let f = nm_core::config::state().join("sprite-families.txt");
    let text = if f.is_file() { std::fs::read_to_string(&f).ok() } else { ask() };
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else { return };
    if !f.is_file() { let _ = std::fs::write(&f, &text); }
    with(|s| for row in text.split('\n') {
        let ids: Vec<String> = row.trim().split(',').map(|x| x.to_string()).collect();
        for id in ids.iter().filter(|i| !i.is_empty()) { s.families.insert(id.clone(), ids.clone()); }
    });
}

/// Every picture to the end of the National Dex, normal and shiny, for offline play.
pub const MAX_DEX: i32 = 1025;
/// Sprites.GetAll on four workers: only what's missing (or a month old) is fetched. (fresh, total, failed)
pub fn get_all(report: &mut dyn FnMut(usize, usize, usize, usize)) -> (usize, usize, usize) {
    let keys: Vec<String> = (1..=MAX_DEX).flat_map(|i| [i.to_string(), format!("shiny/{}", i)]).collect();
    let total = keys.len();
    let queue = Mutex::new(std::collections::VecDeque::from(keys));
    let (tx, rx) = std::sync::mpsc::channel::<(usize, usize, usize)>();
    let counts = Mutex::new((0usize, 0usize, 0usize));
    std::thread::scope(|sc| {
        for _ in 0..4 {
            let (q, c, tx) = (&queue, &counts, tx.clone());
            sc.spawn(move || loop {
                let Some(key) = q.lock().unwrap().pop_front() else { return };
                let got = download(&key);
                let mut n = c.lock().unwrap();
                n.0 += 1;
                if got { n.1 += 1; } else if !file_of(&key).is_file() { n.2 += 1; }
                let _ = tx.send(*n);
            });
        }
        drop(tx);
        for (done, fresh, failed) in rx { report(done, total, fresh, failed); }
    });
    let n = counts.into_inner().unwrap();
    (n.1, total, n.2)
}
