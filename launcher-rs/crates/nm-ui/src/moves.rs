//! Move Pokemon's moves, ported from the C#: OpenBoxView, LoadBoxes with its Busy card, BoxDrop's relocate inside one
//! save, MoveNow (plan, ResolvePage, ChangeMovePage, RunMove). Saves change only through mon-bridge, backed up first.

use crate::boxes::*;
use crate::launcher::Launcher;
use crate::model::{Item, Page};
use crate::pages::theme_of;
use nm_core::game_entry::GameEntry;
use nm_core::{config, saves};
use std::cell::RefCell;
use std::rc::Rc;

/// The launcher's Bridge: RunCapture on mon-bridge. The C# leaves out the originals, Node and OpenHome folders
/// here (a bug: returning Pokemon and OpenHome's converter never work from the launcher); Rust passes them.
pub fn ui_bridge(args: &str) -> (String, i32) {
    let mut psi = nm_core::proc::StartInfo::new(config::mon_bridge(), args);
    psi.set_env("NATIVEMONS_ORIGINALS", &config::state().join("pokemon-originals").to_string_lossy());
    psi.set_env("NATIVEMONS_NODE", &config::node().to_string_lossy());
    psi.set_env("NATIVEMONS_OPENHOME", &config::open_home_tool().to_string_lossy());
    nm_core::engines::run_capture(&psi, 60)
}

fn backups() -> String { config::state().join("save-backups").to_string_lossy().into_owned() }
fn save_flows() -> bool { std::env::var("NATIVEMONS_TEST_SAVE_FLOWS").as_deref() == Ok("1") }
pub fn move_tools(g: &GameEntry) -> bool { g.save_tools() || (g.ctr.is_some() && nm_core::ctr3ds_saves::main(g).is_file()) }

/// ResolvePage's choices: move number -> new id ("0" deletes), in the order they were made, and the held item.
#[derive(Default)]
struct Resolve { answers: Vec<(String, String)>, drop_item: bool }

impl Launcher {
    fn game_by_id(&self, id: &str) -> Option<GameEntry> { self.ctx().lib.games.iter().find(|g| g.id == id).cloned() }

    /// OpenBoxView: the box screen on the current game (when it has a save) and another one.
    pub fn open_box_view(&mut self) {
        if self.hooks.replay && !save_flows() { self.todo("OpenBoxView"); return; }
        let c = self.ctx();
        let games: Vec<GameEntry> = c.lib.games.iter().filter(|g| move_tools(g)).cloned().collect();
        if games.is_empty() { return; }
        let saved: Vec<GameEntry> = games.into_iter().filter(has_box_save).collect();
        if saved.is_empty() { self.show_text("Move Pokemon", "None of your games has a save yet. Play one and save once in the game, then its boxes show here."); return; }
        let cur = c.current.map(|i| c.lib.games[i].id.clone());
        let top = saved.iter().find(|g| Some(&g.id) == cur.as_ref()).unwrap_or(&saved[0]).clone();
        let bottom = saved.iter().find(|g| g.id != top.id).unwrap_or(&top).clone();
        self.load_boxes_job(Some(BoxView::default()), top, bottom);
    }

