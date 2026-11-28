//! GameArt's capture side, line by line from the C# (Capture, CaptureGen1, CaptureGba, CaptureNativeplat, Sort,
//! CapturePc, CapturePcTown): each game's 48 title screen pictures and its logo, made by its engine in the background
//! and kept as JPEGs written byte for byte as GDI+ writes them.

use crate::game_art::COUNT;
use nm_core::art_jobs;
use nm_core::config;
use nm_core::game_entry::GameEntry;
use nm_core::proc::StartInfo;
use nm_draw::{Bitmap, Graphics, RectangleF};
use std::path::{Path, PathBuf};

const STEP: i32 = 6;
pub(crate) const ART_IDENTITY: &str = "nativemons-art";
const GBA_PRESS: i32 = 1000;
const GBA_FIRST_SHOT: i32 = 1500;

pub(crate) fn art_root() -> PathBuf { config::state().join("game-art") }
pub(crate) fn folder(id: &str) -> PathBuf { art_root().join(id) }
pub fn frame_file(id: &str, i: usize) -> PathBuf { folder(id).join(format!("frame-{:02}.jpg", i)) }
fn logo_file(id: &str) -> PathBuf { folder(id).join("logo.png") }
pub(crate) fn town_file(id: &str) -> PathBuf { folder(id).join("town.jpg") }
fn shots_dir(id: &str) -> PathBuf { art_root().join(format!("{}-shots", id)) }
pub(crate) fn reset_dir(d: &Path) -> std::io::Result<()> { if d.is_dir() { std::fs::remove_dir_all(d)?; } std::fs::create_dir_all(d) }
pub(crate) fn save_jpeg(b: &Bitmap, to: &Path, quality: u8) -> Result<(), String> {
    std::fs::write(to, b.encode_jpeg(quality).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
/// Image.Save of a picture as it was loaded (its property items go with it).
fn resave_jpeg(from: &Path, to: &Path, quality: u8) -> Result<(), String> {
    let b = Bitmap::load_any(from).map_err(|e| e.to_string())?;
    std::fs::write(to, b.encode_jpeg_exif(quality, nm_draw::bitmap::exif_of(from)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
/// Files in a folder with an extension (case-insensitive), sorted by name as StringComparer.Ordinal does.
pub(crate) fn files(dir: &Path, prefix: &str, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map_or(false, |x| x.to_string_lossy().eq_ignore_ascii_case(ext)) && p.file_name().map_or(false, |n| n.to_string_lossy().starts_with(prefix)))
        .collect();
    v.sort_by(|a, b| a.to_string_lossy().as_bytes().cmp(b.to_string_lossy().as_bytes()));
    v
}

/// The engine run (Process.Start, at Idle or BelowNormal, output read and dropped, killed past `seconds`). With
/// NATIVEMONS_TEST_ART_SHOTS (tests only, as the C#'s TestHooks.ArtShots) it isn't started: its plan is added to
/// NATIVEMONS_TEST_ART_PLAN and that folder's pictures are copied in as if it had made them.
pub(crate) fn run_engine(psi: &StartInfo, temp: &Path, seconds: u64) -> Option<String> {
    if nm_core::test_hooks::art_shots(psi, Some(temp)) { return None; }
    let mut psi = StartInfo { program: psi.program.clone(), args: psi.args.clone(), env: psi.env.clone(), cwd: psi.cwd.clone(), ..Default::default() };
    art_jobs::lower(&mut psi);
    let pid = std::cell::Cell::new(0);
    let ran = nm_core::proc::run_watched(&psi, seconds, &|p| { pid.set(p); art_jobs::watch(p) });
    art_jobs::unwatch(pid.get());
    match ran {
        Ok(f) if f.code.is_some() => None,
        Ok(_) => Some("It took too long.".into()),
        Err(e) => Some(e.to_string()),
    }
}

/// Capture: None when done, or why not. It uses no save, at low priority, and the ROM is only read.
pub fn capture(g: &GameEntry) -> Option<String> {
    if g.pc.is_some() { return capture_pc(g); }
    if !g.is_ds() && g.is_hack() { return capture_gba(g); }
    if !g.is_ds() { return capture_gen1(g); }
    if g.fulldex_code.is_none() { return capture_nativeplat(g); }
    let temp = shots_dir(&g.id);
    let patched = PathBuf::from(format!("{}-rom", temp.display()));
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        reset_dir(&temp).map_err(io)?;
        let dir = config::fulldex_dir();
        let mut rom = g.fulldex_rom.clone().unwrap_or_default();
        // An encrypted ROM won't start. Its picture comes from a patched copy, and the player's file isn't touched.
        if nm_core::library::needs_patch(Some(&rom)) {
            reset_dir(&patched).map_err(io)?;
            let copy = patched.join("rom.nds");
            std::fs::copy(&rom, &copy).map_err(io)?;
            let mut perm = std::fs::metadata(&copy).map_err(io)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = std::fs::set_permissions(&copy, perm);
            rom = copy.to_string_lossy().into_owned();
            let patcher = config::patcher();
            let (code, _) = nm_core::nds_patch::patch_files(patcher.parent().unwrap_or(Path::new(".")), &[rom.clone()]);
            // The patcher backs up what it changes, and this copy's backup would stop the next copy being patched.
            if let Some(p) = patcher.parent() { let _ = std::fs::remove_file(p.join("backups").join("rom.secure-area.bin")); }
            if code != 0 || nm_core::library::needs_patch(Some(&rom)) { return Ok(Some("Could not patch a copy of the ROM for its picture.".into())); }
        }
        let at: Vec<String> = (0..COUNT as i32).map(|i| (g.art_frame + i * STEP).to_string()).collect();
        let mut args = format!("\"{}\" --no-save --config \"{}\" --jit --headless --compose 960x720 --shot-dir \"{}\" --shot-at {} --frames {}",
            rom, dir.join("settings.cfg").display(), temp.display(), at.join(","), g.art_frame + COUNT as i32 * STEP + 10);
        let code = g.fulldex_code.as_deref().unwrap_or("");
        if code != "IPGE" && code != "IPKE" { args += &format!(" --funcs \"{}\"", dir.join("data").join("funcs").join(format!("{}.txt", code)).display()); }
        let mut psi = StartInfo::new(config::fulldex(), &args);
        psi.cwd = Some(dir);
        if let Some(why) = run_engine(&psi, &temp, 180) { return Ok(Some(why)); }
        let shots = files(&temp, "", "bmp");
        if shots.len() != COUNT { return Ok(Some(format!("Only {} pictures came out.", shots.len()))); }
        sort(&shots, &g.id)?;
        Ok(None)
    })();
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    if patched.is_dir() { let _ = std::fs::remove_dir_all(&patched); }
    r.unwrap_or_else(Some)
}

fn distance(a: &Bitmap, b: &Bitmap) -> i64 {
    let ch = |p: u32, s: u32| ((p >> s) & 255) as i64;
    a.data.iter().zip(b.data.iter()).map(|(&p, &q)| {
        (ch(p, 16) - ch(q, 16)).abs() + (ch(p, 8) - ch(q, 8)).abs() + (ch(p, 0) - ch(q, 0)).abs()
    }).sum()
}
pub(crate) fn lit(b: &Bitmap, x: i32, y: i32) -> bool { let p = b.data[(y * b.width + x) as usize]; ((p >> 16) & 255) + ((p >> 8) & 255) + (p & 255) > 24 }

/// Sort: widescreen shows both screens; the logo is the half that's the same in every picture.
fn sort(shots: &[PathBuf], id: &str) -> Result<(), String> {
    const THUMB_W: i32 = 32;
    const THUMB_H: i32 = 24;
    let (mut band_top, mut band_bottom, mut width, mut height) = (i32::MAX, 0, 0, 0);
    let load = |f: &PathBuf| Bitmap::load_any(f).map_err(|e| e.to_string());
    for f in shots {
        let pic = load(f)?;
        width = pic.width; height = pic.height;
        let mut y = 0;
        while y < pic.height {
            let mut any = false;
            let mut x = 0;
            while x < pic.width && !any { any = lit(&pic, x, y); x += 8; }
            if any { band_top = band_top.min(y); band_bottom = band_bottom.max(y); }
            y += 4;
        }
    }
    if band_top >= band_bottom { return Err("The title screen came out blank.".into()); }
    let band = (band_top, (height - band_top).min(band_bottom - band_top + 4));
    let (mut thumbs, mut sides, mut sources) = (Vec::new(), Vec::new(), Vec::new());
    for (i, f) in shots.iter().enumerate() {
        let pic = load(f)?;
        for right in [false, true] {
            let half = pic.crop(if right { width / 2 } else { 0 }, band.0, width / 2, band.1);
            let mut g = Graphics::new(Bitmap::new(THUMB_W, THUMB_H));
            g.draw_image_bilinear(&half, RectangleF::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32));
            thumbs.push(g.bmp); sides.push(right); sources.push(i);
        }
    }
    let (mut logo, mut best) = (0usize, i64::MAX);
    for i in 0..thumbs.len() {
        let mut near: Vec<i64> = (0..thumbs.len()).step_by(3).map(|j| distance(&thumbs[i], &thumbs[j])).collect();
        near.sort();
        let total: i64 = near[..near.len() / 2].iter().sum();
        if total < best { best = total; logo = i; }
    }
    std::fs::create_dir_all(folder(id)).map_err(|e| e.to_string())?;
    for (i, f) in shots.iter().enumerate() {
        let left_is_logo = distance(&thumbs[i * 2], &thumbs[logo]) < distance(&thumbs[i * 2 + 1], &thumbs[logo]);
        let pic = load(f)?;
        // Without the small print along the bottom, which would be huge once it fills the window.
        let art = pic.crop(if left_is_logo { width / 2 } else { 0 }, band.0, width / 2, band.1 * 88 / 100);
        save_jpeg(&art, &frame_file(id, i), 85)?;
    }
    let pic = load(&shots[sources[logo]])?;
    pic.crop(if sides[logo] { width / 2 } else { 0 }, band.0, width / 2, band.1).save_png(&logo_file(id)).map_err(|e| e.to_string())?;
    Ok(())
}

/// CaptureGen1: Game Boy and GBA title screens, from a hidden, silent Gen1Recomp window.
fn capture_gen1(g: &GameEntry) -> Option<String> {
    let temp = shots_dir(&g.id);
    let throwaway = !g.imported;
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        if throwaway {
            let mut import = nm_core::engines::gen1_start("");
            nm_core::engines::use_art_identity(&mut import, ART_IDENTITY);
            import.set_env("POKEPORT_IMPORT_ONLY", "1");
            import.set_env("POKEPORT_FORCE_IMPORT", "1");
            let rom = nm_core::engines::PlainPath::new(g.gen1_rom.as_deref().unwrap_or("")).map_err(io)?;
            import.set_env("POKEPORT_IMPORT_ROM", &rom.path);
            let (_, code) = nm_core::engines::run_capture(&import, 240);
            if code != 0 { return Ok(Some("Could not prepare the game for its picture.".into())); }
        }
        reset_dir(&temp).map_err(io)?;
        let mut psi = nm_core::engines::gen1_start("");
        if throwaway { nm_core::engines::use_art_identity(&mut psi, ART_IDENTITY); }
        psi.set_env("POKEPORT_VERSION", g.gen1.as_deref().unwrap_or(""));
        psi.set_env("POKEPORT_BACKGROUND", "1");
        psi.set_env("POKEPORT_DRIVER", "src/nativemons/TitleArt.lua");
        psi.set_env("NATIVEMONS_TITLE_ART", &temp.to_string_lossy().replace('\\', "/"));
        if let Some(why) = run_engine(&psi, &temp, 240) { return Ok(Some(why)); }
        let shots = files(&temp, "", "png");
        if shots.len() != COUNT { return Ok(Some(format!("Only {} pictures came out.", shots.len()))); }
        let (mut left, mut top, mut right, mut bottom, mut width, mut height) = (i32::MAX, i32::MAX, 0, 0, 0, 0);
        for f in &shots {
            let pic = Bitmap::load_any(f).map_err(io)?;
            width = pic.width; height = pic.height;
            for y in (0..pic.height).step_by(2) { for x in (0..pic.width).step_by(2) {
                if lit(&pic, x, y) { left = left.min(x); top = top.min(y); right = right.max(x); bottom = bottom.max(y); }
            } }
        }
        if left >= right || top >= bottom { return Ok(Some("The title screen came out blank.".into())); }
        let (r_, b_) = (width.min(right + 2), height.min(bottom + 2));
        std::fs::create_dir_all(folder(&g.id)).map_err(io)?;
        for (i, f) in shots.iter().enumerate() {
            let pic = Bitmap::load_any(f).map_err(io)?;
            save_jpeg(&pic.crop(left, top, r_ - left, b_ - top), &frame_file(&g.id, i), 85)?;
        }
        Ok(None)
    })();
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    if throwaway { nm_core::engines::remove_art_data(ART_IDENTITY); }
    r.unwrap_or_else(Some)
}

