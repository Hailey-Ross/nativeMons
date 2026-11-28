//! The menu pages, ported from the C# LauncherForm page builders (names, order and texts kept).

use crate::art::{app, APP_COLORS};
use crate::model::{Item, Page, Theme};
use nm_core::game_entry::GameEntry;
use nm_core::library::{file_name, Library};
use nm_core::{config, emu3ds, save_sets};
use std::path::PathBuf;

/// The launcher's folder and its state folder (Config.Here, Config.State).
pub struct Config {
    pub here: PathBuf,
    pub state: PathBuf,
}

impl Config {
    pub fn load() -> Config { Config { here: config::here(), state: config::state() } }
    /// Config.Read(name, fallback).
    pub fn read(&self, name: &str, fallback: &str) -> String { std::fs::read_to_string(self.state.join(name)).unwrap_or_else(|_| fallback.to_string()) }
    pub fn has(&self, rel: &str) -> bool { self.here.join(rel).exists() }
}

pub const GEN1_CREDIT: &str = "Parts of Gen1Recomp inspired this project and helped with research along the way. For the Gen1Recomp engine included here: Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).";

/// The library and what the pages read beside it (Library.Games, `current`, first-time setup).
pub struct Ctx {
    pub lib: Library,
    pub current: Option<usize>,
    pub first_setup: bool,
    pub work_area: (i32, i32),
    pub wide_window: bool,
    pub light_auto: bool,
    pub light_why: &'static str,
}


pub fn theme_of(g: &GameEntry) -> Theme {
    let t = &g.theme;
    let mut th = Theme::make(&t.main, &t.deep, &t.light, Some(&t.finish));
    th.game_id = t.game_id.clone();
    th
}

pub fn engine_name(engine: &str, g: Option<&GameEntry>) -> String {
    match engine {
        "recomp" => emu3ds::ZAKURO.name.into(),
        "3ds" => match if g.is_some() { emu3ds::for_game(g) } else { emu3ds::default() } { Some(e) => e.name.into(), None => "a 3DS emulator".into() },
        "fulldex" => "Fulldex (widescreen, 60 fps)".into(),
        "port" => "the Platinum PC port (native)".into(),
        "nativeplat" => "nativeplat (native)".into(),
        "pc" => "its own program".into(),
        _ => "Gen1Recomp (native, with mods)".into(),
    }
}
pub fn engine_short(engine: &str, g: Option<&GameEntry>) -> String {
    match engine {
        "recomp" => emu3ds::ZAKURO.short.into(),
        "3ds" => match if g.is_some() { emu3ds::for_game(g) } else { emu3ds::default() } { Some(e) => e.short.into(), None => "3DS".into() },
        "fulldex" => "Fulldex".into(),
        "port" => "Platinum PC port".into(),
        "nativeplat" => "nativeplat".into(),
        "pc" => "PC".into(),
        _ => "Gen1Recomp".into(),
    }
}
fn mon_bridge() -> bool { config::mon_bridge().is_file() }
fn move_tools(g: &GameEntry) -> bool { g.save_tools() || (g.ctr.is_some() && nm_core::ctr3ds_saves::main(g).is_file()) }
fn label_of(g: &GameEntry, i: Option<usize>) -> String {
    let Some(i) = i else { return String::new() };
    let r = &g.roms[i];
    if g.is_modified(r) { return "Romhack or modified".into(); }
    if !g.roms.iter().enumerate().any(|(k, x)| k != i && x.revision == r.revision && !g.is_modified(x)) { return r.label(); }
    format!("{} ({})", r.label(), &r.sha1[..6])
}

pub fn main_page(c: &Ctx) -> Page {
    let mut p = Page::new("nativeMons");
    p.main = true;
    let games = &c.lib.games;
    let cur = c.current.map(|i| &games[i]);
    match cur {
        Some(g) => { let id = g.id.clone(); p.items.push(Item::new(&format!("Play {}", g.title), &format!("Start {} with {}.", g.title, engine_name(&g.engine(), Some(g))), true).icon('\u{E768}').run(move |l| l.play(&id))) }
        None => p.items.push(Item::new("Play", "Add a ROM folder first.", false).icon('\u{E768}')),
    }
    let have = games.iter().filter(|g| g.playable()).count();
    p.items.push(Item::new("Games", &format!("{} of {} found in your ROM folders.", have, games.len()), true).icon('\u{E7FC}').run(|l| l.push_with(|c| games_page(c, None))));
    p.items.push(Item::new("My Collection", "Your Pokedex, across your games.", have > 0 && mon_bridge()).icon('\u{E8F1}').run(|l| l.open_collection()));
    let mods = games.iter().any(|g| g.on_gen1());
    p.items.push(Item::new("Mods", if mods { "Mods for the Gen1Recomp games." } else { "Needs a Gen1Recomp game." }, mods).icon('\u{EA86}').run(|l| l.push_with(crate::mods_pages::mod_games_page)));
    let folders = config::rom_folders().len();
    p.items.push(Item::new("ROM folders", &format!("{}{}", folders, if folders == 1 { " folder searched for your ROMs." } else { " folders searched for your ROMs." }), true).icon('\u{E8B7}').run(|l| l.push_with(folders_page)));
    let emus = emu3ds::ALL.iter().filter(|e| e.installed()).count();
    let emu_hint = if !emu3ds::enabled() { "3DS games are off. Turn them on here.".to_string() } else if emus == 0 { "Pick an emulator for your 3DS games.".to_string() } else { format!("{}{}", emus, if emus == 1 { " emulator for your 3DS games." } else { " emulators for your 3DS games." }) };
    p.items.push(Item::new("Emulators", &emu_hint, true).icon('\u{E7F4}').run(|l| l.push(emu_page())));
    p.items.push(Item::new("Move Pokemon", "From one game to another.", mon_bridge() && games.iter().filter(|g| move_tools(g)).count() > 1).icon('\u{E8AB}').run(|l| l.open_box_view()));
    p.items.push(Item::new("Settings", "Game options, sprites, credits.", true).icon('\u{E713}').run(|l| l.push_with(settings_page)));
    p.items.push(Item::new("Exit", "Close the launcher.", true).icon('\u{E711}').run(|l| l.request_close()));
    p.on_back(|_| {})
}

fn wide_fits(a: (i32, i32)) -> bool { a.0 >= 1000 && a.0 as f32 > a.1 as f32 * 1.2 }

pub fn settings_page(c: &Ctx) -> Page {
    let cfg = Config::load();
    let mut p = Page::new("Settings");
    let onoff = |b: bool| if b { "On" } else { "Off" };
    let offon = |b: bool| if b { "off" } else { "on" };
    let emu_enabled = emu3ds::enabled();
    let drawn_wallpaper = cfg.read("wallpaper.txt", "") == "drawn";
    let quick_start = cfg.read("quick-start.txt", "on").trim() != "off";
    let discord = cfg.read("discord.txt", "on").trim() != "off";
    p.items.push(Item::new("Fulldex", "", config::fulldex().is_file()).icon('\u{E7FC}').info("DS and GBA games: widescreen, frame rate and resolution.").run(|l| l.open_engine_settings("fulldex")));
    p.items.push(Item::new("Gen1Recomp", "", config::gen1().join("main.lua").is_file()).icon('\u{E7FC}').info("Game Boy, Game Boy Color and GBA games: text speed and battles.").run(|l| l.open_engine_settings("gen1")));
    p.items.push(Item::new(&format!("3DS games: {}", onoff(emu_enabled)), "", true).icon('\u{E7F4}').tint(APP_COLORS[app("Emu")]).info(&format!("Play your Nintendo 3DS Pokemon games with an emulator you choose, from Emu on the home screen. Nothing downloads until you pick one. Select to turn it {}.", offon(emu_enabled))).run(move |l| {
        config::write("3ds-games.txt", if emu_enabled { "off" } else { "on" });
        l.rebuild_top(false, settings_page);
        l.replace_main();
    }));
    if nm_core::recomp::installed() {
        use nm_core::recomp;
        let ask = recomp::ask_to_share();
        p.items.push(Item::new(&format!("Ask to share Zakuro findings: {}", onoff(ask)), "", true).icon('\u{E72D}').info(&format!("After a 3DSrecomp+Zakuro session that found new code its build missed, ask whether to share that list (addresses only) for everyone's builds. Select to turn it {}.", offon(ask)))
            .run(move |l| { recomp::set_ask_to_share(!ask); l.rebuild_top(true, settings_page); }));
        let fast = recomp::fast();
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        p.items.push(Item::new(&format!("3DS build speed: {}", if fast { "Fast" } else { "Quiet" }), "", true).icon('\u{E7FC}').info(&format!("How 3DSrecomp+Zakuro builds a game: Fast uses all but one of your {} processor threads, Quiet half of them, both at the lowest priority. About {} minutes per game now. Select to switch.", threads, recomp::minutes(recomp::cores())))
            .run(move |l| { recomp::set_fast(!fast); l.rebuild_top(true, settings_page); }));
    }
    p.items.push(Item::new("Import saves", "", mon_bridge()).icon('\u{E896}').info("Bring in saves from other emulators and folders, such as those beside your ROMs.").run(|l| l.open_import_saves()));
    p.items.push(Item::new("Export saves", "", c.lib.games.iter().any(|g| g.save_tools())).icon('\u{E898}').info("Send a save back to your 3DS, a flash cart or another emulator.").run(|l| l.push_with(crate::save_pages::export_games_page)));
    p.items.push(Item::new("Download Pokedex", "", true).icon('\u{E896}').info("Gets every Pokemon picture and Pokedex entry for offline play. Run it again at any time: it only fetches what's missing.").run(|l| l.get_all_sprites()));
    p.items.push(Item::new("Game options", "", true).icon('\u{E8D2}').info("Volume, fullscreen, text speed, battle style and more, for every game.").run(|l| l.push(game_options_page(None))));
    p.items.push(Item::new(&format!("Wallpapers: {}", if drawn_wallpaper { "Drawn scenes" } else { "From your games" }), "", true).icon('\u{E790}').info("Drawn scenes or pictures from your own play, and a picture of your own for any game.").run(|l| l.push_with(|c| wallpaper_page(c, false))));
    p.items.push(Item::new("Controls", "", true).icon('\u{E765}').info("The keys each engine uses while you play: saving, fast forward, snapshots, screenshots.").run(|l| l.push(controls_page())));
    p.items.push(Item::new("Screenshots", "", true).icon('\u{E8B9}').info("Open the folders where nativeplat and Infinite Fusion put the screenshots you take while you play.").run(|l| l.push_with(screenshots_page)));
    if wide_fits(c.work_area) {
        p.items.push(Item::new(&format!("Wide layout: {}", onoff(c.wide_window)), "", true).icon('\u{E7F4}').info(&format!("Fills more of the screen, with the menu as a column on the left and the chosen option shown large beside it. Off is the phone. Select to turn it {}.", offon(c.wide_window))).run(move |l| {
            let wide = l.wide_window;
            config::write("layout.txt", if wide { "phone" } else { "wide" });
            l.apply_layout(!wide);
            l.rebuild_top(false, settings_page);
        }));
    }
    p.items.push(Item::new(&format!("Quick start: {}", onoff(quick_start)), "", true).icon('\u{E945}').info(&format!("Games open straight at their title screen, past the intro. The last game you played waits there while the menu is open, so Play starts it at once. Gen1Recomp, nativeplat, DS games on Fulldex, and 3DSrecomp+Zakuro. Select to turn it {}.", offon(quick_start))).run(move |l| {
        config::write("quick-start.txt", if quick_start { "off" } else { "on" });
        l.rebuild_top(false, settings_page);
    }));
    // Rust only (light.rs); parity runs leave it out, as the C# has no such row.
    if std::env::var("NATIVEMONS_TEST_CSHARP_ROWS").as_deref() != Ok("1") {
        let ls = crate::light::read();
        p.items.push(Item::new(&crate::light::row_text(ls, c.light_auto), "", true).icon('\u{E945}').info(&crate::light::row_info(ls, c.light_auto, c.light_why)).run(move |l| {
            crate::light::write(crate::light::next(ls));
            l.light = crate::light::effective(crate::light::read(), l.host.light_auto);
            l.rebuild_top(true, settings_page);
        }));
    }
    p.items.push(Item::new(&format!("Discord status: {}", onoff(discord)), "", true).icon('\u{E8BD}').info(&format!("While you play, Discord shows which game, played through nativeMons. Select to turn it {}.", offon(discord))).run(move |l| {
        nm_core::discord::set_enabled(!discord);
        l.rebuild_top(false, settings_page);
    }));
    p.items.push(Item::new("Updates", "", true).icon('\u{E895}').info("Keeps the launcher and every engine up to date from this project's releases. Your saves and settings stay as they are.").run(|l| l.open_updates()));
    p.items.push(Item::new("Credits", "", true).icon('\u{E946}').info("The projects this is built from.").run(|l| l.push(credits_page())));
    p.items.push(Item::new("Last session", "", true).icon('\u{E81C}').info("How the last game ended.").run(|l| { let s = config::read("last-session.txt", "No session yet."); l.show_text("Last session", &s); }));
    p
}

