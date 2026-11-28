//! GameArt's town captures, line by line from the C# (CaptureTown, CaptureTownOpening, CaptureTownNativeplat,
//! RunNativeplatTest, SaveTown, TrimDark, SaveStops): the home screen's wallpaper and, for Gen1Recomp and DS games,
//! every other town. Always a new game under a throwaway identity or test mode: never the player's save.

use crate::capture::{art_root, folder, reset_dir, run_engine, save_jpeg, town_file, ART_IDENTITY};
use nm_core::art_jobs;
use nm_core::config;
use nm_core::game_entry::GameEntry;
use nm_core::proc::StartInfo;
use nm_draw::Bitmap;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const NL: &str = if cfg!(windows) { "\r\n" } else { "\n" };
const DP_NEW_GAME: &str = "1250:start;1350:a:4:40:35;2800:down;2820:down;2840:a;2900:a:4:40:14;3560:tap:128:100:8:40:3;3700:a:4:40:49;5850:tap:82:118:6;6600:b:4:40:6;6950:down;6980:a;7040:a:4:40:40;9300:a:4:40:120";
const HG_NEW_GAME: &str = "1300:start;1700:start;2100:a:4:40:10;3100:tap:220:170:4:80:30;6100:tap:128:72:4;6500:a:4;6800:tap:128:147:4;7200:tap:220:170:4:80:50;11500:tap:70:140:4;11900:tap:220:180:4:80:3;12500:tap:60:80:4;12800:tap:220:180:4;13200:tap:190:55:4;13500:tap:220:180:4:80:3;14000:start:4;14100:a:4;14500:tap:220:180:4;14800:tap:190:55:4;15100:tap:220:180:4:80:20";
const TWINLEAF: &str = "411";
const TWINLEAF_X: &str = "110";
const TWINLEAF_Z: &str = "888";
// Every other town: name, map header, world tile x, z (the decomps' map matrices and fly points).
const DP_TOWNS: [&str; 14] = [
    "SANDGEM_TOWN 418 176 848", "FLOAROMA_TOWN 426 176 640", "SOLACEON_TOWN 433 576 656", "CELESTIC_TOWN 442 465 528",
    "JUBILIFE_CITY 3 184 776", "CANALAVE_CITY 33 51 736", "OREBURGH_CITY 45 293 762", "ETERNA_CITY 65 315 538",
    "HEARTHOME_CITY 86 480 704", "PASTORIA_CITY 120 608 832", "VEILSTONE_CITY 132 704 608", "SUNYSHORE_CITY 150 860 785",
    "SNOWPOINT_CITY 165 368 224", "FIGHT_AREA 188 640 432"];
// The League comes out white (its weather); Platinum's Battle Zone never loads on a new game.
fn pt_towns() -> &'static [&'static str] { &DP_TOWNS[..13] }
const HG_TOWNS: [&str; 22] = [
    "CHERRYGROVE_CITY 67 547 400", "VIOLET_CITY 73 479 258", "AZALEA_TOWN 74 419 469", "GOLDENROD_CITY 76 342 350",
    "ECRUTEAK_CITY 78 397 184", "OLIVINE_CITY 77 312 264", "CIANWOOD_CITY 75 187 370", "MAHOGANY_TOWN 87 528 175",
    "LAKE_OF_RAGE 88 536 90", "BLACKTHORN_CITY 89 674 177", "MT_SILVER 90 820 266", "PALLET_TOWN 49 1044 364",
    "VIRIDIAN_CITY 50 1032 263", "PEWTER_CITY 51 1060 98", "CERULEAN_CITY 52 1336 136", "LAVENDER_TOWN 53 1419 243",
    "VERMILION_CITY 54 1310 319", "CELADON_CITY 55 1218 250", "FUCHSIA_CITY 56 1215 404", "SAFFRON_CITY 59 1294 243",
    "CINNABAR_ISLAND 57 1039 503", "INDIGO_PLATEAU 58 912 201"];

/// A rectangle as x, y, width, height.
type Rect = (i32, i32, i32, i32);
/// A part of the picture as fractions: x, y, width, height.
type Part = (f32, f32, f32, f32);

