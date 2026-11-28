//! Library: games.json, the ROM folders scan, the SHA-1 cache, romhacks and the chosen dump per game.

use crate::config::{self, lines};
use crate::custom_games;
use crate::emu3ds;
use crate::game_entry::{GameEntry, RomFile, Theme};
use regex::Regex;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Library {
    pub games: Vec<GameEntry>,
    pub note: String,
    pub gen1_found: bool,
    hashes: Vec<(String, String)>,
    hash_index: HashMap<String, usize>,
}

/// DateTime.Ticks of a SystemTime: 100 ns steps since 0001-01-01 (UTC).
fn ticks(t: std::io::Result<std::time::SystemTime>) -> i64 {
    const EPOCH: i64 = 621_355_968_000_000_000;
    match t {
        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => EPOCH + (d.as_nanos() / 100) as i64,
            Err(e) => EPOCH - (e.duration().as_nanos() / 100) as i64,
        },
        Err(_) => 0,
    }
}
pub fn last_write_ticks(f: &Path) -> i64 { ticks(std::fs::metadata(f).and_then(|m| m.modified())) }
pub fn creation_ticks(f: &Path) -> i64 { ticks(std::fs::metadata(f).and_then(|m| m.created())) }

pub fn file_name(p: &str) -> String { Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }
fn file_stem(p: &str) -> String { Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }
/// Path.GetExtension, lower-cased: "" when none.
pub fn ext_lower(p: &str) -> String { Path::new(p).extension().map(|s| format!(".{}", s.to_string_lossy().to_lowercase())).unwrap_or_default() }

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex { cell.get_or_init(|| Regex::new(pat).unwrap()) }

impl Library {
    /// Library.Scan: the games and every ROM found for them. `gen1_cli` asks Gen1Recomp ("games": its accepted dumps).
    pub fn scan(gen1_cli: &dyn Fn(&str) -> Option<String>) -> Result<Library, String> {
        let mut lib = Library { games: Vec::new(), note: String::new(), gen1_found: false, hashes: Vec::new(), hash_index: HashMap::new() };
        let text = std::fs::read_to_string(config::games_file()).map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
        let mut list: Vec<GameEntry> = Vec::new();
        let s = |d: &serde_json::Value, k: &str| d.get(k).and_then(|v| v.as_str()).map(String::from);
        let num = |v: &serde_json::Value| v.as_f64().map(|f| f.round() as i32).or_else(|| v.as_str().and_then(|t| t.parse().ok())).unwrap_or(0);
        for d in json.as_array().cloned().unwrap_or_default() {
            let mut e = GameEntry {
                id: s(&d, "id").unwrap_or_default(),
                title: s(&d, "title").unwrap_or_default(),
                console: s(&d, "console"),
                gen1: s(&d, "gen1"),
                fulldex_code: s(&d, "fulldex"),
                header: s(&d, "header"),
                save: s(&d, "save"),
                port: s(&d, "port"),
                nativeplat: s(&d, "nativeplat"),
                town_core: s(&d, "townCore"),
                pc: s(&d, "pc"),
                ctr: s(&d, "ctr"),
                theme: Theme::default_theme(),
                ..Default::default()
            };
            if let Some(v) = d.get("dex") { e.dex = num(v); }
            if let Some(v) = d.get("artFrame") { e.art_frame = num(v); }
            if let Some(v) = d.get("townFrame") { e.town_frame = num(v); }
            if let Some(v) = d.get("townView").and_then(|v| v.as_array()) { e.town_view = Some(v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()); }
            if let Some(hk) = d.get("hacks").and_then(|v| v.as_array()) {
                for k in hk {
                    if let (Some(id), Some(title), Some(shas)) = (s(k, "id"), s(k, "title"), k.get("sha1").and_then(|v| v.as_array())) {
                        for sh in shas.iter().filter_map(|v| v.as_str()) { e.known_hacks.insert(sh.to_lowercase(), [id.clone(), title.clone()]); }
                    }
                }
            }
            if let Some(c) = d.get("colors").and_then(|v| v.as_array()) {
                let hex: Vec<&str> = c.iter().filter_map(|v| v.as_str()).collect();
                if hex.len() == 3 {
                    e.theme = Theme::make(hex[0], hex[1], hex[2], s(&d, "finish").as_deref());
                    e.theme.game_id = Some(e.id.clone());
                }
            }
            list.push(e);
        }
        // PC games the player added, after the built-in ones.
        let mut added = 0;
        for c in custom_games::all() {
            if !list.iter().any(|x| Some(&x.id) == c.id.as_ref()) {
                list.push(custom_games::to_game(&c, added));
                added += 1;
            }
        }
        // version -> accepted hashes and whether it is imported, from the engine itself.
        let mut accepted: HashMap<String, String> = HashMap::new();
        let games = gen1_cli("games");
        lib.gen1_found = games.is_some();
        match games {
            None => lib.note = "Gen1Recomp was not found, so its games are unavailable.".into(),
            Some(text) => {
                for row in text.split('\n') {
                    let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
                    if f.len() < 4 { continue; }
                    if let Some(g) = list.iter_mut().find(|x| x.gen1.as_deref() == Some(f[0])) { g.imported = f[2] == "yes"; }
                    for hash in f[3].split(',') { accepted.insert(hash.to_string(), f[0].to_string()); }
                }
            }
        }
        lib.load_hashes();
        let mut folders = config::rom_folders();
        folders.push(config::fulldex_dir().join("roms").to_string_lossy().into_owned());
        folders.push(config::unzipped().to_string_lossy().into_owned());
        let mut files: Vec<String> = config::rom_files().into_iter().filter(|f| Path::new(f).is_file()).collect();
        for folder in &folders {
            if Path::new(folder).is_dir() { files.extend(files_under(folder)); }
        }
        for file in &files {
            if is_3ds(file) {
                let _ = sort_3ds(&mut list, file);
                continue;
            }
            if !is_rom(file) {
                if is_ultra_violet_patch(file) { let _ = lib.add_patch(&mut list, file); }
                continue;
            }
            let _ = lib.sort(&mut list, file, &ext_lower(file), &accepted);
        }
        lib.save_hashes();
        let mut list = hacks(list, lib.gen1_found);
        for g in list.iter_mut() { choose_rom(g); }
        lib.games = list;
        Ok(lib)
    }

