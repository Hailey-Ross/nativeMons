//! Move Pokemon's box screen: BoxSide, BoxView, LoadSide (mon-bridge layout and list, read only), RenderBoxes and
//! ButtonHints, BoxAct and the mouse (dragging), ported from the C#. The moves themselves are in moves.rs.

use crate::art::*;
use crate::launcher::{accent, Launcher};
use crate::model::Theme;
use crate::pages::theme_of;
use nm_core::game_entry::GameEntry;
use nm_draw::text::StringAlignment::{Center, Far, Near};
use nm_draw::*;

#[derive(Clone, Debug, Default)]
pub struct BoxMon { pub species: String, pub nick: String, pub trainer: String, pub level: i32, pub dex: i32, pub shiny: bool }

#[derive(Clone, Debug)]
pub struct BoxSide {
    pub game_id: String,
    pub title: String,
    pub theme: Theme,
    pub file: Option<String>,
    pub problem: Option<String>,
    pub no_save: bool,
    pub boxes: i32,
    pub slots: i32,
    pub names: Vec<String>,
    pub mons: std::collections::HashMap<i32, BoxMon>,
}
impl BoxSide {
    pub fn at(&self, b: i32, slot: i32) -> Option<&BoxMon> { self.mons.get(&(b * self.slots + slot)) }
}

#[derive(Clone, Debug)]
pub struct BoxView {
    pub side: [Option<BoxSide>; 2],
    pub boxn: [i32; 2],
    pub pane: i32,
    pub cursor: i32,
    pub col: i32,
    pub held_pane: i32,
    pub held_box: i32,
    pub held_slot: i32,
    pub dragging: bool,
    pub wide: bool,
    pub note: Option<String>,
    pub dragged: bool,
    pub drag: PointF,
    pub drag_start: PointF,
    pub hover_arrow: Option<String>,
    pub hover_since: f32,
    /// boxHits: s<pane>:<slot>, h<pane> (the game), l/r<pane> (boxes), from the last paint, screen-relative.
    pub hits: Vec<(RectangleF, String)>,
    pub swallow_click: bool,
}
impl Default for BoxView {
    fn default() -> Self {
        BoxView { side: [None, None], boxn: [0, 0], pane: 0, cursor: 0, col: 0, held_pane: -1, held_box: 0, held_slot: 0, dragging: false, wide: false, note: None,
            dragged: false, drag: PointF::new(0.0, 0.0), drag_start: PointF::new(0.0, 0.0), hover_arrow: None, hover_since: 0.0, hits: Vec::new(), swallow_click: false }
    }
}
impl BoxView { pub fn holding(&self) -> bool { self.held_pane >= 0 } }

pub fn has_box_save(g: &GameEntry) -> bool {
    if g.engine() != "gen1" { return std::fs::metadata(nm_core::saves::master_of(g)).map_or(false, |m| m.is_file() && m.len() > 0); }
    let dir = nm_core::engines::gen1_root().join("saves").join(g.gen1.as_deref().unwrap_or(""));
    std::fs::read_dir(dir).map_or(false, |rd| rd.flatten().any(|e| { let n = e.file_name().to_string_lossy().to_lowercase(); n.ends_with(".lua") && !n.ends_with(".bak.lua") }))
}

/// LoadSide: a game's boxes from its save, read with mon-bridge.
pub fn load_side(g: &GameEntry) -> BoxSide {
    let mut s = BoxSide { game_id: g.id.clone(), title: g.title.clone(), theme: theme_of(g), file: None, problem: None, no_save: false, boxes: 1, slots: 30, names: Vec::new(), mons: Default::default() };
    let (file, why) = nm_core::saves::save_of(g);
    let Some(file) = file else {
        let w = why.clone().unwrap_or_default();
        s.no_save = !has_box_save(g) || why.is_none() || w.starts_with("No save yet") || w.to_lowercase().contains("no save");
        s.problem = Some(if s.no_save { format!("No save yet\n\nPlay {} and save once in the game. Its boxes show here after that.", g.title) } else { w });
        return s;
    };
    s.file = Some(file.clone());
    let (layout, code) = nm_core::save_guard::bridge(&format!("layout \"{}\"", file));
    if code != 0 { s.problem = Some(layout.trim().to_string()); return s; }
    for row in layout.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f[0] == "boxes" && f.len() >= 3 { s.boxes = f[1].parse::<i32>().unwrap_or(1).max(1); s.slots = f[2].parse::<i32>().unwrap_or(30).max(1); }
        else if f[0] == "box" && f.len() >= 3 { s.names.push(f[2].to_string()); }
    }
    let (list, code) = nm_core::save_guard::bridge(&format!("list \"{}\"", file));
    if code != 0 { s.problem = Some(list.trim().to_string()); return s; }
    for row in list.split('\n') {
        let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
        if f.len() < 8 { continue; }
        let (Ok(b), Ok(slot)) = (f[0].parse::<i32>(), f[1].parse::<i32>()) else { continue };
        s.mons.insert(b * s.slots + slot, BoxMon { species: f[2].into(), nick: f[3].into(), level: f[4].parse().unwrap_or(0), trainer: f[5].into(), dex: f[6].parse().unwrap_or(0), shiny: f[7] == "shiny" });
    }
    s
}

