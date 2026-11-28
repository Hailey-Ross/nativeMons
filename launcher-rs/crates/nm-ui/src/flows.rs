//! Flows that a replay only noted before: BringBack, OpenInPkhex and
//! OpenInOpenHome, ChooseVersion, the patcher (built in), engine settings, the 3DS emulators' install and remove,
//! Find mods' kind, sort and install, GetAllSprites, OpenUpdates and StartUpdate, and wallpapers (a picture of the
//! player's own, Remove, the search, its results and the preview). A dry-run replay with NATIVEMONS_TEST_FLOWS runs them.

use crate::art::*;
use crate::input::Effect;
use crate::launcher::{accent, Launcher};
use crate::model::{Act, Item, Page, Theme, Wall};
use crate::pages::theme_of;
use nm_core::config;
use nm_core::emu3ds;
use nm_core::game_entry::GameEntry;
use nm_core::proc::StartInfo;
use nm_draw::text::StringAlignment::{Center, Near};
use nm_draw::*;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn parent(p: &str) -> String { Path::new(p).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default() }

// ---------- pictures as GDI+ opens them ----------
/// Image.FromFile: PNG and JPEG by their content (as GDI+ decides); BMP and GIF aren't read here yet.
pub fn load_picture(path: &Path) -> Option<Bitmap> {
    let head = std::fs::read(path).ok()?;
    if head.starts_with(&[0x89, b'P', b'N', b'G']) { Bitmap::load_png(path).ok() } else if head.starts_with(&[0xFF, 0xD8]) { Bitmap::load_jpeg(path).ok() } else { None }
}
fn picture_from(bytes: &[u8]) -> Option<Bitmap> {
    let tmp = nm_core::proc::temp_dir().join(format!("nativemons-pic-{}.bin", nm_core::proc::new_guid()));
    std::fs::write(&tmp, bytes).ok()?;
    let b = load_picture(&tmp);
    let _ = std::fs::remove_file(&tmp);
    b
}
/// new Bitmap(img, w, h) when it's smaller than `max` on its long side (the same size: an exact copy).
fn shrink(img: Bitmap, max: f32) -> Bitmap {
    let k = (max / img.width.max(img.height) as f32).min(1.0);
    if k >= 1.0 { return img; }
    let (w, h) = (((img.width as f32 * k) as i32).max(1), ((img.height as f32 * k) as i32).max(1));
    let mut g = Graphics::new(Bitmap::new(w, h));
    g.draw_image_bilinear(&img, RectangleF::new(0.0, 0.0, w as f32, h as f32));
    g.bmp
}

// ---------- Wallpaper: Set, Remove, credits ----------
pub mod wallpaper {
    use super::*;
    const KINDS: [&str; 5] = [".png", ".jpg", ".jpeg", ".bmp", ".gif"];
    fn dir() -> std::path::PathBuf { config::state().join("wallpapers") }
    fn credit_file(id: &str) -> std::path::PathBuf { dir().join(format!("{}.credit.txt", id)) }
    /// A copy is kept, so the player's own file can move or go.
    pub fn set(id: &str, file: &str) -> Option<String> {
        if load_picture(Path::new(file)).is_none() { return Some("That file isn't a picture nativeMons can show. PNG, JPG, BMP and GIF work.".into()); }
        let ext = Path::new(file).extension().map(|x| format!(".{}", x.to_string_lossy().to_lowercase())).unwrap_or_default();
        remove(id);
        let r = std::fs::create_dir_all(dir()).and_then(|_| std::fs::copy(file, dir().join(format!("{}{}", id, ext))));
        r.err().map(|e| format!("Couldn't keep a copy of it: {}", e))
    }
    pub fn remove(id: &str) {
        crate::game_art::forget_own();
        for k in KINDS { let _ = std::fs::remove_file(dir().join(format!("{}{}", id, k))); }
        let _ = std::fs::remove_file(credit_file(id));
    }
    pub fn set_credit(id: &str, text: &str) { let _ = std::fs::write(credit_file(id), text); }
}