/// CaptureGba: a GBA romhack's title screen from mGBA in a process of its own (--gba-art), on a copy of the ROM.
fn capture_gba(g: &GameEntry) -> Option<String> {
    let temp = shots_dir(&g.id);
    let rom = PathBuf::from(format!("{}.gba", temp.display()));
    let file = g.chosen_rom().or_else(|| g.roms.first()).map(|r| r.file.clone()).unwrap_or_default();
    if nm_core::library::is_ultra_violet_patch(&file) { return Some("A patch, not a ROM: no title screen to capture.".into()); }
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        reset_dir(&temp).map_err(io)?;
        std::fs::copy(&file, &rom).map_err(io)?;
        let mut perm = std::fs::metadata(&rom).map_err(io)?.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(&rom, perm);
        let at: Vec<String> = (0..COUNT as i32).map(|i| (GBA_FIRST_SHOT + i * STEP).to_string()).collect();
        let args = format!("--gba-art \"{}\" \"{}\" {} {}:start {}", rom.display(), temp.display(), GBA_FIRST_SHOT + COUNT as i32 * STEP + 2, GBA_PRESS, at.join(","));
        let me = std::env::current_exe().map_err(io)?;
        if let Some(why) = run_engine(&StartInfo::new(me, &args), &temp, 180) { return Ok(Some(why)); }
        let shots = files(&temp, "f-", "png");
        if shots.len() != COUNT { return Ok(Some(format!("Only {} pictures came out.", shots.len()))); }
        std::fs::create_dir_all(folder(&g.id)).map_err(io)?;
        for (i, f) in shots.iter().enumerate() { resave_jpeg(f, &frame_file(&g.id, i), 90)?; }
        Ok(None)
    })();
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    let _ = std::fs::remove_file(&rom);
    r.unwrap_or_else(Some)
}

