//! LauncherForm's save code without the window: one save for both engines (SyncSave), several saves per game
//! (switch, new, copy, delete, adopt), exporting, Gen1Recomp's import and prepare, the Pokedex read for "caught".

use crate::config;
use crate::ctr3ds_saves;
use crate::emu3ds;
use crate::engines::{self, gen1_start, run_capture, win_sep, PlainPath};
use crate::game_entry::GameEntry;
use crate::library::file_name;
use crate::pc_games;
use crate::proc;
use crate::save_guard::{self, hash, hash_of};
use crate::save_sets;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

type R<T> = Result<T, String>;
fn io<T>(r: std::io::Result<T>) -> R<T> { r.map_err(|e| e.to_string()) }
fn last_line(s: &str) -> String { s.trim().split('\n').last().unwrap_or("").trim().to_string() }
fn some_file(p: &Option<String>) -> Option<&Path> { p.as_deref().map(Path::new) }

#[derive(Default, Debug)]
pub struct SyncResult {
    pub conflict: bool,
    pub changed: bool,
    pub failed: bool,
    pub note: Option<String>,
    pub master: Option<String>,
    pub gen1: Option<String>,
}

// ---------- one save for both engines ----------
pub fn master_of(g: &GameEntry) -> PathBuf { if g.ctr.is_some() { ctr3ds_saves::main(g) } else { config::fulldex_dir().join("saves").join(g.save.as_deref().unwrap_or("")) } }
pub fn shares(g: &GameEntry) -> bool { g.has_both() && config::mon_bridge().is_file() }
fn sync_file(g: &GameEntry) -> PathBuf { config::state().join("save-sync").join(format!("{}.txt", g.id)) }
pub(crate) fn read_sync(g: &GameEntry) -> HashMap<String, String> {
    let mut d = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(sync_file(g)) {
        for l in config::lines(&t) { if let Some(e) = l.find('=') { if e > 0 { d.insert(l[..e].to_string(), l[e + 1..].to_string()); } } }
    }
    d
}
pub(crate) fn write_sync(g: &GameEntry, master: &str, gen1: &str, slot: &str) -> R<()> {
    let f = sync_file(g);
    io(std::fs::create_dir_all(f.parent().unwrap()))?;
    let temp = PathBuf::from(format!("{}.part", f.display()));
    io(std::fs::write(&temp, format!("master={}\ngen1={}\nslot={}\n", master, gen1, slot)))?;
    io(std::fs::copy(&temp, &f))?;
    io(std::fs::remove_file(&temp))
}
fn gen1_gen(g: &GameEntry) -> &str { g.gen1.as_deref().unwrap_or("") }
/// Gen1Recomp's save as a cartridge file. None when it has none yet; "why" is set only when something went wrong.
fn gen1_export(g: &GameEntry) -> (Option<String>, Option<String>) {
    let mut psi = gen1_start("");
    psi.set_env("NATIVEMONS_CLI", &format!("export {}", gen1_gen(g)));
    let (o, code) = run_capture(&psi, 90);
    let o = o.trim().to_string();
    if code == 0 && !o.is_empty() { return (Some(win_sep(&last_line(&o))), None); }
    let why = if !o.to_lowercase().contains("no save") { Some(if !o.is_empty() { o } else { "Gen1Recomp couldn't export its save.".into() }) } else { None };
    (None, why)
}
pub fn gen1_slot(_g: &GameEntry, command: &str) -> Option<String> {
    let mut psi = gen1_start("");
    psi.set_env("NATIVEMONS_CLI", command);
    let (o, code) = run_capture(&psi, 60);
    if code == 0 { Some(last_line(&o)) } else { None }
}

