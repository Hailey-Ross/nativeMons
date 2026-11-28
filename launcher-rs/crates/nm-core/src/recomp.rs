//! Recomp: 3DSrecomp+Zakuro. Each game's code is recompiled on the player's PC from their own dump into
//! state\3ds\recomp\<title id>.dll, which never leaves it. Line by line from the C# Recomp class.

use crate::config;
use crate::game_entry::GameEntry;
use crate::proc::{self, StartInfo};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// What Zakuro's COMPATIBILITY.md lists as playable recompiled, plus Moon.
const TITLES: [&str; 8] = ["0004000000055D00", "0004000000055E00", "000400000011C400", "000400000011C500", "0004000000164800", "0004000000175E00", "00040000001B5000", "00040000001B5100"];
pub const ZIG_VERSION: &str = "0.17.0";
const ZIG_URL: &str = "https://ziglang.org/download/0.17.0/zig-x86_64-windows-0.17.0.zip";
const ZIG_SHA256: &str = "b5663f69581dcf391293fbf16c06cb80d81d806545ce618b4d0bab7f0eb8c428";
fn zakuro_dir() -> PathBuf { config::zakuro().parent().map(|p| p.to_path_buf()).unwrap_or_default() }
pub fn tool() -> PathBuf { zakuro_dir().join(crate::config::paths().exe("nativemons-recomp")) }
pub fn installed() -> bool { config::zakuro().is_file() && tool().is_file() }
pub fn supports(g: &GameEntry) -> bool { g.ctr.as_deref().map_or(false, |c| TITLES.contains(&c.to_uppercase().as_str())) }
pub fn dir() -> PathBuf { crate::ctr3ds_saves::root().join("recomp") }
fn ctr(g: &GameEntry) -> String { g.ctr.as_deref().unwrap_or("").to_uppercase() }
fn plain(g: &GameEntry) -> String { g.ctr_plain_rom.clone().unwrap_or_default() }
fn s(p: &Path) -> String { p.display().to_string() }
fn plus(p: &Path, suffix: &str) -> PathBuf { PathBuf::from(format!("{}{}", p.display(), suffix)) }
pub fn library_of(g: &GameEntry) -> PathBuf { dir().join(format!("{}.dll", ctr(g))) }
fn stamp_of(g: &GameEntry) -> PathBuf { dir().join(format!("{}.txt", ctr(g))) }
pub fn built(g: &GameEntry) -> bool { g.ctr.is_some() && library_of(g).is_file() && stamp_of(g).is_file() }
fn work() -> PathBuf { dir().join("work") }
fn cache() -> PathBuf { dir().join("cache") }
fn zig_home() -> PathBuf { crate::ctr3ds_saves::root().join(format!("zig-{}", ZIG_VERSION)) }
pub fn zig_exe() -> PathBuf { zig_home().join(format!("zig-x86_64-windows-{}", ZIG_VERSION)).join(crate::config::paths().exe("zig")) }
pub fn fast() -> bool { config::read("3ds-build-speed.txt", "fast") != "quiet" }
pub fn set_fast(on: bool) { config::write("3ds-build-speed.txt", if on { "fast" } else { "quiet" }); }
fn processors() -> i32 { std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(1) }
pub fn cores() -> i32 { (if fast() { processors() - 1 } else { processors() / 2 }).clamp(1, 64) }
fn mask_of(c: i32) -> u64 { if c >= 64 { u64::MAX } else { (1u64 << c) - 1 } }
pub fn mask() -> u64 { mask_of(cores()) }
/// Measured on Pokemon Y: 17.5 minutes on 4 threads, 8.9 on 8, 5.2 on 16.
pub fn minutes(cores: i32) -> i32 {
    let work = if cores <= 8 { cores as f64 } else { 8.0 + (cores - 8) as f64 * 0.5 };
    ((72.0 / work).ceil() as i32).max(3)
}
/// Zakuro writes down where it had to interpret (beside the library); a rebuild covers those.
pub const IMPROVE_AFTER: i32 = 25;
pub fn hints_file(g: &GameEntry) -> PathBuf { library_of(g).with_extension("hints") }
pub fn hint_count(g: &GameEntry) -> i32 {
    std::fs::read_to_string(hints_file(g)).map(|t| config::lines(&t).iter().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).count() as i32).unwrap_or(0)
}
/// Another dump of the game needs a build of its own.
fn rom_key(g: &GameEntry) -> String { g.roms.iter().find(|x| Some(&x.file) == g.ctr_plain_rom.as_ref()).map(|r| r.sha1.clone()).unwrap_or_default() }
static STATES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
fn read_lines(p: &Path) -> Option<Vec<String>> { std::fs::read_to_string(p).ok().map(|t| config::lines(&t).into_iter().map(String::from).collect()) }
/// "ready", "old" (runs, a rebuild makes it better), "outdated" (this Zakuro can't run it) or "missing".
pub fn state(g: &GameEntry) -> String {
    let lib = library_of(g);
    let stamp = if stamp_of(g).is_file() { match read_lines(&stamp_of(g)) { Some(x) => x, None => return "missing".into() } } else { Vec::new() };
    if !lib.is_file() || stamp.is_empty() || stamp[0].trim() != rom_key(g) { return "missing".into(); }
    let mut better = stamp.len() > 1 && stamp[1].trim().parse::<i32>().map_or(false, |covered| hint_count(g) - covered >= IMPROVE_AFTER);
    let sk = shared_key(g);
    if !sk.is_empty() && (stamp.len() < 3 || stamp[2].trim() != sk) { better = true; }
    let key = format!("{}|{}|{}", s(&lib), crate::library::last_write_ticks(&lib), crate::library::last_write_ticks(&tool()));
    let known = STATES.lock().unwrap().get_or_insert_with(HashMap::new).get(&key).cloned();
    let known = match known {
        Some(k) => k,
        None => {
            let psi = StartInfo::new(tool(), &format!("check \"{}\"", s(&lib)));
            let k = match proc::run(&psi, 20) {
                Err(_) => return "missing".into(),
                Ok(f) => match f.code { None => return "outdated".into(), Some(0) => "ready", Some(4) => "old", Some(_) => "outdated" },
            };
            STATES.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, k.to_string());
            k.to_string()
        }
    };
    if known == "ready" && better { "old".into() } else { known }
}
/// After a session: ask to share once the findings grew by IMPROVE_AFTER since the last answer.
pub fn ask_to_share() -> bool { config::read("zakuro-share-ask.txt", "on") != "never" }
pub fn set_ask_to_share(on: bool) { config::write("zakuro-share-ask.txt", if on { "on" } else { "never" }); }
pub fn should_ask_share(g: &GameEntry) -> bool {
    let last = config::read(&format!("zakuro-share-answered-{}.txt", g.id), "0").parse::<i32>().unwrap_or(0);
    ask_to_share() && hint_count(g) - last >= IMPROVE_AFTER
}
pub fn answered(g: &GameEntry) { config::write(&format!("zakuro-share-answered-{}.txt", g.id), &hint_count(g).to_string()); }
/// "<title id>-<code hash>" of the dump, which names a shared list; None when it can't be read.
pub fn code_id(g: &GameEntry) -> Option<String> {
    let psi = StartInfo::new(tool(), &format!("id \"{}\"", plain(g)));
    let f = proc::run(&psi, 24 * 3600).ok()?;
    let m = regex::Regex::new(r"nativemons: id ([0-9A-F]{16}) ([0-9A-F]{16})").unwrap().captures(&f.out)?;
    Some(format!("{}-{}", &m[1], &m[2]))
}
/// Which launcher and Zakuro made a shared list, so a bad Zakuro build's lists can be found again.
pub fn share_versions() -> String {
    let zakuro = std::fs::read_to_string(zakuro_dir().join("NATIVEMONS-BUILD.txt")).ok()
        .and_then(|t| regex::Regex::new(r"/tree/([0-9a-f]{9})").unwrap().captures(&t).map(|m| m[1].to_string())).unwrap_or_default();
    format!("nativeMons {}, Zakuro {}", crate::updater::installed_label(), if zakuro.is_empty() { "unknown" } else { &zakuro })
}
/// The addresses alone, for sharing: one per line. .txt, since GitHub refuses a .hints attachment.
pub fn share_file(g: &GameEntry, code_id: &str) -> std::io::Result<PathBuf> {
    let to = dir().join("share").join(format!("{}.hints.txt", code_id));
    std::fs::create_dir_all(to.parent().unwrap())?;
    let re = regex::Regex::new("^(0x)?[0-9A-Fa-f]{1,8}$").unwrap();
    let t = std::fs::read_to_string(hints_file(g))?;
    let keep: Vec<String> = config::lines(&t).iter().map(|l| l.trim().to_string()).filter(|l| re.is_match(l)).collect();
    config::write_all_lines(&to, &keep)?;
    Ok(to)
}