/// BoxOther: the cursor to the other game, as HOME's button does.
pub fn box_other(v: &mut BoxView) {
    let mut at = v.cursor;
    v.pane = 1 - v.pane;
    let Some(s) = v.side[v.pane as usize].as_ref().filter(|s| s.problem.is_none()) else { v.cursor = -1; return };
    if at < 0 { return; }
    at = at.min(s.slots - 1);
    let b = v.boxn[v.pane as usize];
    if v.holding() && s.at(b, at).is_some() {
        for d in 1..s.slots {
            if at + d < s.slots && s.at(b, at + d).is_none() { at += d; break; }
            if at - d >= 0 && s.at(b, at - d).is_none() { at -= d; break; }
        }
    }
    v.cursor = at;
    v.col = at % 6;
}

/// ChooseBoxGame: another game in one of the two places.
pub fn choose_box_game(c: &crate::pages::Ctx, v: &BoxView, pane: usize) -> crate::model::Page {
    let mut p = crate::model::Page::new(if pane == 0 { "Top: which game?" } else { "Bottom: which game?" });
    let showing = v.side[pane].as_ref().map(|s| s.game_id.clone()).unwrap_or_default();
    for g in c.lib.games.iter().filter(|g| g.save_tools() || (g.ctr.is_some() && nm_core::ctr3ds_saves::main(g).is_file())) {
        let has = has_box_save(g);
        let hint = if g.id == showing { "Showing now." } else if has { "" } else { "No save yet. Play and save once first." };
        let id = g.id.clone();
        let mut it = crate::model::Item::new(&g.title, hint, has || g.id == showing).icon('\u{E8AB}').run(move |l| {
            l.pages.pop();
            let games = &l.ctx().lib.games;
            let Some(chosen) = games.iter().find(|x| x.id == id).cloned() else { return };
            let Some(v) = l.pages.last().and_then(|p| p.boxes.as_ref()) else { return };
            let other_id = v.side[1 - pane].as_ref().map(|s| s.game_id.clone()).unwrap_or_default();
            let other = games.iter().find(|x| x.id == other_id).cloned().unwrap_or_else(|| chosen.clone());
            let v = l.pages.last_mut().unwrap().boxes.as_mut().unwrap();
            v.boxn[pane] = 0; v.held_pane = -1; v.cursor = -1; v.pane = pane as i32;
            if pane == 0 { l.load_boxes_job(None, chosen, other); } else { l.load_boxes_job(None, other, chosen); }
        });
        it.theme = Some(theme_of(g));
        p.items.push(it);
    }
    p
}

fn inflate(r: RectangleF, dx: f32, dy: f32) -> RectangleF { RectangleF::new(r.x - dx, r.y - dy, r.width + 2.0 * dx, r.height + 2.0 * dy) }

