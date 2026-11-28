//! Action pages with no C# snapshot name: ShareHintsPage, HeaderFixPage, PatchPage, ClashPairPage, CustomGamePage,
//! RemoveRomFilePage, RemoveUnzippedPage, RemoveFolderPage. Plus extra_screen: snapshot names for them and save_pages.

use crate::launcher::Launcher;
use crate::model::{Act, Item, Page};
use crate::pages::{theme_of, Ctx};
use crate::save_pages::{self, game_of};
use nm_core::game_entry::GameEntry;
use nm_core::{config, custom_games, mods::Clash, pc_games};

fn themed(title: &str, g: &GameEntry) -> Page { let mut p = Page::new(title); p.theme = Some(theme_of(g)); p }
fn file_name(f: &str) -> String { std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() }

/// Recomp.HintsFile / HintCount: Zakuro's findings beside the game's build.
fn hints_file(g: &GameEntry) -> std::path::PathBuf { nm_core::recomp::dir().join(format!("{}.hints", g.ctr.as_deref().unwrap_or("").to_uppercase())) }
pub fn hint_count(g: &GameEntry) -> usize {
    std::fs::read_to_string(hints_file(g)).map(|t| t.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).count()).unwrap_or(0)
}
/// Recomp.Answered.
fn answered(g: &GameEntry) { config::write(&format!("zakuro-share-answered-{}.txt", g.id), &hint_count(g).to_string()); }

/// Rescan: the library read again (the C# does it behind a Busy card), then `then`.
pub fn rescan(l: &mut Launcher, then: impl FnOnce(&mut Launcher)) {
    if let Ok(lib) = nm_core::library::Library::scan(&nm_core::engines::gen1_cli) {
        let cur = l.ctx().current.and_then(|i| l.ctx().lib.games.get(i)).map(|g| g.id.clone());
        let c = l.ctx.as_mut().unwrap();
        c.current = cur.and_then(|id| lib.games.iter().position(|g| g.id == id)).or_else(|| lib.games.iter().position(|g| g.playable()));
        c.lib = lib;
    }
    l.replace_main();
    then(l);
    l.ask_about_encrypted();
    l.ask_about_recomp();
}

/// ShareHintsPage(g, afterPlay).
pub fn share_hints_page(g: &GameEntry, after_play: bool) -> Page {
    let mut p = themed("Share Zakuro findings?", g);
    p.fit_body = true;
    p.body = Some(format!("While you played {t}, Zakuro found {n} places in its code that its build didn't cover, so it ran them the slow way.\n\n\
Sharing them improves the game for you and every other player: once checked, they go into the shared lists every launcher downloads, and every build of {t} from then on covers them, yours on any PC included. That means smoother play, with fewer slowdowns in the places you've been.\n\n\
What's shared: those addresses only (numbers like 0010022D), named by the game's code version. No game code, save or anything about you.\n\n\
Share sends them to nativeMons' own submission service (submit.hails.app), signed by this launcher; they're checked before anything uses them. Or share them through GitHub instead.", t = g.title, n = hint_count(g)));
    let id = g.id.clone();
    let ans = move |l: &mut Launcher| { l.pages.pop(); if let Some(g) = game_of(l, &id) { answered(&g); } };
    let a1 = ans.clone();
    p.items.push(Item::new(if after_play { "Yes, share" } else { "Share" }, "", true).icon('\u{E724}').info("Sends your list, signed by this launcher. Nothing about you goes with it.").run({ let id = g.id.clone(); move |l| { a1(l); l.share_send(&id); } }));
    let a2 = ans.clone();
    p.items.push(Item::new("Share through GitHub instead", "", true).icon('\u{E72D}').info("Opens a form on GitHub (you need an account) and copies your list to paste in; you submit it yourself.").run({ let id = g.id.clone(); move |l| { a2(l); l.share_github(&id); } }));
    let a3 = ans.clone();
    p.items.push(Item::new(if after_play { "No" } else { "Not now" }, "", true).icon('\u{E711}').info("Nothing is shared. You're asked again after more new findings.").run(move |l| a3(l)));
    if after_play {
        let a4 = ans;
        p.items.push(Item::new("Never ask again", "", true).icon('\u{E711}').info("Stops this question for every game. Turn it back on in Settings; a game's page can still share any time.")
            .run(move |l| { a4(l); config::write("zakuro-share-ask.txt", "never"); }));
    }
    p
}