    /// LoadBoxes: both boxes read again behind "Opening your boxes"; the same game in both is one save, read once.
    /// A new view gets its page pushed; None updates the box screen already open.
    pub fn load_boxes_job(&mut self, fresh: Option<BoxView>, top: GameEntry, bottom: GameEntry) {
        self.busy("Opening your boxes", move || {
            let a = load_side(&top);
            let b = if bottom.id == top.id { a.clone() } else { load_side(&bottom) };
            Ok((a, b))
        }, move |l, r| {
            let (a, b) = match r { Ok(x) => x, Err(e) => { l.show_text("Can't open the boxes", &e); return; } };
            let set = |v: &mut BoxView| { v.side = [Some(a), Some(b)]; for i in 0..2 { v.boxn[i] = v.boxn[i].min(v.side[i].as_ref().unwrap().boxes - 1); } };
            match fresh {
                Some(mut v) => { set(&mut v); let mut p = Page::new("Move Pokemon"); p.boxes = Some(v); l.push(p); }
                None => if let Some(v) = l.pages.iter_mut().rev().find_map(|p| p.boxes.as_mut()) { set(v); },
            }
            l.dirty = true;
        });
    }
    fn box_view_mut(&mut self) -> Option<&mut BoxView> { self.pages.iter_mut().rev().find_map(|p| p.boxes.as_mut()) }
    fn reload_boxes(&mut self) {
        let Some(v) = self.pages.iter().rev().find_map(|p| p.boxes.as_ref()) else { return };
        let ids: Vec<String> = v.side.iter().map(|s| s.as_ref().map(|s| s.game_id.clone()).unwrap_or_default()).collect();
        let (Some(a), Some(b)) = (self.game_by_id(&ids[0]), self.game_by_id(&ids[1])) else { return };
        self.load_boxes_job(None, a, b);
    }

    /// BoxDrop inside one save: mon-bridge relocate, then Gen1Recomp takes its exported copy back.
    pub fn box_relocate(&mut self, r: BoxOut) {
        let BoxOut::Relocate { game, file, from_box, from_slot, to_box, to_slot, species } = r.clone() else { return };
        // The C# re-runs BoxDrop after "Just a moment" with its held Pokemon already cleared (Side[-1]): the move runs again here.
        if self.after_capture(move |l| l.box_relocate(r)) { return; }
        let running = saves::running_games(self.games());
        if !running.is_empty() { if let Some(v) = self.box_view_mut() { v.note = Some(format!("Close {} first: saves can't change while it plays.", running)); } self.dirty = true; return; }
        let Some(g) = self.game_by_id(&game) else { return };
        self.busy(&format!("Moving {}", species), move || {
            let (o, code) = ui_bridge(&format!("relocate \"{}\" {} {} {} {} \"{}\"", file, from_box, from_slot, to_box, to_slot, backups()));
            if code != 0 { let t = o.trim(); let f: Vec<&str> = t.split('\t').collect(); return Ok(Some(if f.len() > 1 { f[1].to_string() } else { t.to_string() })); }
            Ok(saves::import_back(&g, Some(&file), false).0)
        }, |l, r: Result<Option<String>, String>| {
            let note = match r { Err(e) => Some(e), Ok(p) => p };
            if note.is_some() { if let Some(v) = l.box_view_mut() { v.note = note; } }
            l.reload_boxes();
        });
    }

