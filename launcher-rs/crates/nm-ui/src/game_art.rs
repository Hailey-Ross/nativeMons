//! GameArt's pictures, line by line from the C#: a game's title intro (48 frames, there and back), its logo, and its
//! towns (one picked at random per start) framed between the Play card and the apps, with a blurred, darker copy
//! behind where it doesn't reach; and Wallpaper.DrawOwn, the player's own picture. The capture side isn't here.

use nm_core::config;
use nm_core::game_entry::GameEntry;
use nm_draw::{Bitmap, Color, Graphics, RectangleF};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const COUNT: usize = 48;

#[derive(Default)]
struct Cache {
    loaded_id: Option<String>,
    loaded: Vec<Arc<Bitmap>>,
    logo: Option<Arc<Bitmap>>,
    town_chosen: HashMap<String, PathBuf>,
    dice: Option<nm_draw::net_random::NetRandom>,
    town_file: Option<PathBuf>,
    town: Option<Arc<Bitmap>>,
    framed: Option<(PathBuf, Arc<Bitmap>, Arc<Bitmap>)>,
    own: Option<(PathBuf, std::time::SystemTime, Option<Arc<Bitmap>>)>,
    /// Hack id -> base id (GameArt.BaseOf), and games.json townView by id.
    base_of: HashMap<String, String>,
    views: HashMap<String, Vec<f32>>,
}
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
fn with<R>(f: impl FnOnce(&mut Cache) -> R) -> R { let mut c = CACHE.lock().unwrap(); f(c.get_or_insert_with(Cache::default)) }

/// The library's hacks and town views (Library.Scan sets them in the C#).
pub fn set_library(games: &[GameEntry]) {
    with(|c| {
        c.base_of = games.iter().filter_map(|g| g.base_id.clone().map(|b| (g.id.clone(), b))).collect();
        c.views = games.iter().filter_map(|g| g.town_view.clone().map(|v| (g.id.clone(), v))).collect();
    });
}
/// The random pick's seed (TestHooks.NewRandom): fixed in tests, else the clock.
pub fn set_seed(seed: Option<i32>) {
    let s = seed.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i32).unwrap_or(1));
    with(|c| c.dice = Some(nm_draw::net_random::NetRandom::new(s)));
}

fn folder(id: &str) -> PathBuf { config::state().join("game-art").join(id) }
fn frame_file(id: &str, i: usize) -> PathBuf { folder(id).join(format!("frame-{:02}.jpg", i)) }
fn logo_file(id: &str) -> PathBuf { folder(id).join("logo.png") }
pub fn has(id: &str) -> bool { frame_file(id, COUNT - 1).is_file() }
pub fn town_file(id: &str) -> PathBuf { folder(id).join("town.jpg") }
pub fn has_town(id: &str) -> bool { town_file(id).is_file() }
pub fn towns_dir(id: &str) -> PathBuf { folder(id).join("towns") }
/// Shown: a hack without its own pictures shows its base game's.
fn shown(c: &Cache, id: &str, has: fn(&str) -> bool) -> String {
    if has(id) { return id.to_string(); }
    match c.base_of.get(id) { Some(b) if has(b) => b.clone(), _ => id.to_string() }
}
fn chosen_town(c: &mut Cache, id: &str) -> PathBuf {
    if let Some(f) = c.town_chosen.get(id) { if f.is_file() { return f.clone(); } }
    let mut all = vec![town_file(id)];
    if let Ok(rd) = std::fs::read_dir(towns_dir(id)) {
        let mut more: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("jpg"))).collect();
        more.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        all.extend(more);
    }
    all.retain(|x| x.is_file());
    if all.is_empty() { all.push(town_file(id)); }
    if c.dice.is_none() { c.dice = Some(nm_draw::net_random::NetRandom::new(1)); }
    let f = all[c.dice.as_mut().unwrap().next(all.len() as i32) as usize].clone();
    c.town_chosen.insert(id.to_string(), f.clone());
    f
}
fn town_view(c: &Cache, id: &str) -> Vec<f32> {
    if let Some(v) = c.views.get(id) { return v.clone(); }
    if let Some(v) = c.base_of.get(id).and_then(|b| c.views.get(b)) { return v.clone(); }
    vec![0.5, 0.5, 1.0]
}
fn load(path: &std::path::Path) -> Option<Bitmap> {
    let png = path.extension().map_or(false, |x| x.eq_ignore_ascii_case("png"));
    if png { Bitmap::load_png(path).ok() } else { Bitmap::load_jpeg(path).ok() }
}

