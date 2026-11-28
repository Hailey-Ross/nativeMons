//! Ctr3dsSaves: a 3DS game's save folder in state\3ds, and the hand-off to an emulator around a session.

use crate::config;
use crate::emu3ds::Emu3ds;
use crate::game_entry::GameEntry;
use crate::game_options;
use crate::save_guard;
use std::path::{Path, PathBuf};

pub fn root() -> PathBuf { config::state().join("3ds") }
pub fn master(g: &GameEntry) -> PathBuf { root().join("saves").join(&g.id) }
pub fn main(g: &GameEntry) -> PathBuf { master(g).join("data").join("main") }

/// Each emulator's own data folder (settings, system files), linked in only while it runs.
pub fn emu_data(e: &Emu3ds) -> PathBuf { root().join("emu").join(e.id) }
/// Where the emulator looks for its data: "user" beside its program (Citra's portable mode).
fn link(e: &Emu3ds) -> PathBuf { e.dir().join("user") }
fn ctr(g: &GameEntry) -> String { g.ctr.clone().unwrap_or_default() }
fn low(g: &GameEntry) -> String { ctr(g)[8..].to_lowercase() }
fn high(g: &GameEntry) -> String { ctr(g)[..8].to_lowercase() }
/// The emulator's copy of the save folder, and Citra's format file beside it.
fn emu_save(e: &Emu3ds, g: &GameEntry) -> PathBuf {
    if e.native { return emu_data(e).join("user").join("savedata").join(ctr(g).to_uppercase()); }
    let z = "0".repeat(32);
    emu_data(e).join("sdmc").join("Nintendo 3DS").join(&z).join(&z).join("title").join(high(g)).join(low(g)).join("data").join("00000001")
}
fn emu_meta(e: &Emu3ds, g: &GameEntry) -> PathBuf { PathBuf::from(format!("{}{}", emu_save(e, g).display(), if e.native { ".format" } else { ".metadata" })) }
fn emu_states(e: &Emu3ds, g: &GameEntry) -> PathBuf { if e.native { emu_data(e).join("states").join(ctr(g).to_uppercase()) } else { emu_data(e).join("states") } }
fn kept_states(e: &Emu3ds, g: &GameEntry) -> PathBuf { if e.native { master(g).join("states").join("zakuro") } else { master(g).join("states") } }

#[cfg(windows)]
pub fn is_link(dir: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::symlink_metadata(dir).map(|m| m.file_attributes() & 0x10 != 0 && m.file_attributes() & 0x400 != 0).unwrap_or(false)
}
#[cfg(not(windows))]
pub fn is_link(dir: &Path) -> bool { std::fs::symlink_metadata(dir).map(|m| m.file_type().is_symlink()).unwrap_or(false) && dir.is_dir() }
/// A directory junction: no admin rights needed, unlike a symbolic link.
fn make_link(link: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    #[cfg(windows)]
    {
        let psi = crate::proc::StartInfo::new("cmd.exe", &format!("/c mklink /J \"{}\" \"{}\"", link.display(), target.display()));
        let _ = crate::proc::run(&psi, 15);
    }
    #[cfg(not(windows))]
    {
        let _ = std::os::unix::fs::symlink(target, link);
    }
    if !is_link(link) { return Err(io_err("Couldn't link the emulator to its data folder.".into())); }
    Ok(())
}
/// Removes only the link, never what it points to.
pub fn unlink(e: &Emu3ds) -> std::io::Result<()> {
    let l = link(e);
    if is_link(&l) { std::fs::remove_dir(&l)?; }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    let mut dirs = Vec::new();
    for e in std::fs::read_dir(from)?.flatten() {
        let p = e.path();
        if p.is_dir() { dirs.push(p); } else { save_guard::write_in_place(&p, &to.join(e.file_name()))?; }
    }
    for d in dirs { copy_dir(&d, &to.join(d.file_name().unwrap()))?; }
    Ok(())
}
/// Replaces a folder's contents with another's, or empties it when there's no source.
pub fn mirror(from: Option<&Path>, to: &Path) -> std::io::Result<()> {
    if to.is_dir() { std::fs::remove_dir_all(to)?; }
    if let Some(f) = from.filter(|f| f.is_dir()) { copy_dir(f, to)?; }
    Ok(())
}

