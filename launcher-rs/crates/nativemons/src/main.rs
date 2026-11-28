//! nativeMons launcher (Rust). So far the windowless modes: `--snapshot` and the CLI modes Main checks first
//! (same arguments, outputs and exit codes as the C# Main; see cli()).
#![cfg_attr(all(windows, feature = "shipped"), windows_subsystem = "windows")]

#[cfg(windows)]
fn work_area() -> (i32, i32) {
    #[repr(C)]
    struct Rect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(action: u32, param: u32, out: *mut Rect, ini: u32) -> i32;
    }
    let mut r = Rect { l: 0, t: 0, r: 0, b: 0 };
    // SPI_GETWORKAREA: the primary screen's working area, as Screen.WorkingArea reports it to a DPI-unaware exe.
    unsafe { SystemParametersInfoW(0x0030, 0, &mut r, 0) };
    (r.r - r.l, r.b - r.t)
}

#[cfg(not(windows))]
fn work_area() -> (i32, i32) { (1920, 1040) }

#[cfg_attr(not(feature = "window"), allow(dead_code, unused_imports))]
#[cfg_attr(not(feature = "window"), allow(dead_code))]
mod live;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    nm_core::config::load();
    nm_ui::play_snaps::set_hooks(nm_ui::play_snaps::Hooks { grab: nm_platform::snaps::grab_client, waiter: nm_platform::snaps::exit_waiter, lower: nm_platform::snaps::lower_this_thread });
    if let Some(code) = cli(&a[1..]) { std::process::exit(code); }
    if a.len() >= 3 && a[1] == "--snapshot" {
        let (mut w, mut h) = (600, 1000);
        if a.len() > 4 {
            let wh: Vec<&str> = a[4].split('x').collect();
            w = wh[0].parse().unwrap_or(600);
            h = wh[1].parse().unwrap_or(1000);
        }
        let screen = a.get(3).map(|s| s.as_str()).unwrap_or("main");
        let host = nm_ui::launcher::Host::new(work_area());
        if let Err(e) = nm_ui::snapshot::snapshot(std::path::Path::new(&a[2]), screen, w, h, host) {
            eprintln!("{}", e);
            std::process::exit(2);
        }
        return;
    }
    // --play-snap-test <result> <window.png>...: PlaySnaps.Judge on each picture, as the C# mode.
    if a.len() >= 4 && a[1] == "--play-snap-test" {
        if let Err(e) = nm_ui::play_snaps::play_snap_test(std::path::Path::new(&a[2]), &a[3..]) { eprintln!("{}", e); std::process::exit(2); }
        return;
    }
    // --play-snap-grab <pid> <out folder>: PlaySnaps' grab of that process's window, then Judge (window test only).
    if a.len() >= 4 && a[1] == "--play-snap-grab" {
        let out = std::path::Path::new(&a[3]);
        let _ = std::fs::create_dir_all(out);
        let line = match a[2].parse::<u32>().ok().and_then(nm_platform::snaps::grab_client) {
            None => "grab: none (no window in front, minimized, or under 400x300)".to_string(),
            Some((w, h, data)) => {
                let mut b = nm_draw::Bitmap::new(w, h);
                b.data = data;
                let _ = b.save_png(&out.join("grab.png"));
                let (pic, why) = nm_ui::play_snaps::judge(&b, 0);
                if let Some((p, _)) = pic { let _ = p.save_png(&out.join("top.png")); }
                format!("grab {}x{}: {}", w, h, why)
            }
        };
        let _ = std::fs::write(out.join("result.txt"), &line);
        println!("{}", line);
        return;
    }
    // --replay <script> <out folder> [WxH]: a scripted input sequence, offscreen (nm_ui::replay).
    if a.len() >= 4 && a[1] == "--replay" {
        let wh: Vec<&str> = a.get(4).map(|s| s.as_str()).unwrap_or("600x1000").split('x').collect();
        let (w, h) = (wh[0].parse().unwrap_or(600), wh.get(1).and_then(|v| v.parse().ok()).unwrap_or(1000));
        let host = nm_ui::launcher::Host::new(work_area());
        if let Err(e) = nm_ui::replay::replay(std::path::Path::new(&a[2]), std::path::Path::new(&a[3]), w, h, host) {
            eprintln!("{}", e);
            std::process::exit(2);
        }
        return;
    }
    // --bench <screen> <WxH> <frames> [still]: render timings, offscreen.
    if a.len() >= 5 && a[1] == "--bench" {
        let wh: Vec<&str> = a[3].split('x').collect();
        let (w, h) = (wh[0].parse().unwrap_or(600), wh.get(1).and_then(|v| v.parse().ok()).unwrap_or(1000));
        let host = nm_ui::launcher::Host::new(work_area());
        match nm_ui::snapshot::bench(&a[2], w, h, a[4].parse().unwrap_or(60), a.get(5).map_or(false, |s| s == "still"), host) {
            Ok(b) => {
                let n = b.full.len().max(1);
                for (name, ms, k) in nm_draw::prof::take() { println!("  {:<16} {:8.3} ms/frame {:8} calls/frame", name, ms / n as f64, k / n as u64); }
                let stats = |v: &[f64]| { let mut v = v.to_vec(); v.sort_by(|x, y| x.partial_cmp(y).unwrap()); let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize]; (at(0.5), v.iter().sum::<f64>() / v.len() as f64, at(0.95)) };
                let (fm, fa, fp) = stats(&b.full);
                let (lm, la, lp) = stats(&b.live);
                println!("{}\t{}x{}\tframes {}\tfull median {:.2} mean {:.2} p95 {:.2}\tlive median {:.2} mean {:.2} p95 {:.2}\tredrawn {:.1}%\tdiffer {}", a[2], w, h, n, fm, fa, fp, lm, la, lp, b.redrawn * 100.0, b.differ);
                let (sn, sb, cn, cb) = nm_draw::graphics::cache_stats();
                println!("caches: {} shapes {:.1} MB, {} clips {:.1} MB, {} glyphs", sn, sb as f64 / 1048576.0, cn, cb as f64 / 1048576.0, nm_draw::text::glyph_cache_len());
            }
            Err(e) => { eprintln!("{}", e); std::process::exit(2); }
        }        return;
    }
    // --snapshot-anim <folder> <screen> <WxH> <t0> <t1> <fps>
    if a.len() >= 8 && a[1] == "--snapshot-anim" {
        let wh: Vec<&str> = a[4].split('x').collect();
        let (w, h) = (wh[0].parse().unwrap_or(600), wh.get(1).and_then(|v| v.parse().ok()).unwrap_or(1000));
        let f = |s: &str| s.parse::<f32>().unwrap_or(0.0);
        let host = nm_ui::launcher::Host::new(work_area());
        if let Err(e) = nm_ui::snapshot::snapshot_anim(std::path::Path::new(&a[2]), &a[3], w, h, f(&a[5]), f(&a[6]), f(&a[7]), host) {
            eprintln!("{}", e);
            std::process::exit(2);
        }
        return;
    }
    // --live-headless <seconds>: the window test's run without a window (tools/window-test-flows.ps1 -Headless).
    if a.len() >= 3 && a[1] == "--live-headless" { std::process::exit(live::run_headless(a[2].parse().unwrap_or(20.0))); }
    #[cfg(feature = "window")]
    std::process::exit(live::run(&a[1..]));
    #[cfg(not(feature = "window"))]
    {
        eprintln!("nativemons: built without the window (cargo feature `window`); only --snapshot and the test modes.");
        std::process::exit(1);
    }
}

