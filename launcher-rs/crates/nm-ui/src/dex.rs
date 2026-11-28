//! The Pokedex (My Collection): LoadDex with its cache, the home, region, game and list pages, and their rows
//! (RegionRow, DexRow, Motif), and the entry page (RenderDexEntry, DexInfos, mon-bridge stats), ported from the C#.

use crate::art::*;
use crate::launcher::Launcher;
use crate::model::{Item, Page, Theme};
use crate::pages::{theme_of, Ctx};
use nm_core::game_entry::GameEntry;
use nm_core::saves::DexGame;
use nm_draw::text::StringAlignment::{Center, Far, Near};
use nm_draw::*;
use std::collections::HashMap;

pub const REGIONS: [&str; 9] = ["Kanto", "Johto", "Hoenn", "Sinnoh", "Unova", "Kalos", "Alola", "Galar", "Paldea"];
pub const REGION_START: [i32; 10] = [1, 152, 252, 387, 494, 650, 722, 810, 906, 1026];
pub const MAX_DEX: i32 = 1025;

/// What the pages read: the names, and each game's Pokedex (by game id).
#[derive(Default)]
pub struct Dex {
    pub names: HashMap<i32, String>,
    pub by: Vec<(String, DexGame)>,
}

/// Romhacks and fan games never count toward the collection: each has its own Pokedex.
pub fn counts_in_collection(g: &GameEntry) -> bool { !g.is_hack() && g.pc.is_none() }
fn reads_dex(g: &GameEntry) -> bool {
    g.save_tools() || (nm_core::pc_games::is_fusion(g) && g.on_pc()) || (g.ctr.is_some() && ctr_save_in(g).is_some())
}
fn ctr_save_in(g: &GameEntry) -> Option<std::path::PathBuf> {
    let m = nm_core::ctr3ds_saves::main(g);
    if m.is_file() { return Some(m); }
    nm_core::save_sets::all(g).into_iter().map(|s| nm_core::save_sets::dir(g, &s).join("3ds").join("data").join("main")).find(|f| f.is_file())
}

/// .NET ticks (100 ns since 0001-01-01) of a file's last write, in UTC.
fn ticks(m: &std::fs::Metadata) -> i64 {
    let since = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as i64 / 100);
    621_355_968_000_000_000 + since
}
/// DexPrint: what a game's Pokedex was read from; when it changes, it's read again.
fn dex_print(g: &GameEntry) -> String {
    let mut o = format!("held|{}|{}|{}", g.engine(), nm_core::save_sets::active(g), nm_core::save_sets::all(g).join(","));
    let file = |o: &mut String, f: &std::path::Path| {
        if let Ok(m) = std::fs::metadata(f) { if m.is_file() { o.push_str(&format!("|{}:{}:{}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), m.len(), ticks(&m))); } }
    };
    // Read again when the list grows, as its later Pokemon move into it.
    if nm_core::pc_games::is_fusion(g) { o.push_str(&format!("|{}", crate::sprites::MAX_DEX)); for f in if_saves(g) { file(&mut o, &f); } return o; }
    let folder = |o: &mut String, d: &std::path::Path| {
        let mut all = Vec::new();
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(d) { for e in rd.flatten() { let p = e.path(); if p.is_dir() { walk(&p, out); } else { out.push(p); } } }
        }
        walk(d, &mut all);
        all.sort_by(|a, b| a.to_string_lossy().as_bytes().cmp(b.to_string_lossy().as_bytes()));
        for f in all { file(o, &f); }
    };
    if g.save.is_some() || g.ctr.is_some() { file(&mut o, &nm_core::saves::master_of(g)); }
    folder(&mut o, &nm_core::config::state().join("save-sets").join(&g.id));
    if let Some(v) = &g.gen1 { folder(&mut o, &nm_core::engines::gen1_root().join("saves").join(v)); }
    o
}
/// IfSaves: Infinite Fusion's saves (*.rxdata), in name order ignoring case.
fn if_saves(g: &GameEntry) -> Vec<std::path::PathBuf> {
    let mut l: Vec<std::path::PathBuf> = std::fs::read_dir(nm_core::pc_games::save_dir(g)).map(|rd| rd.flatten().map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map_or(false, |x| x.to_string_lossy().eq_ignore_ascii_case("rxdata"))).collect()).unwrap_or_default();
    l.sort_by_key(|p| p.to_string_lossy().to_uppercase());
    l
}
/// IfDex: Infinite Fusion's own Pokedex, its later Pokemon matched to National Dex numbers by name.
fn if_dex(g: &GameEntry, names: &HashMap<i32, String>) -> Option<DexGame> {
    let saves = if_saves(g);
    if saves.is_empty() { return None; }
    let mut order: Vec<(&i32, &String)> = names.iter().collect();
    order.sort();
    let mut national: HashMap<String, i32> = HashMap::new();
    for (id, n) in order { national.entry(nm_core::if_dex::plain(n)).or_insert(*id); }
    let r = nm_core::if_dex::read(std::path::Path::new(&nm_core::pc_games::dir(g)?), &saves, &national).ok()?;
    Some(DexGame { caught: r.caught, seen: r.seen, fusions_caught: r.fusions_caught, fusions_seen: r.fusions_seen, later_caught: r.later_caught.len() as i32, later_seen: r.later_seen.len() as i32, ..Default::default() })
}
fn ids(s: &std::collections::BTreeSet<i32>) -> String { s.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",") }
fn read_ids(t: &str, into: &mut std::collections::BTreeSet<i32>) { for id in t.split(',') { if let Ok(n) = id.parse() { into.insert(n); } } }

/// LoadDex: the names from mon-bridge, then each game's Pokedex, read again only where its saves changed.
pub fn load_dex(c: &Ctx) -> Result<Dex, String> { load_dex_of(&c.lib.games, None) }

static DEX_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static DEX_STALE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Whether a background read changed what's kept since the Pokedex was loaded (it clears on reading).
pub fn take_stale() -> bool { DEX_STALE.swap(false, std::sync::atomic::Ordering::SeqCst) }
/// LoadDex(true): on the capture thread, it gives way to anything the player starts and keeps what it has so far.
pub fn load_dex_background(games: &[GameEntry], busy: &dyn Fn() -> bool) {
    let stop = || nm_core::art_jobs::hold() || busy();
    let _ = load_dex_of(games, Some(&stop));
}
fn load_dex_of(games: &[GameEntry], stop: Option<&dyn Fn() -> bool>) -> Result<Dex, String> {
    let _held = DEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cache = nm_core::config::state().join("dex-cache.txt");
    // dex-cache.txt: id, print, then caught, seen, shiny (or "none").
    let mut kept: Vec<(String, String, Option<DexGame>)> = Vec::new();
    if let Ok(t) = std::fs::read_to_string(&cache) {
        for l in t.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 3 { continue; }
            let d = if f.len() >= 5 {
                let mut d = DexGame::default(); read_ids(f[2], &mut d.caught); read_ids(f[3], &mut d.seen); read_ids(f[4], &mut d.shiny);
                // int.TryParse: 0 when it doesn't read.
                let n = |i: usize| f[i].trim().parse::<i32>().unwrap_or(0);
                if f.len() >= 7 { d.fusions_caught = n(5); d.fusions_seen = n(6); }
                if f.len() >= 9 { d.later_caught = n(7); d.later_seen = n(8); }
                Some(d)
            } else { None };
            match kept.iter_mut().find(|k| k.0 == f[0]) { Some(k) => { k.1 = f[1].into(); k.2 = d; } None => kept.push((f[0].into(), f[1].into(), d)) }
        }
    }
    let (o, code) = nm_core::save_guard::bridge("names");
    if code != 0 { return Err("Couldn't read the Pokemon names.".into()); }
    let mut dex = Dex::default();
    for line in o.split('\n') {
        let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        if f.len() == 2 { if let Ok(id) = f[0].parse() { dex.names.insert(id, f[1].to_string()); } }
    }
    let mut changed = false;
    for g in games.iter().filter(|g| reads_dex(g)) {
        if stop.map_or(false, |f| f()) { break; }
        let print = dex_print(g);
        if kept.iter().any(|k| k.0 == g.id && k.1 == print) { continue; }
        let d = if g.pc.is_some() { if_dex(g, &dex.names) } else { nm_core::saves::dex_in_all_saves(games, g) };
        // Stopped meanwhile: it's read again later, not remembered wrong.
        if stop.is_some() && nm_core::art_jobs::hold() { continue; }
        match kept.iter_mut().find(|k| k.0 == g.id) { Some(k) => { k.1 = print; k.2 = d; } None => kept.push((g.id.clone(), print, d)) }
        changed = true;
    }
    if changed {
        let lines: Vec<String> = kept.iter().map(|(id, print, d)| match d { None => format!("{}\t{}\tnone", id, print), Some(d) => format!("{}\t{}\t{}\t{}\t{}{}", id, print, ids(&d.caught), ids(&d.seen), ids(&d.shiny),
            if d.fusions_seen >= 0 { format!("\t{}\t{}\t{}\t{}", d.fusions_caught, d.fusions_seen, d.later_caught, d.later_seen) } else { String::new() }) }).collect();
        let _ = nm_core::config::write_all_lines(&cache, &lines);
        if stop.is_some() { DEX_STALE.store(true, std::sync::atomic::Ordering::SeqCst); }
    }
    for g in games.iter().filter(|g| reads_dex(g)) {
        if let Some((_, _, Some(d))) = kept.iter().find(|k| k.0 == g.id) { dex.by.push((g.id.clone(), d.clone())); }
    }
    Ok(dex)
}