/// CaptureNativeplat: a game only nativeplat runs; its test mode shows the title screen with no window or sound.
fn capture_nativeplat(g: &GameEntry) -> Option<String> {
    let temp = shots_dir(&g.id);
    let r = (|| -> Result<Option<String>, String> {
        let io = |e: std::io::Error| e.to_string();
        reset_dir(&temp).map_err(io)?;
        let exe = config::nativeplat();
        let mut psi = StartInfo::new(&exe, "");
        psi.cwd = exe.parent().map(|p| p.to_path_buf());
        psi.set_env("SDL_VIDEO_DRIVER", "dummy");
        psi.set_env("SDL_AUDIO_DRIVER", "dummy");
        let copy = nm_core::play_prep::nativeplat_rom_copy(g)?;
        psi.set_env("NP_AUTOTEST", &format!("frames={},shots={},size=512x384,layout=top,game={},png={},rom={},press=1200:start:10",
            1500 + COUNT as i32 * STEP, STEP, g.nativeplat.as_deref().unwrap_or(""), temp.join("s.png").display(), copy.display()));
        if let Some(why) = run_engine(&psi, &temp, 180) { return Ok(Some(why)); }
        let number = |f: &PathBuf| -> i64 { let n = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(); n.rfind('-').and_then(|at| n[at + 1..].parse::<i32>().ok()).map_or(-1, |v| v as i64) };
        let mut shots = files(&temp, "s-", "png");
        // OrderBy(number): a stable sort by the number after the last dash.
        shots.sort_by_key(|f| number(f));
        if shots.len() < COUNT { return Ok(Some(format!("Only {} pictures came out.", shots.len()))); }
        let shots = &shots[shots.len() - COUNT..];
        std::fs::create_dir_all(folder(&g.id)).map_err(io)?;
        for (i, f) in shots.iter().enumerate() { resave_jpeg(f, &frame_file(&g.id, i), 85)?; }
        Ok(None)
    })();
    if temp.is_dir() { let _ = std::fs::remove_dir_all(&temp); }
    r.unwrap_or_else(Some)
}