/// Each dump's header fixed in turn; the first that can't be, and why.
pub fn fix_headers(roms: &[(String, String)]) -> Option<String> { roms.iter().find_map(|(t, f)| nm_core::ctr_header_fix::fix(f).map(|why| format!("{}: {}", t, why))) }
/// HeaderFixPage(games, then).
pub fn header_fix_page(games: &[&GameEntry], then: Option<Act>) -> Page {
    let mut p = themed(if games.len() == 1 { "Fix 3DS header?" } else { "Fix 3DS headers?" }, games[0]);
    p.fit_body = true;
    let names: Vec<String> = games.iter().map(|g| file_name(g.ctr_plain_rom.as_deref().unwrap_or(""))).collect();
    p.body = Some(format!("{}{} decrypted, but the header still says encrypted, so 3DSrecomp+Zakuro refuses it.\n\nThe fix changes 2 bytes in the dump's header, as GodMode9 does when it decrypts, and keeps the original bytes so it can be undone from the game's page. Emulators play it the same either way.",
        names.join(", "), if games.len() == 1 { " is" } else { " are" }));
    let roms: Vec<(String, String)> = games.iter().map(|g| (g.title.clone(), g.ctr_plain_rom.clone().unwrap_or_default())).collect();
    p.items.push(Item::new(if then.is_some() { "Fix, then build" } else { "Fix now" }, "", true).icon('\u{E90F}').info("Changes your ROM files: a few bytes each, with a backup.").run(move |l| {
        l.pages.pop();
        let (roms, then) = (roms.clone(), then.clone());
        l.busy("Fixing headers", move || Ok(fix_headers(&roms)), move |l, r| {
            match r { Err(x) | Ok(Some(x)) => { l.show_text("Not fixed", &x); return; } Ok(None) => {} }
            rescan(l, move |l| { if let Some(t) = &then { t.call(l); } });
        });
    }));
    p.items.push(Item::new("Not now", "", true).icon('\u{E711}').info("Nothing is changed. The emulators still play it.").run(|l| { l.pages.pop(); }));
    p
}

/// PatchPage(g, thenPlay).
pub fn patch_page(g: &GameEntry, then_play: bool) -> Page {
    let mut p = themed(&format!("Patch {}?", file_name(g.fulldex_rom.as_deref().unwrap_or(""))), g);
    p.body = Some("This ROM's secure area is still encrypted, so Fulldex can't load it without a DS BIOS.\n\nhails.nds-Patcher decrypts it. That changes 2 KB of your ROM file and keeps the original bytes in a backup, so it can be undone.\n\nIf no key table is available it downloads the DeSmuME emulator from GitHub to get one.".into());
    p.items.push(Item::new("Patch it", "Changes the ROM file, with a backup.", true).icon('\u{E90F}').run({ let id = g.id.clone(); move |l| l.patch_rom(&id, then_play) }));
    p.items.push(Item::new("Not now", "Nothing is changed.", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    p
}

/// ClashPairPage(g, c): which of two clashing mods to keep.
pub fn clash_pair_page(g: &GameEntry, c: &Clash) -> Page {
    let mut p = themed("Which to keep?", g);
    p.fit_body = true;
    p.body = Some(format!("{} says it doesn't work with {}. While both are on, {} doesn't load.", c.mod_name, c.other_name, c.mod_name));
    let keep = |off: String, gid: String| move |l: &mut Launcher| {
        let Some(g) = game_of(l, &gid) else { return };
        nm_core::engines::gen1_cli(&format!("set {} {} off", g.gen1.as_deref().unwrap_or(""), off));
        while l.pages.last().map_or(false, |p| p.title != "Mods that clash") { l.pages.pop(); }
        l.pages.pop();
        l.push(crate::mods_pages::clashes_page(&g, None));
    };
    let mut a = Item::new(&format!("Keep {}", c.mod_name), "", true).icon('\u{E73E}').run(keep(c.other_id.clone(), g.id.clone()));
    a.tile_hint = format!("{} is switched off", c.other_name);
    let mut b = Item::new(&format!("Keep {}", c.other_name), "", true).icon('\u{E73E}').run(keep(c.mod_id.clone(), g.id.clone()));
    b.tile_hint = format!("{} is switched off", c.mod_name);
    let mut n = Item::new("Leave both on", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); });
    n.tile_hint = format!("{} stays skipped", c.mod_name);
    p.items.extend([a, b, n]);
    p
}

