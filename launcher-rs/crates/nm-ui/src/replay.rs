//! --replay: a scripted input sequence played offscreen on the fixed test clock, with chosen frames saved.
//! One command per line (# comments): screen <name> | t <s> | wait <s> | until <s> | key <name> | pad <A|B|X|UP|...>
//! | hold <buttons> <s> | move <x> <y> | click <x> <y> [right] | down/up <x> <y> | wheel <delta> | clickitem <i>
//! | snap <name> | expect <title> | effect <text> | page <C# name> [game id] [text]: a page the fixture can't reach by keys,
//! pushed as the C# replay pushes it by reflection. Frames are drawn whenever Poll asks for one, every 16 ms tick.

use crate::input::{self, Button, Key};
use crate::launcher::{Host, Launcher};
use nm_draw::{Bitmap, Graphics};
use std::fmt::Write as _;
use std::path::Path;

const TICK: f32 = 0.016;

struct Run {
    l: Launcher,
    g: Graphics,
    w: i32,
    h: i32,
    t: f32,
    pad: u16,
    log: String,
    seen: Vec<String>,
    frames: u32,
    /// NATIVEMONS_REPLAY_FRAMES=1: every drawn frame saved as frames/f-<ms>-<n>.png, as the C# replay does.
    dump: Option<std::path::PathBuf>,
    last_ms: i32,
    same_ms: u32,
    /// NATIVEMONS_TEST_DAMAGE=1: each frame drawn again as the live launcher draws it (recorded ops, only the changed
    /// parts redrawn over the last such frame) and compared with the full one; damage.txt says how they compared.
    check: Option<Check>,
}

#[derive(Default)]
struct Check { out: Option<std::path::PathBuf>, bmp: Option<Bitmap>, ops: Vec<nm_draw::graphics::Op>, frames: u32, differ: u32, redrawn: f64, first_bad: Option<String> }

impl Run {
    fn draw(&mut self) {
        self.resize();
        if self.dump.is_some() { if let Some(m) = self.l.hooks.mask.as_mut() { m.clear(); } self.g.clear_accepted(); }
        // OnPaint: mid-morph only the drawn device's part of the window. The damage check records the frame, draws it
        // whole from its ops (the frame kept), and again only where it changed over the last one (compared).
        if self.check.is_some() { self.g.begin_record(); }
        self.l.paint(&mut self.g, self.w, self.h);
        let ops = if self.check.is_some() { Some(self.g.flush(&[]).0) } else { None };
        self.frames += 1;
        if let Some(dir) = self.dump.clone() {
            let ms = (self.t * 1000.0).round() as i32;
            self.same_ms = if ms == self.last_ms { self.same_ms + 1 } else { 0 };
            self.last_ms = ms;
            let _ = std::fs::create_dir_all(&dir);
            let _ = crate::snapshot::save(&mut self.g, &mut self.l, &dir.join(format!("f-{:06}-{}.png", ms, self.same_ms)));
        }
        if let (Some(c), Some(ops)) = (self.check.as_mut(), ops) {
            let (w, h) = (self.g.bmp.width, self.g.bmp.height);
            let bmp = c.bmp.take().filter(|b| (b.width, b.height) == (w, h));
            if bmp.is_none() { c.ops.clear(); }
            let mut g = Graphics::new(bmp.unwrap_or_else(|| Bitmap::new(w, h)));
            let (ops, dirty) = g.apply(ops, &c.ops);
            c.ops = ops;
            c.frames += 1;
            c.redrawn += dirty.iter().map(|d| ((d.2 - d.0) as f64) * ((d.3 - d.1) as f64)).sum::<f64>() / (w as f64 * h as f64);
            if g.bmp.data != self.g.bmp.data {
                c.differ += 1;
                if c.first_bad.is_none() {
                    c.first_bad = Some(format!("t {:.3} (frame {})", self.t, self.frames));
                    if let Some(d) = &c.out { let _ = self.g.bmp.save_png(&d.join("damage-full.png")); let _ = g.bmp.save_png(&d.join("damage-live.png")); }
                }
            }
            c.bmp = Some(g.bmp);
        }
    }
    /// The window as MorphTowards last set it (SetBounds changes the client size at once, as WinForms does).
    fn resize(&mut self) {
        let (_, _, cw, ch) = self.l.morph.window;
        if (cw, ch) != (self.w, self.h) && cw > 0 && ch > 0 { self.w = cw; self.h = ch; self.g = Graphics::new(Bitmap::new(cw, ch)); }
    }
    /// One tick: the clock moves, Poll runs, and a frame is drawn when it asks (Invalidate, then OnPaint).
    fn tick(&mut self) {
        self.t += TICK;
        self.l.hooks.t = Some(self.t);
        if self.l.poll(self.pad, true) { self.draw(); }
        self.note_effects();
    }
    fn advance_to(&mut self, until: f32) { while self.t + TICK <= until + 1e-4 { self.tick(); } }
    /// After an input event: the window repaints at once (Invalidate), as WinForms does before the next tick.
    fn after_input(&mut self) {
        if self.l.dirty { self.l.dirty = false; self.draw(); }
        self.note_effects();
    }
    fn note_effects(&mut self) {
        self.l.core_notes();
        for e in self.l.effects.drain(..) {
            let t = match &e { crate::input::Effect::Skip(s) => s.clone(), e => format!("{:?}", e) };
            let _ = writeln!(self.log, "  effect {}", t);
            self.seen.push(t);
        }
    }
    fn top(&self) -> String { self.l.pages.last().map(|p| p.title.clone()).unwrap_or_default() }
}