pub fn towns_dir(id: &str) -> PathBuf { folder(id).join("towns") }
pub fn towns_by_gen1(g: &GameEntry) -> bool { g.pc.is_none() && g.town_frame <= 0 && !g.is_ds() && g.gen1.is_some() }
pub fn towns_by_nativeplat(g: &GameEntry) -> bool { g.pc.is_none() && g.town_frame <= 0 && g.is_ds() && (g.nativeplat.is_some() || g.town_core.is_some()) }
/// File.GetLastWriteTimeUtc's ticks: a missing file reads as 1601-01-01.
fn write_ticks(f: &Path) -> i64 { if f.exists() { nm_core::library::last_write_ticks(f) } else { 504_911_232_000_000_000 } }
/// A DS set is made once per engine build: an older build that can't walk the list is tried again after an update.
fn engine_stamp(g: &GameEntry) -> String { if towns_by_nativeplat(g) { write_ticks(&config::nativeplat()).to_string() } else { String::new() } }
pub fn has_towns(g: &GameEntry) -> bool {
    let done = towns_dir(&g.id).join("done.txt");
    if !done.is_file() { return false; }
    let stamp = engine_stamp(g);
    stamp.is_empty() || std::fs::read_to_string(&done).map_or(true, |t| t.contains(&format!("engine={}", stamp)))
}
/// CanCaptureTown: a game nativeplat shows but doesn't run for the player names its core as townCore.
pub fn can_capture_town(g: &GameEntry) -> bool {
    if g.pc.is_some() { return g.on_pc() && !nm_core::pc_games::is_custom(g); }
    let clean = || !nm_core::library::needs_patch(g.fulldex_rom.as_deref());
    (g.town_frame > 0 && g.fulldex_rom.is_some() && config::fulldex().is_file() && clean())
        || g.on_nativeplat()
        || (g.town_core.is_some() && g.fulldex_rom.is_some() && config::nativeplat().is_file() && clean())
        || (g.gen1.is_some() && g.gen1_rom.is_some())
}

/// TrimDark: the largest rectangle of the picture with no black in it, on a grid of 8-pixel cells.
pub fn trim_dark(pic: &Bitmap, a: Rect) -> Rect {
    const CELL: i32 = 8;
    let (cols, rows) = ((a.2 / CELL) as usize, a.3 / CELL);
    let mut heights = vec![0i32; cols];
    let (mut best_area, mut best) = (0i32, a);
    for r in 0..rows {
        for (c, h) in heights.iter_mut().enumerate() {
            let mut d = 0;
            for dy in (1..CELL).step_by(3) { for dx in (1..CELL).step_by(3) {
                let p = pic.data[((a.1 + r * CELL + dy) * pic.width + a.0 + c as i32 * CELL + dx) as usize];
                if ((p >> 16) & 255) + ((p >> 8) & 255) + (p & 255) < 36 { d += 1; }
            } }
            *h = if d >= 5 { 0 } else { *h + 1 };
        }
        // Largest rectangle under this row's histogram.
        let mut stack: Vec<usize> = Vec::new();
        for c in 0..=cols {
            let h = if c == cols { 0 } else { heights[c] };
            while let Some(&top) = stack.last() {
                if heights[top] < h { break; }
                stack.pop();
                let height = heights[top];
                let left = stack.last().map_or(0, |&s| s + 1);
                let width = (c - left) as i32;
                if height * width > best_area { best_area = height * width; best = (a.0 + left as i32 * CELL, a.1 + (r - height + 1) * CELL, width * CELL, height * CELL); }
            }
            stack.push(c);
        }
    }
    best
}

