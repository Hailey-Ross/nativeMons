//! The engines as the launcher drives them without a window (LauncherForm's "the engines" region): Gen1Recomp's data
//! folder and its one-time move (EnsureGen1Data), Gen1Start, Gen1Cli, RunCapture and PlainPath.

use crate::config;
use crate::config::paths;
use crate::proc::{self, StartInfo};
use crate::save_guard;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Gen1Recomp's save folder, apart from any other install's saves.
pub const IDENTITY: &str = "nativemons";
const DATA_IDENTITY: &str = "nativemons-portable";

pub fn gen1_data() -> PathBuf { config::state().join("gen1recomp") }
/// LOVE's folder follows APPDATA, so the launcher reads it the same way (and a test can point both elsewhere).
fn love_root() -> PathBuf { paths().app_data().join("LOVE") }
pub fn old_gen1_data() -> PathBuf { love_root().join(IDENTITY) }
/// Where its saves are now: the old folder only while the move hasn't happened.
pub fn gen1_root() -> PathBuf { if gen1_data().is_dir() { gen1_data() } else { old_gen1_data() } }
pub fn gen1_data_move_needed() -> bool { !gen1_data().is_dir() && old_gen1_data().is_dir() }

static GEN1_DATA: Mutex<Option<String>> = Mutex::new(None);

/// None once the folder is there. A move that fails leaves nothing half done, and is tried again at the next start.
pub fn ensure_gen1_data() -> Option<String> {
    let mut problem = GEN1_DATA.lock().unwrap();
    if gen1_data().is_dir() { return None; }
    if problem.is_some() { return problem.clone(); }
    let part = PathBuf::from(format!("{}.part", gen1_data().display()));
    let old = old_gen1_data();
    let r = (|| -> std::io::Result<()> {
        if part.is_dir() { std::fs::remove_dir_all(&part)?; }
        std::fs::create_dir_all(&part)?;
        let mut files = 0;
        if old.is_dir() { files = copy_checked(&old, &part)?; }
        let when = save_guard::now("yyyy-MM-dd HH:mm");
        let text = if old.is_dir() {
            format!("Copied from {} on {}, {} files, each checked. The old folder was left as it was.\r\n", old.display(), when, files)
        } else {
            format!("Started here on {}.\r\n", when)
        };
        std::fs::write(part.join("nativemons-moved.txt"), text)?;
        std::fs::rename(&part, gen1_data())
    })();
    match r {
        Ok(()) => None,
        Err(x) => {
            if part.is_dir() { let _ = std::fs::remove_dir_all(&part); }
            *problem = Some(format!("Gen1Recomp's saves couldn't be moved beside the launcher, so they stay where they were, in {}. It'll try again next time. {}", old.display(), x));
            problem.clone()
        }
    }
}

fn walk(dir: &Path, dirs: &mut Vec<PathBuf>, files: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut subs = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() { subs.push(p); } else { files.push(p); }
    }
    for s in subs {
        dirs.push(s.clone());
        walk(&s, dirs, files);
    }
}
/// Every file copied, then read back: same length and the same hash, or the whole move is off.
fn copy_checked(from: &Path, to: &Path) -> std::io::Result<usize> {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    walk(from, &mut dirs, &mut files);
    for d in &dirs { std::fs::create_dir_all(to.join(d.strip_prefix(from).unwrap()))?; }
    let mut n = 0;
    for f in &files {
        let copy = to.join(f.strip_prefix(from).unwrap());
        std::fs::copy(f, &copy)?;
        if std::fs::metadata(&copy)?.len() != std::fs::metadata(f)?.len() || save_guard::hash_of(&copy) != save_guard::hash_of(f) {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("A copy didn't match: {}", f.display())));
        }
        n += 1;
    }
    Ok(n)
}

/// A folder path any program reads correctly from an environment variable: its short form when it isn't plain ASCII.
pub fn plain_dir(dir: &str) -> String {
    if dir.is_ascii() { return dir.to_string(); }
    proc::short_path(dir).unwrap_or_else(|| dir.to_string())
}

