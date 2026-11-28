//! The save pages with no C# snapshot name: SavePage (and its delete question), BackupsPage, ExportGamesPage,
//! ExportPage, ConflictPage, DamagedPage. The save-changing work itself is a later step's (named effects).

use crate::launcher::Launcher;
use crate::model::{Act, Item, Page};
use crate::pages::{engine_name, theme_of, Ctx};
use nm_core::game_entry::GameEntry;
use nm_core::{save_guard, save_sets};

fn file_name(f: &str) -> String { std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() }
pub(crate) fn game_of(l: &Launcher, id: &str) -> Option<GameEntry> { l.ctx().lib.games.iter().find(|g| g.id == id).cloned() }
fn themed(title: &str, g: &GameEntry) -> Page { let mut p = Page::new(title); p.theme = Some(theme_of(g)); p }

pub use nm_core::save_info::backup_when;

/// SavePage(g, set).
pub fn save_page(g: &GameEntry, set: &str) -> Page {
    let in_use = set == save_sets::active(g);
    let mut p = themed(&save_sets::name(g, set), g);
    p.body = Some(if in_use { "This save is in use.".into() } else { "Switching keeps your current save exactly as it is, ready to come back to.".into() });
    if !in_use { p.items.push(Item::new("Use this save", "Play will use it from now on.", true).icon('\u{E73E}').run({ let (id, set) = (g.id.clone(), set.to_string()); move |l| { let set = set.clone(); l.save_action(&id, "Switching saves", move |gs, g| nm_core::saves::switch_save(gs, g, &set), 2) } })); }
    p.items.push(Item::new("Rename", "", true).icon('\u{E70F}').run({ let (id, set) = (g.id.clone(), set.to_string()); move |l| {
        let Some(g) = game_of(l, &id) else { return };
        let (id, set) = (id.clone(), set.clone());
        l.ask_text("Rename", "New name:", &save_sets::name(&g, &set), Box::new(move |l, name| {
            if let Some(name) = name { l.save_action(&id, "Renaming", move |_, g| save_sets::set_name(g, &set, &name).err().map(|e| e.to_string()), 2); }
        }));
    } }));
    p.items.push(Item::new("Copy", "A separate save that starts from this point.", true).icon('\u{E8C8}').run({ let (id, set) = (g.id.clone(), set.to_string()); move |l| { let set = set.clone(); l.save_action(&id, "Copying the save", move |_, g| nm_core::saves::duplicate_save(g, Some(&set)), 2) } }));
    if !in_use {
        let (id, set) = (g.id.clone(), set.to_string());
        p.items.push(Item::new("Delete", "Removes it from this list.", true).icon('\u{E74D}').run(move |l| {
            if let Some(g) = game_of(l, &id) { l.push(save_delete_page(&g, &set)); }
        }));
    }
    p
}
/// The question SavePage's Delete asks.
pub fn save_delete_page(g: &GameEntry, set: &str) -> Page {
    let mut sure = themed(&format!("Delete {}?", save_sets::name(g, set)), g);
    sure.body = Some("It leaves this list. A last copy stays in the game's backups folder.".into());
    sure.items.push(Item::new("Keep it", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    sure.items.push(Item::new("Delete it", "", true).icon('\u{E74D}').run({ let (id, set) = (g.id.clone(), set.to_string()); move |l| { let set = set.clone(); l.save_action(&id, "Deleting the save", move |_, g| nm_core::saves::delete_save(g, Some(&set)), 3) } }));
    sure
}

/// BackupsPage(g).
pub fn backups_page(g: &GameEntry) -> Page {
    let mut p = themed(&format!("{} backups", g.title), g);
    for f in save_guard::backups(g) {
        let bad = f.contains("-damaged");
        let (id, file) = (g.id.clone(), f.clone());
        let mut it = Item::new(&backup_when(&f), "", !bad).icon('\u{E81C}').run(move |l| l.restore_backup(&id, &file, None));
        it.info = if bad { "This one failed its check, and is kept in case it's ever needed.".into() } else { "Select to go back to this save. Your current save is backed up first.".into() };
        it.records = vec![format!("File: {}", file_name(&f))];
        p.items.push(it);
    }
    if p.items.is_empty() { p.items.push(Item::new("No backups yet", "", false).icon('\u{E81C}').info("A backup is made whenever a save is about to change.")); }
    p
}

/// ExportGamesPage.
pub fn export_games_page(c: &Ctx) -> Page {
    let mut p = Page::new("Export saves: which game?");
    for g in c.lib.games.iter().filter(|x| x.save_tools()) {
        let id = g.id.clone();
        let mut it = Item::new(&g.title, "", true).icon('\u{E898}').info(&format!("Uses this game's save from {}.", engine_name(&g.engine(), Some(g))))
            .run(move |l| { if let Some(g) = game_of(l, &id) { l.push(export_page(&g)); } });
        it.theme = Some(theme_of(g));
        p.items.push(it);
    }
    p
}

/// ExportPage(g).
pub fn export_page(g: &GameEntry) -> Page {
    let mut p = themed(&format!("Export {}", g.title), g);
    p.items.push(Item::new("Plain .sav file", "", true).icon('\u{E898}').info("For emulators, flash carts and PKHeX.").run({ let id = g.id.clone(); move |l| l.run_export(&id, false, None) }));
    let console = g.console.as_deref().unwrap_or("");
    let vc = console == "Game Boy" || console == "Game Boy Color";
    if vc { p.items.push(Item::new("3DS Virtual Console", "", true).icon('\u{E898}').info("Makes a Checkpoint folder holding sav.dat, for restoring to the 3DS over FTP.").run({ let id = g.id.clone(); move |l| l.run_export(&id, true, None) })); }
    if vc || g.is_ds() {
        let info = if g.is_ds() { "Uses a Checkpoint backup of this game for the exact file name and size. Make one on your 3DS first and copy it to this PC." } else { "The safest way: checks the size against a Checkpoint backup of this game and copies its SecureValue." };
        p.items.push(Item::new(if g.is_ds() { "3DS, from a backup" } else { "3DS, matched to a backup" }, "", true).icon('\u{E898}').info(info).run({ let id = g.id.clone(); move |l| l.run_export_from_backup(&id) }));
    }
    p
}

/// ConflictPage(g, then): both engines' saves changed; the player picks one.
pub fn conflict_page(g: &GameEntry, then: Option<Act>) -> Page {
    let mut p = themed(&format!("Which {} save?", g.title), g);
    p.body = Some("Both engines' saves changed since they last matched, so they can't simply be merged. Choose the one to keep playing. The other is kept in your backups.".into());
    // use(side): the chosen save shared on a Busy card, then on with the game.
    for (label, side) in [("Use the Fulldex save", "fulldex"), ("Use the Gen1Recomp save", "gen1")] {
        let (id, then) = (g.id.clone(), then.clone());
        p.items.push(Item::new(label, "", true).icon('\u{E7FC}').run(move |l| {
            l.pages.pop();
            let Some(g) = l.game(&id) else { return };
            let then = then.clone();
            l.busy(&format!("Sharing your {} save", g.title), move || Ok(nm_core::saves::sync_save(&g, Some(side))), move |l, r| {
                let note = match r { Err(e) => Some(e), Ok(res) => res.note };
                if let Some(n) = note { l.jobs.notice_after = Some(n); }
                if let Some(t) = &then { t.call(l); }
            });
        }));
    }
    p.items.push(Item::new("Not now", "Starts the game without sharing. You'll be asked again.", true).icon('\u{E711}').run(move |l| { l.pages.pop(); if let Some(t) = &then { t.call(l); } }));
    p
}

/// DamagedPage(g, why, done).
pub fn damaged_page(g: &GameEntry, why: &str, done: Option<Act>) -> Page {
    let good = save_guard::backups(g).into_iter().find(|x| !x.contains("-damaged"));
    let mut p = themed(&format!("Your {} save looks damaged", g.title), g);
    p.body = Some(format!("{}{}", why, match &good { Some(f) => format!("\n\nA healthy backup from {} is available.", backup_when(f)), None => "\n\nThere's no healthy backup to go back to yet.".into() }));
    if let Some(f) = &good {
        let (id, f, done) = (g.id.clone(), f.clone(), done.clone());
        p.items.push(Item::new("Restore that backup", "", true).icon('\u{E72C}').run(move |l| { l.pages.pop(); l.restore_backup(&id, &f, done.clone()); }));
    }
    p.items.push(Item::new("Leave it for now", "Nothing is changed. The damaged save is in your backups.", true).icon('\u{E711}').run(move |l| { l.pages.pop(); if let Some(d) = &done { d.call(l); } }));
    p
}