/// Brings both engines level. Never fails, or replaces a save without a verified backup.
pub fn sync_save(g: &GameEntry, prefer: Option<&str>) -> SyncResult {
    let mut r = SyncResult::default();
    if !shares(g) || !g.imported { return r; }
    match sync_inner(g, prefer, &mut r) {
        Ok(()) => r,
        Err(x) => kept(r, &x),
    }
}
fn sync_inner(g: &GameEntry, prefer: Option<&str>, r: &mut SyncResult) -> R<()> {
    let (gen1, why) = gen1_export(g);
    if let Some(why) = why { kept_in(r, &why); return Ok(()); }
    let master = master_of(g);
    let has_master = master.is_file() && !save_guard::is_blank(&master);
    let (hm, hg) = (hash_of(&master), hash(some_file(&gen1)));
    let st = read_sync(g);
    let (last_m, last_g, slot) = (st.get("master").cloned(), st.get("gen1").cloned(), st.get("slot").cloned());
    let known = last_m.is_some();
    let m_changed = has_master && (!known || Some(&hm) != last_m.as_ref());
    let g_changed = gen1.is_some() && (!known || Some(&hg) != last_g.as_ref());
    if !has_master && gen1.is_none() { return Ok(()); }
    if hm == hg { return write_sync(g, &hm, &hg, slot.as_deref().unwrap_or("")); }
    if !m_changed && !g_changed { return Ok(()); }
    let direction = if m_changed && g_changed { prefer } else if m_changed { Some("fulldex") } else { Some("gen1") };
    let Some(direction) = direction else {
        r.conflict = true;
        r.master = Some(master.to_string_lossy().into_owned());
        r.gen1 = gen1;
        return Ok(());
    };
    if direction == "fulldex" { master_to_gen1(g, r, &master, slot.as_deref()) } else { gen1_to_master(g, r, gen1.as_deref().unwrap_or(""), &master, slot.as_deref()) }
}
fn kept_note(why: &str) -> String { format!("Your saves weren't shared this time, and neither was changed. It'll try again next time.\n\nWhy: {}", why) }
fn kept(mut r: SyncResult, why: &str) -> SyncResult { kept_in(&mut r, why); r }
fn kept_in(r: &mut SyncResult, why: &str) {
    r.failed = true;
    r.note = Some(kept_note(why));
}
fn gen1_to_master(g: &GameEntry, r: &mut SyncResult, gen1: &str, master: &Path, slot: Option<&str>) -> R<()> {
    let gen1p = Path::new(gen1);
    if let Some(problem) = save_guard::problem(gen1p) { kept_in(r, &format!("Gen1Recomp's save failed its check: {}", problem)); return Ok(()); }
    io(save_guard::backup(g, Some(master), "before-sync"))?;
    io(save_guard::backup(g, Some(gen1p), "gen1recomp"))?;
    io(save_guard::write_in_place(gen1p, master))?;
    let (o, code) = save_guard::bridge(&format!("compare \"{}\" \"{}\"", gen1, master.display()));
    if code != 0 { kept_in(r, &format!("The result didn't match: {}", o.trim())); return Ok(()); }
    write_sync(g, &hash_of(master), &hash_of(gen1p), slot.unwrap_or(""))?;
    r.changed = true;
    Ok(())
}
fn repaired_file(g: &GameEntry) -> R<PathBuf> {
    let f = config::state().join("save-sync").join(format!("{}-repaired.sav", g.id));
    io(std::fs::create_dir_all(f.parent().unwrap()))?;
    Ok(f)
}
fn master_to_gen1(g: &GameEntry, r: &mut SyncResult, master: &Path, slot: Option<&str>) -> R<()> {
    let problem = save_guard::problem(master);
    let mut source = master.to_path_buf();
    let mut repaired: Option<PathBuf> = None;
    if let Some(p) = &problem {
        // A wrong checksum may still be repairable. The repaired copy has to pass everything the original would.
        let f = repaired_file(g)?;
        let (_, rc) = save_guard::bridge(&format!("repair \"{}\" \"{}\"", master.display(), f.display()));
        if rc != 0 { kept_in(r, &format!("The Fulldex save failed its check: {}", p)); return Ok(()); }
        source = f.clone();
        repaired = Some(f);
    }
    let (before, _) = gen1_export(g);
    io(save_guard::backup(g, Some(master), "fulldex"))?;
    io(save_guard::backup(g, some_file(&before), "before-sync"))?;
    let active = gen1_slot(g, &format!("active {}", gen1_gen(g)));
    let (mut last, mut why) = try_import_proved(g, &source)?;
    // One more way in: the same save rewritten by PKHeX, which can get past a strict reader.
    if last.is_none() && repaired.is_none() {
        let f = repaired_file(g)?;
        repaired = Some(f.clone());
        let (_, rc) = save_guard::bridge(&format!("repair \"{}\" \"{}\"", master.display(), f.display()));
        let mut again = None;
        if rc == 0 {
            let (l, a) = try_import_proved(g, &f)?;
            last = l;
            again = a;
        }
        if last.is_none() && again.is_some() { why = again; }
    }
    let Some(last) = last else {
        if let Some(a) = active.as_deref().filter(|a| *a != "nil") { gen1_slot(g, &format!("activate {} {}", gen1_gen(g), a)); }
        kept_in(r, why.as_deref().unwrap_or(""));
        return Ok(());
    };
    // The slot this sync made last time isn't needed now; the player's own slots are never removed.
    if let Some(s) = slot.filter(|s| !s.is_empty()) { if Some(s) == active.as_deref() { gen1_slot(g, &format!("delete {} {}", gen1_gen(g), s)); } }
    let (after, _) = gen1_export(g);
    write_sync(g, &hash_of(master), &hash(some_file(&after)), &last)?;
    if let Some(f) = &repaired { let _ = std::fs::remove_file(f); }
    r.changed = true;
    if problem.is_some() { r.note = Some("Fulldex's save had a wrong checksum. It was repaired for Gen1Recomp, and the original is in your backups.".into()); }
    Ok(())
}
/// Imports into a new slot and proves it matches. A slot that doesn't match is removed. (slot, why)
fn try_import_proved(g: &GameEntry, source: &Path) -> R<(Option<String>, Option<String>)> {
    let mut psi = gen1_start("");
    let (slot, code) = {
        let plain = io(PlainPath::new(&source.to_string_lossy()))?;
        psi.set_env("NATIVEMONS_CLI", &format!("import {} {}", gen1_gen(g), plain.path));
        let (o, code) = run_capture(&psi, 90);
        (last_line(&o), code)
    };
    if code != 0 { return Ok((None, Some(format!("Gen1Recomp couldn't read it: {}", slot)))); }
    let (exported, why) = gen1_export(g);
    let Some(exported) = exported else {
        gen1_slot(g, &format!("delete {} {}", gen1_gen(g), slot));
        return Ok((None, Some(why.unwrap_or_else(|| "It couldn't be read back.".into()))));
    };
    let (o, code) = save_guard::bridge(&format!("compare \"{}\" \"{}\"", source.display(), exported));
    if code != 0 {
        gen1_slot(g, &format!("delete {} {}", gen1_gen(g), slot));
        return Ok((None, Some(format!("The result didn't match, so it wasn't used: {}", o.trim().replace('\n', "; ")))));
    }
    Ok((Some(slot), None))
}