fn states_of(dir: &Path, g: &GameEntry, native: bool) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let prefix = format!("{}.", ctr(g).to_lowercase());
    rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).filter(|p| {
        let n = p.file_name().unwrap().to_string_lossy().to_lowercase();
        if native { n.ends_with(".state") } else { n.starts_with(&prefix) && n.ends_with(".cst") }
    }).collect()
}
/// When the launcher changes a save, the slot's states go to the backups with the old save.
pub fn stash_states(g: &GameEntry, reason: &str) -> std::io::Result<()> {
    let zakuro = master(g).join("states").join("zakuro");
    let mut states = states_of(&master(g).join("states"), g, false);
    states.extend(states_of(&zakuro, g, true));
    if states.is_empty() { return Ok(()); }
    let to = save_guard::backup_folder(g).join(format!("states-{}-{}", save_guard::now("yyyyMMdd-HHmmss-fff"), reason));
    std::fs::create_dir_all(&to)?;
    for f in states {
        let base = f.file_name().unwrap().to_string_lossy().into_owned();
        let name = if f.starts_with(&zakuro) { format!("zakuro-{}", base) } else { base };
        save_guard::write_in_place(&f, &to.join(name))?;
        std::fs::remove_file(&f)?;
    }
    Ok(())
}

fn session_file(e: &Emu3ds) -> PathBuf { emu_data(e).join("nativemons-session.txt") }
fn io_err(s: String) -> std::io::Error { std::io::Error::new(std::io::ErrorKind::Other, s) }
/// A session the launcher never saw end (a crash, a power cut): its save is collected before anything else.
pub fn collect_leftover(games: &[GameEntry], e: &Emu3ds) -> Option<String> {
    let r = (|| -> std::io::Result<Option<String>> {
        let f = session_file(e);
        if !f.is_file() { return Ok(None); }
        let id = crate::save_sets::read_trim(&f).unwrap_or_default();
        let Some(g) = games.iter().find(|x| x.id == id && x.ctr.is_some() && x.ctr_rom.is_some()) else {
            std::fs::remove_file(&f)?;
            return Ok(None);
        };
        // Collect removes the marker once done, a refused damaged save included; only a real failure keeps it.
        let why = collect(e, g);
        Ok(if f.is_file() { why } else { None })
    })();
    r.unwrap_or_else(|x| Some(x.to_string()))
}
/// Before play: link the emulator to its data, back up, and hand it this slot's save and states.
pub fn prepare(games: &[GameEntry], e: &Emu3ds, g: &GameEntry) -> std::io::Result<()> {
    if let Some(left) = collect_leftover(games, e) { return Err(io_err(format!("The last session's save couldn't be collected first: {}", left))); }
    if e.native {
        std::fs::create_dir_all(emu_data(e))?;
    } else {
        let l = link(e);
        if is_link(&l) {
            std::fs::remove_dir(&l)?;
        } else if l.is_dir() {
            // A real folder there (the emulator was run by hand) is kept aside, never deleted.
            std::fs::rename(&l, format!("{}.kept-{}", l.display(), save_guard::now("yyyyMMdd-HHmmss")))?;
        }
        make_link(&l, &emu_data(e))?;
    }
    std::fs::write(session_file(e), &g.id)?;
    if e.native { write_zakuro_config(e, g)?; } else { write_citra_config(e, g)?; }
    if main(g).is_file() { save_guard::backup(g, Some(&main(g)), "before-play")?; }
    let save = emu_save(e, g);
    if master(g).join("data").is_dir() {
        mirror(Some(&master(g).join("data")), &save)?;
        let meta = master(g).join("metadata");
        if meta.is_file() {
            save_guard::write_in_place(&meta, &emu_meta(e, g))?;
        } else if e.native && save.join("main").is_file() {
            // Without its format record a game sets up a new save over the old one.
            std::fs::write(emu_meta(e, g), [0u8, 0, 4, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0])?;
        }
    } else if save.is_dir() {
        // No save of the launcher's yet: one the emulator still holds would come back, so it's kept in the backups instead.
        let aside = save_guard::backup_folder(g).join(format!("emulator-{}-{}", e.id, save_guard::now("yyyyMMdd-HHmmss")));
        copy_dir(&save, &aside)?;
        std::fs::remove_dir_all(&save)?;
        if emu_meta(e, g).is_file() { std::fs::remove_file(emu_meta(e, g))?; }
    }
    let states = emu_states(e, g);
    std::fs::create_dir_all(&states)?;
    for f in states_of(&states, g, e.native) { std::fs::remove_file(f)?; }
    for f in states_of(&kept_states(e, g), g, e.native) { save_guard::write_in_place(&f, &states.join(f.file_name().unwrap()))?; }
    Ok(())
}
/// After play: take the save and states back, then unlink. None when done, or why the save stayed where it was.
pub fn collect(e: &Emu3ds, g: &GameEntry) -> Option<String> {
    let r = (|| -> std::io::Result<Option<String>> {
        let mut note = None;
        let save = emu_save(e, g);
        let main_ = save.join("main");
        // A save that fails the check never replaces the one in use: it goes to the backups, marked damaged.
        let bad = if main_.is_file() { save_guard::problem_for(Some(g), &main_) } else { None };
        if let Some(bad) = bad {
            let _ = save_guard::backup(g, Some(&main_), &format!("from-{}", e.id));
            std::fs::remove_dir_all(&save)?;
            if emu_meta(e, g).is_file() { std::fs::remove_file(emu_meta(e, g))?; }
            note = Some(format!("The save {} wrote this time failed its check ({}), so it wasn't kept: your previous save is still the one in use. The damaged one is in this game's backups.", e.short, bad));
        } else if save.is_dir() && main_.is_file() {
            mirror(Some(&save), &master(g).join("data"))?;
            if emu_meta(e, g).is_file() { save_guard::write_in_place(&emu_meta(e, g), &master(g).join("metadata"))?; }
        }
        let (states, kept) = (emu_states(e, g), kept_states(e, g));
        std::fs::create_dir_all(&kept)?;
        for f in states_of(&kept, g, e.native) { std::fs::remove_file(f)?; }
        for f in states_of(&states, g, e.native) { save_guard::write_in_place(&f, &kept.join(f.file_name().unwrap()))?; }
        let _ = std::fs::remove_file(session_file(e));
        Ok(note)
    })();
    let _ = unlink(e);
    r.unwrap_or_else(|x| Some(format!("Your save stayed in the emulator's folder and is collected next time: {}", x)))
}