// ---------- PC games: from their own pictures, not from running them ----------
fn pc_graphics(g: &GameEntry) -> PathBuf { PathBuf::from(nm_core::pc_games::dir(g).unwrap_or_default()).join("Graphics") }
/// CapturePc: the title screen put together from the game's own layers, swirl and logo, 48 frames there and back.
/// Drawn with HighQualityBicubic as the C# (nm-draw's clone: scales exact, the swirl's rotation a close model).
pub fn capture_pc(g: &GameEntry) -> Option<String> {
    let r = (|| -> Result<(), String> {
        let gr = pc_graphics(g);
        let pf = nm_core::pc_games::of(g).ok_or("not a PC game")?;
        let open = |rel: &str| Bitmap::load_any(&gr.join(nm_core::paths::rel(rel))).map_err(|e| e.to_string());
        let layers = pf.title_bg.iter().map(|f| open(f)).collect::<Result<Vec<_>, _>>()?;
        let swirl = open(pf.title_swirl)?;
        let logo = open(pf.title_logo)?;
        std::fs::create_dir_all(folder(&g.id)).map_err(|e| e.to_string())?;
        let (w, h) = (layers[0].width, layers[0].height);
        let lw = w as f32 * 0.62;
        let lh = lw * logo.height as f32 / logo.width as f32;
        for i in 0..COUNT {
            // Played there and back, so the swirl sways a little behind the logo and the light breathes.
            let k = i as f32 / (COUNT - 1) as f32;
            let sway = -7.0 + 14.0 * k;
            let glow = 0.82 + 0.18 * (k as f64 * std::f64::consts::PI).sin() as f32;
            let mut f = Graphics::new(Bitmap::new(w, h));
            for (n, b) in layers.iter().enumerate() {
                // The drifting layer (clouds) slides along with the frames; the others fill the picture.
                if n as i32 == pf.drift { f.draw_image_hq_bicubic(b, RectangleF::new(-((b.width - w) as f32) * k, 0.0, b.width as f32, h as f32)); }
                else { f.draw_image_hq_bicubic(b, RectangleF::new(0.0, 0.0, w as f32, h as f32)); }
            }
            let st = f.save();
            f.translate_transform(w as f32 / 2.0, h as f32 * 0.47);
            f.rotate_transform(sway);
            let faded = with_alpha(&swirl, glow);
            f.draw_image_hq_bicubic(&faded, RectangleF::new((-lw * 0.5) as i32 as f32, (-lh * 0.5) as i32 as f32, lw as i32 as f32, lh as i32 as f32));
            f.restore(st);
            f.draw_image_hq_bicubic(&logo, RectangleF::new((w as f32 - lw) / 2.0, h as f32 * 0.47 - lh / 2.0, lw, lh));
            save_jpeg(&f.bmp, &frame_file(&g.id, i), 88)?;
        }
        Ok(())
    })();
    r.err()
}
/// ColorMatrix Matrix33: alpha times k, rounded.
fn with_alpha(b: &Bitmap, k: f32) -> Bitmap {
    let mut o = b.clone();
    for p in o.data.iter_mut() { let a = ((*p >> 24) as f64 * k as f64).round() as u32; *p = (*p & 0x00FF_FFFF) | a.min(255) << 24; }
    o
}
/// CapturePcTown: the game's own panorama or map as the town, as a JPEG.
pub fn capture_pc_town(g: &GameEntry) -> Option<String> {
    let r = (|| -> Result<(), String> {
        std::fs::create_dir_all(folder(&g.id)).map_err(|e| e.to_string())?;
        let pf = nm_core::pc_games::of(g).ok_or("not a PC game")?;
        resave_jpeg(&pc_graphics(g).join(nm_core::paths::rel(pf.town)), &town_file(&g.id), 90)
    })();
    r.err()
}