    /// MoveNow: first ask mon-bridge what stands in the way. Anything the player must decide goes on a page; a clean
    /// move just happens. `at` is the box screen's slot; `after` goes back to the box screen and reads it again.
    #[allow(clippy::too_many_arguments)]
    pub fn move_now(&mut self, from: &str, file: &str, bx: &str, slot: &str, name: &str, to: &str, at: Option<String>, after: bool) {
        if self.hooks.replay && !save_flows() { self.todo("MoveNow"); return; }
        let (fa, fi, ba, sa, na, ta, aa) = (from.to_string(), file.to_string(), bx.to_string(), slot.to_string(), name.to_string(), to.to_string(), at.clone());
        if self.after_capture(move |l| l.move_now(&fa, &fi, &ba, &sa, &na, &ta, aa, after)) { return; }
        let running = saves::running_games(self.games());
        if !running.is_empty() { self.show_text("Close the game first", &format!("Already running: {}. Saves can't change while it plays.", running)); return; }
        let (Some(_), Some(tg)) = (self.game_by_id(from), self.game_by_id(to)) else { return };
        let m = MoveArgs { from: from.into(), file: file.into(), bx: bx.into(), slot: slot.into(), name: name.into(), to: to.into(), target: String::new(), at, after };
        let (file, bx, slot) = (m.file.clone(), m.bx.clone(), m.slot.clone());
        self.busy(&format!("Checking {}", name), move || {
            let (target, why) = saves::save_of(&tg);
            let Some(target) = target else { return Ok((None, why, String::new(), 0)) };
            let (plan, code) = ui_bridge(&format!("plan \"{}\" {} {} \"{}\"", file, bx, slot, target));
            Ok((Some(target), None, plan, code))
        }, move |l, r: Result<(Option<String>, Option<String>, String, i32), String>| {
            let (target, why, plan, code) = match r { Err(e) => { l.show_text("Not moved", &e); return; } Ok(x) => x };
            let Some(target) = target else { l.show_text("Not moved", &why.unwrap_or_default()); return };
            let rows: Vec<Vec<String>> = plan.split('\n').map(|x| x.trim_end_matches('\r').split('\t').map(|s| s.to_string()).collect::<Vec<_>>()).filter(|f| f[0] != "").collect();
            if code != 0 {
                let n = rows.iter().find(|f| f[0] == "nope");
                l.show_text("Not moved", &match n { Some(n) if n.len() > 1 => n[1].clone(), _ => plan.trim().to_string() });
                return;
            }
            let forced = rows.iter().any(|f| f[0] == "forced");
            let m = MoveArgs { target, ..m };
            if !forced && !rows.iter().any(|f| f[0] == "conflict") { l.run_move(m, String::new()); return; }
            let p = l.resolve_page(m, rows, forced);
            l.push(p);
        });
    }