    fn load_hashes(&mut self) {
        let Ok(t) = std::fs::read_to_string(hash_file()) else { return };
        for l in lines(&t) {
            if let Some(cut) = l.rfind('|') {
                if cut > 0 { self.put_hash(l[..cut].to_string(), l[cut + 1..].to_string()); }
            }
        }
    }
    fn put_hash(&mut self, k: String, v: String) {
        match self.hash_index.get(&k) {
            Some(&i) => self.hashes[i].1 = v,
            None => {
                self.hash_index.insert(k.clone(), self.hashes.len());
                self.hashes.push((k, v));
            }
        }
    }
    fn save_hashes(&self) {
        let _ = std::fs::create_dir_all(config::state());
        let ls: Vec<String> = self.hashes.iter().map(|(k, v)| format!("{}|{}", k, v)).collect();
        let _ = config::write_all_lines(&hash_file(), &ls);
    }
    /// Reading every dump at each start would be slow, so a hash is kept until the file changes.
    pub fn sha1_cached(&mut self, file: &str) -> std::io::Result<String> {
        let len = std::fs::metadata(file)?.len();
        let key = format!("{}|{}|{}", file, len, last_write_ticks(Path::new(file)));
        if let Some(&i) = self.hash_index.get(&key) { return Ok(self.hashes[i].1.clone()); }
        let sha = sha1_of(file)?;
        self.put_hash(key, sha.clone());
        Ok(sha)
    }

    /// Its .ips plays over FireRed 1.0, so it is listed with FireRed's dumps until Hacks splits it off.
    fn add_patch(&mut self, list: &mut [GameEntry], file: &str) -> std::io::Result<()> {
        let Some(gi) = list.iter().position(|x| x.header.as_deref() == Some("BPRE")) else { return Ok(()) };
        let sha = self.sha1_cached(file)?;
        let g = &mut list[gi];
        if g.roms.iter().any(|x| x.sha1 == sha) { return Ok(()); }
        g.roms.push(RomFile { file: file.into(), sha1: sha, added: creation_ticks(Path::new(file)), ..Default::default() });
        Ok(())
    }

