//! The Gen1Recomp mod pages: ModsPage, FindModsPage, ModEntryPage, DepsPage, DepPage, ClashesPage, ClashChoicePage.

use crate::model::{Item, Page};
use crate::pages::theme_of;
use nm_core::game_entry::GameEntry;
use nm_core::mod_index::{self, Entry};
use nm_core::mods::{self, Clash, Dep};
use nm_draw::Color;

pub const WARN: Color = Color::rgb(232, 128, 24);

fn gen1(g: &GameEntry) -> String { g.gen1.clone().unwrap_or_default() }
fn cli(cmd: &str) -> String { nm_core::engines::gen1_cli(cmd).unwrap_or_default() }

/// InstalledMods: id to version, from Gen1Recomp's list.
pub fn installed_mods(g: &GameEntry) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for row in cli(&format!("list {}", gen1(g))).split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 3 { continue; }
        match out.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(f[0])) { Some(e) => e.1 = f[2].to_string(), None => out.push((f[0].to_string(), f[2].to_string())) }
    }
    out
}
fn have<'a>(installed: &'a [(String, String)], id: &str) -> Option<&'a str> { installed.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)).map(|(_, v)| v.as_str()) }

/// ModClashes.Note.
fn clash_note(all: &[Clash], id: &str) -> Option<String> {
    if let Some(mine) = all.iter().find(|c| c.mod_id == id && c.active()).or_else(|| all.iter().find(|c| c.mod_id == id)) {
        return Some(format!("{}{}", if mine.active() { "Won't load: clashes with " } else { "Clashes with " }, mine.other_name));
    }
    let theirs = all.iter().find(|c| c.other_id == id && c.active()).or_else(|| all.iter().find(|c| c.other_id == id))?;
    Some(if theirs.active() { format!("{} won't load while this is on", theirs.mod_name) } else { format!("{} clashes with it", theirs.mod_name) })
}

fn titled(s: &str) -> String { s.split('-').map(|w| { let mut c = w.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }).collect::<Vec<_>>().join(" ") }
fn kind_name(c: &str) -> String { if c == "UI" { "UI".into() } else if c == "QOL" { "Quality of life".into() } else { titled(&c.to_lowercase().replace('_', "-")).replace('-', " ") } }
fn kind_icon(c: &str) -> char {
    match c {
        "GAMEPLAY" => '\u{E7FC}', "CONTENT" => '\u{E8A5}', "BALANCE" => '\u{E8CB}', "ART" => '\u{E790}', "AUDIO" => '\u{E8D6}', "UI" => '\u{E8A9}',
        "QOL" => '\u{E713}', "TRANSLATION" => '\u{E774}', "TOTAL_CONVERSION" => '\u{E7B8}', "LIBRARY" => '\u{E8F1}', "TOOL" => '\u{E90F}', _ => '\u{E74C}',
    }
}
/// "N0": thousands separated by commas.
fn n0(v: i64) -> String {
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); } out.push(ch); }
    if v < 0 { format!("-{}", out) } else { out }
}
fn warn(mut it: Item) -> Item { it.tint = WARN; it }
fn hint(mut it: Item, h: &str) -> Item { it.tile_hint = h.to_string(); it }

/// ModGamesPage: the Gen1Recomp games, each with its own mods.
pub fn mod_games_page(c: &crate::pages::Ctx) -> Page {
    let mut p = Page::new("Mods: choose a game");
    p.mods = true;
    for g in c.lib.games.iter().filter(|x| x.on_gen1()) {
        let id = g.id.clone();
        let mut it = Item::new(&g.title, "", true).icon('\u{E7FC}').info("Each game has its own mods, switched on and off for it alone.").run(move |l| l.push_game(&id, |_, g| mods_page(g)));
        it.theme = Some(theme_of(g));
        it.tile_hint = g.console.clone().unwrap_or_default();
        p.items.push(it);
    }
    p
}