pub fn credits_page() -> Page {
    let mut p = Page::new("Credits");
    p.links = true;
    p.body = Some([
        "Pokemon Fulldex by mamesj (github.com/mamesj/fulldex), a fork of ssrecomp by ClickingMaster. It uses melonDS, and mGBA by way of Pokemon GBA Widescreen. Function names come from the pokebw2, aletheia and pokeplatinum decompilations.",
        "\n\nPokemon Platinum PC port by cybervisi0n and contributors (github.com/cybervisi0n/pokeplatinum), from the pret decompilation, using libntr (MIT). They aren't affiliated with nativeMons.",
        "\n\nnativeplat by sohamb117 (github.com/sohamb117/gen4recomp), native Diamond, Pearl and Platinum from the pret decompilations. Not affiliated with nativeMons.",
        "\n\nPokemon Infinite Fusion and Infinite Fusion 2 by chardub (Frogman) and their contributors (github.com/infinitefusion/infinitefusion-e18, github.com/infinitefusion/infinitefusion-hoenn-public), made with Pokemon Essentials on RPG Maker XP. Its custom fusion sprites are by the Infinite Fusion artist community. Installed from your own copy or its GitHub; not included, and not affiliated with nativeMons.",
        "\n\nMoving Pokemon between generations uses the conversion code of OpenHome by Andrew Benington (github.com/andrewbenington/OpenHome, GPL-3.0), running on Node.js. Open in OpenHome uses your own copy.",
        "\n\nPokemon pictures served by pogo.hails.app (hailsDotGO), from the PokeAPI sprites collection. Pokedex entries from PokeAPI (pokeapi.co).",
        "\n\nFind mods reads the Gen1Recomp community mod index (github.com/bryanthaboi/gen1recomp-mod-index). Its listings are by the mods' authors, and the mods are theirs.",
        "\n\nThe GBA player is ours, in our Fulldex fork, on mGBA by endrift and contributors (MPL-2.0), after Pokemon GBA Widescreen by mamesj. Widescreen for romhacks and the versions our patches don't cover comes from PokeDaisy's EXPAND (GPL-3.0). Based on PokeDaisy by Lidor Itzhari - https://github.com/lidor30/pokedaisy",
        "\n\nRomhacks' title screens are captured with mGBA by endrift and contributors (github.com/mgba-emu/mgba, MPL-2.0), as its libretro core.",
        "\n\nMoving Pokemon uses PKHeX.Core by Kaphotics and contributors (github.com/kwsch/PKHeX, GPL-3.0).",
        "\n\n3DSrecomp+Zakuro: Zakuro and 3dsrecomp by fearkov (github.com/fearkov/zakuro, github.com/fearkov/3dsrecomp, both MIT), built from our fork (github.com/Hailey-Ross/zakuro) with a small build tool of ours; Zig (ziglang.org, MIT) compiles each game on your PC. Not affiliated with nativeMons.",
        "\n\nOptional 3DS emulators, downloaded only when you choose one, each built from our fork with small changes for nativeMons: Azahar by the Azahar team, from Citra (github.com/azahar-emu/azahar, GPL-2.0-or-later; ours: github.com/Hailey-Ross/azahar); AzaharPlus by its contributors, from Azahar (github.com/AzaharPlus/AzaharPlus, GPL-3.0; ours, without its system-file downloader: github.com/Hailey-Ross/AzaharPlus). Not the official builds, and not affiliated with nativeMons.",
        "\n\nNoto Sans by Google (notofonts.github.io, SIL Open Font License 1.1), for where Windows' own fonts aren't installed.",
        // Rust only (deliberately unlike the C#): the Rust launcher's own libraries and the Symbols 2 font.
        "\n\nNoto Sans Symbols 2 by Google (notofonts.github.io, SIL Open Font License 1.1), for symbols such as ▶ ◀ ★.",
        "\n\nDrawn with tiny-skia (BSD-3-Clause) and cosmic-text (MIT); window and input by SDL 3 (Zlib); JPEG by mozjpeg/libjpeg-turbo: this software is based in part on the work of the Independent JPEG Group; HTTPS by rustls with Mozilla's root certificates (webpki-roots, CDLA-Permissive-2.0); Unicode data (Unicode-3.0). The other Rust crates' licenses are in the licenses folder (rust-crates.txt).",
        "\n\nhails.nds-Patcher by Hails. Launcher design from the hails-additions branch of the Skate 3 Rust Engine (SK8-ENGINE), github.com/Hailey-Ross/rusty-trucks.",
        "\n\n",
        GEN1_CREDIT,
        "\n\nNot affiliated with or endorsed by Nintendo, Creatures Inc., GAME FREAK inc. or The Pokemon Company. Pokemon and all related names are the property of their owners. No games are included: you need your own ROMs.",
    ]
    .concat());
    p
}

/// Natural: numbers compare as numbers (each run of digits padded to 10).
fn natural(s: &str) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() { digits.push(ch); continue; }
        if !digits.is_empty() { out.push_str(&format!("{:0>10}", digits)); digits.clear(); }
        out.push(ch);
    }
    if !digits.is_empty() { out.push_str(&format!("{:0>10}", digits)); }
    out
}
/// Family: the title without a trailing version ("v1.3.1").
fn family(title: &str) -> String {
    let re = regex::Regex::new(r"\s+v?\d[\w.\- ]*$").unwrap();
    re.replace(title, "").into_owned()
}
fn culture_cmp(a: &str, b: &str) -> std::cmp::Ordering { a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| b.cmp(a)) }

pub fn games_page(c: &Ctx, at: Option<&str>) -> Page {
    let mut p = Page::new("Games");
    let cfg = Config::load();
    let folded: Vec<String> = cfg.read("games-folded.txt", "").split(['\n', '\r']).filter(|s| !s.is_empty()).map(String::from).collect();
    let games = &c.lib.games;
    let originals: Vec<usize> = (0..games.len()).filter(|&i| !games[i].is_hack() && games[i].pc.is_none()).collect();
    let hacks: Vec<usize> = (0..games.len()).filter(|&i| games[i].is_hack() || games[i].pc.is_some()).collect();
    for (key, name, sec) in [("originals", "Original games", originals), ("hacks", "Romhacks and fan games", hacks)] {
        if sec.is_empty() { continue; }
        let open = !folded.iter().any(|f| f == key);
        if Some(key) == at { p.sel = p.items.len() as i32; }
        let ready = sec.iter().filter(|&&i| games[i].playable()).count();
        let mut it = Item::new(&format!("{}: {}", name, sec.len()), "", true).run(move |l| {
            let mut folded: Vec<String> = nm_core::config::read("games-folded.txt", "").split(['\n', '\r']).filter(|s| !s.is_empty()).map(String::from).collect();
            if open { folded.push(key.to_string()); } else { folded.retain(|f| f != key); }
            nm_core::config::write("games-folded.txt", &folded.join("\n"));
            l.rebuild_top(false, |c| games_page(c, Some(key)));
        });
        it.fold = if open { 1 } else { 2 };
        it.info = format!("{} {} of {} ready to play.", if open { "Select or press Left to hide these." } else { "Select or press Right to show these." }, ready, sec.len());
        p.items.push(it);
        if open { add_game_rows(c, &mut p, &sec); }
    }
    if p.items.is_empty() { p.items.push(Item::new("No games listed", &c.lib.note, false)); }
    p.items.push(Item::new("Add a PC game", "Any game with its own program, such as a fan game.", true).icon('\u{E710}').run(|l| l.add_pc_game()).info("From its folder (run where it is), its .zip (unpacked into the launcher) or its .exe. It's run as it is, with no extras: the launcher starts it and shows it on Discord, and leaves its saves to the game."));
    p
}

