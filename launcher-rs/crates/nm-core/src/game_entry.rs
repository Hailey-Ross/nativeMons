//! Theme (a version's colors), RomFile (one dump) and GameEntry (one game: ROMs, engines, ids, labels).

use crate::config;
use std::collections::HashMap;

/// A version's colors from games.json, as "#RRGGBB" strings plus the finish (drawing lives in nm-ui).
#[derive(Clone, Debug, Default)]
pub struct Theme {
    pub main: String,
    pub deep: String,
    pub light: String,
    pub finish: String,
    pub game_id: Option<String>,
}

impl Theme {
    pub fn make(main: &str, deep: &str, light: &str, finish: Option<&str>) -> Theme {
        Theme { main: main.into(), deep: deep.into(), light: light.into(), finish: finish.unwrap_or("plain").into(), game_id: None }
    }
    pub fn default_theme() -> Theme { Theme::make("#D6302B", "#8E1712", "#F2665C", Some("plain")) }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RomFile {
    pub file: String,
    pub sha1: String,
    pub revision: i32,
    /// File creation time, UTC, in .NET ticks (DateTime.Ticks).
    pub added: i64,
    pub gen1_ok: bool,
    pub nativeplat_ok: bool,
    pub encrypted: bool,
    pub engine_copy: bool,
}

impl RomFile {
    pub fn verified(&self) -> bool { self.gen1_ok || self.nativeplat_ok }
    pub fn label(&self) -> String { format!("Rev {}", self.revision) }
}

#[derive(Clone, Debug, Default)]
pub struct GameEntry {
    pub theme: Theme,
    pub dex: i32,
    pub id: String,
    pub title: String,
    pub console: Option<String>,
    pub gen1: Option<String>,
    pub fulldex_code: Option<String>,
    pub header: Option<String>,
    pub save: Option<String>,
    pub port: Option<String>,
    pub nativeplat: Option<String>,
    pub town_core: Option<String>,
    pub pc: Option<String>,
    pub ctr: Option<String>,
    pub ctr_rom: Option<String>,
    pub ctr_plain_rom: Option<String>,
    pub ctr_encrypted: bool,
    pub art_frame: i32,
    pub town_frame: i32,
    /// games.json townView: the town picture's point to center, and its zoom (GameArt.TownViews).
    pub town_view: Option<Vec<f32>>,
    pub gen1_rom: Option<String>,
    pub fulldex_rom: Option<String>,
    pub nativeplat_rom: Option<String>,
    pub found_unsupported: Option<String>,
    pub found_sha1: Option<String>,
    pub roms: Vec<RomFile>,
    /// Index into `roms` of the chosen dump.
    pub chosen: Option<usize>,
    pub imported: bool,
    pub base_id: Option<String>,
    /// Known patched builds by SHA-1 (lower-cased key, compared ignoring case): [id, title].
    pub known_hacks: HashMap<String, [String; 2]>,
}

impl GameEntry {
    pub fn can_verify(&self) -> bool { self.gen1.is_some() || self.nativeplat.is_some() }
    pub fn is_hack(&self) -> bool { self.base_id.is_some() }
    pub fn is_ds(&self) -> bool { self.console.as_deref() == Some("Nintendo DS") }
    pub fn chosen_rom(&self) -> Option<&RomFile> { self.chosen.map(|i| &self.roms[i]) }
    pub fn known_hack(&self, sha1: &str) -> Option<&[String; 2]> { self.known_hacks.get(&sha1.to_lowercase()) }
    /// Two dumps can share a revision, such as a clean one and a hack. They must not read the same.
    pub fn is_modified(&self, r: &RomFile) -> bool { !r.verified() && self.roms.iter().any(|x| x != r && x.verified() && x.revision == r.revision) }
    pub fn on_gen1(&self) -> bool { self.gen1.is_some() && (self.imported || self.gen1_rom.is_some()) }
    pub fn on_fulldex(&self) -> bool { self.fulldex_code.is_some() && self.fulldex_rom.is_some() && config::fulldex().exists() }
    pub fn on_port(&self) -> bool { self.port.is_some() && self.fulldex_rom.is_some() && config::platinum_port().exists() }
    pub fn on_nativeplat(&self) -> bool { self.nativeplat.is_some() && self.nativeplat_rom.is_some() && config::nativeplat().exists() }
    pub fn on_pc(&self) -> bool { self.pc.is_some() && crate::pc_games::installed(self) }
    pub fn on_recomp(&self) -> bool { self.ctr_plain_rom.is_some() && crate::recomp::installed() && crate::recomp::supports(self) }
    pub fn on_3ds(&self) -> bool { self.ctr.is_some() && self.ctr_rom.is_some() && crate::emu3ds::enabled() && crate::emu3ds::for_game(Some(self)).is_some() }
    pub fn playable(&self) -> bool { self.on_gen1() || self.on_fulldex() || self.on_port() || self.on_nativeplat() || self.on_pc() || self.on_recomp() || self.on_3ds() }
    /// 3DS games stay out of the other save tools (export, import), which don't know their saves yet.
    pub fn save_tools(&self) -> bool { self.playable() && self.pc.is_none() && self.ctr.is_none() }
    pub fn has_both(&self) -> bool { self.on_gen1() && self.on_fulldex() }
    /// The player's choice where both engines can run it; Fulldex by default (widescreen).
    pub fn engines(&self) -> Vec<&'static str> {
        let mut l = Vec::new();
        if self.on_fulldex() { l.push("fulldex"); }
        if self.on_gen1() { l.push("gen1"); }
        if self.on_port() { l.push("port"); }
        if self.on_nativeplat() { l.push("nativeplat"); }
        if self.on_pc() { l.push("pc"); }
        if self.on_recomp() { l.push("recomp"); }
        if self.on_3ds() { l.push("3ds"); }
        // A romhack only runs on Fulldex: the other engines only take clean copies.
        if let Some(c) = self.chosen_rom() { if self.is_modified(c) && l.contains(&"fulldex") { return vec!["fulldex"]; } }
        l
    }
    pub fn engine(&self) -> String {
        let l = self.engines();
        // A 3DS game never falls back to Gen1Recomp, even with no emulator installed.
        if l.is_empty() { return if self.ctr.is_some() { "3ds".into() } else { "gen1".into() }; }
        let chosen = config::read(&format!("engine-{}.txt", self.id), l[0]);
        if l.contains(&chosen.as_str()) { chosen } else { l[0].to_string() }
    }
}
