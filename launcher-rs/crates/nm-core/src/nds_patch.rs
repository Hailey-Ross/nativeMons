//! hails.nds-Patcher (by Hails, MIT) ported line by line from hails_patcher.py, extract_key1.py and get_key1.py:
//! decrypts the 2 KB secure area of .nds ROMs, with backups and --restore, so no Python is needed.

use sha1::{Digest, Sha1};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const TABLE_SIZE: usize = 0x1048;
const TABLE_START: [u8; 4] = [0x99, 0xd5, 0x20, 0x5f];
const TABLE_SHA1: &str = "84e467f2485078e401a17a5f231e3fe6e9686648";
pub const AREA_SIZE: usize = 0x800;
pub const DECRYPTED_MARK: [u8; 8] = [0xff, 0xde, 0xff, 0xe7, 0xff, 0xde, 0xff, 0xe7];
const RELEASE_API: &str = "https://api.github.com/repos/TASEmulators/desmume/releases/latest";
const MAX_EXE_SIZE: u64 = 256 * 1024 * 1024;

/// The patcher's command line: targets, --dry-run, --restore, --key.
#[derive(Clone, Debug, Default)]
pub struct Args {
    pub targets: Vec<String>,
    pub dry_run: bool,
    pub restore: bool,
    pub key: Option<String>,
}

/// One line the patcher printed: `err` is Python's stderr (sys.exit messages), else stdout.
#[derive(Clone, Debug, PartialEq)]
pub struct Line { pub err: bool, pub text: String }

/// Where the patcher keeps key1.bin and backups (the script's folder), plus how it gets a table it lacks.
pub struct Patcher<'a> {
    pub here: PathBuf,
    pub download: &'a mut dyn FnMut(&mut dyn FnMut(Line)) -> Result<Vec<u8>, String>,
}

fn sha1_hex(d: &[u8]) -> String { Sha1::digest(d).iter().map(|b| format!("{:02x}", b)).collect() }

/// extract_key1.find_in_bytes: the first TABLE_START whose TABLE_SIZE bytes have the known SHA-1.
pub fn find_in_bytes(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let at = data[pos..].windows(4).position(|w| w == TABLE_START)? + pos;
        let table = &data[at..(at + TABLE_SIZE).min(data.len())];
        if sha1_hex(table) == TABLE_SHA1 { return Some(table.to_vec()); }
        pos = at + 1;
    }
    None
}

fn words(table: &[u8]) -> Vec<u32> { table.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect() }

fn round_f(kb: &[u32], z: u32) -> u32 {
    let mut x = kb[0x12 + ((z >> 24) & 0xFF) as usize];
    x = kb[0x112 + ((z >> 16) & 0xFF) as usize].wrapping_add(x);
    x ^= kb[0x212 + ((z >> 8) & 0xFF) as usize];
    kb[0x312 + (z & 0xFF) as usize].wrapping_add(x)
}

fn encrypt(kb: &[u32], d: &mut [u32], o: usize) {
    let (mut y, mut x) = (d[o], d[o + 1]);
    for i in 0..0x10 {
        let z = kb[i] ^ x;
        x = y ^ round_f(kb, z);
        y = z;
    }
    d[o] = x ^ kb[0x10];
    d[o + 1] = y ^ kb[0x11];
}

fn decrypt(kb: &[u32], d: &mut [u32], o: usize) {
    let (mut y, mut x) = (d[o], d[o + 1]);
    for i in (2..=0x11).rev() {
        let z = kb[i] ^ x;
        x = y ^ round_f(kb, z);
        y = z;
    }
    d[o] = x ^ kb[1];
    d[o + 1] = y ^ kb[0];
}

fn apply_keycode(kb: &mut [u32], kc: &mut [u32; 3]) {
    encrypt(kb, kc, 1);
    encrypt(kb, kc, 0);
    for i in 0..0x12 { kb[i] ^= kc[i % 2].swap_bytes(); }
    let mut s = [0u32; 2];
    for i in (0..0x412).step_by(2) {
        encrypt(kb, &mut s, 0);
        kb[i] = s[1];
        kb[i + 1] = s[0];
    }
}