/// The C# Main's windowless modes, in its order (the first match wins). Some(exit code) when one ran.
fn cli(argv: &[String]) -> Option<i32> {
    use std::path::Path;
    let n = argv.len();
    let is = |name: &str| n > 0 && argv[0] == name;
    // --3ds-config <result> <emulator> <game>: the emulator settings the launcher writes, with no emulator run.
    if n == 4 && is("--3ds-config") {
        use nm_core::{ctr3ds_saves, emu3ds};
        let lib = scan()?;
        let e = emu3ds::ALL.iter().find(|x| x.id == argv[2]);
        let g = lib.games.iter().find(|x| x.id == argv[3]);
        match (e, g) {
            (Some(e), Some(g)) if g.ctr.is_some() => {
                if let Err(x) = ctr3ds_saves::write_citra_config(e, g) { return Some(crash(&x.to_string())); }
                if let Err(x) = std::fs::copy(ctr3ds_saves::config_file(e), &argv[1]) { return Some(crash(&x.to_string())); }
            }
            _ => { let _ = std::fs::write(&argv[1], "no such emulator or 3DS game"); }
        }
        return Some(0);
    }
    // --3ds-save <result> <game> <emulator> prepare|collect: the save hand-off around a session, with no emulator run.
    if n == 5 && is("--3ds-save") {
        use nm_core::{ctr3ds_saves, emu3ds};
        let lib = scan()?;
        let g = lib.games.iter().find(|x| x.id == argv[2]);
        let e = if argv[3] == "zakuro" { Some(emu3ds::ZAKURO) } else { emu3ds::ALL.iter().copied().find(|x| x.id == argv[3]) };
        let result = match (g, e) {
            (Some(g), Some(e)) if g.ctr_rom.is_some() => {
                if argv[4] == "prepare" { match ctr3ds_saves::prepare(&lib.games, &e, g) { Ok(()) => "ok".to_string(), Err(x) => x.to_string() } } else { ctr3ds_saves::collect(&e, g).unwrap_or_else(|| "ok".into()) }
            }
            _ => "no such 3DS game with a ROM, or emulator".to_string(),
        };
        let _ = std::fs::write(&argv[1], result);
        return Some(0);
    }
    // --submit-hints <result> <code id> <list file>: sends a list as Share does (NATIVEMONS_SUBMIT_URL for a test server).
    if n == 4 && is("--submit-hints") {
        let result = match std::fs::read_to_string(&argv[3]) {
            Err(e) => format!("failed: {}", nm_core::zips::missing(&e, &argv[3])),
            Ok(t) => {
                let lines: Vec<String> = nm_core::config::lines(&t).into_iter().map(String::from).collect();
                match nm_core::submit::zakuro_hints(&argv[2], &lines) { Ok(r) => format!("receipt {}", r), Err(x) => format!("failed: {}", x) }
            }
        };
        let _ = std::fs::write(&argv[1], result);
        return Some(0);
    }
    // --recomp-hints <result>: downloads the shared lists that changed, as start-up does.
    if n == 2 && is("--recomp-hints") {
        use nm_core::{recomp, save_guard};
        let why = recomp::check_shared_hints();
        let list: Vec<String> = recomp::shared_hint_files().iter().map(|f| format!("{}={}", nm_core::library::file_name(&f.to_string_lossy()), &save_guard::hash_of(f)[..8])).collect();
        let _ = std::fs::write(&argv[1], why.unwrap_or_else(|| format!("ok {}", list.join(" "))));
        return Some(0);
    }
    // --recomp <result> <game> state|build: 3DSrecomp+Zakuro with no window (build: Idle, the Settings cores).
    if n == 4 && is("--recomp") {
        use nm_core::recomp;
        let lib = scan()?;
        let g = lib.games.iter().find(|x| x.id == argv[2]);
        let result = match g {
            Some(g) if g.on_recomp() => {
                if argv[3] == "state" { format!("{} {}", recomp::state(g), g.ctr_plain_rom.as_deref().unwrap_or("")) }
                else { recomp::ensure_zig().unwrap_or_else(|| recomp_build(g)) }
            }
            _ => format!("not available: {}", match g { None => "no such game", Some(_) if !recomp::installed() => "engine missing", Some(g) if !recomp::supports(g) => "not supported", _ => "no decrypted dump" }),
        };
        let _ = std::fs::write(&argv[1], result);
        return Some(0);
    }
    if n == 3 && (is("--install-emu") || is("--remove-emu")) {
        use nm_core::emu3ds;
        let result = match emu3ds::ALL.iter().find(|x| x.id == argv[2]) {
            None => format!("unknown emulator {}", argv[2]),
            Some(e) => {
                let r = if argv[0] == "--remove-emu" { emu3ds::remove(e) } else if !emu3ds::can_download(e) { Some("not offered yet".into()) } else { emu3ds::install(e) };
                r.unwrap_or_else(|| format!("ok {}", if e.installed() { e.installed_tag().unwrap_or_default() } else { "removed".into() }))
            }
        };
        let _ = std::fs::write(&argv[1], result);
        return Some(0);
    }
    // --add-pc <result> folder|zip <path> [program number]
    if n >= 4 && is("--add-pc") {
        use nm_core::custom_games as cg;
        let mut o: Vec<String> = Vec::new();
        let zip = argv[2] == "zip";
        let (mut problem, work, programs) = if zip { cg::unpack(&argv[3]) } else { (None, None, cg::programs(&argv[3])) };
        if problem.is_none() && programs.is_empty() { problem = Some("no program".into()); }
        let mut pick = 0usize;
        if problem.is_none() && programs.len() > 1 {
            match argv.get(4).and_then(|s| s.trim().parse::<i64>().ok()) {
                Some(p) => {
                    if p < 0 || p as usize >= programs.len() { return Some(crash("Index was out of range.")); }
                    pick = p as usize;
                }
                None => {
                    for (i, p) in programs.iter().enumerate() { o.push(format!("program {}\t{}", i, p)); }
                    if let Some(w) = &work { let _ = cg::remove_dir(Path::new(w)); }
                    problem = Some("choose".into());
                }
            }
        }
        let mut added = None;
        if problem.is_none() {
            let r = if zip { cg::add_unpacked(&[], &argv[3], work.as_deref().unwrap_or(""), &programs[pick]) } else { cg::add(&[], &programs[pick], false) };
            match r { Ok(e) => added = Some(e), Err(x) => problem = Some(x) }
        }
        let first = match (&problem, &added) {
            (None, Some(e)) => format!("added\t{}\t{}\t{}\tcopy={}", e.id.as_deref().unwrap_or(""), e.title.as_deref().unwrap_or(""), cg::exe(e), if e.copy { "True" } else { "False" }),
            _ => format!("not added\t{}", problem.unwrap_or_default()),
        };
        o.insert(0, first);
        let _ = nm_core::config::write_all_lines(Path::new(&argv[1]), &o);
        return Some(0);
    }
    // --3ds-header <result> <dump> check|fix|undo: the header fixer, on a copy.
    if n == 4 && is("--3ds-header") {
        use nm_core::ctr_header_fix as h;
        let result = if argv[3] == "check" {
            match h::needed(&argv[2]) {
                Err(e) => return Some(crash(&e.to_string())),
                Ok(None) => "still encrypted".to_string(),
                Ok(Some(l)) if l.is_empty() => "fine".to_string(),
                Ok(Some(l)) => format!("needs {}", l.iter().map(|x| format!("{:X}:{:02X}>{:02X}", x.at, x.was, x.now)).collect::<Vec<_>>().join(" ")),
            }
        } else {
            (if argv[3] == "fix" { h::fix(&argv[2]) } else { h::undo(&argv[2]) }).unwrap_or_else(|| "ok".into())
        };
        let _ = std::fs::write(&argv[1], result);
        return Some(0);
    }
    if n >= 3 && is("--add-roms") {
        use nm_core::{adding, library};
        let paths: Vec<String> = argv[2..].to_vec();
        let folders: Vec<String> = paths.iter().filter(|x| Path::new(x).is_dir()).cloned().collect();
        let roms: Vec<String> = paths.iter().filter(|x| Path::new(x).is_file() && library::is_rom(x)).cloned().collect();
        let zips: Vec<String> = paths.iter().filter(|x| Path::new(x).is_file() && x.to_lowercase().ends_with(".zip")).cloned().collect();
        let (mut added, mut notes) = (Vec::new(), Vec::new());
        let lib = match adding::add_paths(&folders, &roms, &zips, &mut added, &mut notes) { Ok(l) => l, Err(e) => return Some(crash(&e)) };
        let mut out: Vec<String> = added.iter().map(|f| format!("{}	{}", f, adding::what_rom_is(&lib, f))).collect();
        out.extend(notes);
        let _ = nm_core::config::write_all_lines(Path::new(&argv[1]), &out);
        return Some(0);
    }
    // --install-index-mod <game id> <mod id> <result file>: Find mods' Install, without the window (for tests).
    if n >= 4 && is("--install-index-mod") {
        use nm_core::mod_index;
        let lib = scan()?;
        let result = match mod_index::load() {
            Err(x) => x,
            Ok((list, _)) => {
                let one = list.iter().find(|x| x.id == argv[2]);
                let game = lib.games.iter().find(|x| x.id == argv[1]);
                match (one, game) {
                    (None, _) => "not in the index".to_string(),
                    (Some(o), Some(g)) if mod_index::fits(o, g.gen1.as_deref()) => mod_index::install(o),
                    _ => "not for that game".to_string(),
                }
            }
        };
        let _ = std::fs::write(&argv[3], result);
        return Some(0);
    }
    if n >= 3 && is("--mod-clashes") {
        let lib = scan()?;
        let lines: Vec<String> = match lib.games.iter().find(|x| x.id == argv[1]) {
            None => vec!["no such game".into()],
            Some(g) => nm_core::mods::read_clashes(g).iter().map(|c| format!("{} ({}) clashes with {} ({}){}", c.mod_id, if c.mod_on { "on" } else { "off" }, c.other_id, if c.other_on { "on" } else { "off" }, if c.active() { format!(": {} won't load", c.mod_id) } else { String::new() })).collect(),
        };
        let _ = nm_core::config::write_all_lines(Path::new(&argv[2]), &if lines.is_empty() { vec!["no clashes".to_string()] } else { lines });
        return Some(0);
    }
    // --mod-deps <game id> <result file>: what the Needs other mods page lists for that game.
    if n >= 3 && is("--mod-deps") {
        let lib = scan()?;
        let lines: Vec<String> = match lib.games.iter().find(|x| x.id == argv[1]) {
            None => vec!["no such game".into()],
            Some(g) => nm_core::mods::open(g).iter().map(|d| format!("{}	{}	{}	for {}{}{}", d.id, if d.required { "required" } else { "optional" }, d.status, d.mod_id, if !d.range.is_empty() { format!("	{}", d.range) } else { String::new() }, if !d.github.is_empty() { format!("	github {}", d.github) } else { String::new() })).collect(),
        };
        let _ = nm_core::config::write_all_lines(Path::new(&argv[2]), &if lines.is_empty() { vec!["nothing missing".to_string()] } else { lines });
        return Some(0);
    }
    // For tests: --update <result file> checks for a release and applies it, with no window.
    if n == 2 && is("--update") {
        use nm_core::updater;
        let mut problem = updater::check();
        let tf = |b: bool| if b { "True" } else { "False" };
        let line = format!("installed {} latest {} ready {}", updater::build(), updater::latest().map_or("?".to_string(), |r| r.build.to_string()), tf(updater::ready()));
        if problem.is_none() && updater::ready() { problem = updater::apply(&mut |_, _| {}); }
        let _ = std::fs::write(&argv[1], format!("{}
result {}
now {}
", line, problem.as_deref().unwrap_or("ok"), updater::build()));
        return Some(0);
    }
    if n >= 3 && is("--sync") {
        let lib = scan()?;
        let Some(shared) = lib.games.iter().find(|x| x.id == argv[1]) else { return Some(crash("no such game")) };
        let o = nm_core::saves::sync_save(shared, argv.get(3).map(|s| s.as_str()));
        let word = if o.conflict { "conflict" } else if o.failed { "kept" } else if o.changed { "changed" } else { "level" };
        let _ = std::fs::write(&argv[2], format!("{}
{}", word, o.note.unwrap_or_default()));
        return Some(0);
    }
    if n == 4 && is("--platinum-convert") {
        let r = std::fs::read(&argv[2]).map_err(|e| e.to_string()).and_then(|b| if argv[1] == "to-port" { nm_core::platinum_save::to_port(&b) } else { nm_core::platinum_save::to_ds(&b) });
        return Some(match r.and_then(|out| std::fs::write(&argv[3], out).map_err(|e| e.to_string())) {
            Ok(()) => 0,
            Err(e) => {
                let _ = std::fs::write(format!("{}.error.txt", argv[3]), e);
                2
            }
        });
    }
    // For tests: --roms <result file> lists what the scan found for every game, and what each engine uses.
    if n == 2 && is("--roms") {
        use nm_core::library::file_name;
        let lib = scan()?;
        let tf = |b: bool| if b { "True" } else { "False" };
        let fname = |p: &Option<String>| file_name(p.as_deref().unwrap_or(""));
        let mut o: Vec<String> = Vec::new();
        for g in &lib.games {
            let chosen = g.chosen_rom().map_or("-".to_string(), |c| format!("{} {}", c.label(), file_name(&c.file)));
            let mut line = format!("{} found={} chosen={} | fulldex={} | gen1={} | nativeplat={} | refused={}", g.id, g.roms.len(), chosen, fname(&g.fulldex_rom), fname(&g.gen1_rom), fname(&g.nativeplat_rom), fname(&g.found_unsupported));
            if g.ctr.is_some() {
                let emu = nm_core::emu3ds::for_game(Some(g)).map_or("-", |e| e.id);
                line += &format!(" | 3ds={} enc={} emu={}", fname(&g.ctr_rom), tf(g.ctr_encrypted), emu);
            }
            o.push(line);
            for r in &g.roms { o.push(format!("    {} gen1={} nativeplat={} enc={} copy={} {}", r.label(), tf(r.gen1_ok), tf(r.nativeplat_ok), tf(r.encrypted), tf(r.engine_copy), r.file)); }
        }
        let _ = nm_core::config::write_all_lines(Path::new(&argv[1]), &o);
        return Some(0);
    }
    if n == 3 && is("--extract-nds") {
        return Some(match nm_core::nds_extract::run(Path::new(&argv[1]), Path::new(&argv[2])) {
            Ok(()) => 0,
            Err(e) => {
                let _ = std::fs::write(Path::new(&argv[2]).join("extract-error.txt"), e);
                2
            }
        });
    }
    // For tests: --saves <game> <result file> list | new [name] | use <n> | copy <n> | delete <n> | recover | prepare | import|import-trim <file>
    if n >= 4 && is("--saves") {
        use nm_core::{save_sets, saves};
        let mut lib = scan()?;
        let Some(at) = lib.games.iter().position(|x| x.id == argv[1]) else { return Some(crash("no such game")) };
        let verb = argv[3].as_str();
        let arg = argv.get(4).map(|s| s.as_str());
        let games = lib.games.clone();
        let saved = &mut lib.games[at];
        let mut outcome: Option<String> = None;
        match verb {
            "new" => outcome = saves::new_save(&games, saved, arg),
            "use" => outcome = saves::switch_save(&games, saved, arg.unwrap_or("")),
            "copy" => outcome = saves::duplicate_save(saved, arg),
            "delete" => outcome = saves::delete_save(saved, arg),
            "adopt" => outcome = saves::adopt_slot(&games, saved, arg, argv.get(5).map(|s| s.as_str())),
            "prepare" => {
                if saved.gen1_rom.is_none() { outcome = Some("no Gen1Recomp ROM".into()); } else if saves::prepare_now(saved, 2) { saved.imported = true; } else { outcome = Some("prepare failed".into()); }
            }
            "import" | "import-trim" => {
                let (o, footer) = saves::import_back(saved, arg, verb == "import-trim");
                outcome = if footer { Some(format!("footer: {}", o.unwrap_or_default())) } else { o };
            }
            "recover" => saves::recover_switches(&games),
            "art" => outcome = nm_ui::capture::capture(saved),
            "town" => outcome = nm_ui::capture::capture_town(saved),
            "port-in" | "port-out" => outcome = Some(format!("{} is not ported yet", verb)),
            "caught" => {
                let c = saves::caught_in_all_saves(&games, saved);
                let text = match c { None => "none".to_string(), Some(l) => l.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",") };
                let _ = std::fs::write(format!("{}.caught", argv[2]), text);
            }
            _ => {}
        }
        let saved = &lib.games[at];
        let _ = std::fs::write(&argv[2], format!("{}
active={}
all={}", outcome.as_deref().unwrap_or("ok"), save_sets::active(saved), save_sets::all(saved).join(",")));
        return Some(if outcome.is_none() { 0 } else { 2 });
    }
    // --install-mod <zip> <result> [mod id]: only that mod, when an id is given.
    if n >= 3 && is("--install-mod") {
        let result = nm_core::mods::extract_mod(&argv[1], argv.get(3).map(|s| s.as_str())).unwrap_or_else(|e| format!("Could not install it: {}", e));
        let _ = std::fs::write(&argv[2], result);
        return Some(0);
    }
    if n >= 4 && is("--export") {
        let lib = scan()?;
        let game = lib.games.iter().find(|x| x.id == argv[1]);
        let (result, done) = match game {
            Some(g) if g.playable() => nm_core::saves::export_save(g, argv[2] == "checkpoint", &argv[3], argv.get(4).map(|s| s.as_str())),
            _ => ("That game isn't available.".to_string(), false),
        };
        let _ = std::fs::create_dir_all(&argv[3]);
        let _ = std::fs::write(Path::new(&argv[3]).join("export-result.txt"), format!("{}{}", if done { "ok
" } else { "failed
" }, result));
        return Some(if done { 0 } else { 2 });
    }
    None
}

/// Library.Scan with Gen1Recomp's command line; a scan that can't read games.json ends the run as the C# would.
fn scan() -> Option<nm_core::library::Library> {
    match nm_core::library::Library::scan(&nm_core::engines::gen1_cli) {
        Ok(l) => Some(l),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// Where the C# dies on a null game (an unhandled exception): no result file, its exit code.
fn crash(why: &str) -> i32 {
    eprintln!("nativemons: {}", why);
    -532462766
}

/// `--recomp build`: the build at Idle on the Settings cores, its output kept but for the per-part lines.
fn recomp_build(g: &nm_core::game_entry::GameEntry) -> String {
    use nm_core::recomp;
    use std::io::{BufRead, Read};
    let mut psi = match recomp::build_start(g) { Ok(p) => p, Err(e) => return e.to_string() };
    psi.idle = Some(recomp::mask());
    let mut child = match nm_core::proc::command_for(&psi).spawn() { Ok(c) => c, Err(e) => return e.to_string() };
    nm_core::proc::apply_priority(&child, &psi);
    let mut se = child.stderr.take().unwrap();
    let err = std::thread::spawn(move || { let mut b = Vec::new(); let _ = se.read_to_end(&mut b); String::from_utf8_lossy(&b).into_owned() });
    let lines: Vec<String> = std::io::BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok).filter(|l| !l.contains(" Compiled ")).collect();
    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
    let ok = recomp::finish(g, code);
    format!("{}{} {}\n{}{}", if ok { "built " } else { "failed " }, code, recomp::state(g), lines.join("\n"), err.join().unwrap_or_default())
}