pub fn replay(script: &Path, out: &Path, w: i32, h: i32, host: Host) -> Result<(), String> {
    let text = std::fs::read_to_string(script).map_err(|e| format!("{}: {}", script.display(), e))?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut run: Option<Run> = None;
    let mut log = String::new();
    // TestHooks.Replay is on from the start, so the library's load already goes through the stubs.
    nm_core::test_hooks::set_replay(true, std::env::var("NATIVEMONS_TEST_DRYRUN").as_deref() == Ok("1"));
    let mut failed = 0;
    let res: Result<(), String> = (|| { for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| -> Result<f32, String> { f.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or_else(|| format!("line {}: {}: a number is missing", n + 1, line)) };
        if f[0] == "screen" {
            let name = f.get(1).copied().unwrap_or("main");
            let t0 = f.get(2).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let mut l = crate::snapshot::setup(name, w, h, host, None)?;
            l.hooks.t = Some(t0);
            l.hooks.replay = true;
            l.jobs.dry_run = std::env::var("NATIVEMONS_TEST_DRYRUN").as_deref() == Ok("1");
            // A fixed screen and window, centered (the C# replay uses the same).
            l.morph.area = (0, 0, 1920, 1040);
            l.morph.window = ((1920 - w) / 2, (1040 - h) / 2, w, h);
            l.effects.clear();
            nm_core::test_hooks::take_notes();
            let dump = if std::env::var("NATIVEMONS_REPLAY_FRAMES").as_deref() == Ok("1") { Some(out.join("frames")) } else { None };
            let check = if std::env::var("NATIVEMONS_TEST_DAMAGE").as_deref() == Ok("1") { Some(Check { out: Some(out.to_path_buf()), ..Default::default() }) } else { None };
            let mut r = Run { l, g: Graphics::new(Bitmap::new(w, h)), w, h, t: t0, pad: 0, log: String::new(), seen: Vec::new(), frames: 0, dump, last_ms: -1, same_ms: 0, check };
            r.draw();
            let _ = writeln!(log, "{}: screen {} at t {:.3}: {}", n + 1, name, t0, r.top());
            run = Some(r);
            continue;
        }
        let r = run.as_mut().ok_or_else(|| format!("line {}: no screen yet", n + 1))?;
        let before = r.t;
        match f[0] {
            "t" => { let t = num(1)?; if t < r.t { return Err(format!("line {}: the clock can't go back", n + 1)); } r.t = t; r.l.hooks.t = Some(t); }
            "wait" => { let s = num(1)?; let to = r.t + s; r.advance_to(to); }
            "until" => { let to = num(1)?; r.advance_to(to); }
            "key" => {
                let k = f.get(1).and_then(|k| Key::parse(k)).ok_or_else(|| format!("line {}: no such key", n + 1))?;
                r.l.key(k);
                r.after_input();
                r.tick();
            }
            "pad" | "hold" => {
                let mut bits = 0u16;
                for b in f.get(1).copied().unwrap_or("").split('+') {
                    bits |= match b.to_ascii_uppercase().as_str() {
                        "A" => input::A, "B" => input::B, "X" => input::X, "UP" => input::UP, "DOWN" => input::DOWN,
                        "LEFT" => input::LEFT, "RIGHT" => input::RIGHT, "LB" => input::LB, "RB" => input::RB,
                        _ => return Err(format!("line {}: no such button {}", n + 1, b)),
                    };
                }
                r.pad = bits;
                if f[0] == "hold" { let to = r.t + num(2)?; r.advance_to(to); } else { r.tick(); }
                r.pad = 0;
                r.tick();
            }
            "move" | "click" | "down" | "up" | "clickitem" => {
                let (x, y) = if f[0] == "clickitem" {
                    let i = num(1)? as i32;
                    let hit = r.l.hits.iter().find(|h| h.1 == i).map(|h| h.0).ok_or_else(|| format!("line {}: no hit rect for item {}", n + 1, i))?;
                    ((hit.x + hit.width / 2.0 + r.l.screen_x) as i32, (hit.y + hit.height / 2.0 + r.l.screen_y) as i32)
                } else { (num(1)? as i32, num(2)? as i32) };
                let b = if f.last() == Some(&"right") { Button::Right } else { Button::Left };
                match f[0] {
                    "move" => r.l.mouse_move(x, y, (x, y)),
                    "down" => r.l.mouse_down(x, y, b),
                    "up" => r.l.mouse_up(x, y, b),
                    _ => {
                        // A click: the pointer moves there, presses and lets go (WinForms: MouseMove, Down, Up, Click).
                        r.l.mouse_move(x, y, (x, y));
                        r.after_input();
                        r.l.mouse_down(x, y, b);
                        r.l.mouse_up(x, y, b);
                        r.l.mouse_click(x, y, b);
                    }
                }
                r.after_input();
                r.tick();
            }
            "page" => {
                let p = page_by_name(&r.l, &f[1..], line).ok_or_else(|| format!("line {}: no such page or game: {}", n + 1, line))?;
                r.l.pages.push(p);
                r.l.dirty = true;
                r.after_input();
                r.tick();
            }
            // Quick start's tick as if the menu had been idle 2 s (a game by id becomes the current one first).
            "quick" => {
                if let Some(id) = f.get(1) { let i = r.l.game_index(id); if let Some(c) = r.l.ctx.as_mut() { c.current = i; } }
                r.l.jobs.quick.menu_since = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(2));
                r.l.quick_start_tick();
                r.after_input();
            }
            // DiscordPresence.Playing for one game or all (NATIVEMONS_TEST_DISCORD notes the frames).
            "discord" => {
                let which = f.get(1).copied().unwrap_or("all");
                let games: Vec<_> = r.l.ctx().lib.games.iter().filter(|g| which == "all" || g.id == which).cloned().collect();
                for g in &games { r.l.discord_playing(Some(g)); }
                r.after_input();
            }
            // The next dialog's answer (the rest of the line; "-" cancels it).
            "answer" => { let a = expand_env(line[6..].trim()); let a = a.as_str(); r.l.jobs.answers.push_back(if a == "-" { None } else { Some(a.to_string()) }); }
            // Files dropped on the window (TakeDrop), paths separated by "|".
            "drop" => { let paths: Vec<String> = expand_env(&line[4..]).split('|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(); r.l.take_drop(paths); r.after_input(); r.tick(); }
            "wheel" => { r.l.mouse_wheel(num(1)? as i32); r.after_input(); r.tick(); }
            // A result page only the Rust launcher shows (the user's call, e.g. UpdatePc's): closed without a frame, so
            // both launchers go on from the same page. The C# takes the line as nothing.
            "deliberate" => {
                let titles: Vec<&str> = line[10..].split('|').map(|x| x.trim()).collect();
                if titles.contains(&r.top().as_str()) { r.l.pages.pop(); }
            }
            // Rust only, for writing scripts: the top page's items into items.txt (the C# has no such command).
            "items" => {
                let list = r.l.pages.last().map(|p| p.items.iter().enumerate().map(|(i, it)| format!("{}: {} [{}]{}", i, it.text, if it.on { "on" } else { "off" }, if it.run.is_some() { "" } else { " no act" })).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                let mut prev = std::fs::read_to_string(out.join("items.txt")).unwrap_or_default();
                prev.push_str(&format!("--- line {}: {}\n{}\n", n + 1, r.top(), list));
                let _ = std::fs::write(out.join("items.txt"), prev);
            }
            "snap" => {
                let name = f.get(1).copied().unwrap_or("frame");
                if let Some(m) = r.l.hooks.mask.as_mut() { m.clear(); }
                r.g.clear_accepted();
                // The snap itself isn't a dumped frame (its mask goes with the snap).
                let dump = r.dump.take();
                r.draw();
                r.dump = dump;
                let file = out.join(format!("{}.png", name));
                crate::snapshot::save(&mut r.g, &mut r.l, &file)?;
                if let Some(m) = r.l.hooks.mask.as_mut() { m.clear(); }
            }
            "expect" => {
                let want = line[6..].trim();
                if r.top() != want { failed += 1; let _ = writeln!(r.log, "  FAIL expected page {:?}, got {:?}", want, r.top()); }
            }
            "effect" => {
                let want = line[6..].trim();
                if !r.seen.iter().any(|e| e.contains(want)) { failed += 1; let _ = writeln!(r.log, "  FAIL expected an effect with {:?}", want); }
                r.seen.clear();
            }
            other => return Err(format!("line {}: unknown command {}", n + 1, other)),
        }
        let _ = writeln!(log, "{}: {} (t {:.3} -> {:.3}) top: {} sel {}{}", n + 1, line, before, r.t, r.top(), r.l.pages.last().map_or(-1, |p| p.sel), if r.log.is_empty() { String::new() } else { format!("\n{}", r.log.trim_end()) });
        r.log.clear();
    } Ok(()) })();
    if let Err(e) = res { let _ = std::fs::write(out.join("replay-partial.log"), format!("{}error: {}\n", log, e)); return Err(e); }
    if let Some(r) = &run { let _ = writeln!(log, "frames drawn: {}", r.frames); }
    if let Some(c) = run.as_ref().and_then(|r| r.check.as_ref()) {
        let _ = std::fs::write(out.join("damage.txt"), format!("frames {} differ {} redrawn {:.1}% first {}\r\n", c.frames, c.differ, 100.0 * c.redrawn / c.frames.max(1) as f64, c.first_bad.as_deref().unwrap_or("-")));
    }
    let _ = writeln!(log, "{}", if failed == 0 { "replay: all checks passed" } else { "replay: checks FAILED" });
    std::fs::write(out.join("replay.log"), &log).map_err(|e| e.to_string())?;
    print!("{}", log);
    if failed > 0 { Err(format!("{} checks failed", failed)) } else { Ok(()) }
}

/// Environment.ExpandEnvironmentVariables: %NAME% replaced when NAME is set (case-insensitive on Windows).
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        let Some(b) = rest[a + 1..].find('%') else { break };
        let name = &rest[a + 1..a + 1 + b];
        match std::env::var(name) {
            Ok(v) if !name.is_empty() => { out.push_str(&rest[..a]); out.push_str(&v); rest = &rest[a + b + 2..]; }
            _ => { out.push_str(&rest[..a + 1]); rest = &rest[a + 1..]; }
        }
    }
    out.push_str(rest);
    out
}