/// InstalledMods from a "list" answer already read.
fn installed_from(list: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for row in list.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 3 { continue; }
        match out.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(f[0])) { Some(e) => e.1 = f[2].to_string(), None => out.push((f[0].to_string(), f[2].to_string())) }
    }
    out
}
type Shared<T> = std::rc::Rc<T>;
/// FindMods: the mod index on a Busy card, with the installed mods the Mods page read.
pub fn find_mods(l: &mut crate::launcher::Launcher, id: &str, installed: Shared<Vec<(String, String)>>) {
    let Some(g) = l.game(id) else { return };
    l.busy("Reading the mod index", mod_index::load, move |l, r| match r {
        Ok((all, note)) => { let p = find_mods_view(l, &g, installed.clone(), Shared::new(all), note); l.push(p); }
        Err(e) => l.show_text("Find mods", &e),
    });
}
/// FindModsPage with the launcher's view (a kind no longer listed goes back to all).
pub fn find_mods_view(l: &mut crate::launcher::Launcher, g: &GameEntry, installed: Shared<Vec<(String, String)>>, all: Shared<Vec<Entry>>, note: Option<String>) -> Page {
    let kinds = kinds_of(g, &all);
    if l.mod_kind.as_ref().map_or(false, |k| !kinds.contains(k)) { l.mod_kind = None; }
    let mut p = find_mods_page_with(g, &installed, &all, note.as_deref(), l.mod_kind.as_deref(), l.mod_alphabetical);
    let again = { let (gid, inst, all, note) = (g.id.clone(), installed.clone(), all.clone(), note.clone()); move |l: &mut crate::launcher::Launcher| {
        let Some(g) = l.game(&gid) else { return };
        let sel = l.pages.last().map_or(0, |p| p.sel);
        l.pages.pop();
        let mut fresh = find_mods_view(l, &g, inst.clone(), all.clone(), note.clone());
        fresh.sel = sel.min(fresh.items.len() as i32 - 1);
        l.push(fresh);
    } };
    let again = std::rc::Rc::new(again);
    let (a1, k1) = (again.clone(), kinds.clone());
    p.items[0].run = Some(crate::model::Act::new(move |l| {
        let at = l.mod_kind.as_ref().and_then(|k| k1.iter().position(|x| x == k)).map_or(-1, |i| i as i32);
        l.mod_kind = if ((at + 1) as usize) < k1.len() { Some(k1[(at + 1) as usize].clone()) } else { None };
        a1(l);
    }));
    let a2 = again.clone();
    p.items[1].run = Some(crate::model::Act::new(move |l| { l.mod_alphabetical = !l.mod_alphabetical; a2(l); }));
    // Each entry opens its page with the list this page was made from.
    let off = if note.is_some() { 3 } else { 2 };
    let shown = shown_mods(g, &all, l.mod_kind.as_deref(), l.mod_alphabetical);
    for (k, m) in shown.into_iter().enumerate() {
        let (gid, inst, m) = (g.id.clone(), installed.clone(), m.clone());
        p.items[off + k].run = Some(crate::model::Act::new(move |l| { let Some(g) = l.game(&gid) else { return }; l.push(mod_entry_page(&g, &m, &inst)); }));
    }
    p
}
fn kinds_of(g: &GameEntry, all: &[Entry]) -> Vec<String> {
    let fits: Vec<&Entry> = all.iter().filter(|x| mod_index::fits(x, g.gen1.as_deref())).collect();
    // Kinds by how many mods have them (a stable sort, as GroupBy then OrderByDescending).
    let mut kinds: Vec<(String, usize)> = Vec::new();
    for c in fits.iter().flat_map(|x| x.categories.iter()) {
        match kinds.iter_mut().find(|(k, _)| k == c) { Some(e) => e.1 += 1, None => kinds.push((c.clone(), 1)) }
    }
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    kinds.into_iter().map(|k| k.0).collect()
}
fn shown_mods<'a>(g: &GameEntry, all: &'a [Entry], kind: Option<&str>, alpha: bool) -> Vec<&'a Entry> {
    let mut shown: Vec<&Entry> = all.iter().filter(|x| mod_index::fits(x, g.gen1.as_deref())).filter(|x| kind.map_or(true, |k| x.categories.iter().any(|c| c == k))).collect();
    if alpha { shown.sort_by(|a, b| a.title.to_uppercase().cmp(&b.title.to_uppercase())); }
    else { shown.sort_by(|a, b| b.downloads.cmp(&a.downloads).then_with(|| a.title.to_uppercase().cmp(&b.title.to_uppercase()))); }
    shown
}
/// BackToMods: back to the game's Mods page, read again.
impl crate::launcher::Launcher {
    /// InstallMod: a mod's .zip chosen, unpacked into Gen1Recomp's mods folder, then AfterModInstall.
    pub fn install_mod(&mut self, id: &str) {
        if self.hooks.replay { self.todo("InstallMod"); return; }
        let id = id.to_string();
        self.pick_file("Choose a mod", "Mod archive (*.zip)|*.zip", Box::new(move |l, zip| {
            let Some(zip) = zip else { return };
            let message = mods::extract_mod(&zip, None).unwrap_or_else(|e| format!("Could not install it: {}", e));
            l.after_mod_install(&id, &message);
        }));
    }
        /// DepPage's install(what, work): on a Busy card, then back to the list, read again (this one done, and anything
    /// it asks for in turn).
    pub fn dep_install(&mut self, gid: &str, what: &str, work: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let gid = gid.to_string();
        self.busy_reporting(&format!("Installing {}", what), move |_| work(), move |l, r| {
            while !l.pages.is_empty() && l.pages.last().unwrap().title != "Needs other mods" { l.pages.pop(); }
            if !l.pages.is_empty() { l.pages.pop(); }
            let result = match r { Ok(x) => x, Err(e) => e };
            if let Some(g) = l.game(&gid) { l.push(deps_page(&g, Some(&result))); }
        });
    }
    /// AfterModInstall: a mod that went in switched off here (it clashes) asks which stays; open needs or clashes show
    /// their page with the result above; else back to the mods with the result.
    pub fn after_mod_install(&mut self, id: &str, result: &str) {
        let Some(g) = self.game(id) else { return };
        let to_mods = |l: &mut crate::launcher::Launcher| {
            while !l.pages.is_empty() && !l.pages.last().unwrap().title.starts_with("Mods: ") { l.pages.pop(); }
            if !l.pages.is_empty() { l.pages.pop(); }
        };
        // NewModClash.
        let last = mods::LAST.lock().unwrap().clone();
        let held = last.as_ref().and_then(|(inst, enable)| {
            let inst = inst.clone()?;
            let left = enable.split('\n').map(|l| l.trim_end_matches('\r').split('\t').collect::<Vec<_>>()).any(|f| f.len() >= 2 && Some(f[0]) == g.gen1.as_deref() && f[1] == "clash");
            if !left { return None; }
            mods::read_clashes(&g).into_iter().find(|c| c.mod_id == inst || c.other_id == inst).map(|c| (inst, c))
        });
        if let Some((inst, held)) = held {
            // It went in switched off here: choose, with the install's result above.
            let name = if held.mod_id == inst { held.mod_name.clone() } else { held.other_name.clone() };
            to_mods(self);
            self.push(mods_page(&g));
            let mut choice = clash_choice_page(&g, &inst, &name, &held);
            choice.body = Some(format!("{} It's off for now.\n\n{}", result, choice.body.clone().unwrap_or_default()));
            self.push(choice);
            *mods::LAST.lock().unwrap() = Some((None, last.map(|x| x.1).unwrap_or_default()));
            return;
        }
        let deps = !mods::open(&g).is_empty();
        let clash = !deps && mods::read_clashes(&g).iter().any(|c| c.active());
        if deps || clash {
            to_mods(self);
            self.push(mods_page(&g));
            self.push(if deps { deps_page(&g, Some(result)) } else { clashes_page(&g, Some(result)) });
        } else { back_to_mods(self, id, Some(result)); }
    }
}