impl Dex {
    pub fn name(&self, id: i32) -> String { self.names.get(&id).cloned().unwrap_or_else(|| format!("#{}", id)) }
    fn of<'a>(&'a self, c: &'a Ctx, only: Option<&'a str>) -> impl Iterator<Item = &'a DexGame> + 'a {
        self.by.iter().filter(move |(id, _)| match only { Some(o) => id == o, None => c.lib.games.iter().find(|g| &g.id == id).map_or(false, counts_in_collection) }).map(|(_, d)| d)
    }
    pub fn caught(&self, c: &Ctx, id: i32, only: Option<&str>) -> bool { self.of(c, only).any(|d| d.caught.contains(&id)) }
    pub fn seen(&self, c: &Ctx, id: i32, only: Option<&str>) -> bool { self.of(c, only).any(|d| d.seen.contains(&id) || d.caught.contains(&id)) }
    pub fn shiny(&self, c: &Ctx, id: i32, only: Option<&str>) -> bool { self.of(c, only).any(|d| d.shiny.contains(&id)) }
    fn get(&self, id: &str) -> Option<&DexGame> { self.by.iter().find(|(g, _)| g == id).map(|(_, d)| d) }
    /// The snapshots' made-up collection (NATIVEMONS_TEST_DEX=sample), to see every kind of row.
    pub fn sample(&mut self, c: &Ctx) {
        let mut d = DexGame::default();
        d.caught.extend([1, 4, 7, 16, 19, 21, 25]);
        d.seen.extend([2, 10, 13, 17, 20, 23, 24]);
        self.by.clear();
        let counted: Vec<&GameEntry> = c.lib.games.iter().filter(|x| counts_in_collection(x)).take(2).collect();
        if counted.len() > 1 {
            let mut d2 = DexGame::default();
            d2.caught.extend([16, 19, 25]);
            d2.shiny.insert(19);
            self.by.push((counted[0].id.clone(), d));
            self.by.push((counted[1].id.clone(), d2));
        } else {
            d.shiny.insert(19);
            if let Some(g) = counted.first() { self.by.push((g.id.clone(), d)); }
        }
    }
}

fn region_themes() -> [Theme; 9] {
    let t = |a: &str, b: &str, c: &str| Theme::make(a, b, c, Some("plain"));
    [t("#E3350D", "#A51E0A", "#FFD9CF"), t("#C9A227", "#8C6A12", "#F7EBC4"), t("#1F8A70", "#0F5E4B", "#CDEFE4"), t("#6A5ACD", "#463A9A", "#E3DEFA"),
     t("#3A3F4B", "#1E2229", "#E2E4E8"), t("#2E6FD8", "#1B4A9C", "#D6E4FB"), t("#F28C28", "#C2601A", "#FFE7CC"), t("#9B2D6F", "#65184A", "#F4D6E8"), t("#C8384B", "#7A2A8F", "#F6D9E0")]
}
pub fn region_theme(region: i32) -> Theme { if (0..9).contains(&region) { region_themes()[region as usize].clone() } else { Theme::default_theme() } }

fn dex_count(d: &Dex, c: &Ctx, from: i32, to: i32) -> (i32, String) {
    let n = (from..=to).filter(|&i| d.caught(c, i, None)).count() as i32;
    (n, format!("{} of {}", n, to - from + 1))
}

pub fn dex_home_page(c: &Ctx, d: &Dex) -> Page {
    let mut p = Page::new("Pokedex");
    let last = REGION_START[REGIONS.len()] - 1;
    let (n, s) = dex_count(d, c, 1, last);
    let mut it = Item::new(&format!("National: {}", s), "", true).icon('\u{E8F1}').info(&format!("Every Pokemon from 001 to {:04}.", last)).run(move |l| l.push_dex(|c, d| dex_list_page(c, d, "National", 1, last, None)));
    it.region = 9; it.region_caught = n; it.region_total = last;
    p.items.push(it);
    for i in 0..REGIONS.len() {
        let (a, b) = (REGION_START[i], REGION_START[i + 1] - 1);
        let (n, s) = dex_count(d, c, a, b);
        let mut it = Item::new(&format!("{}: {}", REGIONS[i], s), "", true).icon('\u{E707}').info(&format!("Numbers {:03} to {:03}, the Pokemon introduced in {}.", a, b, REGIONS[i])).run(move |l| l.push_dex(|c, d| dex_list_page(c, d, REGIONS[i], a, b, None)));
        it.region = i as i32; it.region_caught = n; it.region_total = b - a + 1;
        p.items.push(it);
    }
    p.items.push(Item::new("By game", "", true).icon('\u{E7FC}').info("How many each of your games has caught.").run(|l| l.push_dex(dex_games_page)));
    if c.lib.games.iter().any(own_section) {
        p.items.push(Item::new("Romhacks and fan games", "", true).icon('\u{E734}').info("Each with a Pokedex of its own. They never count toward your collection above.").run(|l| l.push_dex(dex_other_page)));
    }
    p
}
/// Romhacks you have, and Infinite Fusion once it's installed.
fn own_section(g: &GameEntry) -> bool { (g.is_hack() && g.playable()) || (nm_core::pc_games::is_fusion(g) && g.on_pc()) }

