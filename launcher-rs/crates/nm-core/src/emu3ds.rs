//! Emu3ds: the 3DS emulators the launcher offers (Azahar, AzaharPlus) and Zakuro. So far what the library needs:
//! which are installed, the player's pick, and which one plays a game. Install and remove come with their modes.

use crate::config;
use crate::game_entry::GameEntry;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emu3ds {
    pub id: &'static str,
    pub name: &'static str,
    pub short: &'static str,
    pub exe: &'static str,
    pub native: bool,
    /// The pinned release of our fork: repo, asset, tag and the zip's sha256. None: not offered yet.
    pub repo: Option<&'static str>,
    pub asset: Option<&'static str>,
    pub tag: Option<&'static str>,
    pub sha256: Option<&'static str>,
}

pub const ZAKURO: Emu3ds = Emu3ds { id: "zakuro", name: "3DSrecomp+Zakuro (native, recommended)", short: "3DSrecomp+Zakuro", exe: "zakuro.exe", native: true, repo: None, asset: None, tag: None, sha256: None };
pub const ALL: [Emu3ds; 2] = [
    Emu3ds { id: "azaharplus", name: "AzaharPlus-modified (nativeMons)", short: "AzaharPlus", exe: "azahar.exe", native: false, repo: Some("Hailey-Ross/AzaharPlus"), asset: Some("azaharplus-nativemons-windows-x64.zip"), tag: Some("windows-26d3e5994"), sha256: Some("3482a402a5847cedd70c053b3e0bfe7d4f3fff27cfaef8d46c51468df01c754e") },
    Emu3ds { id: "azahar", name: "Azahar-modified (nativeMons)", short: "Azahar", exe: "azahar.exe", native: false, repo: Some("Hailey-Ross/azahar"), asset: Some("azahar-nativemons-windows-x64.zip"), tag: Some("windows-15891deba"), sha256: Some("1ad8a4efa932e51692a734f456450f3f2c11a342580bf963a12e5914671326b4") },
];

impl Emu3ds {
    pub fn dir(&self) -> PathBuf { if self.native { config::zakuro().parent().map(|p| p.to_path_buf()).unwrap_or_default() } else { config::emulators().join(self.id) } }
    pub fn exe_path(&self) -> PathBuf { self.dir().join(self.exe) }
    pub fn installed(&self) -> bool { self.exe_path().is_file() }
    pub fn url(&self) -> String { format!("https://github.com/{}/releases/download/{}/{}", self.repo.unwrap_or(""), self.tag.unwrap_or(""), self.asset.unwrap_or("")) }
    pub fn installed_tag(&self) -> Option<String> { crate::save_sets::read_trim(&self.dir().join("nativemons-tag.txt")) }
}