fn init_key(table: &[u32], idcode: u32, level: u32) -> Vec<u32> {
    let mut kb = table.to_vec();
    let mut kc = [idcode, idcode >> 1, idcode << 1];
    apply_keycode(&mut kb, &mut kc);
    apply_keycode(&mut kb, &mut kc);
    kc[1] <<= 1;
    kc[2] >>= 1;
    if level >= 3 { apply_keycode(&mut kb, &mut kc); }
    kb
}

/// decrypt_area: the first 8 bytes at level 2, then every 8 bytes at level 3.
pub fn decrypt_area(table: &[u32], area: &[u8], idcode: u32) -> Vec<u8> {
    let mut w = words(area);
    decrypt(&init_key(table, idcode, 2), &mut w, 0);
    let kb = init_key(table, idcode, 3);
    for i in (0..w.len()).step_by(2) { decrypt(&kb, &mut w, i); }
    w.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// read_area: (game code, game code as number, ARM9 offset, area), or None if not a usable ROM.
pub fn read_area(path: &Path) -> std::io::Result<Option<(String, u32, u64, Vec<u8>)>> {
    let mut f = std::fs::File::open(path)?;
    let mut head = Vec::new();
    (&mut f).take(0x200).read_to_end(&mut head)?;
    if head.len() < 0x200 { return Ok(None); }
    let off = u32::from_le_bytes([head[0x20], head[0x21], head[0x22], head[0x23]]) as u64;
    if !(0x4000..=(0x8000 - AREA_SIZE as u64)).contains(&off) { return Ok(None); }
    f.seek(SeekFrom::Start(off))?;
    let mut area = Vec::new();
    f.take(AREA_SIZE as u64).read_to_end(&mut area)?;
    if area.len() != AREA_SIZE { return Ok(None); }
    let code: String = head[12..16].iter().map(|&b| b as char).collect();
    Ok(Some((code, u32::from_le_bytes([head[12], head[13], head[14], head[15]]), off, area)))
}

/// os.path.basename (both separators on Windows, '/' elsewhere).
fn basename(path: &str) -> &str {
    let cut = if cfg!(windows) { path.rfind(['\\', '/']) } else { path.rfind('/') };
    cut.map(|i| &path[i + 1..]).unwrap_or(path)
}

/// os.path.splitext(name)[0]: leading dots never start the extension.
fn strip_ext(name: &str) -> &str {
    let lead = name.len() - name.trim_start_matches('.').len();
    match name[lead..].rfind('.') { Some(i) => &name[..lead + i], None => name }
}

/// os.path.join of a folder and a name, as Python prints it.
fn join(dir: &str, name: &str) -> String {
    let sep = if cfg!(windows) { '\\' } else { '/' };
    if dir.is_empty() || dir.ends_with(sep) || (cfg!(windows) && (dir.ends_with('/') || dir.ends_with(':'))) { format!("{}{}", dir, name) } else { format!("{}{}{}", dir, sep, name) }
}

fn write_area(path: &str, off: u64, area: &[u8]) -> std::io::Result<bool> {
    {
        let mut f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
        f.seek(SeekFrom::Start(off))?;
        f.write_all(area)?;
    }
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::Start(off))?;
    let mut back = Vec::new();
    f.take(AREA_SIZE as u64).read_to_end(&mut back)?;
    Ok(back == area)
}

/// glob "*.nds" in a folder, sorted: no hidden names, extension case-insensitive on Windows.
fn nds_in(dir: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).map(|rd| rd.flatten().filter_map(|e| e.file_name().into_string().ok()).collect()).unwrap_or_default();
    names.retain(|n| !n.starts_with('.') && if cfg!(windows) { n.to_lowercase().ends_with(".nds") } else { n.ends_with(".nds") });
    let mut out: Vec<String> = names.iter().map(|n| join(dir, n)).collect();
    out.sort();
    out
}