// ---------- WallFind: the search, as the C# asks it ----------
pub const FAN: &str = "wallhaven";
pub const FREE: &str = "openverse";
/// One search result; the thumbnail arrives later (Pic, Failed).
pub struct WallResult {
    pub title: String,
    pub thumb: String,
    pub full: String,
    pub page: String,
    pub credit: String,
    pub pic: Mutex<Option<Arc<Bitmap>>>,
    pub failed: AtomicBool,
}
/// Uri.EscapeDataString: everything but RFC 3986's unreserved characters, as %XX.
pub fn escape_data(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}
pub fn search_url(source: &str, query: &str, page: i32) -> String {
    let q = escape_data(query);
    if source == FAN { format!("https://wallhaven.cc/api/v1/search?q={}&purity=100&categories=110&sorting=relevance&page={}", q, page) } else { format!("https://api.openverse.org/v1/images/?q={}&mature=false&page_size=20&page={}", q, page) }
}
/// Str(d, k): a value as .NET's ToString shows it ("" when missing or null).
fn js(v: Option<&serde_json::Value>) -> String {
    match v {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Bool(b)) => if *b { "True".into() } else { "False".into() },
        Some(o) => o.to_string(),
    }
}
/// WallFind.Search: one page of results, and whether there are more.
pub fn search(source: &str, query: &str, page: i32) -> Result<(Vec<WallResult>, bool), String> {
    let text = nm_core::net::download_string(&search_url(source, query, page), None)?;
    let root: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    let mk = |title: String, thumb: String, full: String, page: String, credit: String| WallResult { title, thumb, full, page, credit, pic: Mutex::new(None), failed: AtomicBool::new(false) };
    let more;
    if source == FAN {
        more = root.get("meta").and_then(|m| js(m.get("last_page")).parse::<i32>().ok()).map_or(false, |last| page < last);
        for d in root.get("data").and_then(|x| x.as_array()).into_iter().flatten() {
            let url = js(d.get("url"));
            list.push(mk(js(d.get("resolution")), js(d.get("thumbs").and_then(|t| t.get("large"))), js(d.get("path")), url.clone(), format!("From wallhaven.cc, {}", url)));
        }
    } else {
        more = js(root.get("page_count")).parse::<i32>().map_or(false, |count| page < count);
        for d in root.get("results").and_then(|x| x.as_array()).into_iter().flatten() {
            let lic_id = js(d.get("license"));
            let mut lic = format!("CC {} {}", lic_id.to_uppercase(), js(d.get("license_version")));
            if lic_id == "cc0" { lic = "CC0".into(); } else if lic_id == "pdm" { lic = "Public domain".into(); }
            let landing = js(d.get("foreign_landing_url"));
            list.push(mk(js(d.get("title")), js(d.get("thumbnail")), js(d.get("url")), landing.clone(), format!("\"{}\" by {}, {}. {}", js(d.get("title")), js(d.get("creator")), lic.trim(), landing)));
        }
    }
    Ok((list, more))
}
pub fn load_thumb(r: &WallResult) {
    let got = (|| -> Option<Bitmap> {
        let tmp = nm_core::proc::temp_dir().join(format!("nativemons-thumb-{}.bin", nm_core::proc::new_guid()));
        let ok = nm_core::net::download_file(&r.thumb, &tmp, None, &mut |_| {}).is_ok();
        let b = if ok { std::fs::read(&tmp).ok().and_then(|d| picture_from(&d)) } else { None };
        let _ = std::fs::remove_file(&tmp);
        b
    })();
    match got { Some(b) => *r.pic.lock().unwrap() = Some(Arc::new(b)), None => r.failed.store(true, Ordering::SeqCst) }
    crate::sprites::ARRIVED.fetch_add(1, Ordering::SeqCst);
}
/// LoadThumbs: in the background, each one as it arrives; a replay gets them in order, at once.
fn load_thumbs(list: &[Arc<WallResult>], replay: bool) {
    for r in list {
        if r.pic.lock().unwrap().is_some() || r.failed.load(Ordering::SeqCst) { continue; }
        if replay { load_thumb(r); } else { let r = r.clone(); std::thread::spawn(move || load_thumb(&r)); }
    }
}
/// WallFind.Download: the full picture, into a temporary file the caller removes.
fn download_full(r: &WallResult) -> Result<String, String> {
    let path = r.full.split(['?', '#']).next().unwrap_or("");
    let path = path.find("://").map(|i| &path[i + 3..]).and_then(|p| p.find('/').map(|j| &p[j..])).unwrap_or("/");
    let mut ext = Path::new(path).extension().map(|x| format!(".{}", x.to_string_lossy().to_lowercase())).unwrap_or_default();
    if ![".png", ".jpg", ".jpeg", ".gif", ".bmp"].contains(&ext.as_str()) { ext = ".jpg".into(); }
    let tmp = nm_core::proc::temp_dir().join(format!("nativemons-wallpaper-{}{}", nm_core::proc::new_guid(), ext));
    nm_core::net::download_file(&r.full, &tmp, None, &mut |_| {})?;
    Ok(tmp.to_string_lossy().into_owned())
}

impl Launcher {
    fn dry_flow(&self) -> bool { self.hooks.replay && self.jobs.dry_run }

