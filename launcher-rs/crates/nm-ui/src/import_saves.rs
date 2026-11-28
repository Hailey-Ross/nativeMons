//! Importing saves, line by line from the C#: FoundSave, Identify (mon-bridge info), IdentifySlot, GameByName,
//! OpenImportSaves, SaveRow, ImportSaveFiles (dropped .sav files), ChooseSaveFile, AddSaveFolder, ImportSave, AskTrim,
//! CopyInSave, ImportDone; and RestoreBackup. The player's file is only ever read.

use crate::launcher::Launcher;
use crate::model::{Item, Page};
use crate::pages::{engine_name, theme_of};
use nm_core::game_entry::GameEntry;
use nm_core::save_info::SaveInfo;
use nm_core::{config, save_guard, saves};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct FoundSave { pub file: String, pub note: Option<String>, pub game: Option<String>, pub slot: bool, pub info: Option<SaveInfo> }

fn file_name(f: &str) -> String { Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() }
fn parent(f: &str) -> String { Path::new(f).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default() }
fn stem_path(f: &str) -> String { Path::new(&parent(f)).join(Path::new(f).file_stem().unwrap_or_default()).to_string_lossy().into_owned() }

/// What PKHeX calls each kind of save, and the games it can belong to.
fn save_kinds(k: &str) -> Option<&'static [&'static str]> {
    Some(match k {
        "RD" => &["red"], "GN" | "BU" => &["blue"], "YW" => &["yellow"], "RB" => &["red", "blue"], "RBY" => &["red", "blue", "yellow"],
        "GD" => &["gold"], "SI" => &["silver"], "GS" => &["gold", "silver"], "C" => &["crystal"],
        "R" => &["ruby"], "S" => &["sapphire"], "RS" => &["ruby", "sapphire"], "E" => &["emerald"], "FR" => &["firered"], "LG" => &["leafgreen"], "FRLG" => &["firered", "leafgreen"],
        "HG" => &["heartgold"], "SS" => &["soulsilver"], "HGSS" => &["heartgold", "soulsilver"], "Pt" => &["platinum"],
        "B" => &["black"], "W" => &["white"], "BW" => &["black", "white"], "B2" => &["black2"], "W2" => &["white2"], "B2W2" => &["black2", "white2"],
        _ => return None,
    })
}
/// GameByName: the game a save's name or neighboring ROM points to.
fn game_by_name<'a>(games: &'a [GameEntry], file: &str) -> Option<&'a GameEntry> {
    let stem = stem_path(file);
    for g in games {
        for rom in [&g.gen1_rom, &g.fulldex_rom, &g.nativeplat_rom, &g.found_unsupported].into_iter().flatten() {
            if stem_path(rom).eq_ignore_ascii_case(&stem) { return Some(g); }
        }
    }
    let low = Path::new(file).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default().replace(" version", "");
    let name = regex::Regex::new("[^a-z0-9]+").unwrap().replace_all(&low, " ").into_owned();
    let padded = format!(" {} ", name);
    let mut best: Option<&GameEntry> = None;
    for g in games.iter().filter(|g| padded.contains(&format!(" {} ", g.title.replace("Pokemon ", "").to_lowercase()))) {
        if best.map_or(true, |b| g.title.encode_utf16().count() > b.title.encode_utf16().count()) { best = Some(g); }
    }
    best
}
/// Identify: reads the save itself. When its contents and its name disagree, its contents win, and the player is told.
pub fn identify(games: &[GameEntry], file: &str) -> Option<FoundSave> {
    let (o, code) = save_guard::bridge(&format!("info \"{}\"", file));
    if code != 0 { return None; }
    let f: Vec<&str> = o.trim().split('\t').collect();
    if f.len() < 3 { return None; }
    let named = game_by_name(games, file);
    let Some(kinds) = save_kinds(f[1]) else { return Some(FoundSave { file: file.into(), note: Some(format!("This is a {} save, and no game here can use it.", f[1])), ..Default::default() }) };
    if let Some(n) = named { if kinds.contains(&n.base_id.as_deref().unwrap_or(&n.id)) { return Some(FoundSave { file: file.into(), game: Some(n.id.clone()), ..Default::default() }); } }
    let game = kinds.iter().filter_map(|id| games.iter().find(|g| &g.id == id)).find(|g| g.playable()).or_else(|| games.iter().find(|g| g.id == kinds[0]));
    let mut save = FoundSave { file: file.into(), game: game.map(|g| g.id.clone()), ..Default::default() };
    if let Some(n) = named { save.note = Some(format!("It's named for {}, but it's really a {} save.", n.title, game.map(|g| g.title.as_str()).unwrap_or(f[1]))); }
    Some(save)
}
/// IdentifySlot: Gen1Recomp's own slot files, as another install keeps them (saves\<version>\slot<n>.lua).
pub fn identify_slot(games: &[GameEntry], file: &str) -> Option<FoundSave> {
    if !regex::Regex::new(r"(?i)^slot\d+\.lua$").unwrap().is_match(&file_name(file)) { return None; }
    let version = file_name(&parent(file));
    let g = games.iter().find(|g| g.gen1.as_deref().map_or(false, |v| v.eq_ignore_ascii_case(&version)))?;
    Some(FoundSave { file: file.into(), game: Some(g.id.clone()), slot: true, ..Default::default() })
}
/// SaveFolders: the ROM folders and the player's extra save folders.
pub fn save_folders() -> Vec<String> {
    let mut folders = config::rom_folders();
    if let Ok(t) = std::fs::read_to_string(config::state().join("save-folders.txt")) { folders.extend(t.lines().filter(|x| !x.trim().is_empty()).map(|x| x.to_string())); }
    let mut out: Vec<String> = Vec::new();
    for f in folders { if !out.iter().any(|o| o.eq_ignore_ascii_case(&f)) { out.push(f); } }
    out
}
/// Every .sav under a folder (Directory.GetFiles AllDirectories, case-insensitive extension).
fn sav_files_under(folder: &str) -> Vec<String> {
    nm_core::library::files_under(folder).into_iter().filter(|f| nm_core::library::ext_lower(f) == ".sav").collect()
}
/// OpenImportSaves' search: .sav files (not backups) and Gen1Recomp slots in the save folders.
pub fn find_saves(games: &[GameEntry]) -> Vec<FoundSave> {
    let mut found = Vec::new();
    for folder in save_folders() {
        if !Path::new(&folder).is_dir() { continue; }
        for file in sav_files_under(&folder) {
            if file.to_lowercase().contains("backup") { continue; }
            if let Some(mut s) = identify(games, &file) { s.info = nm_core::save_info::read_save_info(Path::new(&file)); found.push(s); }
        }
        for file in nm_core::library::files_under(&folder) {
            if let Some(mut s) = identify_slot(games, &file) { s.info = nm_core::save_info::read_save_info(Path::new(&file)); found.push(s); }
        }
    }
    found
}
/// The slots alone, as the import snapshot lists them.
pub fn find_slots(games: &[GameEntry]) -> Vec<FoundSave> {
    let mut found = Vec::new();
    for folder in save_folders() {
        for file in nm_core::library::files_under(&folder) {
            if let Some(mut s) = identify_slot(games, &file) { s.info = nm_core::save_info::read_save_info(Path::new(&file)); found.push(s); }
        }
    }
    found
}
/// SaveRow: what's in the save comes first, as that's how the player tells two of the same game apart.
pub fn save_row(games: &[GameEntry], one: &FoundSave) -> Item {
    let game = one.game.as_ref().and_then(|id| games.iter().find(|g| &g.id == id));
    let ready = game.map_or(false, |g| g.playable());
    let title = game.map(|g| g.title.clone()).unwrap_or_else(|| "Unknown game".into());
    let mut info = if !ready { if game.is_none() { String::new() } else { "Its ROM isn't in your ROM folders, so there's nowhere to put it yet. ".into() } }
        else { let g = game.unwrap(); format!("Copies it to {}. The file here isn't changed. ", engine_name(&g.engine(), Some(g))) };
    match &one.info {
        Some(i) => info = format!("{}{}{}", i.details(nm_core::save_info::write_time(Path::new(&one.file))), one.note.as_ref().map(|n| format!("\n{}", n)).unwrap_or_default(), if ready { String::new() } else { format!("\n{}", info.trim()) }),
        None => info.push_str(one.note.as_deref().unwrap_or("")),
    }
    let o = one.clone();
    let mut it = Item::new(&title, "", ready).icon('\u{E896}').info(&info).run(move |l| l.import_save(o.clone()));
    it.tile_hint = one.info.as_ref().map(|i| i.tile_line()).unwrap_or_default();
    it.records = vec![format!("File: {}", file_name(&one.file)), format!("Folder: {}", parent(&one.file))];
    if let Some(g) = game { it.theme = Some(theme_of(g)); }
    it
}

