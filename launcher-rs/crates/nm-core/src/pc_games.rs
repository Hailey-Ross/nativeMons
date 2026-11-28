//! PcGames: games that are PC programs of their own (Infinite Fusion, the player's own): where they are, their saves,
//! and what their pages show. Installing, updating and removing come with the play flows.

use crate::config;
use crate::custom_games;
use crate::game_entry::GameEntry;
use std::path::{Path, PathBuf};

pub struct Profile {
    pub name: &'static str,
    pub repo: &'static str,
    pub branch: &'static str,
    pub exe: &'static str,
    pub save_folder: &'static str,
    pub size: &'static str,
    pub own_exe: fn() -> PathBuf,
    /// Its title screen's pictures under Graphics (GameArt.CapturePc): the backdrop layers (one drifting), the swirl and
    /// logo, and the town (a panorama or map).
    pub title_bg: &'static [&'static str],
    pub drift: i32,
    pub title_swirl: &'static str,
    pub title_logo: &'static str,
    pub town: &'static str,
}
pub fn of(g: &GameEntry) -> Option<Profile> {
    match g.pc.as_deref() {
        Some("infinitefusion") => Some(Profile { name: "Pokemon Infinite Fusion", repo: "https://github.com/infinitefusion/infinitefusion-e18.git", branch: "releases", exe: "Game.exe", save_folder: "infinitefusion", size: "about 5 GB", own_exe: config::infinite_fusion,
            title_bg: &["Titles\\gen1_bg_v5.png"], drift: -1, title_swirl: "Titles\\PokemonInfiniteFusionLogo_Back_25.png", title_logo: "Titles\\PokemonInfiniteFusionLogo_Main_25.png", town: "Panoramas\\Kanto_HGSS.png" }),
        Some("infinitefusion2") => Some(Profile { name: "Pokemon Infinite Fusion 2", repo: "https://github.com/infinitefusion/infinitefusion-hoenn-public.git", branch: "releases", exe: "InfiniteFusion2.exe", save_folder: "infinitefusion-hoenn", size: "about 3 GB", own_exe: config::infinite_fusion2,
            title_bg: &["Titles\\bg_back.png", "Titles\\bg_clouds.png", "Titles\\bg_front.png"], drift: 1, title_swirl: "Titles\\PokemonInfiniteFusionLogo_Back_25.png", title_logo: "Titles\\InfiniteFusionHoenn.png", town: "Pictures\\map\\map_hoenn.png" }),
        _ => None,
    }
}
pub fn is_fusion(g: &GameEntry) -> bool { of(g).is_some() }
/// state\pc-<id>.txt names the player's own installation, run where it is.
pub fn existing(g: &GameEntry) -> Option<String> { let d = config::read(&format!("pc-{}.txt", g.id), ""); if d.trim().is_empty() { None } else { Some(d.trim().to_string()) } }
pub fn is_existing(g: &GameEntry) -> bool { existing(g).is_some() }
pub fn is_custom(g: &GameEntry) -> bool { g.pc.as_deref() == Some("custom") }
fn own_exe(g: &GameEntry) -> Option<PathBuf> { of(g).map(|p| (p.own_exe)()) }
pub fn exe(g: &GameEntry) -> Option<String> {
    if is_custom(g) { return custom_games::find(&g.id).map(|c| custom_games::exe(&c)); }
    let p = of(g)?;
    match existing(g) {
        Some(e) => Some(PathBuf::from(e).join(p.exe).to_string_lossy().into_owned()),
        None => Some((p.own_exe)().to_string_lossy().into_owned()),
    }
}
fn parent(f: &str) -> String { Path::new(f).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default() }
/// Dir: the folder its program is in.
pub fn dir(g: &GameEntry) -> Option<String> { exe(g).map(|e| parent(&e)) }
/// OwnDir: the launcher's own copy's folder.
pub fn own_dir(g: &GameEntry) -> Option<String> { own_exe(g).map(|e| parent(&e.to_string_lossy())) }
pub fn installed(g: &GameEntry) -> bool { exe(g).map_or(false, |e| Path::new(&e).is_file()) }
/// GameSaveDir: where the game keeps its saves itself (the roaming application data folder).
/// NATIVEMONS_TEST_PC_SAVES (tests only) stands in for the roaming folder, so a test never reads the player's saves.
pub fn game_save_dir(g: &GameEntry) -> PathBuf {
    let root = std::env::var_os("NATIVEMONS_TEST_PC_SAVES").map(PathBuf::from).unwrap_or_else(crate::paths::roaming);
    root.join(of(g).map(|p| p.save_folder).unwrap_or(""))
}
/// SaveDir: an existing installation's own folder, else beside the launcher (state\saves\<id>).
pub fn save_dir(g: &GameEntry) -> PathBuf { if is_existing(g) { game_save_dir(g) } else { config::state().join("saves").join(&g.id) } }
/// Git: git.exe on the PATH, else Git for Windows' usual place.
pub fn git() -> Option<PathBuf> {
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    let sep = if cfg!(windows) { ';' } else { ':' };
    for d in std::env::var("PATH").unwrap_or_default().split(sep) {
        let d = d.trim();
        if !d.is_empty() && Path::new(d).join(name).is_file() { return Some(Path::new(d).join(name)); }
    }
    if cfg!(windows) {
        let pf = PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or_default()).join("Git").join("cmd").join("git.exe");
        if pf.is_file() { return Some(pf); }
    }
    None
}
/// Version: an imported copy's last commit subject (git log -1). A custom game's file version isn't read yet.
pub fn version(g: &GameEntry) -> Option<String> {
    if is_custom(g) { return None; }
    let d = dir(g)?;
    let git = git()?;
    if !installed(g) || !Path::new(&d).join(".git").is_dir() { return None; }
    let mut psi = crate::proc::StartInfo::new(git, "log -1 --format=%s");
    psi.cwd = Some(PathBuf::from(&d));
    psi.set_env("GIT_TERMINAL_PROMPT", "0");
    let f = crate::proc::run(&psi, 30).ok()?;
    f.out.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

// ---------- install, update, remove (PcGames.AddExisting, Install, Update, Remove) ----------
fn existing_file(g: &GameEntry) -> PathBuf { config::state().join(format!("pc-{}.txt", g.id)) }
/// The profile's repository; NATIVEMONS_TEST_PC_REPO (tests only) points it at a local one.
fn repo(p: &Profile) -> String { std::env::var("NATIVEMONS_TEST_PC_REPO").ok().filter(|r| !r.is_empty()).unwrap_or_else(|| p.repo.to_string()) }
fn same_dir(a: &str, b: &str) -> bool {
    let f = |s: &str| crate::paths::full_path(Path::new(s)).to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    f(a).eq_ignore_ascii_case(&f(b))
}
/// AddExisting: the player's own installation, run where it is (state\pc-<id>.txt names it).
pub fn add_existing(g: &GameEntry, folder: &str) -> Option<String> {
    let pf = of(g)?;
    if !Path::new(folder).join(pf.exe).is_file() { return Some(format!("That folder has no {}, so it isn't {}.", pf.exe, pf.name)); }
    if same_dir(folder, &own_dir(g).unwrap_or_default()) { return Some("That is the launcher's own copy.".into()); }
    let _ = std::fs::create_dir_all(config::state());
    std::fs::write(existing_file(g), crate::paths::full_path(Path::new(folder)).to_string_lossy().as_bytes()).err().map(|e| e.to_string())
}
/// Forget: an existing installation forgotten; nothing of it is deleted.
pub fn forget(g: &GameEntry) { let _ = std::fs::remove_file(existing_file(g)); }
/// Remove: a folder deleted, git's read-only object files included.
pub fn remove(dir: &Path) -> std::io::Result<()> { custom_games::remove_dir(dir) }
/// Room: the drive has the bytes and 1 GB more (true when it can't be read).
fn room(target: &str, bytes: u64) -> bool {
    match free_space(&crate::paths::full_path(Path::new(target))) { Some(free) => free > bytes + (1u64 << 30), None => true }
}
#[cfg(windows)]
fn free_space(p: &Path) -> Option<u64> {
    #[link(name = "kernel32")]
    extern "system" { fn GetDiskFreeSpaceExW(root: *const u16, free: *mut u64, total: *mut u64, total_free: *mut u64) -> i32; }
    let s = p.to_string_lossy();
    if s.len() < 2 || s.as_bytes()[1] != b':' { return None; }
    let w: Vec<u16> = format!("{}\\", &s[..2]).encode_utf16().chain(std::iter::once(0)).collect();
    let (mut free, mut total, mut tf) = (0u64, 0u64, 0u64);
    if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, &mut tf) } != 0 { Some(free) } else { None }
}
#[cfg(not(windows))]
fn free_space(p: &Path) -> Option<u64> {
    let mut d = p.to_path_buf();
    while !d.exists() { if !d.pop() { return None; } }
    let o = std::process::Command::new("df").arg("-Pk").arg(&d).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout).into_owned();
    t.lines().nth(1)?.split_whitespace().nth(3)?.parse::<u64>().ok().map(|k| k * 1024)
}
/// RunGit: git with its output and progress lines (split as .NET's ReadLine splits) passed to `line`.
fn run_git(args: &str, dir: Option<&Path>, line: &mut dyn FnMut(&str)) -> std::io::Result<i32> {
    use std::io::Read;
    let git = git().ok_or_else(|| std::io::Error::other("no git"))?;
    let mut psi = crate::proc::StartInfo::new(git, args);
    psi.cwd = dir.map(Path::to_path_buf);
    psi.set_env("GIT_TERMINAL_PROMPT", "0");
    let mut child = crate::proc::command_for(&psi).spawn()?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    let outs: [Box<dyn Read + Send>; 2] = [Box::new(child.stdout.take().unwrap()), Box::new(child.stderr.take().unwrap())];
    for mut r in outs {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let (mut buf, mut cur, mut cr) = ([0u8; 4096], Vec::new(), false);
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 { break; }
                for &b in &buf[..n] {
                    if b == b'\n' && cr { cr = false; continue; }
                    cr = b == b'\r';
                    if b == b'\n' || b == b'\r' { let _ = tx.send(String::from_utf8_lossy(&std::mem::take(&mut cur)).into_owned()); } else { cur.push(b); }
                }
            }
            if !cur.is_empty() { let _ = tx.send(String::from_utf8_lossy(&cur).into_owned()); }
        }));
    }
    drop(tx);
    for l in rx { line(&l); }
    for r in readers { let _ = r.join(); }
    Ok(child.wait()?.code().unwrap_or(-1))
}
fn rxdata(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().map_or(false, |x| x.eq_ignore_ascii_case("rxdata"))).collect()).unwrap_or_default();
    v.sort();
    v
}
/// BringSaves: saves already in the game's usual folder are copied in; the originals stay.
fn bring_saves(g: &GameEntry) -> std::io::Result<Option<String>> {
    let (from, to) = (game_save_dir(g), save_dir(g));
    std::fs::create_dir_all(&to)?;
    if !from.is_dir() || !rxdata(&to).is_empty() { return Ok(None); }
    let files = rxdata(&from);
    for f in &files { std::fs::copy(f, to.join(f.file_name().unwrap()))?; }
    let n = files.len();
    Ok(if n == 0 { None } else { Some(format!("{} copied in from {}; the originals are still there.", if n == 1 { "1 save was".to_string() } else { format!("{} saves were", n) }, from.display())) })
}
/// Install: from GitHub (source None), a .zip or a folder, into <own>.installing, which replaces the old copy only once
/// complete. (problem, what happened to the saves: PcGames.Saved). `report` gets each step and the share done (-1).
pub fn install(g: &GameEntry, source: Option<&str>, report: &mut dyn FnMut(&str, f32)) -> (Option<String>, Option<String>) {
    let Some(pf) = of(g) else { return (Some("That isn't a PC game the launcher installs.".into()), None) };
    let target = own_dir(g).unwrap_or_default();
    let work = PathBuf::from(format!("{}.installing", target));
    let r = install_into(g, &pf, source, &target, &work, report);
    if work.is_dir() { let _ = remove(&work); }
    r.unwrap_or_else(|e| (Some(format!("Installing stopped: {} The copy you had (if any) is unchanged.", e)), None))
}
type Installed = Result<(Option<String>, Option<String>), String>;
fn install_into(g: &GameEntry, pf: &Profile, source: Option<&str>, target: &str, work: &Path, report: &mut dyn FnMut(&str, f32)) -> Installed {
    let e = |x: std::io::Error| x.to_string();
    let fail = |m: String| -> Installed { Ok((Some(m), None)) };
    if work.is_dir() { remove(work).map_err(e)?; }
    match source {
        None => {
            if git().is_none() { return fail("Downloading needs Git for Windows (git-scm.com). Or install from your own copy of the game.".into()); }
            std::fs::create_dir_all(work.parent().unwrap()).map_err(e)?;
            report(&format!("Downloading {} from GitHub ({})", pf.name, pf.size), -1.0);
            let code = run_git(&format!("clone --depth=1 --branch {} --progress {} \"{}\"", pf.branch, repo(pf), work.display()), None, &mut |l| report(l, -1.0)).map_err(e)?;
            if code != 0 { return fail(format!("The download stopped (git said {}). Nothing was changed.", code)); }
        }
        Some(src) if Path::new(src).is_file() => {
            let mut a = zip::ZipArchive::new(std::fs::File::open(src).map_err(e)?).map_err(|x| x.to_string())?;
            let mut entries = Vec::new();
            for i in 0..a.len() { let z = a.by_index(i).map_err(|x| x.to_string())?; entries.push((i, z.name().to_string(), z.size())); }
            let leaf = |n: &str| n.rsplit(['/', '\\']).next().unwrap_or("").to_string();
            // The game's own zip has its files at the top; one inside a single folder is unwrapped.
            let Some(exe) = entries.iter().filter(|x| leaf(&x.1).eq_ignore_ascii_case(pf.exe)).min_by_key(|x| x.1.encode_utf16().count()) else {
                return fail(format!("That zip has no {}, so it isn't {}.", pf.exe, pf.name));
            };
            let prefix = exe.1[..exe.1.len() - leaf(&exe.1).len()].to_string();
            let files: Vec<_> = entries.iter().filter(|x| !leaf(&x.1).is_empty() && x.1.starts_with(&prefix)).cloned().collect();
            let total = files.iter().map(|x| x.2).sum::<u64>().max(1);
            if !room(target, total) { return fail(format!("There isn't room for it on that drive: it needs about {} GB.", total >> 30)); }
            let mut done = 0u64;
            for (i, name, size) in &files {
                let rel = &name[prefix.len()..];
                // Rust only: an entry climbing out of the folder stops the install (the C# would write it there).
                if rel.split(['/', '\\']).any(|c| c == "..") { return Err(format!("It holds a file meant for outside its folder ({}).", name)); }
                let to = work.join(crate::paths::rel(&rel.replace('/', "\\")));
                std::fs::create_dir_all(to.parent().unwrap()).map_err(e)?;
                crate::zips::extract_entry(&mut a, *i, &to)?;
                done += size;
                report(&format!("Unpacking {}", rel), done as f32 / total as f32);
            }
        }
        Some(src) if Path::new(src).is_dir() => {
            if !Path::new(src).join(pf.exe).is_file() { return fail(format!("That folder has no {}, so it isn't {}.", pf.exe, pf.name)); }
            if same_dir(src, target) { return fail("That is the launcher's own copy.".into()); }
            let files = crate::library::files_under(src);
            let total = files.iter().map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0)).sum::<u64>().max(1);
            if !room(target, total) { return fail(format!("There isn't room for it on that drive: it needs about {} GB.", total >> 30)); }
            let base = src.trim_end_matches(['\\', '/']).len() + 1;
            let mut done = 0u64;
            for f in &files {
                let rel = &f[base..];
                let to = work.join(rel);
                std::fs::create_dir_all(to.parent().unwrap()).map_err(e)?;
                std::fs::copy(f, &to).map_err(e)?;
                if let Ok(m) = std::fs::metadata(&to) {
                    let mut p = m.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    p.set_readonly(false);
                    let _ = std::fs::set_permissions(&to, p);
                }
                done += std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
                report(&format!("Copying {}", rel), done as f32 / total as f32);
            }
        }
        Some(_) => return fail("That file or folder isn't there any more.".into()),
    }
    if !work.join(pf.exe).is_file() { return fail(format!("{} didn't arrive, so nothing was changed.", pf.exe)); }
    report("Finishing", 1.0);
    if Path::new(target).is_dir() { remove(Path::new(target)).map_err(e)?; }
    std::fs::rename(work, target).map_err(e)?;
    // From now on the launcher's copy is the one played, with its saves in the launcher's folder.
    forget(g);
    Ok((None, bring_saves(g).map_err(e)?))
}
/// Update: a shallow fetch and hard reset, as the game's own updater does. Only the launcher's own copy.
pub fn update(g: &GameEntry, line: &mut dyn FnMut(&str)) -> Option<String> {
    if is_existing(g) { return Some("This is your own installation, so the launcher leaves it alone. Use the game's own updater, or import a copy into the launcher.".into()); }
    let pf = of(g)?;
    let dir = PathBuf::from(own_dir(g).unwrap_or_default());
    if git().is_none() { return Some("Updating needs Git for Windows (git-scm.com).".into()); }
    if !dir.join(".git").is_dir() { return Some("This copy has no update history (.git). Install it again from GitHub to get updates.".into()); }
    let _ = std::fs::remove_file(dir.join(".git").join("shallow.lock"));
    let _ = run_git(&format!("remote set-url origin {}", repo(&pf)), Some(&dir), &mut |_| {});
    if run_git(&format!("fetch --depth=1 --progress origin {}", pf.branch), Some(&dir), line).unwrap_or(-1) != 0 { return Some("Couldn't reach the game's GitHub. Nothing was changed.".into()); }
    if run_git(&format!("reset --hard origin/{}", pf.branch), Some(&dir), line).unwrap_or(-1) != 0 { return Some("The update didn't apply cleanly. Install it again to repair it.".into()); }
    None
}