impl Launcher {
    /// RenderBoxes.
    pub(crate) fn render_boxes(&mut self, g: &mut Graphics, v: &mut BoxView, w: i32, h: i32, m: f32, y: f32, bottom: f32, t: f32, th: &Theme) {
        let (wf, hf) = (w as f32, h as f32);
        let acc = accent(th);
        const COLS: i32 = 6;
        let (small, small_bold, item, item_bold) = (self.fonts.small.clone(), self.fonts.small_bold.clone(), self.fonts.item.clone(), self.fonts.item_bold.clone());
        let foot_h = small.height() as f32 * 3.1;
        let gap = hf * 0.008;
        let hh = item.height() as f32 * 2.05;
        v.wide = wf > hf * 1.1;
        let pane_w = if v.wide { (wf - 2.0 * m - gap * 3.0) / 2.0 } else { wf - 2.0 * m };
        let pane_h = if v.wide { bottom - y - foot_h - gap } else { (bottom - y - foot_h - gap) / 2.0 };
        let mut cursor_rect = RectangleF::EMPTY;
        let mut held_cell = 0.0f32;
        v.hits.clear();
        for pane in 0..2usize {
            let Some(s) = v.side[pane].clone() else { continue };
            let px = if v.wide { m + pane as f32 * (pane_w + gap * 3.0) } else { m };
            let py = if v.wide { y } else { y + pane as f32 * (pane_h + gap) };
            let rows = (s.slots + COLS - 1) / COLS;
            let cell = (pane_w / COLS as f32).min((pane_h - hh - gap * 0.5) / rows as f32);
            let gw = cell * COLS as f32;
            let gx = px + (pane_w - gw) / 2.0;
            let head = RectangleF::new(px, py, pane_w, hh);
            let gt = s.theme.clone();
            card(g, head, hh * 0.3, gt.main, gt.deep, Color::EMPTY, 0.0);
            if v.pane == pane as i32 && v.cursor < 0 { g.draw_path(&Pen::new(acc, (wf * 0.007).max(2.5)), &round(head, hh * 0.3)); }
            let aw = hh * 0.9;
            let left = RectangleF::new(head.x, head.y, aw, hh);
            let right = RectangleF::new(head.right() - aw, head.y, aw, hh);
            let mid = RectangleF::new(head.x + aw, head.y, head.width - 2.0 * aw, hh);
            self.w(g, "\u{25C0}", &item, gt.ink, left, Center, Center, false);
            self.w(g, "\u{25B6}", &item, gt.ink, right, Center, Center, false);
            let game_name = s.title.replace("Pokemon ", "");
            let name_area = RectangleF::new(mid.x, mid.y + hh * 0.06, mid.width, hh * 0.5);
            let nf = self.fit_font(g, &game_name, SizeF::new(name_area.width, name_area.height), &item_bold, 0.6, false);
            self.w(g, &game_name, &nf, gt.ink, name_area, Center, Center, false);
            let bn = v.boxn[pane];
            let box_name = if s.no_save { "No save yet".to_string() } else if s.problem.is_some() { "Can't read its save".to_string() } else { format!("{}  \u{b7}  {} of {}", s.names.get(bn as usize).cloned().unwrap_or_else(|| format!("Box {}", bn + 1)), bn + 1, s.boxes) };
            let box_area = RectangleF::new(mid.x, mid.y + hh * 0.5, mid.width, hh * 0.42);
            let bf = self.fit_font(g, &box_name, SizeF::new(box_area.width, box_area.height), &small, 0.6, false);
            self.w(g, &box_name, &bf, fade(gt.ink, 210), box_area, Center, Center, false);
            v.hits.push((left, format!("l{}", pane)));
            v.hits.push((right, format!("r{}", pane)));
            v.hits.push((mid, format!("h{}", pane)));
            let gy = head.bottom() + gap * 0.5;
            let well = RectangleF::new(gx, gy, gw, cell * rows as f32);
            g.fill_path(&solid(PHONE_ROW), &round(well, cell * 0.16));
            if let Some(problem) = &s.problem {
                // Nothing saved yet reads as a plain fact, large: its title, then what to do. A real fault stays small print.
                let inner = inflate(well, -cell * 0.3, -cell * 0.3);
                if s.no_save {
                    let lines: Vec<&str> = problem.split("\n\n").collect();
                    let title_h = item_bold.height() as f32 * 1.4;
                    self.w(g, lines[0], &item_bold, PHONE_TEXT, RectangleF::new(inner.x, inner.y + inner.height / 2.0 - title_h, inner.width, title_h), Center, Far, false);
                    if lines.len() > 1 { self.w(g, lines[1], &small, PHONE_SUB, RectangleF::new(inner.x, inner.y + inner.height / 2.0 + item_bold.height() as f32 * 0.2, inner.width, inner.height / 2.0), Center, Near, true); }
                } else {
                    self.w(g, problem, &small, PHONE_SUB, inner, Center, Center, true);
                }
                continue;
            }
            // The slots first, then the Pokemon over them, as their pictures reach past their own slot.
            for i in 0..s.slots {
                let r = RectangleF::new(gx + (i % COLS) as f32 * cell, gy + (i / COLS) as f32 * cell, cell, cell);
                card(g, inflate(r, -cell * 0.06, -cell * 0.06), cell * 0.16, Color::rgb(242, 242, 247), Color::rgb(232, 232, 238), Color::EMPTY, 0.0);
                v.hits.push((r, format!("s{}:{}", pane, i)));
            }
            for i in 0..s.slots {
                let r = RectangleF::new(gx + (i % COLS) as f32 * cell, gy + (i / COLS) as f32 * cell, cell, cell);
                let inner = inflate(r, -cell * 0.06, -cell * 0.06);
                let from_here = v.holding() && v.side[v.held_pane as usize].as_ref().map_or(false, |x| x.game_id == s.game_id) && v.held_box == bn && v.held_slot == i;
                if let Some(mon) = s.at(bn, i).filter(|_| !from_here) {
                    let key = format!("{}{}", if mon.shiny { "shiny/" } else { "" }, mon.dex);
                    // Sprites carry wide empty margins: drawn larger than the slot, the Pokemon itself fills it.
                    match crate::sprites::get(&key) {
                        Some(pic) => draw_sprite(g, &pic, inflate(inner, cell * 0.22, cell * 0.22)),
                        None if crate::sprites::loading(Some(&key)) => { spinning_ball(g, inner.x + inner.width / 2.0, inner.y + inner.height / 2.0, inner.width * 0.55, t, false); self.spinning = true; }
                        None => self.w(g, &format!("#{}", mon.dex), &small, PHONE_SUB, inner, Center, Center, false),
                    }
                    if mon.shiny { self.w(g, "\u{2605}", &small, Color::rgb(232, 170, 20), RectangleF::new(inner.x + cell * 0.04, inner.y, inner.width, small.height() as f32 * 1.2), Near, Near, false); }
                }
                if from_here {
                    let mut pen = Pen::new(fade(acc, 200), (cell * 0.04).max(1.5));
                    pen.dash_style = DashStyle::Dash;
                    g.draw_path(&pen, &round(inner, cell * 0.16));
                }
                if v.pane == pane as i32 && v.cursor == i {
                    cursor_rect = inner;
                    held_cell = cell;
                    g.draw_path(&Pen::new(acc, (cell * 0.06).max(2.5)), &round(inner, cell * 0.16));
                }
                if held_cell == 0.0 { held_cell = cell; }
            }
            // The game the cursor isn't in steps back, as HOME's does.
            if v.pane != pane as i32 && !v.dragging { g.fill_path(&solid(fade(PHONE_BG, 110)), &round(inflate(well, cell * 0.02, cell * 0.02), cell * 0.16)); }
        }
        // Keep drawing while the pointer moves.
        if v.dragging { self.spinning = true; }
        // The Pokemon in hand: under the pointer while dragging, else just above the cursor.
        let held = if v.holding() { v.side[v.held_pane as usize].as_ref().and_then(|s| s.at(v.held_box, v.held_slot).cloned()) } else { None };
        if let Some(hm) = &held {
            let size = held_cell * 1.8;
            let at = if v.dragging { Some(v.drag) } else if !cursor_rect.is_empty() { Some(PointF::new(cursor_rect.x + cursor_rect.width / 2.0, cursor_rect.y - size * 0.15)) } else { None };
            if let Some(at) = at.filter(|a| a.x != 0.0 || a.y != 0.0) {
                let r = RectangleF::new(at.x - size / 2.0, at.y - size / 2.0, size, size);
                g.fill_ellipse(&solid(Color::from_argb(60, 0, 0, 0)), RectangleF::new(r.x + size * 0.15, r.bottom() - size * 0.18, size * 0.7, size * 0.16));
                if let Some(pic) = crate::sprites::get(&format!("{}{}", if hm.shiny { "shiny/" } else { "" }, hm.dex)) { draw_sprite(g, &pic, r); }
            }
        }
        // What's under the cursor, or what to do; a note when a drop didn't happen.
        let foot = RectangleF::new(m, bottom - foot_h, wf - 2.0 * m, foot_h);
        let under = v.side[v.pane as usize].as_ref().filter(|_| v.cursor >= 0).and_then(|s| s.at(v.boxn[v.pane as usize], v.cursor).cloned());
        let show = held.or(under);
        let first = v.note.clone().unwrap_or_else(|| match &show {
            Some(s) => format!("{}   Lv {}{}   OT {}", if !s.nick.is_empty() && !s.nick.eq_ignore_ascii_case(&s.species) { format!("{} ({})", s.nick, s.species) } else { s.species.clone() }, s.level, if s.shiny { "   \u{2605} shiny" } else { "" }, s.trainer),
            None => String::new(),
        });
        let hints: [&str; 8] = if v.holding() { ["A", "Put down", "X", "Other game", "LB RB", "Box", "B", "Put back"] }
            else if v.cursor < 0 { ["A", "Change game", "\u{25C0} \u{25B6}", "Box", "X", "Other game", "B", "Back"] }
            else { ["A", "Pick up", "X", "Other game", "LB RB", "Box", "B", "Back"] };
        self.w(g, &first, &small_bold, if v.note.is_some() { Color::rgb(200, 60, 50) } else { PHONE_TEXT }, RectangleF::new(foot.x, foot.y, foot.width, foot_h * 0.45), Center, Center, false);
        self.button_hints(g, &hints, RectangleF::new(foot.x, foot.y + foot_h * 0.45, foot.width, foot_h * 0.55));
    }

