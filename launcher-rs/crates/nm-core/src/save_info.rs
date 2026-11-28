//! SaveInfo: what a save holds (trainer, time played, badges, caught, party), from mon-bridge's summary or a
//! Gen1Recomp slot file (FromSlot). Never changes the file.

use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct SaveInfo {
    pub trainer: String,
    pub party: String,
    pub hours: i32,
    pub minutes: i32,
    pub badges: i32,
    pub caught: i32,
}

impl Default for SaveInfo {
    fn default() -> Self { SaveInfo { trainer: String::new(), party: String::new(), hours: 0, minutes: 0, badges: -1, caught: -1 } }
}

fn title(name: &str) -> String {
    name.replace('_', " ").to_lowercase().split(' ').map(|p| { let mut c = p.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }).collect::<Vec<_>>().join(" ")
}
fn unquote(s: &str) -> String { s.trim_end_matches(',').trim().trim_matches('"').to_string() }
fn lead_int(s: &str) -> Option<i32> { s.trim_end_matches(',').parse().ok() }

impl SaveInfo {
    pub fn played(&self) -> String { format!("{}h {}m", self.hours, self.minutes) }
    pub fn tile_line(&self) -> String { format!("{}{}", if self.trainer.is_empty() { String::new() } else { format!("{}, ", self.trainer) }, self.played()) }
    /// Details(saved): `saved` is the file's local write time.
    pub fn details(&self, saved: chrono::NaiveDateTime) -> String {
        let mut first = Vec::new();
        if !self.trainer.is_empty() { first.push(format!("Trainer {}", self.trainer)); }
        first.push(format!("{} played", self.played()));
        if self.badges >= 0 { first.push(format!("{}{}", self.badges, if self.badges == 1 { " badge" } else { " badges" })); }
        if self.caught >= 0 { first.push(format!("{} caught", self.caught)); }
        format!("{}\nParty: {}\nLast saved {}", first.join(", "), if self.party.is_empty() { "empty" } else { &self.party }, saved.format("%-d %b %Y, %H:%M"))
    }

    /// FromSlot: slot files are plain nested tables, two spaces per level, so a line's indent says where it belongs.
    pub fn from_slot(file: &Path) -> std::io::Result<SaveInfo> {
        // File.ReadAllText: a slot can hold bytes that aren't UTF-8 (raw strings); they read as U+FFFD, not a failure.
        let bytes = std::fs::read(file)?;
        let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF".as_slice()).unwrap_or(&bytes)).into_owned();
        let mut info = SaveInfo::default();
        let (mut block, mut inner): (Option<String>, Option<String>) = (None, None);
        let (mut kanto, mut johto, mut saw_badges) = (0, 0, false);
        let mut level: Option<String> = None;
        let mut party = Vec::new();
        for raw in text.lines() {
            let line = raw.trim_end();
            let indent = (line.len() - line.trim_start().len()) as i32;
            let text = line.trim();
            let opens = |t: &str| if t.ends_with("= {") { Some(t[..t.len() - 3].trim().to_string()) } else { None };
            if indent == 2 {
                inner = None;
                block = opens(text);
                if let Some(cut) = text.find(" = ") {
                    if cut > 0 && &text[..cut] == "playTime" && block.is_none() {
                        if let Ok(seconds) = text[cut + 3..].trim_end_matches(',').parse::<f64>() { info.hours = (seconds / 3600.0) as i32; info.minutes = (seconds / 60.0) as i32 % 60; }
                    }
                }
                continue;
            }
            let Some(b) = block.as_deref() else { continue };
            if indent == 4 {
                inner = opens(text);
                if b == "player" && text.starts_with("name = ") { info.trainer = unquote(&text[7..]); }
                if b == "player" && (text.starts_with("badges = {") || text.starts_with("kantoBadges = {")) { saw_badges = true; }
                if b == "playTime" && text.starts_with("hours = ") { info.hours = lead_int(&text[8..]).unwrap_or(0); }
                if b == "playTime" && text.starts_with("minutes = ") { info.minutes = lead_int(&text[10..]).unwrap_or(0); }
                continue;
            }
            if indent != 6 { continue; }
            let inn = inner.as_deref();
            if b == "player" && inn == Some("badges") && text.contains("= true") { johto += 1; }
            if b == "player" && inn == Some("kantoBadges") && text.contains("= true") { kanto += 1; }
            if b == "pokedex" && (inn == Some("caught") || inn == Some("owned")) && text.contains("= true") { info.caught = info.caught.max(0) + 1; }
            if b == "party" {
                if let Some(l) = text.strip_prefix("level = ") { level = Some(l.trim_end_matches(',').to_string()); }
                if let Some(sp) = text.strip_prefix("species = ") { party.push(format!("{}{}", title(&unquote(sp)), level.as_ref().map(|l| format!(" {}", l)).unwrap_or_default())); }
            }
        }
        if saw_badges { info.badges = johto + kanto; }
        if info.caught < 0 { info.caught = 0; }
        info.party = party.join(", ");
        Ok(info)
    }
}

