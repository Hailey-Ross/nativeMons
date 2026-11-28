//! SaveSets: several saves per game. One is in use at the normal paths; the others are parked in
//! state\save-sets\<game>\<n>.

use crate::config;
use crate::game_entry::GameEntry;
use std::path::PathBuf;

pub fn root(g: &GameEntry) -> PathBuf { config::state().join("save-sets").join(&g.id) }
pub fn dir(g: &GameEntry, set: &str) -> PathBuf { root(g).join(set) }
pub fn journal(g: &GameEntry) -> PathBuf { root(g).join("switching.txt") }

/// File.ReadAllText(f).Trim(), as the C# reads these small files (BOM dropped).
pub fn read_trim(f: &std::path::Path) -> Option<String> {
    let b = std::fs::read(f).ok()?;
    let s = String::from_utf8_lossy(&b).into_owned();
    Some(s.strip_prefix('\u{feff}').unwrap_or(&s).trim().to_string())
}

pub fn active(g: &GameEntry) -> String {
    let f = root(g).join("active.txt");
    if f.is_file() {
        if let Some(v) = read_trim(&f) { if !v.is_empty() { return v; } }
    }
    "1".into()
}
pub fn set_active(g: &GameEntry, set: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(root(g))?;
    let f = root(g).join("active.txt");
    let part = root(g).join("active.txt.part");
    std::fs::write(&part, set)?;
    std::fs::copy(&part, &f)?;
    std::fs::remove_file(&part)
}
pub fn parse_int(s: &str) -> Option<i32> { s.trim().parse::<i32>().ok() }
pub fn all(g: &GameEntry) -> Vec<String> {
    let mut l: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root(g)) {
        for d in rd.flatten() { if d.path().is_dir() { l.push(d.file_name().to_string_lossy().into_owned()); } }
    }
    let a = active(g);
    if !l.contains(&a) { l.push(a); }
    l.sort_by(|x, y| match (parse_int(x), parse_int(y)) { (Some(a), Some(b)) => a.cmp(&b), _ => crate::save_guard::ordinal(x, y) });
    l
}
pub fn name(g: &GameEntry, set: &str) -> String {
    let f = dir(g, set).join("name.txt");
    if f.is_file() {
        if let Some(v) = read_trim(&f) { if !v.is_empty() { return v; } }
    }
    format!("Save {}", set)
}
pub fn set_name(g: &GameEntry, set: &str, name: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(g, set))?;
    std::fs::write(dir(g, set).join("name.txt"), name)
}
/// Numbers are never reused, so a new save can't inherit a deleted one's backups.
pub fn next(g: &GameEntry) -> std::io::Result<String> {
    std::fs::create_dir_all(root(g))?;
    let f = root(g).join("next.txt");
    let mut n = 1;
    for s in all(g) { if let Some(v) = parse_int(&s) { if v > n { n = v; } } }
    if f.is_file() { if let Some(v) = read_trim(&f).and_then(|t| parse_int(&t)) { if v > n { n = v; } } }
    n += 1;
    std::fs::write(&f, n.to_string())?;
    std::fs::create_dir_all(dir(g, &n.to_string()))?;
    Ok(n.to_string())
}
