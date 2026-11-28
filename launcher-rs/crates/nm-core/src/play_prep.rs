//! The play flows' static parts from LauncherForm: engine options, the Platinum PC port's and nativeplat's save
//! hand-over, Fulldex's title-screen snapshot checks, and each engine's start (ProcessStartInfo) as the C# builds it.

use crate::config;
use crate::engines::gen1_start;
use crate::game_entry::GameEntry;
use crate::game_options;
use crate::library;
use crate::proc::StartInfo;
use crate::save_guard;
use crate::saves;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn s(p: &Path) -> String { p.to_string_lossy().into_owned() }
fn parent(p: &Path) -> PathBuf { p.parent().map(Path::to_path_buf).unwrap_or_default() }
fn io<T>(r: std::io::Result<T>) -> Result<T, String> { r.map_err(|e| e.to_string()) }

// ---------- options ----------
pub fn gen1_options(g: &GameEntry) -> String { game_options::for_game_ordered(g).iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(",") }
pub fn fulldex_cfg() -> PathBuf { config::fulldex_dir().join("settings.cfg") }
pub fn read_fulldex() -> HashMap<String, String> {
    let mut d = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(fulldex_cfg()) {
        for l in config::lines(&t) { if let Some(e) = l.find('=') { if e > 0 { d.insert(l[..e].to_string(), l[e + 1..].to_string()); } } }
    }
    d
}
/// Changes one line, keeping every other setting in the file.
pub fn write_fulldex(key: &str, value: &str) -> std::io::Result<()> {
    let text = std::fs::read_to_string(fulldex_cfg()).unwrap_or_default();
    let mut lines: Vec<String> = config::lines(&text).iter().map(|l| l.to_string()).collect();
    let head = format!("{}=", key);
    match lines.iter().position(|l| l.starts_with(&head)) { Some(at) => lines[at] = format!("{}={}", key, value), None => lines.push(format!("{}={}", key, value)) }
    std::fs::create_dir_all(parent(&fulldex_cfg()))?;
    config::write_all_lines(&fulldex_cfg(), &lines)
}
/// Each engine's own settings file. Gen1Recomp's come in with NATIVEMONS_OPTIONS.
pub fn apply_engine_options(g: &GameEntry, w: &HashMap<String, String>) -> std::io::Result<()> {
    let engine = g.engine();
    let on = |v: &String, a: &str, b: &str| if v == "on" { a.to_string() } else { b.to_string() };
    if engine == "fulldex" {
        // Fulldex and the GBA player take tens (Fulldex would cut 55 down to 50; the player has no 55 at all).
        if let Some(v) = w.get("vol") { write_fulldex("volume", &game_options::nearest(v, 10).to_string())?; }
        if let Some(v) = w.get("vol") {
            if !g.is_ds() && config::gba_player().is_file() {
                let ini = parent(&config::gba_player()).join("gba-settings.ini");
                let text = std::fs::read_to_string(&ini).unwrap_or_default();
                let mut lines: Vec<String> = config::lines(&text).iter().filter(|x| !x.starts_with("volume=")).map(|x| x.to_string()).collect();
                lines.push(format!("volume={}", game_options::nearest(v, 10)));
                config::write_all_lines(&ini, &lines)?;
            }
        }
        if let Some(v) = w.get("full") { write_fulldex("fullscreen", &on(v, "1", "0"))?; }
        if let Some(v) = w.get("wide") { write_fulldex("wide", &on(v, "1", "0"))?; }
        if let Some(v) = w.get("smooth") { write_fulldex("smooth", &on(v, "1", "0"))?; }
    } else if engine == "port" {
        let ini = port_dir().join("sim_config.ini");
        if let Some(v) = w.get("vol") {
            let n: i32 = v.trim().parse().map_err(|_| std::io::Error::other("Input string was not in a correct format."))?;
            game_options::set_ini(&ini, "General", "MasterVolume", &((n as f64 * 127.0 / 100.0).round() as i32).to_string())?;
        }
        if let Some(v) = w.get("full") { game_options::set_ini(&ini, "General", "FullScreen", &on(v, "true", "false"))?; }
    } else if engine == "nativeplat" {
        let ini = nativeplat_dir().join("userdata").join("options.ini");
        std::fs::create_dir_all(parent(&ini))?;
        if let Some(v) = w.get("vol") { game_options::set_ini(&ini, "audio", "volume", v)?; }
        if let Some(v) = w.get("music") { game_options::set_ini(&ini, "game", "bgm_volume", v)?; }
        if let Some(v) = w.get("sfx") { game_options::set_ini(&ini, "game", "se_volume", v)?; }
        if let Some(v) = w.get("full") { game_options::set_ini(&ini, "video", "fullscreen", &on(v, "1", "0"))?; }
        if let Some(v) = w.get("wide") { game_options::set_ini(&ini, "game", "widescreen", &on(v, "1", "0"))?; }
        if let Some(v) = w.get("smooth") { game_options::set_ini(&ini, "video", "filter", &on(v, "linear", "nearest"))?; }
    }
    Ok(())
}
/// ApplyOptions: the engine's settings, then the in-save ones through mon-bridge (the sync record follows the save).
pub fn apply_options(g: &GameEntry) {
    let ordered = game_options::for_game_ordered(g);
    let wanted: HashMap<String, String> = ordered.iter().cloned().collect();
    let _ = apply_engine_options(g, &wanted);
    let in_save: Vec<String> = ordered.iter().filter(|(k, _)| !game_options::is_engine(k)).map(|(k, v)| format!("{}={}", k, v)).collect();
    let master = saves::master_of(g);
    if in_save.is_empty() || g.engine() == "gen1" || !saves::has_master(g) || !master.is_file() || save_guard::is_blank(&master) || !config::mon_bridge().is_file() { return; }
    let before = save_guard::hash_of(&master);
    let (_, code) = save_guard::bridge(&format!("options \"{}\" {}", master.display(), in_save.join(" ")));
    let after = save_guard::hash_of(&master);
    if code != 0 || before == after { return; }
    let st = saves::read_sync(g);
    if st.get("master").map(String::as_str) == Some(before.as_str()) {
        let _ = saves::write_sync(g, &after, st.get("gen1").map_or("", |x| x), st.get("slot").map_or("", |x| x));
    }
}

