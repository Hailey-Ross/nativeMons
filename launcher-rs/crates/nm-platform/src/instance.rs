//! One launcher per folder (the C# Main's named mutex, flock on unix) and requests passed to the one already open:
//! state\request.txt ("play"|"show", game id, slot) plus a doorbell (WM_REQUEST on Windows, a Unix socket elsewhere).

use std::path::{Path, PathBuf};

pub const WM_REQUEST: u32 = 0x8000 + 0x4E4D;

fn normal(p: &Path) -> String {
    let full = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    full.to_string_lossy().trim_end_matches('\\').to_lowercase().replace('\\', "/")
}

/// "Local\NativeMonsLauncher-<folder>[|<state>]": the same name the C# uses, so the two exclude each other.
pub fn mutex_name(here: &Path, test_state: Option<&Path>) -> String {
    let mut folder = normal(here);
    if let Some(s) = test_state { folder.push('|'); folder.push_str(&normal(s)); }
    format!("Local\\NativeMonsLauncher-{}", folder)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request { pub what: String, pub id: String, pub slot: Option<String> }

pub fn request_text(menu: bool, game: &str, slot: Option<&str>) -> String { format!("{}\n{}\n{}", if menu { "show" } else { "play" }, game, slot.unwrap_or("")) }

/// WndProc + RequestTick: two lines at least, each trimmed; an empty slot is none.
pub fn parse_request(text: &str) -> Option<Request> {
    let l: Vec<&str> = text.split('\n').collect();
    if l.len() < 2 { return None; }
    let slot = l.get(2).map(|s| s.trim()).filter(|s| !s.is_empty()).map(String::from);
    Some(Request { what: l[0].trim().to_string(), id: l[1].trim().to_string(), slot })
}

/// Reads and deletes state\request.txt, as WndProc does when the doorbell rings.
pub fn take_request(state: &Path) -> Option<Request> {
    let f = state.join("request.txt");
    let t = std::fs::read_to_string(&f).ok()?;
    let _ = std::fs::remove_file(&f);
    parse_request(&t)
}

pub fn write_request(state: &Path, menu: bool, game: &str, slot: Option<&str>) -> std::io::Result<()> {
    std::fs::write(state.join("request.txt"), request_text(menu, game, slot))
}

/// The Linux doorbell's socket: $XDG_RUNTIME_DIR/nativemons-<hash of the state path>.sock.
pub fn socket_path(runtime_dir: &Path, state: &Path) -> PathBuf {
    // FNV-1a, stable across runs and builds.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in normal(state).bytes() { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    runtime_dir.join(format!("nativemons-{:016x}.sock", h))
}

/// Held for the launcher's life; dropping it lets the next one start.
pub struct Instance {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(unix)]
    _file: std::fs::File,
    #[cfg(unix)]
    pub listener: Option<std::os::unix::net::UnixListener>,
}

#[cfg(windows)]
impl Drop for Instance {
    fn drop(&mut self) { unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle); } }
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

/// Some(instance) when this is the first launcher of its folder, None when another is open.
#[cfg(windows)]
pub fn acquire(name: &str, _state: &Path) -> Option<Instance> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let n = wide(name);
    let h = unsafe { CreateMutexW(std::ptr::null(), 1, n.as_ptr()) };
    if h == 0 { return None; }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(h); }
        return None;
    }
    Some(Instance { handle: h })
}

#[cfg(unix)]
pub fn acquire(_name: &str, state: &Path) -> Option<Instance> {
    use std::os::unix::io::AsRawFd;
    let _ = std::fs::create_dir_all(state);
    let f = std::fs::OpenOptions::new().create(true).write(true).open(state.join("launcher.lock")).ok()?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 { return None; }
    let listener = std::env::var_os("XDG_RUNTIME_DIR").and_then(|d| {
        let p = socket_path(Path::new(&d), state);
        let _ = std::fs::remove_file(&p);
        let l = std::os::unix::net::UnixListener::bind(&p).ok()?;
        l.set_nonblocking(true).ok()?;
        Some(l)
    });
    Some(Instance { _file: f, listener })
}