    // ---------- PKHeX, OpenHome, BringBack ----------
    /// BringBack: an edited cartridge save back into Gen1Recomp.
    pub fn bring_back(&mut self, id: &str) {
        if self.note_flow("BringBack") { return; }
        let id = id.to_string();
        self.pick_file("Choose the edited save", "Cartridge save (*.sav)|*.sav", Box::new(move |l, file| {
            let (Some(file), Some(g)) = (file, l.game(&id)) else { return };
            let title = g.title.clone();
            l.busy("Bringing back the save", move || Ok(nm_core::saves::import_back(&g, Some(&file), false).0), move |l, r| {
                let (e, problem) = match r { Ok(p) => (None, p), Err(e) => (Some(e), None) };
                let done = e.is_none() && problem.is_none();
                let body = e.or(problem).unwrap_or_else(|| format!("{} will start from the edited save. Your older slots are kept.", title));
                l.show_text(if done { "Done" } else { "Not changed" }, &body);
            });
        }));
    }
    /// OpenInPkhex: the game's save in the player's own copy of PKHeX, which is asked for the first time.
    pub fn open_in_pkhex(&mut self, id: &str) {
        if self.note_flow("OpenInPkhex") { return; }
        self.with_program(id, "pkhex-path.txt", "Where is PKHeX? (projectpokemon.org/home/files/file/1-pkhex)", "PKHeX|PKHeX*.exe|Programs (*.exe)|*.exe", false);
    }
    /// OpenInOpenHome: OpenHome takes no file on its command line, so the save's path goes on the clipboard.
    pub fn open_in_openhome(&mut self, id: &str) {
        if self.note_flow("OpenInOpenHome") { return; }
        self.with_program(id, "openhome-path.txt", "Where is OpenHome? (github.com/andrewbenington/OpenHome/releases)", "OpenHome|OpenHome*.exe|Programs (*.exe)|*.exe", true);
    }
    fn with_program(&mut self, id: &str, setting: &'static str, ask: &str, filter: &str, home: bool) {
        let exe = config::read(setting, "");
        if Path::new(&exe).is_file() { self.open_save_in(id, exe, home); return; }
        let id = id.to_string();
        self.pick_file(ask, filter, Box::new(move |l, picked| {
            let Some(exe) = picked else { return };
            config::write(setting, &exe);
            l.open_save_in(&id, exe, home);
        }));
    }
    fn open_save_in(&mut self, id: &str, exe: String, home: bool) {
        let Some(g) = self.game(id) else { return };
        let g2 = g.clone();
        let title = if home { "Opening OpenHome".to_string() } else { format!("Opening {} in PKHeX", g.title) };
        self.busy(&title, move || Ok(nm_core::saves::save_of(&g2)), move |l, r| {
            let (file, why) = match r { Ok(x) => x, Err(e) => (None, Some(e)) };
            let Some(mut file) = file else { l.show_text("Can't open that save", &why.unwrap_or_default()); return };
            if home { file = file.replace('/', "\\"); }
            let mut psi = StartInfo::new(&exe, &if home { String::new() } else { format!("\"{}\"", file) });
            psi.cwd = Some(parent(&exe).into());
            let name = if home { "OpenHome" } else { "PKHeX" };
            if l.dry_flow() { l.effects.push(Effect::Launch(nm_core::proc::dry_line(name, &psi, false))); }
            else if let Err(x) = nm_core::proc::command_for(&psi).spawn() { l.show_text(&format!("Can't start {}", name), &x.to_string()); return; }
            let gen1 = g.engine() == "gen1";
            if home {
                if !l.hooks.replay { l.effects.push(Effect::Clipboard(file.clone())); }
                let after = if gen1 { "\n\nGen1Recomp keeps its own save format, so this is an exported copy. Save your changes there, then choose Bring back the edited save on this game's page." } else { "\n\nThe launcher picks up what OpenHome saves here. A backup is made before each game." };
                l.show_text("OpenHome is starting", &format!("Open this save in OpenHome. Its path is on your clipboard:\n\n{}{}\n\nOpenHome's moved Pokemon aren't guaranteed legal.", file, after));
            } else if gen1 {
                l.show_text("Editing an exported copy", &format!("Gen1Recomp keeps its own save format, so PKHeX has an exported copy:\n{}\n\nSave your changes there in PKHeX, then choose Bring back the edited save on this game's page.", file));
            }
        });
    }

    // ---------- ROM versions ----------
    /// Rescan(then): the library again, then what follows, then the questions a scan raises.
    pub fn rescan_then(&mut self, then: impl FnOnce(&mut Launcher)) {
        self.rescan();
        then(self);
        self.ask_about_encrypted();
        self.ask_about_recomp();
    }
    /// ChooseVersion: which of a game's dumps it plays.
    pub fn choose_version(&mut self, id: &str, sha1: &str, now: bool) {
        if self.note_flow("ChooseVersion") { return; }
        if now { self.pages.pop(); self.dirty = true; return; }
        { let (i2, s2) = (id.to_string(), sha1.to_string()); if self.after_capture(move |l| l.choose_version(&i2, &s2, false)) { return; } }
        let running = nm_core::saves::running_games(&self.ctx().lib.games);
        if !running.is_empty() { self.show_text("Close your game first", &format!("Already running: {}. Close it, then change version.", running)); return; }
        let Some(g) = self.game(id) else { return };
        config::write(&format!("rom-{}.txt", id), sha1);
        let old_gen1 = g.gen1_rom.clone();
        let id = id.to_string();
        self.pages.pop();
        self.rescan_then(move |l| {
            let Some(fresh) = l.game(&id) else { return };
            // A scan makes new game entries, so the page behind this one still holds the old ROM.
            l.pages.pop();
            let p = crate::pages::game_page(l.ctx(), &fresh);
            l.push(p);
            // Gen1Recomp keeps game data made from its ROM, so it's made again, with saves kept.
            let changed = !old_gen1.as_deref().unwrap_or("").eq_ignore_ascii_case(fresh.gen1_rom.as_deref().unwrap_or("")) || old_gen1.is_none() != fresh.gen1_rom.is_none();
            if fresh.imported && fresh.gen1_rom.is_some() && changed { l.import(&fresh, None); }
        });
    }