    fn sort(&mut self, list: &mut [GameEntry], file: &str, ext: &str, accepted: &HashMap<String, String>) -> std::io::Result<()> {
        let header = header_of(file, ext)?;
        let gb = ext == ".gb" || ext == ".gbc";
        let Some(gi) = list.iter().position(|x| match &x.header { Some(h) => if gb { header.starts_with(h.as_str()) && x.gen1.is_some() } else { &header == h }, None => false }) else { return Ok(()) };
        let sha = self.sha1_cached(file)?;
        let engine_roms = format!("{}{}", config::fulldex_dir().join("roms").to_string_lossy(), std::path::MAIN_SEPARATOR);
        let g = &mut list[gi];
        let mut rom = RomFile {
            file: file.into(),
            sha1: sha.clone(),
            revision: revision_of(file, ext)?,
            added: creation_ticks(Path::new(file)),
            encrypted: needs_patch(Some(file)),
            engine_copy: file.to_lowercase().starts_with(&engine_roms.to_lowercase()),
            ..Default::default()
        };
        rom.gen1_ok = g.gen1.is_some() && accepted.get(&sha) == g.gen1.as_ref();
        rom.nativeplat_ok = g.nativeplat.as_deref().and_then(nativeplat_dumps).is_some_and(|w| w.contains(&sha.as_str()));
        // The same contents twice is one dump: the player's own file, not a copy kept in an engine's folder.
        match g.roms.iter().position(|x| x.sha1 == sha) {
            None => g.roms.push(rom),
            Some(i) => {
                if g.roms[i].engine_copy && !rom.engine_copy { g.roms[i] = rom; }
            }
        }
        Ok(())
    }
}

fn hash_file() -> PathBuf { config::state().join("rom-hashes.txt") }

pub fn sha1_of(file: &str) -> std::io::Result<String> {
    let mut f = std::fs::File::open(file)?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{:02x}", b)).collect())
}

/// Encoding.ASCII.GetString: bytes over 127 read as '?'.
fn ascii(b: &[u8]) -> String { b.iter().map(|&c| if c < 128 { c as char } else { '?' }).collect() }

fn header_of(file: &str, ext: &str) -> std::io::Result<String> {
    let mut f = std::fs::File::open(file)?;
    let (offset, count) = match ext { ".nds" => (0x0C, 4), ".gba" => (0xAC, 4), _ => (0x134, 16) };
    if (f.metadata()?.len() as usize) < offset + count { return Ok(String::new()); }
    let mut b = vec![0u8; count];
    f.seek(SeekFrom::Start(offset as u64))?;
    let n = f.read(&mut b)?;
    b.truncate(n.max(0));
    b.resize(count, 0);
    Ok(ascii(&b).trim_end_matches(['\0', ' ']).to_string())
}

fn revision_of(file: &str, ext: &str) -> std::io::Result<i32> {
    let at = match ext { ".nds" => 0x1E, ".gba" => 0xBC, _ => 0x14C };
    let mut f = std::fs::File::open(file)?;
    if f.metadata()?.len() <= at { return Ok(0); }
    f.seek(SeekFrom::Start(at))?;
    let mut b = [0u8; 1];
    Ok(if f.read(&mut b)? == 1 { b[0] as i32 } else { -1 })
}

/// nativeplat takes Platinum Rev 0 too.
fn nativeplat_dumps(v: &str) -> Option<&'static [&'static str]> {
    match v {
        "diamond" => Some(&["a46233d8b79a69ea87aa295a0efad5237d02841e"]),
        "pearl" => Some(&["99083bf15ec7c6b81b4ba241ee10abd9e80999ac"]),
        "platinum" => Some(&["0862ec35b24de5c7e2dcb88c9eea0873110d755c", "ce81046eda7d232513069519cb2085349896dec7"]),
        _ => None,
    }
}

/// Whether an .nds ROM still has its secure area encrypted, which stops it loading without a DS BIOS.
pub fn needs_patch(file: Option<&str>) -> bool {
    let Some(file) = file else { return false };
    if !file.to_lowercase().ends_with(".nds") { return false; }
    let r = (|| -> std::io::Result<bool> {
        let mut f = std::fs::File::open(file)?;
        let mut head = [0u8; 0x24];
        if f.read(&mut head)? < head.len() { return Ok(false); }
        let off = u32::from_le_bytes(head[0x20..0x24].try_into().unwrap());
        if !(0x4000..0x8000).contains(&off) { return Ok(false); }
        let mut mark = [0u8; 8];
        f.seek(SeekFrom::Start(off as u64))?;
        let _ = f.read(&mut mark)?;
        Ok(mark != [0xff, 0xde, 0xff, 0xe7, 0xff, 0xde, 0xff, 0xe7])
    })();
    r.unwrap_or(false)
}