// ---------- what's running ----------
fn stem(p: &Path) -> String { p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }
fn same_path(a: &str, b: &Path) -> bool { a.to_lowercase() == b.to_string_lossy().to_lowercase() }
pub fn emu_running(all: &[proc::ProcInfo], e: &emu3ds::Emu3ds) -> bool {
    let root = format!("{}{}", e.dir().display(), std::path::MAIN_SEPARATOR).to_lowercase();
    proc::by_name(all, &stem(Path::new(e.exe))).iter().any(|p| p.exe.as_ref().map_or(true, |x| x.to_lowercase().starts_with(&root)))
}
/// RunningGames: what could be writing to a save right now ("" when nothing). The CLI has no waiting game.
/// The waiting quick-start game's and the snapshot maker's process ids (readyPid, snapPid): not running for the player.
pub static READY_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static SNAP_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub fn running_games(games: &[GameEntry]) -> String {
    use std::sync::atomic::Ordering::SeqCst;
    let (ready, snap) = (READY_PID.load(SeqCst), SNAP_PID.load(SeqCst));
    let all = proc::processes();
    let parallel = std::env::var("NATIVEMONS_TEST_PARALLEL").ok().as_deref() == Some("1");
    let me = std::process::id();
    let counts = |p: &proc::ProcInfo| !parallel || p.parent == me;
    let mut names: Vec<String> = Vec::new();
    let port = config::platinum_port();
    if proc::by_name(&all, "main").iter().any(|p| p.exe.as_deref().map_or(false, |x| same_path(x, &port)) && counts(p)) { names.push("the Platinum PC port".into()); }
    if proc::by_name(&all, "nativeplat").iter().any(|p| p.id != ready && counts(p)) { names.push("nativeplat".into()); }
    let pc_running = games.iter().any(|g| {
        g.pc.is_some() && pc_games::installed(g) && pc_games::exe(g).map_or(false, |exe| proc::by_name(&all, &stem(Path::new(&exe))).iter().any(|p| p.exe.as_deref().map_or(false, |x| same_path(x, Path::new(&exe)))))
    });
    if !parallel && pc_running { names.push("a PC game".into()); }
    // A 3DS emulator holds its game's save while it runs.
    for e in emu3ds::ALL.iter().chain(std::iter::once(&emu3ds::ZAKURO)) { if emu_running(&all, e) { names.push(e.short.into()); } }
    for n in [stem(&config::fulldex()), "lovec".into(), "love".into()] {
        if proc::by_name(&all, &n).iter().any(|p| p.id != ready && p.id != snap && counts(p)) { names.push(n); }
    }
    names.join(", ")
}

