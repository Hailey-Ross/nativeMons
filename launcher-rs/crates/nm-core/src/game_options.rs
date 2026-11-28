//! GameOptions: the launcher's per-game and global options (game-options*.txt). So far what engines' settings read.

use crate::config;
use crate::game_entry::GameEntry;
use std::collections::HashMap;

pub const OWN: &str = "own";
pub const FOLLOW: &str = "follow";

/// (key, engine, game only, first generation), in the C# order.
const ALL: [(&str, bool, bool, i32); 12] = [
    ("text", false, false, 0), ("scene", false, false, 0), ("style", false, false, 0), ("sound", false, false, 2), ("button", false, true, 3), ("frame", false, true, 3),
    ("vol", true, false, 0), ("music", true, false, 0), ("sfx", true, false, 0), ("full", true, false, 0), ("wide", true, false, 0), ("smooth", true, false, 0),
];
pub fn generation(g: &GameEntry) -> i32 {
    match g.console.as_deref() {
        Some("Game Boy") => 1,
        Some("Game Boy Color") => 2,
        Some("Game Boy Advance") => 3,
        _ => if g.dex > 493 { 5 } else { 4 },
    }
}
pub fn available(g: &GameEntry) -> bool { generation(g) <= 4 }
fn has(g: &GameEntry, engine: bool, first_gen: i32) -> bool { engine || (available(g) && generation(g) >= first_gen) }
fn file_of(g: Option<&GameEntry>) -> std::path::PathBuf { config::state().join(match g { None => "game-options.txt".to_string(), Some(g) => format!("game-options-{}.txt", g.id) }) }
fn read(g: Option<&GameEntry>) -> HashMap<String, String> {
    let mut d = HashMap::new();
    if let Ok(t) = std::fs::read_to_string(file_of(g)) {
        for l in config::lines(&t) { if let Some(e) = l.find('=') { if e > 0 { d.insert(l[..e].to_string(), l[e + 1..].to_string()); } } }
    }
    // The first version of this setting had a file of its own.
    if g.is_none() && !d.contains_key("text") {
        let old = config::read("text-speed.txt", OWN);
        if old != OWN { d.insert("text".into(), old); }
    }
    d
}
fn stored(g: Option<&GameEntry>, key: &str, game_only: bool) -> String {
    match read(g).get(key) { Some(v) => v.clone(), None => if g.is_none() || game_only { OWN.into() } else { FOLLOW.into() } }
}
pub fn for_game(g: &GameEntry) -> HashMap<String, String> { for_game_ordered(g).into_iter().collect() }
/// ForGame in its Dictionary's order (the order of All), as the C# joins it.
pub fn for_game_ordered(g: &GameEntry) -> Vec<(String, String)> {
    let mut r = Vec::new();
    for (key, engine, game_only, first) in ALL.iter() {
        if !has(g, *engine, *first) { continue; }
        let mut v = stored(Some(g), key, *game_only);
        if v == FOLLOW { v = stored(None, key, *game_only); }
        if v != OWN { r.push((key.to_string(), v)); }
    }
    r
}
pub fn is_engine(key: &str) -> bool { ALL.iter().find(|o| o.0 == key).map_or(false, |o| o.1) }
/// Nearest: a percent to the nearest step (int.TryParse, 0 when it isn't a number), within 0-100.
pub fn nearest(percent: &str, step: i32) -> i32 { let v: i32 = percent.trim().parse().unwrap_or(0); ((v + step / 2) / step * step).clamp(0, 100) }
/// SetIni: one key in one section, every other line kept; a new key goes at the end of its section.
pub fn set_ini(file: &std::path::Path, section: &str, key: &str, value: &str) -> std::io::Result<()> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let mut lines: Vec<String> = config::lines(&text).iter().map(|l| l.to_string()).collect();
    let head = format!("[{}]", section);
    let Some(start) = lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&head)) else {
        lines.push(head);
        lines.push(format!("{} = {}", key, value));
        return config::write_all_lines(file, &lines);
    };
    let mut end = start + 1;
    while end < lines.len() && !lines[end].trim_start().starts_with('[') { end += 1; }
    for i in start + 1..end {
        if let Some(eq) = lines[i].find('=') {
            if eq > 0 && lines[i][..eq].trim().eq_ignore_ascii_case(key) {
                let sp = if lines[i].as_bytes()[eq - 1] == b' ' { " " } else { "" };
                lines[i] = format!("{}{}{}", &lines[i][..eq + 1], sp, value);
                return config::write_all_lines(file, &lines);
            }
        }
    }
    lines.insert(end, format!("{} = {}", key, value));
    config::write_all_lines(file, &lines)
}
