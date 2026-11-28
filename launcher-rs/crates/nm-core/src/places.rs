//! The browser's places: the player's known folders and the drives (DriveInfo.GetDrives), read only.

use std::path::PathBuf;

pub struct Drive {
    /// "C:\".
    pub name: String,
    pub label: String,
    pub removable: bool,
    /// AvailableFreeSpace and TotalSize, when they could be read.
    pub space: Option<(u64, u64)>,
}

#[cfg(windows)]
mod win {
    #[repr(C)]
    pub struct Guid(pub u32, pub u16, pub u16, pub [u8; 8]);
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetLogicalDrives() -> u32;
        pub fn GetDriveTypeW(root: *const u16) -> u32;
        pub fn GetVolumeInformationW(root: *const u16, name: *mut u16, name_len: u32, serial: *mut u32, max_len: *mut u32, flags: *mut u32, fs: *mut u16, fs_len: u32) -> i32;
        pub fn GetDiskFreeSpaceExW(root: *const u16, free: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
        pub fn SetErrorMode(mode: u32) -> u32;
    }
    #[link(name = "shell32")]
    extern "system" {
        pub fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: *mut core::ffi::c_void, path: *mut *mut u16) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        pub fn CoTaskMemFree(p: *mut core::ffi::c_void);
    }
    pub fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
    pub fn known(id: Guid) -> Option<String> {
        let mut p: *mut u16 = std::ptr::null_mut();
        unsafe {
            if SHGetKnownFolderPath(&id, 0, std::ptr::null_mut(), &mut p) != 0 || p.is_null() { return None; }
            let mut n = 0;
            while *p.add(n) != 0 { n += 1; }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            CoTaskMemFree(p as *mut _);
            Some(s)
        }
    }
}

/// Environment.GetFolderPath for Desktop, MyDocuments and MyPictures, and the profile's Downloads.
pub fn known_folders() -> Vec<(&'static str, String)> {
    #[cfg(windows)]
    {
        use win::Guid;
        let get = |g: Guid| win::known(g).unwrap_or_default();
        let profile = get(Guid(0x5E6C858F, 0x0E22, 0x4760, [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73]));
        vec![
            ("Desktop", get(Guid(0xB4BFCC3A, 0xDB2C, 0x424C, [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41]))),
            ("Documents", get(Guid(0xFDD39AD0, 0x238F, 0x46AF, [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7]))),
            ("Downloads", if profile.is_empty() { String::new() } else { PathBuf::from(&profile).join("Downloads").to_string_lossy().into_owned() }),
            ("Pictures", get(Guid(0x33E28130, 0x4E1E, 0x4676, [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB]))),
        ]
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let at = |d: &str| if home.is_empty() { String::new() } else { PathBuf::from(&home).join(d).to_string_lossy().into_owned() };
        vec![("Desktop", at("Desktop")), ("Documents", at("Documents")), ("Downloads", at("Downloads")), ("Pictures", at("Pictures"))]
    }
}

/// The drives that are ready, in letter order, as DriveInfo.GetDrives with IsReady.
pub fn drives() -> Vec<Drive> {
    let mut out = Vec::new();
    #[cfg(windows)]
    unsafe {
        // No "insert a disk" dialogs for empty readers.
        let old = win::SetErrorMode(0x0001 | 0x8000);
        let mask = win::GetLogicalDrives();
        for i in 0..26u32 {
            if mask & (1 << i) == 0 { continue; }
            let name = format!("{}:\\", (b'A' + i as u8) as char);
            let root = win::wide(&name);
            let kind = win::GetDriveTypeW(root.as_ptr());
            if kind <= 1 { continue; }
            let mut label = [0u16; 261];
            let (mut serial, mut max, mut flags) = (0u32, 0u32, 0u32);
            if win::GetVolumeInformationW(root.as_ptr(), label.as_mut_ptr(), 261, &mut serial, &mut max, &mut flags, std::ptr::null_mut(), 0) == 0 { continue; }
            let n = label.iter().position(|&c| c == 0).unwrap_or(0);
            let (mut free, mut total, mut tfree) = (0u64, 0u64, 0u64);
            let space = if win::GetDiskFreeSpaceExW(root.as_ptr(), &mut free, &mut total, &mut tfree) != 0 { Some((free, total)) } else { None };
            out.push(Drive { name, label: String::from_utf16_lossy(&label[..n]), removable: kind == 2, space });
        }
        win::SetErrorMode(old);
    }
    #[cfg(not(windows))]
    {
        out.push(Drive { name: "/".into(), label: String::new(), removable: false, space: None });
    }
    out
}