// ---------- shared findings ----------
const HINTS_BRANCH: &str = "https://raw.githubusercontent.com/Hailey-Ross/zakuro/nativemons-hints/";
pub fn shared_dir() -> PathBuf { dir().join("shared-hints") }
fn packaged_hints() -> PathBuf { zakuro_dir().join("hints") }
fn code_id_file(g: &GameEntry) -> PathBuf { dir().join(format!("{}.codeid", ctr(g))) }
pub fn known_code_id(g: &GameEntry) -> Option<String> { std::fs::read_to_string(code_id_file(g)).ok().map(|t| t.trim().to_string()) }
/// Worked out once per built game, off the window's thread: it reads the dump.
pub fn learn_code_id(g: &GameEntry) {
    if known_code_id(g).is_some() || !library_of(g).is_file() { return; }
    if let Some(id) = code_id(g) { let _ = std::fs::write(code_id_file(g), id); }
}
fn hints_files(dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(dir) { Ok(rd) => rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.to_string_lossy().to_lowercase().ends_with(".hints")).collect(), Err(_) => Vec::new() }
}
fn hints_dir() -> PathBuf { if shared_dir().is_dir() && !hints_files(&shared_dir()).is_empty() { shared_dir() } else { packaged_hints() } }
/// The shared list this game's build would take in, by its checksum; "" when there's none.
pub fn shared_key(g: &GameEntry) -> String {
    match known_code_id(g) { None => String::new(), Some(id) => crate::save_guard::hash_of(&hints_dir().join(format!("{}.hints", id))) }
}
fn hint_text(data: &[u8]) -> bool {
    let re = regex::Regex::new("^[0-9A-F]{8}$").unwrap();
    data.iter().map(|&b| if b < 128 { b as char } else { '?' }).collect::<String>().split('\n').all(|raw| { let l = raw.trim(); l.is_empty() || l.starts_with('#') || re.is_match(l) })
}
/// Downloads the shared lists that changed (read only). None when done, or why not.
pub fn check_shared_hints() -> Option<String> {
    let r = (|| -> Result<(), String> {
        std::fs::create_dir_all(shared_dir()).map_err(|e| e.to_string())?;
        let index = crate::net::download_string(&format!("{}index.txt", HINTS_BRANCH), None)?;
        let re = regex::Regex::new(r"^([0-9A-F]{16})-[0-9A-F]{16}\.hints ([0-9a-f]{64})$").unwrap();
        for line in index.split('\n') {
            let Some(m) = re.captures(line.trim()) else { continue };
            if !TITLES.contains(&&m[1]) { continue; }
            let name = line.trim().split(' ').next().unwrap_or("").to_string();
            let want = m[2].to_uppercase();
            let local = shared_dir().join(&name);
            if crate::save_guard::hash_of(&local) == want { continue; }
            let tmp = plus(&local, ".dl");
            crate::net::download_file(&format!("{}hints/{}", HINTS_BRANCH, name), &tmp, None, &mut |_| {})?;
            let data = std::fs::read(&tmp).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&tmp);
            if data.len() > (2 << 20) || crate::save_guard::hash_of_bytes(&data) != want || !hint_text(&data) { continue; }
            let new = plus(&local, ".new");
            std::fs::write(&new, &data).map_err(|e| e.to_string())?;
            if local.is_file() { std::fs::remove_file(&local).map_err(|e| e.to_string())?; }
            std::fs::rename(&new, &local).map_err(|e| e.to_string())?;
        }
        // The lists this Zakuro came with, until the branch has its own of them.
        for f in hints_files(&packaged_hints()) {
            let to = shared_dir().join(f.file_name().unwrap());
            if !to.is_file() { std::fs::copy(&f, &to).map_err(|e| e.to_string())?; }
        }
        Ok(())
    })();
    r.err()
}
pub fn shared_hint_files() -> Vec<PathBuf> { hints_files(&shared_dir()) }