pub fn back_to_mods(l: &mut crate::launcher::Launcher, id: &str, note: Option<&str>) {
    while !l.pages.is_empty() && !l.pages.last().unwrap().title.starts_with("Mods: ") { l.pages.pop(); }
    if !l.pages.is_empty() { l.pages.pop(); }
    l.push_game(id, |_, g| mods_page(g));
    if let Some(n) = note { l.show_text("Mods", n); }
}

pub fn mods_page(g: &GameEntry) -> Page {
    let mut p = Page::new(&format!("Mods: {}", g.title));
    p.theme = Some(theme_of(g));
    p.mods = true;
    let list = cli(&format!("list {}", gen1(g)));
    let listed = mod_index::cached_count();
    let gid = g.id.clone();
    let id = move || gid.clone();
    p.items.push(hint(Item::new("Find mods", "", true).icon('\u{E721}').run({ let id = id(); let inst = Shared::new(installed_from(&list)); move |l| find_mods(l, &id, inst.clone()) }).info("Browse the Gen1Recomp community mod index and install from it. The mods are their authors' own, and being listed isn't a check of them."),
        &if listed > 0 { format!("{} mods in the community index", listed) } else { "Browse the community index".into() }));
    let open = mods::open(g);
    if !open.is_empty() {
        let needed = open.iter().filter(|x| x.required).count();
        let ids = open.iter().map(|x| x.id.as_str()).collect::<Vec<_>>().join(", ");
        p.items.push(warn(hint(Item::new(&format!("Missing mods: {}", open.len()), "", true).icon('\u{E7BA}').run({ let id = id(); move |l| l.push_game(&id, |_, g| deps_page(g, None)) }).info(&format!("Mods you installed ask for others: {}. Select to choose where each comes from.", ids)),
            &if needed > 0 { format!("{} needed for a mod to load", needed) } else { "Optional extras your mods can use".into() })));
    }
    let clashes = mods::read_clashes(g);
    let active = clashes.iter().filter(|c| c.active()).count();
    if active > 0 {
        p.items.push(warn(hint(Item::new(&format!("Clashes: {}", active), "", true).icon('\u{E7BA}').run({ let id = id(); move |l| l.push_game(&id, |_, g| clashes_page(g, None)) }).info("Some mods say they don't work with others that are on. While both are on, the game skips the one that says so. Select to choose which to keep."),
            if active == 1 { "A mod won't load until you choose" } else { "Mods won't load until you choose" })));
    }
    let mut mine = 0;
    for row in list.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 6 || f[4] != "yes" { continue; }
        let (id, name, on) = (f[0], f[1], f[3] == "on");
        let clash = clash_note(&clashes, id);
        let (gid, mid, mname, known) = (g.id.clone(), id.to_string(), name.to_string(), clashes.clone());
        let mut it = Item::new(&format!("{}{}", name, if on { ": On" } else { ": Off" }), "", true).icon(if clash.is_some() { '\u{E7BA}' } else { '\u{E74C}' }).run(move |l| {
            let Some(g) = l.game(&gid) else { return };
            // Switching on beside a mod it clashes with: ask first (the clashes this page was made with).
            if !on {
                if let Some(against) = known.iter().cloned().find(|c| (c.mod_id == mid && c.other_on) || (c.other_id == mid && c.mod_on)) {
                    l.push(clash_choice_page(&g, &mid, &mname, &against));
                    return;
                }
            }
            set_mod(&g, &mid, !on);
            let sel = l.pages.last().map_or(0, |p| p.sel);
            l.pages.pop();
            let mut fresh = mods_page(&g);
            fresh.sel = sel.min(fresh.items.len() as i32 - 1);
            l.push(fresh);
        });
        if clash.is_some() { it.tint = WARN; }
        it.tile_hint = clash.clone().unwrap_or_else(|| format!("Version {}", f[2]));
        it.info = format!("{}{}{}Select to switch it on or off for {}.", f[5], if f[5].is_empty() { "" } else { "\n" }, clash.map(|c| format!("{}. ", c)).unwrap_or_default(), g.title.replace("Pokemon ", ""));
        p.items.push(it);
        mine += 1;
    }
    if mine == 0 { p.items.push(Item::new("No mods installed yet", "Find mods above, or install one from a file below.", false).icon('\u{E74C}')); }
    p.items.push(hint(Item::new("Install from a .zip file", "", true).icon('\u{E896}').run({ let id = g.id.clone(); move |l| l.install_mod(&id) }).info("Copies a mod into the game's mods folder. Only install mods you trust: they run code in the game."), "A mod you downloaded yourself"));
    p
}

