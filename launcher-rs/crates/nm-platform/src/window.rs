//! The launcher's window on SDL3: borderless, the device outline as its region, a streaming texture presented
//! without scaling, and events in the C# launcher's terms (virtual keys, Pad bits, WinForms wheel deltas).

use crate::geometry::Rect;
use crate::input::{self, PadState};
use sdl3_sys::everything::*;
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub use crate::input::{Event, MouseButton};


#[derive(Clone, Debug)]
pub struct Display { pub name: String, pub device: String, pub bounds: Rect, pub usable: Rect, pub primary: bool }

static REQUEST: AtomicBool = AtomicBool::new(false);
static CLOSE: AtomicBool = AtomicBool::new(false);
static POSTED_KEYS: Mutex<Vec<(isize, u32)>> = Mutex::new(Vec::new());
/// A posted WM_CLOSE's window, when it isn't the launcher's own (the guide overlay's: a resume, not a close).
static CLOSED_OTHER: Mutex<Vec<isize>> = Mutex::new(Vec::new());
/// The guide overlay's window, so posted input for it goes to it.
static OVERLAY_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

fn err() -> String { unsafe { CStr::from_ptr(SDL_GetError()).to_string_lossy().into_owned() } }

pub use crate::quiet;

/// SDL_Init for video and gamepads, after the hints. Windows: DPI unaware, as the C# exe (Windows stretches it).
pub fn init() -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_UNAWARE);
    }
    unsafe {
        #[cfg(unix)]
        SDL_SetHint(SDL_HINT_VIDEO_DRIVER, c"x11,wayland".as_ptr());
        SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"0".as_ptr());
        if quiet() { SDL_SetHint(SDL_HINT_WINDOW_ACTIVATE_WHEN_SHOWN, c"0".as_ptr()); }
        if !SDL_Init(SDL_INIT_VIDEO | SDL_INIT_GAMEPAD) { return Err(err()); }
        #[cfg(windows)]
        SDL_SetWindowsMessageHook(Some(hook), std::ptr::null_mut());
    }
    Ok(())
}

#[cfg(windows)]
unsafe extern "C" fn hook(_u: *mut std::ffi::c_void, msg: *mut MSG) -> bool {
    let m = &*(msg as *const windows_sys::Win32::UI::WindowsAndMessaging::MSG);
    if m.message == crate::instance::WM_REQUEST { REQUEST.store(true, Ordering::SeqCst); return false; }
    // A posted WM_KEYDOWN without a scan code (stable-check posts lParam 0, other scripts 1): SDL drops those.
    if m.message == 0x0100 && posted_key(m.lParam as isize) {
        if let Ok(mut q) = POSTED_KEYS.lock() { q.push((m.hwnd, m.wParam as u32)); }
        return false;
    }
    // A posted WM_CLOSE (a script, taskkill without /F): taken here, so it is one close, never SDL's two events.
    if m.message == 0x0010 {
        let o = OVERLAY_HWND.load(Ordering::SeqCst);
        if o != 0 && m.hwnd == o { if let Ok(mut q) = CLOSED_OTHER.lock() { q.push(o); } } else { CLOSE.store(true, Ordering::SeqCst); }
        return false;
    }
    true
}

/// Real key presses carry their scan code in lParam bits 16-23; a posted key has none.
pub fn posted_key(lparam: isize) -> bool { (lparam >> 16) & 0xFF == 0 && (lparam >> 24) & 1 == 0 }

pub fn displays() -> Vec<Display> {
    let mut out = Vec::new();
    unsafe {
        let mut n = 0;
        let ids = SDL_GetDisplays(&mut n);
        if ids.is_null() { return out; }
        #[cfg(windows)]
        let gdi = gdi_devices();
        let primary = SDL_GetPrimaryDisplay();
        for i in 0..n as usize {
            let id = *ids.add(i);
            let (mut b, mut u) = (SDL_Rect { x: 0, y: 0, w: 0, h: 0 }, SDL_Rect { x: 0, y: 0, w: 0, h: 0 });
            SDL_GetDisplayBounds(id, &mut b);
            SDL_GetDisplayUsableBounds(id, &mut u);
            let name = SDL_GetDisplayName(id);
            let bounds = Rect { x: b.x, y: b.y, w: b.w, h: b.h };
            #[cfg(windows)]
            let device = gdi.iter().find(|(r, _)| *r == bounds).map(|(_, d)| d.clone()).unwrap_or_default();
            #[cfg(not(windows))]
            let device = String::new();
            out.push(Display { name: if name.is_null() { String::new() } else { CStr::from_ptr(name).to_string_lossy().into_owned() }, device, bounds, usable: Rect { x: u.x, y: u.y, w: u.w, h: u.h }, primary: id == primary });
        }
        SDL_free(ids as *mut _);
    }
    out
}

