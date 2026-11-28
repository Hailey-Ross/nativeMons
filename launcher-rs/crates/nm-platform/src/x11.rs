//! Linux window watching over X11 (XWayland under Wayland, gamescope on a Steam Deck): a process's main window by
//! _NET_WM_PID, its title, bringing it forward, posting a key, and PlaySnaps' picture of it. Built on unix; also
//! compiled on Windows for the tests, so the code is type-checked there too.

use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

static CONN: Mutex<Option<(RustConnection, Window)>> = Mutex::new(None);

/// Runs `f` with the display connection, made once (None when there's no X display).
fn with<T>(f: impl FnOnce(&RustConnection, Window) -> Option<T>) -> Option<T> {
    let mut c = CONN.lock().unwrap_or_else(|e| e.into_inner());
    if c.is_none() {
        let (conn, n) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(n)?.root;
        *c = Some((conn, root));
    }
    let (conn, root) = c.as_ref()?;
    let r = f(conn, *root);
    if r.is_none() && conn.flush().is_err() { *c = None; }
    r
}
fn atom(c: &RustConnection, name: &str) -> Option<Atom> { c.intern_atom(false, name.as_bytes()).ok()?.reply().ok().map(|r| r.atom) }
fn prop32(c: &RustConnection, w: Window, p: Atom) -> Vec<u32> {
    c.get_property(false, w, p, AtomEnum::ANY, 0, 1 << 16).ok().and_then(|r| r.reply().ok()).and_then(|r| r.value32().map(|v| v.collect())).unwrap_or_default()
}

/// The first viewable, ownerless client window of `pid` (as Process.MainWindowHandle).
pub fn main_window(pid: u32) -> Option<isize> {
    with(|c, root| {
        let (list, wm_pid) = (atom(c, "_NET_CLIENT_LIST")?, atom(c, "_NET_WM_PID")?);
        for w in prop32(c, root, list) {
            if prop32(c, w, wm_pid).first() != Some(&pid) { continue; }
            let viewable = c.get_window_attributes(w).ok()?.reply().ok().map_or(false, |a| a.map_state == MapState::VIEWABLE);
            if viewable && prop32(c, w, AtomEnum::WM_TRANSIENT_FOR.into()).is_empty() { return Some(w as isize); }
        }
        None
    })
}

/// _NET_WM_NAME (UTF-8), else WM_NAME.
pub fn window_title(w: isize) -> String {
    with(|c, _| {
        let (name, utf8) = (atom(c, "_NET_WM_NAME")?, atom(c, "UTF8_STRING")?);
        let get = |p: Atom, t: Atom| c.get_property(false, w as Window, p, t, 0, 1024).ok().and_then(|r| r.reply().ok()).map(|r| r.value).filter(|v| !v.is_empty());
        let v = get(name, utf8).or_else(|| get(AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))?;
        Some(String::from_utf8_lossy(&v).into_owned())
    }).unwrap_or_default()
}

/// Brings the window forward through the window manager (_NET_ACTIVE_WINDOW, from a pager: source 2).
pub fn front(w: isize) {
    with(|c, root| {
        let active = atom(c, "_NET_ACTIVE_WINDOW")?;
        let ev = ClientMessageEvent::new(32, w as Window, active, [2u32, 0, 0, 0, 0]);
        c.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, ev).ok()?;
        c.flush().ok()
    });
}

/// The X keysym for the Windows virtual keys the launcher posts.
fn keysym(vk: u32) -> Option<u32> {
    Some(match vk { 0x0D => 0xff0d, 0x1B => 0xff1b, 0x20 => 0x20, 0x25 => 0xff51, 0x26 => 0xff52, 0x27 => 0xff53, 0x28 => 0xff54, 0x41..=0x5A => vk + 0x20, 0x30..=0x39 => vk, _ => return None })
}
/// A key down and up sent to the window (WatchGba's Enter for a hack's list).
pub fn post_key(w: isize, vk: u32) {
    with(|c, root| {
        let sym = keysym(vk)?;
        let (min, max) = (c.setup().min_keycode, c.setup().max_keycode);
        let map = c.get_keyboard_mapping(min, max - min + 1).ok()?.reply().ok()?;
        let per = map.keysyms_per_keycode.max(1) as usize;
        let code = map.keysyms.iter().position(|&s| s == sym).map(|i| min + (i / per) as u8)?;
        for (kind, mask) in [(KEY_PRESS_EVENT, EventMask::KEY_PRESS), (KEY_RELEASE_EVENT, EventMask::KEY_RELEASE)] {
            let ev = KeyPressEvent { response_type: kind, detail: code, sequence: 0, time: x11rb::CURRENT_TIME, root, event: w as Window, child: x11rb::NONE,
                root_x: 0, root_y: 0, event_x: 0, event_y: 0, state: KeyButMask::default(), same_screen: true };
            c.send_event(true, w as Window, mask, ev).ok()?;
        }
        c.flush().ok()
    });
}

/// The window's picture as ARGB rows, when it is the active one, not hidden and at least 400x300.
pub fn grab(pid: u32) -> Option<(i32, i32, Vec<u32>)> {
    let w = main_window(pid)? as Window;
    with(|c, root| {
        if prop32(c, root, atom(c, "_NET_ACTIVE_WINDOW")?).first() != Some(&w) { return None; }
        let hidden = atom(c, "_NET_WM_STATE_HIDDEN")?;
        if prop32(c, w, atom(c, "_NET_WM_STATE")?).contains(&hidden) { return None; }
        let g = c.get_geometry(w).ok()?.reply().ok()?;
        let (width, height) = (g.width as i32, g.height as i32);
        if width < 400 || height < 300 { return None; }
        let img = c.get_image(ImageFormat::Z_PIXMAP, w, 0, 0, g.width, g.height, !0).ok()?.reply().ok()?;
        // 24- and 32-bit visuals come as 4 bytes a pixel, blue first.
        if img.depth < 24 || img.data.len() < (width * height * 4) as usize { return None; }
        Some((width, height, to_argb(&img.data, (width * height) as usize)))
    })
}
fn to_argb(data: &[u8], n: usize) -> Vec<u32> { data.chunks_exact(4).take(n).map(|p| u32::from_le_bytes([p[0], p[1], p[2], 0xFF])).collect() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_and_pixels() {
        assert_eq!(keysym(0x0D), Some(0xff0d));
        assert_eq!(keysym(0x41), Some(0x61));
        assert_eq!(keysym(0x70), None);
        assert_eq!(to_argb(&[1, 2, 3, 0, 9, 9, 9, 9], 1), vec![0xFF03_0201]);
    }
}