/// Zig, the C compiler, downloaded once and checked against its pinned checksum (the one Zakuro pins).
pub fn ensure_zig() -> Option<String> {
    let (zip, temp) = (plus(&zig_home(), ".download.zip"), plus(&zig_home(), ".new"));
    let r = (|| -> Result<Option<String>, String> {
        let io = |x: std::io::Error| x.to_string();
        if zig_exe().is_file() { return Ok(None); }
        std::fs::create_dir_all(crate::ctr3ds_saves::root()).map_err(io)?;
        crate::net::download_file(ZIG_URL, &zip, None, &mut |_| {})?;
        if crate::save_guard::hash_of(&zip).to_lowercase() != ZIG_SHA256 { return Ok(Some("The download wasn't the expected file, so it wasn't used.".into())); }
        // Unpacked in place: a virus scanner still reading the new files blocks moving their folder.
        if zig_home().is_dir() { std::fs::remove_dir_all(zig_home()).map_err(io)?; }
        crate::zips::extract_all(&s(&zip), &zig_home())?;
        if zig_exe().is_file() { return Ok(None); }
        let _ = std::fs::remove_dir_all(zig_home());
        Ok(Some("The download was missing zig.exe.".into()))
    })();
    let _ = std::fs::remove_file(&zip);
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    match r { Ok(x) => x, Err(x) => Some(x) }
}

