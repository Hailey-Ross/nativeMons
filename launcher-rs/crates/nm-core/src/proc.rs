//! Processes as the C# drives them: ProcessStartInfo with a raw argument string, captured output, a time limit, and
//! Process.GetProcessesByName with each one's program path and parent.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct StartInfo {
    pub program: PathBuf,
    /// The arguments as one string, passed on unchanged (ProcessStartInfo.Arguments).
    pub args: String,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    pub below_normal: bool,
    /// Idle priority on these cores (PriorityClass = Idle, ProcessorAffinity), set just after the start.
    pub idle: Option<u64>,
    /// RedirectStandardInput: the waiting game is told "play" (and to quit) on its input.
    pub stdin: bool,
}

impl StartInfo {
    pub fn new(program: impl Into<PathBuf>, args: &str) -> StartInfo { StartInfo { program: program.into(), args: args.to_string(), ..Default::default() } }
    /// psi.EnvironmentVariables[k] = v: the last one set wins.
    pub fn set_env(&mut self, k: &str, v: &str) {
        self.env.retain(|(a, _)| !a.eq_ignore_ascii_case(k));
        self.env.push((k.to_string(), v.to_string()));
    }
    pub fn has_env(&self, k: &str) -> bool { self.env.iter().any(|(a, _)| a.eq_ignore_ascii_case(k)) }
}

pub struct Finished {
    pub out: String,
    pub err: String,
    /// None when it ran past its time and was stopped.
    pub code: Option<i32>,
}

#[cfg(not(windows))]
fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in s.chars() {
        match c {
            '"' => { quoted = !quoted; any = true; }
            ' ' | '\t' if !quoted => { if any || !cur.is_empty() { out.push(std::mem::take(&mut cur)); } any = false; }
            _ => cur.push(c),
        }
    }
    if any || !cur.is_empty() { out.push(cur); }
    out
}