// ---------- the Platinum PC port ----------
pub fn port_dir() -> PathBuf { parent(&config::platinum_port()) }
pub fn port_save() -> PathBuf { port_dir().join("save.bin") }
fn port_record(g: &GameEntry) -> PathBuf { config::state().join("save-sync").join(format!("{}-port.txt", g.id)) }
fn read_record(f: &Path) -> Option<String> { if f.is_file() { std::fs::read_to_string(f).ok().map(|t| t.trim().to_string()) } else { None } }
fn write_record(f: &Path, hash: &str) -> std::io::Result<()> { std::fs::create_dir_all(parent(f))?; std::fs::write(f, hash) }
/// None when all is well, or what the player should know. The shared save only changes once the port's has passed.
pub fn save_from_port(g: &GameEntry) -> Option<String> {
    let run = || -> Result<Option<String>, String> {
        let (save, master) = (port_save(), saves::master_of(g));
        if !save.is_file() || save_guard::is_blank(&save) || Some(save_guard::hash_of(&save)) == read_record(&port_record(g)) { return Ok(None); }
        let incoming = config::state().join("save-sync").join(format!("{}-port-incoming.sav", g.id));
        io(std::fs::create_dir_all(parent(&incoming)))?;
        let raw = io(std::fs::read(&save))?;
        // A file the converter can't read is still kept, whole, as it may be the only copy of what was played.
        let (data, mut problem) = if crate::platinum_save::is_old_port(&raw) {
            match crate::platinum_save::to_ds(&raw) { Ok(d) => (d, None), Err(x) => (raw.clone(), Some(x)) }
        } else { (raw, None) };
        io(std::fs::write(&incoming, &data))?;
        if problem.is_none() { problem = save_guard::problem(&incoming); }
        if let Some(problem) = problem {
            io(save_guard::backup(g, Some(&incoming), "platinum-port"))?;
            let _ = std::fs::remove_file(&incoming);
            return Ok(Some(format!("The Platinum PC port's save didn't pass its check, so your shared save wasn't changed. A copy is in your backups.\n\nWhy: {}", problem)));
        }
        io(save_guard::backup(g, Some(&master), "before-port"))?;
        io(save_guard::backup(g, Some(&incoming), "platinum-port"))?;
        io(save_guard::write_in_place(&incoming, &master))?;
        let (o, code) = save_guard::bridge(&format!("compare \"{}\" \"{}\"", incoming.display(), master.display()));
        let _ = std::fs::remove_file(&incoming);
        if code != 0 { return Ok(Some(format!("The Platinum PC port's save didn't copy correctly: {} Both are in your backups.", o.trim()))); }
        io(write_record(&port_record(g), &save_guard::hash_of(&save)))?;
        Ok(None)
    };
    run().unwrap_or_else(|x| Some(format!("The Platinum PC port's save wasn't brought back, and it'll try again next time. {}", x)))
}
/// Everything the port needs before it starts. An error if it can't be got ready, so a save is never put at risk.
pub fn prepare_port(g: &GameEntry) -> Result<(), String> {
    if !port_dir().join("arm9.bin").is_file() { crate::nds_extract::run(Path::new(g.fulldex_rom.as_deref().unwrap_or("")), &port_dir())?; }
    // Anything left from a run that never came back goes home first, and a copy is kept if it can't.
    let note = save_from_port(g);
    if note.is_some() && port_save().is_file() { io(save_guard::backup(g, Some(&port_save()), "platinum-port-leftover"))?; }
    let master = saves::master_of(g);
    if master.is_file() && !save_guard::is_blank(&master) { io(save_guard::write_in_place(&master, &port_save()))?; }
    else if port_save().is_file() { io(std::fs::remove_file(port_save()))?; }
    io(write_record(&port_record(g), &if port_save().is_file() { save_guard::hash_of(&port_save()) } else { String::new() }))
}
pub fn port_start() -> StartInfo { let mut psi = StartInfo::new(config::platinum_port(), ""); psi.cwd = Some(port_dir()); psi }

