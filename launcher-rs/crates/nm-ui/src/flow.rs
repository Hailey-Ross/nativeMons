//! Page flow helpers the items' actions use (pages.Push, ReplaceMain, ShowText, OpenCollection...).

use crate::input::Effect;
use crate::launcher::Launcher;
use crate::model::Page;
use crate::pages::{self, Ctx};

impl Launcher {
    pub fn ctx(&self) -> &Ctx { self.ctx.as_ref().expect("the library is loaded") }
    pub fn push(&mut self, p: Page) { self.pages.push(p); self.dirty = true; }
    /// Builds a page from the library and pushes it.
    pub fn push_with(&mut self, f: impl FnOnce(&Ctx) -> Page) { let p = f(self.ctx()); self.push(p); }
    pub fn current_game(&self) -> Option<&nm_core::game_entry::GameEntry> { let c = self.ctx(); c.current.map(|i| &c.lib.games[i]) }
    pub fn game_index(&self, id: &str) -> Option<usize> { self.ctx().lib.games.iter().position(|g| g.id == id) }
    pub fn main_page_now(&self) -> Page { pages::main_page(self.ctx()) }
    /// Builds a page for one game, found by its id at the time the action runs.
    pub fn push_game(&mut self, id: &str, f: impl FnOnce(&Ctx, &nm_core::game_entry::GameEntry) -> Page) {
        let Some(i) = self.game_index(id) else { return };
        let p = { let c = self.ctx(); f(c, &c.lib.games[i]) };
        self.push(p);
    }
    pub fn rebuild_game(&mut self, id: &str, keep_sel: bool, f: impl FnOnce(&Ctx, &nm_core::game_entry::GameEntry) -> Page) {
        let Some(i) = self.game_index(id) else { return };
        self.rebuild_top(keep_sel, |c| f(c, &c.lib.games[i]));
    }
    /// Pops back to the home screen (while pages.Count > 1 && !Peek().Main).
    pub fn back_to_main(&mut self) {
        while self.pages.len() > 1 && !self.pages.last().unwrap().main { self.pages.pop(); }
        self.dirty = true;
    }
    /// BackAndRebuild: pops this page and rebuilds the one under it, keeping its selection.
    pub fn back_and_rebuild(&mut self, f: impl FnOnce(&Ctx) -> Page) {
        self.pages.pop();
        if self.pages.is_empty() { return; }
        self.rebuild_top(true, f);
    }

    /// ReplaceMain: the home screen rebuilt in place, keeping its selection.
    pub fn replace_main(&mut self) {
        for i in 0..self.pages.len() {
            if self.pages[i].main {
                let mut m = self.main_page_now();
                m.sel = self.pages[i].sel.min(m.items.len() as i32 - 1);
                self.pages[i] = m;
            }
        }
        self.dirty = true;
    }
    /// RebuildTop / pages.Pop(); pages.Push(make()), keeping the selection when `keep_sel`.
    pub fn rebuild_top(&mut self, keep_sel: bool, f: impl FnOnce(&Ctx) -> Page) {
        let sel = self.pages.last().map_or(0, |p| p.sel);
        self.pages.pop();
        let mut p = f(self.ctx());
        if keep_sel { p.sel = sel.min(p.items.len() as i32 - 1).max(0); }
        self.push(p);
    }
    /// ShowText(title, body, back).
    pub fn show_text(&mut self, title: &str, body: &str) { self.push(pages::show_text(title, body)); }
    /// An action that belongs to a later step (play flows, downloads, dialogs): named, so tests can see it ran.
    pub fn todo(&mut self, what: &str) { self.core_notes(); self.effects.push(Effect::Todo(what.to_string())); }
    /// What nm-core's test stubs noted (downloads, Gen1Recomp's answers), into the effects in order.
    pub fn core_notes(&mut self) {
        for (k, v) in nm_core::test_hooks::take_notes() { self.effects.push(if k == "Fetch" { Effect::Fetch(v) } else { Effect::Gen1Cli(v) }); }
    }
    /// TestHooks.Note: a flow a replay only notes, unless NATIVEMONS_TEST_FLOWS runs it (true: noted, stop).
    pub fn note_flow(&mut self, what: &str) -> bool { if !self.hooks.replay || nm_core::test_hooks::flows() { return false; } self.todo(what); true }

    /// The Pokedex, loaded once (LoadDex), for My Collection's pages.
    pub fn ensure_dex(&mut self) -> Result<(), String> {
        // A background read (after the start or a game) changed what's kept: read it again from there.
        if crate::dex::take_stale() { self.dex = None; }
        if self.dex.is_none() {
            // The sample collection is the snapshots' only (as the C#): a replay that opens My Collection reads the real one.
            let d = crate::dex::load_dex(self.ctx())?;
            self.dex = Some(d);
        }
        Ok(())
    }
    pub fn push_dex(&mut self, f: impl FnOnce(&Ctx, &crate::dex::Dex) -> Page) {
        if let Err(e) = self.ensure_dex() { self.show_text("Pokedex", &e); return; }
        let p = f(self.ctx(), self.dex.as_ref().unwrap());
        self.push(p);
    }
    /// OpenCollection (the Pokedex is read at once here; the C# waits on a Busy card when it isn't ready).
    pub fn open_collection(&mut self) {
        if self.after_capture(|l| l.open_collection()) { return; }
        self.push_dex(crate::dex::dex_home_page);
    }
    pub fn dex_shiny_of(&self, id: i32, only: Option<&str>) -> bool { self.dex.as_ref().map_or(false, |d| d.shiny(self.ctx(), id, only)) }
    pub fn dex_entry_page_now(&mut self, id: i32, order: Vec<i32>, only: Option<String>) -> Page {
        let _ = self.ensure_dex();
        let mut p = crate::dex::dex_entry_page(self.ctx(), self.dex.as_ref().unwrap(), id, only.as_deref());
        p.dex_order = order;
        p.dex_only = only;
        p
    }