fn command(psi: &StartInfo) -> Command {
    let mut c = Command::new(&psi.program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if !psi.args.is_empty() { c.raw_arg(&psi.args); }
        // CREATE_NO_WINDOW, as CreateNoWindow = true.
        c.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    c.args(split_args(&psi.args));
    for (k, v) in &psi.env { c.env(k, v); }
    if let Some(d) = &psi.cwd { c.current_dir(d); }
    c.stdin(if psi.stdin { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    c
}

#[cfg(windows)]
fn lower_priority(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" { fn SetPriorityClass(h: *mut std::ffi::c_void, class: u32) -> i32; }
    unsafe { SetPriorityClass(child.as_raw_handle() as _, 0x4000) };
}
#[cfg(not(windows))]
fn lower_priority(_child: &std::process::Child) {}
/// PriorityClass = Normal again.
#[cfg(windows)]
pub fn normal_priority(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" { fn SetPriorityClass(h: *mut std::ffi::c_void, class: u32) -> i32; }
    unsafe { SetPriorityClass(child.as_raw_handle() as _, 0x20) };
}
#[cfg(not(windows))]
pub fn normal_priority(_child: &std::process::Child) {}

/// PriorityClass = Idle, then ProcessorAffinity = mask; failures are ignored, as the C#'s try.
#[cfg(windows)]
fn idle_on(child: &std::process::Child, mask: u64) {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" {
        fn SetPriorityClass(h: *mut std::ffi::c_void, class: u32) -> i32;
        fn SetProcessAffinityMask(h: *mut std::ffi::c_void, mask: usize) -> i32;
    }
    let h = child.as_raw_handle() as _;
    unsafe { if SetPriorityClass(h, 0x40) != 0 { SetProcessAffinityMask(h, mask as usize); } }
}
#[cfg(not(windows))]
fn idle_on(child: &std::process::Child, _mask: u64) {
    let _ = Command::new("renice").args(["-n", "19", "-p", &child.id().to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
}
/// Process.Kill by id; a process already gone is ignored.
#[cfg(windows)]
pub fn kill_pid(pid: u32) {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn TerminateProcess(h: isize, code: u32) -> i32;
        fn CloseHandle(h: isize) -> i32;
    }
    unsafe { let h = OpenProcess(0x0001, 0, pid); if h != 0 { TerminateProcess(h, u32::MAX); CloseHandle(h); } }
}
#[cfg(not(windows))]
pub fn kill_pid(pid: u32) { let _ = Command::new("kill").args(["-9", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
/// What the StartInfo asks for once started: BelowNormal, or Idle on some cores.
pub fn apply_priority(child: &std::process::Child, psi: &StartInfo) {
    if psi.below_normal { lower_priority(child); }
    if let Some(m) = psi.idle { idle_on(child, m); }
}

/// Starts it, reads both outputs to the end and waits at most `seconds`; a run past that is killed.
pub fn run(psi: &StartInfo, seconds: u64) -> std::io::Result<Finished> { run_watched(psi, seconds, &|_| {}) }
/// The same, telling `started` the process id once it runs (GameArt.Watch).
pub fn run_watched(psi: &StartInfo, seconds: u64, started: &dyn Fn(u32)) -> std::io::Result<Finished> {
    let mut child = command(psi).spawn()?;
    apply_priority(&child, psi);
    started(child.id());
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let to = std::thread::spawn(move || { let mut b = Vec::new(); let _ = so.read_to_end(&mut b); b });
    let te = std::thread::spawn(move || { let mut b = Vec::new(); let _ = se.read_to_end(&mut b); b });
    let end = Instant::now() + Duration::from_secs(seconds);
    let code = loop {
        if let Some(s) = child.try_wait()? { break Some(s.code().unwrap_or(-1)); }
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    let out = String::from_utf8_lossy(&to.join().unwrap_or_default()).into_owned();
    let err = String::from_utf8_lossy(&te.join().unwrap_or_default()).into_owned();
    Ok(Finished { out, err, code })
}

/// A game or tool the launcher started and watches (RunJob's Process): its output lines, its end, its window.
pub trait RunningJob {
    fn id(&self) -> u32;
    /// Output lines (stdout and stderr) that arrived since the last call.
    fn lines(&mut self) -> Vec<String>;
    /// Some(code) once it has ended.
    fn exit_code(&mut self) -> Option<i32>;
    fn kill(&mut self);
    /// Its main window is up and visible (WatchGame: the launcher hides then).
    fn window_shown(&mut self) -> bool;
    /// Brings its window to the front.
    fn front(&mut self) {}
    /// Its main window's handle and title (Process.MainWindowHandle / MainWindowTitle), when it has one.
    fn main_window(&mut self) -> Option<(isize, String)> { None }
    /// PostMessage of a key down and up to its main window (WatchGba).
    fn post_key(&mut self, _vk: u32, _down_lparam: isize, _up_lparam: isize) {}
    /// One line on its input (the waiting game's "play").
    fn write_line(&mut self, _line: &str) {}
    /// PriorityClass = Normal (UseReady: the waiting game becomes the game played).
    fn normal_priority(&mut self) {}
    /// DropReady: its input closed (it quits), killed after `grace_ms`; the thread doing it is returned to wait on.
    fn end_gently(self: Box<Self>, _grace_ms: u64) -> Option<std::thread::JoinHandle<()>> { let mut me = self; me.kill(); None }
}
/// Starts jobs (the platform's; a dry run notes them instead).
pub trait Spawner {
    fn start(&mut self, psi: &StartInfo) -> std::io::Result<Box<dyn RunningJob>>;
}
/// The same Command the CLI runs use, for a platform spawner.
pub fn command_for(psi: &StartInfo) -> Command { command(psi) }
/// The one-line form a dry run notes: title | program | args | cwd | env (only what differs from ours, sorted) [| game]
/// [| idle <affinity mask, hex>].
pub fn dry_line(title: &str, psi: &StartInfo, is_game: bool) -> String {
    let mut env: Vec<String> = psi.env.iter().filter(|(k, v)| std::env::var(k).unwrap_or_default() != *v).map(|(k, v)| format!("{}={}", k.to_uppercase(), v)).collect();
    env.sort();
    format!("{} | {} | {} | {} | {}{}{}", title, psi.program.display(), psi.args, psi.cwd.as_ref().map(|d| d.display().to_string()).unwrap_or_default(), env.join(";"), if is_game { " | game" } else { "" },
        psi.idle.map(|m| format!(" | idle {:X}", m)).unwrap_or_else(|| if psi.below_normal { " | below normal".into() } else { String::new() }))
}

/// One running process: id, name without ".exe", program path (None when it can't be read) and parent id.
#[derive(Clone, Debug)]
pub struct ProcInfo {
    pub id: u32,
    pub name: String,
    pub exe: Option<String>,
    pub parent: u32,
}

#[cfg(windows)]
pub fn processes() -> Vec<ProcInfo> {
    #[repr(C)]
    struct Entry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        pri: i32,
        flags: u32,
        exe: [u16; 260],
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        fn Process32FirstW(h: isize, e: *mut Entry) -> i32;
        fn Process32NextW(h: isize, e: *mut Entry) -> i32;
        fn CloseHandle(h: isize) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(h: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(2, 0);
        if snap == -1 { return out; }
        let mut e: Entry = std::mem::zeroed();
        e.size = std::mem::size_of::<Entry>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let len = e.exe.iter().position(|&c| c == 0).unwrap_or(260);
            let file = String::from_utf16_lossy(&e.exe[..len]);
            let name = match file.rfind('.') { Some(i) if file[i..].eq_ignore_ascii_case(".exe") => file[..i].to_string(), _ => file.clone() };
            let mut exe = None;
            let h = OpenProcess(0x1000, 0, e.pid);
            if h != 0 {
                let mut buf = [0u16; 1024];
                let mut n = buf.len() as u32;
                if QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n) != 0 { exe = Some(String::from_utf16_lossy(&buf[..n as usize])); }
                CloseHandle(h);
            }
            out.push(ProcInfo { id: e.pid, name, exe, parent: e.parent });
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
pub fn processes() -> Vec<ProcInfo> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return out };
    for d in rd.flatten() {
        let Ok(id) = d.file_name().to_string_lossy().parse::<u32>() else { continue };
        let name = std::fs::read_to_string(d.path().join("comm")).map(|s| s.trim().to_string()).unwrap_or_default();
        let exe = std::fs::read_link(d.path().join("exe")).ok().map(|p| p.to_string_lossy().into_owned());
        let parent = std::fs::read_to_string(d.path().join("stat")).ok().and_then(|s| s.rsplit(')').next().and_then(|r| r.split_whitespace().nth(1)).and_then(|p| p.parse().ok())).unwrap_or(0);
        out.push(ProcInfo { id, name, exe, parent });
    }
    out
}

/// Process.GetProcessesByName: the name ignoring case, without ".exe".
pub fn by_name(all: &[ProcInfo], name: &str) -> Vec<ProcInfo> { all.iter().filter(|p| p.name.eq_ignore_ascii_case(name)).cloned().collect() }

/// GetShortPathName: the 8.3 form of an existing path, or None.
#[cfg(windows)]
pub fn short_path(p: &str) -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" { fn GetShortPathNameW(long: *const u16, short: *mut u16, n: u32) -> u32; }
    let w: Vec<u16> = p.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buf = vec![0u16; 600];
    let n = unsafe { GetShortPathNameW(w.as_ptr(), buf.as_mut_ptr(), 600) };
    if n == 0 || n as usize >= buf.len() { return None; }
    Some(String::from_utf16_lossy(&buf[..n as usize]))
}
#[cfg(not(windows))]
pub fn short_path(_p: &str) -> Option<String> { None }

/// Path.GetTempPath without the trailing separator.
pub fn temp_dir() -> PathBuf { std::env::temp_dir() }

/// Guid.NewGuid().ToString("N"): 32 random hex digits.
pub fn new_guid() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut s = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i ^ std::process::id() as u64);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        s += &format!("{:016x}", h.finish());
    }
    s
}