/// glob ..\desmume*\*.exe next to the patcher's folder.
fn desmume_exes(here: &str) -> Vec<String> {
    let parent = join(here, "..");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&parent) else { return out };
    let mut dirs: Vec<String> = rd.flatten().filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.') && if cfg!(windows) { n.to_lowercase().starts_with("desmume") } else { n.starts_with("desmume") }).collect();
    dirs.sort();
    for d in dirs {
        let dir = join(&parent, &d);
        let mut exes: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.flatten().filter_map(|e| e.file_name().into_string().ok()).collect()).unwrap_or_default();
        exes.retain(|n| !n.starts_with('.') && if cfg!(windows) { n.to_lowercase().ends_with(".exe") } else { n.ends_with(".exe") });
        exes.sort();
        out.extend(exes.iter().map(|n| join(&dir, n)));
    }
    out
}

/// An early stop (Python's sys.exit("...")): the message goes to stderr and the code is 1.
struct Exit(String);

impl Patcher<'_> {
    fn here_s(&self) -> String { self.here.to_string_lossy().into_owned() }
    fn backups(&self) -> String { join(&self.here_s(), "backups") }
    fn backup_path(&self, path: &str) -> String { join(&self.backups(), &format!("{}.secure-area.bin", strip_ext(basename(path)))) }

    fn key_candidates(&self, explicit: Option<&str>) -> Result<Vec<String>, Exit> {
        if let Some(e) = explicit.filter(|e| !e.is_empty()) {
            if !Path::new(e).is_file() { return Err(Exit(format!("{} not found.", e))); }
            return Ok(vec![e.to_string()]);
        }
        let here = self.here_s();
        let mut found: Vec<String> = ["key1.bin", "biosnds7.rom", "bios7.bin"].iter().map(|n| join(&here, n)).collect();
        found.extend(desmume_exes(&here));
        Ok(found.into_iter().filter(|p| Path::new(p).is_file()).collect())
    }

    fn load_table(&mut self, explicit: Option<&str>, out: &mut dyn FnMut(Line)) -> Result<Vec<u32>, Exit> {
        for path in self.key_candidates(explicit)? {
            let data = std::fs::read(&path).map_err(|e| Exit(format!("{}", e)))?;
            if let Some(table) = find_in_bytes(&data) {
                out(Line { err: false, text: format!("Key table: {}", path) });
                return Ok(words(&table));
            }
        }
        if let Some(e) = explicit.filter(|e| !e.is_empty()) { return Err(Exit(format!("No key table in {}.", e))); }
        out(Line { err: false, text: "No key table here, getting one from the latest DeSmuME release.".into() });
        let got = (self.download)(out).and_then(|table| {
            let key = join(&self.here_s(), "key1.bin");
            std::fs::write(&key, &table).map_err(|e| e.to_string())?;
            out(Line { err: false, text: format!("Wrote {} ({} bytes)", key, table.len()) });
            Ok(table)
        });
        match got {
            Ok(table) => Ok(words(&table)),
            Err(e) => Err(Exit(format!("Could not get the key table: {}\nUse --key with a DeSmuME exe or an ARM7 BIOS file.", e))),
        }
    }

    fn patch(&self, path: &str, table: &[u32], dry_run: bool, out: &mut dyn FnMut(Line)) -> std::io::Result<bool> {
        let name = basename(path);
        let mut say = |t: String| out(Line { err: false, text: t });
        let Some((code, idcode, off, area)) = read_area(Path::new(path))? else {
            say(format!("SKIP    {}: no secure area", name));
            return Ok(true);
        };
        if area[..8] == DECRYPTED_MARK {
            say(format!("OK      {} ({}): already decrypted", name, code));
            return Ok(true);
        }
        let dec = decrypt_area(table, &area, idcode);
        if &dec[..8] != b"encryObj" {
            say(format!("FAILED  {} ({}): did not decrypt, ROM left unchanged", name, code));
            return Ok(false);
        }
        if dry_run {
            say(format!("WOULD   {} ({}): needs decrypting", name, code));
            return Ok(true);
        }
        let backup = self.backup_path(path);
        if Path::new(&backup).exists() {
            say(format!("FAILED  {} ({}): backup already exists, ROM left unchanged: {}", name, code, backup));
            return Ok(false);
        }
        std::fs::create_dir_all(self.backups())?;
        std::fs::write(&backup, &area)?;
        let mut fixed = DECRYPTED_MARK.to_vec();
        fixed.extend_from_slice(&dec[8..]);
        if !write_area(path, off, &fixed)? {
            say(format!("FAILED  {} ({}): write did not verify, restore from {}", name, code, backup));
            return Ok(false);
        }
        say(format!("FIXED   {} ({}), backup: {}", name, code, backup));
        Ok(true)
    }

    fn restore(&self, path: &str, out: &mut dyn FnMut(Line)) -> std::io::Result<bool> {
        let name = basename(path);
        let info = read_area(Path::new(path))?;
        let backup = self.backup_path(path);
        let usable = Path::new(&backup).is_file() && std::fs::metadata(&backup).map(|m| m.len() == AREA_SIZE as u64).unwrap_or(false);
        let (Some(info), true) = (info, usable) else {
            out(Line { err: false, text: format!("FAILED  {}: no usable backup at {}", name, backup) });
            return Ok(false);
        };
        let area = std::fs::read(&backup)?;
        if !write_area(path, info.2, &area)? {
            out(Line { err: false, text: format!("FAILED  {}: write did not verify", name) });
            return Ok(false);
        }
        std::fs::remove_file(&backup)?;
        out(Line { err: false, text: format!("RESTORED {}", name) });
        Ok(true)
    }

    fn collect(targets: &[String], out: &mut dyn FnMut(Line)) -> Vec<String> {
        let mut roms = Vec::new();
        for t in targets {
            let p = Path::new(t);
            if p.is_dir() { roms.extend(nds_in(t)); } else if p.is_file() { roms.push(t.clone()); } else { out(Line { err: false, text: format!("FAILED  {}: not found", t) }); }
        }
        roms
    }

    /// hails_patcher.main: prints through `out`, returns the exit code.
    pub fn run(&mut self, args: &Args, out: &mut dyn FnMut(Line)) -> i32 {
        let roms = Self::collect(&args.targets, out);
        let results: std::io::Result<Vec<bool>> = if args.restore {
            roms.iter().map(|p| self.restore(p, out)).collect()
        } else {
            let table = match self.load_table(args.key.as_deref(), out) {
                Ok(t) => t,
                Err(Exit(msg)) => { for l in msg.lines() { out(Line { err: true, text: l.to_string() }); } return 1; }
            };
            roms.iter().map(|p| self.patch(p, &table, args.dry_run, out)).collect()
        };
        match results {
            Ok(r) => if roms.is_empty() || !r.iter().all(|&b| b) { 1 } else { 0 },
            Err(e) => { out(Line { err: true, text: e.to_string() }); 1 }
        }
    }
}