fn add_game_rows(c: &Ctx, p: &mut Page, list: &[usize]) {
    let games = &c.lib.games;
    let order = |i: usize| list.iter().position(|&x| x == i).unwrap_or(0);
    let base_at = |i: usize| -> usize {
        match &games[i].base_id {
            Some(b) => games.iter().position(|x| &x.id == b).unwrap_or_else(|| order(i)),
            None => order(i),
        }
    };
    let mut sorted = list.to_vec();
    sorted.sort_by(|&a, &b| {
        let (ga, gb) = (&games[a], &games[b]);
        (if ga.playable() { 0 } else { 1 }).cmp(&(if gb.playable() { 0 } else { 1 }))
            .then(base_at(a).cmp(&base_at(b)))
            .then_with(|| culture_cmp(&family(&ga.title), &family(&gb.title)))
            .then_with(|| culture_cmp(&natural(&gb.title), &natural(&ga.title)))
    });
    for i in sorted {
        let g = &games[i];
        let why = nm_core::adding::why_not_playable(&c.lib, g);
        let id = g.id.clone();
        let mut it = Item::new(&g.title, if g.playable() { "" } else { &why }, g.playable() || g.pc.is_some()).icon('\u{E7FC}').run(move |l| l.push_game(&id, game_page));
        it.theme = Some(theme_of(g));
        it.game_id = Some(g.id.clone());
        let console = g.console.clone().unwrap_or_default();
        it.info = if g.pc.is_some() { if g.playable() { "A PC game of its own, installed in the launcher.".into() } else { why.clone() } } else if g.playable() { format!("{}. Runs with {}.", console, engine_name(&g.engine(), Some(g))) } else { format!("{}. {}", console, why) };
        it.records = game_records(g);
        p.items.push(it);
    }
}

fn game_records(g: &GameEntry) -> Vec<String> {
    let mut r = Vec::new();
    if let Some(f) = &g.gen1_rom { r.push(format!("Gen1Recomp: {}{}", file_name(f), if g.imported { " (prepared)" } else { "" })); } else if g.imported { r.push("Gen1Recomp: prepared".into()); }
    if let Some(f) = &g.fulldex_rom { r.push(format!("Fulldex: {}", file_name(f))); }
    if nm_core::library::needs_patch(g.fulldex_rom.as_deref()) { r.push("Encrypted: needs patching for Fulldex".into()); }
    if let Some(f) = &g.nativeplat_rom { r.push(format!("nativeplat: {}", file_name(f))); }
    if save_sets::all(g).len() > 1 { r.push(format!("Save: {}", save_sets::name(g, &save_sets::active(g)))); }
    r
}

pub fn game_page(c: &Ctx, g: &GameEntry) -> Page {
    if g.pc.is_some() { return crate::more_pages::pc_game_page(g); }
    let mut p = Page::new(&g.title);
    p.theme = Some(theme_of(g));
    p.game_id = Some(g.id.clone());
    let engine = g.engine();
    let gid = g.id.clone();
    let id = move || gid.clone();
    let mut play = Item::new("Play", &format!("Start with {}.", engine_name(&engine, Some(g))), g.playable()).icon('\u{E768}');
    { let id = id(); play = play.run(move |l| l.play(&id)); }
    play.records = game_records(g);
    p.items.push(play);
    let engines = g.engines();
    if engines.len() > 1 {
        p.items.push(Item::new(&format!("Engine: {}", engine_short(&engine, Some(g))), "", true).icon('\u{E72C}').info(&format!("{} plays with {}. Select to choose from its {} engines.", g.title, engine_name(&engine, Some(g)), engines.len())).run({ let id = id(); move |l| l.push_game(&id, |_, g| engine_page(g)) }));
    }
    if g.ctr.is_some() && g.ctr_plain_rom.as_deref().map_or(false, nm_core::ctr_header_fix::needs) {
        p.items.push(Item::new("Fix 3DS header", "", true).icon('\u{E90F}').info("Your dump is decrypted, but its header still says encrypted, which 3DSrecomp+Zakuro refuses. The fix changes a few bytes, with a backup.").run({ let id = id(); move |l| l.push_game(&id, |_, g| crate::more_pages::header_fix_page(&[g], None)) }));
    } else if g.ctr.is_some() && nm_core::ctr_header_fix::can_undo(g.ctr_plain_rom.as_deref()) {
        let (gid, rom) = (id(), g.ctr_plain_rom.clone().unwrap_or_default());
        p.items.push(Item::new("Undo 3DS header fix", "", true).icon('\u{E7A7}').info("Puts back the bytes the header fix changed.").run(move |l| {
            match nm_core::ctr_header_fix::undo(&rom) {
                Some(why) => l.show_text("Not undone", &why),
                None => { let gid = gid.clone(); crate::more_pages::rescan(l, move |l| l.push_game(&gid, game_page)); }
            }
        }));
    }
    if g.on_recomp() {
        use nm_core::recomp;
        let st = recomp::state(g);
        let label = if recomp::improving(g) { "Improving" } else { match st.as_str() { "ready" => "Ready", "old" => "Can be improved", "outdated" => "Needs a rebuild", _ => "Not built yet" } };
        p.items.push(Item::new(&format!("3DSrecomp+Zakuro build: {}", label), "", true).icon('\u{E896}')
            .info(&format!("Its code, recompiled on this PC from your own dump so it runs natively. It never leaves this PC. Select to {}.", if st == "ready" { "build it again" } else { "build it" }))
            .run({ let id = id(); move |l| l.push_game(&id, |_, g| recomp_ask_page(&[g], None)) }));
        let fps = config::read(&format!("zakuro-60fps-{}.txt", g.id), "off") == "on";
        p.items.push(Item::new(&format!("60 fps (Zakuro): {}", if fps { "On" } else { "Off" }), "", true).icon('\u{E916}')
            .info(&format!("A community code that doubles the frames, and with them the whole game's speed: everything runs twice as fast. It only takes on the game version it was tested on. Select to turn it {}.", if fps { "off" } else { "on" }))
            .run({ let id = id(); move |l| { config::write(&format!("zakuro-60fps-{}.txt", id), if fps { "off" } else { "on" }); l.rebuild_game(&id, true, game_page); } }));
        let mods = nm_core::ctr3ds_saves::emu_data(&emu3ds::ZAKURO).join("mods").join(g.ctr.as_deref().unwrap_or("").to_uppercase());
        p.items.push(Item::new("Zakuro mods folder", "", true).icon('\u{E8B7}').info("Mods for this game, laid out as for Luma3DS and Azahar (romfs, exefs, code.ips). Cheats are in the game's menu: Esc, then Cheats.")
            .run(move |l| { let _ = std::fs::create_dir_all(&mods); l.effects.push(crate::input::Effect::OpenFolder(mods.display().to_string())); }));
        let found = recomp::hint_count(g);
        if found > 0 {
            p.items.push(Item::new("Share Zakuro findings", "", true).icon('\u{E72D}').info(&format!("Zakuro found {} places in its code that its build didn't cover. Sharing them helps everyone's builds of this game run faster.", found))
                .run({ let id = id(); move |l| l.push_game(&id, |_, g| crate::more_pages::share_hints_page(g, false)) }));
        }
    }
    if g.roms.len() > 1 {
        let chosen = g.chosen_rom().map(|r| file_name(&r.file)).unwrap_or_default();
        p.items.push(Item::new(&format!("ROM version: {}", label_of(g, g.chosen)), "", true).icon('\u{E8B7}').info(&format!("{} different dumps of this game were found. {} is in use.", g.roms.len(), chosen)).run({ let id = id(); move |l| l.push_game(&id, |_, g| roms_page(g)) }));
    }
    if g.is_hack() {
        let base = c.lib.games.iter().find(|x| Some(&x.id) == g.base_id.as_ref());
        let of = base.map(|b| b.title.clone()).unwrap_or_else(|| g.base_id.clone().unwrap_or_default());
        let file = g.chosen_rom().map(|r| format!(" ({})", file_name(&r.file))).unwrap_or_default();
        p.items.push(Item::new("Rename", &format!("Shown as {}.", g.title), g.chosen.is_some()).icon('\u{E70F}').info(&format!("A romhack of {}, named after its file{}. It runs on Fulldex, with a save of its own, and its saves read as {}'s do: for Move Pokemon, My Collection and PKHeX.", of, file, of)).run({ let (id, title, sha1) = (id(), g.title.clone(), g.chosen_rom().map(|r| r.sha1.clone()).unwrap_or_default()); move |l| {
            let (id, sha1) = (id.clone(), sha1.clone());
            l.ask_text("Rename", "Name for this game:", &title, Box::new(move |l, name| {
                let Some(name) = name.filter(|n| !n.trim().is_empty()) else { return };
                let _ = nm_core::library::rename_hack(&sha1, name.trim());
                crate::more_pages::rescan(l, |l| {
                    l.pages.pop();
                    if let Some(g) = l.game(&id) { l.push(game_page(l.ctx(), &g)); }
                    l.replace_main();
                });
            }));
        } }));
    }
    p.items.push(Item::new("Saves", &format!("In use: {}.", save_sets::name(g, &save_sets::active(g))), true).icon('\u{E896}').run({ let id = id(); move |l| l.open_saves(&id) }));
    p.items.push(Item::new("Game options", if nm_core::game_options::available(g) { "Volume, text speed, battle style and more." } else { "Volume, fullscreen and more." }, true).icon('\u{E8D2}').run({ let id = id(); move |l| l.push_game(&id, |_, g| game_options_page(Some(g))) }));
    if g.on_gen1() { p.items.push(Item::new("Mods", &format!("Mods for {}.", g.title), true).icon('\u{EA86}').run({ let id = id(); move |l| l.push_game(&id, |_, g| crate::mods_pages::mods_page(g)) })); }
    p.items.push(Item::new("Save backups", "Checked copies you can go back to.", true).icon('\u{E81C}').run({ let id = id(); move |l| l.push_game(&id, |_, g| crate::save_pages::backups_page(g)) }));
    p.items.push(Item::new("Export save", "Send it to a 3DS, flash cart or emulator.", true).icon('\u{E898}').run({ let id = id(); move |l| l.push_game(&id, |_, g| crate::save_pages::export_page(g)) }));
    p.items.push(Item::new("Open in PKHeX", "View or edit this game's save in PKHeX.", true).icon('\u{E70F}').run({ let id = id(); move |l| l.open_in_pkhex(&id) }));
    p.items.push(Item::new("Open in OpenHome", "Move Pokemon across games and generations in OpenHome.", true).icon('\u{E8AB}').run({ let id = id(); move |l| l.open_in_openhome(&id) }));
    if engine == "gen1" { p.items.push(Item::new("Bring back the edited save", "Takes the save you edited in PKHeX. Your older save slots stay.", true).icon('\u{E896}').run({ let id = id(); move |l| l.bring_back(&id) })); }
    if g.imported && g.gen1_rom.is_some() { p.items.push(Item::new("Prepare again", "Rebuilds the game data from your ROM. Saves are kept.", true).icon('\u{E72C}').run({ let id = g.id.clone(); move |l| if let Some(g) = l.game(&id) { l.import(&g, None) } })); }
    if engine == "fulldex" && nm_core::library::needs_patch(g.fulldex_rom.as_deref()) { p.items.push(Item::new("Patch this ROM", "It won't load until its secure area is decrypted.", true).icon('\u{E90F}').run({ let id = id(); move |l| l.push_game(&id, |_, g| crate::more_pages::patch_page(g, false)) })); }
    p
}