// ---------- builds ----------
pub fn build_start(g: &GameEntry) -> std::io::Result<StartInfo> {
    let _ = std::fs::remove_file(stamp_of(g));
    build_into(g, &library_of(g), &work())
}
fn build_into(g: &GameEntry, library: &Path, work: &Path) -> std::io::Result<StartInfo> {
    std::fs::create_dir_all(dir())?;
    if work.is_dir() { std::fs::remove_dir_all(work)?; }
    let shared = hints_dir();
    let _ = std::fs::write(plus(library, ".shared"), shared_key(g));
    let mut args = format!("build \"{}\" \"{}\" \"{}\" --cache \"{}\"", plain(g), s(work), s(library), s(&cache()));
    if shared.is_dir() { args += &format!(" --shared-hints \"{}\"", s(&shared)); }
    let mut psi = StartInfo::new(tool(), &args);
    psi.cwd = Some(dir());
    let sep = if cfg!(windows) { ";" } else { ":" };
    psi.set_env("PATH", &format!("{}{}{}", s(zig_exe().parent().unwrap()), sep, std::env::var("PATH").unwrap_or_default()));
    let appdata = dir().join("appdata");
    let hints_for = appdata.join("3dsrecomp").join(format!("{}.hints", ctr(g)));
    std::fs::create_dir_all(hints_for.parent().unwrap())?;
    if hints_file(g).is_file() { std::fs::copy(hints_file(g), &hints_for)?; } else if hints_for.is_file() { std::fs::remove_file(&hints_for)?; }
    psi.set_env("APPDATA", &s(&appdata));
    psi.set_env("CC", "zig cc");
    psi.set_env("NATIVEMONS_RECOMP_COMPILER", &format!("zig {}", ZIG_VERSION));
    psi.set_env("ZIG_GLOBAL_CACHE_DIR", &s(&dir().join("zig-cache")));
    psi.set_env("ZIG_LOCAL_CACHE_DIR", &s(&dir().join("zig-cache")));
    Ok(psi)
}
/// Directory.GetFiles(Dir, pattern) for "*.pdb", "*.lib", "*.part": deleted, each on its own.
fn delete_leftovers() {
    let Ok(rd) = std::fs::read_dir(dir()) else { return };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_lowercase();
        if e.path().is_file() && [".pdb", ".lib", ".part"].iter().any(|x| n.ends_with(x)) { let _ = std::fs::remove_file(e.path()); }
    }
}
/// The generated C and objects are deleted either way: only the library and the cache stay.
pub fn finish(g: &GameEntry, code: i32) -> bool {
    let ok = code == 0 && library_of(g).is_file();
    if work().is_dir() { let _ = std::fs::remove_dir_all(work()); }
    // Zig's own cache repeats ours and grew to 7 GB over six games: it goes after every build.
    if dir().join("zig-cache").is_dir() { let _ = std::fs::remove_dir_all(dir().join("zig-cache")); }
    // The linker's debug file (about 750 MB) and import library aren't needed to play.
    delete_leftovers();
    let used = used_shared(&library_of(g));
    if ok { let _ = config::write_all_lines(&stamp_of(g), &[rom_key(g), hint_count(g).to_string(), used]); }
    else if library_of(g).is_file() && code != 0 { let _ = std::fs::remove_file(library_of(g)); }
    prune_cache(3 << 30);
    ok
}
fn used_shared(library: &Path) -> String {
    let f = plus(library, ".shared");
    let k = if f.is_file() { match std::fs::read_to_string(&f) { Ok(t) => t.trim().to_string(), Err(_) => return String::new() } } else { String::new() };
    if std::fs::remove_file(&f).is_err() && f.exists() { return String::new(); }
    k
}