/// get_key1.download_table: the latest DeSmuME release's Windows x64 zip, its exes read as data only, never run.
pub fn download_table(temp: &Path, out: &mut dyn FnMut(Line)) -> Result<Vec<u8>, String> {
    let release: serde_json::Value = serde_json::from_str(&crate::net::download_string_unnoted(RELEASE_API)?).map_err(|e| e.to_string())?;
    let asset = release["assets"].as_array().and_then(|a| a.iter().find(|x| {
        let n = x["name"].as_str().unwrap_or("").to_lowercase();
        n.ends_with(".zip") && n.contains("win64")
    })).ok_or_else(|| format!("release {} has no Windows x64 zip", release["tag_name"].as_str().unwrap_or("None")))?;
    let name = asset["name"].as_str().unwrap_or("");
    out(Line { err: false, text: format!("Downloading DeSmuME {}: {}", release["tag_name"].as_str().unwrap_or(""), name) });
    let dir = temp.join(format!("hails-patcher-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let got = (|| {
        let zip = dir.join("desmume.zip");
        crate::net::download_file_unnoted(asset["browser_download_url"].as_str().unwrap_or(""), &zip)?;
        if std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0) != asset["size"].as_u64().unwrap_or(u64::MAX) { return Err("download is incomplete".to_string()); }
        let f = std::fs::File::open(&zip).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(f).map_err(|e| format!("download is not a valid zip: {}", e))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("download is not a valid zip: {}", e))?;
            if entry.name().to_lowercase().ends_with(".exe") && entry.size() <= MAX_EXE_SIZE {
                let mut data = Vec::new();
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                if let Some(t) = find_in_bytes(&data) { return Ok(t); }
            }
        }
        Err(format!("no key table in {}", name))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    got
}