    /// ButtonHints: buttons and what they do, in a row, each button a dark chip.
    fn button_hints(&mut self, g: &mut Graphics, pairs: &[&str], r: RectangleF) {
        let sb = self.fonts.small_bold.clone();
        let chip_h = sb.height() as f32 * 1.45;
        let sp = chip_h * 0.3;
        let mut total = 0.0;
        let mut widths = vec![0.0f32; pairs.len()];
        for i in 0..pairs.len() {
            widths[i] = g.measure_string(pairs[i], &sb).width + if i % 2 == 0 { chip_h * 0.3 } else { 0.0 };
            if i % 2 == 0 { widths[i] = widths[i].max(chip_h); }
            total += widths[i] + if i % 2 == 0 { sp * 0.5 } else { sp * 2.2 };
        }
        let mut x = r.x + (r.width - total) / 2.0;
        let cy = r.y + (r.height - chip_h) / 2.0;
        for i in 0..pairs.len() {
            let cr = RectangleF::new(x, cy, widths[i], chip_h);
            if i % 2 == 0 {
                card(g, cr, chip_h / 2.0, Color::rgb(60, 60, 67), Color::rgb(40, 40, 46), Color::EMPTY, 0.0);
                self.w(g, pairs[i], &sb, Color::WHITE, cr, Center, Center, false);
                x += widths[i] + sp * 0.5;
            } else {
                self.w(g, pairs[i], &sb, PHONE_TEXT, RectangleF::new(x, cy - chip_h, widths[i] + sp, chip_h * 3.0), Near, Center, false);
                x += widths[i] + sp * 2.2;
            }
        }
    }
}