fn rom_engines(g: &GameEntry, i: usize) -> String {
    let r = &g.roms[i];
    let (mut yes, mut no): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    if g.fulldex_code.is_some() { if r.encrypted { no.push("Fulldex until it's patched"); } else { yes.push("Fulldex"); } }
    if g.port.is_some() { yes.push("Platinum PC port"); }
    if g.gen1.is_some() && !g.is_modified(r) { if r.gen1_ok { yes.push("Gen1Recomp"); } else { no.push("Gen1Recomp"); } }
    if g.nativeplat.is_some() { if r.nativeplat_ok { yes.push("nativeplat"); } else { no.push("nativeplat"); } }
    format!("{}{}", if !yes.is_empty() { format!("Runs on {}.", yes.join(", ")) } else { String::new() }, if !no.is_empty() { format!(" Not on {}.", no.join(", ")) } else { String::new() })
}

pub fn roms_page(g: &GameEntry) -> Page {
    let mut p = Page::new(&format!("ROM version: {}", g.title.replace("Pokemon ", "")));
    p.theme = Some(theme_of(g));
    let mut order: Vec<usize> = (0..g.roms.len()).collect();
    order.sort_by(|&a, &b| g.roms[b].revision.cmp(&g.roms[a].revision).then_with(|| culture_cmp(&g.roms[a].file, &g.roms[b].file)));
    for i in order {
        let r = &g.roms[i];
        let now = Some(i) == g.chosen;
        let modified = g.is_modified(r);
        let mut it = Item::new(&format!("{}{}", label_of(g, Some(i)), if now { ": In use" } else { "" }), "", true).icon('\u{E8B7}')
            .run({ let (id, sha) = (g.id.clone(), r.sha1.clone()); move |l| l.choose_version(&id, &sha, now) });
        it.tile_hint = file_name(&r.file);
        it.info = format!("{}{}{}", rom_engines(g, i).trim(),
            if modified { " A romhack or changed copy, not a clean one. It only runs on Fulldex, and has its own save there, separate from this game's." } else { " Your saves stay with this game whichever version you choose." },
            if modified || r.verified() || !g.can_verify() { "" } else { " No engine here has verified this dump: it may be modified." });
        it.records = vec![format!("File: {}", r.file), format!("SHA-1: {}", r.sha1)];
        p.items.push(it);
    }
    p
}

fn has_master(g: &GameEntry) -> bool { (g.fulldex_code.is_some() || g.nativeplat.is_some()) && g.save.is_some() }

/// SavesPage. A game without a master save asks Gen1Recomp for its slot, summed up by SaveInfo.FromSlot.
/// SavesPage's slow part (Gen1Recomp's slot asked, each save read), apart so OpenSaves runs it behind its card.
pub fn saves_rows(g: &GameEntry) -> Vec<(String, bool, String, String)> {
    let active = save_sets::active(g);
    save_sets::all(g).into_iter().map(|set| {
        let in_use = set == active;
        let file = if !has_master(g) {
            use nm_core::saves::{gen1_slot, gen1_slot_file, read_slot};
            let slot = if in_use { gen1_slot(g, &format!("active {}", g.gen1.clone().unwrap_or_default())) } else { read_slot(g, &set) };
            gen1_slot_file(g, slot.as_deref())
        } else if in_use { Some(nm_core::saves::master_of(g)) } else { Some(save_sets::dir(g, &set).join("fulldex.sav")) };
        let saved = file.as_deref().and_then(nm_core::save_info::read_save_info);
        let what = match (&saved, &file) {
            (Some(s), Some(f)) => s.details(nm_core::save_info::write_time(f)),
            _ => "Nothing saved here yet, or it's only in Gen1Recomp.".to_string(),
        };
        (set, in_use, what, saved.as_ref().map(|s| s.tile_line()).unwrap_or_default())
    }).collect()
}

pub fn saves_page(g: &GameEntry) -> Option<Page> { Some(saves_page_of(g, saves_rows(g))) }
pub fn saves_page_of(g: &GameEntry, rows: Vec<(String, bool, String, String)>) -> Page {
    let mut p = Page::new(&format!("{} saves", g.title));
    p.theme = Some(theme_of(g));
    for (set, in_use, what, tile) in rows {
        if in_use { p.sel = p.items.len() as i32; }
        let (id, one) = (g.id.clone(), set.clone());
        let mut it = Item::new(&save_sets::name(g, &set), "", true).icon('\u{E896}').run(move |l| {
            if let Some(g) = l.ctx().lib.games.iter().find(|x| x.id == id) { let p = crate::save_pages::save_page(g, &one); l.push(p); }
        });
        it.theme = Some(theme_of(g));
        it.info = format!("{}{}", if in_use { "In use now.\n" } else { "" }, what);
        it.tile_hint = tile;
        p.items.push(it);
    }
    p.items.push(Item::new("New save", "", true).icon('\u{E710}').info("Starts this game from the beginning. Your other saves stay as they are.").run({ let id = g.id.clone(); move |l| {
        let id = id.clone();
        l.ask_text("New save", "Name for the new save:", "", Box::new(move |l, name| {
            if let Some(name) = name { l.save_action(&id, "Making a new save", move |gs, g| nm_core::saves::new_save(gs, g, Some(&name)), 1); }
        }));
    } }));
    p
}

pub fn folders_page(c: &Ctx) -> Page {
    let mut p = Page::new("ROM folders");
    for f in config::rom_folders() {
        let name = file_name(f.trim_end_matches('\\'));
        p.items.push(Item::new(&name, "", true).icon('\u{E8B7}').info(&format!("{}\n\nSelect to stop searching this folder. Your ROMs stay where they are.", f)).run({ let f = f.clone(); move |l| l.push(crate::more_pages::remove_folder_page(&f)) }));
    }
    for f in config::rom_files() {
        let gone = if std::path::Path::new(&f).exists() { "" } else { "\n\nNot found there any more." };
        p.items.push(Item::new(&file_name(&f), "", true).icon('\u{E8E5}').info(&format!("{}{}\n\nA ROM added on its own. Select to stop using it. The file stays where it is.", f, gone)).run({ let f = f.clone(); move |l| l.push(crate::more_pages::remove_rom_file_page(&f)) }));
    }
    if let Ok(rd) = std::fs::read_dir(config::unzipped()) {
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|d| d.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            p.items.push(Item::new(&format!("{}.zip", d.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()), "", true).icon('\u{E8B7}').info("ROMs taken out of this zip, kept by nativeMons in its own folder.\n\nSelect to delete these copies. The zip itself was never changed."));
        }
    }
    p.items.push(Item::new("Add a folder", "", true).icon('\u{E710}').run(|l| l.add_folder()).info("nativeMons finds games by reading each ROM, so file names don't matter. No games are included: you need your own ROMs.\n\nYou can also drop a folder, a ROM or a .zip on this window."));
    p.items.push(Item::new("Add a ROM file", "", true).icon('\u{E8E5}').run(|l| l.add_rom_file()).info("One .gb, .gbc, .gba or .nds ROM, used where it is. A .zip works too: its ROMs are copied out, and the zip isn't changed."));
    if !config::rom_folders().is_empty() || !config::rom_files().is_empty() || c.lib.games.iter().any(|g| g.playable()) {
        p.items.push(Item::new(if c.first_setup { "Complete setup" } else { "Done" }, "", true).icon('\u{E73E}').info("Go back to the main screen. Your folders are saved.").run(|l| l.finish_folders()));
    }
    p
}

pub const CONTROL_SHEETS: [[&str; 3]; 6] = [
    ["Fulldex", "DS and GBA games in widescreen.",
        "Move: arrow keys\nA: X    B: Z\nX: S    Y: A\nL: Q    R: W\nStart: Enter\nSelect: Backspace\nTouch screen: the mouse\n\nPause menu: Esc\nFast forward: hold Left Shift\nSave state: F5\nLoad state: F8\nWindow or fullscreen: F11\n\nA controller plays as a DS: hold its right trigger to fast forward, Start opens the pause menu.\n\nChange any key in the pause menu, under Controls, then Keys and buttons."],
    ["Gen1Recomp", "Game Boy, Game Boy Color and GBA games.",
        "Move: arrow keys or W A S D\nA: Z, Enter or Space\nB: X or Backspace\nStart: Esc\nSelect: Tab or Shift\n\nSave: F1\nLoad: F2\nGame speed up, down: 1, 0\nColours: 2\nTilt: 3\nZoom: 4, or - and =, or the mouse wheel\nMods: F10\n\nOn a controller, R2 and L2 change the game speed.\n\nChange any key in the game's Options, under Controls."],
    ["nativeplat", "Diamond, Pearl and Platinum, native.",
        "Move: arrow keys or W A S D\nA: Z, Enter or Space\nB: X or Backspace\nX: C    Y: V\nL: Q    R: E\nStart: Esc\nSelect: Tab or Shift\nTouch screen: the mouse on the lower screen\n\nOptions: F10\nFast forward: hold F, or G to turn it on and off\nSpeed 1x to uncapped: 1\nRewind: hold R\nSave in the game: F1\nBack to the last save: F2 twice\nSnapshot: F5 to take, F6 for the next of 4, F7 to go back\nCamera: - and =, tilt 3 and 4, reset 0\nTouch controls: F9\nFullscreen: F11\nScreenshot: F12\n\nChange any key in Options, under Controls."],
    ["Platinum PC port", "Platinum, native.",
        "Settings and debug tools: the ` key (grave, under Esc).\n\nIts other keys are listed in its own settings."],
    ["3DSrecomp+Zakuro", "X to Ultra Moon, native.",
        "Move: arrow keys\nCircle Pad: I J K L\nA: X    B: Z\nX: S    Y: A\nL: Q    R: W\nZL: 1    ZR: 2\nStart: Enter\nSelect: Backspace\nTouch screen: the mouse on the lower screen\n\nMenu (controls, cheats, graphics): Esc\nPause: F1\nFast forward: hold Tab\nSave state: F5\nLoad state: F8\nScreen layout: F9\nFullscreen: F11\n\nA controller plays by the letters on its buttons; hold its right trigger to fast forward. Change keys and buttons in its menu (Esc), under Controls."],
    ["Infinite Fusion", "Its own program.",
        "Speed up: the A key\nScreenshot: F8\nKeys and controller: F1\n\nWhether A holds or toggles the speed is set in the game's Options."],
];

