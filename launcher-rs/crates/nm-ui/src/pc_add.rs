//! AddPcGame, ChooseProgram, AddCustom and AddPcGameExe, line by line from the C#: a PC game from its folder (run
//! where it is), from a .zip (unpacked into games\) or from its program (run there, or its folder copied in).

use crate::launcher::Launcher;
use crate::model::{Item, Page};
use nm_core::custom_games as cg;

impl Launcher {
    fn taken_ids(&self) -> Vec<String> { self.ctx().lib.games.iter().map(|g| g.id.clone()).collect() }
    /// AddPcGame: how the game comes in.
    pub fn add_pc_game(&mut self) {
        if self.hooks.replay { self.todo("AddPcGame"); return; }
        let mut p = Page::new("Add a PC game");
        p.body = Some("Any game with its own program, such as a fan game. It runs as it is, with no extras, and keeps its saves wherever it always does.".into());
        p.items.push(Item::new("From its folder", "Runs where it is. Nothing is copied.", true).icon('\u{E8B7}')
            .info("Point to the folder the game lives in. The launcher finds its program and leaves every file where it is.")
            .run(|l| l.pick_folder("Choose the game's folder", Box::new(|l, dir| {
                let Some(dir) = dir else { return };
                let programs = cg::programs(&dir);
                if programs.is_empty() { l.show_text("No program there", &format!("There's no .exe in {} or its folders, so there's nothing to start.", dir)); return; }
                l.pages.pop();
                l.choose_program(programs, &dir, Box::new(|l, exe| { let taken = l.taken_ids(); l.add_custom("Adding the game", move || cg::add(&taken, &exe, false)); }), None);
            }))));
        p.items.push(Item::new("From a .zip", "Unpacked into the launcher's games folder.", true).icon('\u{E7B8}')
            .info("The launcher unpacks the zip into games\\, finds the game's program and keeps the copy, removed again with the game. The zip stays as it is.")
            .run(|l| l.pick_file("Choose the game's .zip", "Zip (*.zip)|*.zip", Box::new(|l, zip| {
                let Some(zip) = zip else { return };
                l.pages.pop();
                let z2 = zip.clone();
                l.busy("Unpacking the game", move || Ok(cg::unpack(&z2)), move |l, r| {
                    let (problem, work, programs) = match r { Ok(x) => x, Err(e) => (Some(e), None, Vec::new()) };
                    let work = match (problem, work) { (None, Some(w)) => w, (p, _) => { l.show_text("Not added", &p.unwrap_or_default()); return; } };
                    let (w1, w2, z) = (work.clone(), work.clone(), zip.clone());
                    l.choose_program(programs, &work, Box::new(move |l, exe| { let (taken, z, w1) = (l.taken_ids(), z.clone(), w1.clone()); l.add_custom("Adding the game", move || cg::add_unpacked(&taken, &z, &w1, &exe)); }),
                        Some(Box::new(move |_| { let _ = cg::remove_dir(std::path::Path::new(&w2)); })));
                });
            }))));
        p.items.push(Item::new("Choose its program (.exe)", "Then run it where it is, or copy its folder.", true).icon('\u{E7FC}').run(|l| { l.pages.pop(); l.add_pc_game_exe(); }));
        p.items.push(Item::new("Cancel", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
        self.push(p);
    }
    /// ChooseProgram: one program goes ahead; several are listed, the likeliest first.
    fn choose_program(&mut self, programs: Vec<String>, root: &str, chosen: Box<dyn Fn(&mut Launcher, String)>, cancelled: Option<Box<dyn Fn(&mut Launcher)>>) {
        if programs.len() == 1 { chosen(self, programs[0].clone()); return; }
        let mut p = Page::new("Which one starts the game?");
        p.body = Some("This folder has more than one program.".into());
        let chosen = std::rc::Rc::new(chosen);
        let root = root.trim_end_matches('\\').to_string();
        for exe in programs.iter().take(12) {
            let (e, c) = (exe.clone(), chosen.clone());
            let name = std::path::Path::new(exe).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            p.items.push(Item::new(&name, "", true).icon('\u{E7FC}').info(exe.get(root.len() + 1..).unwrap_or(exe)).run(move |l| { l.pages.pop(); c(l, e.clone()); }));
        }
        let has = cancelled.is_some();
        let cancelled = std::rc::Rc::new(cancelled);
        p.items.push(Item::new("Cancel", if has { "Nothing is added, and the unpacked files are deleted." } else { "" }, true).icon('\u{E711}')
            .run(move |l| { l.pages.pop(); if let Some(c) = cancelled.as_ref() { c(l); } }));
        self.push(p);
    }
    /// AddCustom: the work on a Busy card, then the library read again and the new game's page over Games.
    fn add_custom(&mut self, title: &str, work: impl FnOnce() -> Result<cg::Entry, String> + Send + 'static) {
        self.busy(title, work, |l, r| {
            let id = match r { Ok(e) => e.id.unwrap_or_default(), Err(e) => { l.show_text("Not added", &e); return; } };
            l.rescan();
            while l.pages.len() > 1 && !l.pages.last().map_or(false, |p| p.main) { l.pages.pop(); }
            let gp = crate::pages::games_page(l.ctx(), None);
            l.push(gp);
            if let Some(g) = l.game(&id) { let p = crate::pages::game_page(l.ctx(), &g); l.push(p); }
        });
    }
    /// AddPcGameExe: run the chosen program where it is, or copy its folder in.
    pub fn add_pc_game_exe(&mut self) {
        self.pick_file("Choose the game's program (.exe)", "Programs (*.exe)|*.exe", Box::new(|l, exe| {
            let Some(exe) = exe else { return };
            let stem = std::path::Path::new(&exe).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let mut p = Page::new(&format!("Add {}", stem));
            p.body = Some(format!("{}\n\nRun it where it is, or let the launcher keep its own copy of the game's folder (removed again with it). Either way the game keeps its saves wherever it always does.", exe));
            for (label, hint, icon, copy) in [("Run it where it is", "Nothing is copied.", '\u{E71B}', false), ("Copy it into the launcher", "Its whole folder, kept in games\\.", '\u{E8B7}', true)] {
                let exe = exe.clone();
                p.items.push(Item::new(label, hint, true).icon(icon).run(move |l| {
                    l.pages.pop();
                    let (taken, exe) = (l.taken_ids(), exe.clone());
                    l.add_custom(if copy { "Copying the game" } else { "Adding the game" }, move || cg::add(&taken, &exe, copy));
                }));
            }
            p.items.push(Item::new("Cancel", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
            l.push(p);
        }));
    }
}