pub fn dex_other_page(c: &Ctx, d: &Dex) -> Page {
    let mut p = Page::new("Romhacks and fan games");
    for g in c.lib.games.iter().filter(|g| own_section(g)) {
        let has = d.get(&g.id);
        if g.pc.is_some() {
            let (caught_all, seen_all) = has.map_or((0, 0), |x| (x.caught.len() as i32 + x.later_caught, x.seen.len() as i32 + x.later_seen));
            let (title, id) = (g.title.clone(), g.id.clone());
            let last = REGION_START[REGIONS.len()] - 1;
            let mut it = Item::new(&format!("{}: {}", g.title, if has.is_some() { format!("{} caught", caught_all) } else { "no save".into() }), "", has.is_some()).icon('\u{E7FC}')
                .run(move |l| l.push_dex(|c, d| dex_list_page(c, d, &title, 1, last, Some(&id))));
            it.theme = Some(theme_of(g));
            it.info = match has {
                Some(x) => format!("Its own Pokedex, read from its saves: {} caught and {} seen{}, and {} fusions caught, {} seen. It doesn't count toward your collection.", caught_all, seen_all,
                    if x.later_seen > 0 { format!(" ({} of them outside the National Dex list)", x.later_seen) } else { String::new() }, x.fusions_caught, x.fusions_seen),
                None => "No save to read yet, or its saves couldn't be read. Play and save once.".into(),
            };
            if let Some(x) = has.filter(|x| x.fusions_seen >= 0) { it.records = vec![format!("Fusions: {} caught, {} seen", x.fusions_caught, x.fusions_seen)]; }
            p.items.push(it);
            continue;
        }
        let last = (if g.dex > 0 { g.dex } else { REGION_START[REGIONS.len()] - 1 }).min(REGION_START[REGIONS.len()] - 1);
        let (title, id) = (g.title.clone(), g.id.clone());
        let mut it = Item::new(&format!("{}: {}", g.title, match has { Some(x) => format!("{} caught", x.caught.len()), None => "no save".into() }), "", has.is_some()).icon('\u{E7FC}')
            .run(move |l| l.push_dex(|c, d| dex_list_page(c, d, &title, 1, last, Some(&id))));
        it.theme = Some(theme_of(g));
        let base = c.lib.games.iter().find(|x| Some(&x.id) == g.base_id.as_ref()).map(|x| x.title.clone()).unwrap_or_else(|| "its base game".into());
        it.info = if has.is_some() { format!("Its own Pokedex, read from its saves as {}'s. It doesn't count toward your collection.", base) } else { "No save to read yet. Play and save once.".into() };
        p.items.push(it);
    }
    p
}

pub fn dex_games_page(c: &Ctx, d: &Dex) -> Page {
    let mut p = Page::new("By game");
    for g in c.lib.games.iter().filter(|x| (x.save_tools() || (x.ctr.is_some() && reads_dex(x))) && counts_in_collection(x)) {
        let has = d.get(&g.id);
        let mut it = Item::new(&format!("{}: {}", g.title, match has { Some(x) => format!("{} of {}", x.caught.len(), g.dex), None => "no save".into() }), "", has.is_some()).icon('\u{E7FC}');
        it.theme = Some(theme_of(g));
        let saves = nm_core::save_sets::all(g).len();
        it.info = if has.is_some() { format!("{}, out of the {} entries in this game.", if saves > 1 { format!("Different Pokemon caught across your {} saves", saves) } else { "Caught in this save".into() }, g.dex) } else { "No save to read yet. Play and save once.".into() };
        if let Some(x) = has {
            let only = x.caught.iter().filter(|n| !d.by.iter().any(|(o, od)| o != &g.id && c.lib.games.iter().find(|q| &q.id == o).map_or(false, counts_in_collection) && od.caught.contains(n))).count();
            it.records = vec![format!("Only here: {}", only)];
        }
        p.items.push(it);
    }
    p
}

fn region_of(from: i32, to: i32) -> i32 { (0..REGIONS.len()).find(|&i| REGION_START[i] == from && REGION_START[i + 1] - 1 == to).map_or(-1, |i| i as i32) }

/// DexListPage(title, from, to, only).
pub fn dex_list_page(c: &Ctx, d: &Dex, title: &str, from: i32, to: i32, only: Option<&str>) -> Page {
    let region = if only.is_none() { region_of(from, to) } else { -1 };
    let mut p = Page::new(title);
    p.jump = 10;
    p.dex_only = only.map(String::from);
    if region >= 0 { p.theme = Some(region_theme(region)); }
    let order: std::rc::Rc<Vec<i32>> = std::rc::Rc::new((from..=to).filter(|&x| d.seen(c, x, only)).collect());
    for id in from..=to {
        let (caught, seen, star) = (d.caught(c, id, only), d.seen(c, id, only), d.shiny(c, id, only));
        let (order, only_s) = (order.clone(), only.map(String::from));
        let run = move |l: &mut crate::launcher::Launcher| { let e = l.dex_entry_page_now(id, (*order).clone(), only_s.clone()); l.push(e); };
        let label = format!("No.{:03} {}: {}", id, if seen { d.name(id) } else { "----------".into() }, if star { "Caught, shiny" } else if caught { "Caught" } else if seen { "Seen" } else { "Not seen yet" });
        let mut it = Item::new(&label, "", seen).icon('\u{E9CE}').run(run);

        it.sprite = if seen { Some(format!("{}{}", if star { "shiny/" } else { "" }, id)) } else { None };
        it.dex_no = id;
        it.dex_state = if caught { 2 } else if seen { 1 } else { 0 };
        it.dex_star = star;
        it.region = region;
        if caught { it.dex_entry = Some(std::rc::Rc::new(dex_entry_data(c, d, id, only))); }
        it.info = if caught { "In your collection: caught, traded for, or moved in.".into() } else if seen { "Seen, not caught yet.".into() } else { "Not met yet. Its name shows once you've seen it.".into() };
        p.items.push(it);
    }
    p
}

