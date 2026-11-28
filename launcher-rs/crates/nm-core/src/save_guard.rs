//! SaveGuard: hashes, verified backups, checks through mon-bridge and verified in-place writes.

use crate::config;
use crate::game_entry::GameEntry;
use crate::proc::{self, StartInfo};
use crate::save_sets;
use sha2::{Digest, Sha256};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

/// string.CompareOrdinal: UTF-16 code units in order.
pub fn ordinal(a: &str, b: &str) -> std::cmp::Ordering { a.encode_utf16().cmp(b.encode_utf16()) }

/// DateTime.Now formatted with a .NET pattern made of the parts the launcher uses.
pub fn now(pattern: &str) -> String {
    let fmt = pattern.replace("yyyy", "%Y").replace("MM", "%m").replace("dd", "%d").replace("HH", "%H").replace("mm", "%M").replace("ss", "%S").replace("fff", "%3f");
    chrono::Local::now().format(&fmt).to_string()
}

pub fn hash(file: Option<&Path>) -> String {
    let Some(file) = file else { return String::new() };
    let Ok(data) = std::fs::read(file) else { return String::new() };
    if !file.is_file() { return String::new(); }
    Sha256::digest(&data).iter().map(|b| format!("{:02X}", b)).collect()
}
pub fn hash_of(file: &Path) -> String { hash(Some(file)) }
pub fn hash_of_bytes(data: &[u8]) -> String { Sha256::digest(data).iter().map(|b| format!("{:02X}", b)).collect() }

/// Each save has its own backups, so one can't push out another's. The first keeps the original folder.
pub fn backup_folder(g: &GameEntry) -> PathBuf { backup_folder_of(g, &save_sets::active(g)) }
pub fn backup_folder_of(g: &GameEntry, set: &str) -> PathBuf {
    let folder = config::state().join("save-backups").join(&g.id);
    if set == "1" { folder } else { folder.join(format!("save-{}", set)) }
}

/// Runs mon-bridge with the originals, Node and OpenHome it may need. (output or its errors, exit code)
pub fn bridge(args: &str) -> (String, i32) {
    let mut psi = StartInfo::new(config::mon_bridge(), args);
    psi.set_env("NATIVEMONS_ORIGINALS", &config::state().join("pokemon-originals").to_string_lossy());
    psi.set_env("NATIVEMONS_NODE", &config::node().to_string_lossy());
    psi.set_env("NATIVEMONS_OPENHOME", &config::open_home_tool().to_string_lossy());
    match proc::run(&psi, 60) {
        Ok(f) => match f.code {
            None => ("That took too long and was stopped.".into(), -1),
            Some(c) => (if !f.out.trim().is_empty() { f.out } else { f.err }, c),
        },
        Err(e) => (e.to_string(), -1),
    }
}

/// Fulldex makes an empty file when a game first starts. Nothing has been saved in it yet, so it isn't damage.
pub fn is_blank(file: &Path) -> bool {
    match std::fs::read(file) {
        Ok(d) => !d.is_empty() && d.iter().all(|&b| b == d[0]),
        Err(_) => false,
    }
}

/// None when the save is healthy, otherwise what's wrong with it. A romhack's save only has to read.
pub fn problem_for(g: Option<&GameEntry>, file: &Path) -> Option<String> {
    match g {
        Some(g) if g.is_hack() => {
            if !config::mon_bridge().is_file() || is_blank(file) { return None; }
            let (_, code) = bridge(&format!("info \"{}\"", file.display()));
            if code == 0 { return None; }
            if !g.is_ds() && gen3_intact(file) { None } else { Some("It can't be read.".into()) }
        }
        _ => problem(file),
    }
}
/// A GBA Pokemon save the game can load: at least one whole save (14 sectors) carrying the game's sector signature.
fn gen3_intact(file: &Path) -> bool {
    let Ok(d) = std::fs::read(file) else { return false };
    if d.len() < 0x20000 { return false; }
    let signed = (0..32).filter(|s| { let at = s * 0x1000 + 0xFF8; u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]) == 0x0801_2025 }).count();
    signed >= 14
}
pub fn problem(file: &Path) -> Option<String> {
    if !config::mon_bridge().is_file() || is_blank(file) { return None; }
    let (o, code) = bridge(&format!("check \"{}\"", file.display()));
    if code == 0 { return None; }
    let t = o.trim();
    let f: Vec<&str> = t.split('\t').collect();
    Some(if f.len() > 1 { f[1].to_string() } else { t.to_string() })
}

fn sav_files(folder: &Path) -> Vec<String> {
    let mut l: Vec<String> = match std::fs::read_dir(folder) {
        Ok(rd) => rd.flatten().filter(|e| e.path().is_file()).map(|e| e.path().to_string_lossy().into_owned()).filter(|p| p.to_lowercase().ends_with(".sav")).collect(),
        Err(_) => Vec::new(),
    };
    l.sort_by(|a, b| ordinal(b, a));
    l
}

/// A verified copy, skipped if the newest backup already holds these bytes. Damaged saves are kept too, and marked.
pub fn backup(g: &GameEntry, file: Option<&Path>, reason: &str) -> std::io::Result<Option<PathBuf>> {
    let Some(file) = file.filter(|f| f.is_file()) else { return Ok(None) };
    let folder = backup_folder(g);
    std::fs::create_dir_all(&folder)?;
    let h = hash_of(file);
    if let Some(newest) = sav_files(&folder).into_iter().next() {
        if hash_of(Path::new(&newest)) == h { return Ok(Some(PathBuf::from(newest))); }
    }
    let mark = if problem_for(Some(g), file).is_some() { "-damaged" } else { "" };
    let target = folder.join(format!("{}-{}{}.sav", now("yyyyMMdd-HHmmss-fff"), reason, mark));
    let temp = PathBuf::from(format!("{}.part", target.display()));
    std::fs::copy(file, &temp)?;
    if hash_of(&temp) != h {
        let _ = std::fs::remove_file(&temp);
        return Err(Error::new(ErrorKind::Other, "A backup didn't match its save, so nothing was changed. Check the drive."));
    }
    std::fs::rename(&temp, &target)?;
    // The newest 30 stay. The oldest healthy one is never the last to go, as only extras are removed.
    for old in sav_files(&folder).into_iter().skip(30) { let _ = std::fs::remove_file(old); }
    Ok(Some(target))
}
pub fn backups(g: &GameEntry) -> Vec<String> { sav_files(&backup_folder(g)) }

pub fn write_in_place(source: &Path, target: &Path) -> std::io::Result<()> {
    let temp = PathBuf::from(format!("{}.part", target.display()));
    let h = hash_of(source);
    if let Some(p) = target.parent() { std::fs::create_dir_all(p)?; }
    std::fs::copy(source, &temp)?;
    if hash_of(&temp) != h {
        let _ = std::fs::remove_file(&temp);
        return Err(Error::new(ErrorKind::Other, "A copy didn't match its source, so nothing was changed."));
    }
    std::fs::copy(&temp, target)?;
    std::fs::remove_file(&temp)
}