// ---------- several saves per game ----------
pub fn has_master(g: &GameEntry) -> bool { (g.fulldex_code.is_some() || g.nativeplat.is_some()) && g.save.is_some() }
fn has_gen1_slots(g: &GameEntry) -> bool { g.gen1.is_some() && g.imported }
fn parked_save(g: &GameEntry, set: &str) -> PathBuf { save_sets::dir(g, set).join("fulldex.sav") }
fn parked_sync(g: &GameEntry, set: &str) -> PathBuf { save_sets::dir(g, set).join("sync.txt") }
fn parked_slot(g: &GameEntry, set: &str) -> PathBuf { save_sets::dir(g, set).join("gen1.txt") }
fn copy_or_remove(source: &Path, target: &Path) -> R<()> {
    if source.is_file() { io(save_guard::write_in_place(source, target)) } else if target.is_file() { io(std::fs::remove_file(target)) } else { Ok(()) }
}
pub fn read_slot(g: &GameEntry, set: &str) -> Option<String> {
    let f = parked_slot(g, set);
    if f.is_file() { if let Some(v) = save_sets::read_trim(&f) { if !v.is_empty() && v != "nil" { return Some(v); } } }
    None
}
fn park(g: &GameEntry, set: &str) -> R<()> {
    io(std::fs::create_dir_all(save_sets::dir(g, set)))?;
    // A 3DS game's whole save folder, format file and states go with the slot.
    if g.ctr.is_some() { io(ctr3ds_saves::mirror(Some(&ctr3ds_saves::master(g)), &save_sets::dir(g, set).join("3ds")))?; }
    if has_master(g) { copy_or_remove(&master_of(g), &parked_save(g, set))?; }
    copy_or_remove(&sync_file(g), &parked_sync(g, set))?;
    if has_gen1_slots(g) {
        let Some(slot) = gen1_slot(g, &format!("active {}", gen1_gen(g))) else { return Err("Gen1Recomp couldn't say which of its save slots is in use.".into()) };
        io(std::fs::write(parked_slot(g, set), slot))?;
    }
    Ok(())
}
/// Puts a parked save in use. A save with nothing in it yet starts a new game in each engine.
fn bring_in(g: &GameEntry, set: &str) -> R<()> {
    if g.ctr.is_some() { io(ctr3ds_saves::mirror(Some(&save_sets::dir(g, set).join("3ds")), &ctr3ds_saves::master(g)))?; }
    if has_master(g) { copy_or_remove(&parked_save(g, set), &master_of(g))?; }
    io(std::fs::create_dir_all(sync_file(g).parent().unwrap()))?;
    copy_or_remove(&parked_sync(g, set), &sync_file(g))?;
    if has_gen1_slots(g) {
        match read_slot(g, set) {
            Some(slot) => { if gen1_slot(g, &format!("activate {} {}", gen1_gen(g), slot)).is_none() { return Err("Gen1Recomp couldn't switch to that save's slot.".into()); } }
            None => {
                let slot = gen1_slot(g, &format!("new {}", gen1_gen(g))).unwrap_or_default();
                if slot.is_empty() { return Err("Gen1Recomp couldn't make a new save slot.".into()); }
                io(std::fs::write(parked_slot(g, set), slot))?;
            }
        }
    }
    io(save_sets::set_active(g, set))
}
/// Each save is parked as it stands. The one being left is proven first, so a failure can be undone.
pub fn switch_save(games: &[GameEntry], g: &GameEntry, target: &str) -> Option<String> {
    let from = save_sets::active(g);
    if target == from { return None; }
    let running = running_games(games);
    if !running.is_empty() { return Some(format!("Close {} first, so nothing is writing to the save.", running)); }
    let first = (|| -> R<()> {
        if has_master(g) { io(save_guard::backup(g, Some(&master_of(g)), "before-switch"))?; }
        park(g, &from)
    })();
    if let Err(x) = first { return Some(format!("Nothing was changed. {}", x)); }
    let second = (|| -> R<()> {
        io(std::fs::write(save_sets::journal(g), &from))?;
        bring_in(g, target)?;
        io(std::fs::remove_file(save_sets::journal(g)))
    })();
    match second {
        Ok(()) => None,
        Err(x) => match roll_back(g, &from) {
            Ok(()) => Some(format!("You're still on your previous save. {}", x)),
            Err(y) => Some(format!("The switch failed and couldn't be undone yet. It'll try again next time the launcher starts. {} {}", x, y)),
        },
    }
}
fn roll_back(g: &GameEntry, from: &str) -> R<()> {
    bring_in(g, from)?;
    io(std::fs::remove_file(save_sets::journal(g)))
}
/// A switch cut short, by a crash or a power cut, is undone at the next start.
pub fn recover_switches(games: &[GameEntry]) {
    for g in games {
        if !save_sets::journal(g).is_file() { continue; }
        let from = save_sets::read_trim(&save_sets::journal(g)).unwrap_or_default();
        let _ = roll_back(g, &from);
    }
}
pub fn new_save(games: &[GameEntry], g: &GameEntry, name: Option<&str>) -> Option<String> {
    let set = match save_sets::next(g) { Ok(s) => s, Err(e) => return Some(e.to_string()) };
    if let Some(n) = name.filter(|n| !n.trim().is_empty()) { if let Err(e) = save_sets::set_name(g, &set, n) { return Some(e.to_string()); } }
    let problem = switch_save(games, g, &set);
    if problem.is_some() { let _ = std::fs::remove_dir_all(save_sets::dir(g, &set)); }
    problem
}
/// A separate copy, which starts its own story from here. The save in use stays in use.
pub fn duplicate_save(g: &GameEntry, source: Option<&str>) -> Option<String> {
    let mut set: Option<String> = None;
    let r = (|| -> R<()> {
        let source = source.unwrap_or("");
        let active = source == save_sets::active(g);
        let s = io(save_sets::next(g))?;
        set = Some(s.clone());
        io(save_sets::set_name(g, &s, &format!("{} copy", save_sets::name(g, source))))?;
        if has_master(g) { copy_or_remove(&if active { master_of(g) } else { parked_save(g, source) }, &parked_save(g, &s))?; }
        copy_or_remove(&if active { sync_file(g) } else { parked_sync(g, source) }, &parked_sync(g, &s))?;
        if has_gen1_slots(g) {
            let slot = if active { gen1_slot(g, &format!("active {}", gen1_gen(g))) } else { read_slot(g, source) };
            // A slot with nothing saved in it has nothing to copy, and the copy gets a new one when first used.
            if let Some(slot) = slot.filter(|x| !x.is_empty() && x != "nil") {
                if let Some(copy) = gen1_slot(g, &format!("duplicate {} {}", gen1_gen(g), slot)).filter(|c| !c.is_empty()) { io(std::fs::write(parked_slot(g, &s), copy))?; }
            }
        }
        Ok(())
    })();
    match r {
        Ok(()) => None,
        Err(x) => {
            if let Some(s) = &set { let _ = std::fs::remove_dir_all(save_sets::dir(g, s)); }
            Some(x)
        }
    }
}
/// Only a save not in use. Its Fulldex file and its Gen1Recomp slot go to its backups first, then both are removed.
pub fn delete_save(g: &GameEntry, set: Option<&str>) -> Option<String> {
    let set = set.unwrap_or("");
    if set == save_sets::active(g) { return Some("That save is in use. Switch to another one first.".into()); }
    let r = (|| -> R<Option<String>> {
        let folder = save_guard::backup_folder_of(g, set);
        let stamp = save_guard::now("yyyyMMdd-HHmmss-fff");
        let parked = parked_save(g, set);
        if parked.is_file() {
            io(std::fs::create_dir_all(&folder))?;
            io(save_guard::write_in_place(&parked, &folder.join(format!("{}-deleted.sav", stamp))))?;
        }
        let mut slot = if has_gen1_slots(g) { read_slot(g, set) } else { None };
        if let Some(s) = slot.clone() {
            // Never the slot in use, nor one another save still points at.
            if Some(s.clone()) == gen1_slot(g, &format!("active {}", gen1_gen(g))) || save_sets::all(g).iter().any(|x| x != set && read_slot(g, x).as_deref() == Some(s.as_str())) { slot = None; }
        }
        if let Some(s) = slot {
            if let Some(file) = gen1_slot_file(g, Some(&s)).filter(|f| f.is_file()) {
                io(std::fs::create_dir_all(&folder))?;
                io(save_guard::write_in_place(&file, &folder.join(format!("{}-deleted-gen1recomp.lua", stamp))))?;
            }
            if gen1_slot(g, &format!("delete {} {}", gen1_gen(g), s)).is_none() { return Ok(Some("Gen1Recomp couldn't delete that save's slot, so nothing was deleted.".into())); }
        }
        io(std::fs::remove_dir_all(save_sets::dir(g, set)))?;
        Ok(None)
    })();
    r.unwrap_or_else(Some)
}
/// Where Gen1Recomp keeps a slot, for reading what's in it.
pub fn gen1_slot_file(g: &GameEntry, slot: Option<&str>) -> Option<PathBuf> {
    let slot = slot.filter(|s| !s.is_empty() && *s != "nil")?;
    if !has_gen1_slots(g) { return None; }
    let paths = engines::gen1_cli("paths")?;
    let row = paths.split('\n').map(|x| x.trim_end_matches('\r').split('\t').map(String::from).collect::<Vec<_>>()).find(|x| x.len() == 2 && x[0] == "save")?;
    Some(PathBuf::from(win_sep(&row[1])).join("saves").join(gen1_gen(g)).join(format!("{}.lua", slot)))
}