/// FindModsPage, with the default view (all kinds, most downloaded).
pub fn find_mods_page(g: &GameEntry, installed: &[(String, String)], all: &[Entry], note: Option<&str>) -> Page { find_mods_page_with(g, installed, all, note, None, false) }
/// FindModsPage for one kind (or all), most downloaded or A to Z; find_mods_view sets what its rows do.
pub fn find_mods_page_with(g: &GameEntry, installed: &[(String, String)], all: &[Entry], note: Option<&str>, kind: Option<&str>, alpha: bool) -> Page {
    let fits = all.iter().filter(|x| mod_index::fits(x, g.gen1.as_deref())).count();
    let kinds = kinds_of(g, all);
    let shown = shown_mods(g, all, kind, alpha);
    let mut p = Page::new(&format!("Find mods: {}", g.title.replace("Pokemon ", "")));
    p.theme = Some(theme_of(g));
    p.jump = 10;
    p.items.push(hint(Item::new(&format!("Kind: {}", kind.map_or("All".to_string(), kind_name)), "", true).icon('\u{E71C}').info(&format!("Select to show one kind of mod at a time: {}, or all.", kinds.iter().map(|k| kind_name(k)).collect::<Vec<_>>().join(", "))),
        &format!("{} of {} mods for this game", shown.len(), fits)));
    p.items.push(Item::new(&format!("Sort: {}", if alpha { "A to Z" } else { "Most downloaded" }), "", true).icon('\u{E8CB}').info(if alpha { "Select to sort by downloads." } else { "Select to sort by name." }));
    if let Some(n) = note { p.items.push(Item::new("Showing the last list", "The index couldn't be reached", false).icon('\u{E946}').info(n)); }
    for m in &shown {
        let state = match have(installed, &m.id) { Some(v) => if Some(v) == m.version.as_deref() { "Installed" } else { "Update" }, None => if m.experimental { "Experimental" } else { "" } };
        let (gid, mid) = (g.id.clone(), m.id.clone());
        let mut it = Item::new(&format!("{}{}", m.title, if state.is_empty() { String::new() } else { format!(": {}", state) }), "", true).icon(kind_icon(m.categories.first().map(|s| s.as_str()).unwrap_or(""))).run(move |l| {
            let Ok((all, _)) = mod_index::load() else { return };
            let Some(m) = all.iter().find(|x| x.id == mid) else { return };
            let Some(i) = l.game_index(&gid) else { return };
            let g = l.ctx().lib.games[i].clone();
            let have = installed_mods(&g);
            l.push(mod_entry_page(&g, m, &have));
        });
        it.tile_hint = format!("by {}{}", m.author.clone().unwrap_or_default(), if m.downloads >= 0 { format!("  \u{b7}  {} downloads", n0(m.downloads)) } else { String::new() });
        it.info = m.summary.clone().unwrap_or_default();
        p.items.push(it);
    }
    if shown.is_empty() { p.items.push(Item::new("Nothing listed here yet", if kind.is_some() { "Try another kind" } else { "" }, false).icon('\u{E74C}')); }
    p
}