/// SaveTown: whatever surrounds the picture is cut away (the window's edge, or the unloaded dark beyond a map).
fn save_town(g: &GameEntry, shot: &Path, part: Option<Part>, to: Option<&Path>) -> Result<Option<String>, String> {
    std::fs::create_dir_all(folder(&g.id)).map_err(|e| e.to_string())?;
    let pic = Bitmap::load_any(shot).map_err(|e| e.to_string())?;
    let (mut left, mut top, mut right, mut bottom) = (pic.width, pic.height, 0, 0);
    for y in (0..pic.height).step_by(2) { for x in (0..pic.width).step_by(2) {
        if crate::capture::lit(&pic, x, y) { left = left.min(x); top = top.min(y); right = right.max(x); bottom = bottom.max(y); }
    } }
    if left >= right || top >= bottom { return Ok(Some("The town came out blank.".into())); }
    let (r_, b_) = (pic.width.min(right + 2), pic.height.min(bottom + 2));
    let mut area = (left, top, r_ - left, b_ - top);
    if let Some(p) = part {
        let (w, h) = (pic.width as f32, pic.height as f32);
        area = ((w * p.0) as i32, (h * p.1) as i32, (w * p.2) as i32, (h * p.3) as i32);
    }
    // A DS town's view reaches past the map blocks the game has loaded: its mostly black edges are cut away.
    if part.is_some() && to.is_some() { area = trim_dark(&pic, area); }
    if area.2 < 200 || area.3 < 160 { return Ok(Some("The town came out mostly blank.".into())); }
    let dest = to.map(Path::to_path_buf).unwrap_or_else(|| town_file(&g.id));
    save_jpeg(&pic.crop(area.0, area.1, area.2, area.3), &dest, 90)?;
    Ok(None)
}

/// CaptureTown: None when done, or why not. Always under the throwaway identity: the new game never meets a real save.
pub fn capture_town(g: &GameEntry) -> Option<String> {
    if g.pc.is_some() { return crate::capture::capture_pc_town(g); }
    if g.town_frame > 0 { return capture_town_opening(g); }
    if g.is_ds() { return capture_town_nativeplat(g); }
    let shot = art_root().join(format!("{}-town.png", g.id));
    let many = art_root().join(format!("{}-towns", g.id));
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        std::fs::create_dir_all(art_root()).map_err(io)?;
        if shot.is_file() { std::fs::remove_file(&shot).map_err(io)?; }
        let mut import = nm_core::engines::gen1_start("");
        nm_core::engines::use_art_identity(&mut import, ART_IDENTITY);
        import.set_env("POKEPORT_IMPORT_ONLY", "1");
        import.set_env("POKEPORT_FORCE_IMPORT", "1");
        let code = {
            // The C#'s PlainPath(null) throws ArgumentNullException.
            let Some(file) = g.gen1_rom.as_deref() else { return Err("Value cannot be null.".into()) };
            let rom = nm_core::engines::PlainPath::new(file).map_err(io)?;
            import.set_env("POKEPORT_IMPORT_ROM", &rom.path);
            nm_core::engines::run_capture(&import, 240).1
        };
        if code != 0 { return Ok(Some("Could not prepare the game for its picture.".into())); }
        let mut psi = nm_core::engines::gen1_start("");
        nm_core::engines::use_art_identity(&mut psi, ART_IDENTITY);
        psi.set_env("POKEPORT_VERSION", g.gen1.as_deref().unwrap_or(""));
        psi.set_env("POKEPORT_BACKGROUND", "1");
        psi.set_env("POKEPORT_DRIVER", "src/nativemons/TownArt.lua");
        // Every town and city, one picture each, the home town first (TownArt.lua prints them in order).
        reset_dir(&many).map_err(io)?;
        psi.set_env("NATIVEMONS_TOWN_ART", &many.to_string_lossy().replace('\\', "/"));
        let (said, _) = nm_core::engines::run_capture(&psi, 1500);
        let names: Vec<String> = said.split('\n').map(str::trim).filter(|l| l.starts_with("town art: ")).map(|l| l[10..].replace("MAP_", "")).collect();
        if names.is_empty() || !many.join(format!("{}.png", names[0])).is_file() { return Ok(Some(format!("No picture came out. {}", said))); }
        if let Some(why) = save_town(g, &many.join(format!("{}.png", names[0])), None, None)? { return Ok(Some(why)); }
        let towns = towns_dir(&g.id);
        reset_dir(&towns).map_err(io)?;
        for n in &names[1..] {
            let png = many.join(format!("{}.png", n));
            if png.is_file() { save_town(g, &png, None, Some(&towns.join(format!("{}.jpg", n))))?; }
        }
        std::fs::write(towns.join("done.txt"), names.join(NL)).map_err(io)?;
        Ok(None)
    })();
    let _ = std::fs::remove_file(&shot);
    if many.is_dir() { let _ = std::fs::remove_dir_all(&many); }
    nm_core::engines::remove_art_data(ART_IDENTITY);
    r.unwrap_or_else(Some)
}