fn read_lines(f: &Path) -> Vec<String> { std::fs::read_to_string(f).map(|t| config::lines(&t).into_iter().map(String::from).collect()).unwrap_or_default() }
/// (n / 100.0).ToString("0.##"): up to two decimals, none when whole.
fn hundredths(v: &str) -> String {
    let n: i32 = v.trim().parse().unwrap_or(0);
    let s = format!("{:.2}", n as f64 / 100.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
/// A window that fits the screen, so its Esc menu shows whole: the biggest whole scale of the layout Zakuro is set to
/// (sizes from zakuro-gpu's ScreenLayout::size_f).
pub fn zakuro_scale(area_w: i32, area_h: i32) -> i32 {
    let file = emu_data(&crate::emu3ds::ZAKURO).join("appdata").join("zakuro").join("settings.toml");
    let layout = read_lines(&file).into_iter().find(|l| l.starts_with("layout =")).map(|l| l[8..].trim().trim_matches('"').to_string()).unwrap_or_else(|| "stacked".into());
    let w = match layout.as_str() { "large_top" => 560, "side_by_side" => 720, "bottom_only" => 320, _ => 400 };
    let h = if layout == "stacked" { 480 } else { 240 };
    ((area_h - 70) / h).min((area_w - 80) / w).clamp(1, 6)
}
fn write_zakuro_config(e: &Emu3ds, g: &GameEntry) -> std::io::Result<()> {
    let file = emu_data(e).join("appdata").join("zakuro").join("settings.toml");
    let mark = emu_data(e).join("nativemons-defaults-3.txt");
    std::fs::create_dir_all(file.parent().unwrap())?;
    let mut lines = read_lines(&file);
    let w = game_options::for_game(g);
    if let Some(v) = w.get("vol") {
        set_toml(&mut lines, None, "volume", &hundredths(v));
        set_toml(&mut lines, None, "mute", "false");
    }
    if let Some(v) = w.get("smooth") { set_toml(&mut lines, None, "filter", if v == "off" { "\"pixels\"" } else { "\"smooth\"" }); }
    // The 60 fps switch on the game's page: Zakuro's enhancement of that name, for this title.
    let fps = if config::read(&format!("zakuro-60fps-{}.txt", g.id), "off") == "on" { "[\"60 FPS\"]" } else { "[]" };
    set_toml(&mut lines, Some("enhancements"), &ctr(g).to_uppercase(), fps);
    // Big top screen, small bottom one by default; a layout picked in Zakuro stays.
    let layout_mark = emu_data(e).join("nativemons-layout-1.txt");
    if !layout_mark.is_file() {
        set_toml(&mut lines, None, "layout", "\"large_top\"");
        std::fs::write(&layout_mark, "large_top")?;
    }
    if !mark.is_file() {
        // ZR off the right trigger, on C: a button Xbox and PlayStation pads don't have.
        for b in [["a", "South"], ["b", "East"], ["x", "West"], ["y", "North"], ["zr", "C"]] { set_toml(&mut lines, Some("pad"), b[0], &format!("\"{}\"", b[1])); }
        std::fs::write(&mark, format!("Controller by letter, set {}", save_guard::now("yyyy-MM-dd")))?;
    }
    config::write_all_lines(&file, &lines)
}
/// One key in a TOML table (None: the top, before any table).
fn set_toml(lines: &mut Vec<String>, table: Option<&str>, key: &str, value: &str) {
    let mut start = 0;
    if let Some(t) = table {
        let head = format!("[{}]", t);
        start = match lines.iter().position(|l| l.trim() == head) {
            Some(i) => i,
            None => {
                if !lines.is_empty() && !lines[lines.len() - 1].trim().is_empty() { lines.push(String::new()); }
                lines.push(head);
                lines.len() - 1
            }
        };
        start += 1;
    }
    let end = lines.iter().skip(start).position(|l| l.trim_start().starts_with('[')).map(|i| i + start).unwrap_or(lines.len());
    for i in start..end {
        let t = lines[i].trim_start();
        if t.starts_with(&format!("{} =", key)) || t.starts_with(&format!("{}=", key)) {
            lines[i] = format!("{} = {}", key, value);
            return;
        }
    }
    let mut at = end;
    while at > start && lines[at - 1].trim().is_empty() { at -= 1; }
    lines.insert(at, format!("{} = {}", key, value));
}
/// One "key=value" in a section of a Qt settings file, with Qt's "\default=false" so the emulator keeps it.
fn set_ini(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let head = format!("[{}]", section);
    let at = match lines.iter().position(|l| l.trim() == head) {
        Some(i) => i,
        None => {
            if !lines.is_empty() && !lines[lines.len() - 1].trim().is_empty() { lines.push(String::new()); }
            lines.push(head);
            lines.len() - 1
        }
    };
    let mut end = lines.iter().skip(at + 1).position(|l| l.starts_with('[')).map(|i| i + at + 1).unwrap_or(lines.len());
    for pair in [[format!("{}\\default", key), "false".to_string()], [key.to_string(), value.to_string()]] {
        let prefix = format!("{}=", pair[0]);
        match (at + 1..end).find(|&i| lines[i].starts_with(&prefix)) {
            Some(i) => lines[i] = format!("{}={}", pair[0], pair[1]),
            None => {
                lines.insert(end, format!("{}={}", pair[0], pair[1]));
                end += 1;
            }
        }
    }
}
// Any controller, as Fulldex has it: each button by the letter printed on it, SDL's standard buttons and axes.
const PAD_MAP: [[&str; 2]; 17] = [
    ["button_a", "button:0"], ["button_b", "button:1"], ["button_x", "button:2"], ["button_y", "button:3"],
    ["button_up", "button:11"], ["button_down", "button:12"], ["button_left", "button:13"], ["button_right", "button:14"],
    ["button_l", "button:9"], ["button_r", "button:10"], ["button_start", "button:6"], ["button_select", "button:4"],
    ["button_zl", "axis:4,direction:+,threshold:0.5"], ["button_zr", "axis:5,direction:+,threshold:0.5"], ["button_home", "button:5"],
    ["circle_pad", "axis_x:0,axis_y:1,deadzone:0.1"], ["c_stick", "axis_x:2,axis_y:3,deadzone:0.1"],
];
// Fulldex's keys, as a second profile (Qt key codes).
const KEY_MAP: [[&str; 2]; 12] = [
    ["button_a", "88"], ["button_b", "90"], ["button_x", "83"], ["button_y", "65"],
    ["button_l", "81"], ["button_r", "87"], ["button_start", "16777220"], ["button_select", "16777219"],
    ["button_up", "16777235"], ["button_down", "16777237"], ["button_left", "16777234"], ["button_right", "16777236"],
];
pub fn config_file(e: &Emu3ds) -> PathBuf { emu_data(e).join("config").join("qt-config.ini") }
/// Azahar's own settings the launcher keeps: no "are you sure" on close, no Discord status of its own, controllers mapped once.
pub fn write_citra_config(e: &Emu3ds, g: &GameEntry) -> std::io::Result<()> {
    let f = config_file(e);
    std::fs::create_dir_all(f.parent().unwrap())?;
    let mut lines = read_lines(&f);
    set_ini(&mut lines, "UI", "confirmClose", "false");
    set_ini(&mut lines, "UI", "enable_discord_presence", "false");
    // The launcher's Volume, every launch; unset = the emulator's own.
    if let Some(vol) = game_options::for_game(g).get("vol") {
        set_ini(&mut lines, "Audio", "volume", &hundredths(vol));
        // A volume given to this game inside the emulator would win over the launcher's: back to its global one.
        let custom = emu_data(e).join("config").join("custom").join(format!("{}.ini", ctr(g).to_uppercase()));
        if custom.is_file() {
            if let Ok(t) = std::fs::read_to_string(&custom) { let _ = std::fs::write(&custom, t.replace("volume\\use_global=false", "volume\\use_global=true")); }
        }
    }
    // Once per version of these defaults, so the player's own changes stay afterwards.
    let marker = emu_data(e).join("nativemons-defaults-3.txt");
    if !marker.is_file() {
        for m in PAD_MAP {
            let joined = format!("api:controller,engine:sdl,guid:0,maptype:all,port:0,{}", m[1]);
            let mut parts: Vec<&str> = joined.split(',').collect();
            parts.sort_by(|a, b| save_guard::ordinal(a, b));
            set_ini(&mut lines, "Controls", &format!("profiles\\1\\{}", m[0]), &format!("\"{}\"", parts.join(",")));
        }
        // 0 = all controllers (Settings::InputMappingType).
        set_ini(&mut lines, "Controls", "profiles\\1\\input_maptype", "0");
        set_ini(&mut lines, "Controls", "profiles\\1\\name", "Controller (Fulldex layout)");
        for k in KEY_MAP { set_ini(&mut lines, "Controls", &format!("profiles\\2\\{}", k[0]), &format!("\"code:{},engine:keyboard\"", k[1])); }
        set_ini(&mut lines, "Controls", "profiles\\2\\name", "Keyboard (Fulldex keys)");
        let have = lines.iter().find(|l| l.starts_with("profiles\\size=")).and_then(|s| s[14..].trim().parse::<i32>().ok());
        if have.map_or(true, |h| h < 2) { set_ini(&mut lines, "Controls", "profiles\\size", "2"); }
        // Vulkan (2), and shaders compiled in the background so a new effect doesn't freeze the game.
        set_ini(&mut lines, "Renderer", "graphics_api", "2");
        set_ini(&mut lines, "Renderer", "async_shader_compilation", "true");
        std::fs::write(&marker, "Fulldex's controller and keyboard layouts, Vulkan and async shaders written by nativeMons. Delete this file to have them written again.")?;
        for old in ["nativemons-controls.txt", "nativemons-defaults-2.txt"] { let _ = std::fs::remove_file(emu_data(e).join(old)); }
    }
    config::write_all_lines(&f, &lines)
}