pub fn controls_page() -> Page {
    let mut p = Page::new("Controls");
    for s in CONTROL_SHEETS { p.items.push(Item::new(s[0], "", true).icon('\u{E765}').info(&format!("{} The keys it answers to while you play.", s[1])).run(move |l| l.show_text(&format!("{} controls", s[0]), s[2]))); }
    p
}
pub fn show_text(title: &str, body: &str) -> Page {
    let mut p = Page::new(title);
    p.body = Some(body.to_string());
    p
}

pub fn sprites_page(c: &Ctx) -> Page {
    let mut p = Page::new("Pokemon FireRed: boxes");
    p.theme = c.current.map(|i| theme_of(&c.lib.games[i]));
    for key in ["25", "shiny/6", "150", "245", "384", "635"] {
        let mut it = Item::new(&format!("Dex {}", key), "", true).icon('\u{E8AB}').info("A sample row.");
        it.sprite = Some(key.to_string());
        p.items.push(it);
    }
    p
}

/// Where each engine's screenshot key writes (ScreenshotFolders). Fulldex and Gen1Recomp have none.
pub fn screenshots_page(c: &Ctx) -> Page {
    let mut p = Page::new("Screenshots");
    let mut list: Vec<(String, PathBuf)> = Vec::new();
    if config::nativeplat().is_file() { list.push(("nativeplat".into(), config::nativeplat().parent().map(|d| d.to_path_buf()).unwrap_or_default().join("userdata").join("screenshots"))); }
    let _ = c;
    for (key, dir) in list {
        let n = std::fs::read_dir(&dir).map(|r| r.flatten().filter(|e| e.path().extension().map_or(false, |x| x.eq_ignore_ascii_case("png"))).count()).unwrap_or(0);
        let k = if key == "nativeplat" { "F12" } else { "F8" };
        let count = if n == 0 { format!("None yet: press {} in the game.", k) } else { format!("{}{}, taken with {}.", n, if n == 1 { " picture" } else { " pictures" }, k) };
        let d = dir.to_string_lossy().into_owned();
        p.items.push(Item::new(&key, "", true).icon('\u{E8B9}').info(&format!("{} Select to open the folder: {}", count, dir.display())).run(move |l| { let _ = std::fs::create_dir_all(&d); l.effects.push(crate::input::Effect::OpenFolder(d.clone())); }));
    }
    p.items.push(Item::new("Other games", "", true).icon('\u{E946}').info("Taking screenshots in Fulldex, Gen1Recomp and the Platinum PC port.").run(|l| l.show_text("Screenshots in other games", "Fulldex and Gen1Recomp have no screenshot key of their own.\n\nWindows can take one of any game, these and the Platinum PC port too: Windows + Shift + S picks an area, and Windows + Alt + Print Screen saves the game's window to Videos\\Captures.")));

    p
}

/// ControlsFor: the controls sheet of an engine, for the guide overlay.
pub fn controls_for(engine: &str) -> String {
    let i = match engine { "fulldex" => 0, "gen1" => 1, "nativeplat" => 2, "port" => 3, "recomp" => 4, "pc" => 5, _ => usize::MAX };
    if i != usize::MAX { CONTROL_SHEETS[i][2].to_string() } else { "A controller works as on Fulldex: each button by the letter on it.\n\nFor Fulldex's keys (arrows, X, Z, S, A, Q, W, Enter, Backspace), pick the profile \"Keyboard (Fulldex keys)\" in the emulator's Controls.".to_string() }
}

/// The emulators' own words (Emu3ds fields the UI shows), by id: upstream, size, recommended, notes.
fn emu_info(id: &str) -> (&'static str, &'static str, bool, &'static str) {
    match id {
        "azaharplus" => ("AzaharPlus/AzaharPlus", "about 73 MB", true, "The one to use: X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon and the Ultra games all run well. Based on Azahar, with extras: it also opens encrypted dumps, and has built-in cheats."),
        _ => ("azahar-emu/azahar", "about 44 MB", false, "The backup: plain Azahar, for when AzaharPlus misbehaves. It opens decrypted dumps only: encrypted ones play with AzaharPlus."),
    }
}
fn emu_update_ready(e: &emu3ds::Emu3ds) -> bool { e.installed() && e.tag.is_some() && e.installed_tag().as_deref() != e.tag }

pub fn emu_page() -> Page {
    let mut p = Page::new("Emulators");
    let on = emu3ds::enabled();
    p.items.push(Item::new(&format!("3DS games: {}", if on { "On" } else { "Off" }), "", true).icon('\u{E7F4}').tint(APP_COLORS[app("Emu")]).info(&format!("Play your Nintendo 3DS Pokemon games with an emulator you choose. Nothing downloads until you pick one, and turning this off keeps your saves. Select to turn it {}.", if on { "off" } else { "on" })).run(move |l| { nm_core::config::write("3ds-games.txt", if on { "off" } else { "on" }); l.rebuild_top(false, |_| emu_page()); l.replace_main(); }));
    if !on {
        p.body = Some("nativeMons can play X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon, Ultra Sun and Ultra Moon through a 3DS emulator, with your saves, collection and Move Pokemon working as they do for the other games.\n\nYou need dumps of your own cartridges. Encrypted ones play with AzaharPlus, decrypted ones with either emulator. No games or keys are included.".into());
        p.fit_body = true;
        return p;
    }
    let chosen = emu3ds::default();
    for e in emu3ds::ALL.iter() {
        let (upstream, _, rec, notes) = emu_info(e.id);
        let state = if e.installed() { if chosen.map_or(false, |c| c.id == e.id) { "Plays your 3DS games" } else { "Installed" } } else { "Not installed" };
        let mut it = Item::new(&format!("{}{}", e.short, if rec { " (recommended)" } else { " (backup)" }), &format!("{}.", state), true).run({ let id = e.id.to_string(); move |l| if let Some(e) = emu3ds::ALL.iter().find(|x| x.id == id) { l.push(emu_detail_page(e)); } }).icon('\u{E7F4}').tint(APP_COLORS[app("Emu")]).info(notes);
        it.tile_hint = state.into();
        it.records = vec![format!("Name: {}", e.name), format!("Based on: github.com/{}", upstream), format!("Built at: github.com/{}", e.repo.unwrap_or(""))];
        p.items.push(it);
    }
    p
}

pub fn emu_detail_page(e: &emu3ds::Emu3ds) -> Page {
    let mut p = Page::new(e.short);
    let (_, size, _, notes) = emu_info(e.id);
    let have = e.installed();
    let is_default = have && emu3ds::default().map_or(false, |d| d.id == e.id);
    p.body = Some(format!("{}\n\n{}", e.name, notes));
    p.fit_body = true;
    let can = emu3ds::can_download(e);
    let ready = emu_update_ready(e);
    let label = if !have { "Install" } else if ready { "Update ready" } else { "Up to date" };
    let hint = if !can { "Not available yet: its nativeMons build is still being made.".to_string() } else if have && !ready { "You have the newest nativeMons build.".to_string() } else { format!("Downloads {} and checks it before putting it in place.", size) };
    p.items.push(Item::new(label, &hint, can && (!have || ready)).icon('\u{E896}').info(&format!("Gets {} from github.com/{}, checks the download, and puts it beside the launcher. Your saves stay where they are.", e.name, e.repo.unwrap_or(""))).run({ let id = e.id; move |l| l.emu_install(id) }));
    let info = if is_default { format!("Your 3DS games play with {}.", e.short) } else if have { format!("Select to play your 3DS games with {} instead.", e.short) } else { "Install it first.".to_string() };
    p.items.push(Item::new(&format!("Use for 3DS games: {}", if is_default { "Yes" } else { "No" }), "", have && !is_default).icon('\u{E7F4}').tint(APP_COLORS[app("Emu")]).info(&info).run({ let id = e.id.to_string(); move |l| { nm_core::config::write("3ds-emulator.txt", &id); if let Some(e) = emu3ds::ALL.iter().find(|x| x.id == id) { l.rebuild_top(false, |_| emu_detail_page(e)); } } }));
    p.items.push(Item::new("Open its folder", "", have).icon('\u{E8B7}').info(&e.dir().to_string_lossy()).run({ let d = e.dir().to_string_lossy().into_owned(); move |l| l.effects.push(crate::input::Effect::OpenFolder(d.clone())) }));
    p.items.push(Item::new("Remove", "Deletes the emulator. Your saves stay.", have).icon('\u{E74D}').run({ let id = e.id; move |l| l.emu_remove(id) }));
    p
}

