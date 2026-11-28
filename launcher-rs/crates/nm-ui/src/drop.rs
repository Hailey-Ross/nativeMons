//! AddRomFile and TakeDrop, line by line from the C#: ROMs chosen in the launcher's own browser (or dropped on the
//! window) are used where they are, folders become ROM folders, a zip's ROMs are copied out; saves are for the import.

use crate::launcher::Launcher;
use crate::model::Item;
use std::path::Path;

impl Launcher {
    /// AddRomFile: the browser, several at once, then TakeDrop.
    pub fn add_rom_file(&mut self) {
        if self.hooks.replay { self.todo("AddRomFile"); return; }
        self.browse(crate::pages::BrowseRequest { title: "Choose ROMs".into(), multi: true, kinds: crate::pages::ROM_KINDS.to_vec(),
            hint: "Open the folder with your ROM and select it. Select several to add them together. A .zip works too: its ROMs are copied out.".into(), done: "addrom".into(), ..Default::default() });
    }
    /// CanTakeDrop.
    pub fn can_take_drop(&self) -> bool { !self.job && !self.starting && self.pages.last().map_or(false, |p| p.boxes.is_none()) }
    /// TakeDrop: nothing dropped changes; what each ROM turned out to be is said after.
    pub fn take_drop(&mut self, paths: Vec<String>) {
        if !self.can_take_drop() { return; }
        let ext = |x: &str| Path::new(x).extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default();
        let folders: Vec<String> = paths.iter().filter(|x| Path::new(x).is_dir()).cloned().collect();
        let roms: Vec<String> = paths.iter().filter(|x| Path::new(x).is_file() && nm_core::library::is_rom(x)).cloned().collect();
        let zips: Vec<String> = paths.iter().filter(|x| Path::new(x).is_file() && ext(x) == ".zip").cloned().collect();
        let saves: Vec<String> = paths.iter().filter(|x| Path::new(x).is_file() && ext(x) == ".sav").cloned().collect();
        let other: Vec<String> = paths.iter().filter(|x| !folders.contains(x) && !roms.contains(x) && !zips.contains(x) && !saves.contains(x)).cloned().collect();
        let name = |x: &String| Path::new(x).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if folders.len() + roms.len() + zips.len() == 0 {
            if !saves.is_empty() { self.import_save_files(saves); return; }
            let packed = other.iter().any(|x| ext(x) == ".7z" || ext(x) == ".rar");
            let list = other.iter().map(name).collect::<Vec<_>>().join(", ");
            self.show_text("Can't use that", &format!("{}\n\nDrop a folder of ROMs, a .gb, .gbc, .gba, .nds or .3ds ROM, a .zip of ROMs, or a .sav save.{}", list,
                if packed { "\n\n.7z and .rar can't be opened here: extract it first, then drop the ROM." } else { "" }));
            return;
        }
        let (f2, r2, z2) = (folders.clone(), roms.clone(), zips.clone());
        self.busy("Adding your games", move || {
            let (mut added, mut notes) = (Vec::new(), Vec::new());
            let lib = nm_core::adding::add_paths(&f2, &r2, &z2, &mut added, &mut notes)?;
            Ok((lib, added, notes))
        }, move |l, r| {
            let (lib, added, notes) = match r { Ok(x) => x, Err(e) => { l.replace_main(); l.show_text("Something went wrong", &e); return; } };
            let lines: Vec<String> = folders.iter().map(|f| format!("Folder added: {}", f))
                .chain(added.iter().map(|f| format!("{}: {}", name(f), nm_core::adding::what_rom_is(&lib, f))))
                .chain(notes.iter().cloned())
                .chain(if other.is_empty() { None } else { Some(format!("Not used: {}", other.iter().map(name).collect::<Vec<_>>().join(", "))) })
                .collect();
            let (have, all) = (lib.games.iter().filter(|g| g.playable()).count(), lib.games.len());
            // PickCurrent with the library read again.
            let cur = l.current_game().map(|g| g.id.clone());
            if let Some(c) = l.ctx.as_mut() {
                c.lib = lib;
                c.current = cur.and_then(|id| c.lib.games.iter().position(|g| g.id == id && g.playable())).or_else(|| c.lib.games.iter().position(|g| g.playable()));
            }
            l.replace_main();
            // During setup the step itself says what was found.
            if l.pages.iter().any(|p| p.step >= 0) {
                while l.pages.len() > 1 && !l.pages.last().map_or(false, |p| p.main) { l.pages.pop(); }
                let step = crate::setup::setup_page(l.ctx(), 1, false);
                l.push(step);
                if !saves.is_empty() { l.import_save_files(saves.clone()); }
                l.ask_about_encrypted();
                return;
            }
            let mut p = crate::pages::show_text("Added", &format!("{}\n\nYou have {} of {} games ready.", lines.join("\n\n"), have, all));
            p.items.push(Item::new("Done", "Back to where you were.", true).icon('\u{E73E}').run(|l| { l.pages.pop(); }));
            if !saves.is_empty() {
                let n = saves.len();
                p.items.push(Item::new("Import the saves too", &format!("{} dropped with them.", if n == 1 { "1 save".to_string() } else { format!("{} saves", n) }), nm_core::config::mon_bridge().is_file())
                    .icon('\u{E896}').run(move |l| { l.pages.pop(); l.import_save_files(saves.clone()); }));
            }
            l.push(p);
            l.ask_about_encrypted();
        });
    }
}