/// CustomGamePage(g): a program the player added.
pub fn custom_game_page(g: &GameEntry) -> Page {
    let mut p = themed(&g.title, g);
    let Some(c) = custom_games::find(&g.id) else { p.body = Some("This game is no longer in the launcher.".into()); return p };
    let exe = custom_games::exe(&c);
    let on_pc = pc_games::installed(g);
    if !on_pc { p.body = Some(format!("Its program isn't at {} any more. Remove it here, then add it again from where it is now.", exe)); }
    let mut play = Item::new("Play", &format!("Start {}.", g.title), on_pc).icon('\u{E768}').run({ let id = g.id.clone(); move |l| l.play(&id) });
    play.records = vec![format!("{}{}", if c.copy { "The launcher's copy: " } else { "Runs from: " }, exe), "Saves: wherever the game keeps them".into()];
    p.items.push(play);
    let (id, title) = (g.id.clone(), g.title.clone());
    p.items.push(Item::new("Rename", &format!("Shown as {}.", g.title), true).icon('\u{E8AC}').run(move |l| {
        let id = id.clone();
        l.ask_text("Rename", "Name for this game:", &title, Box::new(move |l, name| {
            let Some(name) = name.filter(|n| !n.trim().is_empty()) else { return };
            custom_games::rename(&id, name.trim());
            rescan(l, |l| {
                l.pages.pop();
                l.pages.pop();
                l.push_with(|c| crate::pages::games_page(c, None));
                if let Some(g) = l.game(&id) { l.push(crate::pages::game_page(l.ctx(), &g)); }
                l.replace_main();
            });
        }));
    }));
    let dir = custom_games::dir(&c);
    let ds = dir.to_string_lossy().into_owned();
    p.items.push(Item::new("Open its folder", "", dir.is_dir()).icon('\u{E8B7}').info(&ds).run(move |l| l.effects.push(crate::input::Effect::OpenFolder(ds.clone()))));
    let (id, title) = (g.id.clone(), g.title.clone());
    p.items.push(Item::new("Remove from the launcher", if c.copy { "Deletes the launcher's copy. Saves stay." } else { "Forgets it. Nothing is deleted." }, true).icon('\u{E74D}').run(move |l| {
        let running = nm_core::saves::running_games(&l.ctx().lib.games);
        if !running.is_empty() { l.show_text("Close your game first", &format!("Already running: {}.", running)); return; }
        let id = id.clone();
        l.busy(&format!("Removing {}", title), { let id = id.clone(); move || Ok(custom_games::remove(&id)) }, move |l, r| {
            let problem = match r { Ok(p) => p, Err(e) => Some(e) };
            if let Some(p) = problem { l.show_text("Not removed", &p); return; }
            if config::read("last-game.txt", "") == id { config::write("last-game.txt", ""); }
            rescan(l, |l| { l.back_to_main(); l.push_with(|c| crate::pages::games_page(c, None)); l.replace_main(); });
        });
    }));
    p
}