fn permission_words(p: &str) -> String {
    match p { "engine_internals" => "reaches into the game engine's own code".into(), "filesystem" => "reads and writes files".into(), "network" => "goes online".into(), _ => p.replace('_', " ") }
}

pub fn mod_entry_page(g: &GameEntry, m: &Entry, installed: &[(String, String)]) -> Page {
    let has = have(installed, &m.id);
    let about = m.summary.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "No description given.".into());
    let mut p = Page::new(&m.title);
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some(format!("{}\n\nIts author's own code, run in the game. Being listed isn't a check of it: install only what you trust.", about));
    let version = m.version.clone().unwrap_or_default();
    let label = match has { None => "Install".to_string(), Some(v) if v == version => "Install again".into(), Some(_) => format!("Update to {}", version) };
    let th = match has { Some(v) => format!("You have version {}", v), None => if m.experimental { "Experimental: it installs switched off".into() } else if m.zip_size > 0 { format!("{} KB download", n0((m.zip_size / 1024).max(1))) } else { String::new() } };
    let (gid, entry) = (g.id.clone(), m.clone());
    p.items.push(hint(Item::new(&label, "", true).icon('\u{E896}').run(move |l| {
        // ModIndex.Install notes a replay instead of downloading (unless NATIVEMONS_TEST_FLOWS runs it).
        let noted = l.note_flow("InstallFromIndex");
        let (e, gid) = (entry.clone(), gid.clone());
        l.busy_reporting(&format!("Installing {}", e.title), move |_| Ok(if noted { "a replay".to_string() } else { mod_index::install(&e) }), move |l, r| {
            let msg = match r { Ok(m) => m, Err(x) => x };
            l.after_mod_install(&gid, &msg);
        });
    }), &th));
    let fact = |p: &mut Page, name: &str, value: &str, icon: char| { if !value.is_empty() { p.items.push(Item::new(&format!("{}: {}", name, value), "", true).icon(icon).info(&format!("{}: {}", name, value))); } };
    if !m.needs.is_empty() { p.items.push(warn(Item::new(&format!("Needs: {}", m.needs.join(", ")), "", true).icon('\u{E7BA}').info("It needs these mods too. After it's installed, the launcher asks where to get each one."))); }
    if !m.clashes.is_empty() {
        let mine: Vec<&str> = m.clashes.iter().filter(|c| have(installed, c).is_some()).map(|s| s.as_str()).collect();
        let info = format!("Its author says it doesn't work with these. While one of them is on, it doesn't load{}", if mine.is_empty() { ".".to_string() } else { format!(". You have {}: after the install you choose which to keep on.", mine.join(", ")) });
        p.items.push(warn(hint(Item::new(&format!("Clashes with: {}", m.clashes.join(", ")), "", true).icon('\u{E7BA}').info(&info), &if mine.is_empty() { String::new() } else { format!("You have {}", mine.join(", ")) })));
    }
    fact(&mut p, "Author", &m.author.clone().unwrap_or_default(), '\u{E77B}');
    fact(&mut p, "Version", &version, '\u{E946}');
    if m.downloads >= 0 { fact(&mut p, "Downloads", &n0(m.downloads), '\u{E896}'); }
    fact(&mut p, "Games", &if m.games.is_empty() { "Any".into() } else { m.games.iter().map(|x| titled(x)).collect::<Vec<_>>().join(", ") }, '\u{E7FC}');
    if !m.categories.is_empty() { fact(&mut p, "Kind", &m.categories.iter().map(|c| kind_name(c)).collect::<Vec<_>>().join(", "), kind_icon(&m.categories[0])); }
    if !m.permissions.is_empty() {
        let names = m.permissions.iter().map(|x| match x.as_str() { "engine_internals" => "Engine".into(), "filesystem" => "Files".into(), "network" => "Internet".into(), _ => titled(&x.replace('_', "-")) }).collect::<Vec<String>>().join(", ");
        let words = m.permissions.iter().map(|x| permission_words(x)).collect::<Vec<_>>().join(", and ");
        p.items.push(Item::new(&format!("Asks for: {}", names), "", true).icon('\u{E72E}').info(&format!("Besides what a plain mod does, it {}.", words)));
    }
    fact(&mut p, "Licence", &m.license.clone().unwrap_or_default(), '\u{E8A5}');
    if let Some(repo) = m.repo.clone().filter(|r| !r.is_empty()) {
        let url = repo.clone();
        p.items.push(hint(Item::new("Open its page", "", repo.to_lowercase().starts_with("https://")).icon('\u{E774}').info(&repo).run(move |l| l.effects.push(crate::input::Effect::OpenUrl(url.clone()))), &repo.replace("https://", "")));
    }
    p
}