    pub fn clash_showing(&self) -> bool { self.clash_game.is_some() && !self.starting && !self.job && !self.pages.iter().any(|x| x.mods) }
    /// OpenClashes (no Busy card: the mods are read at once).
    pub fn open_clashes(&mut self) {
        let Some(i) = self.clash_game else { return };
        let g = self.ctx().lib.games[i].clone();
        self.push(crate::mods_pages::mods_page(&g));
        self.push(crate::mods_pages::clashes_page(&g, None));
    }
    /// BannerAge without its side effects, for Poll's redraw check.
    pub fn banner_age_peek(&self, t: f32) -> f32 {
        if !nm_core::updater::ready() || self.job || self.starting { return -1.0; }
        let build = nm_core::updater::latest().map(|r| r.build).unwrap_or(0);
        if self.banner_closed.as_deref() == Some(build.to_string().as_str()) { return -1.0; }
        if self.banner_build != build { return 0.0; }
        let age = t - self.banner_at;
        if age <= 8.0 { age } else { -1.0 }
    }

    /// OpenSaves: the saves read behind "Reading your saves" (Gen1Recomp is asked for its slots), then the list.
    pub fn open_saves(&mut self, id: &str) {
        let Some(i) = self.game_index(id) else { return };
        let g = self.ctx().lib.games[i].clone();
        self.busy("Reading your saves", { let g = g.clone(); move || Ok(pages::saves_rows(&g)) }, move |l, r| match r {
            Ok(rows) => { let p = pages::saves_page_of(&g, rows); l.push(p); }
            Err(e) => l.show_text("Saves", &e),
        });
    }
    /// Browse: opens at the last folder browsed, with its parents underneath, so Back walks up.
    pub fn browse(&mut self, mut req: pages::BrowseRequest) {
        self.input.browse_ids += 1;
        req.id = self.input.browse_ids;
        self.push(pages::browse_page(&req, None));
        let last = nm_core::config::read("browse-last.txt", "").trim().to_string();
        if last.is_empty() || !std::path::Path::new(&last).is_dir() { return; }
        let mut trail = Vec::new();
        let mut d = Some(std::path::PathBuf::from(&last));
        while let Some(x) = d { trail.insert(0, x.to_string_lossy().into_owned()); d = x.parent().map(|p| p.to_path_buf()); }
        for t in trail { self.push(pages::browse_page(&req, Some(&t))); }
    }
    pub fn browse_finish(&mut self, req: &pages::BrowseRequest, paths: Vec<String>, at: &str) {
        if !at.is_empty() { nm_core::config::write("browse-last.txt", at); }
        while self.pages.len() > 1 && self.pages.last().unwrap().browse == req.id { self.pages.pop(); }
        match req.done.as_str() {
            "addfolder" => {
                let chosen = paths[0].clone();
                let mut l = nm_core::config::rom_folders();
                if !l.contains(&chosen) { l.push(chosen.clone()); nm_core::config::save_rom_folders(&l); }
                self.back_to_main();
                self.rescan();
                let p1 = pages::folders_page(self.ctx());
                let p2 = pages::folder_added_page(self.ctx(), &chosen);
                self.push(p1);
                self.push(p2);
                // Rescan's questions, after its pages.
                self.ask_about_encrypted();
                self.ask_about_recomp();
            }
            "addrom" => self.take_drop(paths),
            other => match other.strip_prefix("picture:") { Some(id) => self.wallpaper_picked(id, &paths[0]), None => self.todo(&format!("BrowseFinish {}", other)) },
        }
        self.dirty = true;
    }
    /// AddFolder.
    pub fn add_folder(&mut self) {
        self.browse(pages::BrowseRequest { title: "Choose your ROM folder".into(), folders: true, kinds: pages::ROM_KINDS.to_vec(), hint: "Open the folder that holds your ROMs, then choose Use this folder. Every folder inside it is searched too.".into(), done: "addfolder".into(), ..Default::default() });
    }
    /// FinishFolders.
    pub fn finish_folders(&mut self) {
        let setup = self.ctx().first_setup;
        self.ctx.as_mut().unwrap().first_setup = false;
        self.back_to_main();
        if setup && !pages::wallpaper::chosen() { let p = pages::wallpaper_page(self.ctx(), true); self.push(p); }
    }
    /// Rescan (at once; the C# scans on a Busy card): the library again, the same game kept current.
    pub fn rescan(&mut self) {
        let cur = self.current_game().map(|g| g.id.clone());
        if let Ok(lib) = nm_core::library::Library::scan(&nm_core::engines::gen1_cli) {
            let c = self.ctx.as_mut().unwrap();
            c.lib = lib;
            c.current = cur.and_then(|id| c.lib.games.iter().position(|g| g.id == id && g.playable())).or_else(|| c.lib.games.iter().position(|g| g.playable()));
        }
        self.replace_main();
    }
}