/// CaptureTownOpening: generation 5's opening flies over the home town, at a frame set in games.json.
fn capture_town_opening(g: &GameEntry) -> Option<String> {
    let temp = art_root().join(format!("{}-town", g.id));
    let script = PathBuf::from(format!("{}.txt", temp.display()));
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        reset_dir(&temp).map_err(io)?;
        let presses: String = (1500..g.town_frame).step_by(60).map(|f| format!("{} {} A{}", f, f + 6, NL)).collect();
        std::fs::write(&script, presses).map_err(io)?;
        let dir = config::fulldex_dir();
        let args = format!("\"{}\" --no-save --config \"{}\" --jit --headless --compose 960x720 --input \"{}\" --shot-dir \"{}\" --shot-at {} --frames {} --funcs \"{}\"",
            g.fulldex_rom.as_deref().unwrap_or(""), dir.join("settings.cfg").display(), script.display(), temp.display(), g.town_frame, g.town_frame + 10,
            dir.join("data").join("funcs").join(format!("{}.txt", g.fulldex_code.as_deref().unwrap_or(""))).display());
        let mut psi = StartInfo::new(config::fulldex(), &args);
        psi.cwd = Some(dir);
        if let Some(why) = run_engine(&psi, &temp, 240) { return Ok(Some(why)); }
        match crate::capture::files(&temp, "", "bmp").first() {
            None => Ok(Some("No picture came out.".into())),
            Some(shot) => save_town(g, shot, None, None),
        }
    })();
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    let _ = std::fs::remove_file(&script);
    r.unwrap_or_else(Some)
}

// Seconds from the start of the last list run to each stop's picture (kept in done.txt), and the stops the engine
// gave up on (its log: "stop N never arrived"): their pictures are of somewhere else.
static STOP_TIMES: Mutex<String> = Mutex::new(String::new());
static STOPS_MISSED: Mutex<Option<HashSet<i32>>> = Mutex::new(None);
fn missed(n: i32) -> bool { STOPS_MISSED.lock().unwrap().as_ref().map_or(false, |s| s.contains(&n)) }

fn warp_list(home: &str, towns: &[&str]) -> String {
    let mut s = home.to_string();
    for t in towns { let f: Vec<&str> = t.split(' ').collect(); s += &format!(";{}:{}:{}:1", f[1], f[2], f[3]); }
    s
}
/// A run that walks a list leaves <shot>-stop-NN.png per town (gen4recomp NP_STAT_WARP_STOPS).
fn stop_file(shot: &Path, n: usize) -> PathBuf { let s = shot.to_string_lossy(); PathBuf::from(format!("{}-stop-{:02}.png", &s[..s.len() - 4], n)) }
fn delete_stops(shot: &Path) {
    let s = shot.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = format!("{}-stop-", &s[..s.len().saturating_sub(4)]);
    let Some(dir) = shot.parent() else { return };
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with(&stem) && n.to_lowercase().ends_with(".png") { let _ = std::fs::remove_file(e.path()); }
    }
}
/// SaveStops: stop `first` is towns[0]. A run stopped early (the launcher closing, a game starting) isn't the set.
fn save_stops(g: &GameEntry, shot: &Path, towns: &[&str], first: usize, part: Part) -> Result<usize, String> {
    if art_jobs::hold() { return Ok(0); }
    let io = |e: std::io::Error| e.to_string();
    let dir = towns_dir(&g.id);
    reset_dir(&dir).map_err(io)?;
    let mut made = 0;
    for (i, t) in towns.iter().enumerate() {
        let png = stop_file(shot, first + i);
        let name = t.split(' ').next().unwrap_or("");
        if !missed((first + i) as i32) && png.is_file() && save_town(g, &png, Some(part), Some(&dir.join(format!("{}.jpg", name))))?.is_none() { made += 1; }
    }
    // Written even when none came out (an older engine): tried once per engine build, not at every start.
    let times = STOP_TIMES.lock().unwrap().clone();
    std::fs::write(dir.join("done.txt"), format!("engine={}{}{} towns{}{}", engine_stamp(g), NL, made, NL, times)).map_err(io)?;
    Ok(made)
}