    // ---------- the patcher ----------
    /// RunTool: a tool as a job (UseShellExecute off, no window, output read).
    pub fn run_tool(&mut self, title: &str, exe: &str, args: &str, dir: &str, done: Box<dyn FnOnce(&mut Launcher, i32)>) {
        let mut psi = StartInfo::new(exe, args);
        psi.cwd = Some(dir.into());
        self.run_job(title, psi, false, done);
    }
    /// hails.nds-Patcher, built in (Rust only: no Python): a Busy card, then `done` with the patcher's exit code.
    pub fn run_patcher(&mut self, title: &str, roms: Vec<String>, done: Box<dyn FnOnce(&mut Launcher, i32)>) {
        let here = config::patcher().parent().map(|p| p.to_path_buf()).unwrap_or_default();
        if self.dry_flow() {
            self.effects.push(Effect::Launch(format!("{} | hails.nds-Patcher (built in) | {} | {}", title, roms.iter().map(|r| format!("\"{}\"", r)).collect::<Vec<_>>().join(" "), here.display())));
            return;
        }
        self.busy(title, move || Ok(nm_core::nds_patch::patch_files(&here, &roms).0), move |l, r| done(l, r.unwrap_or(1)));
    }
    /// PatchPage's "Patch it": the ROM patched, then played or its page shown again.
    pub fn patch_rom(&mut self, id: &str, then_play: bool) {
        self.pages.pop();
        let Some(g) = self.game(id) else { return };
        let id = id.to_string();
        self.run_patcher(&format!("Patching {}", g.title), vec![g.fulldex_rom.clone().unwrap_or_default()], Box::new(move |l, code| {
            if code != 0 { l.show_log("Patching failed", &format!("The patcher stopped with code {}.", code), None); return; }
            if then_play { l.play(&id); } else { l.pages.pop(); l.push_game(&id, crate::pages::game_page); }
        }));
    }
    /// AskAboutEncrypted's "Patch now": every ROM found at once.
    pub fn patch_found(&mut self, roms: Vec<String>) {
        self.pages.pop();
        self.run_patcher("Patching", roms, Box::new(|l, code| {
            if code != 0 { l.show_log("Patching failed", &format!("The patcher stopped with code {}.", code), None); } else { l.replace_main(); }
        }));
    }