/// LoadBoxes without the Busy card: both sides read again, the boxes kept in range.
pub fn load_boxes(v: &mut BoxView, top: &GameEntry, bottom: &GameEntry) {
    let a = load_side(top);
    let b = if bottom.id == top.id { a.clone() } else { load_side(bottom) };
    v.side = [Some(a), Some(b)];
    for i in 0..2 { v.boxn[i] = v.boxn[i].min(v.side[i].as_ref().unwrap().boxes - 1); }
}

fn contains(r: &RectangleF, p: PointF) -> bool { p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height }
fn slot_of(hit: &str) -> (usize, i32) { ((hit.as_bytes()[1] - b'0') as usize, hit[3..].parse().unwrap_or(0)) }

/// What a drop or A asks the launcher to do: another game, a move inside one save, or a move between two.
#[derive(Clone, Debug, PartialEq)]
pub enum BoxOut {
    Choose,
    Relocate { game: String, file: String, from_box: i32, from_slot: i32, to_box: i32, to_slot: i32, species: String },
    Move { from: String, file: String, from_box: i32, from_slot: i32, species: String, to: String, at: String },
}

/// BoxDrop's checks; the move itself runs in the launcher (`Launcher::box_outcome`).
pub fn box_drop(v: &mut BoxView, to_pane: usize, to_slot: i32) -> Option<BoxOut> {
    let (from_pane, from_box, from_slot, to_box) = (v.held_pane, v.held_box, v.held_slot, v.boxn[to_pane]);
    v.held_pane = -1;
    v.dragging = false;
    v.note = None;
    if from_pane < 0 { return None; }
    let from = v.side[from_pane as usize].as_ref()?;
    let to = v.side[to_pane].as_ref()?;
    let mon = from.at(from_box, from_slot)?;
    // Moves go between the games: back on its own side, it goes back where it was.
    if from_pane as usize == to_pane { return None; }
    let file = from.file.clone().unwrap_or_default();
    if from.game_id == to.game_id {
        if from_box == to_box && from_slot == to_slot { return None; }
        return Some(BoxOut::Relocate { game: from.game_id.clone(), file, from_box, from_slot, to_box, to_slot, species: mon.species.clone() });
    }
    if to.at(to_box, to_slot).is_some() { v.note = Some("That slot is taken. Drop it on an empty one.".into()); return None; }
    Some(BoxOut::Move { from: from.game_id.clone(), file, from_box, from_slot, species: mon.species.clone(), to: to.game_id.clone(), at: format!("at:{}:{}", to_box, to_slot) })
}