/// Town: the chosen town picture, loaded once.
pub fn town(id: &str) -> Option<Arc<Bitmap>> {
    with(|c| {
        let id = shown(c, id, has_town);
        if !has_town(&id) { return None; }
        let file = chosen_town(c, &id);
        if c.town_file.as_ref() != Some(&file) {
            let next = Arc::new(load(&file)?);
            c.town = Some(next);
            c.town_file = Some(file);
        }
        c.town.clone()
    })
}
fn load_frames(c: &mut Cache, id: &str) -> Option<()> {
    if c.loaded_id.as_deref() == Some(id) { return Some(()); }
    let mut next = Vec::with_capacity(COUNT);
    for i in 0..COUNT { next.push(Arc::new(load(&frame_file(id, i))?)); }
    c.logo = if logo_file(id).is_file() { load(&logo_file(id)).map(Arc::new) } else { None };
    c.loaded = next;
    c.loaded_id = Some(id.to_string());
    Some(())
}
/// Frame: there and back, so the end meets the beginning without a jump (ten pictures a second).
pub fn frame(id: &str, t: f32) -> Option<Arc<Bitmap>> {
    with(|c| {
        let id = shown(c, id, has);
        if !has(&id) { return None; }
        load_frames(c, &id)?;
        let n = ((t * 10.0) as i32).rem_euclid((COUNT * 2 - 2) as i32) as usize;
        Some(c.loaded[if n < COUNT { n } else { COUNT * 2 - 2 - n }].clone())
    })
}
pub fn logo(id: &str) -> Option<Arc<Bitmap>> {
    with(|c| { let id = shown(c, id, has); if !has(&id) { return None; } load_frames(c, &id)?; c.logo.clone() })
}

/// Draw: the picture covering the rect, centered (NearestNeighbor, PixelOffsetMode.Half).
pub fn draw(g: &mut Graphics, pic: &Arc<Bitmap>, r: RectangleF) {
    let scale = (r.width / pic.width as f32).max(r.height / pic.height as f32);
    let (w, h) = (pic.width as f32 * scale, pic.height as f32 * scale);
    g.draw_image_nearest_shared(pic, RectangleF::new(r.x + (r.width - w) / 2.0, r.y + (r.height - h) / 2.0, w, h));
}
/// DrawFitted: a tile much wider than the picture shows all of it at its height, over a dimmed copy filling the tile.
pub fn draw_fitted(g: &mut Graphics, pic: &Arc<Bitmap>, r: RectangleF, always: bool) {
    // The phone's tile (about 2.5:1) keeps its close crop.
    if !always && r.width < r.height * 3.5 { draw(g, pic, r); return; }
    let fit = r.height / pic.height as f32;
    draw(g, pic, r);
    g.fill_rectangle(&nm_draw::Brush::Solid(Color::from_argb(150, 0, 0, 0)), r);
    let w = pic.width as f32 * fit;
    draw(g, pic, RectangleF::new(r.x + (r.width - w) / 2.0, r.y, w, r.height));
}

/// Feathered: its top and bottom fade out, so the sharp picture meets the blurred one softly.
fn feathered(pic: &Bitmap) -> Bitmap {
    let mut b = pic.clone();
    let edge = (b.height / 10).max(1);
    for row in 0..b.height {
        let d = row.min(b.height - 1 - row);
        if d >= edge { continue; }
        let k = d as f32 / edge as f32;
        let a = (255.0 * k * k * (3.0 - 2.0 * k)) as u8 as u32;
        for col in 0..b.width { let i = (row * b.width + col) as usize; b.data[i] = (b.data[i] & 0x00FF_FFFF) | a << 24; }
    }
    b
}
fn blurred(pic: &Bitmap) -> Bitmap {
    let (w, h) = ((pic.width / 24).max(4), (pic.height / 24).max(4));
    let mut g = Graphics::new(Bitmap::new(w, h));
    g.draw_image_hq_bilinear(pic, RectangleF::new(0.0, 0.0, w as f32, h as f32));
    g.bmp
}