impl Instance {
    /// Whether the doorbell rang since the last call (Linux; on Windows WM_REQUEST arrives as a window message).
    pub fn rang(&self) -> bool {
        #[cfg(unix)]
        if let Some(l) = &self.listener {
            let mut any = false;
            while let Ok((_s, _)) = l.accept() { any = true; }
            return any;
        }
        false
    }
}

/// Rings every other launcher titled "nativeMons" (each reads its own state folder's request.txt) and, outside
/// tests, brings the first visible one to the front. Returns how many were told.
#[cfg(windows)]
pub fn ring_others(post: bool, front: bool, _state: &Path) -> usize {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    struct Ctx { me: u32, found: Vec<HWND> }
    unsafe extern "system" fn each(h: HWND, l: LPARAM) -> BOOL {
        let c = &mut *(l as *mut Ctx);
        let mut buf = [0u16; 32];
        let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, &mut pid);
        if pid != c.me && String::from_utf16_lossy(&buf[..n.max(0) as usize]) == "nativeMons" { c.found.push(h); }
        1
    }
    let mut c = Ctx { me: std::process::id(), found: Vec::new() };
    unsafe { EnumWindows(Some(each), &mut c as *mut Ctx as LPARAM) };
    let mut fronted = false;
    for &h in &c.found {
        if post { unsafe { PostMessageW(h, WM_REQUEST, 0, 0) }; }
        if front && !fronted && unsafe { IsWindowVisible(h) } != 0 { unsafe { SetForegroundWindow(h) }; fronted = true; }
    }
    c.found.len()
}

#[cfg(unix)]
pub fn ring_others(post: bool, _front: bool, state: &Path) -> usize {
    let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") else { return 0 };
    if !post { return 0; }
    match std::os::unix::net::UnixStream::connect(socket_path(Path::new(&d), state)) { Ok(_) => 1, Err(_) => 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        #[cfg(windows)]
        {
            assert_eq!(mutex_name(Path::new("C:\\Games\\nativeMons App\\"), None), "Local\\NativeMonsLauncher-c:/games/nativemons app");
            assert_eq!(mutex_name(Path::new("C:\\A"), Some(Path::new("C:\\A\\State"))), "Local\\NativeMonsLauncher-c:/a|c:/a/state");
        }
        let a = socket_path(Path::new("/run/user/1000"), Path::new("/home/deck/nm/state"));
        assert_eq!(a, socket_path(Path::new("/run/user/1000"), Path::new("/home/deck/nm/state")));
        assert!(a.to_string_lossy().ends_with(".sock"));
    }
    #[test]
    fn requests() {
        assert_eq!(request_text(false, "red", Some("2")), "play\nred\n2");
        assert_eq!(parse_request("play\nred\n2"), Some(Request { what: "play".into(), id: "red".into(), slot: Some("2".into()) }));
        assert_eq!(parse_request("show\r\ncrystal\r\n"), Some(Request { what: "show".into(), id: "crystal".into(), slot: None }));
        assert_eq!(parse_request("play"), None);
        let dir = std::env::temp_dir().join(format!("nm-platform-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_request(&dir, true, "x", None).unwrap();
        assert_eq!(take_request(&dir).map(|r| r.what), Some("show".into()));
        assert!(!dir.join("request.txt").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn one_instance() {
        let dir = std::env::temp_dir().join(format!("nm-platform-lock-{}", std::process::id()));
        let name = format!("Local\\NativeMonsLauncher-test-{}", std::process::id());
        let first = acquire(&name, &dir);
        assert!(first.is_some());
        #[cfg(windows)]
        assert!(acquire(&name, &dir).is_none());
        drop(first);
        #[cfg(windows)]
        assert!(acquire(&name, &dir).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
