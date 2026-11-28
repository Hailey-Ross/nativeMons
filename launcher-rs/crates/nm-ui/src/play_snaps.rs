//! PlaySnaps (Judge, Look) and GameArt.TrimDark, line by line from the C#: which part of a 3DS emulator's window is
//! worth keeping as a wallpaper. The capture itself (CopyFromScreen while the game is in front) is the platform's.

use nm_draw::Bitmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

/// TrimDark: the largest rectangle of the picture with no black in it, on a grid of 8-pixel cells.
pub fn trim_dark(pic: &Bitmap, a: Rect) -> Rect {
    const CELL: i32 = 8;
    let (cols, rows) = (a.w / CELL, a.h / CELL);
    let mut heights = vec![0i32; cols.max(0) as usize];
    let (mut best_area, mut best) = (0, a);
    for r in 0..rows {
        for c in 0..cols {
            let mut d = 0;
            for dy in (1..CELL).step_by(3) {
                for dx in (1..CELL).step_by(3) {
                    let px = pic.get(a.x + c * CELL + dx, a.y + r * CELL + dy);
                    if px.r as i32 + px.g as i32 + px.b as i32 > 35 { continue; }
                    d += 1;
                }
            }
            heights[c as usize] = if d >= 5 { 0 } else { heights[c as usize] + 1 };
        }
        // Largest rectangle under this row's histogram.
        let mut stack: Vec<i32> = Vec::new();
        for c in 0..=cols {
            let h = if c == cols { 0 } else { heights[c as usize] };
            while let Some(&top) = stack.last() {
                if heights[top as usize] < h { break; }
                stack.pop();
                let height = heights[top as usize];
                let left = stack.last().map_or(0, |&p| p + 1);
                let width = c - left;
                if height * width > best_area { best_area = height * width; best = Rect { x: a.x + left * CELL, y: a.y + (r - height + 1) * CELL, w: width * CELL, h: height * CELL }; }
            }
            stack.push(c);
        }
    }
    best
}

/// Look: brightness, its spread, and a 64-bit fingerprint (8x8 cells brighter than the mean).
pub fn look(pic: &Bitmap) -> (u64, f64, f64) {
    let mut cells = [0f64; 64];
    let (mut sum, mut sq, mut n) = (0f64, 0f64, 0i32);
    for cy in 0..8 {
        for cx in 0..8 {
            let (mut cell, mut m) = (0f64, 0);
            let mut y = cy * pic.height / 8;
            while y < (cy + 1) * pic.height / 8 {
                let mut x = cx * pic.width / 8;
                while x < (cx + 1) * pic.width / 8 {
                    let c = pic.get(x, y);
                    let l = 0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64;
                    cell += l; m += 1; sum += l; sq += l * l; n += 1;
                    x += 6;
                }
                y += 6;
            }
            cells[(cy * 8 + cx) as usize] = if m > 0 { cell / m as f64 } else { 0.0 };
        }
    }
    let mean = if n > 0 { sum / n as f64 } else { 0.0 };
    let spread = if n > 0 { (sq / n as f64 - mean * mean).max(0.0).sqrt() } else { 0.0 };
    let mut print = 0u64;
    for (i, c) in cells.iter().enumerate() { if *c > mean { print |= 1 << i; } }
    (print, mean, spread)
}

fn crop(b: &Bitmap, r: Rect) -> Bitmap {
    let mut o = Bitmap::new(r.w, r.h);
    for y in 0..r.h { for x in 0..r.w { o.data[(y * r.w + x) as usize] = b.data[((r.y + y) * b.width + r.x + x) as usize]; } }
    o
}

/// Judge: the emulator window's picture, its top screen if worth keeping (with its fingerprint), and why or why not.
pub fn judge(shot: &Bitmap, last: u64) -> (Option<(Bitmap, u64)>, String) {
    let (w, ht) = (shot.width, shot.height);
    // The top screen: the largest lit rectangle in the upper part, 5:3 like the 3DS's.
    let top = trim_dark(shot, Rect { x: 0, y: 0, w: w - w % 8, h: (ht * 6 / 10) - (ht * 6 / 10) % 8 });
    let ratio = top.w as f32 / top.h.max(1) as f32;
    if top.w < 300 || top.h < 180 || !(1.45..=1.9).contains(&ratio) { return (None, format!("no top screen ({}x{})", top.w, top.h)); }
    let pic = crop(shot, top);
    let (print, mean, spread) = look(&pic);
    if mean < 35.0 || mean > 225.0 || spread < 28.0 { return (None, format!("too dark, bright or flat (mean {}, spread {})", mean as i32, spread as i32)); }
    if last != 0 && (print ^ last).count_ones() < 10 { return (None, "same as the last one".into()); }
    let why = format!("kept {}x{} at {},{}", top.w, top.h, top.x, top.y);
    (Some((pic, print)), why)
}

