//! PlaySnaps' platform half: the game window's picture (CopyFromScreen of its client area while it is in front) and
//! waiting on the emulator's end. Linux takes the picture over X11 (x11.rs).

/// The client area of `pid`'s main window as ARGB rows, when it is in front, not minimized and at least 400x300.
#[cfg(windows)]
pub fn grab_client(pid: u32) -> Option<(i32, i32, Vec<u32>)> {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, GetForegroundWindow, IsIconic};
    // Physical pixels, as the screen has them (DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).
    unsafe { SetThreadDpiAwarenessContext(-4isize as _) };
    let h = super::process::main_window_of(pid)?;
    unsafe {
        if IsIconic(h) != 0 || GetForegroundWindow() != h { return None; }
        let mut rc: RECT = std::mem::zeroed();
        if GetClientRect(h, &mut rc) == 0 { return None; }
        let mut at = POINT { x: 0, y: 0 };
        ClientToScreen(h, &mut at);
        let (w, ht) = (rc.right - rc.left, rc.bottom - rc.top);
        if w < 400 || ht < 300 { return None; }
        let screen = GetDC(0);
        if screen == 0 { return None; }
        let mem = CreateCompatibleDC(screen);
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -ht;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB as u32;
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(mem, &bi, DIB_RGB_COLORS, &mut bits, 0, 0);
        let mut out = None;
        if dib != 0 && !bits.is_null() {
            let old = SelectObject(mem, dib);
            if BitBlt(mem, 0, 0, w, ht, screen, at.x, at.y, SRCCOPY) != 0 {
                let px = std::slice::from_raw_parts(bits as *const u32, (w * ht) as usize);
                out = Some((w, ht, px.iter().map(|v| v | 0xFF00_0000).collect()));
            }
            SelectObject(mem, old);
            DeleteObject(dib);
        }
        DeleteDC(mem);
        ReleaseDC(0, screen);
        out
    }
}
#[cfg(unix)]
pub fn grab_client(pid: u32) -> Option<(i32, i32, Vec<u32>)> { crate::x11::grab(pid) }
#[cfg(not(any(windows, unix)))]
pub fn grab_client(_pid: u32) -> Option<(i32, i32, Vec<u32>)> { None }

/// Waits on `pid`'s end: the returned function waits up to that many ms and says whether it has ended. None when the
/// process can't be opened (already gone).
#[cfg(windows)]
pub fn exit_waiter(pid: u32) -> Option<Box<dyn FnMut(u64) -> bool + Send>> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
    struct Handle(isize);
    impl Drop for Handle { fn drop(&mut self) { unsafe { CloseHandle(self.0) }; } }
    let h = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if h == 0 { return None; }
    let h = Handle(h);
    Some(Box::new(move |ms: u64| unsafe { WaitForSingleObject(h.0, ms.min(u32::MAX as u64 - 1) as u32) } != WAIT_TIMEOUT))
}
#[cfg(unix)]
pub fn exit_waiter(pid: u32) -> Option<Box<dyn FnMut(u64) -> bool + Send>> {
    let gone = move || unsafe { libc::kill(pid as libc::pid_t, 0) } != 0;
    if gone() { return None; }
    Some(Box::new(move |ms: u64| {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < end { if gone() { return true; } std::thread::sleep(std::time::Duration::from_millis(500)); }
        gone()
    }))
}

/// The calling thread at BelowNormal priority (the C# watcher thread's).
#[cfg(windows)]
pub fn lower_this_thread() {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) };
}
#[cfg(not(windows))]
pub fn lower_this_thread() {}