/// GameOption: one of Game options' settings (the C# GameOptions.All, with what the pages show).
pub struct GameOption {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: char,
    pub values: Vec<String>,
    pub labels: Vec<String>,
    pub game_only: bool,
    pub engine: bool,
    pub first_gen: i32,
    pub note: Option<&'static str>,
}
impl GameOption {
    fn label(&self, v: &str) -> String { self.values.iter().position(|x| x == v).map(|i| self.labels[i].clone()).unwrap_or_else(|| v.to_string()) }
}
fn strs(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
#[allow(clippy::too_many_arguments)]
fn opt(key: &'static str, name: &'static str, icon: char, values: Vec<String>, labels: Vec<String>, game_only: bool, engine: bool, first_gen: i32, note: Option<&'static str>) -> GameOption {
    GameOption { key, name, icon, values, labels, game_only, engine, first_gen, note }
}
pub fn game_options_all() -> Vec<GameOption> {
    let percents: Vec<String> = (0..21).map(|i| (i * 5).to_string()).collect();
    let plabels: Vec<String> = (0..21).map(|i| if i == 0 { "Off".to_string() } else { format!("{}%", i * 5) }).collect();
    let two = || (strs(&["on", "off"]), strs(&["On", "Off"]));
    vec![
        opt("text", "Text speed", '\u{E8D2}', strs(&["slow", "mid", "fast"]), strs(&["Slow", "Medium", "Fast"]), false, false, 1, None),
        opt("scene", "Battle scene", '\u{E8B2}', two().0, two().1, false, false, 1, None),
        opt("style", "Battle style", '\u{E8AB}', strs(&["shift", "set"]), strs(&["Shift", "Set"]), false, false, 1, None),
        opt("sound", "Sound", '\u{E7F5}', strs(&["mono", "stereo"]), strs(&["Mono", "Stereo"]), false, false, 2, None),
        opt("button", "Button mode", '\u{E7FC}', strs(&["normal", "alt", "lequalsa"]), strs(&["Normal", "Alternate (LR or Start = X)", "L = A"]), true, false, 3, None),
        opt("frame", "Text frame", '\u{E8A9}', (1..=20).map(|i| i.to_string()).collect(), (1..=20).map(|i| format!("Type {}", i)).collect(), true, false, 3, None),
        opt("vol", "Volume", '\u{E767}', percents.clone(), plabels.clone(), false, true, 1, Some("Overall volume, on every engine. Fulldex and the GBA player go in steps of 10% and Gen1Recomp has 8 levels, so they use the nearest one.")),
        opt("music", "Music volume", '\u{E8D6}', percents.clone(), plabels.clone(), false, true, 1, Some("On Gen1Recomp and nativeplat. Fulldex, the Platinum PC port, 3DSrecomp+Zakuro and the 3DS emulators only have the overall volume.")),
        opt("sfx", "Sound effects", '\u{E7F6}', percents, plabels, false, true, 1, Some("On Gen1Recomp and nativeplat. Fulldex, the Platinum PC port, 3DSrecomp+Zakuro and the 3DS emulators only have the overall volume.")),
        opt("full", "Fullscreen", '\u{E740}', two().0, two().1, false, true, 1, Some("On every engine.")),
        opt("wide", "Widescreen", '\u{E799}', two().0, two().1, false, true, 1, Some("The world on Fulldex and nativeplat, and battles on Gen1Recomp. The Platinum PC port doesn't have it.")),
        opt("smooth", "Smooth scaling", '\u{E91B}', two().0, two().1, false, true, 1, Some("Softens the picture on Fulldex, nativeplat and 3DSrecomp+Zakuro, instead of sharp pixels.")),
    ]
}
fn options_file(g: Option<&GameEntry>) -> PathBuf { config::state().join(match g { None => "game-options.txt".to_string(), Some(g) => format!("game-options-{}.txt", g.id) }) }
fn options_read(g: Option<&GameEntry>) -> std::collections::HashMap<String, String> {
    let mut d = std::collections::HashMap::new();
    if let Ok(t) = std::fs::read_to_string(options_file(g)) {
        for l in config::lines(&t) {
            if let Some(e) = l.find('=') {
                if e > 0 { d.insert(l[..e].to_string(), l[e + 1..].to_string()); }
            }
        }
    }
    if g.is_none() && !d.contains_key("text") {
        let old = config::read("text-speed.txt", nm_core::game_options::OWN);
        if old != nm_core::game_options::OWN { d.insert("text".into(), old); }
    }
    d
}
fn option_stored(g: Option<&GameEntry>, o: &GameOption) -> String {
    match options_read(g).get(o.key) {
        Some(v) => v.clone(),
        None => if g.is_none() || o.game_only { "own".into() } else { "follow".into() },
    }
}
fn option_describe(g: Option<&GameEntry>, o: &GameOption, v: &str) -> String {
    if v == "follow" { return "Global".into(); }
    if v == "own" { return if g.is_none() { "Each game's own".into() } else { "This game's own".into() }; }
    o.label(v)
}
/// GameOptions.Next: the value after the stored one, in Global, own, values order.
fn option_next(g: Option<&GameEntry>, o: &GameOption) -> String {
    let mut order: Vec<String> = Vec::new();
    if g.is_some() && !o.game_only { order.push("follow".into()); }
    order.push("own".into());
    order.extend(o.values.iter().cloned());
    let at = order.iter().position(|x| *x == option_stored(g, o)).map_or(-1, |i| i as i32);
    order[((at + 1) as usize) % order.len()].clone()
}
/// GameOptions.Store: the file rewritten in its own order (a Dictionary's), the value replaced or added.
fn option_store(g: Option<&GameEntry>, o: &GameOption, value: &str) {
    let mut d: Vec<(String, String)> = Vec::new();
    if let Ok(t) = std::fs::read_to_string(options_file(g)) {
        for l in config::lines(&t) {
            if let Some(e) = l.find('=') {
                if e == 0 { continue; }
                let (k, v) = (l[..e].to_string(), l[e + 1..].to_string());
                match d.iter_mut().find(|x| x.0 == k) { Some(x) => x.1 = v, None => d.push((k, v)) }
            }
        }
    }
    if g.is_none() && !d.iter().any(|x| x.0 == "text") {
        let old = config::read("text-speed.txt", nm_core::game_options::OWN);
        if old != nm_core::game_options::OWN { d.push(("text".into(), old)); }
    }
    match d.iter_mut().find(|x| x.0 == o.key) { Some(x) => x.1 = value.to_string(), None => d.push((o.key.to_string(), value.to_string())) }
    let _ = std::fs::create_dir_all(config::state());
    let _ = config::write_all_lines(&options_file(g), &d.iter().map(|x| format!("{}={}", x.0, x.1)).collect::<Vec<_>>());
}
/// Game options rebuilt in place (its selection and scroll kept), after a change.
fn options_again(l: &mut crate::launcher::Launcher, gid: &Option<String>) {
    let Some(was) = l.pages.pop() else { return };
    let g = gid.as_ref().and_then(|id| l.game_index(id)).map(|i| l.ctx().lib.games[i].clone());
    let mut again = game_options_page(g.as_ref());
    again.sel = was.sel;
    again.top_row = was.top_row;
    again.scroll_row = was.scroll_row;
    l.push(again);
}
fn option_by_key(key: &str) -> Option<GameOption> { game_options_all().into_iter().find(|o| o.key == key) }
fn with_option(l: &mut crate::launcher::Launcher, gid: &Option<String>, key: &str, f: impl FnOnce(&mut crate::launcher::Launcher, Option<&GameEntry>, &GameOption)) {
    let Some(o) = option_by_key(key) else { return };
    let g = gid.as_ref().and_then(|id| l.game_index(id)).map(|i| l.ctx().lib.games[i].clone());
    f(l, g.as_ref(), &o);
}

fn option_has(g: &GameEntry, o: &GameOption) -> bool { o.engine || (nm_core::game_options::available(g) && nm_core::game_options::generation(g) >= o.first_gen) }

pub fn game_options_page(g: Option<&GameEntry>) -> Page {
    let mut p = Page::new(&match g { None => "Game options".to_string(), Some(g) => format!("Options: {}", g.title) });
    if let Some(g) = g { p.theme = Some(theme_of(g)); }
    for o in game_options_all() {
        if match g { None => o.game_only, Some(g) => !option_has(g, &o) } { continue; }
        let pick = o.values.len() > 6;
        let note = o.note.map(|n| format!("{} ", n)).unwrap_or_default();
        if o.values.len() == 21 && o.values[20] == "100" {
            let stored = option_stored(g, &o);
            let shown = if stored == "follow" { option_stored(None, &o) } else { stored.clone() };
            let at = shown.parse::<i32>().ok();
            let own = at.is_none();
            let (gid, key) = (g.map(|g| g.id.clone()), o.key);
            let gid2 = gid.clone();
            let mut it = Item::new(o.name, "", true).icon(o.icon)
                .run(move |l| with_option(l, &gid, key, |l, g, o| l.push(option_pick_page(g, o))))
                .slide(move |l, step| with_option(l, &gid2, key, |l, g, o| {
                    let shown = { let s = option_stored(g, o); if s == "follow" { option_stored(None, o) } else { s } };
                    let from = shown.parse::<i32>().unwrap_or(100);
                    option_store(g, o, &(from + step).clamp(0, 100).to_string());
                    options_again(l, &g.map(|g| g.id.clone()));
                }));
            it.is_slider = true;
            it.slider = at.unwrap_or(-1);
            it.slider_note = if stored == "follow" { format!("Global{}", if own { ": the engine's own" } else { "" }) } else if own { if g.is_none() { "Each game's own".into() } else { "This game's own".into() } } else { String::new() };
            it.info = format!("{}Left and right change it by 5. Press for Global or the engine's own.", note);
            p.items.push(it);
            continue;
        }
        let tail = if g.is_none() { format!("Applies to every game when it starts, unless the game's own page says otherwise.{}", if o.engine { "" } else { " It replaces what's set in the game. Not on Black, White, Black 2 and White 2 yet." }) } else { "Applies when this game starts. Global follows Game options in Settings.".to_string() };
        let (gid, key) = (g.map(|g| g.id.clone()), o.key);
        p.items.push(Item::new(&format!("{}: {}", o.name, option_describe(g, &o, &option_stored(g, &o))), if pick { "Press to choose." } else { "Press to change." }, true).icon(o.icon).info(&format!("{}{}", note, tail))
            .run(move |l| with_option(l, &gid, key, |l, g, o| {
                if pick { l.push(option_pick_page(g, o)); return; }
                option_store(g, o, &option_next(g, o));
                options_again(l, &g.map(|g| g.id.clone()));
            })));
    }
    p
}

pub fn option_pick_page(g: Option<&GameEntry>, o: &GameOption) -> Page {
    let mut p = Page::new(o.name);
    p.theme = g.map(theme_of);
    let now = option_stored(g, o);
    let mut choices: Vec<String> = Vec::new();
    if g.is_some() && !o.game_only { choices.push("follow".into()); }
    choices.push("own".into());
    choices.extend(o.values.iter().cloned());
    for c in choices {
        let info = if c == "follow" { format!("Uses what's set in Game options in Settings: {}.", option_describe(None, o, &option_stored(None, o))) } else if c == "own" { "Leaves it to the engine's own setting.".to_string() } else { o.note.unwrap_or("").to_string() };
        let (gid, key, value) = (g.map(|g| g.id.clone()), o.key, c.clone());
        p.items.push(Item::new(&option_describe(g, o, &c), if c == now { "Chosen now." } else { "" }, true).icon(o.icon).info(&info)
            .run(move |l| with_option(l, &gid, key, |l, g, o| {
                option_store(g, o, &value);
                l.pages.pop();
                options_again(l, &g.map(|g| g.id.clone()));
            })));

        if c == now { p.sel = p.items.len() as i32 - 1; }
    }
    p
}

pub fn folder_added_page(c: &Ctx, folder: &str) -> Page {
    let games = &c.lib.games;
    let have = games.iter().filter(|g| g.playable()).count();
    let roms = games.iter().filter(|g| g.fulldex_rom.is_some() || g.found_unsupported.is_some()).count();
    let mut p = Page::new("Folder added");
    let finish = Item::new(if c.first_setup { "Complete setup" } else { "Done" }, "Go to the main screen.", true).icon('\u{E73E}');
    let another = Item::new(if have > 0 { "Add another folder" } else { "Try another folder" }, "Choose a folder.", true).icon('\u{E710}');
    if have > 0 {
        p.body = Some(format!("{}\n\nFound {} of {} games.\n\nYou're ready to play. If you keep some games in another folder, add that one too.", folder, have, games.len()));
        p.items.push(finish);
        p.items.push(another);
    } else {
        p.body = Some(format!("{}{}", folder, if roms > 0 { format!("\n\nFound ROMs for {}{}, but none can start. Games shows the reason for each one.", roms, if roms == 1 { " game" } else { " games" }) } else { "\n\nNo games found. nativeMons looks through this folder and every folder inside it for .gb, .gbc, .gba and .nds ROMs.".to_string() }));
        p.items.push(another);
        p.items.push(finish);
    }
    p
}

fn shares(g: &GameEntry) -> bool { g.has_both() && mon_bridge() }

pub fn engine_page(g: &GameEntry) -> Page {
    let engine = g.engine();
    let engines = g.engines();
    let share = shares(g) || g.port.is_some() || g.nativeplat.is_some();
    let mut p = Page::new("Engine");
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some(format!("Choose what plays {}.{}", g.title, if share { " They all share one save." } else { "" }));
    for e in &engines {
        let inuse = *e == engine;
        p.items.push(Item::new(&format!("{}{}", engine_short(e, Some(g)), if inuse { ": In use" } else { "" }), "", true).icon('\u{E72C}')
            .info(&format!("{}.{}", engine_name(e, Some(g)), if inuse { " In use now.".to_string() } else { format!(" Select to play {} with it.", g.title) }))
            .run({ let (id, e) = (g.id.clone(), e.to_string()); move |l| {
                config::write(&format!("engine-{}.txt", id), &e);
                l.pages.pop();
                let id2 = id.clone();
                l.rebuild_top(true, move |c| match c.lib.games.iter().find(|x| x.id == id2) { Some(g) => game_page(c, g), None => Page::new("") });
                l.replace_main();
            } }));
    }
    p.sel = engines.iter().position(|e| *e == engine).unwrap_or(0) as i32;
    p
}

pub fn recomp_ask_page(games: &[&GameEntry], play: Option<&GameEntry>) -> Page {
    use nm_core::recomp;
    let (cores, one) = (recomp::cores(), games.len() == 1);
    let min = recomp::minutes(cores);
    let mut p = Page::new(&if one { format!("Build {}?", games[0].title) } else { format!("Build {} games?", games.len()) });
    p.theme = Some(theme_of(games[0]));
    p.fit_body = true;
    let names = games.iter().map(|g| g.title.as_str()).collect::<Vec<_>>().join(", ");
    p.body = Some(format!("{}{} natively with 3DSrecomp+Zakuro after a one-time build of {} code, made on this PC from your own dump. The build never leaves this PC.\n\nAbout {} minutes {}here ({} in Settings), at the lowest priority, so you can keep using your PC.{}",
        names, if one { " plays" } else { " play" }, if one { "its" } else { "their" }, min, if one { "" } else { "per game " }, if recomp::fast() { "Fast" } else { "Quiet" },
        if recomp::zig_exe().is_file() { "" } else { " The first build downloads Zig, the C compiler it uses (about 100 MB)." }));
    let ids: Vec<String> = games.iter().map(|g| g.id.clone()).collect();
    let play_id = play.map(|g| g.id.clone());
    p.items.push(Item::new(if play.is_some() { "Build, then play" } else { "Build now" }, "", true).icon('\u{E896}').info("Esc or B stops a build; it starts over next time.")
        .run(move |l| { l.pages.pop(); l.build_recomp(ids.clone(), play_id.clone()); }));
    if let Some(pl) = play.filter(|g| g.on_3ds()) {
        let id = pl.id.clone();
        p.items.push(Item::new(&format!("Play with {} this time", engine_short("3ds", Some(pl))), "", true).icon('\u{E768}').info("The emulator plays it now; the build can wait.")
            .run(move |l| { l.pages.pop(); if let Some(g) = l.game(&id) { l.play_3ds(&g); } }));
    }
    p.items.push(Item::new("Not now", "", true).icon('\u{E711}').info("Build it later from the game's page.").run(|l| { l.pages.pop(); }));
    p
}

/// Wallpaper: the player's choice and own pictures (state\wallpaper.txt, state\wallpapers).
pub mod wallpaper {
    use nm_core::config;
    use std::path::PathBuf;
    const KINDS: [&str; 5] = [".png", ".jpg", ".jpeg", ".bmp", ".gif"];
    fn dir() -> PathBuf { config::state().join("wallpapers") }
    pub fn chosen() -> bool { !config::read("wallpaper.txt", "").is_empty() }
    pub fn drawn() -> bool { config::read("wallpaper.txt", "") == "drawn" }
    pub fn own(id: &str) -> Option<PathBuf> {
        if !dir().is_dir() { return None; }
        KINDS.iter().map(|k| dir().join(format!("{}{}", id, k))).find(|f| f.is_file())
    }
    pub fn credit(id: &str) -> Option<String> { std::fs::read_to_string(dir().join(format!("{}.credit.txt", id))).ok().map(|s| s.trim().to_string()) }
    pub fn region_of(id: &str) -> Option<&'static str> {
        Some(match id {
            "red" | "blue" | "yellow" | "firered" | "leafgreen" => "Kanto",
            "gold" | "silver" | "crystal" | "heartgold" | "soulsilver" => "Johto",
            "ruby" | "sapphire" | "emerald" | "omegaruby" | "alphasapphire" => "Hoenn",
            "diamond" | "pearl" | "platinum" => "Sinnoh",
            "black" | "white" | "black2" | "white2" => "Unova",
            "x" | "y" => "Kalos",
            "sun" | "moon" | "ultrasun" | "ultramoon" => "Alola",
            _ => return None,
        })
    }
}