pub fn is_rom(file: &str) -> bool { matches!(ext_lower(file).as_str(), ".nds" | ".gb" | ".gbc" | ".gba" | ".3ds" | ".cci") }
pub fn is_3ds(file: &str) -> bool { matches!(ext_lower(file).as_str(), ".3ds" | ".cci") }
/// Ultra Violet (a FireRed 1.0 hack), told by its file's name as Fulldex does.
pub fn is_ultra_violet(file: &str) -> bool { file_stem(file).to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect::<String>().contains("ultraviolet") }
pub fn is_ultra_violet_patch(file: &str) -> bool { ext_lower(file) == ".ips" && is_ultra_violet(file) }

/// One unreadable or over-long subfolder is skipped alone, instead of losing the whole folder.
pub fn files_under(root: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut left = vec![PathBuf::from(root)];
    while let Some(folder) = left.pop() {
        let Ok(rd) = std::fs::read_dir(&folder) else { continue };
        let entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
        for e in &entries {
            if e.file_type().map(|t| t.is_file()).unwrap_or(false) { found.push(e.path().to_string_lossy().into_owned()); }
        }
        for e in &entries {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) { left.push(e.path()); }
        }
    }
    found
}

pub struct Ctr3ds {
    pub title_id: String,
    pub decrypted: bool,
    pub no_crypto_flag: bool,
}

/// A 3DS cartridge dump (NCSD): its title ID and whether it is decrypted, from the headers alone (never hashed).
pub fn read_ctr(file: &str) -> std::io::Result<Option<Ctr3ds>> {
    let mut f = std::fs::File::open(file)?;
    let mut h = vec![0u8; 0x200];
    if read_full(&mut f, &mut h)? < h.len() || ascii(&h[0x100..0x104]) != "NCSD" { return Ok(None); }
    let mut r = Ctr3ds { title_id: format!("{:016X}", u64::from_le_bytes(h[0x108..0x110].try_into().unwrap())), decrypted: false, no_crypto_flag: false };
    let part = u32::from_le_bytes(h[0x120..0x124].try_into().unwrap()) as u64 * 0x200;
    let mut n = vec![0u8; 0x200];
    f.seek(SeekFrom::Start(part))?;
    if read_full(&mut f, &mut n)? < n.len() || ascii(&n[0x100..0x104]) != "NCCH" { return Ok(Some(r)); }
    r.decrypted = n[0x18F] & 0x04 != 0;
    r.no_crypto_flag = r.decrypted;
    // Some decrypted dumps leave the flag unset: an ExeFS whose first file reads ".code" is plain text.
    if !r.decrypted {
        let mut e = [0u8; 8];
        f.seek(SeekFrom::Start(part + u32::from_le_bytes(n[0x1A0..0x1A4].try_into().unwrap()) as u64 * 0x200))?;
        r.decrypted = read_full(&mut f, &mut e)? == 8 && ascii(&e[..5]) == ".code";
    }
    Ok(Some(r))
}
fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let k = f.read(&mut buf[n..])?;
        if k == 0 { break; }
        n += k;
    }
    Ok(n)
}

fn sort_3ds(list: &mut [GameEntry], file: &str) -> std::io::Result<()> {
    let Some(c) = read_ctr(file)? else { return Ok(()) };
    let Some(g) = list.iter_mut().find(|x| x.ctr.as_ref().is_some_and(|t| t.eq_ignore_ascii_case(&c.title_id))) else { return Ok(()) };
    let len = std::fs::metadata(file)?.len();
    let key = format!("ctr-{}-{:x}-{:x}", c.title_id.to_lowercase(), len, last_write_ticks(Path::new(file)));
    if g.roms.iter().any(|x| x.sha1 == key) { return Ok(()); }
    g.roms.push(RomFile { file: file.into(), sha1: key, added: creation_ticks(Path::new(file)), encrypted: !c.decrypted, ..Default::default() });
    Ok(())
}

/// What a file name says besides its base game, such as "Rocket Edition". Tags, region and version words don't count.
const PLAIN_WORDS: [&str; 16] = ["pokemon", "pokémon", "version", "usa", "europe", "eur", "us", "u", "e", "j", "en", "rev", "v1", "v0", "rom", "the"];
pub fn hack_words(g: &GameEntry, file: &str) -> String {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    static DIGITS: OnceLock<Regex> = OnceLock::new();
    let name = re(&TAGS, r"[\(\[\{][^\)\]\}]*[\)\]\}]").replace_all(&file_stem(file), " ").into_owned();
    let mut base_words: Vec<String> = g.title.to_lowercase().split(' ').map(String::from).collect();
    base_words.push(g.id.clone());
    let words: Vec<&str> = re(&SPLIT, r"[^\p{L}\p{N}']+").split(&name).filter(|w| !w.is_empty()).collect();
    let kept: Vec<&str> = words.into_iter().filter(|w| {
        let lw = w.to_lowercase();
        !PLAIN_WORDS.contains(&lw.as_str()) && !base_words.contains(&lw) && !re(&DIGITS, r"^\d+$").is_match(w)
    }).collect();
    kept.join(" ")
}