    // ---------- engine settings ----------
    /// OpenEngineSettings: Fulldex's settings.cfg, or Gen1Recomp's options through its command line.
    pub fn open_engine_settings(&mut self, group: &str) {
        if self.note_flow(&format!("OpenEngineSettings {}", group)) { return; }
        let has_fulldex = config::fulldex().is_file();
        let has_gen1 = config::gen1().join("main.lua").is_file();
        let group = group.to_string();
        let ask = has_gen1 && group != "fulldex";
        self.busy("Reading settings", move || {
            let mut gen1 = std::collections::HashMap::new();
            if ask {
                for row in gen1_ask("options").unwrap_or_default().split('\n') {
                    let f: Vec<&str> = row.trim_end_matches('\r').split('\t').collect();
                    if f.len() == 2 { gen1.insert(f[0].to_string(), f[1].to_string()); }
                }
            }
            Ok(gen1)
        }, move |l, r| {
            let gen1 = r.unwrap_or_default();
            let cfg = if has_fulldex { nm_core::play_prep::read_fulldex() } else { Default::default() };
            let g1 = |k: &str, d: &str| gen1.get(k).filter(|v| *v != "nil").cloned().unwrap_or_else(|| d.to_string());
            let mut p = Page::new("");
            if group == "fulldex" {
                p.title = "Fulldex".into();
                let switch = |p: &mut Page, key: &'static str, label: &str, fallback: bool, info: &str| {
                    let on = cfg.get(key).map_or(fallback, |v| v == "1");
                    choice(p, label, info, &["ON", "OFF"], if on { 0 } else { 1 }, move |i| { let _ = nm_core::play_prep::write_fulldex(key, if i == 0 { "1" } else { "0" }); });
                };
                switch(&mut p, "wide", "Widescreen world", true, "Draws the world out to the sides of a widescreen display.");
                switch(&mut p, "fps60", "60 FPS", true, "Adds in-between frames for smoother motion.");
                switch(&mut p, "smooth", "Smooth scaling", false, "Softens the picture instead of keeping sharp pixels.");
                switch(&mut p, "wide_map", "Wide map loading", true, "Black, White, Black 2 and White 2: loads more of the map, so the sides don't run out of land.");
                switch(&mut p, "busy_detail", "Lower res when busy", false, "Drops the resolution when your PC can't keep up.");
                switch(&mut p, "show_fps", "FPS counter", false, "Shows frames per second in a corner.");
                const HEIGHTS: [&str; 4] = ["720", "1080", "1440", "2160"];
                let lines = cfg.get("wide_lines").cloned().unwrap_or_else(|| "1080".into());
                choice(&mut p, "3D resolution", "How sharply the 3D world is drawn. Higher needs a stronger PC.", &["720P", "1080P", "1440P", "4K"], HEIGHTS.iter().position(|h| *h == lines).map_or(-1, |i| i as i32), |i| { let _ = nm_core::play_prep::write_fulldex("wide_lines", HEIGHTS[i]); });
            } else {
                p.title = "Gen1Recomp".into();
                const SPEEDS: [&str; 3] = ["1", "3", "5"];
                let speed = g1("textSpeed", "3");
                choice(&mut p, "Text speed", "How fast text appears in dialogue.", &["FAST", "MEDIUM", "SLOW"], SPEEDS.iter().position(|s| *s == speed).map_or(-1, |i| i as i32), |i| write_gen1("textSpeed", SPEEDS[i]));
                choice(&mut p, "Battle animations", "Plays the move animations in battle.", &["ON", "OFF"], if g1("animations", "true") == "true" { 0 } else { 1 }, |i| write_gen1("animations", if i == 0 { "true" } else { "false" }));
                choice(&mut p, "Battle layout", "The original screen shape, or a wider battle view.", &["ORIGINAL", "WIDE"], if g1("battleLayout", "og") == "wide" { 1 } else { 0 }, |i| write_gen1("battleLayout", if i == 1 { "wide" } else { "og" }));
            }
            l.push(p);
        });
    }

    // ---------- the 3DS emulators ----------
    /// The emulator page's Install / Update item.
    pub fn emu_install(&mut self, id: &str) {
        let Some(e) = emu3ds::ALL.iter().find(|x| x.id == id).copied() else { return };
        let have = e.installed();
        if emu3ds::running(&e) { self.show_text(&format!("Close {} first", e.short), &format!("{} is open. Close it, then {} it.", e.short, if have { "update" } else { "install" })); return; }
        let flows = nm_core::test_hooks::flows();
        let replay = self.hooks.replay;
        self.busy(&format!("{}{}", if have { "Updating " } else { "Installing " }, e.short), move || Ok(if replay && !flows { None } else { emu3ds::install(&e) }), move |l, r| {
            if replay && !flows { l.todo("Emu3ds.Install"); }
            let problem = match r { Ok(p) => p.or_else(|| if replay && !flows { Some("a replay".into()) } else { None }), Err(x) => Some(x) };
            if let Some(p) = problem { l.show_text("Not installed", &p); return; }
            if emu3ds::default().is_none() || config::read("3ds-emulator.txt", "").is_empty() { config::write("3ds-emulator.txt", e.id); }
            // Right here: the animation plays over this page, then its menus show it installed.
            l.pages.pop(); l.pages.pop();
            l.push(crate::pages::emu_page());
            l.push(crate::pages::emu_detail_page(&e));
            l.replace_main();
            l.emu_install_name = Some(e.short.to_string());
            l.emu_install_at = l.now();
            l.dirty_poll = true;
        });
    }
    pub fn emu_remove(&mut self, id: &str) {
        let Some(e) = emu3ds::ALL.iter().find(|x| x.id == id).copied() else { return };
        if emu3ds::running(&e) { self.show_text(&format!("Close {} first", e.short), &format!("{} is open. Close it, then remove it.", e.short)); return; }
        self.busy(&format!("Removing {}", e.short), move || Ok(emu3ds::remove(&e)), move |l, r| {
            let problem = match r { Ok(p) => p, Err(x) => Some(x) };
            if let Some(p) = problem { l.show_text("Not removed", &p); return; }
            l.pages.pop(); l.pages.pop();
            l.push(crate::pages::emu_page());
            l.replace_main();
        });
    }

    // ---------- the Pokedex for offline play ----------
    /// GetAllSprites: every picture and Pokedex entry, only what's missing fetched.
    pub fn get_all_sprites(&mut self) {
        if self.note_flow("GetAllSprites") { return; }
        self.busy_reporting("Downloading the Pokedex", |report| {
            let summary = crate::sprites::get_all(&mut |done, total, _, _| { report("", done as f32 / total as f32); });
            let dex = crate::dex::get_all_infos(crate::sprites::MAX_DEX, &mut |done, total, _, _| { report("", done as f32 / total as f32); });
            Ok((summary, dex))
        }, |l, r| {
            match r {
                Ok(((fresh, total, failed), (dfresh, dtotal, dfailed))) => {
                    let summary = format!("{} downloaded, {} already here.{}", fresh, total - fresh - failed, if failed > 0 { format!("\n\n{} couldn't be fetched. Run this again when you're online: it only fetches what's missing.", failed) } else { "\n\nEvery picture is ready for offline play.".into() });
                    let dex = format!("Pokedex entries: {} downloaded, {} already here.{}", dfresh, dtotal - dfresh - dfailed, if dfailed > 0 { format!(" {} couldn't be fetched. Run this again when you're online.", dfailed) } else { String::new() });
                    l.show_text("Download Pokedex", &format!("Pictures: {}\n\n{}", summary, dex));
                }
                Err(e) => l.show_text("Download Pokedex", &e),
            }
        });
    }

    // ---------- updates ----------
    /// OpenUpdates: look, then say what there is and offer it.
    pub fn open_updates(&mut self) {
        if self.note_flow("OpenUpdates") { return; }
        use nm_core::updater as u;
        self.busy("Looking for updates", || Ok(u::check()), |l, r| {
            let (problem, e) = match r { Ok(p) => (p, None), Err(x) => (None, Some(x)) };
            let local = u::local();
            let here = match &local {
                Some(lc) => format!("a local build{}", if lc.len() >= 7 && lc != "yes" { format!(" of {}", &lc[..7]) } else { String::new() }),
                None => if u::build() > 0 { format!("{} {}", u::installed_name(), u::installed_label()) } else { "a local build, not from a release".into() },
            };
            let ch = u::channel();
            let chosen = u::name(&ch);
            let other = if ch == u::STABLE_TAG { u::DEV_TAG } else { u::STABLE_TAG };
            let channel = Item::new(&format!("Get updates from: {}", chosen), "", true).icon('\u{E895}').info(&format!("Stable is the tested build. Dev gets every change first, and may have more problems. Select to switch to {}.", u::name(other)))
                .run(move |l| { u::choose(other); l.pages.pop(); l.open_updates(); });
            let mut p = Page::new("Updates");
            if problem.is_some() || e.is_some() {
                p.body = Some(format!("This copy is {}.\n\nCouldn't look for updates on {}. {}", here, chosen, e.or(problem).unwrap_or_default()));
                p.items.push(channel); l.push(p); return;
            }
            let latest = u::latest();
            if local.is_some() && latest.as_ref().map_or(false, |r| r.sha256.as_deref().map_or(false, |s| !s.is_empty())) {
                p.body = Some(format!("This copy is {}, so updates aren't offered: a release could replace newer work.\n\nThe newest on {} is {}. Updating replaces this copy's launcher and engines with it; your saves and settings stay.", here, chosen, u::latest_label()));
                p.items.push(Item::new("Update anyway", "", true).icon('\u{E896}').run(|l| { l.pages.pop(); l.start_update(); }));
                p.items.push(Item::new("Not now", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
                p.items.push(channel); l.push(p); return;
            }
            if !u::ready() { p.body = Some(format!("This copy is {}.\n\nThe newest on {} is {}. You're up to date.", here, chosen, u::latest_label())); p.items.push(channel); l.push(p); return; }
            p.body = Some(format!("This copy is {}.\n\n{} {} is ready.{} It updates the launcher and every engine together, downloading about 200 MB.\n\nYour saves, ROM copies, wallpapers and settings stay as they are. The launcher restarts when it's done.", here, chosen, u::latest_label(), if u::switching() { " It can be older than what you have now." } else { "" }));
            p.items.push(Item::new("Update now", "", true).icon('\u{E896}').run(|l| { l.pages.pop(); l.start_update(); }));
            p.items.push(Item::new("Not now", "", true).icon('\u{E711}').run(|l| { l.pages.pop(); }));
            p.items.push(channel);
            l.push(p);
        });
    }
    /// StartUpdate: the release applied on a Busy card, then the new copy started.
    pub fn start_update(&mut self) {
        if self.after_capture(|l| l.start_update()) { return; }
        let running = nm_core::saves::running_games(&self.ctx().lib.games);
        if !running.is_empty() { self.show_text("Close your game first", &format!("Already running: {}. Close it, then update.", running)); return; }
        self.busy_stages("Updating nativeMons", |report| Ok(nm_core::updater::apply(report)), |l, r| {
            let wrong = match r { Ok(w) => w, Err(e) => Some(e) };
            if let Some(w) = wrong { l.show_text("Update stopped", &w); return; }
            l.effects.push(Effect::Restart);
        });
    }
    /// Busy whose work reports stages: each new stage a line on the card (not prefixed), with its share done.
    fn busy_stages<T: Send + 'static>(&mut self, title: &str, work: impl FnOnce(&mut dyn FnMut(&str, f32)) -> Result<T, String> + Send + 'static, then: impl FnOnce(&mut Launcher, Result<T, String>) + 'static) {
        self.busy_reporting(title, work, then);
    }

    // ---------- wallpapers ----------
    /// The picture the launcher's browser chose for a game (OwnWallpaperPage's Done).
    pub fn wallpaper_picked(&mut self, id: &str, file: &str) {
        if let Some(problem) = wallpaper::set(id, file) { self.show_text("Not used", &problem); return; }
        let id = id.to_string();
        self.rebuild_game_top(&id);
    }
    fn rebuild_game_top(&mut self, id: &str) {
        let Some(g) = self.game(id) else { return };
        self.rebuild_top(true, move |_| crate::pages::own_wallpaper_page(&g));
    }
    pub fn wallpaper_remove(&mut self, id: &str) {
        wallpaper::remove(id);
        self.rebuild_game_top(id);
        self.dirty = true;
    }
    /// RunWallSearch: one page of results, added after the ones shown before (More).
    pub fn run_wall_search(&mut self, id: &str, source: &str, query: &str, page: i32, before: Option<std::rc::Rc<Vec<Arc<WallResult>>>>) {
        if self.note_flow("RunWallSearch") { return; }
        let Some(g) = self.game(id) else { return };
        let (src, q) = (source.to_string(), query.to_string());
        let replay = self.hooks.replay;
        let (src2, q2) = (src.clone(), q.clone());
        self.busy("Searching", move || search(&src2, &q2, page), move |l, r| {
            let (found, more) = match r {
                Ok(x) => x,
                Err(e) => { l.show_text("Couldn't search", &format!("The search didn't answer: {}\n\nCheck your internet connection and try again.", e)); return; }
            };
            let found: Vec<Arc<WallResult>> = found.into_iter().map(Arc::new).collect();
            let mut all: Vec<Arc<WallResult>> = before.as_ref().map(|b| b.iter().cloned().collect()).unwrap_or_default();
            all.extend(found.iter().cloned());
            if all.is_empty() { l.show_text("Nothing found", &format!("No pictures for \"{}\". Try another search.", q)); return; }
            let mut sel = 0;
            if let Some(b) = &before { sel = b.len() as i32; l.pages.pop(); }
            let mut res = wall_results_page(&g, &src, &q, std::rc::Rc::new(all), page, more);
            res.sel = sel.min(res.items.len() as i32 - 1);
            l.push(res);
            load_thumbs(&found, replay);
            l.dirty = true;
        });
    }
}

