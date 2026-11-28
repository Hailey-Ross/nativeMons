//! ModDeps and ModClashes: what Gen1Recomp's ModsCli reports about one game's mods.

use crate::config;
use crate::engines::gen1_cli;
use crate::game_entry::GameEntry;
use std::collections::HashSet;

/// Comparer<string>.Default for the plain ids and lines these lists hold: case-insensitive first, then lower case first.
pub fn culture_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| b.cmp(a))
}

#[derive(Clone, Debug)]
pub struct Dep {
    pub mod_id: String,
    pub mod_name: String,
    pub id: String,
    pub range: String,
    pub github: String,
    pub status: String,
    pub have: String,
    pub required: bool,
}
fn skip_file() -> std::path::PathBuf { config::state().join("mod-deps-declined.txt") }
fn declined() -> HashSet<String> {
    match std::fs::read_to_string(skip_file()) {
        Ok(t) => config::lines(&t).into_iter().filter(|x| !x.trim().is_empty()).map(String::from).collect(),
        Err(_) => HashSet::new(),
    }
}
pub fn decline(d: &Dep) {
    let mut all: Vec<String> = declined().into_iter().collect();
    let k = format!("{}\t{}", d.mod_id, d.id);
    if !all.contains(&k) { all.push(k); }
    all.sort_by(|a, b| culture_cmp(a, b));
    let _ = std::fs::create_dir_all(config::state());
    let _ = config::write_all_lines(&skip_file(), &all);
}
/// What's still to settle for this game: missing or the wrong version, required first.
pub fn open(g: &GameEntry) -> Vec<Dep> {
    let declined = declined();
    let mut list: Vec<Dep> = Vec::new();
    let text = gen1_cli(&format!("deps {}", g.gen1.as_deref().unwrap_or(""))).unwrap_or_default();
    for row in text.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 8 || f[6] == "ok" { continue; }
        let d = Dep { mod_id: f[0].into(), mod_name: f[1].into(), required: f[2] == "required", id: f[3].into(), range: f[4].into(), github: f[5].into(), status: f[6].into(), have: f[7].into() };
        if !d.required && declined.contains(&format!("{}\t{}", d.mod_id, d.id)) { continue; }
        // The same mod asked for by several: one row, the strongest ask.
        if let Some(i) = list.iter().position(|x| x.id == d.id) {
            if d.required && !list[i].required { list[i] = d; }
            continue;
        }
        list.push(d);
    }
    list.sort_by(|a, b| (if a.required { 0 } else { 1 }).cmp(&(if b.required { 0 } else { 1 })).then_with(|| culture_cmp(&a.id, &b.id)));
    list
}

#[derive(Clone, Debug)]
pub struct Clash {
    pub mod_id: String,
    pub mod_name: String,
    pub other_id: String,
    pub other_name: String,
    pub range: String,
    pub mod_on: bool,
    pub other_on: bool,
}
impl Clash {
    pub fn active(&self) -> bool { self.mod_on && self.other_on }
}
pub fn read_clashes(g: &GameEntry) -> Vec<Clash> {
    let text = gen1_cli(&format!("conflicts {}", g.gen1.as_deref().unwrap_or(""))).unwrap_or_default();
    let mut list = Vec::new();
    for row in text.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 7 { continue; }
        list.push(Clash { mod_id: f[0].into(), mod_name: f[1].into(), mod_on: f[2] == "on", other_id: f[3].into(), other_name: f[4].into(), other_on: f[5] == "on", range: f[6].into() });
    }
    list
}

/// The mod's id from its manifest.json, or None.
fn manifest_id(folder: &std::path::Path) -> Option<String> {
    let t = std::fs::read_to_string(folder.join("manifest.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
    v.get("id").and_then(|x| x.as_str()).map(String::from)
}
/// A move, or a copy then delete when the folders are on different drives.
fn move_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    let root = |p: &std::path::Path| p.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase());
    if root(from) == root(to) { return std::fs::rename(from, to); }
    std::fs::create_dir_all(to)?;
    let mut dirs = Vec::new();
    for e in std::fs::read_dir(from)?.flatten() {
        if e.path().is_dir() { dirs.push(e.path()); } else { std::fs::copy(e.path(), to.join(e.file_name()))?; }
    }
    for d in dirs { move_tree(&d, &to.join(d.file_name().unwrap()))?; }
    std::fs::remove_dir_all(from)
}
/// LastInstalled and LastEnable: the mod id the last install held and what `enable-new` answered.
pub static LAST: std::sync::Mutex<Option<(Option<String>, String)>> = std::sync::Mutex::new(None);
/// expected: the mod this zip has to be (a dependency asked for by id), or None for any. Err is an exception's message.
pub fn extract_mod(zip: &str, expected: Option<&str>) -> Result<String, String> {
    let Some(paths) = gen1_cli("paths") else { return Ok("Gen1Recomp was not found.".into()) };
    let mods = paths.split('\n').map(|l| l.trim_end_matches('\r').split('\t').map(String::from).collect::<Vec<_>>()).find(|f| f[0] == "mods").and_then(|f| f.get(1).cloned()).ok_or("Sequence contains no elements")?;
    let mods = std::path::PathBuf::from(crate::engines::win_sep(&mods));
    // Unpacked beside the mods folder, so the last step is a move on one drive. Not inside it, where the game would see it.
    std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
    let parent = mods.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let (temp, old) = (parent.join("nativemons-mod-unpack"), parent.join("nativemons-mod-old"));
    let r = (|| -> Result<String, String> {
        let e = |x: std::io::Error| x.to_string();
        if temp.is_dir() { std::fs::remove_dir_all(&temp).map_err(e)?; }
        crate::zips::extract_all(zip, &temp)?;
        // The manifest is at the top of the archive, or inside one folder.
        let root = if temp.join("manifest.json").is_file() { Some(temp.clone()) } else {
            std::fs::read_dir(&temp).map_err(e)?.flatten().map(|d| d.path()).filter(|p| p.is_dir()).find(|x| x.join("manifest.json").is_file())
        };
        let Some(root) = root else { return Ok("This archive has no manifest.json, so it is not a Gen1Recomp mod.".into()) };
        let holds = manifest_id(&root);
        if let Some(x) = expected {
            if holds.as_deref().map(|h| h.to_lowercase()) != Some(x.to_lowercase()) { return Ok(format!("That archive holds {}, not {}, so nothing was installed.", holds.as_deref().unwrap_or("a mod with no id"), x)); }
        }
        let name = if root == temp { std::path::Path::new(zip).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() } else { root.file_name().unwrap().to_string_lossy().into_owned() };
        let target = mods.join(&name);
        // An older copy steps aside first and comes back if the new one can't go in.
        if old.is_dir() { std::fs::remove_dir_all(&old).map_err(e)?; }
        let replacing = target.is_dir();
        if replacing { std::fs::rename(&target, &old).map_err(e)?; }
        if let Err(x) = move_tree(&root, &target) {
            if target.is_dir() { let _ = std::fs::remove_dir_all(&target); }
            if replacing { let _ = std::fs::rename(&old, &target); }
            return Err(x.to_string());
        }
        if replacing { std::fs::remove_dir_all(&old).map_err(e)?; }
        // A newly installed mod is switched on in each game, unless it clashes there with one that's on.
        let enable = match &holds { Some(h) => gen1_cli(&format!("enable-new {}", h)).unwrap_or_default(), None => String::new() };
        *LAST.lock().unwrap() = Some((holds.clone(), enable));
        Ok(format!("{}{}.", if replacing { "Updated " } else { "Installed " }, name))
    })();
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    r
}
