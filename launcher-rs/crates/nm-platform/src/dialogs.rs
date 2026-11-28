//! The C#'s pickers and its save-name box: SDL3's system file and folder dialogs (the xdg portal on Linux), answered
//! later through `take_answer`; and AskText as a native modal prompt laid out as the WinForms one (Windows), or
//! zenity's entry box on Linux.

#[cfg(feature = "sdl")]
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::Mutex;

/// Some(answer) once the open dialog closed: the path, or None when canceled.
static ANSWER: Mutex<Option<Option<String>>> = Mutex::new(None);
/// The filters' strings, kept alive until the dialog answers (SDL reads them meanwhile).
#[cfg(feature = "sdl")]
static KEPT: Mutex<Vec<CString>> = Mutex::new(Vec::new());

pub fn take_answer() -> Option<Option<String>> { ANSWER.lock().unwrap().take() }

/// SDL's pattern list: extensions only ("sav;dat", "*"); a name pattern such as OpenHome*.exe keeps its extension.
pub fn sdl_pattern(p: &str) -> String {
    if p == "*" { return p.into(); }
    p.split(';').map(|x| x.rsplit('.').next().unwrap_or(x).to_string()).collect::<Vec<_>>().join(";")
}

#[cfg(feature = "sdl")]
mod sdl {
    use super::*;
    use sdl3_sys::everything::*;
    unsafe extern "C" fn answered(_u: *mut c_void, list: *const *const c_char, _filter: c_int) {
        let a = if list.is_null() || (*list).is_null() { None } else { Some(CStr::from_ptr(*list).to_string_lossy().into_owned()) };
        *ANSWER.lock().unwrap() = Some(a);
        KEPT.lock().unwrap().clear();
    }
    /// FolderBrowserDialog { Description } / OpenFileDialog { Title, Filter }, over `parent`.
    pub fn show(parent: *mut SDL_Window, folder: bool, title: &str, filters: &[(String, String)]) {
        unsafe {
            let props = SDL_CreateProperties();
            let t = CString::new(title.replace('\0', " ")).unwrap();
            SDL_SetStringProperty(props, SDL_PROP_FILE_DIALOG_TITLE_STRING, t.as_ptr());
            SDL_SetPointerProperty(props, SDL_PROP_FILE_DIALOG_WINDOW_POINTER, parent as *mut c_void);
            let mut list: Vec<SDL_DialogFileFilter> = Vec::new();
            let mut kept = KEPT.lock().unwrap();
            kept.clear();
            for (name, pat) in filters {
                let (n, p) = (CString::new(name.as_str()).unwrap_or_default(), CString::new(sdl_pattern(pat)).unwrap_or_default());
                list.push(SDL_DialogFileFilter { name: n.as_ptr(), pattern: p.as_ptr() });
                kept.push(n);
                kept.push(p);
            }
            // The filter array itself must outlive the call too: leaked into the kept strings' lifetime.
            let arr = Box::leak(list.into_boxed_slice());
            if !arr.is_empty() {
                SDL_SetPointerProperty(props, SDL_PROP_FILE_DIALOG_FILTERS_POINTER, arr.as_mut_ptr() as *mut c_void);
                SDL_SetNumberProperty(props, SDL_PROP_FILE_DIALOG_NFILTERS_NUMBER, arr.len() as i64);
            }
            drop(kept);
            SDL_ShowFileDialogWithProperties(if folder { SDL_FILEDIALOG_OPENFOLDER } else { SDL_FILEDIALOG_OPENFILE }, Some(answered), std::ptr::null_mut(), props);
            SDL_DestroyProperties(props);
        }
    }
}
#[cfg(feature = "sdl")]
pub use sdl::show;