/// Choice: a row that steps through its choices when pressed (the label changes in place; the work runs off the UI
/// thread, at once in a replay).
fn choice(p: &mut Page, label: &str, info: &str, names: &[&str], current: i32, apply: impl Fn(usize) + Send + Sync + 'static) {
    let at = std::rc::Rc::new(std::cell::Cell::new(current.max(0) as usize));
    let index = p.items.len();
    let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    let (label_s, n2) = (label.to_string(), names.clone());
    let apply = Arc::new(apply);
    let a2 = at.clone();
    p.items.push(Item::new(&format!("{}: {}", label, names[at.get()]), "", true).icon('\u{E713}').info(info).run(move |l| {
        a2.set((a2.get() + 1) % n2.len());
        if let Some(top) = l.pages.last_mut() { if let Some(it) = top.items.get_mut(index) { it.text = format!("{}: {}", label_s, n2[a2.get()]); } }
        let chosen = a2.get();
        let apply = apply.clone();
        if l.hooks.replay && nm_core::test_hooks::flows() { apply(chosen); } else { std::thread::spawn(move || apply(chosen)); }
        l.dirty = true;
    }));
}
/// Gen1Ask: Gen1Recomp's answer when it ran fine.
fn gen1_ask(command: &str) -> Option<String> {
    let mut psi = nm_core::engines::gen1_start("");
    psi.set_env("NATIVEMONS_CLI", command);
    let (o, code) = nm_core::engines::run_capture(&psi, 30);
    if code == 0 { Some(o) } else { None }
}
fn write_gen1(key: &str, value: &str) { gen1_ask(&format!("option {} {}", key, value)); }