/// DrawTown: the town framed in band; where it doesn't reach the screen's edges, a blurred, darker copy fills in.
pub fn draw_town(g: &mut Graphics, game_id: &str, screen: RectangleF, band: RectangleF) -> bool {
    let Some(pic) = town(game_id) else { return false };
    // A wide window has no open band between them: the town just fills it.
    if screen.width > screen.height || band.height < screen.height * 0.08 { draw(g, &pic, screen); return true; }
    let (view, soft, blur) = with(|c| {
        let id = shown(c, game_id, has_town);
        let file = chosen_town(c, &id);
        // games.json's framing is for the home town; the others are centered, a little closer than their whole width.
        let view = if file == town_file(&id) { town_view(c, &id) } else { vec![0.5, 0.5, 1.4] };
        if c.framed.as_ref().map_or(true, |f| f.0 != file) { c.framed = Some((file, Arc::new(feathered(&pic)), Arc::new(blurred(&pic)))); }
        let f = c.framed.as_ref().unwrap();
        (view, f.1.clone(), f.2.clone())
    });
    let scale = screen.width / pic.width as f32 * view[2].max(1.0);
    let (dw, dh) = (pic.width as f32 * scale, pic.height as f32 * scale);
    let x = (screen.right() - dw).max(screen.x.min(band.x + band.width / 2.0 - view[0] * dw));
    let mut y = band.y + band.height / 2.0 - view[1] * dh;
    if dh >= screen.height { y = (screen.bottom() - dh).max(screen.y.min(y)); }
    if y > screen.y || y + dh < screen.bottom() {
        let cover = (screen.width / blur.width as f32).max(screen.height / blur.height as f32) * 1.1;
        g.draw_image_hq_bilinear_shared(&blur, RectangleF::new(screen.x + (screen.width - blur.width as f32 * cover) / 2.0, screen.y + (screen.height - blur.height as f32 * cover) / 2.0, blur.width as f32 * cover, blur.height as f32 * cover));
        g.fill_rectangle(&nm_draw::Brush::Solid(Color::from_argb(90, 0, 0, 0)), screen);
    }
    g.draw_image_nearest_shared(if dh >= screen.height { &pic } else { &soft }, RectangleF::new(x, y, dw, dh));
    true
}

// ---------- Wallpaper: the player's own picture for a game ----------
const KINDS: [&str; 5] = [".png", ".jpg", ".jpeg", ".bmp", ".gif"];
fn wall_dir() -> PathBuf { config::state().join("wallpapers") }
pub fn own(id: &str) -> Option<PathBuf> { KINDS.iter().map(|k| wall_dir().join(format!("{}{}", id, k))).find(|f| f.is_file()) }
pub fn wallpaper_drawn() -> bool { config::read("wallpaper.txt", "") == "drawn" }
/// Wallpaper.Remove's cache reset: the next DrawOwn reads the file again.
pub fn forget_own() { with(|c| c.own = None); }
/// DrawOwn: the game's own picture, cropped to fit (kept at most 1600 px). False when it has none or can't be read.
pub fn draw_own(g: &mut Graphics, id: &str, screen: RectangleF) -> bool {
    let Some(f) = own(id) else { return false };
    let t = std::fs::metadata(&f).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
    let pic = with(|c| {
        if c.own.as_ref().map_or(true, |o| o.0 != f || o.1 != t) {
            let img = load(&f).map(|img| {
                let k = (1600.0 / img.width.max(img.height) as f32).min(1.0);
                if k >= 1.0 { return img; }
                let (w, h) = (((img.width as f32 * k) as i32).max(1), ((img.height as f32 * k) as i32).max(1));
                let mut gg = Graphics::new(Bitmap::new(w, h));
                gg.draw_image_hq_bilinear(&img, RectangleF::new(0.0, 0.0, w as f32, h as f32));
                gg.bmp
            });
            c.own = Some((f.clone(), t, img.map(Arc::new)));
        }
        c.own.as_ref().and_then(|o| o.2.clone())
    });
    let Some(pic) = pic else { return false };
    let s = (screen.width / pic.width as f32).max(screen.height / pic.height as f32);
    let st = g.save();
    g.set_clip_rect(screen);
    g.draw_image_hq_bilinear_shared(&pic, RectangleF::new(screen.x + (screen.width - pic.width as f32 * s) / 2.0, screen.y + (screen.height - pic.height as f32 * s) / 2.0, pic.width as f32 * s, pic.height as f32 * s));
    g.restore(st);
    true
}