/// "PC_WARP stop N never arrived": N.
fn stop_never_arrived(line: &str) -> Option<i32> {
    let at = line.find("PC_WARP stop ")? + 13;
    let digits: String = line[at..].chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || !line[at + digits.len()..].starts_with(" never arrived") { return None; }
    digits.parse().ok()
}
/// RunNativeplatTest: nativeplat's test mode, no window or sound. A list run ends once its last stop is pictured.
fn run_nativeplat_test(spec: &str, lab: Option<&str>, warp: Option<&str>, sweep: Option<&str>, shot: Option<&Path>, stops: usize) -> Result<i32, String> {
    let exe = config::nativeplat();
    let mut psi = StartInfo::new(&exe, "");
    psi.cwd = exe.parent().map(Path::to_path_buf);
    psi.set_env("SDL_VIDEO_DRIVER", "dummy");
    psi.set_env("SDL_AUDIO_DRIVER", "dummy");
    psi.set_env("NP_AUTOTEST", spec);
    if let Some(l) = lab { psi.set_env("PC_LAB", l); psi.set_env("PC_LAB_AT", "300"); }
    if let Some(w) = warp { psi.set_env("PC_WARP", w); psi.set_env("PC_WARP_SETTLE", "120"); }
    *STOPS_MISSED.lock().unwrap() = Some(HashSet::new());
    if let Some(s) = sweep { psi.set_env("PC_SWEEP", s); }
    if nm_core::test_hooks::art_shots(&psi, Some(&art_root())) { return Ok(0); }
    art_jobs::lower(&mut psi);
    let mut child = nm_core::proc::command_for(&psi).spawn().map_err(|e| e.to_string())?;
    nm_core::proc::apply_priority(&child, &psi);
    let pid = child.id();
    art_jobs::watch(pid);
    let said: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let readers: Vec<_> = [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)]
        .into_iter().flatten().map(|r| {
            let said = said.clone();
            std::thread::spawn(move || {
                use std::io::BufRead;
                for l in std::io::BufReader::new(r).split(b'\n').map_while(Result::ok) {
                    let l = String::from_utf8_lossy(&l).trim_end_matches('\r').to_string();
                    if let Some(n) = stop_never_arrived(&l) { if let Some(s) = STOPS_MISSED.lock().unwrap().as_mut() { s.insert(n); } }
                    let mut q = said.lock().unwrap();
                    q.push_back(l);
                    if q.len() > 40 { q.pop_front(); }
                }
            })
        }).collect();
    // WaitForExit(ms): whether it ended within that time.
    let wait = |child: &mut std::process::Child, ms: u64| -> bool {
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            if let Ok(Some(_)) = child.try_wait() { return true; }
            if Instant::now() >= end { return false; }
            std::thread::sleep(Duration::from_millis(15));
        }
    };
    let clock = Instant::now();
    let (mut seen, mut last_at) = (0usize, 0f64);
    let mut times: Vec<String> = Vec::new();
    while !wait(&mut child, 500) {
        let now = clock.elapsed().as_secs_f64();
        if now > 1500.0 { let _ = child.kill(); let _ = child.wait(); art_jobs::unwatch(pid); return Ok(-1); }
        let Some(shot) = shot.filter(|_| stops > 0) else { continue };
        while seen < stops && stop_file(shot, seen + 1).is_file() { seen += 1; last_at = clock.elapsed().as_secs_f64(); times.push((last_at as i32).to_string()); }
        // A place that hangs the game without a word: keep what came out.
        if seen > 0 && seen < stops && clock.elapsed().as_secs_f64() - last_at > 60.0 { times.push("stalled".into()); let _ = child.kill(); wait(&mut child, 5000); break; }
        if seen == stops { std::thread::sleep(Duration::from_secs(1)); let _ = child.kill(); wait(&mut child, 5000); break; }
    }
    let code = child.try_wait().ok().flatten();
    if code.is_some() { for r in readers { let _ = r.join(); } }
    art_jobs::unwatch(pid);
    if shot.is_some() {
        let q = said.lock().unwrap();
        *STOP_TIMES.lock().unwrap() = format!("stops at s: {}, run {} s{}last output:{}{}", times.join(" "), clock.elapsed().as_secs_f64() as i32, NL, NL, q.iter().cloned().collect::<Vec<_>>().join(NL));
    }
    Ok(code.map_or(0, |c| c.code().unwrap_or(-1)))
}