/// The file itself is only read. If the save in use has nothing in it yet this fills that one, otherwise it's a new save.
pub fn adopt_slot(games: &[GameEntry], g: &GameEntry, file: Option<&str>, name: Option<&str>) -> Option<String> {
    let r = (|| -> R<Option<String>> {
        let running = running_games(games);
        if !running.is_empty() { return Ok(Some(format!("Close {} first, so nothing is writing to the save.", running))); }
        if !g.imported { return Ok(Some(format!("Prepare {} first.", g.title))); }
        let (current, why) = gen1_export(g);
        let empty = current.is_none() && why.is_none() && !(has_master(g) && master_of(g).is_file() && !save_guard::is_blank(&master_of(g)));
        let mut made_empty: Option<String> = None;
        let came_from = save_sets::active(g);
        if !empty {
            if let Some(problem) = new_save(games, g, name) { return Ok(Some(problem)); }
            made_empty = gen1_slot(g, &format!("active {}", gen1_gen(g)));
        } else if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
            io(save_sets::set_name(g, &save_sets::active(g), n))?;
        }
        let mut psi = gen1_start("");
        let (slot, code) = {
            let plain = io(PlainPath::new(file.unwrap_or("")))?;
            psi.set_env("NATIVEMONS_CLI", &format!("adopt {} {}", gen1_gen(g), plain.path));
            let (o, code) = run_capture(&psi, 60);
            (last_line(&o), code)
        };
        if code != 0 {
            // Back to the save the player was on, without the empty one made for this.
            if !empty {
                let made = save_sets::active(g);
                if switch_save(games, g, &came_from).is_none() {
                    if let Some(m) = made_empty.as_deref().filter(|m| !m.is_empty() && *m != "nil") { gen1_slot(g, &format!("delete {} {}", gen1_gen(g), m)); }
                    delete_save(g, Some(&made));
                }
            }
            return Ok(Some(format!("Gen1Recomp couldn't take that save: {}", slot)));
        }
        io(std::fs::write(parked_slot(g, &save_sets::active(g)), &slot))?;
        // Only the empty slot made a moment ago goes. Slots holding a save are never removed.
        if let Some(m) = made_empty.as_deref().filter(|m| !m.is_empty() && *m != "nil" && *m != slot) { gen1_slot(g, &format!("delete {} {}", gen1_gen(g), m)); }
        if shares(g) {
            let r = sync_save(g, Some("gen1"));
            if r.failed { return Ok(r.note); }
        }
        Ok(None)
    })();
    r.unwrap_or_else(Some)
}