// ---------- quiet builds in the background ----------
struct Quiet { pid: u32, game: GameEntry, suspended: bool }
static QUIET: Mutex<Option<Quiet>> = Mutex::new(None);
static QUEUED: Mutex<VecDeque<GameEntry>> = Mutex::new(VecDeque::new());
static PROGRESS: AtomicU32 = AtomicU32::new(0);
pub static FAKE_RUNNING: AtomicBool = AtomicBool::new(false);
struct Banner { title: Option<String>, verb: String, done_at: Option<Instant> }
static BANNER: Mutex<Banner> = Mutex::new(Banner { title: None, verb: String::new(), done_at: None });
/// A dry-run replay's notes of what would start in the background (Some while it runs).
pub static DRY: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// For the launcher's banner: title, verb ("Building" or "Improving"), progress, seconds since one finished.
pub struct BannerState { pub title: Option<String>, pub verb: String, pub progress: f32, pub done_ago: Option<f64>, pub running: bool, pub paused: bool }
pub fn banner() -> BannerState {
    let b = BANNER.lock().unwrap();
    BannerState { title: b.title.clone(), verb: if b.verb.is_empty() { "Improving".into() } else { b.verb.clone() }, progress: quiet_progress(), done_ago: b.done_at.map(|t| t.elapsed().as_secs_f64()), running: busy() || FAKE_RUNNING.load(Ordering::SeqCst), paused: paused() && busy() }
}