/// Motif: a region's motif in one color, over its band or tile. 9 is the whole National dex.
pub fn motif(g: &mut Graphics, r: RectangleF, region: i32, c: Color) {
    let st = g.save();
    g.set_clip_rect(r);
    g.smoothing_mode = SmoothingMode::AntiAlias;
    let u = r.width.min(r.height);
    let lw = (u * 0.035).max(1.5);
    let pen = Pen::new(c, lw);
    let b = solid(c);
    let (cos, sin) = (|a: f64| a.cos() as f32, |a: f64| a.sin() as f32);
    match region {
        0 => {
            for k in [[0.78f32, 0.3, 0.32], [0.16, 0.82, 0.22], [0.5, 0.95, 0.14]] {
                let (cx, cy, rr) = (r.x + r.width * k[0], r.y + r.height * k[1], u * k[2]);
                g.draw_ellipse(&pen, RectangleF::new(cx - rr, cy - rr, rr * 2.0, rr * 2.0));
                g.draw_line_f(&pen, cx - rr, cy, cx + rr, cy);
                g.draw_ellipse(&pen, RectangleF::new(cx - rr * 0.3, cy - rr * 0.3, rr * 0.6, rr * 0.6));
            }
        }
        1 => {
            let (cx, cy) = (r.x + r.width * 0.85, r.bottom());
            for i in 0..9 {
                let a = std::f64::consts::PI + i as f64 * std::f64::consts::PI / 8.0;
                g.draw_line_f(&pen, cx + cos(a) * u * 0.3, cy + sin(a) * u * 0.3, cx + cos(a) * u * 1.4, cy + sin(a) * u * 1.4);
            }
            g.draw_ellipse(&pen, RectangleF::new(cx - u * 0.22, cy - u * 0.22, u * 0.44, u * 0.44));
        }
        2 => {
            for row in 0..4 {
                let mut pts = Vec::new();
                let mut x = r.x;
                while x <= r.right() + 4.0 {
                    pts.push(PointF::new(x, r.y + r.height * (0.35 + row as f32 * 0.2) + (((x - r.x) / (u * 0.18) + row as f32) as f64).sin() as f32 * u * 0.05));
                    x += u * 0.05;
                }
                g.draw_lines(&pen, &pts);
            }
        }
        3 => {
            let base = r.bottom();
            g.draw_lines(&pen, &[PointF::new(r.x - u * 0.1, base), PointF::new(r.x + r.width * 0.3, r.y + r.height * 0.3), PointF::new(r.x + r.width * 0.5, r.y + r.height * 0.6), PointF::new(r.x + r.width * 0.72, r.y + r.height * 0.15), PointF::new(r.right() + u * 0.1, base)]);
            for i in 0..7 {
                let (x, y) = (r.x + r.width * (0.08 + i as f32 * 0.14), r.y + r.height * (0.15 + (i % 3) as f32 * 0.12));
                g.fill_ellipse(&b, RectangleF::new(x, y, lw * 1.6, lw * 1.6));
            }
        }
        4 => {
            let mut x = r.x;
            let hs = [0.35f32, 0.6, 0.45, 0.8, 0.5, 0.68, 0.4, 0.55];
            let mut i = 0;
            while x < r.right() {
                let (w, hh) = (u * 0.16, r.height * hs[i % hs.len()]);
                g.draw_rectangle(&pen, RectangleF::new(x, r.bottom() - hh, w, hh + lw));
                x += w + u * 0.05;
                i += 1;
            }
        }
        5 => {
            let mut d = -r.height;
            while d < r.width {
                g.draw_line_f(&pen, r.x + d, r.bottom(), r.x + d + r.height, r.y);
                g.draw_line_f(&pen, r.x + d, r.y, r.x + d + r.height, r.bottom());
                d += u * 0.22;
            }
        }
        6 => {
            let (cx, cy, rr) = (r.x + r.width * 0.75, r.y + r.height * 0.45, u * 0.28);
            g.fill_ellipse(&b, RectangleF::new(cx - rr, cy - rr, rr * 2.0, rr * 2.0));
            for i in 0..4 {
                let y = r.y + r.height * (0.72 + i as f32 * 0.08);
                g.draw_line_f(&pen, r.x + r.width * (0.1 + i as f32 * 0.05), y, r.right() - r.width * (0.05 + i as f32 * 0.04), y);
            }
        }
        7 => {
            let thin = Pen::new(c, lw * 0.4);
            let mut x = r.x;
            while x < r.right() { g.draw_line_f(&pen, x, r.y, x, r.bottom()); g.draw_line_f(&thin, x + u * 0.08, r.y, x + u * 0.08, r.bottom()); x += u * 0.3; }
            let mut y = r.y;
            while y < r.bottom() { g.draw_line_f(&pen, r.x, y, r.right(), y); g.draw_line_f(&thin, r.x, y + u * 0.08, r.right(), y + u * 0.08); y += u * 0.3; }
        }
        8 => {
            let t = u * 0.28;
            let mut y = r.y;
            while y < r.bottom() {
                let mut x = r.x;
                while x < r.right() {
                    g.draw_rectangle(&pen, RectangleF::new(x, y, t, t));
                    g.draw_arc(&pen, RectangleF::new(x - t / 2.0, y - t / 2.0, t, t), 0.0, 90.0);
                    g.draw_arc(&pen, RectangleF::new(x + t / 2.0, y + t / 2.0, t, t), 180.0, 90.0);
                    x += t;
                }
                y += t;
            }
        }
        _ => {
            let (cx, cy) = (r.x + r.width * 0.8, r.y + r.height * 0.4);
            for i in 1..=6 { let rr = u * 0.14 * i as f32; g.draw_ellipse(&pen, RectangleF::new(cx - rr, cy - rr, rr * 2.0, rr * 2.0)); }
        }
    }
    g.restore(st);
}

impl Launcher {
    /// RegionRow: a tile with the region's motif in its colors, its name, and a bar of what's caught.
    pub(crate) fn region_row(&mut self, g: &mut Graphics, it: &Item, r: RectangleF) {
        let th = region_theme(it.region);
        let tile = RectangleF::new(r.x, r.y, r.height * 2.1, r.height);
        {
            let st = g.save();
            g.set_clip_path(&round(r, r.height / 2.0));
            let mut lg = LinearGradient::new(RectangleF::new(tile.x - 1.0, tile.y, tile.width + 2.0, tile.height), mix(th.main, Color::WHITE, 0.15), th.deep, 0.0);
            lg.wrap = WrapMode::Tile;
            g.fill_rectangle(&Brush::Linear(lg), tile);
            motif(g, tile, it.region, Color::from_argb(70, 255, 255, 255));
            g.restore(st);
        }
        let pad = r.height * 0.3;
        let x = tile.right() + pad;
        let right = r.right() - pad * 1.2;
        let name = match it.text.find(": ") { Some(c) if c > 0 => it.text[..c].to_string(), _ => it.text.clone() };
        let count = format!("{} / {}", it.region_caught, it.region_total);
        let (item, bold) = (self.fonts.item.clone(), self.fonts.item_bold.clone());
        let count_w = g.measure_string(&count, &item).width + 4.0;
        let bar_h = (r.height * 0.1).max(4.0);
        let by = r.bottom() - r.height * 0.2 - bar_h / 2.0;
        let ty = r.y + r.height * 0.06;
        let line_h = by - ty - r.height * 0.06;
        self.w(g, &name, &bold, PHONE_TEXT, RectangleF::new(x, ty, right - x - count_w, line_h), Near, Center, false);
        self.w(g, &count, &item, PHONE_SUB, RectangleF::new(right - count_w, ty, count_w, line_h), Far, Center, false);
        let bar = RectangleF::new(x, by, right - x, bar_h);
        g.fill_path(&solid(Color::rgb(226, 226, 232)), &round(bar, bar_h / 2.0));
        let k = if it.region_total > 0 { it.region_caught as f32 / it.region_total as f32 } else { 0.0 };
        if k > 0.0 { g.fill_path(&solid(th.main), &round(RectangleF::new(x, by, bar_h.max(bar.width * k), bar_h), bar_h / 2.0)); }
    }

    /// DexRow: a Poke Ball for one caught, the number, the name (dashes before it's seen), its picture once seen.
    pub(crate) fn dex_row(&mut self, g: &mut Graphics, it: &Item, r: RectangleF) {
        let pad = r.height * 0.28;
        let ball_r = r.height * 0.2;
        let mut x = r.x + pad;
        if it.dex_state == 2 {
            ball(g, PointF::new(x + ball_r, r.y + r.height / 2.0), ball_r, &Pen::new(INK, (ball_r * 0.16).max(1.0)));
        } else {
            g.draw_ellipse(&Pen::new(Color::from_argb(if it.dex_state == 1 { 120 } else { 60 }, 150, 150, 160), (ball_r * 0.14).max(1.0)), RectangleF::new(x, r.y + r.height / 2.0 - ball_r, ball_r * 2.0, ball_r * 2.0));
        }
        x += ball_r * 2.0 + pad * 0.8;
        let no = format!("No.{:03}", it.dex_no);
        let mut title = it.text[no.len() + 1..].to_string();
        if let Some(colon) = title.rfind(": ") { if colon > 0 { title.truncate(colon); } }
        let (item, bold) = (self.fonts.item.clone(), self.fonts.item_bold.clone());
        let no_w = g.measure_string("No.0000", &bold).width;
        let ink2 = if it.dex_state == 0 { TEXT3 } else { PHONE_TEXT };
        self.w(g, &no, &bold, ink2, RectangleF::new(x, r.y, no_w, r.height), Near, Center, false);
        let pic = r.height * 1.15;
        let right = r.right() - pad * 0.6;
        if it.dex_state > 0 {
            if let Some(sp) = it.sprite.as_deref().and_then(crate::sprites::get) {
                let st = g.save();
                g.set_clip_rect(r);
                draw_sprite(g, &sp, RectangleF::new(right - pic, r.y + (r.height - pic) / 2.0 - r.height * 0.06, pic, pic));
                g.restore(st);
            } else if crate::sprites::loading(it.sprite.as_deref()) { let t = self.now(); spinning_ball(g, right - pic / 2.0, r.y + r.height / 2.0, r.height * 0.5, t, false); self.spinning = true; }
            if it.dex_star {
                let ih = item.height() as f32;
                self.w(g, "\u{2605}", &item, Color::rgb(226, 164, 16), RectangleF::new(right - pic - ih * 1.4, r.y, ih * 1.4, r.height), Center, Center, false);
            }
        }
        self.w(g, &title, &item, ink2, RectangleF::new(x + no_w, r.y, right - pic - x - no_w, r.height), Near, Center, false);
    }
}