/// Gen1Recomp's start: LOVE with the fork's folder, its data beside the launcher once that's there.
fn gen1_art_data() -> PathBuf { config::state().join("gen1recomp-art") }
/// UseArtIdentity: a throwaway identity (and data folder) for a picture capture.
pub fn use_art_identity(psi: &mut StartInfo, identity: &str) {
    psi.set_env("POKEPORT_IDENTITY", identity);
    if psi.has_env("NATIVEMONS_DATA_DIR") {
        let _ = std::fs::create_dir_all(gen1_art_data());
        psi.set_env("NATIVEMONS_DATA_DIR", &plain_dir(&gen1_art_data().to_string_lossy()));
    }
}
/// RemoveArtData: what that identity left.
pub fn remove_art_data(identity: &str) {
    for d in [gen1_art_data(), love_root().join(identity)] { if d.is_dir() { let _ = std::fs::remove_dir_all(&d); } }
}
pub fn gen1_start(args: &str) -> StartInfo {
    let mut psi = StartInfo::new(config::love(), &format!("\"{}\"{}", config::gen1().display(), args));
    psi.cwd = Some(config::gen1());
    if ensure_gen1_data().is_none() {
        psi.set_env("POKEPORT_IDENTITY", DATA_IDENTITY);
        psi.set_env("NATIVEMONS_DATA_DIR", &plain_dir(&gen1_data().to_string_lossy()));
    } else {
        psi.set_env("POKEPORT_IDENTITY", IDENTITY);
    }
    // The launcher shows its own Discord status, so Gen1Recomp's is off.
    psi.set_env("POKEPORT_NO_DISCORD", "1");
    psi
}

/// Asks the Gen1Recomp fork something (src/nativemons/ModsCli.lua). None when it can't run.
pub fn gen1_cli(command: &str) -> Option<String> {
    if !config::love().is_file() || !config::gen1().join("main.lua").is_file() { return None; }
    let mut psi = gen1_start("");
    psi.set_env("NATIVEMONS_CLI", command);
    if let Some((o, _)) = gen1_stub(&psi) { return Some(o); }
    match proc::run(&psi, 30) {
        Ok(f) if f.code.is_some() => Some(f.out),
        _ => None,
    }
}

/// A replay's answer for Gen1Recomp's command line (test_hooks::gen1), never run.
fn gen1_stub(psi: &StartInfo) -> Option<(String, i32)> {
    if psi.program != config::love() { return None; }
    let cmd = psi.env.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case("NATIVEMONS_CLI"))?.1.clone();
    crate::test_hooks::gen1(&cmd)
}

/// RunCapture: output (or its errors when it failed silently) with the audio library's noise left out. (text, code)
pub fn run_capture(psi: &StartInfo, seconds: u64) -> (String, i32) {
    if let Some((o, code)) = gen1_stub(psi) {
        let plain = o.split('\n').filter(|l| !l.starts_with("AL lib:") && !l.starts_with('[')).collect::<Vec<_>>().join("\n");
        return (if code != 0 && plain.trim().is_empty() { o } else { plain }, code);
    }
    if let Some(o) = crate::test_hooks::art_run(psi) { return (o, 0); }
    // On the capture thread: watched, so it can be stopped, and at a capture's priority.
    let art = crate::art_jobs::on_capture_thread();
    let mut psi = psi.clone();
    if art { crate::art_jobs::lower(&mut psi); }
    let pid = std::cell::Cell::new(0);
    let ran = proc::run_watched(&psi, seconds, &|p| if art { pid.set(p); crate::art_jobs::watch(p) });
    crate::art_jobs::unwatch(pid.get());
    let f = match ran {
        Ok(f) => f,
        Err(e) => return (e.to_string(), -1),
    };
    let Some(code) = f.code else { return ("That took too long and was stopped.".into(), -1) };
    let shown = if code == 0 || !f.out.trim().is_empty() { &f.out } else { &f.err };
    let plain: Vec<&str> = shown.split('\n').filter(|l| !l.starts_with("AL lib:") && !l.starts_with('[')).collect();
    let plain = plain.join("\n");
    if code != 0 && plain.trim().is_empty() { (format!("{}{}", f.err, f.out), code) } else { (plain, code) }
}

/// Gen1Recomp garbles accented paths, so such files go in by their short 8.3 name, or as plain-named copies.
pub struct PlainPath {
    pub path: String,
    temp: Option<PathBuf>,
}
impl PlainPath {
    pub fn new(file: &str) -> std::io::Result<PlainPath> {
        if file.is_ascii() { return Ok(PlainPath { path: file.into(), temp: None }); }
        if let Some(s) = proc::short_path(file).filter(|s| s.is_ascii()) { return Ok(PlainPath { path: s, temp: None }); }
        let ext = Path::new(file).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let temp = proc::temp_dir().join(format!("nativemons-{}{}", proc::new_guid(), ext));
        std::fs::copy(file, &temp)?;
        let t = temp.to_string_lossy().into_owned();
        let path = if !t.is_ascii() { proc::short_path(&t).unwrap_or(t) } else { t };
        Ok(PlainPath { path, temp: Some(temp) })
    }
}
impl Drop for PlainPath {
    fn drop(&mut self) { if let Some(t) = &self.temp { let _ = std::fs::remove_file(t); } }
}

/// Gen1Recomp prints Windows paths with forward slashes; the C# turns them back (on Windows only here).
pub fn win_sep(s: &str) -> String { if cfg!(windows) { s.replace('/', "\\") } else { s.to_string() } }