const KEEP: usize = 24;
/// Snap's second half: the window's picture judged, and kept when it's worth it; its fingerprint, or 0.
pub fn snap(shot: &Bitmap, game_id: &str, last: u64) -> u64 {
    let (pic, _) = judge(shot, last);
    match pic { Some((pic, print)) => { save(game_id, &pic); print } None => 0 }
}
/// Save: towns\play-<time>.jpg (the newest 24 kept); the first becomes the game's own wallpaper.
pub fn save(game_id: &str, pic: &Bitmap) {
    let dir = crate::game_art::towns_dir(game_id);
    let _ = std::fs::create_dir_all(&dir);
    let Ok(jpeg) = pic.encode_jpeg(90) else { return };
    let file = dir.join(format!("play-{}.jpg", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let _ = std::fs::write(&file, &jpeg);
    if !crate::game_art::has_town(game_id) { let _ = std::fs::write(crate::game_art::town_file(game_id), &jpeg); }
    let mut old: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy().to_lowercase(); n.starts_with("play-") && n.ends_with(".jpg") })).collect();
    old.sort_by(|a, b| b.to_string_lossy().as_bytes().cmp(a.to_string_lossy().as_bytes()));
    for f in old.into_iter().skip(KEEP) { let _ = std::fs::remove_file(f); }
}

/// --play-snap-test <result> <window.png>...: what the 3DS wallpaper pictures would keep from each window picture.
pub fn play_snap_test(result: &std::path::Path, files: &[String]) -> std::io::Result<()> {
    let mut lines = Vec::new();
    let mut last = 0u64;
    for f in files {
        let b = Bitmap::load_png(std::path::Path::new(f))?;
        let (pic, why) = judge(&b, last);
        if let Some((_, p)) = pic { last = p; }
        lines.push(format!("{}: {}", std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), why));
    }
    nm_core::config::write_all_lines(result, &lines)
}

/// The platform's half (nm-platform::snaps), set by the binary at start: the window grab, waiting on the end, and a
/// lower priority for the watcher thread.
pub struct Hooks {
    pub grab: fn(u32) -> Option<(i32, i32, Vec<u32>)>,
    pub waiter: fn(u32) -> Option<Box<dyn FnMut(u64) -> bool + Send>>,
    pub lower: fn(),
}
static HOOKS: std::sync::OnceLock<Hooks> = std::sync::OnceLock::new();
pub fn set_hooks(h: Hooks) { let _ = HOOKS.set(h); }

/// Watch: a minute into the session, then every 75 s while the emulator runs, its window's picture (when in front).
pub fn watch(pid: u32, game_id: String) {
    let Some(h) = HOOKS.get() else { return };
    let Some(mut wait) = (h.waiter)(pid) else { return };
    let _ = std::thread::Builder::new().name("play-snaps".into()).spawn(move || {
        (h.lower)();
        let grab = || (h.grab)(pid).map(|(w, ht, data)| { let mut b = Bitmap::new(w, ht); b.data = data; b });
        watch_loop(&mut *wait, &grab, &mut |shot, last| snap(shot, &game_id, last));
    });
}
/// Watch's loop: nothing in the first minute; then a snap, every 75 s until the end. `keep` returns the kept print or 0.
pub fn watch_loop(wait: &mut dyn FnMut(u64) -> bool, grab: &dyn Fn() -> Option<Bitmap>, keep: &mut dyn FnMut(&Bitmap, u64) -> u64) {
    if wait(60_000) { return; }
    let mut last = 0u64;
    loop {
        if let Some(shot) = grab() { let kept = keep(&shot, last); if kept != 0 { last = kept; } }
        if wait(75_000) { break; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn watch_loop_waits_a_minute_then_snaps_until_the_end() {
        // Ends before the first minute: nothing grabbed.
        let mut waits = Vec::new();
        let grabs = std::cell::Cell::new(0);
        watch_loop(&mut |ms| { waits.push(ms); true }, &|| { grabs.set(grabs.get() + 1); None }, &mut |_, _| 0);
        assert_eq!((waits, grabs.get()), (vec![60_000], 0));
        // Three snaps 75 s apart; the second is kept, so the third is judged against it.
        let mut n = 0;
        let mut seen = Vec::new();
        watch_loop(&mut |_| { n += 1; n == 4 }, &|| Some(Bitmap::new(4, 4)), &mut |_, last| { seen.push(last); if seen.len() == 2 { 77 } else { 0 } });
        assert_eq!(seen, vec![0, 0, 77]);
    }
    #[test]
    fn judge_keeps_a_lit_top_screen() {
        // A 1000x900 window: a 500x300 noisy picture on black in the upper part.
        let mut b = Bitmap::new(1000, 900);
        for v in b.data.iter_mut() { *v = 0xFF00_0000; }
        let mut r = nm_draw::NetRandom::new(5);
        for y in 40..340 { for x in 200..700 { let c = r.next(200) as u32 + 40; b.data[(y * 1000 + x) as usize] = 0xFF00_0000 | c << 16 | c << 8 | c; } }
        let (pic, why) = judge(&b, 0);
        assert!(why.starts_with("kept "), "{}", why);
        let (_, print) = pic.expect("kept");
        assert_eq!(judge(&b, print).1, "same as the last one");
    }
}