// ---------- Light mode: paused while a game plays (user, 2026-11-25; Rust only) ----------
static PAUSED: AtomicBool = AtomicBool::new(false);
pub fn paused() -> bool { PAUSED.load(Ordering::SeqCst) }
/// Pauses the background builds (the running one is suspended with everything it started, the queue waits) or lets
/// them go on. Nothing is cancelled.
pub fn set_paused(on: bool) {
    if PAUSED.swap(on, Ordering::SeqCst) == on { return; }
    // Under the lock, and only when it changes: a build starting meanwhile checks the same flag.
    if let Some(q) = QUIET.lock().unwrap().as_mut() { if q.suspended != on { suspend_tree(q.pid, on); q.suspended = on; } }
    if !on { next_queued(); }
}
/// The process and its descendants, found by parent ids; twice, for children started while the first pass ran.
fn suspend_tree(root: u32, on: bool) {
    let mut done: Vec<u32> = Vec::new();
    for _ in 0..2 {
        let all = proc::processes();
        let mut tree = vec![root];
        let mut i = 0;
        while i < tree.len() { let p = tree[i]; for x in all.iter().filter(|x| x.parent == p && x.id != p) { if !tree.contains(&x.id) { tree.push(x.id); } } i += 1; }
        for p in tree { if !done.contains(&p) { suspend_one(p, on); done.push(p); } }
    }
}
#[cfg(windows)]
fn suspend_one(pid: u32, on: bool) {
    #[link(name = "kernel32")]
    extern "system" { fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize; fn CloseHandle(h: isize) -> i32; }
    #[link(name = "ntdll")]
    extern "system" { fn NtSuspendProcess(h: isize) -> i32; fn NtResumeProcess(h: isize) -> i32; }
    unsafe {
        let h = OpenProcess(0x0800, 0, pid);
        if h == 0 { return; }
        if on { NtSuspendProcess(h); } else { NtResumeProcess(h); }
        CloseHandle(h);
    }
}
#[cfg(not(windows))]
fn suspend_one(pid: u32, on: bool) { let _ = proc::run(&StartInfo::new("kill", &format!("-{} {}", if on { "STOP" } else { "CONT" }, pid)), 5); }
pub fn quiet_progress() -> f32 { f32::from_bits(PROGRESS.load(Ordering::SeqCst)) }
fn set_progress(p: f32) { PROGRESS.store(p.to_bits(), Ordering::SeqCst); }
pub fn banner_showing() -> bool { let b = banner(); b.running || b.done_ago.map_or(false, |a| a < 5.0) }
pub fn busy() -> bool { QUIET.lock().unwrap().is_some() }
fn next(g: &GameEntry) -> PathBuf { plus(&library_of(g), ".next") }
fn quiet_work() -> PathBuf { dir().join("work-quiet") }
pub fn improving(g: &GameEntry) -> bool { QUIET.lock().unwrap().as_ref().map_or(false, |q| q.game.id == g.id) }
pub fn queued(g: &GameEntry) -> bool { QUEUED.lock().unwrap().iter().any(|x| x.id == g.id) }
/// New games found at start-up, built one by one in the background.
pub fn build_quietly(games: &[GameEntry]) {
    { let mut q = QUEUED.lock().unwrap(); for g in games { if !q.iter().any(|x| x.id == g.id) { q.push_back(g.clone()); } } }
    next_queued();
}
fn next_queued() {
    if busy() || paused() { return; }
    let mut pick: Option<(GameEntry, bool)> = None;
    {
        let mut q = QUEUED.lock().unwrap();
        while pick.is_none() {
            let Some(c) = q.pop_front() else { break };
            let st = state(&c);
            if st == "missing" || st == "outdated" { pick = Some((c, true)); } else if st == "old" { pick = Some((c, false)); }
        }
    }
    if let Some((g, first)) = pick { improve_quietly_as(&g, first); }
}
/// While paused it waits in the queue (as an "old" game, it's picked as an improvement then).
pub fn improve_quietly(g: &GameEntry) {
    if paused() { let mut q = QUEUED.lock().unwrap(); if !q.iter().any(|x| x.id == g.id) { q.push_back(g.clone()); } return; }
    improve_quietly_as(g, false);
}
fn improve_quietly_as(g: &GameEntry, first: bool) {
    if busy() || !zig_exe().is_file() { return; }
    let st = state(g);
    if st != if first { state(g) } else { "old".to_string() } { return; }
    if first && crate::ctr_header_fix::needs(&plain(g)) { return; }
    let covered = hint_count(g);
    let Ok(mut psi) = build_into(g, &next(g), &quiet_work()) else { return };
    psi.idle = Some(mask_of((processors() / 2).clamp(1, 64)));
    let verb = if first { "Building" } else { "Improving" };
    if let Some(d) = DRY.lock().unwrap().as_mut() { d.push(proc::dry_line(&format!("{} {}", verb, g.title), &psi, false)); return; }
    set_progress(0.0);
    { let mut b = BANNER.lock().unwrap(); b.title = Some(g.title.clone()); b.verb = verb.into(); }
    let mut q = QUIET.lock().unwrap();
    let Ok(mut child) = proc::command_for(&psi).spawn() else { return };
    proc::apply_priority(&child, &psi);
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let re = regex::Regex::new(r"^nativemons: (\d+) ").unwrap();
            for l in std::io::BufReader::new(out).lines().map_while(Result::ok) { if let Some(m) = re.captures(&l) { if let Ok(p) = m[1].parse::<i32>() { set_progress(p as f32 / 100.0); } } }
        });
    }
    if let Some(mut err) = child.stderr.take() { std::thread::spawn(move || { let _ = std::io::copy(&mut err, &mut std::io::sink()); }); }
    let suspended = paused();
    if suspended { suspend_tree(child.id(), true); }
    *q = Some(Quiet { pid: child.id(), game: g.clone(), suspended });
    drop(q);
    let g = g.clone();
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        let ok = code == 0 && next(&g).is_file();
        if quiet_work().is_dir() { let _ = std::fs::remove_dir_all(quiet_work()); }
        delete_leftovers();
        if dir().join("zig-cache").is_dir() { let _ = std::fs::remove_dir_all(dir().join("zig-cache")); }
        if ok {
            let _ = std::fs::write(plus(&next(&g), ".covers"), covered.to_string());
            swap_in(&g);
            BANNER.lock().unwrap().done_at = Some(Instant::now());
        } else {
            let _ = std::fs::remove_file(next(&g));
            let _ = std::fs::remove_file(plus(&next(&g), ".shared"));
        }
        *QUIET.lock().unwrap() = None;
        next_queued();
    });
}
/// The improved library takes over unless Zakuro has the old one open; then at the next Play.
pub fn swap_in(g: &GameEntry) {
    let n = next(g);
    if !n.is_file() || crate::saves::emu_running(&proc::processes(), &crate::emu3ds::ZAKURO) { return; }
    let _ = (|| -> std::io::Result<()> {
        let covers_f = plus(&n, ".covers");
        let covers = if covers_f.is_file() { std::fs::read_to_string(&covers_f)?.trim().to_string() } else { hint_count(g).to_string() };
        let used = used_shared(&n);
        std::fs::create_dir_all(dir())?;
        std::fs::copy(&n, library_of(g))?;
        std::fs::remove_file(&n)?;
        if covers_f.exists() { std::fs::remove_file(&covers_f)?; }
        config::write_all_lines(&stamp_of(g), &[rom_key(g), covers, used])
    })();
}
/// A quiet rebuild cut off by the launcher closing leaves its work folder: cleared at start.
pub fn clear_leftovers() {
    if busy() { return; }
    if quiet_work().is_dir() { let _ = std::fs::remove_dir_all(quiet_work()); }
    if work().is_dir() { let _ = std::fs::remove_dir_all(work()); }
}
pub fn stop_quiet() {
    QUEUED.lock().unwrap().clear();
    let q = QUIET.lock().unwrap().as_ref().map(|q| (q.pid, q.game.clone()));
    let Some((pid, g)) = q else { return };
    stop(pid);
    if quiet_work().is_dir() { let _ = std::fs::remove_dir_all(quiet_work()); }
    let _ = std::fs::remove_file(next(&g));
}
/// The process and everything it started (taskkill /T /F).
pub fn stop(pid: u32) {
    let psi = if cfg!(windows) { StartInfo::new("taskkill", &format!("/T /F /PID {}", pid)) } else { StartInfo::new("kill", &format!("-9 {}", pid)) };
    let _ = proc::run(&psi, 15);
}
/// The newest pieces stay; past the cap the oldest go.
fn prune_cache(cap: u64) {
    let Ok(rd) = std::fs::read_dir(cache()) else { return };
    let mut files: Vec<(PathBuf, u64, i64)> = rd.flatten().filter(|e| e.path().is_file()).map(|e| { let p = e.path(); let len = e.metadata().map(|m| m.len()).unwrap_or(0); let t = crate::library::last_write_ticks(&p); (p, len, t) }).collect();
    files.sort_by(|a, b| b.2.cmp(&a.2));
    let mut total = 0u64;
    for (p, len, _) in files {
        total += len;
        if total > cap || p.to_string_lossy().ends_with(".part") { let _ = std::fs::remove_file(&p); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_minutes_and_the_dry_note() {
        assert_eq!(mask_of(1), 1);
        assert_eq!(mask_of(27), 0x7FF_FFFF);
        assert_eq!(mask_of(64), u64::MAX);
        assert_eq!(minutes(4), 18);
        assert_eq!(minutes(27), 5);
        assert_eq!(minutes(64), 3);
        let mut psi = StartInfo::new("tool.exe", "build \"a\"");
        psi.idle = Some(mask_of(14));
        assert_eq!(proc::dry_line("Building Pokemon X", &psi, false), "Building Pokemon X | tool.exe | build \"a\" |  |  | idle 3FFF");
    }
}

#[cfg(all(test, windows))]
mod pause_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    /// Light mode's pause on a real process tree (cmd counting into a pipe): no output while paused, more after.
    #[test]
    fn pause_suspends_the_build_tree_and_resume_goes_on() {
        use std::io::BufRead;
        let mut child = std::process::Command::new("cmd").args(["/c", "for /l %i in (1,1,2000000) do @echo %i"]).stdout(std::process::Stdio::piped()).spawn().unwrap();
        let out = child.stdout.take().unwrap();
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        std::thread::spawn(move || for _ in std::io::BufReader::new(out).lines().map_while(Result::ok) { n2.fetch_add(1, Ordering::SeqCst); });
        std::thread::sleep(std::time::Duration::from_millis(300));
        *QUIET.lock().unwrap() = Some(Quiet { pid: child.id(), game: GameEntry::default(), suspended: false });
        set_paused(true);
        assert!(paused() && QUIET.lock().unwrap().as_ref().unwrap().suspended);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let a = n.load(Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert_eq!(n.load(Ordering::SeqCst), a, "output while paused");
        // While paused, an improvement waits in the queue instead of starting.
        let mut g = GameEntry::default();
        g.id = "pause-test".into();
        improve_quietly(&g);
        assert!(queued(&g));
        QUEUED.lock().unwrap().clear();
        set_paused(false);
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(n.load(Ordering::SeqCst) > a, "no output after resuming");
        assert!(!QUIET.lock().unwrap().as_ref().unwrap().suspended);
        *QUIET.lock().unwrap() = None;
        stop(child.id());
        let _ = child.wait();
    }
}