pub fn deps_page(g: &GameEntry, intro: Option<&str>) -> Page {
    let open = mods::open(g);
    let mut p = Page::new("Needs other mods");
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some(format!("{}{}", intro.map(|i| format!("{}\n\n", i)).unwrap_or_default(), if open.is_empty() { "Every mod they ask for is installed." } else { "Your mods ask for these. A required one has to be there for the mod to load; an optional one adds to it. Choose where each comes from." }));
    for d in &open {
        let state = if d.status == "version" { "Wrong version" } else if d.required { "Required" } else { "Optional" };
        let (gid, did) = (g.id.clone(), d.id.clone());
        let mut it = Item::new(&format!("{}: {}", d.id, state), "", true).icon(if d.required { '\u{E7BA}' } else { '\u{E74C}' }).run(move |l| {
            let Some(i) = l.game_index(&gid) else { return };
            let g = l.ctx().lib.games[i].clone();
            let Some(d) = mods::open(&g).into_iter().find(|x| x.id == did) else { return };
            let all = mod_index::load().map(|x| x.0).unwrap_or_default();
            l.push(dep_page(&g, &d, &all));
        });
        if d.required { it.tint = WARN; }
        it.tile_hint = format!("For {}{}{}", d.mod_name, if d.range.is_empty() { String::new() } else { format!(", version {}", d.range) }, if d.status == "version" { format!(" (you have {})", d.have) } else { String::new() });
        it.info = format!("{} Select to install it.", if d.required { format!("{} won't load without it.", d.mod_name) } else { format!("{} can use it, and works without it.", d.mod_name) });
        p.items.push(it);
    }
    let gid = g.id.clone();
    p.items.push(hint(Item::new(if open.is_empty() { "Done" } else { "Not now" }, "", true).icon('\u{E8FB}').run(move |l| back_to_mods(l, &gid, None)), if open.is_empty() { "" } else { "Back to the mods. The Mods page keeps a row for these." }));
    p
}