/// CaptureTownNativeplat: towns from nativeplat's test mode, which never uses the player's saves.
fn capture_town_nativeplat(g: &GameEntry) -> Option<String> {
    let shot = art_root().join(format!("{}-town.png", g.id));
    let exe = config::nativeplat();
    let root = exe.parent().map(Path::to_path_buf).unwrap_or_default().join("userdata");
    let core = g.town_core.clone().or_else(|| g.nativeplat.clone()).unwrap_or_default();
    let slot_name = "nativemons-wallpaper";
    let slot = root.join("saves").join(&core).join(format!("{}.sav", slot_name));
    let script = PathBuf::from(format!("{}.press", shot.display()));
    let rom_copy = PathBuf::from(format!("{}.nds", shot.display()));
    let rom = if g.town_core.is_some() { rom_copy.clone() } else { match nm_core::play_prep::nativeplat_rom_copy(g) { Ok(r) => r, Err(e) => return Some(e) } };
    let view = format!("size=1024x768,layout=top,zoom=1024,game={},png={},rom={}", core, shot.display(), rom.display());
    let plus = |p: &Path, s: &str| PathBuf::from(format!("{}{}", p.display(), s));
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        std::fs::create_dir_all(art_root()).map_err(io)?;
        if shot.is_file() { std::fs::remove_file(&shot).map_err(io)?; }
        if g.town_core.is_some() {
            // New Bark Town, from a new game on a copy of the player's ROM. The ROM stays as it is.
            std::fs::copy(g.fulldex_rom.as_deref().unwrap_or(""), &rom_copy).map_err(io)?;
            let mut perm = std::fs::metadata(&rom_copy).map_err(io)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = std::fs::set_permissions(&rom_copy, perm);
            std::fs::write(&script, HG_NEW_GAME.replace(';', NL)).map_err(io)?;
            // New Bark first, then every other town; an engine without the list stops at New Bark.
            let hg_part = (0.02, 0.24, 0.76, 0.64);
            delete_stops(&shot);
            run_nativeplat_test(&format!("frames={},{},press=@{}", 18500 + 2000 * HG_TOWNS.len(), view, script.display()), None, Some(&warp_list("60:686:393:1", &HG_TOWNS)), None, Some(&shot), HG_TOWNS.len() + 1)?;
            let home = if stop_file(&shot, 1).is_file() { stop_file(&shot, 1) } else { shot.clone() };
            if !home.is_file() { return Ok(Some("No picture came out.".into())); }
            let why = save_town(g, &home, Some(hg_part), None)?;
            if why.is_none() { save_stops(g, &shot, &HG_TOWNS, 2, hg_part)?; }
            return Ok(why);
        }
        let part = (0.1, 0.06, 0.66, 0.9);
        let platinum = g.nativeplat.as_deref() == Some("platinum");
        let twinleaf = format!("inline:map {} {} {} 1;", TWINLEAF, TWINLEAF_X, TWINLEAF_Z);
        if platinum {
            std::fs::create_dir_all(slot.parent().unwrap_or(&root)).map_err(io)?;
            std::fs::write(&slot, b"").map_err(io)?;
            let stored = format!(",storage=1,slot={}", slot_name);
            // This run ends itself once the save is made, with an exit code of its own.
            run_nativeplat_test(&format!("frames=1500,{}{}", view, stored), Some(&twinleaf), None, None, None, 0)?;
            if std::fs::metadata(&slot).map_err(io)?.len() == 0 { return Ok(Some("The wallpaper's save wasn't made.".into())); }
            if shot.is_file() { std::fs::remove_file(&shot).map_err(io)?; }
            run_nativeplat_test(&format!("frames=2800,{}{},press=1450:start:4:40:4;1800:a:4:60:6", view, stored), None, None, None, None, 0)?;
        } else {
            std::fs::write(&script, DP_NEW_GAME.replace(';', "\n")).map_err(io)?;
            delete_stops(&shot);
            let warp = warp_list(&format!("{}:{}:{}:1", TWINLEAF, TWINLEAF_X, TWINLEAF_Z), &DP_TOWNS);
            run_nativeplat_test(&format!("frames={},{},press=@{}", 15500 + 2000 * DP_TOWNS.len(), view, script.display()), None, Some(&warp), None, Some(&shot), DP_TOWNS.len() + 1)?;
        }
        let home = if !platinum && stop_file(&shot, 1).is_file() { stop_file(&shot, 1) } else { shot.clone() };
        if !home.is_file() { return Ok(Some("No picture came out.".into())); }
        // Around the town itself, which sits in the upper middle of this view, clear of the dark past the loaded map.
        let why = save_town(g, &home, Some(part), None)?;
        if why.is_some() || !platinum { if why.is_none() { save_stops(g, &shot, &DP_TOWNS, 2, part)?; } return Ok(why); }
        // Platinum's other towns: one new-game run of its lab's sweep (MAP -1 X Z lines), a picture per town.
        delete_stops(&shot);
        std::fs::create_dir_all(slot.parent().unwrap_or(&root)).map_err(io)?;
        std::fs::write(&slot, b"").map_err(io)?;
        // Inline: the game core has no file system to read a list from.
        let sweep = format!("inline:{}", pt_towns().iter().map(|t| { let f: Vec<&str> = t.split(' ').collect(); format!("{} -1 {} {}", f[1], f[2], f[3]) }).collect::<Vec<_>>().join(";"));
        run_nativeplat_test(&format!("frames={},{},storage=1,slot={}", 1500 + 2000 * pt_towns().len(), view, slot_name), Some(&twinleaf), None, Some(&sweep), Some(&shot), pt_towns().len())?;
        save_stops(g, &shot, pt_towns(), 1, part)?;
        Ok(None)
    })();
    for f in [shot.clone(), script, rom_copy, slot.clone(), plus(&slot, ".bak"), plus(&slot, ".tmp"), root.join("options-autotest.ini")] { if f.is_file() { let _ = std::fs::remove_file(&f); } }
    delete_stops(&shot);
    r.unwrap_or_else(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trim_dark_keeps_the_lit_block() {
        let mut b = Bitmap::new(64, 48);
        for y in 8..40 { for x in 16..64 { b.data[(y * 64 + x) as usize] = 0xFF80_8080; } }
        assert_eq!(trim_dark(&b, (0, 0, 64, 48)), (16, 8, 48, 32));
    }
    #[test]
    fn warp_and_stop_names() {
        assert_eq!(warp_list("1:2:3:1", &DP_TOWNS[..2]), "1:2:3:1;418:176:848:1;426:176:640:1");
        assert_eq!(stop_file(Path::new("a/b-town.png"), 3), PathBuf::from("a/b-town-stop-03.png"));
        assert_eq!(stop_never_arrived("x PC_WARP stop 12 never arrived"), Some(12));
        assert_eq!(stop_never_arrived("PC_WARP stop 12 arrived"), None);
    }
}