// ---------- the entry page ----------

/// DexInfo: PokeAPI's entry, kept in state\pokedex-cache\<id>.json in the C#'s own form.
#[derive(Clone, Debug, Default)]
pub struct DexInfo {
    pub genus: String,
    pub height: i32,
    pub weight: i32,
    pub types: Vec<String>,
    pub abilities: Vec<String>,
    pub stats: Vec<i32>,
    pub flavor: Vec<(String, String)>,
}
fn json_strs(v: &serde_json::Value) -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() }
/// DexInfos.Get, fetched now: from the cache, else from PokeAPI (then kept).
pub fn dex_info(id: i32) -> Option<DexInfo> {
    let file = nm_core::config::state().join("pokedex-cache").join(format!("{}.json", id));
    if let Some(v) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok()) {
        return Some(DexInfo {
            genus: v["Genus"].as_str().unwrap_or("").into(),
            height: v["Height"].as_i64().unwrap_or(0) as i32,
            weight: v["Weight"].as_i64().unwrap_or(0) as i32,
            types: json_strs(&v["Types"]),
            abilities: json_strs(&v["Abilities"]),
            stats: v["Stats"].as_array().map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0) as i32).collect()).unwrap_or_default(),
            flavor: v["Flavor"].as_object().map(|o| o.iter().map(|(k, x)| (k.clone(), x.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default(),
        });
    }
    let get = |u: &str| nm_core::net::download_string_unnoted(u).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let species = get(&format!("https://pokeapi.co/api/v2/pokemon-species/{}", id))?;
    let mon = get(&format!("https://pokeapi.co/api/v2/pokemon/{}", id))?;
    let name = |row: &serde_json::Value, key: &str| row[key]["name"].as_str().unwrap_or("").to_string();
    let mut info = DexInfo { height: mon["height"].as_i64().unwrap_or(0) as i32, weight: mon["weight"].as_i64().unwrap_or(0) as i32, ..Default::default() };
    info.types = mon["types"].as_array().map(|a| a.iter().map(|x| name(x, "type")).collect()).unwrap_or_default();
    info.abilities = mon["abilities"].as_array().map(|a| a.iter().map(|x| name(x, "ability")).collect()).unwrap_or_default();
    info.stats = mon["stats"].as_array().map(|a| a.iter().take(6).map(|x| x["base_stat"].as_i64().unwrap_or(0) as i32).collect()).unwrap_or_default();
    for row in species["genera"].as_array().cloned().unwrap_or_default() { if name(&row, "language") == "en" { info.genus = row["genus"].as_str().unwrap_or("").into(); } }
    let spaces = regex::Regex::new(" +").unwrap();
    for row in species["flavor_text_entries"].as_array().cloned().unwrap_or_default() {
        if name(&row, "language") != "en" { continue; }
        let text = row["flavor_text"].as_str().unwrap_or("").replace("-\n", "-").replace('\n', " ").replace('\u{c}', " ").replace("\u{ad} ", "").replace("POK\u{e9}MON", "Pok\u{e9}mon").replace("POKeMON", "Pok\u{e9}mon");
        let v = name(&row, "version");
        let t = spaces.replace_all(&text, " ").trim().to_string();
        match info.flavor.iter_mut().find(|(k, _)| *k == v) { Some(e) => e.1 = t, None => info.flavor.push((v, t)) }
    }
    let json = serde_json::json!({ "Genus": info.genus, "Height": info.height, "Weight": info.weight, "Types": info.types, "Abilities": info.abilities, "Stats": info.stats,
        "Flavor": serde_json::Value::Object(info.flavor.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect()) });
    let _ = std::fs::create_dir_all(file.parent().unwrap());
    let _ = std::fs::write(&file, json.to_string());
    Some(info)
}

/// DexInfos.GetAll on three workers: every entry for offline use, the ones here kept. (fresh, total, failed)
pub fn get_all_infos(total: i32, report: &mut dyn FnMut(usize, usize, usize, usize)) -> (usize, usize, usize) {
    let next = std::sync::atomic::AtomicI32::new(1);
    let counts = std::sync::Mutex::new((0usize, 0usize, 0usize));
    let (tx, rx) = std::sync::mpsc::channel::<(usize, usize, usize)>();
    std::thread::scope(|sc| {
        for _ in 0..3 {
            let (next, c, tx) = (&next, &counts, tx.clone());
            sc.spawn(move || loop {
                let id = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if id > total { return; }
                let had = nm_core::config::state().join("pokedex-cache").join(format!("{}.json", id)).is_file();
                let ok = dex_info(id).is_some();
                let mut n = c.lock().unwrap();
                n.0 += 1;
                if ok { if !had { n.1 += 1; } } else { n.2 += 1; }
                let _ = tx.send(*n);
            });
        }
        drop(tx);
        for (done, fresh, failed) in rx { report(done, total as usize, fresh, failed); }
    });
    let n = counts.into_inner().unwrap();
    (n.1, total as usize, n.2)
}