impl Launcher {
    /// OpenImportSaves: the save folders searched on a Busy card, then the list with Choose a file and Add a save folder.
    pub fn open_import_saves(&mut self) {
        let games = self.ctx().lib.games.clone();
        self.busy("Looking for saves", move || Ok(find_saves(&games)), |l, r| {
            let found = match r { Ok(f) => f, Err(e) => { l.show_text("Import saves", &e); return; } };
            let mut p = Page::new("Import saves");
            for one in &found { let it = save_row(&l.ctx().lib.games, one); p.items.push(it); }
            p.items.push(Item::new("Choose a file", "", true).icon('\u{E8E5}').info("Pick any .sav file. It's read to work out which game it belongs to.").run(|l| l.choose_save_file()));
            p.items.push(Item::new("Add a save folder", "", true).icon('\u{E710}').info("Also search another folder, such as another emulator's saves. Your ROM folders are always searched.").run(|l| l.add_save_folder()));
            l.push(p);
        });
    }
    /// ChooseSaveFile.
    fn choose_save_file(&mut self) {
        self.pick_file("Choose a save", "Cartridge save (*.sav)|*.sav|All files (*.*)|*.*", Box::new(|l, file| {
            let Some(file) = file else { return };
            let games = l.ctx().lib.games.clone();
            l.busy("Reading the save", move || Ok(identify(&games, &file)), |l, r| {
                let save = match r { Ok(Some(s)) => s, Ok(None) => { l.show_text("Can't read that save", "PKHeX doesn't recognise this file as a save."); return; } Err(e) => { l.show_text("Can't read that save", &e); return; } };
                if !save.game.as_ref().and_then(|id| l.game(id)).map_or(false, |g| g.playable()) { l.show_text("Can't import that save", &format!("{} Its game isn't in your ROM folders.", save.note.clone().unwrap_or_default())); return; }
                l.import_save(save);
            });
        }));
    }
    /// AddSaveFolder.
    fn add_save_folder(&mut self) {
        self.pick_folder("Choose a folder with your saves", Box::new(|l, dir| {
            let Some(dir) = dir else { return };
            let _ = std::fs::create_dir_all(config::state());
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(config::state().join("save-folders.txt")) { let _ = write!(f, "{}\r\n", dir); }
            l.pages.pop();
            l.open_import_saves();
        }));
    }
    /// ImportSaveFiles: one dropped save goes straight to importing, as Choose a file does; several get a list.
    pub fn import_save_files(&mut self, files: Vec<String>) {
        if !config::mon_bridge().is_file() { self.show_text("Can't import saves", "This copy of nativeMons is missing its save tools."); return; }
        let games = self.ctx().lib.games.clone();
        let n = files.len();
        self.busy("Reading the saves", move || {
            let mut found = Vec::new();
            for f in &files {
                if let Some(mut one) = identify(&games, f).or_else(|| identify_slot(&games, f)) { one.info = nm_core::save_info::read_save_info(Path::new(f)); found.push(one); }
            }
            Ok(found)
        }, move |l, r| {
            let found = match r { Ok(f) => f, Err(e) => { l.show_text("Can't read that save", &e); return; } };
            if found.is_empty() { l.show_text("Can't read that save", &format!("PKHeX doesn't recognise {} as a save.", if n == 1 { "this file" } else { "these files" })); return; }
            if found.len() == 1 && n == 1 {
                let one = found[0].clone();
                if !one.game.as_ref().and_then(|id| l.game(id)).map_or(false, |g| g.playable()) {
                    l.show_text("Can't import that save", format!("{} Its game isn't in your ROM folders.", one.note.clone().unwrap_or_default()).trim());
                    return;
                }
                l.import_save(one);
                return;
            }
            let mut p = Page::new("Import saves");
            for one in &found { let it = save_row(&l.ctx().lib.games, one); p.items.push(it); }
            l.push(p);
        });
    }
    /// ImportSave: a slot is adopted (named first), a Gen1Recomp-only game takes it as a new slot, else it's copied
    /// to Fulldex's save, the old one backed up first.
    pub fn import_save(&mut self, one: FoundSave) {
        let Some(g) = one.game.as_ref().and_then(|id| self.game(id)) else { return };
        if one.slot {
            let start = format!("{} {}", g.title.replace("Pokemon ", ""), nm_core::save_info::write_time(Path::new(&one.file)).format("%-d %b"));
            let (id, file) = (g.id.clone(), one.file.clone());
            self.ask_text("Name this save", "Name for this save:", &start, Box::new(move |l, name| {
                let Some(name) = name else { return };
                let Some(g) = l.game(&id) else { return };
                let (games, gw, file, title) = (l.ctx().lib.games.clone(), g.clone(), file.clone(), g.title.clone());
                let adopt = move |l: &mut Launcher| {
                    let id = gw.id.clone();
                    l.busy(&format!("Importing {}", title), move || Ok(saves::adopt_slot(&games, &gw, Some(&file), Some(&name))), move |l, r| {
                        l.replace_main();
                        let problem = match r { Ok(p) => p, Err(e) => Some(e) };
                        if let Some(g) = l.game(&id) { l.import_done(&g, problem.as_deref()); }
                    });
                };
                if !g.imported { l.import(&g, Some(Box::new(adopt))); } else { adopt(l); }
            }));
            return;
        }
        if g.engine() == "gen1" && !saves::shares(&g) {
            // A new save slot: anything already there is kept.
            let (id, file) = (g.id.clone(), one.file.clone());
            if !g.imported { self.import(&g, Some(Box::new(move |l| l.import_back_run(&id, &file, false)))); } else { self.import_back_run(&id, &file, false); }
            return;
        }
        let target = config::fulldex_dir().join("saves").join(g.save.as_deref().unwrap_or("")).to_string_lossy().into_owned();
        if !Path::new(&target).is_file() { self.copy_in_save(&g, &one.file, &target); return; }
        let mut p = Page::new(&format!("Replace your {} save?", g.title));
        p.body = Some(format!("There's already a Fulldex save for this game. It'll be copied to save-backups first, so it can be restored.\n\n{}", one.note.clone().unwrap_or_default()));
        p.theme = Some(theme_of(&g));
        p.items.push(Item::new("Keep it", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
        let (id, file) = (g.id.clone(), one.file.clone());
        p.items.push(Item::new("Replace it", "", true).icon('\u{E896}').run(move |l| {
            l.pages.pop();
            if let Some(g) = l.game(&id) { let t = target.clone(); l.copy_in_save(&g, &file, &t); }
        }));
        self.push(p);
    }
    /// ImportSave's run(trim): ImportBack on a Busy card; a footer asks first (AskTrim).
    fn import_back_run(&mut self, id: &str, file: &str, trim: bool) {
        let Some(g) = self.game(id) else { return };
        let (gw, f) = (g.clone(), file.to_string());
        let (id, file) = (id.to_string(), file.to_string());
        self.busy(&format!("Importing {}", g.title), move || Ok(saves::import_back(&gw, Some(&f), trim)), move |l, r| {
            let Some(g) = l.game(&id) else { return };
            match r {
                Ok((_, true)) => l.ask_trim(&g, &file),
                Ok((problem, false)) => l.import_done(&g, problem.as_deref()),
                Err(e) => l.import_done(&g, Some(&e)),
            }
        });
    }
    /// AskTrim: a Gen 1 save bigger than 32 KB; the extra bytes are an emulator's footer.
    fn ask_trim(&mut self, g: &GameEntry, file: &str) {
        let size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
        let mut p = Page::new("This save has extra bytes");
        p.theme = Some(theme_of(g));
        p.body = Some(format!("This {} save is {} bytes. A Game Boy save is 32,768 bytes; the rest is usually a footer an emulator added, which {} doesn't use.\n\nIt can be imported without the extra bytes. Your file isn't changed.", g.title, group(size), g.title));
        let (id, f) = (g.id.clone(), file.to_string());
        p.items.push(Item::new("Import without the extra bytes", "", true).icon('\u{E896}').run(move |l| { l.pages.pop(); l.import_back_run(&id, &f, true); }));
        p.items.push(Item::new("Don't import", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
        self.push(p);
    }
    /// CopyInSave: the old Fulldex save to save-backups (yyyyMMdd-HHmmss-replaced-<name>), then the file copied in.
    fn copy_in_save(&mut self, g: &GameEntry, file: &str, target: &str) {
        let r = (|| -> std::io::Result<()> {
            let t = Path::new(target);
            if t.is_file() {
                let backups = config::state().join("save-backups");
                std::fs::create_dir_all(&backups)?;
                std::fs::copy(t, backups.join(format!("{}-replaced-{}", save_guard::now("yyyyMMdd-HHmmss"), file_name(target))))?;
            }
            std::fs::create_dir_all(t.parent().unwrap())?;
            std::fs::copy(file, t)?;
            Ok(())
        })();
        self.import_done(g, r.err().map(|e| e.to_string()).as_deref());
    }
    fn import_done(&mut self, g: &GameEntry, problem: Option<&str>) {
        let body = problem.map(|p| p.to_string()).unwrap_or_else(|| format!("Your save is ready in {}. It now counts in My Collection and Move Pokemon.", g.title));
        self.show_text(if problem.is_none() { "Imported" } else { "Not imported" }, &body);
    }
    /// RestoreBackup: the backup checked, the save in use backed up, then the backup written in place (Gen1Recomp's
    /// slot taken back, shared saves brought level). A replay notes it unless NATIVEMONS_TEST_SAVE_FLOWS=1.
    pub fn restore_backup(&mut self, id: &str, file: &str, done: Option<crate::model::Act>) {
        if self.hooks.replay && std::env::var("NATIVEMONS_TEST_SAVE_FLOWS").as_deref() != Ok("1") { self.todo("RestoreBackup"); return; }
        let Some(g) = self.game(id) else { return };
        let (gw, f) = (g.clone(), file.to_string());
        self.busy("Restoring your save", move || Ok(restore_work(&gw, &f)), move |l, r| {
            let problem = match r { Ok(p) => p, Err(e) => Some(e) };
            let ok = problem.is_none();
            l.show_text(if ok { "Restored" } else { "Not restored" }, &problem.unwrap_or_else(|| format!("Your {} save is back to that backup.", g.title)));
            if let Some(d) = &done { d.call(l); }
        });
    }
}

/// RestoreBackup's work (its try block).
fn restore_work(g: &GameEntry, file: &str) -> Option<String> {
    let r = (|| -> Result<Option<String>, String> {
        if let Some(p) = save_guard::problem_for(Some(g), Path::new(file)) { return Ok(Some(format!("That backup failed its check: {}", p))); }
        let (current, _) = saves::save_of(g);
        save_guard::backup(g, current.as_deref().map(Path::new), "before-restore").map_err(|e| e.to_string())?;
        // A 3DS slot's newer states go to the backups too, so loading one can't undo the restore.
        if g.ctr.is_some() { let _ = nm_core::ctr3ds_saves::stash_states(g, "before-restore"); }
        if g.engine() != "gen1" || saves::shares(g) { save_guard::write_in_place(Path::new(file), &saves::master_of(g)).map_err(|e| e.to_string())?; }
        let mut problem = None;
        if g.engine() == "gen1" { problem = saves::import_back(g, Some(file), false).0; }
        if problem.is_none() && saves::shares(g) { let s = saves::sync_save(g, Some("fulldex")); problem = if s.failed { s.note } else { None }; }
        Ok(problem)
    })();
    r.unwrap_or_else(Some)
}

/// ToString("N0"): digits grouped by commas.
fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); } out.push(c); }
    out
}