/// Screen.AllScreens' device names (\\.\DISPLAY3) by bounds, since SDL names monitors by model.
#[cfg(windows)]
fn gdi_devices() -> Vec<(Rect, String)> {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    unsafe extern "system" fn each(m: HMONITOR, _dc: HDC, _r: *mut RECT, l: LPARAM) -> BOOL {
        let out = &mut *(l as *mut Vec<(Rect, String)>);
        let mut mi: MONITORINFOEXW = std::mem::zeroed();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(m, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO) != 0 {
            let r = mi.monitorInfo.rcMonitor;
            let len = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
            out.push((Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top }, String::from_utf16_lossy(&mi.szDevice[..len])));
        }
        1
    }
    let mut out: Vec<(Rect, String)> = Vec::new();
    unsafe { EnumDisplayMonitors(0, std::ptr::null(), Some(each), &mut out as *mut _ as LPARAM) };
    out
}

/// NATIVEMONS_TEST_SCREEN (DISPLAY3) picks the display whose device name ends with it, else the primary.
pub fn pick_display(all: &[Display], test_screen: Option<&str>) -> Option<usize> {
    if all.is_empty() { return None; }
    if let Some(t) = test_screen.filter(|t| !t.is_empty()) {
        let t = t.to_ascii_lowercase();
        if let Some(i) = all.iter().position(|d| d.device.to_ascii_lowercase().ends_with(&t) || d.name.to_ascii_lowercase().ends_with(&t)) { return Some(i); }
    }
    Some(all.iter().position(|d| d.primary).unwrap_or(0))
}

/// The guide overlay's own window (GuideOverlay): borderless, on top, out of the taskbar, 94% opaque, over the
/// screen the game is on. Its events come out of the one SDL queue by window id.
struct Overlay { win: *mut SDL_Window, ren: *mut SDL_Renderer, tex: *mut SDL_Texture, tex_size: (i32, i32), id: u32, queue: Vec<Event> }
impl Drop for Overlay {
    fn drop(&mut self) { unsafe { if !self.tex.is_null() { SDL_DestroyTexture(self.tex); } SDL_DestroyRenderer(self.ren); SDL_DestroyWindow(self.win); } }
}
fn present_to(ren: *mut SDL_Renderer, tex: &mut *mut SDL_Texture, tex_size: &mut (i32, i32), pixels: &[u32], w: i32, h: i32) {
    unsafe {
        if tex.is_null() || *tex_size != (w, h) {
            if !tex.is_null() { SDL_DestroyTexture(*tex); }
            *tex = SDL_CreateTexture(ren, SDL_PIXELFORMAT_ARGB8888, SDL_TEXTUREACCESS_STREAMING, w, h);
            SDL_SetTextureBlendMode(*tex, SDL_BLENDMODE_NONE);
            *tex_size = (w, h);
        }
        SDL_UpdateTexture(*tex, std::ptr::null(), pixels.as_ptr() as *const _, w * 4);
        SDL_RenderTexture(ren, *tex, std::ptr::null(), std::ptr::null());
        SDL_RenderPresent(ren);
    }
}
/// Screen.FromHandle(game).Bounds: the whole monitor the game's window is on.
#[cfg(windows)]
pub fn screen_of(w: isize) -> Option<Rect> {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    unsafe {
        let m = MonitorFromWindow(w, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(m, &mut mi) == 0 { return None; }
        let r = mi.rcMonitor;
        Some(Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top })
    }
}
#[cfg(not(windows))]
pub fn screen_of(_w: isize) -> Option<Rect> { None }
/// PostMessage(WM_CLOSE) to the game (Close game), and Native.Front (Resume).
#[cfg(windows)]
pub fn close_window(w: isize) { unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(w, 0x10, 0, 0); } }
#[cfg(windows)]
pub fn front_window(w: isize) { if quiet() { return; } unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(w); } }
#[cfg(not(windows))]
pub fn close_window(_w: isize) {}
#[cfg(not(windows))]
pub fn front_window(_w: isize) {}
/// The window an SDL event belongs to (0 when none).
unsafe fn event_window(e: &SDL_Event) -> u32 {
    let t = e.r#type;
    if t == SDL_EVENT_KEY_DOWN.0 || t == SDL_EVENT_KEY_UP.0 { e.key.windowID.0 }
    else if t == SDL_EVENT_MOUSE_MOTION.0 { e.motion.windowID.0 }
    else if t == SDL_EVENT_MOUSE_BUTTON_DOWN.0 || t == SDL_EVENT_MOUSE_BUTTON_UP.0 { e.button.windowID.0 }
    else if t == SDL_EVENT_MOUSE_WHEEL.0 { e.wheel.windowID.0 }
    else if (SDL_EVENT_WINDOW_FIRST.0..=SDL_EVENT_WINDOW_LAST.0).contains(&t) { e.window.windowID.0 }
    else { 0 }
}