/// Each Pokemon as each generation had it (mon-bridge stats). Key: generation * 10000 + dex number.
#[derive(Clone, Debug)]
pub struct DexStat { pub types: Vec<String>, pub abilities: Vec<String>, pub stats: Vec<i32> }
thread_local! { static STATS: std::cell::RefCell<HashMap<i32, DexStat>> = std::cell::RefCell::new(HashMap::new()); }
fn read_dex_stats() {
    if STATS.with(|s| !s.borrow().is_empty()) || !nm_core::config::mon_bridge().is_file() { return; }
    let (o, code) = nm_core::save_guard::bridge("stats");
    if code != 0 { return; }
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        for line in o.split('\n') {
            let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
            if f.len() < 5 { continue; }
            let (Ok(gen), Ok(id)) = (f[0].parse::<i32>(), f[1].parse::<i32>()) else { continue };
            s.insert(gen * 10000 + id, DexStat { types: f[2].split(',').map(String::from).collect(), stats: f[3].split(',').filter_map(|v| v.parse().ok()).collect(), abilities: if f[4].is_empty() { Vec::new() } else { f[4].split(',').map(String::from).collect() } });
        }
    });
}
/// As that generation had it, else the nearest later one that has it, else an earlier one.
fn dex_stat_of(id: i32, gen: &mut i32) -> Option<DexStat> {
    STATS.with(|s| {
        let s = s.borrow();
        for g in *gen..=9 { if let Some(x) = s.get(&(g * 10000 + id)) { *gen = g; return Some(x.clone()); } }
        for g in (1..*gen).rev() { if let Some(x) = s.get(&(g * 10000 + id)) { *gen = g; return Some(x.clone()); } }
        None
    })
}
const GEN_LAST: [i32; 9] = [151, 251, 386, 493, 649, 721, 809, 905, 1025];
fn gen_of_dex(dex: i32) -> i32 { GEN_LAST.iter().position(|&l| dex > 0 && dex <= l).map_or(9, |i| i as i32 + 1) }
fn titled(s: &str) -> String { s.split('-').map(|w| { let mut c = w.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }).collect::<Vec<_>>().join(" ") }
fn type_color(t: &str) -> Option<Color> {
    Some(match t {
        "normal" => Color::rgb(156, 156, 118), "fire" => Color::rgb(232, 118, 52), "water" => Color::rgb(84, 132, 232), "electric" => Color::rgb(232, 186, 36),
        "grass" => Color::rgb(104, 178, 72), "ice" => Color::rgb(104, 192, 192), "fighting" => Color::rgb(186, 48, 42), "poison" => Color::rgb(158, 66, 158),
        "ground" => Color::rgb(206, 170, 86), "flying" => Color::rgb(150, 124, 232), "psychic" => Color::rgb(236, 76, 126), "bug" => Color::rgb(160, 172, 42),
        "rock" => Color::rgb(176, 150, 60), "ghost" => Color::rgb(108, 84, 148), "dragon" => Color::rgb(108, 56, 240), "dark" => Color::rgb(108, 84, 70),
        "steel" => Color::rgb(160, 160, 190), "fairy" => Color::rgb(226, 130, 170), _ => return None,
    })
}

/// What the entry page reads of the collection, taken when the page is made.
#[derive(Clone, Debug, Default)]
pub struct DexEntry {
    pub caught: bool,
    /// DexFlavor's order: games it was caught in, then every game newest first (flavor key, title, dex size).
    pub flavor_from: Vec<(String, String, i32)>,
    /// Where you have it: title, theme, caught, shiny.
    pub rows: Vec<(String, Theme, bool, bool)>,
    /// GameBadges: the games (in the collection) it's caught in, in library order: title, theme, shiny.
    pub badges: Vec<(String, Theme, bool)>,
}

/// What the wide panel and the entry page read of the collection for one Pokemon.
pub fn dex_entry_data(c: &Ctx, d: &Dex, id: i32, only: Option<&str>) -> DexEntry {
    let p = dex_entry_page(c, d, id, only);
    let mut e = (*p.dex_entry.unwrap()).clone();
    e.badges = c.lib.games.iter().filter(|g| counts_in_collection(g)).filter_map(|g| d.get(&g.id).filter(|x| x.caught.contains(&id)).map(|x| (g.title.clone(), theme_of(g), x.shiny.contains(&id)))).collect();
    e
}

/// DexEntryPage(id, order, only).
pub fn dex_entry_page(c: &Ctx, d: &Dex, id: i32, only: Option<&str>) -> Page {
    let mut p = Page::new(&format!("#{:03} {}", id, d.name(id)));
    p.dex_id = id;
    p.dex_shiny = d.shiny(c, id, only);
    let key = |g: &GameEntry| { let gid = g.base_id.clone().unwrap_or_else(|| g.id.clone()); if gid == "black2" { "black-2".to_string() } else if gid == "white2" { "white-2".to_string() } else { gid } };
    let mut from: Vec<(String, String, i32)> = d.by.iter().filter(|(_, x)| x.caught.contains(&id)).filter_map(|(gid, _)| c.lib.games.iter().find(|g| &g.id == gid)).map(|g| (key(g), g.title.clone(), g.dex)).collect();
    from.extend(c.lib.games.iter().rev().map(|g| (key(g), g.title.clone(), g.dex)));
    let mut rows: Vec<(String, Theme, bool, bool)> = d.by.iter().filter(|(gid, x)| match only { Some(o) => gid == o, None => c.lib.games.iter().find(|g| &g.id == gid).map_or(false, counts_in_collection) } && (x.caught.contains(&id) || x.seen.contains(&id)))
        .filter_map(|(gid, x)| c.lib.games.iter().find(|g| &g.id == gid).map(|g| (g.title.clone(), theme_of(g), x.caught.contains(&id), x.shiny.contains(&id)))).collect();
    rows.sort_by_key(|r| if r.2 { 0 } else { 1 });
    p.dex_entry = Some(std::rc::Rc::new(DexEntry { caught: d.caught(c, id, only), flavor_from: from, rows, badges: Vec::new() }));
    p
}