pub fn dep_page(g: &GameEntry, d: &Dep, index: &[Entry]) -> Page {
    let listed = index.iter().find(|x| x.id.eq_ignore_ascii_case(&d.id));
    let mut p = Page::new(&d.id);
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some(format!("{}{}{}{}\n\nChoose where to get it. Whatever you choose has to hold {}, or nothing is installed.",
        d.mod_name, if d.required { " needs this mod to load." } else { " can use this mod. It works without it." },
        if d.range.is_empty() { String::new() } else { format!(" It asks for version {}.", d.range) },
        if d.status == "version" { format!(" You have {}, which doesn't fit.", d.have) } else { String::new() }, d.id));
    if let Some(l) = listed {
        let (gid, e) = (g.id.clone(), l.clone());
        p.items.push(hint(Item::new("From the mod index", "", true).icon('\u{E721}').info("The release the Gen1Recomp community mod index lists for it.").run(move |l| {
            // ModIndex.Install notes a replay instead of downloading (unless NATIVEMONS_TEST_FLOWS runs it).
            if l.note_flow("InstallFromIndex") { let (gid, t) = (gid.clone(), e.title.clone()); l.dep_install(&gid, &t, move || Ok("a replay".to_string())); return; }
            let e = e.clone();
            l.dep_install(&gid, &e.title.clone(), move || Ok(mod_index::install(&e)));
        }),
            &format!("{}, version {} by {}", l.title, l.version.clone().unwrap_or_default(), l.author.clone().unwrap_or_default())));
    }
    if !d.github.is_empty() {
        let (gid, id, gh) = (g.id.clone(), d.id.clone(), d.github.clone());
        p.items.push(hint(Item::new("From its GitHub releases", "", true).icon('\u{E774}').info(&format!("The newest release there with a .zip, as {} names it.", d.mod_name)).run(move |l| {
            let (id2, gh) = (id.clone(), gh.clone());
            l.dep_install(&gid, &id, move || Ok(match mod_index::github_zip(&gh, &id2)? {
                None => format!("{} has no release with a .zip to install.", gh),
                Some((url, _, version)) => mod_index::install_from(Some(&url), &id2, &format!("{} {}", id2, version)),
            }));
        }), &format!("github.com/{}", d.github)));
    }
    let (gid, id) = (g.id.clone(), d.id.clone());
    p.items.push(hint(Item::new("Choose a downloaded .zip", "", true).icon('\u{E8B7}').info(&format!("A mod archive on your PC. It has to be {}.", d.id)).run(move |l| {
        let (gid, id) = (gid.clone(), id.clone());
        l.pick_file(&format!("Choose the .zip for {}", id), "Mod archive (*.zip)|*.zip", Box::new(move |l, zip| {
            let Some(zip) = zip else { return };
            let id2 = id.clone();
            l.dep_install(&gid, &id, move || Ok(mods::extract_mod(&zip, Some(&id2)).unwrap_or_else(|e| format!("Could not install it: {}", e))));
        }));
    }), "One you already downloaded"));
    let (gid, id) = (g.id.clone(), d.id.clone());
    p.items.push(hint(Item::new("Enter a download link", "", true).icon('\u{E71B}').info("Where its author publishes it: an https:// link straight to the .zip.").run(move |l| {
        let (gid, id) = (gid.clone(), id.clone());
        l.ask_text("Download link", &format!("Link to the .zip of {} (https://):", id), "", Box::new(move |l, url| {
            let Some(url) = url.filter(|u| !u.trim().is_empty()) else { return };
            let (url, id2) = (url.trim().to_string(), id.clone());
            l.dep_install(&gid, &id, move || Ok(mod_index::install_from(Some(&url), &id2, &id2)));
        }));
    }), "A link to its .zip"));
    if !d.required {
        let (gid, dep) = (g.id.clone(), d.clone());
        p.items.push(hint(Item::new("Don't install it", "", true).icon('\u{E711}').info(&format!("{} works without it. You can still install it later from a .zip.", d.mod_name)).run(move |l| {
            mods::decline(&dep);
            l.pages.pop();
            l.pages.pop();
            if let Some(g) = l.game(&gid) { l.push(deps_page(&g, None)); }
        }), &format!("Not asked again for {}", d.mod_name)));
    }
    p
}