    /// ResolvePage: a blocked move; the player deletes or changes each conflict, then retries.
    fn resolve_page(&self, m: MoveArgs, rows: Vec<Vec<String>>, forced: bool) -> Page {
        let to = self.game_by_id(&m.to).unwrap();
        let short = to.title.replace("Pokemon ", "");
        let title = format!("{} to {}", m.name, short);
        let mut p = Page::new(&title);
        p.theme = Some(theme_of(&to));
        let st = Rc::new(RefCell::new(Resolve::default()));
        let kept = rows.iter().filter(|f| f[0] == "ok").count();
        let conflicts: Vec<Vec<String>> = rows.iter().filter(|f| f[0] == "conflict").cloned().collect();
        let item = rows.iter().find(|f| f[0] == "item").cloned();
        let nc = conflicts.len();
        let retry_at = nc + item.is_some() as usize;
        // refresh: the Retry item's state, on the resolve page wherever it is in the stack.
        let refresh: Rc<dyn Fn(&mut Launcher)> = {
            let (st, title, conflicts, has_item) = (st.clone(), title.clone(), conflicts.clone(), item.is_some());
            Rc::new(move |l: &mut Launcher| {
                let s = st.borrow();
                let ans = |k: &str| s.answers.iter().find(|a| a.0 == k).map(|a| a.1.clone());
                let left = kept + conflicts.iter().filter(|c| ans(&c[1]).map_or(false, |a| a != "0")).count();
                let done = conflicts.iter().all(|c| ans(&c[1]).is_some()) && (!has_item || s.drop_item);
                let Some(page) = l.pages.iter_mut().rev().find(|p| p.title == title) else { return };
                let r = &mut page.items[retry_at];
                r.on = done && left > 0;
                r.hint = if !done { "Decide each conflict first.".into() } else if left == 0 { "It needs at least one move: change one instead of deleting it.".into() } else { String::new() };
                r.info = if !r.hint.is_empty() { r.hint.clone() } else { "Moves it with your choices. Both saves are backed up first.".into() };
                l.dirty = true;
            })
        };
        let set_text = |title: String, i: usize| move |l: &mut Launcher, t: String| { if let Some(page) = l.pages.iter_mut().rev().find(|p| p.title == title) { page.items[i].text = t; } };
        for (i, c) in conflicts.iter().enumerate() {
            let (number, move_name) = (c[1].clone(), c.get(2).cloned().unwrap_or_default());
            let (st, refresh, to_id, target, set) = (st.clone(), refresh.clone(), m.to.clone(), m.target.clone(), Rc::new(set_text(title.clone(), i)));
            p.items.push(Item::new(&format!("{}: decide", move_name), "", true).icon('\u{E8AB}').info("This move doesn't exist in that game. Delete it, or change it to another move.").run(move |l| {
                let Some(to) = l.game_by_id(&to_id) else { return };
                let mut choice = Page::new(&format!("{} doesn't exist in {}", move_name, to.title.replace("Pokemon ", "")));
                choice.theme = Some(theme_of(&to));
                let (st1, rf1, set1, number1, mn1) = (st.clone(), refresh.clone(), set.clone(), number.clone(), move_name.clone());
                choice.items.push(Item::new("Delete this move", "", true).icon('\u{E74D}').info("It leaves with one move fewer.").run(move |l| {
                    put(&st1, &number1, "0"); set1(l, format!("{}: delete", mn1)); rf1(l); l.pages.pop(); l.dirty = true;
                }));
                let (st2, rf2, set2, number2, mn2, target2, to2) = (st.clone(), refresh.clone(), set.clone(), number.clone(), move_name.clone(), target.clone(), to.clone());
                choice.items.push(Item::new("Change it to...", "", true).icon('\u{E70F}').info("Pick any move from that game.").run(move |l| {
                    let (st, rf, set, number, mn) = (st2.clone(), rf2.clone(), set2.clone(), number2.clone(), mn2.clone());
                    let p = change_move_page(&to2, &target2, Rc::new(move |l: &mut Launcher, id: String, new_name: String| {
                        put(&st, &number, &id); set(l, format!("{} to {}", mn, new_name)); rf(l); l.pages.pop(); l.pages.pop(); l.dirty = true;
                    }));
                    l.push(p);
                }));
                l.push(choice);
            }));
        }
        if let Some(it) = &item {
            let (st, refresh, set, name) = (st.clone(), refresh.clone(), set_text(title.clone(), nc), it.get(1).cloned().unwrap_or_default());
            p.items.push(Item::new(&format!("Remove {}?", name), "", true).icon('\u{E74D}').info("Held items can't cross between these games. Select to agree to removing it.").run(move |l| {
                let d = { let mut s = st.borrow_mut(); s.drop_item = !s.drop_item; s.drop_item };
                set(l, if d { format!("{}: remove", name) } else { format!("Remove {}?", name) });
                refresh(l);
            }));
        }
        {
            let (st, m) = (st.clone(), m.clone());
            p.items.push(Item::new("Retry the move", "", false).icon('\u{E72C}').run(move |l| {
                l.pages.pop();
                let s = st.borrow();
                let opts = s.answers.iter().map(|a| format!("move:{}:{}", a.0, a.1)).collect::<Vec<_>>().join(" ") + if s.drop_item { " dropitem" } else { "" };
                drop(s);
                l.run_move(m.clone(), opts);
            }));
        }
        if forced { p.items.push(Item::new("About this move", "", false).icon('\u{E946}').info("No real transfer exists here, so OpenHome's converter makes the copy. Its form, ball and origin may change, and PKHeX may flag it as not legal. Its original data is kept, and comes back when it returns to its own generation.")); }
        // refresh() before the page is pushed (nothing answered yet), applied to the page itself.
        let (done, left) = (conflicts.is_empty() && item.is_none(), kept);
        let r = &mut p.items[retry_at];
        r.on = done && left > 0;
        r.hint = if !done { "Decide each conflict first.".into() } else if left == 0 { "It needs at least one move: change one instead of deleting it.".into() } else { String::new() };
        r.info = if !r.hint.is_empty() { r.hint.clone() } else { "Moves it with your choices. Both saves are backed up first.".into() };
        p
    }

