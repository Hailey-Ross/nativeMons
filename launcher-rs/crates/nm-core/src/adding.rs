//! Adding games without the window (LauncherForm.AddPaths, WhatRomIs, Unzip; Library.WhyNotPlayable): folders become
//! ROM folders, ROMs are used in place, a zip's ROMs are copied out into state\roms\<zip name>.

use crate::config;
use crate::emu3ds;
use crate::engines::gen1_cli;
use crate::game_entry::GameEntry;
use crate::library::{self, file_name, Library};
use sha1::{Digest, Sha1};
use std::path::Path;

fn has_ci(l: &[String], s: &str) -> bool { l.iter().any(|x| x.to_lowercase() == s.to_lowercase()) }

/// The work of a drop, apart from the window. Returns the library scanned afterwards.
pub fn add_paths(folders: &[String], roms: &[String], zips: &[String], added: &mut Vec<String>, notes: &mut Vec<String>) -> Result<Library, String> {
    if !folders.is_empty() {
        let mut l = config::rom_folders();
        for f in folders { if !has_ci(&l, f) { l.push(f.clone()); } }
        config::save_rom_folders(&l);
    }
    if !roms.is_empty() {
        let mut l = config::rom_files();
        for f in roms {
            let full = crate::paths::full_path(Path::new(f)).to_string_lossy().into_owned();
            // Already inside a ROM folder: it's found there, so it needn't be listed on its own.
            let covered = config::rom_folders().iter().any(|dir| {
                let d = crate::paths::full_path(Path::new(dir)).to_string_lossy().trim_end_matches('\\').to_string();
                full.to_lowercase().starts_with(&format!("{}\\", d).to_lowercase())
            });
            if !covered && !has_ci(&l, &full) { l.push(full.clone()); }
            added.push(full);
        }
        config::save_rom_files(&l);
    }
    for z in zips {
        if let Some(problem) = unzip(z, added) { notes.push(format!("{}: {}", file_name(z), problem)); }
    }
    let lib = Library::scan(&gen1_cli)?;
    // A ROM no game here uses isn't kept in the list: it would only be read again at every start.
    if !roms.is_empty() {
        let used: Vec<String> = lib.games.iter().flat_map(|g| g.roms.iter().map(|r| r.file.to_lowercase())).collect();
        let mut l = config::rom_files();
        let before = l.len();
        l.retain(|f| !(has_ci(added, f) && !used.contains(&f.to_lowercase())));
        if l.len() < before { config::save_rom_files(&l); }
    }
    Ok(lib)
}

pub fn why_not_playable(lib: &Library, g: &GameEntry) -> String {
    if g.pc.as_deref() == Some("custom") { return "Its program has moved or been deleted.".into(); }
    if g.pc.is_some() { return "Not installed yet. Select it to install.".into(); }
    if g.ctr.is_some() {
        let Some(rom) = &g.ctr_rom else { return "No ROM found. Add the folder with your .3ds dump under ROM folders.".into() };
        if !emu3ds::enabled() { return format!("Found {}. Turn on 3DS games under Emu on the home screen to play it.", file_name(rom)); }
        if g.ctr_encrypted && emu3ds::default().is_some() { return format!("Found {}, an encrypted dump. Azahar can't open it, but AzaharPlus can: install AzaharPlus under Emu on the home screen, and this game plays with it.", file_name(rom)); }
        return format!("Found {}. Install an emulator under Emu on the home screen to play it.", file_name(rom));
    }
    let rom = g.fulldex_rom.as_ref().or(g.found_unsupported.as_ref());
    let can_run = (g.gen1.is_some() && lib.gen1_found) || (g.fulldex_code.is_some() && config::fulldex().is_file()) || (g.nativeplat.is_some() && config::nativeplat().is_file());
    if let Some(r) = rom { if !can_run { return format!("Found {}, but this copy of nativeMons doesn't have the engine to run it. Use the full download from the Releases page.", file_name(r)); } }
    if let Some(u) = &g.found_unsupported { return format!("Found {}, but it is not a version this game's engine accepts. Its SHA-1 is {}. The README lists the accepted ones.", file_name(u), g.found_sha1.as_deref().unwrap_or("")); }
    "No ROM found. Add its folder under ROM folders.".into()
}

/// Which game a ROM turned out to be, or why it can't be used, with its SHA-1 when nothing knows it.
pub fn what_rom_is(lib: &Library, file: &str) -> String {
    for g in &lib.games {
        if !g.roms.iter().any(|r| r.file.to_lowercase() == file.to_lowercase()) { continue; }
        if g.playable() { return format!("{}, ready to play.{}", g.title, if g.roms.len() > 1 { " With more than one copy, its ROM version page picks which." } else { "" }); }
        return format!("{}. {}", g.title, why_not_playable(lib, g));
    }
    if library::is_3ds(file) {
        let c = library::read_ctr(file).ok().flatten();
        return match c { None => "not a 3DS cartridge dump nativeMons can read, so it wasn't added.".into(), Some(c) => format!("not one of the Pokemon 3DS games nativeMons runs (title ID {}), so it wasn't added.", c.title_id) };
    }
    let sha = std::fs::read(file).map(|d| Sha1::digest(&d).iter().map(|b| format!("{:02x}", b)).collect::<String>()).unwrap_or_default();
    // The same dump found somewhere else first: the list keeps one file per dump.
    for g in &lib.games {
        if let Some(same) = g.roms.iter().find(|r| r.sha1 == sha) { return format!("{}. You already have this exact ROM: {}", g.title, same.file); }
    }
    format!("not one of the games nativeMons runs{}, so it wasn't added. The README lists the versions each engine accepts.", if !sha.is_empty() { format!(" (SHA-1 {})", sha) } else { String::new() })
}

/// Copies a zip's ROMs out into state\roms\<zip name>. Other files in it are left alone.
fn unzip(zip: &str, added: &mut Vec<String>) -> Option<String> {
    let r = (|| -> Result<usize, String> {
        let stem = Path::new(zip).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let to = config::unzipped().join(stem);
        let mut archive = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut n = 0;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let full = entry.name().replace('\\', "/");
            let name = full.rsplit('/').next().unwrap_or("").to_string();
            // 3DS dumps are played in place, never copied out of a zip (they're 2-4 GB).
            if name.is_empty() || !library::is_rom(&name) || library::is_3ds(&name) { continue; }
            std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            let target = to.join(&name);
            n += 1;
            added.push(target.to_string_lossy().into_owned());
            if target.is_file() && std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0) == entry.size() { continue; }
            let part = std::path::PathBuf::from(format!("{}.part", target.display()));
            drop(entry);
            crate::zips::extract_entry(&mut archive, i, &part)?;
            if target.is_file() { std::fs::remove_file(&target).map_err(|e| e.to_string())?; }
            std::fs::rename(&part, &target).map_err(|e| e.to_string())?;
        }
        Ok(n)
    })();
    match r {
        Ok(0) => Some("no .gb, .gbc, .gba or .nds ROM inside.".into()),
        Ok(_) => None,
        Err(x) => Some(format!("couldn't be opened ({}).", x)),
    }
}