pub fn clashes_page(g: &GameEntry, intro: Option<&str>) -> Page {
    let active: Vec<Clash> = mods::read_clashes(g).into_iter().filter(|c| c.active()).collect();
    let mut p = Page::new("Mods that clash");
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some(format!("{}{}", intro.map(|i| format!("{}\n\n", i)).unwrap_or_default(), if active.is_empty() { "No mods that are on clash with each other." } else { "These mods say they don't work together. While both are on, the game skips the one that says so. Choose which to keep on." }));
    for c in &active {
        p.items.push(warn(hint(Item::new(&format!("{} and {}", c.mod_name, c.other_name), "", true).icon('\u{E7BA}')
            .info(&format!("{} names {}{} as a mod it doesn't work with.", c.mod_name, c.other_name, if c.range.is_empty() { String::new() } else { format!(" (version {})", c.range) })),
            &format!("{} won't load while {} is on", c.mod_name, c.other_name)).run({ let (gid, a, b) = (g.id.clone(), c.mod_id.clone(), c.other_id.clone()); move |l| { let Some(i) = l.game_index(&gid) else { return }; let g = l.ctx().lib.games[i].clone(); if let Some(c) = mods::read_clashes(&g).into_iter().find(|x| x.mod_id == a && x.other_id == b) { l.push(crate::more_pages::clash_pair_page(&g, &c)); } } })));
    }
    let gid = g.id.clone();
    p.items.push(hint(Item::new(if active.is_empty() { "Done" } else { "Not now" }, "", true).icon('\u{E8FB}').run(move |l| back_to_mods(l, &gid, None)), if active.is_empty() { "" } else { "The Mods page keeps a row for these." }));

    p
}

/// SetMod: Gen1Recomp's mods switch for one game.
fn set_mod(g: &GameEntry, id: &str, on: bool) { nm_core::engines::gen1_cli(&format!("set {} {} {}", g.gen1.as_deref().unwrap_or(""), id, if on { "on" } else { "off" })); }

pub fn clash_choice_page(g: &GameEntry, id: &str, name: &str, c: &Clash) -> Page {
    let claims = c.mod_id == id;
    let (other, _other_id) = if claims { (&c.other_name, &c.other_id) } else { (&c.mod_name, &c.mod_id) };
    let mut p = Page::new(&format!("Switch {} on?", name));
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some(if claims { format!("{} says it doesn't work with {}, which is on. While both are on, {} doesn't load.", name, other, name) } else { format!("{} says it doesn't work with {}. While both are on, {} doesn't load.", other, name, other) });
    let back = |l: &mut crate::launcher::Launcher, g: &GameEntry| {
        while !l.pages.is_empty() && !l.pages.last().unwrap().title.starts_with("Mods: ") { l.pages.pop(); }
        if !l.pages.is_empty() { l.pages.pop(); }
        l.push(mods_page(g));
    };
    let (gid, id1, off) = (g.id.clone(), id.to_string(), _other_id.clone());
    p.items.push(hint(Item::new(&format!("Switch {} off", other), "", true).icon('\u{E73E}').run(move |l| {
        let Some(g) = l.game(&gid) else { return };
        set_mod(&g, &off, false);
        set_mod(&g, &id1, true);
        back(l, &g);
    }), &format!("{} on, {} off", name, other)));
    let (gid, id2) = (g.id.clone(), id.to_string());
    p.items.push(warn(hint(Item::new("Switch it on anyway", "", true).icon('\u{E7BA}').run(move |l| {
        let Some(g) = l.game(&gid) else { return };
        set_mod(&g, &id2, true);
        back(l, &g);
    }), &format!("{} won't load", if claims { name } else { other.as_str() }))));
    p.items.push(Item::new("Leave it off", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
    p
}