pub struct Window {
    overlay: Option<Overlay>,
    win: *mut SDL_Window,
    ren: *mut SDL_Renderer,
    tex: *mut SDL_Texture,
    tex_size: (i32, i32),
    pads: Vec<*mut SDL_Gamepad>,
    arrow: *mut SDL_Cursor,
    hand: *mut SDL_Cursor,
    down: Option<MouseButton>,
    dragged: bool,
    /// Manual drag where the OS can't move it for us: (cursor x, y, window x, y) at the press.
    drag_from: Option<(f32, f32, i32, i32)>,
    #[cfg(unix)]
    mask: Option<(i32, i32, Vec<bool>)>,
    #[cfg(unix)]
    scratch: Vec<u32>,
}

fn button(b: u8) -> Option<MouseButton> {
    match b as i32 { SDL_BUTTON_LEFT => Some(MouseButton::Left), SDL_BUTTON_RIGHT => Some(MouseButton::Right), SDL_BUTTON_MIDDLE => Some(MouseButton::Middle), _ => None }
}

impl Window {
    /// A hidden borderless window titled "nativeMons" (the doorbell finds it by title); show() puts it up.
    pub fn create(size: (i32, i32), pos: (i32, i32)) -> Result<Window, String> {
        unsafe {
            let title = CString::new("nativeMons").unwrap();
            #[allow(unused_mut)]
            let mut flags = SDL_WINDOW_BORDERLESS | SDL_WINDOW_HIDDEN;
            #[cfg(unix)]
            { flags = flags | SDL_WINDOW_TRANSPARENT; }
            let win = SDL_CreateWindow(title.as_ptr(), size.0, size.1, flags);
            if win.is_null() { return Err(err()); }
            SDL_SetWindowPosition(win, pos.0, pos.1);
            let ren = SDL_CreateRenderer(win, std::ptr::null());
            if ren.is_null() { return Err(err()); }
            SDL_SetRenderVSync(ren, 0);
            let w = Window {
                overlay: None,
                win, ren, tex: std::ptr::null_mut(), tex_size: (0, 0), pads: Vec::new(),
                arrow: SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_DEFAULT), hand: SDL_CreateSystemCursor(SDL_SYSTEM_CURSOR_POINTER),
                down: None, dragged: false, drag_from: None,
                #[cfg(unix)]
                mask: None,
                #[cfg(unix)]
                scratch: Vec::new(),
            };
            Ok(w)
        }
    }

    #[cfg(windows)]
    pub fn hwnd(&self) -> isize {
        unsafe { SDL_GetPointerProperty(SDL_GetWindowProperties(self.win), SDL_PROP_WINDOW_WIN32_HWND_POINTER, std::ptr::null_mut()) as isize }
    }

    /// The device's outline: SetWindowRgn with the aliased rounded rectangle's rows (jagged corners, as the C#'s Region;
    /// the caller works out the spans), on X11 the same mask as alpha. `x` shifts it in the window (SetMorphRegion mid-morph).
    pub fn set_outline(&mut self, x: i32, h: i32, spans: &[(i32, i32, i32)]) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::*;
            let _ = h;
            let rgn = CreateRectRgn(0, 0, 0, 0);
            let mut i = 0;
            while i < spans.len() {
                let (y0, x0, x1) = spans[i];
                let mut j = i + 1;
                while j < spans.len() && spans[j].1 == x0 && spans[j].2 == x1 && spans[j].0 == spans[j - 1].0 + 1 { j += 1; }
                let part = CreateRectRgn(x + x0, y0, x + x1, spans[j - 1].0 + 1);
                CombineRgn(rgn, rgn, part, RGN_OR);
                DeleteObject(part);
                i = j;
            }
            // The window owns the region from here on.
            SetWindowRgn(self.hwnd(), rgn, 1);
        }
        #[cfg(unix)]
        {
            let (ww, _) = self.size();
            let mut m = vec![false; (ww.max(0) * h.max(0)) as usize];
            for &(y, x0, x1) in spans { for c in (x + x0).max(0)..(x + x1).min(ww) { m[(y * ww + c) as usize] = true; } }
            self.mask = Some((ww, h, m));
        }
    }

    /// One frame: 0xAARRGGBB pixels (nm-draw's Bitmap), one texel per window pixel.
    pub fn present(&mut self, pixels: &[u32], w: i32, h: i32) {
        unsafe {
            if self.tex.is_null() || self.tex_size != (w, h) {
                if !self.tex.is_null() { SDL_DestroyTexture(self.tex); }
                self.tex = SDL_CreateTexture(self.ren, SDL_PIXELFORMAT_ARGB8888, SDL_TEXTUREACCESS_STREAMING, w, h);
                SDL_SetTextureBlendMode(self.tex, SDL_BLENDMODE_NONE);
                self.tex_size = (w, h);
            }
            #[allow(unused_mut)]
            let mut src = pixels;
            #[cfg(unix)]
            if let Some((mw, mh, m)) = &self.mask {
                if (*mw, *mh) == (w, h) {
                    self.scratch.clear();
                    self.scratch.extend(pixels.iter().zip(m.iter()).map(|(p, &inside)| if inside { *p | 0xFF00_0000 } else { 0 }));
                    src = &self.scratch;
                }
            }
            SDL_UpdateTexture(self.tex, std::ptr::null(), src.as_ptr() as *const _, w * 4);
            SDL_RenderTexture(self.ren, self.tex, std::ptr::null(), std::ptr::null());
            SDL_RenderPresent(self.ren);
        }
    }

    pub fn show(&self) { unsafe { SDL_ShowWindow(self.win); if !quiet() { SDL_RaiseWindow(self.win); } } }
    pub fn hide(&self) { unsafe { SDL_HideWindow(self.win); } }
    pub fn minimize(&self) { unsafe { SDL_MinimizeWindow(self.win); } }
    pub fn raise(&self) { unsafe { SDL_RaiseWindow(self.win); } }
    pub fn visible(&self) -> bool { unsafe { (SDL_GetWindowFlags(self.win).0 & (SDL_WINDOW_HIDDEN.0 | SDL_WINDOW_MINIMIZED.0)) == 0 } }
    pub fn focused(&self) -> bool { unsafe { SDL_GetWindowFlags(self.win).0 & SDL_WINDOW_INPUT_FOCUS.0 != 0 } }
    pub fn position(&self) -> (i32, i32) { let (mut x, mut y) = (0, 0); unsafe { SDL_GetWindowPosition(self.win, &mut x, &mut y) }; (x, y) }
    pub fn set_position(&self, x: i32, y: i32) { unsafe { SDL_SetWindowPosition(self.win, x, y); } }
    pub fn size(&self) -> (i32, i32) { let (mut w, mut h) = (0, 0); unsafe { SDL_GetWindowSize(self.win, &mut w, &mut h) }; (w, h) }
    pub fn set_bounds(&mut self, x: i32, y: i32, w: i32, h: i32) { unsafe { SDL_SetWindowSize(self.win, w, h); SDL_SetWindowPosition(self.win, x, y); } }
    pub fn set_hand(&self, hand: bool) { unsafe { SDL_SetCursor(if hand { self.hand } else { self.arrow }); } }
    /// Any controller's guide button (Xbox's, or the PlayStation button) is down.
    pub fn guide_down(&self) -> bool { unsafe { self.pads.iter().any(|&p| SDL_GetGamepadButton(p, SDL_GAMEPAD_BUTTON_GUIDE)) } }
    /// OpenGuide: the overlay over the game's screen; its size, or None when it can't open.
    pub fn guide_open(&mut self, game: isize) -> Option<(i32, i32)> {
        let r = screen_of(game)?;
        unsafe {
            let title = CString::new("nativeMons").unwrap();
            let win = SDL_CreateWindow(title.as_ptr(), r.w, r.h, SDL_WINDOW_BORDERLESS | SDL_WINDOW_ALWAYS_ON_TOP | SDL_WINDOW_UTILITY | SDL_WINDOW_HIDDEN);
            if win.is_null() { return None; }
            SDL_SetWindowPosition(win, r.x, r.y);
            let ren = SDL_CreateRenderer(win, std::ptr::null());
            if ren.is_null() { SDL_DestroyWindow(win); return None; }
            SDL_SetWindowOpacity(win, 0.94);
            let id = SDL_GetWindowID(win).0;
            SDL_ShowWindow(win);
            SDL_RaiseWindow(win);
            #[cfg(windows)]
            OVERLAY_HWND.store(SDL_GetPointerProperty(SDL_GetWindowProperties(win), SDL_PROP_WINDOW_WIN32_HWND_POINTER, std::ptr::null_mut()) as isize, Ordering::SeqCst);
            self.overlay = Some(Overlay { win, ren, tex: std::ptr::null_mut(), tex_size: (0, 0), id, queue: Vec::new() });
        }
        Some((r.w, r.h))
    }
    pub fn guide_events(&mut self) -> Vec<Event> { self.overlay.as_mut().map(|o| std::mem::take(&mut o.queue)).unwrap_or_default() }
    pub fn guide_present(&mut self, pixels: &[u32], w: i32, h: i32) { if let Some(o) = self.overlay.as_mut() { present_to(o.ren, &mut o.tex, &mut o.tex_size, pixels, w, h); } }
    /// The overlay closes; the game is told to close, or brought back to the front.
    pub fn guide_close(&mut self, game: isize, close_game: bool) {
        self.overlay = None;
        OVERLAY_HWND.store(0, Ordering::SeqCst);
        if close_game { close_window(game); } else { front_window(game); }
    }
    /// The system's folder or file picker over this window; the answer arrives in dialogs::take_answer.
    pub fn pick(&self, folder: bool, title: &str, filters: &[(String, String)]) { crate::dialogs::show(self.win, folder, title, filters); }
    /// AskText over this window (modal, as ShowDialog).
    pub fn ask_text(&self, title: &str, prompt: &str, start: &str) -> Option<String> {
        #[cfg(windows)]
        { crate::dialogs::ask_text(self.hwnd(), title, prompt, start) }
        #[cfg(not(windows))]
        { crate::dialogs::ask_text(0, title, prompt, start) }
    }
    pub fn set_clipboard(text: &str) { if let Ok(c) = CString::new(text) { unsafe { SDL_SetClipboardText(c.as_ptr()); } } }
    pub fn open_url(url: &str) { if let Ok(c) = CString::new(url) { unsafe { SDL_OpenURL(c.as_ptr()); } } }

    /// Dragging the shell moves the device: Windows' own move loop (ReleaseCapture + WM_NCLBUTTONDOWN at the
    /// caption, as the C#), else followed by hand from the next mouse moves.
    pub fn begin_drag(&mut self) {
        self.dragged = true;
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
            use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
            SDL_CaptureMouse(false);
            ReleaseCapture();
            SendMessageW(self.hwnd(), 0xA1, 2, 0);
            self.down = None;
        }
        #[cfg(not(windows))]
        {
            let (mut gx, mut gy) = (0f32, 0f32);
            unsafe { SDL_GetGlobalMouseState(&mut gx, &mut gy) };
            let (wx, wy) = self.position();
            self.drag_from = Some((gx, gy, wx, wy));
        }
    }

    /// The first open controller's buttons as Pad() reports them. Only controllers SDL announced are read.
    pub fn pad_count(&self) -> usize { self.pads.len() }
    pub fn pad(&self) -> u16 {
        let Some(&p) = self.pads.first() else { return 0 };
        unsafe {
            let b = |x: SDL_GamepadButton| SDL_GetGamepadButton(p, x);
            input::pad_bits(&PadState {
                up: b(SDL_GAMEPAD_BUTTON_DPAD_UP), down: b(SDL_GAMEPAD_BUTTON_DPAD_DOWN), left: b(SDL_GAMEPAD_BUTTON_DPAD_LEFT), right: b(SDL_GAMEPAD_BUTTON_DPAD_RIGHT),
                south: b(SDL_GAMEPAD_BUTTON_SOUTH), east: b(SDL_GAMEPAD_BUTTON_EAST), west: b(SDL_GAMEPAD_BUTTON_WEST), north: b(SDL_GAMEPAD_BUTTON_NORTH),
                lb: b(SDL_GAMEPAD_BUTTON_LEFT_SHOULDER), rb: b(SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER), start: b(SDL_GAMEPAD_BUTTON_START), back: b(SDL_GAMEPAD_BUTTON_BACK),
                lx: SDL_GetGamepadAxis(p, SDL_GAMEPAD_AXIS_LEFTX), ly: SDL_GetGamepadAxis(p, SDL_GAMEPAD_AXIS_LEFTY),
            })
        }
    }

    /// Everything since the last call, without waiting.
    pub fn events(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        if REQUEST.swap(false, Ordering::SeqCst) { out.push(Event::Request); }
        let over = OVERLAY_HWND.load(Ordering::SeqCst);
        if let Ok(mut q) = POSTED_KEYS.lock() {
            for (h, vk) in q.drain(..) {
                match self.overlay.as_mut() { Some(o) if over != 0 && h == over => o.queue.push(Event::Key(vk)), _ => out.push(Event::Key(vk)) }
            }
        }
        if let (Ok(mut q), Some(o)) = (CLOSED_OTHER.lock(), self.overlay.as_mut()) { if !q.is_empty() { q.clear(); o.queue.push(Event::Quit); } }
        if CLOSE.swap(false, Ordering::SeqCst) { out.push(Event::Quit); }
        unsafe {
            let mut e: SDL_Event = std::mem::zeroed();
            while SDL_PollEvent(&mut e) {
                let t = e.r#type;
                // The overlay's own events go to its queue.
                if let Some(o) = self.overlay.as_mut() {
                    if event_window(&e) == o.id {
                        if t == SDL_EVENT_WINDOW_CLOSE_REQUESTED.0 { o.queue.push(Event::Quit); }
                        else if t == SDL_EVENT_KEY_DOWN.0 { let vk = input::scancode_to_vk(e.key.scancode.0 as u32); if vk != 0 { o.queue.push(Event::Key(vk)); } }
                        else if t == SDL_EVENT_MOUSE_MOTION.0 { o.queue.push(Event::MouseMove { x: e.motion.x as i32, y: e.motion.y as i32, gx: 0, gy: 0 }); }
                        else if t == SDL_EVENT_MOUSE_BUTTON_UP.0 { if let Some(b) = button(e.button.button) { o.queue.push(Event::Click { x: e.button.x as i32, y: e.button.y as i32, b }); } }
                        continue;
                    }
                }
                if t == SDL_EVENT_QUIT.0 || t == SDL_EVENT_WINDOW_CLOSE_REQUESTED.0 { out.push(Event::Quit); }
                else if t == SDL_EVENT_KEY_DOWN.0 {
                    let vk = input::scancode_to_vk(e.key.scancode.0 as u32);
                    if vk != 0 { out.push(Event::Key(vk)); }
                } else if t == SDL_EVENT_MOUSE_MOTION.0 {
                    let (mut gx, mut gy) = (0f32, 0f32);
                    SDL_GetGlobalMouseState(&mut gx, &mut gy);
                    if let Some((fx, fy, wx, wy)) = self.drag_from { self.set_position(wx + (gx - fx) as i32, wy + (gy - fy) as i32); continue; }
                    out.push(Event::MouseMove { x: e.motion.x as i32, y: e.motion.y as i32, gx: gx as i32, gy: gy as i32 });
                } else if t == SDL_EVENT_MOUSE_BUTTON_DOWN.0 {
                    if let Some(b) = button(e.button.button) {
                        self.down = Some(b);
                        self.dragged = false;
                        out.push(Event::MouseDown { x: e.button.x as i32, y: e.button.y as i32, b });
                    }
                } else if t == SDL_EVENT_MOUSE_BUTTON_UP.0 {
                    self.drag_from = None;
                    if let Some(b) = button(e.button.button) {
                        let (x, y) = (e.button.x as i32, e.button.y as i32);
                        out.push(Event::MouseUp { x, y, b });
                        if self.down == Some(b) && !self.dragged { out.push(Event::Click { x, y, b }); }
                        self.down = None;
                    }
                } else if t == SDL_EVENT_MOUSE_WHEEL.0 {
                    let d = input::wheel_delta(e.wheel.y);
                    if d != 0 { out.push(Event::Wheel(d)); }
                } else if t == SDL_EVENT_WINDOW_FOCUS_GAINED.0 { out.push(Event::Focus(true)); }
                else if t == SDL_EVENT_WINDOW_FOCUS_LOST.0 { out.push(Event::Focus(false)); }
                else if t == SDL_EVENT_WINDOW_EXPOSED.0 { out.push(Event::Exposed); }
                else if t == SDL_EVENT_DROP_FILE.0 && !e.drop.data.is_null() {
                    let p = CStr::from_ptr(e.drop.data).to_string_lossy().into_owned();
                    match out.last_mut() { Some(Event::Drop(v)) => v.push(p), _ => out.push(Event::Drop(vec![p])) }
                }
                else if t == SDL_EVENT_GAMEPAD_ADDED.0 {
                    let p = SDL_OpenGamepad(e.gdevice.which);
                    if !p.is_null() { self.pads.push(p); }
                } else if t == SDL_EVENT_GAMEPAD_REMOVED.0 {
                    let p = SDL_GetGamepadFromID(e.gdevice.which);
                    if !p.is_null() { self.pads.retain(|&x| x != p); SDL_CloseGamepad(p); }
                }
            }
        }
        out
    }

    /// SDL_DelayPrecise: the tick loop's wait.
    pub fn delay(seconds: f32) { if seconds > 0.0 { unsafe { SDL_DelayPrecise((seconds as f64 * 1e9) as u64) }; } }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            for &p in &self.pads { SDL_CloseGamepad(p); }
            if !self.tex.is_null() { SDL_DestroyTexture(self.tex); }
            SDL_DestroyRenderer(self.ren);
            SDL_DestroyWindow(self.win);
            SDL_DestroyCursor(self.arrow);
            SDL_DestroyCursor(self.hand);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(device: &str, name: &str) -> Display { let r = Rect { x: 0, y: 0, w: 10, h: 10 }; Display { name: name.into(), device: device.into(), bounds: r, usable: r, primary: name == "Primary" } }
    #[test]
    fn posted_keys() {
        assert!(posted_key(0) && posted_key(1));
        assert!(!posted_key(0x001C_0001) && !posted_key(0x0100_0001));
    }
    #[test]
    fn display_pick() {
        let all = [d("\\\\.\\DISPLAY1", "Primary"), d("\\\\.\\DISPLAY2", "X"), d("\\\\.\\DISPLAY3", "GN246HL")];
        assert_eq!(pick_display(&all, Some("DISPLAY3")), Some(2));
        assert_eq!(pick_display(&all, Some("display2")), Some(1));
        assert_eq!(pick_display(&all, Some("DISPLAY9")), Some(0));
        assert_eq!(pick_display(&all, None), Some(0));
        let mut two = all.clone(); two[0].primary = false; two[1].primary = true;
        assert_eq!(pick_display(&two, None), Some(1));
        assert_eq!(pick_display(&[], None), None);
    }
}