/// RenameHack: the player's name for a hack, written back in the file's order (a new one last), as the C# does.
pub fn rename_hack(sha1: &str, name: &str) -> std::io::Result<()> {
    let mut d: Vec<(String, String)> = Vec::new();
    if let Ok(t) = std::fs::read_to_string(config::state().join("hack-names.txt")) {
        for l in lines(&t) {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 2 || f[1].trim().is_empty() { continue; }
            match d.iter_mut().find(|x| x.0 == f[0]) { Some(x) => x.1 = f[1].trim().to_string(), None => d.push((f[0].to_string(), f[1].trim().to_string())) }
        }
    }
    match d.iter_mut().find(|x| x.0 == sha1) { Some(x) => x.1 = name.to_string(), None => d.push((sha1.to_string(), name.to_string())) }
    std::fs::create_dir_all(config::state())?;
    std::fs::write(config::state().join("hack-names.txt"), d.iter().map(|x| format!("{}\t{}\r\n", x.0, x.1)).collect::<String>())
}

/// The player's names for hacks, by SHA-1.
pub fn hack_names() -> HashMap<String, String> {
    let mut d = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(config::state().join("hack-names.txt")) {
        for l in lines(&t) {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() == 2 && !f[1].trim().is_empty() { d.insert(f[0].to_string(), f[1].trim().to_string()); }
        }
    }
    d
}

fn slugify(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, "[^a-z0-9]+").replace_all(&s.to_lowercase(), "-").trim_matches('-').to_string()
}
/// Fulldex names a GBA game's save after its ROM file: lower case, every run of other characters one dash.
pub fn fulldex_key(file: &str) -> String { slugify(&file_stem(file)) }

fn hack_of(b: &GameEntry, r: RomFile, taken: &[String]) -> GameEntry {
    let mut h = b.clone();
    let words = hack_words(b, &r.file);
    let mut title = if !words.is_empty() { format!("{} {}", b.title, words) } else { format!("{} (modified {})", b.title, &r.sha1[..6]) };
    // A known build is named by its hash, whatever its file is called. The player's own name still wins.
    let known = b.known_hack(&r.sha1).cloned();
    if let Some(k) = &known { title = k[1].clone(); }
    let uv = !b.is_ds() && is_ultra_violet(&r.file);
    if uv && known.is_none() { title = "Pokemon Ultra Violet".into(); }
    if let Some(own) = hack_names().get(&r.sha1) { title = own.clone(); }
    let mut slug = slugify(if !words.is_empty() { &words } else { &r.sha1[..8] });
    if let Some(k) = &known { slug = if k[0].starts_with(&format!("{}-", b.id)) { k[0][b.id.len() + 1..].to_string() } else { k[0].clone() }; }
    let mut id = format!("{}-{}", b.id, if slug.is_empty() { r.sha1[..8].to_string() } else { slug.clone() });
    let mut i = 2;
    while taken.contains(&id) {
        id = format!("{}-{}-{}", b.id, slug, i);
        i += 1;
    }
    h.id = id.clone();
    h.title = title;
    h.base_id = Some(b.id.clone());
    h.gen1 = None;
    h.nativeplat = None;
    h.port = None;
    h.pc = None;
    h.imported = false;
    h.gen1_rom = None;
    h.fulldex_rom = None;
    h.nativeplat_rom = None;
    h.found_unsupported = None;
    h.found_sha1 = None;
    h.chosen = None;
    // Its save is the one Fulldex already gives it (a GBA game's is named after its file), so nothing moves.
    h.save = Some(if b.is_ds() { format!("{}.sav", id) } else if uv { "ultraviolet.sav".into() } else { format!("{}.sav", fulldex_key(&r.file)) });
    h.roms = vec![r];
    h.theme = b.theme.clone();
    h.theme.game_id = Some(id);
    h
}