/// WallResultsPage: the results as a grid of two, then More when there are more.
pub fn wall_results_page(g: &GameEntry, source: &str, query: &str, all: std::rc::Rc<Vec<Arc<WallResult>>>, page: i32, more: bool) -> Page {
    let mut p = Page::new(if source == FAN { "Fan art" } else { "Free-license art" });
    p.theme = Some(theme_of(g));
    p.gallery = true;
    for (i, r) in all.iter().enumerate() {
        let (id, all2) = (g.id.clone(), all.clone());
        p.items.push(Item::new(&r.title, "", true).run(move |l| l.open_wall_preview(&id, all2[i].clone())));
    }
    if more {
        let (id, src, q, all2) = (g.id.clone(), source.to_string(), query.to_string(), all.clone());
        p.items.push(Item::new("More", "", true).run(move |l| l.run_wall_search(&id, &src, &q, page + 1, Some(all2.clone()))));
    }
    p.wall = Wall { results: all, picture: None };
    p
}

impl Launcher {
    /// OpenWallPreview: the full picture downloaded, shown as the home screen would show it, with its credit.
    pub fn open_wall_preview(&mut self, id: &str, r: Arc<WallResult>) {
        let Some(g) = self.game(id) else { return };
        let r2 = r.clone();
        self.busy("Getting the picture", move || download_full(&r2), move |l, got| {
            let file = match got { Ok(f) => f, Err(e) => { l.show_text("Couldn't get it", &format!("The picture didn't download: {}", e)); return; } };
            let Some(pic) = load_picture(Path::new(&file)).map(|b| shrink(b, 1400.0)) else {
                let _ = std::fs::remove_file(&file);
                l.show_text("Couldn't use it", "That download isn't a picture nativeMons can show.");
                return;
            };
            let mut p = Page::new("Preview");
            p.theme = Some(theme_of(&g));
            p.picture = true;
            p.body = Some(r.credit.clone());
            p.wall = Wall { results: Default::default(), picture: Some(Arc::new(pic)) };
            let (gid, gtitle, f2, credit) = (g.id.clone(), g.title.clone(), file.clone(), r.credit.clone());
            p.items.push(Item::new(&format!("Use for {}", g.title.replace("Pokemon ", "")), "", true).run(move |l| {
                let bad = wallpaper::set(&gid, &f2);
                let _ = std::fs::remove_file(&f2);
                if let Some(b) = bad { l.show_text("Not used", &b); return; }
                wallpaper::set_credit(&gid, &credit);
                // Back to the game's own picture page, which now shows it.
                while l.pages.len() > 1 && l.pages.last().unwrap().title != gtitle { l.pages.pop(); }
                if l.pages.last().map_or(false, |p| p.title == gtitle) { l.rebuild_game_top(&gid); }
                l.dirty = true;
            }));
            let page = r.page.clone();
            p.items.push(Item::new("Open its page", "", !page.is_empty()).run(move |l| l.effects.push(Effect::OpenUrl(page.clone()))));
            let f3 = file.clone();
            p.on_back = Some(Act::new(move |l| { l.pages.pop(); let _ = std::fs::remove_file(&f3); }));
            l.push(p);
        });
    }

