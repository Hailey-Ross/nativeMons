//! Updater: the newest release of the chosen channel (release.json), and applying its package over the launcher folder.

use crate::config;
use crate::net;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

const SITE: &str = "https://github.com/Hailey-Ross/nativeMons/releases/download/";
pub const STABLE_TAG: &str = "stable";
pub const DEV_TAG: &str = "bleeding-edge";

#[derive(Clone, Debug, Default)]
pub struct Release {
    pub channel: Option<String>,
    pub revision: Option<String>,
    pub version: Option<String>,
    pub package: Option<String>,
    pub sha256: Option<String>,
    pub build: i32,
}
static LATEST: Mutex<Option<Release>> = Mutex::new(None);
pub fn latest() -> Option<Release> { LATEST.lock().unwrap().clone() }
/// Sets the release the check found (the snapshot's update banner sets a sample one).
pub fn set_latest(r: Option<Release>) { *LATEST.lock().unwrap() = r; }
pub fn latest_label() -> String { latest().map(|r| label(r.channel.as_deref().unwrap_or(""), r.build, r.version.as_deref())).unwrap_or_default() }
pub fn upper(s: &str) -> String { let mut c = s.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }

fn base() -> String { std::env::var("NATIVEMONS_UPDATE_URL").unwrap_or_else(|_| SITE.into()) }
fn version() -> HashMap<String, String> {
    let mut d = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(config::here().join("version.txt")) {
        for l in config::lines(&t) { if let Some(e) = l.find('=') { if e > 0 { d.insert(l[..e].to_string(), l[e + 1..].trim().to_string()); } } }
    }
    d
}
pub fn build() -> i32 { version().get("build").and_then(|v| v.trim().parse().ok()).unwrap_or(0) }
fn installed() -> Option<String> { version().get("channel").cloned() }
fn revision() -> String { version().get("revision").cloned().unwrap_or_default() }
pub fn installed_label() -> String { label(&installed().unwrap_or_default(), build(), version().get("version").map(|s| s.as_str())) }
pub fn channel() -> String {
    let c = config::read("update-channel.txt", "").trim().to_string();
    if c == STABLE_TAG || c == DEV_TAG { c } else { installed().unwrap_or_else(|| DEV_TAG.into()) }
}
pub fn choose(channel: &str) { config::write("update-channel.txt", channel); set_latest(None); }
/// The tags stay as published; players see Stable and Dev.
pub fn name(channel: &str) -> &'static str { if channel == STABLE_TAG { "Stable" } else { "Dev" } }
pub fn installed_name() -> &'static str { name(&installed().unwrap_or_else(|| DEV_TAG.into())) }
pub fn switching() -> bool { installed().map_or(false, |i| i != channel()) }
/// A copy built here (version.txt's local=): no update is offered over it.
pub fn local() -> Option<String> { version().get("local").cloned() }
pub fn ready() -> bool {
    let Some(r) = latest() else { return false };
    let same = r.revision.as_deref() == Some(revision().as_str());
    if local().is_some() || r.sha256.as_deref().map_or(true, |s| s.is_empty()) || same { return false; }
    match installed() {
        Some(i) if r.channel.as_deref() != Some(i.as_str()) => !same,
        _ => r.build > build(),
    }
}
fn parse(text: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let s = |k: &str| v.get(k).and_then(|x| match x { serde_json::Value::String(s) => Some(s.clone()), serde_json::Value::Null => None, o => Some(o.to_string()) });
    let build = v.get("build").map(|b| b.as_i64().map(|n| n as i32).or_else(|| b.as_str().and_then(|t| t.trim().parse().ok())).unwrap_or(0)).unwrap_or(0);
    Ok(Release { channel: s("channel"), revision: s("revision"), version: s("version"), package: s("package"), sha256: s("sha256"), build })
}
/// None when all is well, or why it couldn't look.
pub fn check() -> Option<String> {
    let r = net::download_string(&format!("{}{}/release.json", base(), channel()), None).and_then(|t| parse(&t));
    let mut l = LATEST.lock().unwrap();
    match r {
        Ok(rel) => { *l = Some(rel); None }
        Err(e) => { *l = None; Some(e) }
    }
}

