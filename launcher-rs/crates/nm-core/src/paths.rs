//! Where things are on this platform: the launcher's folder, the state folder, APPDATA and executable names.

use std::path::{Path, PathBuf};

pub trait Paths: Send + Sync {
    /// The launcher's own folder (Config.Here).
    fn here(&self) -> PathBuf;
    /// The state folder: NATIVEMONS_STATE, else <here>/state.
    fn state(&self) -> PathBuf;
    /// %APPDATA% (LOVE's folder follows it); the XDG data home on Linux.
    fn app_data(&self) -> PathBuf;
    /// A program's file name: "x.exe" on Windows, "x" elsewhere.
    fn exe(&self, base: &str) -> String;
}

pub struct SystemPaths;

impl Paths for SystemPaths {
    fn here(&self) -> PathBuf { std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default() }
    fn state(&self) -> PathBuf { std::env::var_os("NATIVEMONS_STATE").map(PathBuf::from).unwrap_or_else(|| self.here().join("state")) }
    fn app_data(&self) -> PathBuf {
        if let Some(v) = std::env::var_os("APPDATA") { return PathBuf::from(v); }
        if cfg!(windows) { return PathBuf::new(); }
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"))
    }
    fn exe(&self, base: &str) -> String { if cfg!(windows) { format!("{}.exe", base) } else { base.to_string() } }
}

/// SpecialFolder.ApplicationData: the user's roaming folder itself, whatever APPDATA says (the C# ignores it there).
pub fn roaming() -> PathBuf {
    if cfg!(windows) { if let Some(u) = std::env::var_os("USERPROFILE") { return PathBuf::from(u).join("AppData").join("Roaming"); } }
    SystemPaths.app_data()
}

/// A Windows-style relative path from the C# ("engines\\fulldex\\x.exe") with this platform's separators.
pub fn rel(s: &str) -> PathBuf { PathBuf::from(s.replace('\\', std::path::MAIN_SEPARATOR_STR)) }

/// Path.GetFullPath: absolute, with "." and ".." resolved, without touching the disk.
pub fn full_path(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