/// AskText: a modal box titled `title` with `prompt`, a one-line box holding `start` (at most 40 characters), OK and
/// Cancel; Enter is OK, Escape is Cancel. None when canceled.
#[cfg(windows)]
pub fn ask_text(parent: isize, title: &str, prompt: &str, start: &str) -> Option<String> {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    fn w(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
    static RESULT: Mutex<Option<i32>> = Mutex::new(None);
    unsafe extern "system" fn proc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match m {
            WM_COMMAND if (wp & 0xFFFF) == 1 || (wp & 0xFFFF) == 2 => { *RESULT.lock().unwrap() = Some((wp & 0xFFFF) as i32); 0 }
            WM_CLOSE => { *RESULT.lock().unwrap() = Some(2); 0 }
            _ => DefWindowProcW(h, m, wp, lp),
        }
    }
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = w("NativeMonsAskText");
        let wc = WNDCLASSW { style: 0, lpfnWndProc: Some(proc), cbClsExtra: 0, cbWndExtra: 0, hInstance: inst, hIcon: 0, hCursor: LoadCursorW(0, IDC_ARROW), hbrBackground: (COLOR_BTNFACE + 1) as isize, lpszMenuName: std::ptr::null(), lpszClassName: class.as_ptr() };
        RegisterClassW(&wc);
        // FormBorderStyle.FixedDialog with a 340 x 110 client area, centered on the launcher.
        let style = WS_CAPTION | WS_SYSMENU | WS_DLGFRAME | WS_POPUP;
        let ex = WS_EX_DLGMODALFRAME;
        let mut r = RECT { left: 0, top: 0, right: 340, bottom: 110 };
        AdjustWindowRectEx(&mut r, style, 0, ex);
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let mut pr = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(parent, &mut pr);
        let (x, y) = (pr.left + (pr.right - pr.left - ww) / 2, pr.top + (pr.bottom - pr.top - wh) / 2);
        let t = w(title);
        let dlg = CreateWindowExW(ex, class.as_ptr(), t.as_ptr(), style, x, y, ww, wh, parent, 0, inst, std::ptr::null());
        if dlg == 0 { return None; }
        // WinForms' default font: Microsoft Sans Serif 8.25 pt.
        let face = w("Microsoft Sans Serif");
        let font = CreateFontW(-11, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 0, 0, face.as_ptr());
        let make = |cls: &str, text: &str, st: u32, exs: u32, id: isize, (cx, cy, cw, chh): (i32, i32, i32, i32)| {
            let (c, tx) = (w(cls), w(text));
            let hw = CreateWindowExW(exs, c.as_ptr(), tx.as_ptr(), WS_CHILD | WS_VISIBLE | st, cx, cy, cw, chh, dlg, id, inst, std::ptr::null());
            SendMessageW(hw, WM_SETFONT, font as usize, 1);
            hw
        };
        make("STATIC", prompt, 0, 0, 0, (12, 12, 316, 23));
        let edit = make("EDIT", start, (ES_AUTOHSCROLL as u32) | WS_TABSTOP, WS_EX_CLIENTEDGE, 3, (12, 36, 316, 20));
        SendMessageW(edit, 0x00C5, 40, 0); // EM_LIMITTEXT
        make("BUTTON", "OK", (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP, 0, 1, (172, 72, 75, 23));
        make("BUTTON", "Cancel", (BS_PUSHBUTTON as u32) | WS_TABSTOP, 0, 2, (253, 72, 75, 23));
        *RESULT.lock().unwrap() = None;
        EnableWindow(parent, 0);
        ShowWindow(dlg, SW_SHOW);
        SetFocus(edit);
        SendMessageW(edit, 0x00B1, 0, -1); // EM_SETSEL
        let mut msg: MSG = std::mem::zeroed();
        while RESULT.lock().unwrap().is_none() && GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if IsDialogMessageW(dlg, &msg) == 0 { TranslateMessage(&msg); DispatchMessageW(&msg); }
        }
        let ok = *RESULT.lock().unwrap() == Some(1);
        let mut buf = [0u16; 64];
        let n = GetWindowTextW(edit, buf.as_mut_ptr(), buf.len() as i32);
        EnableWindow(parent, 1);
        DestroyWindow(dlg);
        DeleteObject(font);
        if ok { Some(String::from_utf16_lossy(&buf[..n.max(0) as usize])) } else { None }
    }
}
/// Linux: zenity's entry box when it's there (the Deck's code-drawn prompt is a later screen).
#[cfg(unix)]
pub fn ask_text(_parent: isize, title: &str, prompt: &str, start: &str) -> Option<String> {
    let out = std::process::Command::new("zenity").args(["--entry", "--title", title, "--text", prompt, "--entry-text", start]).output().ok()?;
    if !out.status.success() { return None; }
    Some(String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').chars().take(40).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn patterns() {
        assert_eq!(super::sdl_pattern("sav"), "sav");
        assert_eq!(super::sdl_pattern("OpenHome*.exe"), "exe");
        assert_eq!(super::sdl_pattern("png;jpg"), "png;jpg");
        assert_eq!(super::sdl_pattern("*"), "*");
    }
}