impl Launcher {
    /// RenderDexEntry.
    pub(crate) fn render_dex_entry(&mut self, g: &mut Graphics, p: &mut Page, w: i32, h: i32, m: f32, y: f32, bottom: f32, t: f32, th: &Theme) {
        let (wf, hf) = (w as f32, h as f32);
        let id = p.dex_id;
        let e = p.dex_entry.clone().unwrap_or_default();
        let caught = e.caught;
        let area = RectangleF::new(0.0, y, wf, bottom - y);
        let view = area.height;
        let width = wf - 2.0 * m;
        let pad = wf * 0.04;
        let rad = wf * 0.04;
        let gap = hf * 0.016;
        p.body_scroll = p.body_scroll.min((p.dex_height - view).max(0.0)).max(0.0);
        let kept = g.save();
        g.set_clip_rect(area);
        let top0 = y - p.body_scroll;
        let mut cy = top0;
        let info = if caught { dex_info(id) } else { None };
        let (mut from, mut flavor, mut gen, mut era) = (None::<String>, None::<String>, 9, None::<DexStat>);
        if let Some(info) = &info {
            read_dex_stats();
            for (k, title, dex) in &e.flavor_from {
                if let Some((_, text)) = info.flavor.iter().find(|(fk, _)| fk == k) { from = Some(title.clone()); gen = gen_of_dex(*dex); flavor = Some(text.clone()); break; }
            }
            if flavor.is_none() { if let Some((k, v)) = info.flavor.last() { from = Some(titled(k)); flavor = Some(v.clone()); } }
            era = dex_stat_of(id, &mut gen);
        }
        let types: Vec<String> = match (&era, &info) { (Some(x), _) => x.types.clone(), (None, Some(i)) => i.types.clone(), _ => Vec::new() };
        let (item, small, small_bold, body, title) = (self.fonts.item.clone(), self.fonts.small.clone(), self.fonts.small_bold.clone(), self.fonts.body.clone(), self.fonts.title.clone());
        let (sh, sbh, ih, th_) = (small.height() as f32, small_bold.height() as f32, item.height() as f32, title.height() as f32);
        let pic = width * 0.42;
        let head_h = pic + pad * 0.6;
        g.fill_path(&solid(PHONE_ROW), &round(RectangleF::new(m, cy, width, head_h), rad));
        let key = format!("{}{}", if p.dex_shiny { "shiny/" } else { "" }, id);
        if let Some(sp) = crate::sprites::get(&key) { draw_sprite(g, &sp, RectangleF::new(m + pad * 0.3, cy + pad * 0.3, pic, pic)); }
        else if crate::sprites::loading(Some(&key)) { spinning_ball(g, m + pad * 0.3 + pic / 2.0, cy + pad * 0.3 + pic / 2.0, pic * 0.32, t, false); self.spinning = true; }
        if p.dex_shiny { self.w(g, "\u{2605}", &title, Color::rgb(232, 170, 20), RectangleF::new(m + pad * 0.6, cy + pad * 0.4, pic, th_ * 1.2), Near, Near, false); }
        let fx = m + pic + pad * 0.8;
        let fw = width - pic - pad * 1.2;
        let mut fy = cy + pad;
        match &info {
            _ if !caught => self.w(g, "Seen, not caught yet.", &item, PHONE_SUB, RectangleF::new(fx, fy, fw, head_h - 2.0 * pad), Near, Near, true),
            None => self.w(g, "Getting this entry. It needs the internet the first time.", &small, PHONE_SUB, RectangleF::new(fx, fy, fw, head_h - 2.0 * pad), Near, Near, true),
            Some(info) => {
                self.w(g, &info.genus, &item, PHONE_TEXT, RectangleF::new(fx, fy, fw, ih * 2.4), Near, Near, true);
                fy += g.measure_string_w(&info.genus, &item, fw as i32).height + pad * 0.4;
                let mut tx = fx;
                let chip_h = sbh * 1.5;
                for ty in &types {
                    let label = titled(ty);
                    let cw = g.measure_string(&label, &small_bold).width + chip_h * 0.5;
                    if tx + cw > fx + fw { tx = fx; fy += chip_h * 1.15; }
                    let tc = type_color(ty).unwrap_or(PHONE_SUB);
                    card(g, RectangleF::new(tx, fy, cw, chip_h), chip_h / 2.0, tc, tc, Color::EMPTY, 0.0);
                    self.w(g, &label, &small_bold, Color::WHITE, RectangleF::new(tx, fy, cw, chip_h), Center, Center, false);
                    tx += cw + chip_h * 0.25;
                }
                fy += chip_h + pad * 0.6;
                self.w(g, "Height", &small, PHONE_SUB, RectangleF::new(fx, fy, fw, sh * 1.3), Near, Near, false);
                self.w(g, &format!("{:.1} m", info.height as f64 / 10.0), &small_bold, PHONE_TEXT, RectangleF::new(fx, fy, fw, sh * 1.3), Far, Near, false);
                fy += sh * 1.4;
                self.w(g, "Weight", &small, PHONE_SUB, RectangleF::new(fx, fy, fw, sh * 1.3), Near, Near, false);
                self.w(g, &format!("{:.1} kg", info.weight as f64 / 10.0), &small_bold, PHONE_TEXT, RectangleF::new(fx, fy, fw, sh * 1.3), Far, Near, false);
            }
        }
        cy += head_h + gap;
        if let (true, Some(info)) = (caught, &info) {
            if let Some(fl) = &flavor {
                let text_w = width - 2.0 * pad;
                let text_h = g.measure_string_w(fl, &body, text_w as i32).height + 2.0;
                let box_h = text_h + sh * 1.5 + pad * 1.4;
                g.fill_path(&solid(PHONE_ROW), &round(RectangleF::new(m, cy, width, box_h), rad));
                self.w(g, fl, &body, PHONE_TEXT, RectangleF::new(m + pad, cy + pad * 0.7, text_w, text_h), Near, Near, true);
                self.w(g, &from.clone().unwrap_or_default().replace("Pokemon ", ""), &small, PHONE_SUB, RectangleF::new(m + pad, cy + pad * 0.7 + text_h + 2.0, text_w, sh * 1.3), Near, Near, false);
                cy += box_h + gap;
            }
            // Base stats, as that game's generation had them: Generation 1 had one Special.
            let stats = era.as_ref().map(|x| x.stats.clone()).unwrap_or_else(|| info.stats.clone());
            let special = era.is_some() && gen == 1;
            let names: &[&str] = if special { &["HP", "Attack", "Defense", "Special", "Speed"] } else { &["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"] };
            let pick: &[usize] = if special { &[0, 1, 2, 3, 5] } else { &[0, 1, 2, 3, 4, 5] };
            let era0 = if era.is_none() { "Base stats".to_string() } else { format!("Base stats{} (Generation {})", from.as_ref().map(|f| format!(", as in {}", f.replace("Pokemon ", ""))).unwrap_or_default(), gen) };
            let row_h = sh * 1.75;
            let stats_h = row_h * (names.len() + 2) as f32 + pad * 0.8;
            let (name_w, num_w) = (width * 0.22, width * 0.1);
            g.fill_path(&solid(PHONE_ROW), &round(RectangleF::new(m, cy, width, stats_h), rad));
            let mut sy = cy + pad * 0.4;
            let bar_x = m + pad + name_w + num_w;
            let bar_w = width - 2.0 * pad - name_w - num_w;
            let bar_h = (hf * 0.008).max(4.0);
            self.w(g, &era0, &small, PHONE_SUB, RectangleF::new(m + pad, sy, width - 2.0 * pad, row_h), Near, Center, false);
            sy += row_h;
            let stroke = crate::launcher::accent(th);
            let mut total = 0;
            for i in 0..names.len() {
                if pick[i] >= stats.len() { break; }
                let v = stats[pick[i]];
                total += v;
                self.w(g, names[i], &small, PHONE_SUB, RectangleF::new(m + pad, sy, name_w, row_h), Near, Center, false);
                self.w(g, &v.to_string(), &small_bold, PHONE_TEXT, RectangleF::new(m + pad + name_w, sy, num_w * 0.8, row_h), Far, Center, false);
                let bar = RectangleF::new(bar_x, sy + (row_h - bar_h) / 2.0, bar_w, bar_h);
                card(g, bar, bar_h / 2.0, PHONE_LINE, PHONE_LINE, Color::EMPTY, 0.0);
                card(g, RectangleF::new(bar.x, bar.y, bar_h.max(bar_w * (v as f32 / 255.0).min(1.0)), bar_h), bar_h / 2.0, stroke, stroke, Color::EMPTY, 0.0);
                sy += row_h;
            }
            self.w(g, "Total", &small_bold, PHONE_TEXT, RectangleF::new(m + pad, sy, name_w, row_h), Near, Center, false);
            self.w(g, &total.to_string(), &small_bold, PHONE_TEXT, RectangleF::new(m + pad + name_w, sy, num_w * 0.8, row_h), Far, Center, false);
            cy += stats_h + gap;
            // Abilities came with Generation 3.
            if era.is_none() || gen >= 3 {
                let abilities = match &era { Some(x) => x.abilities.join(", "), None => info.abilities.iter().map(|a| titled(a)).collect::<Vec<_>>().join(", ") };
                let ab_h = g.measure_string_w(&abilities, &item, (width - 2.0 * pad) as i32).height + sh * 1.3 + pad;
                g.fill_path(&solid(PHONE_ROW), &round(RectangleF::new(m, cy, width, ab_h), rad));
                self.w(g, "Abilities", &small, PHONE_SUB, RectangleF::new(m + pad, cy + pad * 0.5, width - 2.0 * pad, sh * 1.3), Near, Near, false);
                self.w(g, &abilities, &item, PHONE_TEXT, RectangleF::new(m + pad, cy + pad * 0.5 + sh * 1.3, width - 2.0 * pad, ab_h), Near, Near, true);
                cy += ab_h + gap;
            }
        }
        // Where you have it.
        let game_h = hf * 0.068;
        if !e.rows.is_empty() {
            g.fill_path(&solid(PHONE_ROW), &round(RectangleF::new(m, cy, width, game_h * e.rows.len() as f32), rad));
            for (title_s, gth, rcaught, shiny) in &e.rows {
                let icon = game_h * 0.62;
                cart(g, m + pad, cy + (game_h - icon) / 2.0, icon, gth);
                self.w(g, &title_s.replace("Pokemon ", ""), &item, PHONE_TEXT, RectangleF::new(m + pad * 1.4 + icon, cy, width * 0.5, game_h), Near, Center, false);
                let mark = if *shiny { "\u{2605} Shiny" } else if *rcaught { "Caught" } else { "Seen" };
                let f = if *shiny { small_bold.clone() } else { small.clone() };
                self.w(g, mark, &f, if *shiny { Color::rgb(196, 138, 8) } else { PHONE_SUB }, RectangleF::new(m, cy, width - pad, game_h), Far, Center, false);
                cy += game_h;
            }
            cy += gap;
        }
        p.dex_height = cy - top0;
        g.restore(kept);
        if p.dex_height > view {
            let bar_w = (wf * 0.008).max(3.0);
            let bar_h = view * view / p.dex_height;
            let bar_y = y + (view - bar_h) * p.body_scroll / (p.dex_height - view);
            g.fill_path(&solid(Color::from_argb(120, 120, 120, 128)), &round(RectangleF::new(wf - m * 0.5, bar_y, bar_w, bar_h), bar_w / 2.0));
        }
    }
}