/// PcGamePage: a PC game of its own (Infinite Fusion). Installing, updating and the folder pickers come with step 5.
pub fn pc_game_page(g: &GameEntry) -> Page {
    if pc_games::is_custom(g) { return custom_game_page(g); }
    let mut p = themed(&g.title, g);
    p.game_id = Some(g.id.clone());
    let Some(pf) = pc_games::of(g) else { return p };
    let git = pc_games::git().is_some();
    let import_items = |into: &mut Page| {
        let (a, b, c) = (g.id.clone(), g.id.clone(), g.id.clone());
        into.items.push(Item::new("Import from a folder", "Copies your installation into the launcher.", true).icon('\u{E8B7}').run(move |l| l.pc_import_folder(&a)));
        into.items.push(Item::new("Import from a zip", "Unpacks the game's download into the launcher.", true).icon('\u{E896}').run(move |l| l.pc_import_zip(&b)));
        into.items.push(Item::new("Download", "From the game's GitHub (a few GB). Needs Git for Windows.", git).icon('\u{E896}').run(move |l| l.install_pc(&c, None)));
    };
    if !g.on_pc() {
        p.body = Some(format!("{} is a PC game of its own, {}.\n\nImport it, and the launcher keeps the game and its saves in its own folders and updates it from the game's GitHub. Saves you already have are copied in.\n\nOr add your existing installation: it runs where it is, unchanged, with its saves where the game keeps them ({}).",
            g.title, pf.size, pc_games::game_save_dir(g).display()));
        import_items(&mut p);
        let id = g.id.clone();
        p.items.push(Item::new("Add an existing installation", "Runs your own copy where it is.", true).icon('\u{E71B}').run(move |l| l.pc_pick_existing(&id)));
        return p;
    }
    let existing = pc_games::is_existing(g);
    let version = pc_games::version(g);
    let id = g.id.clone();
    let mut play = Item::new("Play", &match &version { Some(v) => format!("Version {}.", v), None => format!("Start {}.", g.title) }, true).icon('\u{E768}')
        .run(move |l| l.play(&id));
    play.records = vec![format!("{}{}", if existing { "Your installation: " } else { "Imported into the launcher: " }, pc_games::dir(g).unwrap_or_default()),
        format!("Saves in {}", pc_games::save_dir(g).display())];
    p.items.push(play);
    if !existing {
        p.items.push(Item::new("Update", "Gets the newest release from the game's GitHub.", git).icon('\u{E895}').info("The same update the game's own updater does. Your saves aren't touched.").run({ let id = g.id.clone(); move |l| l.update_pc(&id) }));
    }
    let saves = pc_games::save_dir(g);
    let sd = saves.to_string_lossy().into_owned();
    p.items.push(Item::new("Saves folder", if existing { "Where the game keeps them." } else { "In the launcher's folder." }, true).icon('\u{E8B7}').info(&sd)
        .run(move |l| { let _ = std::fs::create_dir_all(&sd); l.effects.push(crate::input::Effect::OpenFolder(sd.clone())); }));
    let backups = config::state().join("save-backups").join(&g.id).to_string_lossy().into_owned();
    p.items.push(Item::new("Save backups", "Copies made before each session.", true).icon('\u{E81C}')
        .info("The launcher copies your save files here before each session, keeping the newest 30. The game also keeps backups of its own in its save folder.")
        .run(move |l| { let _ = std::fs::create_dir_all(&backups); l.effects.push(crate::input::Effect::OpenFolder(backups.clone())); }));
    let id = g.id.clone();
    if existing {
        p.items.push(Item::new("Import into the launcher", "A copy the launcher keeps and updates.", true).icon('\u{E896}').run(move |l| {
            let Some(g) = l.ctx().lib.games.iter().find(|x| x.id == id).cloned() else { return };
            let mut sub = themed(&format!("Import {}", g.title), &g);
            sub.body = Some("The launcher makes its own copy, with its own saves (yours are copied in), and updates it. Your installation stays as it is.".into());
            let (a, b, c, d, dir) = (g.id.clone(), g.id.clone(), g.id.clone(), g.id.clone(), pc_games::dir(&g).unwrap_or_default());
            sub.items.push(Item::new("Import this installation", &format!("Copies {}.", dir), true).icon('\u{E8B7}').run(move |l| l.install_pc(&a, Some(dir.clone()))));
            let git = pc_games::git().is_some();
            sub.items.push(Item::new("Import from a folder", "Copies your installation into the launcher.", true).icon('\u{E8B7}').run(move |l| l.pc_import_folder(&b)));
            sub.items.push(Item::new("Import from a zip", "Unpacks the game's download into the launcher.", true).icon('\u{E896}').run(move |l| l.pc_import_zip(&c)));
            sub.items.push(Item::new("Download", "From the game's GitHub (a few GB). Needs Git for Windows.", git).icon('\u{E896}').run(move |l| l.install_pc(&d, None)));
            l.push(sub);
        }));
        let id = g.id.clone();
        p.items.push(Item::new("Remove from the launcher", "Forgets it. Nothing is deleted.", true).icon('\u{E74D}').run(move |l| {
            // PcGames.Forget: the note naming the installation goes; the installation and its saves stay.
            let _ = std::fs::remove_file(config::state().join(format!("pc-{}.txt", id)));
            let title = l.ctx().lib.games.iter().find(|x| x.id == id).map(|g| g.title.clone()).unwrap_or_default();
            l.pages.pop();
            l.show_text("Removed", &format!("{} is no longer in the launcher. Your installation and saves are where they were.", title));
            l.replace_main();
        }));
    } else {
        p.items.push(Item::new("Use an existing installation", "Plays your own copy instead.", true).icon('\u{E71B}')
            .info("This copy and its saves in the launcher's folder stay; switching back is a matter of importing again.").run({ let id = g.id.clone(); move |l| l.pc_pick_existing(&id) }));
        p.items.push(Item::new("Uninstall", "Removes the launcher's copy. Saves stay.", true).icon('\u{E74D}').run(move |l| {
            let again = id.clone();
            if l.after_capture(move |l| { l.pages.pop(); if let Some(g) = l.ctx().lib.games.iter().find(|x| x.id == again) { let p = pc_remove_page(g); l.push(p); } }) { return; }
            if let Some(g) = l.ctx().lib.games.iter().find(|x| x.id == id) { let p = pc_remove_page(g); l.push(p); }
        }));
    }
    p
}