    /// RenderGallery: results as a grid of two, each cropped as it would fill the screen; the grid follows the selection.
    pub(crate) fn render_gallery(&mut self, g: &mut Graphics, p: &mut Page, w: i32, h: i32, m: f32, top: f32, bottom: f32, t: f32, th: &Theme) {
        let (wf, _hf) = (w as f32, h as f32);
        let accent_c = accent(th);
        let mut y0 = self.back_button_pub(g, w, h, m, top, accent_c);
        let tf = self.fonts.title.clone();
        self.w(g, &p.title, &tf, PHONE_TEXT, RectangleF::new(m, y0, wf - 2.0 * m, tf.height() as f32 * 1.3), Near, Center, false);
        y0 += tf.height() as f32 * 1.5;
        let gap = m * 0.6;
        let cw = (wf - 2.0 * m - gap) / 2.0;
        let ch = cw * 1.45;
        let row_h = ch + gap;
        let view_h = bottom - y0;
        let rows = (p.items.len() as i32 + 1) / 2;
        let sel_row = p.sel / 2;
        let want = 0f32.max((rows as f32 * row_h - view_h).min(sel_row as f32 * row_h - (view_h - row_h) / 2.0));
        if p.scroll_row < 0.0 { p.scroll_row = want; }
        p.scroll_row += (want - p.scroll_row) * 0.25;
        if (want - p.scroll_row).abs() > 0.5 { self.scrolling = true; } else { p.scroll_row = want; }
        let st = g.save();
        g.set_clip_rect(RectangleF::new(0.0, y0, wf, view_h));
        let results = p.wall.results.clone();
        for i in 0..p.items.len() {
            let x = m + (i % 2) as f32 * (cw + gap);
            let y = y0 + (i / 2) as f32 * row_h - p.scroll_row;
            if y > bottom || y + ch < y0 { continue; }
            let sel = i as i32 == p.sel;
            let lift = if sel { self.sel_ease * ch * 0.02 } else { 0.0 };
            let c = RectangleF::new(x, y - lift, cw, ch);
            let path = round(c, cw * 0.08);
            let r = results.get(i);
            let pic = r.and_then(|r| r.pic.lock().unwrap().clone());
            let cs = g.save();
            g.set_clip_path(&path);
            g.fill_rectangle(&solid(if r.is_none() { fade(accent_c, 30) } else { Color::rgb(226, 228, 234) }), c);
            if let Some(pic) = pic {
                let s = (c.width / pic.width as f32).max(c.height / pic.height as f32);
                g.draw_image_bilinear_shared(&pic, RectangleF::new(c.x + (c.width - pic.width as f32 * s) / 2.0, c.y + (c.height - pic.height as f32 * s) / 2.0, pic.width as f32 * s, pic.height as f32 * s));
            } else if r.is_none() {
                let f = self.fonts.item_bold.clone();
                self.w(g, "More", &f, accent_c, c, Center, Center, false);
            } else if r.map_or(false, |r| r.failed.load(Ordering::SeqCst)) {
                let f = self.fonts.small.clone();
                self.w(g, "Couldn't load", &f, PHONE_SUB, c, Center, Center, false);
            } else {
                spinning_ball(g, c.x + c.width / 2.0, c.y + c.height / 2.0, cw * 0.28, t, false);
                self.spinning = true;
            }
            g.restore(cs);
            g.draw_path(&Pen::new(if sel { accent_c } else { PHONE_LINE }, if sel { 3f32.max(wf * 0.008) } else { 1.0 }), &path);
            self.hits.push((c, i as i32));
        }
        g.restore(st);
    }
    /// RenderPreview: a found picture as the home screen would show it, its credit, and what to do with it.
    pub(crate) fn render_preview(&mut self, g: &mut Graphics, p: &mut Page, w: i32, h: i32, m: f32, top: f32, bottom: f32, _t: f32, th: &Theme) {
        let (wf, hf) = (w as f32, h as f32);
        let accent_c = accent(th);
        let y0 = self.back_button_pub(g, w, h, m, top, accent_c) + hf * 0.006;
        let (bh, gap) = (hf * 0.06, hf * 0.012);
        let by = bottom - hf * 0.01 - p.items.len() as f32 * (bh + gap);
        let small = self.fonts.small.clone();
        let body = p.body.clone().unwrap_or_default();
        let cred_h = g.measure_string_w(&body, &small, (wf - 2.0 * m) as i32).height + 4.0;
        let ph = by - cred_h - gap * 2.0 - y0;
        let pw = (wf - 2.0 * m).min(ph * 0.62);
        let c = RectangleF::new((wf - pw) / 2.0, y0, pw, ph);
        let path = round(c, pw * 0.06);
        if let Some(pic) = p.wall.picture.clone() {
            let st = g.save();
            g.set_clip_path(&path);
            let s = (c.width / pic.width as f32).max(c.height / pic.height as f32);
            g.draw_image_hq_bilinear_shared(&pic, RectangleF::new(c.x + (c.width - pic.width as f32 * s) / 2.0, c.y + (c.height - pic.height as f32 * s) / 2.0, pic.width as f32 * s, pic.height as f32 * s));
            g.restore(st);
        }
        g.draw_path(&Pen::new(PHONE_LINE, 2.0), &path);
        self.w(g, &body, &small, PHONE_SUB, RectangleF::new(m, c.bottom() + gap, wf - 2.0 * m, cred_h), Center, Near, true);
        for i in 0..p.items.len() {
            let it = p.items[i].clone();
            let sel = i as i32 == p.sel;
            let fill = if sel { self.sel_ease } else { 0.0 };
            let r = RectangleF::new(m, by + i as f32 * (bh + gap), wf - 2.0 * m, bh);
            let rp = round(r, bh / 2.0);
            g.fill_path(&solid(mix(PHONE_ROW, accent_c, fill)), &rp);
            g.draw_path(&Pen::new(mix(PHONE_LINE, accent_c, fill), 1.5f32.max(hf * 0.002)), &rp);
            let f = if sel { self.fonts.item_bold.clone() } else { self.fonts.item.clone() };
            self.w(g, &it.text, &f, if it.on { mix(PHONE_TEXT, Color::WHITE, fill) } else { TEXT3 }, r, Center, Center, false);
            self.hits.push((r, i as i32));
        }
    }
}
