//! CustomGames: PC games the player added (state\pc-games.json). So far reading them into the library.

use crate::config;
use crate::game_entry::{GameEntry, Theme};

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub id: Option<String>,
    pub title: Option<String>,
    pub exe: Option<String>,
    pub rel: Option<String>,
    pub copy: bool,
}

pub fn all() -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(config::state().join("pc-games.json")) else { return Vec::new() };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { return Vec::new() };
    let s = |o: &serde_json::Value, k: &str| o.get(k).and_then(|v| v.as_str()).map(String::from);
    items.iter().map(|o| Entry { id: s(o, "id"), title: s(o, "title"), exe: s(o, "exe"), rel: s(o, "rel"), copy: o.get("copy").and_then(|v| v.as_bool()).unwrap_or(false) }).collect()
}

/// Their themes come from this set in turn, so they don't all look alike.
const LOOKS: [[&str; 3]; 6] = [
    ["#4F6D8F", "#243447", "#9CC3E6"],
    ["#5E8C61", "#2C4430", "#A9D8A3"],
    ["#8A5A9E", "#41284D", "#D6B0E6"],
    ["#B5793A", "#5A3A16", "#F0C48A"],
    ["#3E8A8A", "#1C4242", "#9BDADA"],
    ["#9E4F5E", "#4A2129", "#E6A7B3"],
];

pub fn to_game(e: &Entry, n: usize) -> GameEntry {
    let look = LOOKS[n % LOOKS.len()];
    let mut g = GameEntry { id: e.id.clone().unwrap_or_default(), title: e.title.clone().unwrap_or_default(), console: Some("PC".into()), pc: Some("custom".into()), ..Default::default() };
    g.theme = Theme::make(look[0], look[1], look[2], Some("plain"));
    g.theme.game_id = Some(g.id.clone());
    g
}

pub fn find(id: &str) -> Option<Entry> { all().into_iter().find(|e| e.id.as_deref() == Some(id)) }
pub fn dir(e: &Entry) -> std::path::PathBuf {
    if e.copy { config::here().join("games").join(e.id.as_deref().unwrap_or("")) } else { std::path::Path::new(e.exe.as_deref().unwrap_or("")).parent().map(|p| p.to_path_buf()).unwrap_or_default() }
}
pub fn exe(e: &Entry) -> String {
    if !e.copy { return e.exe.clone().unwrap_or_default(); }
    let inner = match e.rel.as_deref() { Some(r) if !r.is_empty() => r.to_string(), _ => crate::library::file_name(e.exe.as_deref().unwrap_or("")) };
    dir(e).join(crate::paths::rel(&inner)).to_string_lossy().into_owned()
}

