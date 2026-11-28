//! The C# TestHooks' stubs below the UI, inert outside a replay: downloads answered from a folder
//! (NATIVEMONS_TEST_FETCH), Gen1Recomp's command line from a file (NATIVEMONS_TEST_GEN1_CLI), and in a dry-run
//! replay with NATIVEMONS_TEST_FLOWS=1 Python and a download's checksum. Notes go to the replay's effects.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static REPLAY: AtomicBool = AtomicBool::new(false);
static DRY: AtomicBool = AtomicBool::new(false);
static NOTES: Mutex<Vec<(&str, String)>> = Mutex::new(Vec::new());

fn var(n: &str) -> Option<String> { std::env::var(n).ok().filter(|v| !v.is_empty()) }
/// --replay started (TestHooks.Replay), and whether it's a dry run.
pub fn set_replay(on: bool, dry: bool) { REPLAY.store(on, Ordering::SeqCst); DRY.store(dry, Ordering::SeqCst); }
pub fn replay() -> bool { REPLAY.load(Ordering::SeqCst) }
/// TestHooks.Flows: NATIVEMONS_TEST_FLOWS=1 in a dry-run replay (or a window test's live stubs, Rust only).
pub fn flows() -> bool { ((replay() && DRY.load(Ordering::SeqCst)) || live_stubs()) && var("NATIVEMONS_TEST_FLOWS").as_deref() == Some("1") }
/// Window tests only (Rust): NATIVEMONS_TEST_LIVE_STUBS=1 with a test log lets the live launcher use the same stubs.
pub fn live_stubs() -> bool { var("NATIVEMONS_TEST_LIVE_STUBS").as_deref() == Some("1") && var("NATIVEMONS_TEST_LOG").is_some() }
/// A note for the replay's effects: ("Fetch", url) or ("Gen1Cli", command).
pub fn note(kind: &'static str, what: &str) { NOTES.lock().unwrap().push((kind, what.to_string())); }
/// What was noted since the last call, in order.
pub fn take_notes() -> Vec<(&'static str, String)> { std::mem::take(&mut *NOTES.lock().unwrap()) }

/// Environment.ExpandEnvironmentVariables: %NAME% of a set variable is replaced, anything else stays.
pub fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('%') {
            Some(j) => match std::env::var(&after[..j]) {
                Ok(v) if j > 0 => { out.push_str(&v); rest = &after[j + 1..]; }
                _ => { out.push('%'); rest = after; }
            },
            None => { out.push('%'); rest = after; }
        }
    }
    out.push_str(rest);
    out
}

/// TestHooks.Fetch: None outside such a replay; else the folder's file named after the URL, or a 404.
pub fn fetch(url: &str, noted: bool) -> Option<Result<Vec<u8>, String>> {
    let dir = var("NATIVEMONS_TEST_FETCH")?;
    if !replay() && !live_stubs() { return None; }
    if noted { note("Fetch", url); }
    let name: String = url.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    Some(std::fs::read(std::path::Path::new(&dir).join(name)).map_err(|_| "The remote server returned an error: (404) Not Found.".to_string()))
}

/// TestHooks.Gen1: Gen1Recomp's answer to `command` from the file (exit code 1 and nothing when unlisted).
pub fn gen1(command: &str) -> Option<(String, i32)> {
    let file = var("NATIVEMONS_TEST_GEN1_CLI")?;
    if !replay() && var("NATIVEMONS_TEST_ART_SHOTS").is_none() { return None; }
    note("Gen1Cli", command);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    for l in text.lines() {
        let f: Vec<&str> = l.splitn(3, '\t').collect();
        if f.len() == 3 && expand_env(f[0]) == command { return Some((expand_env(&f[2].replace("\\n", "\n")), f[1].trim().parse().unwrap_or(1))); }
    }
    Some((String::new(), 1))
}

/// TestHooks.ArtShots (NATIVEMONS_TEST_ART_SHOTS, tests only): a capture's engine isn't started; its plan is added to
/// NATIVEMONS_TEST_ART_PLAN and the folder's pictures are copied into `temp` as if it had made them.
pub fn art_shots(psi: &crate::proc::StartInfo, temp: Option<&std::path::Path>) -> bool {
    let Some(dir) = var("NATIVEMONS_TEST_ART_SHOTS") else { return false };
    let plain = crate::proc::StartInfo { program: psi.program.clone(), args: psi.args.clone(), env: psi.env.clone(), cwd: psi.cwd.clone(), ..Default::default() };
    let line = format!("{}\r\n", crate::proc::dry_line("Capture", &plain, false));
    if let Some(plan) = var("NATIVEMONS_TEST_ART_PLAN") {
        use std::io::Write;
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(plan).and_then(|mut f| f.write_all(line.as_bytes()));
    }
    if let Some(temp) = temp {
        for f in std::fs::read_dir(&dir).into_iter().flatten().flatten() { if f.path().is_file() { let _ = std::fs::copy(f.path(), temp.join(f.file_name())); } }
    }
    // Window tests (Rust only): the capture "runs" this long, so it can be seen behind the menu; a stop ends it early.
    if let Some(ms) = var("NATIVEMONS_TEST_ART_DELAY").and_then(|v| v.parse::<u64>().ok()) {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < end && !crate::art_jobs::hold() { std::thread::sleep(std::time::Duration::from_millis(100)); }
    }
    true
}
/// TestHooks.ArtRun: RunCapture in such a test, noted the same way; pictures go to its NATIVEMONS_TOWN_ART folder and
/// that run's output is NATIVEMONS_TEST_ART_SAID's text.
pub fn art_run(psi: &crate::proc::StartInfo) -> Option<String> {
    let many = psi.env.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case("NATIVEMONS_TOWN_ART")).map(|(_, v)| v.clone());
    if !art_shots(psi, many.as_deref().map(std::path::Path::new)) { return None; }
    let said = var("NATIVEMONS_TEST_ART_SAID").filter(|_| many.is_some());
    Some(said.and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default())
}

/// TestHooks.Python: whether Python 3 works (NATIVEMONS_TEST_PYTHON=yes), in a flows replay only.
pub fn python() -> Option<bool> { if flows() { Some(var("NATIVEMONS_TEST_PYTHON").as_deref() == Some("yes")) } else { None } }
/// TestHooks.Sha: a download's expected SHA-256 (NATIVEMONS_TEST_SHA256 in a flows replay), else the real one.
pub fn sha(real: &str) -> String { if flows() { if let Some(s) = var("NATIVEMONS_TEST_SHA256") { return s; } } real.to_string() }
