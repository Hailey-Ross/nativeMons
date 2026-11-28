//! RunJob's processes: started with the CLI's Command (raw arguments, no console window), output read line by line on
//! two threads, the end polled each tick, and the main window found by process id (WatchGame).

use nm_core::proc::{RunningJob, Spawner, StartInfo};
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::mpsc::{channel, Receiver};

pub struct SystemSpawner;
struct Job {
    child: Child,
    input: Option<std::process::ChildStdin>,
    rx: Receiver<String>,
    code: Option<i32>,
}

impl Spawner for SystemSpawner {
    fn start(&mut self, psi: &StartInfo) -> std::io::Result<Box<dyn RunningJob>> {
        let mut child = nm_core::proc::command_for(psi).spawn()?;
        nm_core::proc::apply_priority(&child, psi);
        let input = child.stdin.take();
        let (tx, rx) = channel();
        if let Some(out) = child.stdout.take() { let t = tx.clone(); std::thread::spawn(move || for l in BufReader::new(out).lines().map_while(Result::ok) { let _ = t.send(l); }); }
        if let Some(err) = child.stderr.take() { std::thread::spawn(move || for l in BufReader::new(err).lines().map_while(Result::ok) { let _ = tx.send(l); }); }
        Ok(Box::new(Job { child, input, rx, code: None }))
    }
}

impl RunningJob for Job {
    fn id(&self) -> u32 { self.child.id() }
    fn lines(&mut self) -> Vec<String> { self.rx.try_iter().collect() }
    fn exit_code(&mut self) -> Option<i32> {
        if self.code.is_none() { if let Ok(Some(s)) = self.child.try_wait() { self.code = Some(s.code().unwrap_or(-1)); } }
        self.code
    }
    fn kill(&mut self) { let _ = self.child.kill(); }
    fn window_shown(&mut self) -> bool { main_window(self.child.id()).is_some() }
    fn front(&mut self) { if let Some(w) = main_window(self.child.id()) { front(w); } }
    fn main_window(&mut self) -> Option<(isize, String)> { main_window(self.child.id()).map(|w| (w, window_title(w))) }
    fn post_key(&mut self, vk: u32, down: isize, up: isize) { if let Some(w) = main_window(self.child.id()) { post_key(w, vk, down, up); } }
    fn write_line(&mut self, line: &str) {
        use std::io::Write;
        if let Some(i) = self.input.as_mut() { let _ = writeln!(i, "{}", line); let _ = i.flush(); }
    }
    fn normal_priority(&mut self) { nm_core::proc::normal_priority(&self.child); }
    fn end_gently(self: Box<Self>, grace_ms: u64) -> Option<std::thread::JoinHandle<()>> {
        let mut me = *self;
        Some(std::thread::spawn(move || {
            drop(me.input.take());
            let end = std::time::Instant::now() + std::time::Duration::from_millis(grace_ms);
            while std::time::Instant::now() < end { if let Ok(Some(_)) = me.child.try_wait() { return; } std::thread::sleep(std::time::Duration::from_millis(50)); }
            let _ = me.child.kill();
            let _ = me.child.wait();
        }))
    }
}

/// GetWindowText of a window.
#[cfg(windows)]
pub fn window_title(w: isize) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(w, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}
#[cfg(unix)]
pub fn window_title(w: isize) -> String { crate::x11::window_title(w) }
#[cfg(not(any(windows, unix)))]
pub fn window_title(_w: isize) -> String { String::new() }
/// PostMessage of a key down and up (WatchGba's Enter for a hack's list).
#[cfg(windows)]
pub fn post_key(w: isize, vk: u32, down_lparam: isize, up_lparam: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
    unsafe { PostMessageW(w, 0x100, vk as usize, down_lparam); PostMessageW(w, 0x101, vk as usize, up_lparam); }
}
#[cfg(unix)]
pub fn post_key(w: isize, vk: u32, _d: isize, _u: isize) { crate::x11::post_key(w, vk) }
#[cfg(not(any(windows, unix)))]
pub fn post_key(_w: isize, _vk: u32, _d: isize, _u: isize) {}

/// `pid`'s main window (PlaySnaps' grab).
pub fn main_window_of(pid: u32) -> Option<isize> { main_window(pid) }
/// The process's first visible top-level window without an owner (Process.MainWindowHandle, as far as WatchGame asks).
#[cfg(windows)]
fn main_window(pid: u32) -> Option<isize> {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER};
    struct Find { pid: u32, found: HWND }
    unsafe extern "system" fn each(w: HWND, l: LPARAM) -> BOOL {
        let f = &mut *(l as *mut Find);
        let mut p = 0u32;
        GetWindowThreadProcessId(w, &mut p);
        if p == f.pid && IsWindowVisible(w) != 0 && GetWindow(w, GW_OWNER) == 0 { f.found = w; return 0; }
        1
    }
    let mut f = Find { pid, found: 0 };
    unsafe { EnumWindows(Some(each), &mut f as *mut Find as LPARAM) };
    if f.found != 0 { Some(f.found) } else { None }
}
#[cfg(windows)]
fn front(w: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    unsafe { SetForegroundWindow(w) };
}
/// Linux: X11 window watching (XWayland under Wayland).
#[cfg(unix)]
fn main_window(pid: u32) -> Option<isize> { crate::x11::main_window(pid) }
#[cfg(unix)]
fn front(w: isize) { crate::x11::front(w) }
#[cfg(not(any(windows, unix)))]
fn main_window(_pid: u32) -> Option<isize> { None }
#[cfg(not(any(windows, unix)))]
fn front(_w: isize) {}