    /// RunMove: mon-bridge move, then the destination's save is taken back first: a failure after that duplicates
    /// the Pokemon, it never loses it.
    fn run_move(&mut self, m: MoveArgs, options: String) {
        let (Some(from), Some(to)) = (self.game_by_id(&m.from), self.game_by_id(&m.to)) else { return };
        let (file, target, name, to_title) = (m.file.clone(), m.target.clone(), m.name.clone(), to.title.clone());
        let args = format!("move \"{}\" {} {} \"{}\" \"{}\" {}{}", m.file, m.bx, m.slot, m.target, backups(), options, m.at.as_ref().map(|a| format!(" {}", a)).unwrap_or_default());
        let after = m.after;
        self.busy(&format!("Moving {}", m.name), move || {
            let (o, code) = ui_bridge(&args);
            let t = o.trim().to_string();
            let f: Vec<&str> = t.split('\t').collect();
            if code != 0 { return Ok((false, false, if f.len() > 1 { f[1].to_string() } else { t.clone() })); }
            let back = saves::import_back(&to, Some(&target), false).0.or_else(|| saves::import_back(&from, Some(&file), false).0);
            let ok = back.is_none();
            // ok, name, legal or not-legal, restored (or empty), and what PKHeX's legality check found
            let restored = f.len() > 3 && f[3] == "restored";
            let not_legal = f.len() > 2 && f[2] == "not-legal";
            let why = if f.len() > 4 { f[4].trim().to_string() } else { String::new() };
            let notes = ok && (not_legal || restored);
            let message = if ok {
                format!("{} is now in {}'s PC boxes.{}{}", name, to_title,
                    if restored { "\n\nIts original data is back: its origin, ball and moves, as they were before it left." } else { "" },
                    if not_legal { format!("\n\nPKHeX's legality check flags it as not legal, which matters only for online play.{}{}", if !why.is_empty() { format!("\n\nWhat it found: {}.", why) } else { String::new() }, if restored { "" } else { "\n\nIts original data is kept, and comes back if it returns to its own generation." }) } else { String::new() })
            } else { back.unwrap() };
            Ok((ok, notes, message))
        }, move |l, r: Result<(bool, bool, String), String>| {
            let (ok, notes, message, failed) = match r { Ok((ok, n, m)) => (ok, n, m, false), Err(e) => (false, false, e, true) };
            if after {
                // From the box screen: back to it, read again; a word only when there's something to say.
                while l.pages.len() > 1 && l.pages.last().unwrap().boxes.is_none() { l.pages.pop(); }
                l.reload_boxes();
                if failed || !ok || notes { l.show_text(if ok { "Moved" } else { "Not moved" }, &message); }
                return;
            }
            while l.pages.len() > 1 { l.pages.pop(); }
            l.show_text(if ok { "Moved" } else { "Not moved" }, &message);
        });
    }
}

#[derive(Clone)]
struct MoveArgs { from: String, file: String, bx: String, slot: String, name: String, to: String, target: String, at: Option<String>, after: bool }

fn put(st: &Rc<RefCell<Resolve>>, k: &str, v: &str) {
    let mut s = st.borrow_mut();
    match s.answers.iter_mut().find(|a| a.0 == k) { Some(a) => a.1 = v.to_string(), None => s.answers.push((k.to_string(), v.to_string())) }
}

/// ChangeMovePage: every move the target game has (mon-bridge moves, read at once as the C# does).
fn change_move_page(to: &GameEntry, target: &str, chosen: Rc<dyn Fn(&mut Launcher, String, String)>) -> Page {
    let mut p = Page::new("Change to which move?");
    p.theme = Some(theme_of(to));
    let (list, _) = ui_bridge(&format!("moves \"{}\"", target));
    for row in list.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 2 { continue; }
        let (id, n) = (f[0].to_string(), f[1].to_string());
        let c = chosen.clone();
        let nn = n.clone();
        p.items.push(Item::new(&n, "", true).icon('\u{E8AB}').info(&format!("Use {} instead.", n)).run(move |l| c(l, id.clone(), nn.clone())));
    }
    p
}
