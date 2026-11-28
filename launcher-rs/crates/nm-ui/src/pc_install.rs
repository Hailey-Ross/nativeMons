//! PcGamePage's flows, line by line from the C#: pickExisting (AddExisting), importItems (a folder, a zip, the
//! download), InstallPc, UpdatePc and the uninstall (PcRemovePage's Busy). The work is nm-core's pc_games.

use crate::launcher::Launcher;
use nm_core::pc_games;

impl Launcher {
    fn close_game_first(&mut self) -> bool {
        let running = nm_core::saves::running_games(&self.ctx().lib.games);
        if running.is_empty() { return false; }
        self.show_text("Close your game first", &format!("Already running: {}.", running));
        true
    }
    /// "Making its pictures": its title screen and town from the game's own pictures, then the menu shows them.
    fn make_pc_pictures(&mut self, g: &nm_core::game_entry::GameEntry) {
        let g = g.clone();
        self.busy("Making its pictures", move || { nm_core::art_jobs::background(|| { crate::capture::capture(&g); crate::capture::capture_town(&g); }); Ok(()) }, |l, _| l.replace_main());
    }
    /// pickExisting: the player's own installation, run where it is.
    pub fn pc_pick_existing(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let exe = pc_games::of(&g).map(|p| p.exe).unwrap_or("");
        let id = id.to_string();
        self.pick_folder(&format!("Choose your installation's folder, the one with {}", exe), Box::new(move |l, f| {
            let (Some(f), Some(g)) = (f, l.game(&id)) else { return };
            if let Some(problem) = pc_games::add_existing(&g, &f) { l.show_text("Not added", &problem); return; }
            l.pages.pop();
            l.push(crate::more_pages::pc_game_page(&g));
            l.make_pc_pictures(&g);
        }));
    }
    /// importItems' "Import from a folder".
    pub fn pc_import_folder(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let exe = pc_games::of(&g).map(|p| p.exe).unwrap_or("");
        let id = id.to_string();
        self.pick_folder(&format!("Choose the folder with {}", exe), Box::new(move |l, f| { if let Some(f) = f { l.install_pc(&id, Some(f)); } }));
    }
    /// importItems' "Import from a zip".
    pub fn pc_import_zip(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        let id = id.to_string();
        self.pick_file(&format!("Choose {}'s zip", g.title), "Zip (*.zip)|*.zip", Box::new(move |l, f| { if let Some(f) = f { l.install_pc(&id, Some(f)); } }));
    }
    /// InstallPc: the install on a Busy card with its steps, then Games and the game's page, what happened to the
    /// saves, and its pictures.
    pub fn install_pc(&mut self, id: &str, source: Option<String>) {
        if self.close_game_first() { return; }
        let Some(g) = self.game(id) else { return };
        let (gw, id) = (g.clone(), id.to_string());
        self.busy_reporting(&format!("Importing {}", g.title), move |report| Ok(pc_games::install(&gw, source.as_deref(), report)), move |l, r| {
            let saved = match r {
                Ok((None, saved)) => saved,
                Ok((Some(p), _)) | Err(p) => { l.show_text("Not imported", &p); return; }
            };
            let Some(g) = l.game(&id) else { return };
            l.back_to_main();
            let gp = crate::pages::games_page(l.ctx(), None);
            l.push(gp);
            l.push(crate::more_pages::pc_game_page(&g));
            if let Some(s) = saved { l.show_text("Imported", &format!("{} is in the launcher. {}", g.title, s)); }
            l.make_pc_pictures(&g);
        });
    }
    /// UpdatePc: the game page rebuilt, then the result on top (deliberately unlike the C#, which hides it).
    pub fn update_pc(&mut self, id: &str) {
        if self.close_game_first() { return; }
        let Some(g) = self.game(id) else { return };
        let before = pc_games::version(&g);
        let (gw, id) = (g.clone(), id.to_string());
        self.busy_reporting(&format!("Updating {}", g.title), move |report| Ok(pc_games::update(&gw, &mut |line| report(line, -1.0))), move |l, r| {
            let Some(g) = l.game(&id) else { return };
            let after = pc_games::version(&g);
            let problem = match r { Ok(p) => p, Err(e) => Some(e) };
            let a = after.clone().unwrap_or_default();
            let body = problem.clone().unwrap_or_else(|| if after == before { format!("You already have the newest release: {}.", a) } else { format!("Now on {}.", a) });
            // The user's call (2026-11-25): the result shows over the rebuilt page (the C# pops it at once).
            l.pages.pop();
            l.push(crate::more_pages::pc_game_page(&g));
            l.show_text(if problem.is_some() { "Not updated" } else { "Updated" }, &body);
        });
    }
    /// PcRemovePage's Uninstall: the launcher's copy deleted on a Busy card; saves stay.
    pub fn uninstall_pc(&mut self, id: &str) {
        if self.close_game_first() { return; }
        let Some(g) = self.game(id) else { return };
        let dir = pc_games::own_dir(&g).unwrap_or_default();
        let title = g.title.clone();
        self.busy(&format!("Removing {}", title), move || pc_games::remove(std::path::Path::new(&dir)).map_err(|e| e.to_string()), move |l, r| {
            l.pages.pop();
            l.pages.pop();
            match r { Ok(()) => l.show_text("Uninstalled", &format!("{} was removed. Its saves are where they were.", title)), Err(e) => l.show_text("Not fully removed", &e) }
            l.replace_main();
        });
    }
}