/// BoxAct: the pad and keys on the box screen. Returns a later step's flow to run, by its C# name.
pub fn box_act(v: &mut BoxView, a: &str) -> Option<BoxOut> {
    const COLS: i32 = 6;
    let Some(side) = v.side[v.pane as usize].clone() else { return None };
    let count = side.slots;
    let rows = (count + COLS - 1) / COLS;
    let p = v.pane as usize;
    if a == "switch" { box_other(v); v.note = None; return None; }
    if a == "boxprev" || a == "boxnext" {
        if side.problem.is_none() { v.boxn[p] = (v.boxn[p] + if a == "boxnext" { 1 } else { -1 } + side.boxes) % side.boxes; v.note = None; }
        if v.holding() && v.cursor >= 0 && v.pane != v.held_pane { v.pane = 1 - v.pane; box_other(v); }
        return None;
    }
    if a == "left" || a == "right" {
        let d = if a == "right" { 1 } else { -1 };
        if v.cursor < 0 { v.boxn[p] = (v.boxn[p] + d + side.boxes) % side.boxes; v.note = None; }
        else {
            let col = v.cursor % COLS + d;
            if col >= 0 && col < COLS && v.cursor + d < count { v.cursor += d; v.col = col; }
            else if v.wide && ((d > 0 && v.pane == 0) || (d < 0 && v.pane == 1)) {
                // Side by side, the edge of one box leads into the other game's, on the same row.
                let row = v.cursor / COLS;
                v.pane = 1 - v.pane;
                let oslots = v.side[v.pane as usize].as_ref().map_or(30, |o| o.slots);
                let orows = (oslots + COLS - 1) / COLS;
                v.col = if d > 0 { 0 } else { COLS - 1 };
                v.cursor = (oslots - 1).min(row.min(orows - 1) * COLS + v.col);
            }
        }
    } else if v.wide && (a == "up" || a == "down") {
        // Side by side, up and down stay in the one game: the header at the top, its rows below.
        if a == "up" { if v.cursor >= COLS { v.cursor -= COLS; } else { v.cursor = -1; } }
        else if v.cursor < 0 { v.cursor = (count - 1).min(v.col); } else if v.cursor / COLS < rows - 1 { v.cursor = (count - 1).min(v.cursor + COLS); }
    } else if a == "up" {
        if v.cursor < 0 {
            if v.pane == 1 {
                v.pane = 0;
                let s0 = v.side[0].as_ref().map_or(30, |s| s.slots);
                let r0 = (s0 + COLS - 1) / COLS;
                v.cursor = (s0 - 1).min((r0 - 1) * COLS + v.col);
            }
        } else if v.cursor < COLS { v.cursor = -1; } else { v.cursor -= COLS; }
    } else if a == "down" {
        if v.cursor < 0 { v.cursor = (count - 1).min(v.col); }
        else if v.cursor / COLS < rows - 1 { v.cursor = (count - 1).min(v.cursor + COLS); }
        else if v.pane == 0 { v.pane = 1; v.cursor = -1; }
    } else if a == "ok" {
        if v.cursor < 0 { return if !v.holding() { Some(BoxOut::Choose) } else { None }; }
        if !v.holding() {
            if side.at(v.boxn[p], v.cursor).is_some() { v.held_pane = v.pane; v.held_box = v.boxn[p]; v.held_slot = v.cursor; v.note = None; box_other(v); }
        } else { return box_drop(v, p, v.cursor); }
    }
    None
}

