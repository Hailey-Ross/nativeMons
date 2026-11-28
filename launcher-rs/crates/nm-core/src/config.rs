//! Config: nativemons.cfg, engine paths, state\*.txt reads and writes.

use crate::paths::{full_path, rel, Paths, SystemPaths};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

static PATHS: OnceLock<Box<dyn Paths>> = OnceLock::new();
static VALUES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

pub fn paths() -> &'static dyn Paths { PATHS.get_or_init(|| Box::new(SystemPaths)).as_ref() }
fn values() -> &'static RwLock<HashMap<String, String>> { VALUES.get_or_init(|| RwLock::new(HashMap::new())) }

pub fn here() -> PathBuf { paths().here() }
pub fn state() -> PathBuf { paths().state() }

/// Config.Load: creates the state folder and reads nativemons.cfg (key=value, # comments).
pub fn load() {
    let _ = std::fs::create_dir_all(state());
    let Ok(text) = std::fs::read_to_string(state().join("nativemons.cfg")) else { return };
    let mut v = values().write().unwrap();
    for l in lines(&text) {
        if let Some(eq) = l.find('=') {
            if eq > 0 && !l.starts_with('#') { v.insert(l[..eq].trim().to_string(), l[eq + 1..].trim().to_string()); }
        }
    }
}

/// File.ReadAllLines: split on \r\n or \n, BOM dropped, no trailing empty line.
pub fn lines(text: &str) -> Vec<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    if out.last() == Some(&"") { out.pop(); }
    out
}

/// File.WriteAllLines: UTF-8 without BOM, CRLF after every line.
pub fn write_all_lines(p: &Path, ls: &[String]) -> std::io::Result<()> { std::fs::write(p, ls.iter().map(|l| format!("{}\r\n", l)).collect::<String>()) }

fn get(key: &str, fallback: &str) -> PathBuf {
    let v = values().read().unwrap().get(key).cloned();
    full_path(&here().join(rel(v.as_deref().unwrap_or(fallback))))
}
fn exe_in(dir: &str, base: &str) -> String { format!("{}\\{}", dir, paths().exe(base)) }

pub fn love() -> PathBuf { get("love", &exe_in("love", "lovec")) }
pub fn gen1() -> PathBuf { get("gen1recomp", "engines\\gen1recomp") }
pub fn fulldex() -> PathBuf { get("fulldex", &exe_in("engines\\fulldex", "Pokemon Fulldex")) }
pub fn node() -> PathBuf { get("node", &exe_in("tools\\openhome", "node")) }
pub fn open_home_tool() -> PathBuf { get("openhome", "tools\\openhome\\convert.cjs") }
pub fn platinum_port() -> PathBuf { get("pokeplatinum", &exe_in("engines\\pokeplatinum", "main")) }
pub fn nativeplat() -> PathBuf { get("nativeplat", &exe_in("engines\\nativeplat", "nativeplat")) }
pub fn gba_player() -> PathBuf { get("gbawide", &exe_in("engines\\gbawide", "gbawide")) }
/// Installed PC games live beside the engines, where a launcher update never looks.
pub fn infinite_fusion() -> PathBuf { get("infinitefusion", "games\\infinitefusion\\Game.exe") }
pub fn infinite_fusion2() -> PathBuf { get("infinitefusion2", "games\\infinitefusion2\\InfiniteFusion2.exe") }
pub fn emulators() -> PathBuf { get("emulators", "emulators") }
pub fn zakuro() -> PathBuf { get("zakuro", &exe_in("engines\\zakuro", "zakuro")) }
pub fn patcher() -> PathBuf { get("patcher", "tools\\nds-patcher\\hails_patcher.py") }
pub fn mon_bridge() -> PathBuf { get("monbridge", &exe_in("tools\\mon-bridge", "mon-bridge")) }
pub fn mgba_core() -> PathBuf { get("mgbacore", if cfg!(windows) { "tools\\mgba\\mgba_libretro.dll" } else { "tools\\mgba\\mgba_libretro.so" }) }
pub fn games_file() -> PathBuf { get("games", "games.json") }
pub fn python_dir() -> PathBuf { get("pythondir", "tools\\python") }
/// A python= setting, else the Python the launcher fetched for the patcher, else the system's.
pub fn python() -> String {
    if let Some(v) = values().read().unwrap().get("python") { return v.clone(); }
    let own = python_dir().join(paths().exe("python"));
    if own.exists() { own.to_string_lossy().into_owned() } else if cfg!(windows) { "python".into() } else { "python3".into() }
}
pub fn fulldex_dir() -> PathBuf { fulldex().parent().map(Path::to_path_buf).unwrap_or_default() }

pub fn rom_folders() -> Vec<String> {
    match std::fs::read_to_string(state().join("rom-folders.txt")) {
        Ok(t) => lines(&t).into_iter().filter(|x| !x.trim().is_empty()).map(String::from).collect(),
        Err(_) => Vec::new(),
    }
}
pub fn save_rom_folders(folders: &[String]) { let _ = write_all_lines(&state().join("rom-folders.txt"), folders); }
/// ROMs added one at a time, used where they are (distinct, ignoring case).
pub fn rom_files() -> Vec<String> {
    let Ok(t) = std::fs::read_to_string(state().join("rom-files.txt")) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for l in lines(&t) {
        if l.trim().is_empty() { continue; }
        if !out.iter().any(|o| o.to_lowercase() == l.to_lowercase()) { out.push(l.to_string()); }
    }
    out
}
pub fn save_rom_files(files: &[String]) { let _ = write_all_lines(&state().join("rom-files.txt"), files); }
/// ROMs from a .zip: our own copies, one folder per zip.
pub fn unzipped() -> PathBuf { state().join("roms") }
/// Config.Read: the file's text, trimmed, or the fallback.
pub fn read(name: &str, fallback: &str) -> String {
    match std::fs::read(state().join(name)) {
        Ok(b) => {
            let s = String::from_utf8_lossy(&b).into_owned();
            s.strip_prefix('\u{feff}').unwrap_or(&s).trim().to_string()
        }
        Err(_) => fallback.to_string(),
    }
}
pub fn write(name: &str, text: &str) { let _ = std::fs::write(state().join(name), text); }