// ---------- reading and exporting saves ----------
/// LauncherForm.Bridge: mon-bridge through RunCapture (no extra environment).
pub fn lf_bridge(args: &str) -> (String, i32) { run_capture(&proc::StartInfo::new(config::mon_bridge(), args), 60) }
pub fn save_of(g: &GameEntry) -> (Option<String>, Option<String>) {
    let no_save = || format!("No save yet for {}. Play and save once first.", g.title);
    if g.ctr.is_some() {
        let m = ctr3ds_saves::main(g);
        return if m.is_file() { (Some(m.to_string_lossy().into_owned()), None) } else { (None, Some(no_save())) };
    }
    if g.engine() != "gen1" {
        let f = config::fulldex_dir().join("saves").join(g.save.as_deref().unwrap_or(""));
        return if f.is_file() { (Some(f.to_string_lossy().into_owned()), None) } else { (None, Some(no_save())) };
    }
    let mut psi = gen1_start("");
    psi.set_env("NATIVEMONS_CLI", &format!("export {}", gen1_gen(g)));
    let (o, code) = run_capture(&psi, 60);
    let o = o.trim().to_string();
    if code == 0 && !o.is_empty() { return (Some(last_line(&o)), None); }
    (None, Some(if !o.is_empty() { o } else { no_save() }))
}
/// footer: a Gen 1 save with extra bytes after its 32 KB (an emulator's footer). Nothing was imported then.
pub fn import_back(g: &GameEntry, file: Option<&str>, trim: bool) -> (Option<String>, bool) {
    // A 3DS save the launcher just changed: its slot's states go to the backups, so an old one can't undo it.
    if g.ctr.is_some() { let _ = ctr3ds_saves::stash_states(g, "after-move"); return (None, false); }
    if g.engine() != "gen1" { return (None, false); }
    let mut psi = gen1_start("");
    let plain = match PlainPath::new(file.unwrap_or("")) { Ok(p) => p, Err(e) => return (Some(e.to_string()), false) };
    psi.set_env("NATIVEMONS_CLI", &format!("{}{} {}", if trim { "import-trim " } else { "import " }, gen1_gen(g), plain.path));
    let (o, code) = run_capture(&psi, 60);
    drop(plain);
    if code == 3 { return (Some("This save has extra bytes after the game's 32 KB, so it wasn't imported.".into()), true); }
    if code == 0 { (None, false) } else { (Some(if !o.trim().is_empty() { o.trim().to_string() } else { "Gen1Recomp could not take the save back.".into() }), false) }
}
pub fn prepare_now(g: &GameEntry, workers: i32) -> bool {
    let mut psi = gen1_start("");
    psi.set_env("POKEPORT_EXTRACT_WORKERS", &workers.to_string());
    psi.set_env("POKEPORT_IMPORT_ONLY", "1");
    psi.set_env("POKEPORT_FORCE_IMPORT", "1");
    let Ok(rom) = PlainPath::new(g.gen1_rom.as_deref().unwrap_or("")) else { return false };
    psi.set_env("POKEPORT_IMPORT_ROM", &rom.path);
    // Below whatever the player has open, so a video or game they're running keeps its CPU.
    psi.below_normal = true;
    matches!(proc::run(&psi, 300), Ok(f) if f.code == Some(0))
}
/// Copies byte for byte: the file is already in cartridge format. (message, ok)
pub fn export_save(g: &GameEntry, checkpoint: bool, out_folder: &str, backup: Option<&str>) -> (String, bool) {
    match export_inner(g, checkpoint, out_folder, backup) {
        Ok(r) => r,
        Err(e) => (e, false),
    }
}
fn export_inner(g: &GameEntry, checkpoint: bool, out_folder: &str, backup: Option<&str>) -> R<(String, bool)> {
    let (file, why) = save_of(g);
    let Some(file) = file else { return Ok((why.unwrap_or_default(), false)) };
    let (info, code) = lf_bridge(&format!("info \"{}\"", file));
    if code != 0 { return Ok((format!("This save didn't pass its checks, so it wasn't exported: {}", info.trim()), false)); }
    let file = PathBuf::from(win_sep(&file));
    let size = io(std::fs::metadata(&file))?.len();
    if !checkpoint {
        io(std::fs::create_dir_all(out_folder))?;
        let target = Path::new(out_folder).join(format!("{}.sav", g.id));
        if target.is_file() { io(std::fs::copy(&target, format!("{}.old", target.display())))?; }
        io(std::fs::copy(&file, &target))?;
        return Ok((format!("Exported to:\n{}\n\nThis is a plain cartridge save ({} bytes). Rename it to match your ROM if your emulator or flash cart needs that.", target.display(), size), true));
    }
    let mut file_name_ = "sav.dat".to_string();
    let mut secure: Option<PathBuf> = None;
    let console = g.console.as_deref().unwrap_or("");
    if g.is_ds() {
        let Some(b) = backup else { return Ok(("DS saves need a Checkpoint backup of this game, because Checkpoint names the file after the game on your 3DS. Back the game up in Checkpoint, copy that folder to this PC and choose it.".into(), false)) };
        let first = std::fs::read_dir(b).ok().and_then(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.to_string_lossy().to_lowercase().ends_with(".sav")).map(|p| file_name(&p.to_string_lossy())).next());
        match first { Some(n) => file_name_ = n, None => return Ok(("That folder has no .sav file, so it isn't a Checkpoint backup of a DS game.".into(), false)) }
    } else if console != "Game Boy" && console != "Game Boy Color" {
        return Ok(("Game Boy Advance games aren't on the 3DS as Virtual Console, so Checkpoint can't restore them. Use a plain .sav for flash carts or open_agb_firm.".into(), false));
    }
    if let Some(b) = backup {
        let reference = Path::new(b).join(&file_name_);
        if !reference.is_file() { return Ok((format!("That folder has no {}, so it isn't a Checkpoint backup of this game.", file_name_), false)); }
        let expected = io(std::fs::metadata(&reference))?.len();
        if expected != size { return Ok((format!("Sizes don't match, so nothing was exported: your 3DS backup is {} bytes and this save is {}. It could report as corrupted on the 3DS. Check that the backup is of the same game and region.", expected, size), false)); }
        let sv = Path::new(b).join("SecureValue");
        if sv.is_file() { secure = Some(sv); }
    }
    // Virtual Console saves are 32 KB, plus a 16-byte clock on Crystal. Anything longer carries another emulator's clock.
    if !g.is_ds() && size != 32768 && size != 32784 {
        return Ok((format!("This save came from another emulator: at {} bytes it has that emulator's clock on the end, and Virtual Console expects 32768 or 32784. It could report as corrupted on the 3DS, so nothing was exported. A plain .sav still works everywhere else.", size), false));
    }
    let folder = Path::new(out_folder).join(format!("nativeMons-{}", g.id));
    io(std::fs::create_dir_all(&folder))?;
    io(std::fs::copy(&file, folder.join(&file_name_)))?;
    if let Some(s) = &secure { io(std::fs::copy(s, folder.join("SecureValue")))?; }
    Ok((format!("Exported to:\n{}\n\nTo restore it:\n1. Over FTP, copy this folder to /3ds/Checkpoint/saves/<your game's folder>/, with the FTP client in binary mode.\n2. Check {} on the 3DS is {} bytes.\n3. In Checkpoint, select this backup, choose Restore, then start the game.{}\n\nBack up your 3DS save first.", folder.display(), file_name_, size, if backup.is_none() { "\n\nNo 3DS backup was given to compare with, so the size wasn't checked." } else { "" }), true))
}