impl Launcher {
    fn top_box_view(&mut self) -> Option<&mut BoxView> { if self.job { return None; } self.pages.last_mut().and_then(|p| p.boxes.as_mut()) }
    fn box_hit_at(&self, x: i32, y: i32) -> Option<String> {
        let v = self.pages.last()?.boxes.as_ref()?;
        let inside = PointF::new(x as f32 - self.screen_x, y as f32 - self.screen_y);
        v.hits.iter().find(|h| contains(&h.0, inside)).map(|h| h.1.clone())
    }
    /// What a drop or A asked for: ChooseBoxGame, a move inside one save (relocate) or between two (MoveNow).
    fn box_outcome(&mut self, what: Option<BoxOut>) {
        match what {
            Some(BoxOut::Choose) => {
                let v = self.pages.last().and_then(|p| p.boxes.clone()).unwrap();
                let p = choose_box_game(self.ctx(), &v, v.pane as usize);
                self.push(p);
            }
            Some(r @ BoxOut::Relocate { .. }) => self.box_relocate(r),
            Some(BoxOut::Move { from, file, from_box, from_slot, species, to, at }) =>
                self.move_now(&from, &file, &from_box.to_string(), &from_slot.to_string(), &species, &to, Some(at), true),
            None => {}
        }
    }
    pub fn box_act(&mut self, a: &str) {
        let r = match self.top_box_view() { Some(v) => box_act(v, a), None => None };
        self.box_outcome(r);
    }
    /// Pressing on a Pokemon picks it up, to drag; with one already held, pressing on a slot puts it down there.
    pub fn box_mouse_down(&mut self, x: i32, y: i32, b: crate::input::Button) -> bool {
        if b != crate::input::Button::Left || self.top_box_view().is_none() { return false; }
        let Some(hit) = self.box_hit_at(x, y) else { return false };
        if !hit.starts_with('s') { return true; }
        let (pane, slot) = slot_of(&hit);
        let at = PointF::new(x as f32 - self.screen_x, y as f32 - self.screen_y);
        let v = self.top_box_view().unwrap();
        v.pane = pane as i32; v.cursor = slot; v.col = slot % 6;
        if v.holding() { let r = box_drop(v, pane, slot); v.swallow_click = true; self.dirty = true; self.box_outcome(r); return true; }
        if v.side[pane].as_ref().map_or(true, |s| s.at(v.boxn[pane], slot).is_none()) { return true; }
        v.held_pane = pane as i32; v.held_box = v.boxn[pane]; v.held_slot = slot; v.note = None;
        v.dragging = true; v.dragged = false; v.drag_start = at; v.drag = at;
        v.swallow_click = true;
        self.dirty = true;
        true
    }
    pub fn box_mouse_move(&mut self, x: i32, y: i32) -> bool {
        if self.top_box_view().is_none() { return false; }
        let hit = self.box_hit_at(x, y);
        let now = self.now();
        let at = PointF::new(x as f32 - self.screen_x, y as f32 - self.screen_y);
        let v = self.top_box_view().unwrap();
        let mut dirty = false;
        if v.dragging {
            v.drag = at;
            if (v.drag.x - v.drag_start.x).abs() + (v.drag.y - v.drag_start.y).abs() > 8.0 { v.dragged = true; }
            // Held over a box arrow, the box turns, as HOME does.
            let arrow = hit.clone().filter(|h| h.starts_with('l') || h.starts_with('r'));
            if arrow != v.hover_arrow { v.hover_arrow = arrow; v.hover_since = now; }
            else if let Some(ar) = arrow.filter(|_| now - v.hover_since > 0.7) {
                let pane = (ar.as_bytes()[1] - b'0') as usize;
                let n = v.side[pane].as_ref().map_or(1, |s| s.boxes);
                v.boxn[pane] = (v.boxn[pane] + if ar.starts_with('r') { 1 } else { -1 } + n) % n;
                v.hover_since = now;
            }
            dirty = true;
        }
        if let Some(h) = hit.filter(|h| h.starts_with('s')) {
            let (pane, slot) = slot_of(&h);
            if v.pane != pane as i32 || v.cursor != slot { v.pane = pane as i32; v.cursor = slot; v.col = slot % 6; dirty = true; }
        }
        if dirty { self.dirty = true; }
        true
    }
    pub fn box_mouse_up(&mut self, x: i32, y: i32, _b: crate::input::Button) -> bool {
        let hit = self.box_hit_at(x, y);
        let Some(v) = self.top_box_view().filter(|v| v.dragging) else { return false };
        v.dragging = false;
        v.hover_arrow = None;
        // A press and release on the same place is a pick-up: the next press puts it down.
        let mut r = None;
        if v.dragged {
            match hit.filter(|h| h.starts_with('s')) { Some(h) => { let (p, s) = slot_of(&h); r = box_drop(v, p, s); } None => v.held_pane = -1 }
        }
        self.dirty = true;
        self.box_outcome(r);
        true
    }
    pub fn box_click(&mut self, x: i32, y: i32, b: crate::input::Button) -> bool {
        let hit = self.box_hit_at(x, y);
        let Some(v) = self.top_box_view() else { return false };
        if v.swallow_click { v.swallow_click = false; return true; }
        if b != crate::input::Button::Left { return false; }
        let Some(hit) = hit else { return false };
        let pane = (hit.as_bytes()[1] - b'0') as usize;
        let mut r = None;
        if hit.starts_with('l') || hit.starts_with('r') {
            let n = v.side[pane].as_ref().map_or(1, |s| s.boxes);
            v.boxn[pane] = (v.boxn[pane] + if hit.starts_with('r') { 1 } else { -1 } + n) % n;
            v.pane = pane as i32; v.cursor = -1; v.note = None;
        } else if hit.starts_with('h') && !v.holding() { r = Some(BoxOut::Choose); }
        self.dirty = true;
        if r.is_some() {
            let vv = self.pages.last().and_then(|p| p.boxes.clone()).unwrap();
            let p = choose_box_game(self.ctx(), &vv, pane);
            self.push(p);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn side(id: &str, mons: &[i32]) -> BoxSide {
        let mut s = BoxSide { game_id: id.into(), title: id.into(), theme: Theme::default_theme(), file: Some("x".into()), problem: None, no_save: false, boxes: 3, slots: 30, names: vec![], mons: Default::default() };
        for &k in mons { s.mons.insert(k, BoxMon { species: format!("m{}", k), dex: k + 1, ..Default::default() }); }
        s
    }
    fn view() -> BoxView { let mut v = BoxView::default(); v.side = [Some(side("a", &[0, 1, 7])), Some(side("b", &[0]))]; v }

    #[test]
    fn cursor_moves() {
        let mut v = view();
        box_act(&mut v, "right"); assert_eq!((v.cursor, v.col), (1, 1));
        box_act(&mut v, "down"); assert_eq!(v.cursor, 7);
        box_act(&mut v, "up"); box_act(&mut v, "up"); assert_eq!(v.cursor, -1);
        box_act(&mut v, "right"); assert_eq!(v.boxn[0], 1);
        box_act(&mut v, "left"); box_act(&mut v, "left"); assert_eq!(v.boxn[0], 2);
        for _ in 0..6 { box_act(&mut v, "down"); }
        assert_eq!((v.pane, v.cursor), (1, -1));
        box_act(&mut v, "up"); assert_eq!((v.pane, v.cursor), (0, 25));
    }
    #[test]
    fn switch_and_boxes() {
        let mut v = view();
        v.cursor = 4;
        box_act(&mut v, "switch"); assert_eq!((v.pane, v.cursor), (1, 4));
        box_act(&mut v, "boxnext"); assert_eq!(v.boxn[1], 1);
        box_act(&mut v, "boxprev"); box_act(&mut v, "boxprev"); assert_eq!(v.boxn[1], 2);
    }
    #[test]
    fn pick_up_and_put_down() {
        let mut v = view();
        assert_eq!(box_act(&mut v, "ok"), None);
        assert!(v.holding());
        // Holding, the cursor goes to the other game's nearest empty slot.
        assert_eq!((v.pane, v.cursor), (1, 1));
        assert!(matches!(box_act(&mut v, "ok"), Some(BoxOut::Move { ref at, .. }) if at == "at:0:1"));
        assert!(!v.holding());
        // Dropping on a taken slot leaves a note.
        let mut v = view();
        box_act(&mut v, "ok");
        v.cursor = 0;
        assert_eq!(box_act(&mut v, "ok"), None);
        assert_eq!(v.note.as_deref(), Some("That slot is taken. Drop it on an empty one."));
        // The header asks for another game.
        let mut v = view();
        v.cursor = -1;
        assert_eq!(box_act(&mut v, "ok"), Some(BoxOut::Choose));
    }
    #[test]
    fn wide_edges() {
        let mut v = view();
        v.wide = true;
        v.cursor = 5; v.col = 5;
        box_act(&mut v, "right"); assert_eq!((v.pane, v.cursor, v.col), (1, 0, 0));
        box_act(&mut v, "left"); assert_eq!((v.pane, v.cursor), (0, 5));
    }
}