const KEPT: [&str; 9] = ["engines\\fulldex\\saves\\", "engines\\fulldex\\roms\\", "engines\\fulldex\\states\\", "engines\\fulldex\\cheats\\", "engines\\fulldex\\settings.cfg", "engines\\nativeplat\\userdata\\", "engines\\pokeplatinum\\save.bin", "engines\\pokeplatinum\\sim_config.ini", "state\\"];
fn is_kept(relative: &str) -> bool { KEPT.iter().any(|k| relative.to_lowercase().starts_with(&k.to_lowercase())) }
fn label(channel: &str, build: i32, version: Option<&str>) -> String {
    let v = version.unwrap_or("");
    if channel == STABLE_TAG && !v.is_empty() { format!("version {}, build {}", v, build) } else { format!("build {}{}", build, if v.is_empty() { String::new() } else { format!(", after version {}", v) }) }
}
fn sha_hex(r: &mut dyn Read) -> std::io::Result<String> {
    let mut h = Sha256::new();
    std::io::copy(r, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{:02x}", b)).collect())
}

/// Downloads, proves and applies the latest release. None when done. Nothing changes until the download has proved good.
pub fn apply(report: &mut dyn FnMut(&str, f32)) -> Option<String> {
    let Some(release) = latest().filter(|r| r.sha256.as_deref().map_or(false, |s| !s.is_empty())) else { return Some("There's no release to update to.".into()) };
    let work = config::state().join("update");
    let zip = work.join("package.zip");
    let here = config::here();
    let r = (|| -> Result<Option<String>, String> {
        let e = |x: std::io::Error| x.to_string();
        if work.is_dir() { std::fs::remove_dir_all(&work).map_err(e)?; }
        std::fs::create_dir_all(&work).map_err(e)?;
        let what = format!("Downloading {}", label(release.channel.as_deref().unwrap_or(""), release.build, release.version.as_deref()));
        if let Err(x) = net::download_file(&format!("{}{}/{}", base(), channel(), release.package.as_deref().unwrap_or("")), &zip, None, &mut |_| report(&what, 0.0)) {
            return Ok(Some(format!("The download didn't finish, and nothing was changed. {}", x)));
        }
        report("Checking the download", 0.72);
        let hash = sha_hex(&mut std::fs::File::open(&zip).map_err(e)?).map_err(e)?;
        if hash != release.sha256.clone().unwrap_or_default().to_lowercase() { return Ok(Some("The download didn't match its checksum, so nothing was changed.".into())); }
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip).map_err(e)?).map_err(|x| x.to_string())?;
        let relative = |full: &str| { let p = full.replace('/', "\\"); match p.find('\\') { Some(c) => p[c + 1..].to_string(), None => p } };
        let mut files: Vec<(usize, String, u64)> = Vec::new();
        for i in 0..archive.len() {
            let f = archive.by_index(i).map_err(|x| x.to_string())?;
            let name = f.name().to_string();
            if name.ends_with('/') || name.ends_with('\\') { continue; }
            files.push((i, relative(&name), f.size()));
        }
        let names: Vec<String> = files.iter().map(|f| f.1.to_lowercase()).collect();
        for needed in ["NativeMonsLauncher.exe", "games.json", "engines\\gen1recomp\\main.lua", "engines\\fulldex\\Pokemon Fulldex.exe", "engines\\nativeplat\\nativeplat.exe", "tools\\mon-bridge\\mon-bridge.exe"] {
            if !names.contains(&needed.to_lowercase()) { return Ok(Some(format!("This release is missing {}, so nothing was changed.", needed))); }
        }
        let (mut done, mut changed) = (0usize, 0usize);
        let count = files.len();
        for (i, rel, size) in &files {
            let target = here.join(crate::paths::rel(rel));
            report("Updating", 0.75 + 0.25 * done as f32 / count as f32);
            done += 1;
            if is_kept(rel) { continue; }
            if target.is_file() && std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0) == *size {
                let a = sha_hex(&mut archive.by_index(*i).map_err(|x| x.to_string())?).map_err(e)?;
                let b = sha_hex(&mut std::fs::File::open(&target).map_err(e)?).map_err(e)?;
                if a == b { continue; }
            }
            std::fs::create_dir_all(target.parent().unwrap()).map_err(e)?;
            let part = std::path::PathBuf::from(format!("{}.update", target.display()));
            crate::zips::extract_entry(&mut archive, *i, &part)?;
            if rel.eq_ignore_ascii_case("NativeMonsLauncher.exe") && target.is_file() {
                // A running program can be renamed but not overwritten. The old one goes at the next start.
                let old = std::path::PathBuf::from(format!("{}.old", target.display()));
                if old.is_file() { std::fs::remove_file(&old).map_err(e)?; }
                std::fs::rename(&target, &old).map_err(e)?;
            } else if target.is_file() {
                std::fs::remove_file(&target).map_err(e)?;
            }
            std::fs::rename(&part, &target).map_err(e)?;
            changed += 1;
        }
        // Gen1Recomp's code is replaced exactly, so a file upstream removed can't linger.
        let code = here.join("engines").join("gen1recomp").join("src");
        if code.is_dir() {
            for f in crate::library::files_under(&code.to_string_lossy()) {
                let rel = Path::new(&f).strip_prefix(&here).map(|p| p.to_string_lossy().replace('/', "\\")).unwrap_or_default();
                if !names.contains(&rel.to_lowercase()) { std::fs::remove_file(&f).map_err(e)?; }
            }
        }
        // A package without this stamp would look out of date for ever.
        std::fs::write(here.join("version.txt"), format!("channel={}\nbuild={}\nrevision={}\nversion={}\n", release.channel.as_deref().unwrap_or(""), release.build, release.revision.as_deref().unwrap_or(""), release.version.as_deref().unwrap_or(""))).map_err(e)?;
        report(&format!("{} files updated", changed), 1.0);
        Ok(None)
    })();
    if work.is_dir() { let _ = std::fs::remove_dir_all(&work); }
    match r {
        Ok(x) => x,
        Err(x) => Some(format!("The update stopped partway: {} Your saves and settings weren't touched. Run it again to finish.", x)),
    }
}