/// The `page` command: the C# page methods by name, with a game by id and the rest of the line as text.
fn page_by_name(l: &Launcher, f: &[&str], line: &str) -> Option<crate::model::Page> {
    use crate::{more_pages as mp, save_pages as sp};
    let c = l.ctx();
    let game = f.get(1).and_then(|id| c.lib.games.iter().find(|g| &g.id == id));
    // Text after the name (and the game, when the page takes one).
    let rest = |skip: usize| -> String { line.split_whitespace().skip(skip).collect::<Vec<_>>().join(" ") };
    Some(match f[0] {
        "ExportGamesPage" => sp::export_games_page(c),
        "RemoveRomFilePage" => mp::remove_rom_file_page(&rest(2)),
        "RemoveUnzippedPage" => mp::remove_unzipped_page(&rest(2)),
        "RemoveFolderPage" => mp::remove_folder_page(&rest(2)),
        name => {
            let g = game?;
            match name {
                "SavePage" => sp::save_page(g, &rest(3)),
                "BackupsPage" => sp::backups_page(g),
                "ExportPage" => sp::export_page(g),
                "ConflictPage" => sp::conflict_page(g, None),
                "DamagedPage" => sp::damaged_page(g, &rest(3), None),
                "ShareHintsPage" => mp::share_hints_page(g, rest(3) == "true"),
                "HeaderFixPage" => mp::header_fix_page(&[g], None),
                "PatchPage" => mp::patch_page(g, false),
                "Setup3dsPage" => mp::setup_3ds_page(&[g]),
                "CustomGamePage" => mp::custom_game_page(g),
                "PcGamePage" => mp::pc_game_page(g),
                "PcRemovePage" => mp::pc_remove_page(g),
                _ => return None,
            }
        }
    })
}