impl Launcher {
    /// DexPanelName: the wide panel's name for a Pokemon, a Poke Ball before it once caught, a star after a shiny,
    /// then its kind, the games it's caught in and its entry.
    pub(crate) fn dex_panel_name(&mut self, g: &mut Graphics, it: &Item, title: &str, mut x: f32, mut y: f32, tw: f32, bottom: f32) {
        let x0 = x;
        let tf = self.fonts.title.clone();
        let tfh = tf.height() as f32;
        let line_h = tfh * 1.3;
        let cy = y + line_h / 2.0;
        let ball_r = tfh * 0.36;
        if it.dex_state == 2 { ball(g, PointF::new(x + ball_r, cy), ball_r, &Pen::new(INK, (ball_r * 0.14).max(1.2))); }
        else { g.draw_ellipse(&Pen::new(Color::from_argb(if it.dex_state == 1 { 130 } else { 70 }, 150, 150, 160), (ball_r * 0.12).max(1.2)), RectangleF::new(x, cy - ball_r, ball_r * 2.0, ball_r * 2.0)); }
        x += ball_r * 2.0 + ball_r * 0.9;
        let name_w = g.measure_string(title, &tf).width;
        self.w(g, title, &tf, if it.dex_state == 0 { TEXT3 } else { PHONE_TEXT }, RectangleF::new(x, y, tw - (x - (x - ball_r * 2.9)), line_h), Near, Center, false);
        if it.dex_star {
            let sx = x + name_w.min(tw) + tfh * 0.2;
            let sr = tfh * 0.32;
            let pts: Vec<PointF> = (0..10).map(|i| { let a = -std::f64::consts::PI / 2.0 + i as f64 * std::f64::consts::PI / 5.0; let rr = if i % 2 == 0 { sr } else { sr * 0.45 }; PointF::new(sx + sr + a.cos() as f32 * rr, cy + a.sin() as f32 * rr) }).collect();
            let mode = g.smoothing_mode;
            g.smoothing_mode = SmoothingMode::AntiAlias;
            g.fill_polygon(&solid(Color::rgb(232, 60, 50)), &pts);
            g.draw_polygon(&Pen::new(Color::rgb(150, 30, 26), (sr * 0.12).max(1.0)), &pts);
            g.smoothing_mode = mode;
        }
        // Its Pokedex entry once caught, as on the entry page: kind, then the entry and the game it's from.
        let Some(info) = (if it.dex_state == 2 { dex_info(it.dex_no) } else { None }) else { return };
        let e = it.dex_entry.clone().unwrap_or_default();
        let mut flavor = None;
        for (k, _, _) in &e.flavor_from { if let Some((_, t)) = info.flavor.iter().find(|(fk, _)| fk == k) { flavor = Some(t.clone()); break; } }
        if flavor.is_none() { flavor = info.flavor.last().map(|(_, v)| v.clone()); }
        y += line_h + tfh * 0.1;
        if !info.genus.is_empty() {
            let ib = self.fonts.item_bold.clone();
            self.w(g, &info.genus, &ib, PHONE_SUB, RectangleF::new(x0, y, tw, ib.height() as f32 * 1.3), Near, Near, false);
            y += ib.height() as f32 * 1.5;
        }
        y = self.game_badges(g, &e.badges, x0, y, tw, bottom);
        let body = self.fonts.body.clone();
        if let Some(fl) = flavor { if bottom - y > body.height() as f32 {
            let fh = (bottom - y).min(g.measure_string_w(&fl, &body, tw as i32).height + 2.0);
            self.w(g, &fl, &body, PHONE_TEXT, RectangleF::new(x0, y, tw, fh), Near, Near, true);
        } }
    }

    /// GameBadges: a badge for each game it's caught in: the game's colors, a little cartridge, its name, a star where shiny.
    fn game_badges(&mut self, g: &mut Graphics, games: &[(String, Theme, bool)], x0: f32, mut y: f32, tw: f32, bottom: f32) -> f32 {
        if games.is_empty() { return y; }
        let sb = self.fonts.small_bold.clone();
        let bh = sb.height() as f32 * 1.75;
        let gap = bh * 0.25;
        let mut x = x0;
        let mut rows = 1;
        let mode = g.smoothing_mode;
        g.smoothing_mode = SmoothingMode::AntiAlias;
        for (title, th, shiny) in games {
            let name = title.replace("Pokemon ", "");
            let cart_l = bh * 0.62;
            let text_w = g.measure_string(&name, &sb).width + 2.0;
            let bw = bh * 0.3 + cart_l + bh * 0.25 + text_w + if *shiny { bh * 0.7 } else { 0.0 } + bh * 0.35;
            if x + bw > x0 + tw && x > x0 { x = x0; y += bh + gap; rows += 1; }
            if rows > 2 || y + bh > bottom { break; }
            let badge = RectangleF::new(x, y, bw, bh);
            let path = round(badge, bh / 2.0);
            g.fill_path(&Brush::Linear(LinearGradient::new(RectangleF::new(badge.x, badge.y - 1.0, badge.width, badge.height + 2.0), mix(th.main, Color::WHITE, 0.12), th.deep, 90.0)), &path);
            g.draw_path(&Pen::new(mix(th.deep, Color::BLACK, 0.3), (bh * 0.05).max(1.0)), &path);
            g.fill_ellipse(&solid(Color::from_argb(60, 255, 255, 255)), RectangleF::new(badge.x + bh * 0.15, badge.y + bh * 0.1, bw * 0.5, bh * 0.3));
            cart(g, badge.x + bh * 0.3, badge.y + (bh - cart_l) / 2.0, cart_l, th);
            let tx = badge.x + bh * 0.3 + cart_l + bh * 0.25;
            self.w(g, &name, &sb, th.ink, RectangleF::new(tx, badge.y, text_w, bh), Near, Center, false);
            if *shiny {
                let sr = bh * 0.24;
                let sx = tx + text_w + bh * 0.1 + sr;
                let sy = badge.y + bh / 2.0;
                let pts: Vec<PointF> = (0..10).map(|i| { let a = -std::f64::consts::PI / 2.0 + i as f64 * std::f64::consts::PI / 5.0; let rr = if i % 2 == 0 { sr } else { sr * 0.45 }; PointF::new(sx + a.cos() as f32 * rr, sy + a.sin() as f32 * rr) }).collect();
                g.fill_polygon(&solid(Color::rgb(255, 214, 64)), &pts);
                g.draw_polygon(&Pen::new(Color::rgb(150, 100, 10), (sr * 0.12).max(1.0)), &pts);
            }
            x += bw + gap;
        }
        g.smoothing_mode = mode;
        y + bh + gap * 2.0
    }
}