// ---------- nativeplat ----------
pub const NATIVEPLAT_SLOT_NAME: &str = "nativemons";
const DS_SAVE_BYTES: u64 = 0x80000;
pub fn nativeplat_dir() -> PathBuf { parent(&config::nativeplat()) }
pub fn nativeplat_slot(g: &GameEntry) -> PathBuf { nativeplat_dir().join("userdata").join("saves").join(g.nativeplat.as_deref().unwrap_or("")).join(format!("{}.sav", NATIVEPLAT_SLOT_NAME)) }
fn nativeplat_record(g: &GameEntry) -> PathBuf { config::state().join("save-sync").join(format!("{}-nativeplat.txt", g.id)) }
pub fn nativeplat_options_file() -> PathBuf { nativeplat_dir().join("userdata").join("options.ini") }
/// None when all is well, or what the player should know. The shared save only changes once nativeplat's has passed.
pub fn save_from_nativeplat(g: &GameEntry) -> Option<String> {
    let run = || -> Result<Option<String>, String> {
        let (save, master) = (nativeplat_slot(g), saves::master_of(g));
        let len = std::fs::metadata(&save).map(|m| m.len()).unwrap_or(0);
        if !save.is_file() || len == 0 || save_guard::is_blank(&save) || Some(save_guard::hash_of(&save)) == read_record(&nativeplat_record(g)) { return Ok(None); }
        if let Some(problem) = save_guard::problem(&save) {
            io(save_guard::backup(g, Some(&save), "nativeplat"))?;
            return Ok(Some(format!("nativeplat's save didn't pass its check, so your shared save wasn't changed. A copy is in your backups.\n\nWhy: {}", problem)));
        }
        io(std::fs::create_dir_all(parent(&master)))?;
        io(save_guard::backup(g, Some(&master), "before-nativeplat"))?;
        io(save_guard::backup(g, Some(&save), "nativeplat"))?;
        io(save_guard::write_in_place(&save, &master))?;
        if save_guard::hash_of(&save) != save_guard::hash_of(&master) { return Ok(Some("nativeplat's save didn't copy correctly. Both are in your backups.".into())); }
        io(write_record(&nativeplat_record(g), &save_guard::hash_of(&save)))?;
        Ok(None)
    };
    run().unwrap_or_else(|x| Some(format!("nativeplat's save wasn't brought back, and it'll try again next time. {}", x)))
}
/// Its copy follows the dump in use, or a change of version would have no effect.
pub fn nativeplat_rom_copy(g: &GameEntry) -> Result<PathBuf, String> {
    let rom = nativeplat_dir().join("userdata").join("roms").join(format!("{}.nds", g.nativeplat.as_deref().unwrap_or("")));
    io(std::fs::create_dir_all(parent(&rom)))?;
    let src = g.nativeplat_rom.clone().unwrap_or_default();
    let len = |p: &Path| std::fs::metadata(p).map(|m| m.len()).map_err(|e| e.to_string());
    if !rom.is_file() || len(&rom)? != len(Path::new(&src))? || io(library::sha1_of(&s(&rom)))? != io(library::sha1_of(&src))? {
        let part = PathBuf::from(format!("{}.part", rom.display()));
        io(std::fs::copy(&src, &part))?;
        if rom.is_file() { io(std::fs::remove_file(&rom))?; }
        io(std::fs::rename(&part, &rom))?;
    }
    Ok(rom)
}
/// Everything nativeplat needs before it starts. An error if it can't be got ready, so a save is never put at risk.
pub fn prepare_nativeplat(g: &GameEntry) -> Result<(), String> {
    let marker = nativeplat_dir().join("portable.txt");
    if !marker.is_file() { io(std::fs::write(&marker, ""))?; }
    nativeplat_rom_copy(g)?;
    let (note, save, master) = (save_from_nativeplat(g), nativeplat_slot(g), saves::master_of(g));
    if note.is_some() && save.is_file() { io(save_guard::backup(g, Some(&save), "nativeplat-leftover"))?; }
    io(std::fs::create_dir_all(parent(&save)))?;
    if master.is_file() && !save_guard::is_blank(&master) {
        let size = std::fs::metadata(&master).map(|m| m.len()).unwrap_or(0);
        if size != DS_SAVE_BYTES { return Err(format!("nativeplat needs a save of exactly 512 KB, and this one is {} bytes.", size)); }
        io(save_guard::write_in_place(&master, &save))?;
    } else { io(std::fs::write(&save, []))?; }
    let _ = std::fs::remove_file(format!("{}.bak", save.display()));
    io(write_record(&nativeplat_record(g), &save_guard::hash_of(&save)))
}
/// The play start (PlayNativeplat) or the waiting one (StartReady, quick = "wait").
pub fn nativeplat_start(g: &GameEntry, quick: Option<&str>) -> StartInfo {
    let mut psi = StartInfo::new(config::nativeplat(), &format!("--game {} --slot {}", g.nativeplat.as_deref().unwrap_or(""), NATIVEPLAT_SLOT_NAME));
    psi.cwd = Some(nativeplat_dir());
    if let Some(q) = quick { psi.set_env("NP_QUICK_START", q); }
    psi
}