// ---------- Background: one game at a time, behind everything else (GameArt's scheduling) ----------
static MAKING: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// The game whose title screen is being made now, so its Play tile shows the loading Poke Ball meanwhile.
pub fn making() -> Option<String> { MAKING.lock().unwrap().clone() }
/// NATIVEMONS_TEST_NO_CAPTURE=1: no title or town captures, for test runs that check something else.
fn test_no_capture() -> bool { std::env::var("NATIVEMONS_TEST_NO_CAPTURE").as_deref() == Ok("1") }
/// CanCapture: added PC games go without a captured title screen or wallpaper.
pub fn can_capture(g: &GameEntry) -> bool {
    if g.pc.is_some() { return g.on_pc() && !nm_core::pc_games::is_custom(g); }
    if g.is_ds() && g.fulldex_code.is_none() { return g.on_nativeplat(); }
    if g.is_ds() { return g.art_frame > 0 && g.fulldex_rom.is_some() && config::fulldex().is_file(); }
    if g.is_hack() { return !g.roms.is_empty() && config::mgba_core().is_file(); }
    g.gen1.is_some() && (g.imported || g.gen1_rom.is_some())
}
struct Job { game: GameEntry, town: bool }
fn missing(games: &[GameEntry], first: Option<&str>) -> Vec<Job> {
    use crate::towns::{can_capture_town, has_towns, towns_by_gen1, towns_by_nativeplat};
    let mut jobs: Vec<Job> = games.iter().filter(|x| can_capture(x) && !crate::game_art::has(&x.id) && !failed(x, false)).map(|x| Job { game: x.clone(), town: false })
        .chain(games.iter().filter(|x| can_capture_town(x) && (!crate::game_art::has_town(&x.id) || ((towns_by_gen1(x) || towns_by_nativeplat(x)) && !has_towns(x))) && !failed(x, true)).map(|x| Job { game: x.clone(), town: true }))
        .collect();
    // The game the launcher opens on comes first, so its screen is ready even if the rest is stopped.
    jobs.sort_by_key(|j| if Some(j.game.id.as_str()) == first { 0 } else { 1 });
    jobs
}
fn run_job(j: &Job) -> Option<String> {
    if !j.town { *MAKING.lock().unwrap() = Some(j.game.id.clone()); }
    let why = if j.town { crate::towns::capture_town(&j.game) } else { capture(&j.game) };
    if why.is_some() && !art_jobs::hold() { note_failed(&j.game, j.town); }
    *MAKING.lock().unwrap() = None;
    why
}
/// CaptureAtStart: only the game the menu opens on, behind the splash; the rest are made once the menu is up.
pub fn capture_at_start(games: &[GameEntry], first: Option<&str>, progress: impl Fn(usize, usize)) {
    if test_no_capture() { return; }
    art_jobs::background(|| {
        let jobs: Vec<Job> = missing(games, first).into_iter().filter(|j| Some(j.game.id.as_str()) == first).collect();
        art_jobs::set_eager(true);
        let mut i = 0;
        while i < jobs.len() && !art_jobs::hold() { progress(i, jobs.len()); run_job(&jobs[i]); i += 1; }
        art_jobs::set_eager(false);
    });
}
/// CaptureMissing: `first`, then every missing picture until `busy` (a game or job running) or a hold.
pub fn capture_missing(games: &[GameEntry], busy: impl Fn() -> bool, first: impl FnOnce()) {
    if test_no_capture() { first(); return; }
    art_jobs::background(|| {
        first();
        for j in missing(games, None) {
            if art_jobs::hold() || busy() { return; }
            run_job(&j);
        }
    });
}
// A picture that can't be made isn't tried at every start: only after the ROM or the launcher changes.
fn failed_file() -> PathBuf { art_root().join("failed.txt") }
fn fail_key(g: &GameEntry, town: bool) -> String {
    let mut parts = vec![g.id.clone(), (if town { "town" } else { "title" }).to_string()];
    for rom in [&g.fulldex_rom, &g.gen1_rom].into_iter().flatten() {
        if let Ok(m) = std::fs::metadata(rom) { parts.push(format!("{}|{}|{}", rom, m.len(), nm_core::library::last_write_ticks(Path::new(rom)))); }
    }
    if let Ok(me) = std::env::current_exe() { parts.push(nm_core::library::last_write_ticks(&me).to_string()); }
    parts.join("|")
}
fn failed(g: &GameEntry, town: bool) -> bool {
    let key = fail_key(g, town);
    std::fs::read_to_string(failed_file()).map_or(false, |t| t.lines().any(|l| l == key))
}
fn note_failed(g: &GameEntry, town: bool) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(art_root());
    let nl = if cfg!(windows) { "\r\n" } else { "\n" };
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(failed_file()).and_then(|mut f| f.write_all(format!("{}{}", fail_key(g, town), nl).as_bytes()));
}
/// CaptureTown (towns.rs).
pub fn capture_town(g: &GameEntry) -> Option<String> { crate::towns::capture_town(g) }