pub fn wallpaper_page(c: &Ctx, asking: bool) -> Page {
    let mut p = Page::new("Wallpapers");
    p.fit_body = true;
    p.body = Some(format!("The home screen can show a scene drawn for each game, or pictures from your own play: the towns captured from your games, and snapshots taken while you play 3DS games. A game with no pictures yet shows its scene.{}", if asking { "\n\nYou can change this, or give any game a picture of your own, in Settings." } else { "" }));
    let (chosen, drawn) = (wallpaper::chosen(), wallpaper::drawn());
    p.items.push(Item::new(&format!("Drawn scenes{}", if chosen && drawn { " (in use)" } else { "" }), "", true).icon('\u{E790}').info("Each game's own drawn scene, in its colors. No snapshots are taken while you play.").run(move |l| { nm_core::config::write("wallpaper.txt", if true { "drawn" } else { "play" }); if asking { l.pages.pop(); } else { l.rebuild_top(true, |c| wallpaper_page(c, false)); } l.dirty = true; }));
    p.items.push(Item::new(&format!("Pictures from your games{}", if chosen && !drawn { " (in use)" } else { "" }), "", true).icon('\u{E91B}').info("Towns captured from your games, and pictures of the top screen taken while you play a 3DS game. They stay on your PC. Games without any show their drawn scene.").run(move |l| { nm_core::config::write("wallpaper.txt", if false { "drawn" } else { "play" }); if asking { l.pages.pop(); } else { l.rebuild_top(true, |c| wallpaper_page(c, false)); } l.dirty = true; }));
    if !asking {
        let own = c.lib.games.iter().filter(|g| wallpaper::own(&g.id).is_some()).count();
        let hint = if own == 0 { "None yet.".to_string() } else { format!("{}{}", own, if own == 1 { " game has one." } else { " games have one." }) };
        p.items.push(Item::new("Your own pictures", &hint, true).icon('\u{E8B9}').info("Pick any picture on your PC for a game. It shows behind the home screen whenever that game is in front, instead of the choice above.").run(|l| l.push_with(own_wallpapers_page)));
    }
    if !asking { p.on_back = Some(crate::model::Act::new(|l| l.back_and_rebuild(settings_page))); }
    p
}

pub fn own_wallpapers_page(c: &Ctx) -> Page {
    let mut p = Page::new("Your own pictures");
    for g in &c.lib.games {
        let has = wallpaper::own(&g.id).is_some();
        let id = g.id.clone();
        let mut it = Item::new(&g.title, if has { "Your picture." } else { "" }, true).run(move |l| l.push_game(&id, |_, g| own_wallpaper_page(g)));
        it.theme = Some(theme_of(g));
        it.tile_hint = if has { "Your picture".into() } else { String::new() };
        it.info = if has { "Shows your own picture. Select to change or remove it.".into() } else { "Shows the wallpaper chosen under Wallpapers. Select to pick a picture of your own.".into() };
        p.items.push(it);
    }
    p.on_back = Some(crate::model::Act::new(|l| l.back_and_rebuild(|c| wallpaper_page(c, false))));
    p
}