/// JavaScriptSerializer's string form: quotes, backslashes, control characters and <>&' escaped.
fn js(s: &Option<String>) -> String {
    let Some(s) = s else { return "null".into() };
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            '\n' => o += "\\n",
            '\r' => o += "\\r",
            '\t' => o += "\\t",
            '\u{8}' => o += "\\b",
            '\u{c}' => o += "\\f",
            '<' | '>' | '&' | '\'' => o += &format!("\\u{:04x}", c as u32),
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn save(all: &[Entry]) -> std::io::Result<()> {
    std::fs::create_dir_all(config::state())?;
    let items: Vec<String> = all.iter().map(|e| format!("{{\"id\":{},\"title\":{},\"exe\":{},\"rel\":{},\"copy\":{}}}", js(&e.id), js(&e.title), js(&e.exe), js(&e.rel), e.copy)).collect();
    std::fs::write(config::state().join("pc-games.json"), format!("[{}]", items.join(",")))
}

/// A folder's programs, likeliest first, leaving out installers, crash reporters and runtimes.
const NOT_THE_GAME: [&str; 11] = ["unins", "setup", "install", "crash", "redist", "vcredist", "dxsetup", "dotnet", "update", "launcherhelper", "notification_helper"];
pub fn programs(dir: &str) -> Vec<String> {
    let root = dir.trim_end_matches('\\').to_string();
    let mut l: Vec<String> = crate::library::files_under(&root).into_iter().filter(|f| crate::library::ext_lower(f) == ".exe").filter(|f| {
        let n = std::path::Path::new(f).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default().replace(' ', "").replace('_', "");
        !NOT_THE_GAME.iter().any(|x| n.contains(x))
    }).collect();
    l.sort_by(|a, b| {
        let da = a[root.len()..].matches('\\').count();
        let db = b[root.len()..].matches('\\').count();
        da.cmp(&db).then_with(|| crate::save_guard::ordinal(&a.to_uppercase(), &b.to_uppercase()))
    });
    l
}
fn writable(f: &std::path::Path) {
    if let Ok(m) = std::fs::metadata(f) {
        let mut p = m.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        let _ = std::fs::set_permissions(f, p);
    }
}
/// PcGames.Remove: a folder deleted, read-only files included.
pub fn remove_dir(dir: &std::path::Path) -> std::io::Result<()> {
    for f in crate::library::files_under(&dir.to_string_lossy()) { writable(std::path::Path::new(&f)); }
    std::fs::remove_dir_all(dir)
}
/// Unpacks a .zip into games\unpacking-<n>, never outside it. (problem, work folder, programs)
pub fn unpack(zip: &str) -> (Option<String>, Option<String>, Vec<String>) {
    if !std::path::Path::new(zip).is_file() { return (Some("That zip isn't there.".into()), None, Vec::new()); }
    // One left by a choice that was never made goes first.
    if let Ok(rd) = std::fs::read_dir(config::here().join("games")) {
        for d in rd.flatten() {
            if d.path().is_dir() && d.file_name().to_string_lossy().to_lowercase().starts_with("unpacking-") { let _ = remove_dir(&d.path()); }
        }
    }
    let stage = config::here().join("games").join(format!("unpacking-{}", &crate::proc::new_guid()[..8]));
    let r = (|| -> Result<Option<String>, String> {
        let mut a = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        for i in 0..a.len() {
            let n = a.by_index(i).map_err(|e| e.to_string())?.name().replace('\\', "/");
            if !n.ends_with('/') { files.push((i, n)); }
        }
        if files.is_empty() { return Ok(Some("That zip is empty.".into())); }
        let firsts: Vec<Vec<&str>> = files.iter().map(|(_, n)| n.split('/').collect()).collect();
        let mut top: Option<String> = None;
        if firsts.iter().all(|s| s.len() > 1) {
            let mut seen: Vec<String> = Vec::new();
            for s in &firsts { if !seen.iter().any(|x| x.eq_ignore_ascii_case(s[0])) { seen.push(s[0].to_string()); } }
            if seen.len() == 1 { top = Some(format!("{}/", firsts[0][0])); }
        }
        let full = format!("{}\\", crate::paths::full_path(&stage).display());
        for (i, entry) in &files {
            let mut name = entry.clone();
            if let Some(t) = &top { name = name[t.len()..].to_string(); }
            let target = crate::paths::full_path(&stage.join(crate::paths::rel(&name.replace('/', "\\"))));
            if !target.to_string_lossy().to_lowercase().starts_with(&full.to_lowercase()) { return Err(format!("It holds a file meant for outside its folder ({}).", entry)); }
            std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            crate::zips::extract_entry(&mut a, *i, &target)?;
        }
        Ok(None)
    })();
    match r {
        Ok(Some(p)) => {
            let _ = remove_dir(&stage);
            (Some(p), None, Vec::new())
        }
        Ok(None) => {
            let programs = programs(&stage.to_string_lossy());
            if programs.is_empty() {
                let _ = remove_dir(&stage);
                return (Some("That zip has no program (.exe) in it, so there's nothing to start. Nothing was added.".into()), None, Vec::new());
            }
            (None, Some(stage.to_string_lossy().into_owned()), programs)
        }
        Err(x) => {
            if stage.is_dir() { let _ = remove_dir(&stage); }
            (Some(format!("Unpacking stopped: {} Nothing was added.", x)), None, Vec::new())
        }
    }
}
/// FileVersionInfo.ProductName, or None.
#[cfg(windows)]
fn product_name(exe: &str) -> Option<String> {
    #[link(name = "version")]
    extern "system" {
        fn GetFileVersionInfoSizeW(f: *const u16, h: *mut u32) -> u32;
        fn GetFileVersionInfoW(f: *const u16, h: u32, len: u32, data: *mut u8) -> i32;
        fn VerQueryValueW(block: *const u8, sub: *const u16, buf: *mut *const u8, len: *mut u32) -> i32;
    }
    let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let f = w(exe);
    unsafe {
        let mut h = 0;
        let n = GetFileVersionInfoSizeW(f.as_ptr(), &mut h);
        if n == 0 { return None; }
        let mut data = vec![0u8; n as usize];
        if GetFileVersionInfoW(f.as_ptr(), 0, n, data.as_mut_ptr()) == 0 { return None; }
        let mut tried: Vec<String> = Vec::new();
        let mut p: *const u8 = std::ptr::null();
        let mut len = 0u32;
        if VerQueryValueW(data.as_ptr(), w("\\VarFileInfo\\Translation").as_ptr(), &mut p, &mut len) != 0 && len >= 4 {
            let lang = u16::from_le_bytes([*p, *p.add(1)]);
            let cp = u16::from_le_bytes([*p.add(2), *p.add(3)]);
            tried.push(format!("{:04x}{:04x}", lang, cp));
        }
        tried.extend(["040904b0", "040904e4", "04090000"].iter().map(|s| s.to_string()));
        for t in tried {
            let mut q: *const u8 = std::ptr::null();
            let mut ql = 0u32;
            if VerQueryValueW(data.as_ptr(), w(&format!("\\StringFileInfo\\{}\\ProductName", t)).as_ptr(), &mut q, &mut ql) != 0 && ql > 0 {
                let s = std::slice::from_raw_parts(q as *const u16, ql as usize);
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                return Some(String::from_utf16_lossy(&s[..end]));
            }
        }
        None
    }
}
#[cfg(not(windows))]
fn product_name(_exe: &str) -> Option<String> { None }

/// An id and a name: the program's own product name, else the fallback (its folder's, or the zip's).
fn new_entry(taken: &[String], exe: &str, fallback: &str) -> Entry {
    let mut title = product_name(exe).unwrap_or_default();
    if title.trim().is_empty() || title.encode_utf16().count() > 40 { title = fallback.to_string(); }
    let re = regex::Regex::new("[^a-z0-9]+").unwrap();
    let mut slug = re.replace_all(&title.to_lowercase(), "-").trim_matches('-').to_string();
    if slug.is_empty() { slug = "game".into(); }
    let all = all();
    let mut id = format!("pc-{}", slug);
    let mut i = 2;
    while all.iter().any(|x| x.id.as_deref() == Some(id.as_str())) || taken.contains(&id) {
        id = format!("pc-{}-{}", slug, i);
        i += 1;
    }
    Entry { id: Some(id), title: Some(title), exe: Some(crate::paths::full_path(std::path::Path::new(exe)).to_string_lossy().into_owned()), rel: None, copy: false }
}
/// The unpacked zip becomes the game's own folder in games\, running the chosen program.
pub fn add_unpacked(taken: &[String], zip: &str, work: &str, program: &str) -> Result<Entry, String> {
    let r = (|| -> Result<Entry, String> {
        let stem = std::path::Path::new(zip).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let mut e = new_entry(taken, program, &stem);
        e.exe = Some(crate::paths::full_path(std::path::Path::new(zip)).to_string_lossy().into_owned());
        e.copy = true;
        e.rel = Some(program[work.trim_end_matches('\\').len() + 1..].to_string());
        std::fs::rename(work, dir(&e)).map_err(|x| x.to_string())?;
        let mut a = all();
        a.push(e.clone());
        save(&a).map_err(|x| x.to_string())?;
        Ok(e)
    })();
    r.map_err(|x| {
        if std::path::Path::new(work).is_dir() { let _ = remove_dir(std::path::Path::new(work)); }
        format!("Adding stopped: {} Nothing was added.", x)
    })
}
/// Run where it is (copy false), or copied into the launcher. Named after the program's product name, else its folder.
pub fn add(taken: &[String], exe: &str, copy: bool) -> Result<Entry, String> {
    if !std::path::Path::new(exe).is_file() { return Err("That program isn't there.".into()); }
    let mut a = all();
    let folder = std::path::Path::new(exe).parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut e = new_entry(taken, exe, &folder);
    e.copy = copy;
    if copy {
        let from = std::path::Path::new(e.exe.as_deref().unwrap_or("")).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let to = dir(&e);
        let work = std::path::PathBuf::from(format!("{}.adding", to.display()));
        if from.trim_end_matches('\\').len() <= 2 { return Err("That program sits at the top of a drive, so its folder can't be copied. Add it where it is instead.".into()); }
        let r = (|| -> std::io::Result<()> {
            if work.is_dir() { remove_dir(&work)?; }
            for f in crate::library::files_under(&from) {
                let rel = &f[from.trim_end_matches('\\').len() + 1..];
                let target = work.join(crate::paths::rel(rel));
                std::fs::create_dir_all(target.parent().unwrap())?;
                std::fs::copy(&f, &target)?;
                writable(&target);
            }
            std::fs::rename(&work, &to)
        })();
        if let Err(x) = r {
            if work.is_dir() { let _ = remove_dir(&work); }
            return Err(format!("Copying stopped: {} Nothing was added.", x));
        }
    }
    a.push(e.clone());
    save(&a).map_err(|x| x.to_string())?;
    Ok(e)
}
/// Rename: the title shown.
pub fn rename(id: &str, title: &str) {
    let mut a = all();
    let Some(e) = a.iter_mut().find(|x| x.id.as_deref() == Some(id)) else { return };
    e.title = Some(title.to_string());
    let _ = save(&a);
}
/// Remove: forgets the game; a copy the launcher held is deleted, the player's own never is.
pub fn remove(id: &str) -> Option<String> {
    let mut a = all();
    let i = a.iter().position(|x| x.id.as_deref() == Some(id))?;
    let d = dir(&a[i]);
    if a[i].copy && d.is_dir() { if let Err(x) = remove_dir(&d) { return Some(format!("Couldn't delete its copy: {}", x)); } }
    a.remove(i);
    save(&a).err().map(|x| x.to_string())
}