// ---------- the Pokedex of a game's saves ----------
#[derive(Clone, Debug)]
pub struct DexGame {
    pub caught: BTreeSet<i32>,
    pub seen: BTreeSet<i32>,
    pub shiny: BTreeSet<i32>,
    /// Infinite Fusion only (-1 elsewhere): fusions caught and seen, and Pokemon past the National Dex list.
    pub fusions_caught: i32,
    pub fusions_seen: i32,
    pub later_caught: i32,
    pub later_seen: i32,
}
impl Default for DexGame {
    fn default() -> Self { DexGame { caught: BTreeSet::new(), seen: BTreeSet::new(), shiny: BTreeSet::new(), fusions_caught: -1, fusions_seen: -1, later_caught: 0, later_seen: 0 } }
}
fn add_dex(into: &mut DexGame, file: Option<&str>) -> bool {
    let Some(file) = file.filter(|f| Path::new(f).is_file() && !save_guard::is_blank(Path::new(f))) else { return false };
    let (o, code) = lf_bridge(&format!("dex \"{}\"", file));
    if code != 0 { return false; }
    for line in o.split('\n') {
        let Some(at) = line.find(':') else { continue };
        let set = if line.starts_with("caught") { &mut into.caught } else if line.starts_with("seen") { &mut into.seen } else if line.starts_with("shiny") { &mut into.shiny } else { continue };
        for id in line[at + 1..].trim().split(',') { if let Ok(n) = id.trim().parse::<i32>() { set.insert(n); } }
    }
    true
}
fn parked_ctr_save(g: &GameEntry, set: &str) -> PathBuf { save_sets::dir(g, set).join("3ds").join("data").join("main") }
/// Each Pokemon once, however many of a game's saves have it. None when none of them could be read.
pub fn dex_in_all_saves(games: &[GameEntry], g: &GameEntry) -> Option<DexGame> {
    let mut all = DexGame::default();
    let mut any = false;
    let active = save_sets::active(g);
    for set in save_sets::all(g) {
        if set == active { let (f, _) = save_of(g); any |= add_dex(&mut all, f.as_deref()); continue; }
        if has_master(g) && parked_save(g, &set).is_file() { any |= add_dex(&mut all, Some(&parked_save(g, &set).to_string_lossy())); continue; }
        if g.ctr.is_some() { if parked_ctr_save(g, &set).is_file() { any |= add_dex(&mut all, Some(&parked_ctr_save(g, &set).to_string_lossy())); } continue; }
        // A Gen1Recomp slot can only be exported while it's in use, so it's briefly made so, unless a game is running.
        let Some(slot) = read_slot(g, &set) else { continue };
        if !has_gen1_slots(g) { continue; }
        if !running_games(games).is_empty() { continue; }
        let Some(current) = gen1_slot(g, &format!("active {}", gen1_gen(g))) else { continue };
        if gen1_slot(g, &format!("activate {} {}", gen1_gen(g), slot)).is_none() { continue; }
        let (f, _) = gen1_export(g);
        any |= add_dex(&mut all, f.as_deref());
        gen1_slot(g, &format!("activate {} {}", gen1_gen(g), current));
    }
    if any { Some(all) } else { None }
}
pub fn caught_in_all_saves(games: &[GameEntry], g: &GameEntry) -> Option<Vec<i32>> { dex_in_all_saves(games, g).map(|d| d.caught.into_iter().collect()) }