pub fn can_download(e: &Emu3ds) -> bool { e.tag.is_some() && e.sha256.is_some() }
/// An emulator's own data folder ("user" beside it in portable mode) survives updates and removal.
const KEPT: [&str; 1] = ["user"];
fn kept_dir(e: &Emu3ds, k: &str) -> PathBuf { config::emulators().join(format!("{}-{}", e.id, k.replace(' ', "-").to_lowercase())) }
fn sibling(e: &Emu3ds, suffix: &str) -> PathBuf { PathBuf::from(format!("{}{}", e.dir().display(), suffix)) }
/// Download, check, unpack beside, then swap in. None when done, or why not.
pub fn install(e: &Emu3ds) -> Option<String> {
    let (zip, temp, old) = (sibling(e, ".download.zip"), sibling(e, ".new"), sibling(e, ".old"));
    let r = (|| -> Result<Option<String>, String> {
        let io = |x: std::io::Error| x.to_string();
        // The link to its data goes first, so nothing below can reach the saves through it.
        crate::ctr3ds_saves::unlink(e).map_err(io)?;
        std::fs::create_dir_all(config::emulators()).map_err(io)?;
        crate::net::download_file(&e.url(), &zip, None, &mut |_| {})?;
        let hash = crate::save_guard::hash_of(&zip).to_lowercase();
        if hash != crate::test_hooks::sha(e.sha256.unwrap_or("")) { return Ok(Some("The download wasn't the expected file, so it wasn't used.".into())); }
        if temp.is_dir() { std::fs::remove_dir_all(&temp).map_err(io)?; }
        crate::zips::extract_all(&zip.to_string_lossy(), &temp)?;
        // The zip holds one folder (azahar\) with the program inside.
        let entries: Vec<PathBuf> = std::fs::read_dir(&temp).map_err(io)?.flatten().map(|d| d.path()).collect();
        let dirs: Vec<&PathBuf> = entries.iter().filter(|p| p.is_dir()).collect();
        let top = if dirs.len() == 1 && entries.len() == 1 { dirs[0].clone() } else { temp.clone() };
        if !top.join(e.exe).is_file() { return Ok(Some(format!("The download was missing {}.", e.exe))); }
        std::fs::write(top.join("nativemons-tag.txt"), e.tag.unwrap_or("")).map_err(io)?;
        if old.is_dir() { std::fs::remove_dir_all(&old).map_err(io)?; }
        if e.dir().is_dir() {
            std::fs::rename(e.dir(), &old).map_err(io)?;
            for k in KEPT { if old.join(k).is_dir() { std::fs::rename(old.join(k), top.join(k)).map_err(io)?; } }
        }
        std::fs::rename(&top, e.dir()).map_err(io)?;
        if old.is_dir() { std::fs::remove_dir_all(&old).map_err(io)?; }
        // Data kept from an earlier Remove comes back.
        for k in KEPT { if kept_dir(e, k).is_dir() && !e.dir().join(k).is_dir() { std::fs::rename(kept_dir(e, k), e.dir().join(k)).map_err(io)?; } }
        Ok(None)
    })();
    let _ = std::fs::remove_file(&zip);
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    match r { Ok(x) => x, Err(x) => Some(format!("{} couldn't be installed: {}", e.short, x)) }
}
/// Deletes the program; its own data folder is kept, beside the others, for a later install.
pub fn remove(e: &Emu3ds) -> Option<String> {
    let r = (|| -> std::io::Result<()> {
        if !e.dir().is_dir() { return Ok(()); }
        crate::ctr3ds_saves::unlink(e)?;
        for k in KEPT {
            let (from, to) = (e.dir().join(k), kept_dir(e, k));
            if from.is_dir() && !to.is_dir() { std::fs::rename(&from, &to)?; }
        }
        std::fs::remove_dir_all(e.dir())?;
        // A removed emulator stops being the pick, so the next install becomes it.
        if config::read("3ds-emulator.txt", "") == e.id { config::write("3ds-emulator.txt", ""); }
        Ok(())
    })();
    r.err().map(|x| x.to_string())
}

/// Only this emulator's own program blocks replacing its folder; other games and background jobs don't.
pub fn running(e: &Emu3ds) -> bool {
    let name = e.exe.trim_end_matches(".exe");
    let dir = format!("{}{}", e.dir().display(), std::path::MAIN_SEPARATOR).to_lowercase();
    crate::proc::by_name(&crate::proc::processes(), name).iter().any(|p| p.exe.as_ref().map_or(true, |x| x.to_lowercase().starts_with(&dir)))
}

pub fn enabled() -> bool { config::read("3ds-games.txt", "off") == "on" }
pub fn default() -> Option<Emu3ds> {
    let id = config::read("3ds-emulator.txt", "");
    ALL.iter().copied().find(|e| e.id == id && e.installed()).or_else(|| ALL.iter().copied().find(|e| e.installed()))
}
pub fn plus() -> Emu3ds { ALL[0] }
/// An encrypted dump goes to AzaharPlus, whichever is picked: Azahar can't open it.
pub fn for_game(g: Option<&GameEntry>) -> Option<Emu3ds> {
    let d = default();
    match g {
        Some(g) if g.ctr_encrypted && d != Some(plus()) => if plus().installed() { Some(plus()) } else { None },
        _ => d,
    }
}

#[cfg(test)]
mod tests {
    /// The engine sync bot moves the C#'s pins: the Rust launcher must offer the same builds.
    #[test]
    fn pins_match_the_csharp_launcher() {
        let cs = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../launcher/NativeMonsLauncher.cs")).unwrap();
        for e in super::ALL.iter() {
            let at = cs.find(&format!("Id = \"{}\"", e.id)).unwrap();
            let line = &cs[at..at + cs[at..].find("Notes =").unwrap()];
            assert!(line.contains(&format!("Tag = \"{}\"", e.tag.unwrap())), "{}: tag differs from the C#", e.id);
            assert!(line.contains(&format!("Sha256 = \"{}\"", e.sha256.unwrap())), "{}: sha256 differs from the C#", e.id);
        }
    }
}