pub fn own_wallpaper_page(g: &GameEntry) -> Page {
    let has = wallpaper::own(&g.id).is_some();
    let mut p = Page::new(&g.title);
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    let credit = if has { wallpaper::credit(&g.id) } else { None };
    p.body = Some(if has { format!("Your picture shows behind the home screen whenever {} is in front.{}", g.title, credit.map(|c| format!("\n\n{}", c)).unwrap_or_default()) } else { format!("Pick a picture on your PC, or find one online, to show behind the home screen whenever {} is in front. nativeMons keeps its own copy, so the original can move or go.", g.title) });
    p.items.push(Item::new("Find a wallpaper", "", true).icon('\u{E721}').info("Search fan art and free-license art online. Only what you pick is downloaded.").run({ let id = g.id.clone(); move |l| l.push_game(&id, |_, g| find_wallpaper_page(g)) }));
    p.items.push(Item::new(if has { "Choose another picture" } else { "Choose a picture" }, "", true).icon('\u{E8B9}').info("PNG, JPG, BMP or GIF. It's cropped to fill the screen.").run({ let id = g.id.clone(); move |l| l.browse(BrowseRequest { title: "Choose a picture".into(), kinds: vec![".png", ".jpg", ".jpeg", ".bmp", ".gif"], hint: "Open the folder with your picture and select it. PNG, JPG, BMP and GIF work.".into(), done: format!("picture:{}", id), ..Default::default() }) }));
    p.items.push(Item::new("Remove your picture", "", has).icon('\u{E74D}').info(if has { "Goes back to the wallpaper chosen under Wallpapers. Only nativeMons' copy is deleted." } else { "This game has no picture of your own." }).run({ let id = g.id.clone(); move |l| l.wallpaper_remove(&id) }));
    p.on_back = Some(crate::model::Act::new(|l| l.back_and_rebuild(own_wallpapers_page)));
    p
}

pub fn find_wallpaper_page(g: &GameEntry) -> Page {
    const FAN: &str = "wallhaven";
    let mut p = Page::new("Find a wallpaper");
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    p.body = Some("Fan art comes from wallhaven.cc, safe-for-work pictures only. Free-license art comes from Openverse, with its artist and license kept beside it. Pictures are only downloaded when you pick one, and the artists' rights stay theirs.".into());
    let name = g.title.replace("Pokemon ", "");
    let mut searches = vec![(FAN, format!("pokemon {}", name.to_lowercase()), format!("Fan art: {}", name))];
    if let Some(region) = wallpaper::region_of(&g.id) { searches.push((FAN, format!("pokemon {}", region.to_lowercase()), format!("Fan art: {}", region))); }
    searches.push((FAN, "pokemon".into(), "Fan art: all Pokemon".into()));
    searches.push(("openverse", format!("pokemon {}", name.to_lowercase()), format!("Free-license art: {}", name)));
    searches.push(("openverse", "pokemon".into(), "Free-license art: all Pokemon".into()));
    for (src, q, label) in searches {
        let info = if src == FAN { format!("Searches wallhaven.cc for \"{}\".", q) } else { format!("Searches Openverse for \"{}\". Every result there carries a Creative Commons license or is public domain.", q) };
        let (id, src, q) = (g.id.clone(), src.to_string(), q.clone());
        p.items.push(Item::new(&label, "", true).icon('\u{E721}').info(&info).run(move |l| l.run_wall_search(&id, &src, &q, 1, None)));
    }
    p
}

pub fn emu_update_ask_page(e: &emu3ds::Emu3ds, g: &GameEntry, quiet: bool) -> Page {
    let mut p = Page::new(&format!("Update {}?", e.short));
    p.theme = Some(theme_of(g));
    p.fit_body = true;
    let size = emu_info(e.id).1;
    let tag = e.tag.unwrap_or("");
    p.body = Some(format!("A newer nativeMons build of {} is ready: {} (you have {}). It downloads {}. Your saves aren't touched either way.", e.short, tag, e.installed_tag().unwrap_or_else(|| "an older one".into()), size));
    p.items.push(Item::new("Update, then play", "", true).icon('\u{E896}').info(&format!("Gets the new build, checks it, puts it in place, then starts {}.", g.title)));
    p.items.push(Item::new("Play without updating", "", true).icon('\u{E768}').info(if quiet { "You won't be asked about this update again. A later one asks again." } else { "You'll be asked again next time." }));
    p.items.push(Item::new(&format!("Do not remind me of this update: {}", if quiet { "On" } else { "Off" }), "", true).icon('\u{E708}').info(&format!("Turn on, then choose Play without updating, to stop being asked about {}. You can still update from Emu at any time.", tag)));
    p
}

/// BrowseRequest: what the launcher's own browser is choosing.
#[derive(Clone, Debug, Default)]
pub struct BrowseRequest {
    pub title: String,
    pub hint: String,
    pub folders: bool,
    pub multi: bool,
    pub kinds: Vec<&'static str>,
    pub picked: Vec<String>,
    /// Which pages are this request's (Page.Browse == req), and what Done does ("addfolder", "addrom", "picture:<id>").
    pub id: u32,
    pub done: String,
}
pub const ROM_KINDS: [&str; 7] = [".gb", ".gbc", ".gba", ".nds", ".3ds", ".cci", ".zip"];

fn folder_name(path: &str) -> String {
    let n = file_name(path.trim_end_matches('\\'));
    if n.is_empty() { path.to_string() } else { n }
}
fn ext_lower(f: &str) -> String { std::path::Path::new(f).extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default() }

/// BrowsePage(req, dir).
pub fn browse_page(req: &BrowseRequest, dir: Option<&str>) -> Page {
    use crate::art::*;
    let mut p = Page::new(&match dir { None => req.title.clone(), Some(d) => folder_name(d) });
    p.fit_body = true;
    p.browse = req.id;
    let open = |to: String| { let r = req.clone(); move |l: &mut crate::launcher::Launcher| l.push(browse_page(&r, Some(&to))) };
    let Some(dir) = dir else {
        p.body = Some(req.hint.clone());
        for (name, path) in nm_core::places::known_folders() {
            if path.is_empty() || !std::path::Path::new(&path).is_dir() { continue; }
            let icon = match name { "Desktop" => ICO_DESKTOP, "Documents" => ICO_DOCS, "Downloads" => ICO_DOWN, _ => ICO_PICS };
            let mut it = Item::new(name, "", true).icon(icon).info(&path).run(open(path.clone()));
            it.tile_hint = path.clone();
            p.items.push(it);
        }
        for d in nm_core::places::drives() {
            const GB: u64 = 1073741824;
            let free = d.space.map(|(f, t)| format!("{} GB free of {} GB", f / GB, t / GB)).unwrap_or_default();
            let label = if d.label.is_empty() { "Local disk".to_string() } else { d.label.clone() };
            let mut it = Item::new(&format!("{} ({})", label, d.name.trim_end_matches('\\')), "", true).icon(if d.removable { ICO_USB } else { ICO_DRIVE }).run(open(d.name.clone()));
            it.info = format!("{}{}", d.name, if free.is_empty() { String::new() } else { format!("\n\n{}", free) });
            it.tile_hint = free;
            p.items.push(it);
        }
        return p;
    };
    let read = || -> std::io::Result<(Vec<String>, Vec<String>)> {
        let (mut subs, mut files) = (Vec::new(), Vec::new());
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let path = e.path().to_string_lossy().into_owned();
            let md = e.metadata()?;
            if md.is_dir() {
                #[cfg(windows)]
                { use std::os::windows::fs::MetadataExt; if md.file_attributes() & 0x6 != 0 { continue; } }
                subs.push(path);
            } else if req.kinds.contains(&ext_lower(&path).as_str()) {
                files.push(path);
            }
        }
        let ci = |a: &String, b: &String| a.to_uppercase().cmp(&b.to_uppercase());
        subs.sort_by(ci);
        files.sort_by(ci);
        Ok((subs, files))
    };
    let (subs, files) = match read() {
        Ok(v) => v,
        Err(e) => { p.body = Some(format!("{}\n\nCan't look inside: {}", dir, e)); return p; }
    };
    p.body = Some(dir.to_string());
    if req.folders {
        let (r, at) = (req.clone(), dir.to_string());
        let mut it = Item::new("Use this folder", "", true).icon(ICO_USE).info(&format!("Every folder inside {} is searched too.", folder_name(dir))).run(move |l| l.browse_finish(&r, vec![at.clone()], &at));
        it.tile_hint = if files.is_empty() { "No ROMs right here; folders inside are searched too.".into() } else { format!("{}{}, and folders inside are searched too.", files.len(), if files.len() == 1 { " ROM here" } else { " ROMs here" }) };
        p.items.push(it);
    }
    if req.multi && !req.picked.is_empty() {
        let n = req.picked.len();
        let (r, at) = (req.clone(), dir.to_string());
        p.items.push(Item::new(&format!("Add {}{}", n, if n == 1 { " file" } else { " files" }), "", true).icon(ICO_ADD).info(&req.picked.iter().map(|f| file_name(f)).collect::<Vec<_>>().join("\n")).run(move |l| l.browse_finish(&r, r.picked.clone(), &at)));
    }
    for s in &subs { p.items.push(Item::new(&folder_name(s), "", true).icon(ICO_FOLDER).info(s).run(open(s.clone()))); }
    for f in &files {
        let picked = req.picked.iter().any(|x| x.eq_ignore_ascii_case(f));
        let size = std::fs::metadata(f).map(|m| format!("{} KB", m.len() / 1024 + 1)).unwrap_or_default();
        let (r, at, file) = (req.clone(), dir.to_string(), f.clone());
        let mut it = Item::new(&format!("{}{}", file_name(f), if picked { ": Selected" } else { "" }), "", !req.folders).icon(file_icon(f)).run(move |l| {
            if !r.multi { l.browse_finish(&r, vec![file.clone()], &at); return; }
            let mut r2 = r.clone();
            match r2.picked.iter().position(|x| x == &file) { Some(k) => { r2.picked.remove(k); } None => r2.picked.push(file.clone()) }
            l.rebuild_top(true, |_| browse_page(&r2, Some(&at)));
        });

        it.tile_hint = if req.folders { String::new() } else { size.clone() };
        it.info = if req.folders { format!("{}\n\nPick the folder to use every ROM in it.", f) } else { format!("{}{}", f, if size.is_empty() { String::new() } else { format!("\n\n{}", size) }) };
        p.items.push(it);
    }
    if subs.is_empty() && files.is_empty() { p.body = Some(format!("{}\n\nNothing here to pick.", dir)); }
    if p.items.len() > 40 { p.jump = 10; }
    p
}

/// The import snapshot's page: Gen1Recomp slot files (saves\<version>\slot<n>.lua) found in the save folders.
pub fn import_page(c: &Ctx) -> Page {
    let mut p = Page::new("Import saves");
    for one in crate::import_saves::find_slots(&c.lib.games) { p.items.push(crate::import_saves::save_row(&c.lib.games, &one)); }
    p
}