/// PcRemovePage: the uninstall question (the removal itself is a Busy job, step 5).
pub fn pc_remove_page(g: &GameEntry) -> Page {
    let mut p = themed(&format!("Uninstall {}?", g.title), g);
    p.body = Some(format!("This deletes the launcher's copy in {}.\n\nIts saves in {} stay, and so does any copy of the game you imported it from.",
        pc_games::own_dir(g).unwrap_or_default(), pc_games::save_dir(g).display()));
    p.items.push(Item::new("Uninstall", "", true).icon('\u{E74D}').run({ let id = g.id.clone(); move |l| l.uninstall_pc(&id) }));
    p.items.push(Item::new("Keep it", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    p
}

/// RemoveRomFilePage(file).
pub fn remove_rom_file_page(file: &str) -> Page {
    let mut p = Page::new("Stop using this ROM?");
    p.body = Some(file.into());
    p.items.push(Item::new("Keep it", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    let f = file.to_string();
    p.items.push(Item::new("Stop using it", "Nothing is deleted.", true).icon('\u{E74D}').run(move |l| {
        let mut list = config::rom_files();
        list.retain(|x| !x.eq_ignore_ascii_case(&f));
        config::save_rom_files(&list);
        l.pages.pop(); l.pages.pop();
        rescan(l, |l| l.push_with(crate::pages::folders_page));
    }));
    p
}
/// RemoveUnzippedPage(dir).
pub fn remove_unzipped_page(dir: &str) -> Page {
    let mut p = Page::new("Delete these copies?");
    p.body = Some(format!("The ROMs taken out of {}.zip. The zip itself stays as it is, and your saves are kept.", file_name(dir)));
    p.items.push(Item::new("Keep them", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    // RunningGames() == "" decides whether it's on; the process scan is the platform's.
    let d = dir.to_string();
    p.items.push(Item::new("Delete the copies", "", true).icon('\u{E74D}').run(move |l| {
        l.pages.pop();
        l.pages.pop();
        let d = d.clone();
        l.busy("Deleting", move || std::fs::remove_dir_all(&d).map_err(|e| e.to_string()), |l, r| {
            rescan(l, move |l| { l.push_with(crate::pages::folders_page); if let Err(e) = r { l.show_text("Not deleted", &e); } });
        });
    }));
    p
}
/// RemoveFolderPage(folder).
pub fn remove_folder_page(folder: &str) -> Page {
    let mut p = Page::new("Stop searching this folder?");
    p.body = Some(folder.into());
    p.items.push(Item::new("Keep it", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    let f = folder.to_string();
    p.items.push(Item::new("Stop searching it", "Nothing is deleted.", true).icon('\u{E74D}').run(move |l| {
        let mut list = config::rom_folders();
        if let Some(i) = list.iter().position(|x| x == &f) { list.remove(i); }
        config::save_rom_folders(&list);
        l.pages.pop(); l.pages.pop();
        rescan(l, |l| l.push_with(crate::pages::folders_page));
    }));
    p
}

/// Setup3dsPage(games): 3DS games found that 3DSrecomp+Zakuro can build.
pub fn setup_3ds_page(games: &[&GameEntry]) -> Page {
    let fix: Vec<&&GameEntry> = games.iter().filter(|g| nm_core::ctr_header_fix::needs(g.ctr_plain_rom.as_deref().unwrap_or(""))).collect();
    let cores = (std::thread::available_parallelism().map_or(1, |n| n.get()) as i32 / 2).max(1);
    let min = nm_core::recomp::minutes(cores);
    let one = games.len() == 1;
    let mut p = themed(&format!("Set up {}?", if one { games[0].title.clone() } else { format!("{} 3DS games", games.len()) }), games[0]);
    p.fit_body = true;
    let names: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
    let fixed: Vec<&str> = fix.iter().map(|g| g.title.as_str()).collect();
    p.body = Some(format!("Found: {}. {} natively with 3DSrecomp+Zakuro after a one-time build, made on this PC from your own dump; the build never leaves this PC.\n\nThey build one by one in the background, about {} minutes each at the lowest priority, with a banner showing how far along. You can keep using the launcher, and play any that's ready.{}",
        names.join(", "), if one { "It plays" } else { "They play" }, min,
        if fix.is_empty() { String::new() } else { format!("\n\n{}{} decrypted but still marked encrypted. Setting up fixes that first: 2 bytes in each file, as GodMode9 does, with a backup so it can be undone from the game's page.", fixed.join(", "), if fix.len() == 1 { "'s dump is" } else { "'s dumps are" }) }));
    let info = if fix.is_empty() { "Builds them in the background, one at a time." } else { "Fixes the headers (with a backup), then builds them in the background." };
    let roms: Vec<(String, String)> = fix.iter().map(|g| (g.title.clone(), g.ctr_plain_rom.clone().unwrap_or_default())).collect();
    let ids: Vec<String> = games.iter().map(|g| g.id.clone()).collect();
    p.items.push(Item::new("Set up in the background", "", true).icon('\u{E896}').info(info).run(move |l| {
        l.pages.pop();
        let (roms, ids) = (roms.clone(), ids.clone());
        l.busy(if roms.is_empty() { "Starting" } else { "Fixing headers" }, move || Ok(fix_headers(&roms)), move |l, r| {
            if let Err(x) | Ok(Some(x)) = r { l.show_text("Not set up", &format!("{}\n\nNothing was built. The emulators can still play them.", x)); return; }
            rescan(l, move |l| {
                fn games(l: &Launcher, ids: &[String]) -> Vec<GameEntry> { l.ctx().lib.games.iter().filter(|g| ids.contains(&g.id)).cloned().collect() }
                if nm_core::recomp::zig_exe().is_file() { let gs = games(l, &ids); l.build_quietly(&gs); return; }
                l.busy("Getting Zig", || Ok(nm_core::recomp::ensure_zig()), move |l, r| {
                    if let Err(y) | Ok(Some(y)) = r { l.show_text("Couldn't get Zig", &format!("{}\n\nNothing was built.", y)); return; }
                    let gs = games(l, &ids);
                    l.build_quietly(&gs);
                });
            });
        });
    }));
    p.items.push(Item::new("Not now", "", true).icon('\u{E711}').info("Build any of them later from its page, or by pressing Play.").run(|l| { l.pages.pop(); }));
    p
}

/// Snapshot names for these pages (Rust only: the C# has none). `s` is the name before the colon.
/// backups|savepage|savedelete|export|conflict|damaged|sharehints|sharehints-after|headerfix|patch|setup3ds|customgame:<game>,
/// exportgames, clashpair:<game>,<mod id>, removefolder, removerom:<game>.
pub fn extra_screen(c: &Ctx, s: &str, preview: Option<&str>) -> Option<Vec<Page>> {
    let ids: Vec<&str> = preview.unwrap_or("").split(',').collect();
    let game = c.lib.games.iter().find(|g| g.id == ids[0]).or_else(|| c.current.map(|i| &c.lib.games[i]));
    Some(match s {
        "exportgames" => vec![save_pages::export_games_page(c)],
        "removefolder" => vec![crate::pages::folders_page(c), remove_folder_page(&config::rom_folders().first().cloned().unwrap_or_default())],
        _ => {
            let g = game?;
            match s {
                "backups" => vec![save_pages::backups_page(g)],
                "savepage" | "savedelete" => {
                    // savepage: the save in use; savedelete: a parked one's delete question (the first other set).
                    let active = nm_core::save_sets::active(g);
                    let other = nm_core::save_sets::all(g).into_iter().find(|x| x != &active).unwrap_or_else(|| active.clone());
                    if s == "savepage" { vec![save_pages::save_page(g, &active)] } else { vec![save_pages::save_page(g, &other), save_pages::save_delete_page(g, &other)] }
                }
                "export" => vec![save_pages::export_games_page(c), save_pages::export_page(g)],
                "conflict" => vec![save_pages::conflict_page(g, None)],
                "damaged" => vec![save_pages::damaged_page(g, "Its checksum doesn't match: the game would call it corrupted.", None)],
                "sharehints" | "sharehints-after" => vec![share_hints_page(g, s == "sharehints-after")],
                "headerfix" => vec![header_fix_page(&[g], None)],
                "patch" => vec![patch_page(g, false)],
                "setup3ds" => vec![setup_3ds_page(&[g])],
                "customgame" => vec![custom_game_page(g)],
                "removerom" => vec![crate::pages::folders_page(c), remove_rom_file_page(g.roms.first().map(|r| r.file.as_str()).unwrap_or(""))],
                "clashpair" => {
                    let cl = nm_core::mods::read_clashes(g).into_iter().find(|x| ids.len() < 2 || x.mod_id == ids[1] || x.other_id == ids[1])?;
                    vec![crate::mods_pages::mods_page(g), crate::mods_pages::clashes_page(g, None), clash_pair_page(g, &cl)]
                }
                _ => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::{Host, Launcher};

    /// Draws each Rust-only page into NM_PAGES_OUT (a folder) to look at; needs NATIVEMONS_FONTS_DIR and an empty
    /// NATIVEMONS_STATE. Run with --ignored.
    #[test]
    #[ignore]
    fn render_rust_only_pages() {
        let out = std::path::PathBuf::from(std::env::var("NM_PAGES_OUT").expect("NM_PAGES_OUT"));
        std::fs::create_dir_all(&out).unwrap();
        nm_core::config::load();
        let lib = nm_core::library::Library::scan(&|_| None).unwrap();
        let mut g = GameEntry::default();
        g.id = "crystal".into();
        g.title = "Pokemon Crystal".into();
        g.console = Some("Game Boy Color".into());
        g.theme = nm_core::game_entry::Theme { main: "#4FA3C7".into(), deep: "#1E5F7A".into(), light: "#BFE6F5".into(), finish: "crystal".into(), game_id: Some("crystal".into()) };
        g.ctr_plain_rom = Some("C:\\Games\\roms\\Pokemon X (decrypted).3ds".into());
        g.fulldex_rom = Some("C:\\Games\\roms\\Pokemon Platinum.nds".into());
        let clash = Clash { mod_id: "kanto_colors".into(), mod_name: "Kanto Colors".into(), other_id: "mono_palette".into(), other_name: "Mono Palette".into(), range: String::new(), mod_on: true, other_on: true };
        let pages: Vec<(&str, Page)> = vec![
            ("savepage", save_pages::save_page(&g, "2")),
            ("savedelete", save_pages::save_delete_page(&g, "2")),
            ("backups", save_pages::backups_page(&g)),
            ("export", save_pages::export_page(&g)),
            ("conflict", save_pages::conflict_page(&g, None)),
            ("damaged", save_pages::damaged_page(&g, "Its checksum doesn't match: the game would call it corrupted.", None)),
            ("sharehints-after", share_hints_page(&g, true)),
            ("headerfix", header_fix_page(&[&g], None)),
            ("patch", patch_page(&g, false)),
            ("clashpair", clash_pair_page(&g, &clash)),
            ("setup3ds", setup_3ds_page(&[&g])),
            ("removefolder", remove_folder_page("C:\\Games\\roms")),
        ];
        for (name, page) in pages {
            let mut l = Launcher::new(600, 1000, Host::new((1920, 1040)));
            l.hooks.t = Some(7.25);
            l.ctx = Some(crate::pages::Ctx { lib: nm_core::library::Library::scan(&|_| None).unwrap(), current: None, first_setup: false, work_area: (1920, 1040), wide_window: false, light_auto: false, light_why: "a test run" });
            let m = crate::pages::main_page(l.ctx());
            l.pages.push(m);
            l.pages.push(page);
            let mut gr = nm_draw::Graphics::new(nm_draw::Bitmap::new(600, 1000));
            l.render(&mut gr, 600, 1000);
            l.screen_at = -10.0;
            l.sel_at = -10.0;
            l.render(&mut gr, 600, 1000);
            gr.bmp.save_png(&out.join(format!("{}.png", name))).unwrap();
        }
        drop(lib);
    }
}