/// The launcher's call: `hails_patcher.py "rom" ...` with `here` as the script's folder, a missing table downloaded.
pub fn patch_files(here: &Path, roms: &[String]) -> (i32, Vec<Line>) {
    let _ = std::fs::create_dir_all(here);
    let temp = crate::proc::temp_dir();
    let mut dl = move |out: &mut dyn FnMut(Line)| download_table(&temp, out);
    let mut p = Patcher { here: here.to_path_buf(), download: &mut dl };
    let mut lines = Vec::new();
    let code = p.run(&Args { targets: roms.to_vec(), ..Default::default() }, &mut |l| lines.push(l));
    (code, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_python() {
        assert_eq!(strip_ext("game.nds"), "game");
        assert_eq!(strip_ext("a.b.nds"), "a.b");
        assert_eq!(strip_ext(".nds"), ".nds");
        assert_eq!(strip_ext("noext"), "noext");
        assert_eq!(basename(if cfg!(windows) { "C:\\x\\rom.nds" } else { "/x/rom.nds" }), "rom.nds");
        let s = if cfg!(windows) { "\\" } else { "/" };
        assert_eq!(join("dir", "f"), format!("dir{}f", s));
        assert_eq!(join(&format!("dir{}", s), "f"), format!("dir{}f", s));
    }

    #[test]
    fn find_table_needs_the_hash() {
        let mut data = vec![0u8; 16];
        data.extend_from_slice(&TABLE_START);
        data.extend(vec![1u8; TABLE_SIZE]);
        assert_eq!(find_in_bytes(&data), None);
        assert_eq!(find_in_bytes(&[]), None);
        assert_eq!(find_in_bytes(&TABLE_START[..3]), None);
    }

    #[test]
    fn cipher_round_trips() {
        let table: Vec<u32> = (0..(TABLE_SIZE / 4) as u32).map(|i| i.wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995).collect();
        let kb = init_key(&table, 0x4555_4643, 3);
        let mut d = [0x1234_5678u32, 0x9abc_def0];
        encrypt(&kb, &mut d, 0);
        assert_ne!(d, [0x1234_5678, 0x9abc_def0]);
        decrypt(&kb, &mut d, 0);
        assert_eq!(d, [0x1234_5678, 0x9abc_def0]);
    }

    #[test]
    fn short_or_odd_headers_are_skipped() {
        let dir = std::env::temp_dir().join(format!("nm-ndspatch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.nds");
        std::fs::write(&small, vec![0u8; 0x100]).unwrap();
        assert!(read_area(&small).unwrap().is_none());
        let mut head = vec![0u8; 0x8000];
        head[0x20..0x24].copy_from_slice(&0x3000u32.to_le_bytes());
        let low = dir.join("low.nds");
        std::fs::write(&low, &head).unwrap();
        assert!(read_area(&low).unwrap().is_none());
        head[0x20..0x24].copy_from_slice(&0x4000u32.to_le_bytes());
        head[12..16].copy_from_slice(b"CPUE");
        std::fs::write(&low, &head).unwrap();
        let (code, id, off, area) = read_area(&low).unwrap().unwrap();
        assert_eq!((code.as_str(), id, off, area.len()), ("CPUE", u32::from_le_bytes(*b"CPUE"), 0x4000, AREA_SIZE));
        let mut fail = |_: &mut dyn FnMut(Line)| -> Result<Vec<u8>, String> { Err("offline".into()) };
        let mut p = Patcher { here: dir.clone(), download: &mut fail };
        let mut lines = Vec::new();
        let code = p.run(&Args { targets: vec![small.to_string_lossy().into()], ..Default::default() }, &mut |l| lines.push(l));
        assert_eq!(code, 1);
        assert_eq!(lines[0].text, "No key table here, getting one from the latest DeSmuME release.");
        assert_eq!(lines[1], Line { err: true, text: "Could not get the key table: offline".into() });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