// ---------- Gen1Recomp ----------
/// PlayGen1's start (and StartReady's, with NATIVEMONS_QUICK_START instead of NATIVEMONS_TO_TITLE).
pub fn gen1_play_start(g: &GameEntry, to_title: bool, waiting: bool) -> StartInfo {
    let mut psi = gen1_start(&format!(" --game={}", g.gen1.as_deref().unwrap_or("")));
    let options = gen1_options(g);
    if !options.is_empty() { psi.set_env("NATIVEMONS_OPTIONS", &options); }
    if waiting { psi.set_env("NATIVEMONS_QUICK_START", "1"); } else if to_title { psi.set_env("NATIVEMONS_TO_TITLE", "1"); }
    psi
}
/// Import's start: the ROM goes in by a plain path (the caller keeps the PlainPath alive while it runs).
pub fn gen1_import_start(rom_plain: &str) -> StartInfo {
    let mut psi = gen1_start("");
    psi.set_env("POKEPORT_IMPORT_ONLY", "1");
    psi.set_env("POKEPORT_FORCE_IMPORT", "1");
    psi.set_env("POKEPORT_IMPORT_ROM", rom_plain);
    psi
}

// ---------- Fulldex ----------
pub fn snap_dir() -> PathBuf { config::state().join("quick-start") }
pub fn snap_file(g: &GameEntry) -> PathBuf { snap_dir().join(format!("{}.snap", g.id)) }
pub fn snap_info(g: &GameEntry) -> PathBuf { snap_dir().join(format!("{}.txt", g.id)) }
pub fn fulldex_save(g: &GameEntry) -> PathBuf { config::fulldex_dir().join("saves").join(g.save.as_deref().unwrap_or("")) }
pub fn fulldex_funcs(g: &GameEntry) -> Option<PathBuf> {
    match g.fulldex_code.as_deref() { Some("IPGE") | Some("IPKE") => None, c => Some(config::fulldex_dir().join("data").join("funcs").join(format!("{}.txt", c.unwrap_or("")))) }
}
/// ZakuroSnap: 3DSrecomp+Zakuro's title-screen snapshot, for a built game while no quiet build runs.
pub fn zakuro_snap(g: &GameEntry) -> bool {
    g.playable() && g.engine() == "recomp" && g.ctr_plain_rom.is_some() && crate::recomp::built(g) && !crate::recomp::busy() && config::zakuro().is_file()
}
/// Frames from boot to the title screen, which waits there.
pub const ZAKURO_TITLE_FRAMES: i32 = 1800;
pub fn snap_save(g: &GameEntry) -> PathBuf { if g.engine() == "recomp" { crate::ctr3ds_saves::main(g) } else { fulldex_save(g) } }
pub fn can_snapshot(g: &GameEntry) -> bool {
    if zakuro_snap(g) { return true; }
    g.playable() && g.engine() == "fulldex" && g.is_ds() && g.art_frame > 0 && g.fulldex_rom.is_some() && g.fulldex_code.is_some()
        && config::fulldex().is_file() && !library::needs_patch(g.fulldex_rom.as_deref())
}
pub fn file_key(path: &Path) -> String {
    if path.as_os_str().is_empty() { return "|?".into(); }
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => format!("{}|{}|{}", path.display(), m.len(), library::last_write_ticks(path)),
        _ => format!("{}|none", path.display()),
    }
}
pub fn snap_key(g: &GameEntry) -> String {
    if g.engine() == "recomp" {
        return [file_key(Path::new(g.ctr_plain_rom.as_deref().unwrap_or(""))), file_key(&config::zakuro()), file_key(&crate::recomp::library_of(g)),
            save_hash(&crate::ctr3ds_saves::master(g).join("metadata")), ZAKURO_TITLE_FRAMES.to_string()].join("\n");
    }
    [file_key(Path::new(g.fulldex_rom.as_deref().unwrap_or(""))), file_key(&config::fulldex()), save_guard::hash_of(&fulldex_cfg()),
        fulldex_funcs(g).map(|f| s(&f)).unwrap_or_default(), g.art_frame.to_string()].join("\n")
}
static HASHES: Mutex<Option<HashMap<PathBuf, (String, String)>>> = Mutex::new(None);
/// The save's hash, worked out again only when the file changes: this is asked several times a second.
pub fn save_hash(path: &Path) -> String {
    let at = file_key(path);
    if let Some((k, h)) = HASHES.lock().unwrap().get_or_insert_with(HashMap::new).get(path) { if *k == at { return h.clone(); } }
    let h = save_guard::hash_of(path);
    HASHES.lock().unwrap().get_or_insert_with(HashMap::new).insert(path.to_path_buf(), (at, h.clone()));
    h
}
pub fn snapshot_good(g: &GameEntry) -> bool {
    if !can_snapshot(g) || !snap_file(g).is_file() || !snap_info(g).is_file() { return false; }
    let Ok(t) = std::fs::read_to_string(snap_info(g)) else { return false };
    let info: Vec<&str> = t.split("\n--\n").collect();
    info.len() == 2 && info[0] == snap_key(g) && info[1] == save_hash(&snap_save(g))
}
/// SnapshotTick's making: the save copied into a work folder (Zakuro: its data folder), then the headless start that
/// saves the title-screen state. None when the copy isn't the hashed save (try later) or the folder can't be made.
pub struct SnapshotMaking { pub psi: StartInfo, pub work: PathBuf, pub made: PathBuf }
pub fn snapshot_making(g: &GameEntry, save_hash_was: &str) -> Option<SnapshotMaking> {
    use crate::ctr3ds_saves as c3;
    let work = snap_dir().join(format!("{}-making", g.id));
    let made = work.join("title.snap");
    let mut copy = work.join("source.sav");
    let save = snap_save(g);
    let mut prep = || -> std::io::Result<bool> {
        if work.is_dir() { std::fs::remove_dir_all(&work)?; }
        std::fs::create_dir_all(&work)?;
        if g.engine() == "recomp" {
            // Zakuro plays a copy of the save folder, as Play hands it over, in a data folder of the snapshot's own.
            let to = work.join("data").join("user").join("savedata").join(g.ctr.as_deref().unwrap_or("").to_uppercase());
            let from = c3::master(g).join("data");
            if from.is_dir() {
                c3::mirror(Some(&from), &to)?;
                let meta = c3::master(g).join("metadata");
                let format = PathBuf::from(format!("{}.format", to.display()));
                if meta.is_file() { std::fs::copy(&meta, &format)?; }
                else if to.join("main").is_file() { std::fs::write(&format, [0u8, 0, 4, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0])?; }
            }
            copy = to.join("main");
        } else if save.is_file() {
            std::fs::copy(&save, &copy)?;
            let mut perm = std::fs::metadata(&copy)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            std::fs::set_permissions(&copy, perm)?;
        }
        // The copy must be the save that was hashed; if the player's save moved on meanwhile, try again later.
        if save_guard::hash_of(&copy) != save_hash_was { std::fs::remove_dir_all(&work)?; return Ok(false); }
        Ok(true)
    };
    if !matches!(prep(), Ok(true)) { return None; }
    let mut psi;
    if g.engine() == "recomp" {
        let e = crate::emu3ds::ZAKURO;
        // Its data folder is only made at the first play: a start in a missing folder fails.
        let _ = std::fs::create_dir_all(c3::emu_data(&e));
        psi = StartInfo::new(config::zakuro(), &format!("\"{}\" --recompiled \"{}\" --data \"{}\" --mute --headless {} --state-save \"{}\"",
            g.ctr_plain_rom.as_deref().unwrap_or(""), s(&crate::recomp::library_of(g)), s(&work.join("data")), ZAKURO_TITLE_FRAMES, s(&made)));
        psi.cwd = Some(c3::emu_data(&e));
        psi.set_env("APPDATA", &s(&c3::emu_data(&e).join("appdata")));
    } else {
        let dir = config::fulldex_dir();
        let mut args = format!("\"{}\" --save \"{}\" --no-save --config \"{}\" --keep-settings --jit", g.fulldex_rom.as_deref().unwrap_or(""), s(&copy), s(&dir.join("settings.cfg")));
        if let Some(f) = fulldex_funcs(g) { args += &format!(" --funcs \"{}\"", s(&f)); }
        args += &format!(" --headless --frames {} --state-save \"{}\" --state-at {}", g.art_frame + 5, s(&made), g.art_frame);
        psi = StartInfo::new(config::fulldex(), &args);
        psi.cwd = Some(dir);
    }
    psi.below_normal = true;
    Some(SnapshotMaking { psi, work, made })
}
/// The snapshot's end: kept (with its key and save hash) only when made cleanly and nothing it depends on changed.
pub fn snapshot_keep(g: &GameEntry, m: &SnapshotMaking, ok_exit: bool, key: &str, save_hash_was: &str) -> bool {
    let mut ok = false;
    if ok_exit && m.made.is_file() && snap_key(g) == key {
        let _ = std::fs::remove_file(snap_file(g));
        ok = std::fs::rename(&m.made, snap_file(g)).is_ok() && std::fs::write(snap_info(g), format!("{}
--
{}", key, save_hash_was)).is_ok();
    }
    let _ = std::fs::remove_dir_all(&m.work);
    ok
}
/// QuickOptions: what the waiting game read when it started (nativeplat's options file, else Gen1Recomp's options).
pub fn quick_options(g: &GameEntry) -> String { if g.engine() == "nativeplat" { save_hash(&nativeplat_options_file()) } else { gen1_options(g) } }
/// CanQuickStart: Gen1Recomp games, and nativeplat's.
pub fn can_quick_start(g: &GameEntry) -> bool {
    if !g.playable() { return false; }
    if g.engine() == "gen1" { return g.imported && g.gen1.is_some(); }
    g.engine() == "nativeplat" && g.nativeplat.is_some() && config::nativeplat().is_file()
}
/// PlayFulldex for a DS game: (start, quick-start note).
pub fn fulldex_ds_start(g: &GameEntry, quick_start: bool) -> (StartInfo, Option<String>) {
    let dir = config::fulldex_dir();
    let saves = dir.join("saves");
    let rom = g.fulldex_rom.as_deref().unwrap_or("");
    let mut args = format!("\"{}\" --save \"{}\" --config \"{}\" --jit", rom, saves.join(g.save.as_deref().unwrap_or("")).display(), dir.join("settings.cfg").display());
    // SoulSilver and HeartGold use the program's own tables and must not be given a function table.
    let code = g.fulldex_code.as_deref().unwrap_or("");
    if code != "IPGE" && code != "IPKE" { args += &format!(" --funcs \"{}\"", dir.join("data").join("funcs").join(format!("{}.txt", code)).display()); }
    let mut note = None;
    // Quick start: from the title-screen snapshot, only while the save is exactly the one it was made from.
    if quick_start && snapshot_good(g) { args += &format!(" --state-load \"{}\"", snap_file(g).display()); note = Some(format!("snapshot used {}", g.id)); }
    else if quick_start && can_snapshot(g) { note = Some(format!("snapshot not used {}", g.id)); }
    let mut psi = StartInfo::new(config::fulldex(), &args);
    psi.cwd = Some(dir);
    (psi, note)
}
/// PlayGba: our GBA player with the ROM where it is (plain paths) and the shared save in Fulldex's saves folder.
pub fn gba_player_start(g: &GameEntry, games: &[GameEntry], saves: &Path, rom: &str, base: &mut dyn FnMut(&str) -> Option<String>) -> StartInfo {
    let dir = parent(&config::gba_player());
    let mut args = format!("--rom \"{}\" --saves \"{}\" --data \"{}\"", rom, saves.display(), dir.display());
    if let Some(sv) = g.save.as_deref().filter(|x| !x.is_empty()) { args += &format!(" --save \"{}\"", saves.join(sv).display()); }
    // RetroAchievements: the account Fulldex keeps.
    args += &format!(" --config \"{}\"", config::fulldex_dir().join("settings.cfg").display());
    if library::is_ultra_violet_patch(g.fulldex_rom.as_deref().unwrap_or("")) {
        let b = games.iter().find(|x| Some(&x.id) == g.base_id.as_ref());
        let fr = b.and_then(|b| b.roms.iter().find(|r| r.revision == 0 && library::ext_lower(&r.file) == ".gba"));
        if let Some(fr) = fr { if let Some(p) = base(&fr.file) { args += &format!(" --base \"{}\"", p); } }
    }
    if read_fulldex().get("fullscreen").map_or(true, |v| v == "1") { args += " --fullscreen"; }
    let mut psi = StartInfo::new(config::gba_player(), &args);
    psi.cwd = Some(dir);
    psi
}

// ---------- PC games ----------
const PRELOAD_NAME: &str = "nativemons-saves.rb";
const PRELOAD: &str = "# nativeMons: when the launcher starts this copy, its saves go in the launcher's folder.\n\
if ENV['NATIVEMONS_SAVE_DIR'] && !ENV['NATIVEMONS_SAVE_DIR'].empty?\n  module System\n    def self.data_directory\n      d = ENV['NATIVEMONS_SAVE_DIR'].tr('\\\\', '/')\n      d.end_with?('/') ? d : d + '/'\n    end\n  end\nend\n";
/// PcGames.PrepareSaves: an imported copy keeps its saves beside the launcher (its preload script, mkxp.json, the env).
pub fn pc_prepare_saves(g: &GameEntry, psi: &mut StartInfo) -> std::io::Result<()> {
    if crate::pc_games::is_existing(g) || crate::pc_games::is_custom(g) { return Ok(()); }
    let (Some(dir), saves) = (crate::pc_games::own_dir(g), crate::pc_games::save_dir(g)) else { return Ok(()) };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&saves)?;
    std::fs::write(dir.join(PRELOAD_NAME), PRELOAD)?;
    // The game's own mkxp.json settings (vsync, frame skip...) stay: the preload line goes in after its opening brace.
    let json = dir.join("mkxp.json");
    let line = format!("  \"preloadScript\": [\"{}\"],\n", PRELOAD_NAME);
    let had = if json.is_file() { Some(std::fs::read_to_string(&json)?) } else { None };
    match had.as_ref().and_then(|h| h.find('{').map(|b| (h, b))) {
        None => std::fs::write(&json, format!("{{\n{}}}\n", line))?,
        Some((h, b)) => if !h.contains(PRELOAD_NAME) { std::fs::write(&json, format!("{}\n{}{}", &h[..b + 1], line, &h[b + 1..]))?; },
    }
    psi.set_env("NATIVEMONS_SAVE_DIR", &s(&saves));
    Ok(())
}
/// PcGames.Backup: before each session the save files go to state\save-backups\<id>\<time>, newest 30 kept.
pub fn pc_backup(g: &GameEntry) -> std::io::Result<()> {
    if crate::pc_games::is_custom(g) { return Ok(()); }
    let from = crate::pc_games::save_dir(g);
    if !from.is_dir() { return Ok(()); }
    let files: Vec<PathBuf> = std::fs::read_dir(&from)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file() && p.extension().map_or(false, |x| x.eq_ignore_ascii_case("rxdata"))).collect();
    if files.is_empty() { return Ok(()); }
    let root = config::state().join("save-backups").join(&g.id);
    let to = root.join(chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
    std::fs::create_dir_all(&to)?;
    for f in &files { std::fs::copy(f, to.join(f.file_name().unwrap_or_default()))?; }
    let mut dirs: Vec<String> = std::fs::read_dir(&root)?.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| s(&e.path())).collect();
    dirs.sort_by(|a, b| save_guard::ordinal(b, a));
    for old in dirs.into_iter().skip(30) { let _ = std::fs::remove_dir_all(old); }
    Ok(())
}
/// Play3dsNow for an emulator the player chose (not Zakuro): the ROM where it is, from the emulator's own folder.
pub fn emu3ds_start(g: &GameEntry, e: &crate::emu3ds::Emu3ds) -> StartInfo {
    let mut psi = StartInfo::new(e.exe_path(), &format!("\"{}\"", g.ctr_rom.as_deref().unwrap_or("")));
    psi.cwd = Some(if e.native { crate::ctr3ds_saves::emu_data(e) } else { e.dir() });
    psi
}
/// Play3dsNow on Zakuro: the recompiled library, a window placed in `area` (x, y, w, h) at a scale that fits, and the
/// title-screen snapshot when it's good. (start, quick-start note).
pub fn zakuro_start(g: &GameEntry, e: &crate::emu3ds::Emu3ds, area: (i32, i32, i32, i32), quick_start: bool) -> (StartInfo, Option<String>) {
    use crate::ctr3ds_saves as c3;
    let full = game_options::for_game(g).get("full").map_or(false, |v| v == "on");
    let place = format!(" --at {},{} --scale {} --focus-pad --fast-forward 4", area.0 + 40, area.1 + 30, c3::zakuro_scale(area.2, area.3));
    let mut args = format!("\"{}\" --recompiled \"{}\" --data \"{}\"{}{}", g.ctr_plain_rom.as_deref().unwrap_or(""), s(&crate::recomp::library_of(g)), s(&c3::emu_data(e)), place, if full { " --fullscreen" } else { "" });
    let mut note = None;
    if quick_start && snapshot_good(g) { args += &format!(" --state-load \"{}\"", s(&snap_file(g))); note = Some(format!("snapshot used {}", g.id)); }
    else if quick_start && can_snapshot(g) { note = Some(format!("snapshot not used {}", g.id)); }
    let mut psi = StartInfo::new(e.exe_path(), &args);
    psi.cwd = Some(c3::emu_data(e));
    psi.set_env("APPDATA", &s(&c3::emu_data(e).join("appdata")));
    (psi, note)
}
pub fn pc_start(g: &GameEntry) -> StartInfo {
    let exe = crate::pc_games::exe(g).unwrap_or_default();
    let mut psi = StartInfo::new(&exe, "");
    psi.cwd = crate::pc_games::dir(g).map(PathBuf::from);
    psi
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_and_ini() {
        assert_eq!(game_options::nearest("55", 10), 60);
        assert_eq!(game_options::nearest("x", 10), 0);
        assert_eq!(game_options::nearest("104", 10), 100);
        let d = std::env::temp_dir().join(format!("nm-ini-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let f = d.join("a.ini");
        std::fs::write(&f, "[General]\r\nFullScreen=false\r\n[Other]\r\nx = 1\r\n").unwrap();
        game_options::set_ini(&f, "general", "fullscreen", "true").unwrap();
        game_options::set_ini(&f, "General", "MasterVolume", "64").unwrap();
        game_options::set_ini(&f, "New", "k", "v").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "[General]\r\nFullScreen=true\r\nMasterVolume = 64\r\n[Other]\r\nx = 1\r\n[New]\r\nk = v\r\n");
        let _ = std::fs::remove_dir_all(&d);
    }
}