/// Romhacks become games of their own, after their base game.
fn hacks(list: Vec<GameEntry>, gen1_found: bool) -> Vec<GameEntry> {
    let all_ids: Vec<String> = list.iter().map(|g| g.id.clone()).collect();
    let mut result: Vec<GameEntry> = Vec::new();
    for g in list {
        result.push(g);
        let gi = result.len() - 1;
        let (fulldex, pc) = (result[gi].fulldex_code.is_some(), result[gi].pc.is_some());
        if !fulldex || pc { continue; }
        let clean = result[gi].can_verify() && !(result[gi].gen1.is_some() && !gen1_found);
        let picked: Vec<RomFile> = result[gi].roms.iter().filter(|r| !r.encrypted && (is_ultra_violet(&r.file) || if clean { !r.verified() } else { result[gi].known_hack(&r.sha1).is_some() })).cloned().collect();
        for r in picked {
            let pos = result[gi].roms.iter().position(|x| *x == r).unwrap();
            result[gi].roms.remove(pos);
            let mut taken = all_ids.clone();
            taken.extend(result.iter().map(|x| x.id.clone()));
            let h = hack_of(&result[gi], r, &taken);
            result.push(h);
        }
    }
    result
}

/// Verified dumps first, then ready to run, then the oldest, so a new version never takes over.
fn ranked(g: &GameEntry) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..g.roms.len()).collect();
    idx.sort_by(|&a, &b| {
        let (x, y) = (&g.roms[a], &g.roms[b]);
        (x.engine_copy as u8).cmp(&(y.engine_copy as u8))
            .then((!x.verified() as u8).cmp(&(!y.verified() as u8)))
            .then((x.encrypted as u8).cmp(&(y.encrypted as u8)))
            .then(x.added.cmp(&y.added))
            .then(ordinal_ignore_case(&x.file, &y.file))
    });
    idx
}
/// StringComparer.OrdinalIgnoreCase: compares upper-cased UTF-16 code units.
fn ordinal_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let up = |s: &str| s.encode_utf16().map(|u| char::from_u32(u as u32).map_or(u as u32, |c| { let mut it = c.to_uppercase(); if it.len() == 1 { it.next().unwrap() as u32 } else { u as u32 } })).collect::<Vec<u32>>();
    up(a).cmp(&up(b))
}

fn choose_rom(g: &mut GameEntry) {
    if g.roms.is_empty() { return; }
    let saved = config::read(&format!("rom-{}.txt", g.id), "").trim().to_string();
    let rk = ranked(g);
    let chosen = g.roms.iter().position(|r| r.sha1 == saved).unwrap_or(rk[0]);
    g.chosen = Some(chosen);
    // Kept from the first time there's a choice, so moving or adding files can't change it later.
    if g.roms.len() > 1 && g.roms[chosen].sha1 != saved { config::write(&format!("rom-{}.txt", g.id), &g.roms[chosen].sha1); }
    let c = g.roms[chosen].clone();
    if g.fulldex_code.is_some() { g.fulldex_rom = Some(c.file.clone()); }
    if g.ctr.is_some() {
        // With Azahar picked, a decrypted dump is used when there is one; an encrypted one goes to AzaharPlus.
        let mut r = c.clone();
        if r.encrypted && emu3ds::default() != Some(emu3ds::plus()) {
            if let Some(&i) = rk.iter().find(|&&i| !g.roms[i].encrypted) { r = g.roms[i].clone(); }
        }
        g.ctr_rom = Some(r.file.clone());
        g.ctr_encrypted = r.encrypted;
        let plain = if c.encrypted { rk.iter().find(|&&i| !g.roms[i].encrypted).map(|&i| g.roms[i].clone()) } else { Some(c.clone()) };
        g.ctr_plain_rom = plain.map(|p| p.file);
    }
    // An engine with an exact list uses the chosen dump if it's on the list, or its best one that is.
    if g.gen1.is_some() {
        let r = if c.gen1_ok { Some(c.clone()) } else { rk.iter().find(|&&i| g.roms[i].gen1_ok).map(|&i| g.roms[i].clone()) };
        g.gen1_rom = r.map(|r| r.file);
    }
    if g.nativeplat.is_some() {
        let r = if c.nativeplat_ok { Some(c.clone()) } else { rk.iter().find(|&&i| g.roms[i].nativeplat_ok).map(|&i| g.roms[i].clone()) };
        g.nativeplat_rom = r.map(|r| r.file);
    }
    let refused = if g.gen1.is_some() { g.gen1_rom.is_none() } else { g.nativeplat.is_some() && g.fulldex_code.is_none() && g.nativeplat_rom.is_none() };
    if refused {
        g.found_unsupported = Some(c.file.clone());
        g.found_sha1 = Some(c.sha1.clone());
    }
}