/// ReadSaveInfo: None when it can't be read, or nothing has been saved in it yet.
pub fn read_save_info(file: &Path) -> Option<SaveInfo> {
    if !file.is_file() { return None; }
    if file.extension().map_or(false, |e| e.eq_ignore_ascii_case("lua")) { return SaveInfo::from_slot(file).ok(); }
    if crate::save_guard::is_blank(file) { return None; }
    let (o, code) = crate::save_guard::bridge(&format!("summary \"{}\"", file.display()));
    let f: Vec<&str> = o.trim_end_matches(['\r', '\n']).split('\t').collect();
    if code != 0 || f.len() < 6 { return None; }
    Some(SaveInfo { trainer: f[0].into(), hours: f[1].parse().ok()?, minutes: f[2].parse().ok()?, badges: f[3].parse().ok()?, caught: f[4].parse().ok()?, party: f[5].into() })
}

/// BackupWhen: a backup's time from its name (yyyyMMdd-HHmmss...), else the name.
pub fn backup_when(file: &str) -> String {
    let n = Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if n.len() >= 15 && n.is_char_boundary(15) {
        if let Ok(t) = chrono::NaiveDateTime::parse_from_str(&n[..15], "%Y%m%d-%H%M%S") { return t.format("%-d %b %Y, %H:%M").to_string(); }
    }
    n
}

/// File.GetLastWriteTime, local.
pub fn write_time(file: &Path) -> chrono::NaiveDateTime {
    let m = std::fs::metadata(file).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    chrono::DateTime::<chrono::Local>::from(m).naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn from_slot_reads_a_slot() {
        let d = std::env::temp_dir().join(format!("nm-saveinfo-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("slot1.lua");
        std::fs::write(&f, "return {\n  player = {\n    name = \"RED\",\n    badges = {\n      [1] = true,\n      [2] = false,\n    },\n    kantoBadges = {\n      [1] = true,\n    },\n  },\n  playTime = 7384.5,\n  pokedex = {\n    caught = {\n      [1] = true,\n      [4] = true,\n    },\n  },\n  party = {\n    {\n      level = 12,\n      species = \"MR_MIME\",\n    },\n  },\n}\n").unwrap();
        let i = SaveInfo::from_slot(&f).unwrap();
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!((i.trainer.as_str(), i.hours, i.minutes, i.badges, i.caught), ("RED", 2, 3, 2, 2));
        assert_eq!(i.party, "Mr Mime 12");
        assert_eq!(i.tile_line(), "RED, 2h 3m");
        assert_eq!(backup_when("b/20261124-235901-played.sav"), "24 Nov 2026, 23:59");
        assert_eq!(backup_when("odd.sav"), "odd.sav");
    }
}
