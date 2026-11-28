//! Fonts (cosmic-text: FontSystem, swash glyphs) under a GDI+-style layout of ours (StringFormat): grid-fitted
//! advances, 1/6 em padding, word wrap, NoWrap, LineLimit, EllipsisCharacter and the 9 alignments.

use crate::geom::{PointF, RectangleF, SizeF};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// GDI+ (AntiAliasGridFit) measures and wraps with nominal advances times this.
pub const LAYOUT_K: f32 = 1.03;

pub struct FontFace {
    pub family: String,
    /// The face in the shared cosmic-text FontSystem (font database, fallback, swash rasterizing).
    pub id: cosmic_text::fontdb::ID,
    pub weight: cosmic_text::fontdb::Weight,
    data: Arc<Vec<u8>>,
    pub upem: f32,
    pub ascent: f32,
    pub descent: f32,
    pub spacing: f32,
    advances: Mutex<HashMap<char, (u16, u16)>>,
    /// Our code-drawn stand-in for Segoe MDL2 Assets (icons.rs): no font file, 1 em square glyphs.
    pub icons: bool,
}

/// cosmic-text's FontSystem and SwashCache, shared: our bundled faces only, never system fonts.
pub struct TextSystem {
    pub fonts: cosmic_text::FontSystem,
    pub cache: cosmic_text::SwashCache,
}

pub fn text_system() -> &'static Mutex<TextSystem> {
    static S: std::sync::OnceLock<Mutex<TextSystem>> = std::sync::OnceLock::new();
    S.get_or_init(|| Mutex::new(TextSystem { fonts: cosmic_text::FontSystem::new_with_locale_and_db("en-US".into(), cosmic_text::fontdb::Database::new()), cache: cosmic_text::SwashCache::new() }))
}

impl FontFace {
    pub fn load(path: &std::path::Path) -> Option<Arc<FontFace>> {
        let data = Arc::new(std::fs::read(path).ok()?);
        let f = swash::FontRef::from_index(&data, 0)?;
        let family = f.localized_strings().find_by_id(swash::StringId::Family, None).map(|s| s.to_string()).unwrap_or_default();
        let m = f.metrics(&[]);
        let (upem, asc, desc, gap) = (m.units_per_em as f32, m.ascent, m.descent, m.leading);
        let mut sys = text_system().lock().unwrap();
        let ids = sys.fonts.db_mut().load_font_source(cosmic_text::fontdb::Source::Binary(data.clone()));
        let id = *ids.first()?;
        let weight = sys.fonts.db().face(id).map(|i| i.weight).unwrap_or_default();
        drop(sys);
        // GDI+'s cell ascent, descent and line spacing for these fonts: hhea's.
        Some(Arc::new(FontFace { family, id, weight, data, upem, ascent: asc, descent: desc, spacing: asc + desc + gap.max(0.0), advances: Mutex::new(HashMap::new()), icons: false }))
    }
    /// The code-drawn icon set under the Segoe MDL2 Assets name and metrics (em 2048, ascent 2048, descent 0).
    pub fn icon_face() -> Arc<FontFace> {
        Arc::new(FontFace {
            family: "Segoe MDL2 Assets".into(),
            id: cosmic_text::fontdb::ID::dummy(),
            weight: cosmic_text::fontdb::Weight::NORMAL,
            data: Arc::new(Vec::new()),
            upem: 2048.0,
            ascent: 2048.0,
            descent: 0.0,
            spacing: 2048.0,
            advances: Mutex::new(HashMap::new()),
            icons: true,
        })
    }
    pub fn swash(&self) -> swash::FontRef<'_> { swash::FontRef::from_index(&self.data, 0).unwrap() }
    /// (glyph id, advance in font units); 0 when missing.
    pub fn glyph(&self, c: char) -> (u16, u16) {
        if self.icons { return if crate::icons::has(c) { (1, 2048) } else { (0, 0) }; }
        if let Some(v) = self.advances.lock().unwrap().get(&c) { return *v; }
        let f = self.swash();
        let gid = f.charmap().map(c);
        let v = if gid == 0 { (0, 0) } else { (gid, f.glyph_metrics(&[]).advance_width(gid).round() as u16) };
        self.advances.lock().unwrap().insert(c, v);
        v
    }
    pub fn has(&self, c: char) -> bool { if self.icons { crate::icons::has(c) } else { self.swash().charmap().map(c) != 0 } }
}

/// A font at a size in pixels (GraphicsUnit.Pixel).
#[derive(Clone)]
pub struct Font {
    pub face: Arc<FontFace>,
    pub size: f32,
}

impl Font {
    pub fn new(face: &Arc<FontFace>, size: f32) -> Font { Font { face: face.clone(), size } }
    /// Font.GetHeight(): the line spacing in pixels.
    pub fn get_height(&self) -> f32 { self.size * self.face.spacing / self.face.upem }
    /// Font.Height: rounded up.
    pub fn height(&self) -> i32 { self.get_height().ceil() as i32 }
    pub fn name(&self) -> &str { &self.face.family }
    /// The grid-fitted advance of one character: whole pixels at the rounded size.
    pub fn advance(&self, c: char) -> f32 {
        let f = self.face_for(c);
        let (_, a) = f.glyph(c);
        (a as f32 * self.size.round() / f.upem).round()
    }
    /// The advance GDI+ lays lines out and measures with: the nominal one, 3% wider (probed).
    pub fn advance_f(&self, c: char) -> f32 {
        let f = self.face_for(c);
        let (_, a) = f.glyph(c);
        a as f32 * self.size / f.upem * LAYOUT_K
    }
    /// The face that draws `c`: this one, else the first fallback that has it (as GDI+ font-links).
    pub fn face_for(&self, c: char) -> Arc<FontFace> {
        if self.face.icons || self.face.glyph(c).0 != 0 || c == ' ' { return self.face.clone(); }
        for f in FALLBACKS.lock().unwrap().iter() {
            if f.glyph(c).0 != 0 { return f.clone(); }
        }
        self.face.clone()
    }
}

static FALLBACKS: Mutex<Vec<Arc<FontFace>>> = Mutex::new(Vec::new());

/// Adds a fallback face (e.g. Noto Sans Symbols 2 for the arrows and stars Noto Sans lacks).
pub fn add_fallback(face: Arc<FontFace>) {
    let mut v = FALLBACKS.lock().unwrap();
    if !v.iter().any(|f| Arc::ptr_eq(f, &face)) { v.push(face); }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StringAlignment {
    Near,
    Center,
    Far,
}

#[derive(Clone, Copy, Debug)]
pub struct StringFormat {
    pub alignment: StringAlignment,
    pub line_alignment: StringAlignment,
    pub no_wrap: bool,
    pub line_limit: bool,
    pub ellipsis: bool,
}

impl Default for StringFormat {
    fn default() -> Self { StringFormat { alignment: StringAlignment::Near, line_alignment: StringAlignment::Near, no_wrap: false, line_limit: false, ellipsis: false } }
}

/// One laid-out line: the characters it holds (byte ranges of the text) and their x offsets.
#[derive(Clone, Debug)]
pub struct Line {
    pub chars: Vec<(usize, char, f32, f32)>,
    pub width: f32,
}

#[derive(Clone)]
pub struct Layout {
    pub lines: Vec<Line>,
    pub line_height: f32,
    pub pad: f32,
}

fn is_break_after(c: char) -> bool { c == ' ' || c == '-' }

type LayoutKey = (String, usize, u32, u32, bool, usize);
thread_local! {
    /// Recent layouts (pure functions of their inputs), each holding its face so the face's address stays its own.
    static LAYOUTS: std::cell::RefCell<HashMap<LayoutKey, (Arc<FontFace>, Layout)>> = std::cell::RefCell::new(HashMap::new());
}

/// Lines as GDI+ breaks them in `width` (0: unlimited), positions from the line's start.
pub fn layout(text: &str, f: &Font, width: f32, wrap: bool) -> Layout {
    let _p = crate::prof::span("text layout");
    let key = (text.to_string(), Arc::as_ptr(&f.face) as usize, f.size.to_bits(), width.to_bits(), wrap, FALLBACKS.lock().unwrap().len());
    if let Some(l) = LAYOUTS.with(|m| m.borrow().get(&key).map(|v| v.1.clone())) { return l; }
    let l = layout_new(text, f, width, wrap);
    LAYOUTS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 2048 { m.clear(); }
        m.insert(key, (f.face.clone(), l.clone()));
    });
    l
}

fn layout_new(text: &str, f: &Font, width: f32, wrap: bool) -> Layout {
    let pad = (f.size / 6.0).round();
    let avail = if width > 0.0 { width - 2.0 * f.size / 6.0 } else { f32::MAX };
    let base = text.as_ptr() as usize;
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let off = para.as_ptr() as usize - base;
        let chars: Vec<(usize, char)> = para.char_indices().map(|(i, c)| (i + off, c)).collect();
        let mut cur: Vec<(usize, char, f32, f32)> = Vec::new();
        let (mut x, mut nx) = (0.0f32, 0.0f32);
        let mut i = 0;
        while i < chars.len() {
            let mut j = i;
            while j < chars.len() && !is_break_after(chars[j].1) { j += 1; }
            if j < chars.len() { j += 1; }
            let word = &chars[i..j];
            let ww: f32 = word.iter().filter(|c| c.1 != ' ').map(|c| f.advance_f(c.1)).sum();
            if wrap && !cur.is_empty() && nx + ww > avail {
                let w = line_width(&cur);
                lines.push(Line { chars: std::mem::take(&mut cur), width: w });
                x = 0.0;
                nx = 0.0;
            }
            if wrap && cur.is_empty() && ww > avail {
                // a word longer than the line breaks by character
                for &(bi, c) in word {
                    let a = f.advance_f(c);
                    if !cur.is_empty() && nx + a > avail && c != ' ' {
                        let w = line_width(&cur);
                        lines.push(Line { chars: std::mem::take(&mut cur), width: w });
                        x = 0.0;
                        nx = 0.0;
                    }
                    cur.push((bi, c, x, f.advance(c)));
                    x += f.advance(c);
                    nx += a;
                }
            } else {
                // words start where the nominal layout puts them, glyphs inside at hinted widths
                if cur.last().map_or(false, |c| c.1 == ' ') { x = x.max((nx / LAYOUT_K).round()); }
                for &(bi, c) in word {
                    cur.push((bi, c, x, f.advance(c)));
                    x += f.advance(c);
                    nx += f.advance_f(c);
                }
            }
            i = j;
        }
        let w = line_width(&cur);
        lines.push(Line { chars: cur, width: w });
    }
    Layout { lines, line_height: line_step(f), pad }
}

/// The distance between lines: GetHeight on GDI+'s 1/16 px grid (probed).
pub fn line_step(f: &Font) -> f32 { (f.get_height() * 16.0).round() / 16.0 }

fn line_width(cs: &[(usize, char, f32, f32)]) -> f32 {
    let mut end = cs.len();
    while end > 0 && cs[end - 1].1 == ' ' { end -= 1; }
    if end == 0 { 0.0 } else { cs[end - 1].2 + cs[end - 1].3 }
}

/// Graphics.MeasureString(text, font[, width]).
pub fn measure_string(text: &str, f: &Font, width: f32) -> SizeF {
    let l = layout(text, f, width, width > 0.0);
    let w = l
        .lines
        .iter()
        .map(|ln| {
            let end = ln.chars.iter().rposition(|c| c.1 != ' ').map_or(0, |e| e + 1);
            ln.chars[..end].iter().map(|c| f.advance_f(c.1)).sum::<f32>()
        })
        .fold(0.0, f32::max);
    SizeF::new(w + 2.0 * f.size / 6.0, measured_height(f, l.lines.len()))
}

/// MeasureString's height: one line is GetHeight plus em/8; more are each the line spacing in 1/2048 em, plus em/8
/// (probed: 2-4 lines at 13.27-28.7 px within 0.0005).
pub fn measured_height(f: &Font, lines: usize) -> f32 {
    if lines <= 1 { return f.get_height() + f.size / 8.0; }
    let l = f.size * (f.face.spacing * 2048.0 / f.face.upem).round() / 2048.0;
    lines as f32 * l + f.size / 8.0
}

/// Where each line's characters land for DrawString in `r` with `sf`.
pub fn place(text: &str, f: &Font, r: RectangleF, sf: &StringFormat) -> Vec<(PointF, Line)> {
    let wrap = !sf.no_wrap;
    let mut l = layout(text, f, r.width, wrap);
    let lh = l.line_height;
    if sf.line_limit || sf.no_wrap {
        let fit = ((r.height / lh).floor() as usize).max(if sf.line_limit { 0 } else { 1 });
        if sf.no_wrap { l.lines.truncate(l.lines.len().max(1)); }
        if sf.line_limit && l.lines.len() > fit.max(1) { l.lines.truncate(fit.max(1)); }
    }
    // Centered and far blocks are placed by their measured height (MeasureString's extra 1/8 em included).
    let total = measured_height(f, l.lines.len());
    let y0 = match sf.line_alignment {
        StringAlignment::Near => r.y,
        StringAlignment::Center => r.y + (r.height - total) / 2.0,
        StringAlignment::Far => r.bottom() - total,
    };
    let inner = r.width - 2.0 * f.size / 6.0;
    l.lines
        .into_iter()
        .enumerate()
        .map(|(i, ln)| {
            let x = match sf.alignment {
                StringAlignment::Near => r.x + l.pad,
                StringAlignment::Center => r.x + f.size / 6.0 + ((inner - ln.width) / 2.0),
                StringAlignment::Far => r.x + f.size / 6.0 + (inner - ln.width),
            };
            (PointF::new(x, y0 + i as f32 * lh), ln)
        })
        .collect()
}

/// The outlines of a line of text, its baseline at `baseline` (the fallback when the transform rotates or scales).
pub fn glyph_path(f: &Font, line: &Line, x: f32, baseline: f32, path: &mut crate::path::GraphicsPath) {
    use cosmic_text::Command;
    let mut sys = text_system().lock().unwrap();
    let TextSystem { fonts, cache } = &mut *sys;
    for &(_, c, cx, _) in &line.chars {
        let face = f.face_for(c);
        let (gid, _) = face.glyph(c);
        if gid == 0 { continue; }
        let (key, _, _) = cosmic_text::CacheKey::new(face.id, gid, f.size, (0.0, 0.0), face.weight, cosmic_text::CacheKeyFlags::empty());
        let Some(cmds) = cache.get_outline_commands(fonts, key) else { continue };
        let pt = |v: swash::zeno::Vector| PointF::new(x + cx + v.x, baseline - v.y);
        let mut last = PointF::default();
        let mut open = false;
        for cmd in cmds.iter() {
            match *cmd {
                Command::MoveTo(p) => {
                    if open { path.close_figure(); }
                    path.start_figure();
                    last = pt(p);
                    open = false;
                }
                Command::LineTo(p) => {
                    let q = pt(p);
                    path.add_line(last, q);
                    last = q;
                    open = true;
                }
                Command::QuadTo(c1, p) => {
                    let (c1, q) = (pt(c1), pt(p));
                    let a = PointF::new(last.x + (c1.x - last.x) * 2.0 / 3.0, last.y + (c1.y - last.y) * 2.0 / 3.0);
                    let b = PointF::new(q.x + (c1.x - q.x) * 2.0 / 3.0, q.y + (c1.y - q.y) * 2.0 / 3.0);
                    path.add_bezier(last, a, b, q);
                    last = q;
                    open = true;
                }
                Command::CurveTo(c1, c2, p) => {
                    let q = pt(p);
                    path.add_bezier(last, pt(c1), pt(c2), q);
                    last = q;
                    open = true;
                }
                Command::Close => {
                    path.close_figure();
                    open = false;
                }
            }
        }
        if open { path.close_figure(); }
    }
}

/// Rasterized glyphs (swash, hinted, grayscale) for a line: (device x, device y, width, height, alpha) each.
pub fn glyph_images(f: &Font, line: &Line, x: f32, baseline: f32) -> Vec<(i32, i32, u32, u32, Arc<Vec<u8>>)> {
    let _p = crate::prof::span("glyph images");
    let mut out = Vec::new();
    for &(_, c, cx, _) in &line.chars {
        let face = f.face_for(c);
        let (gid, _) = face.glyph(c);
        if gid == 0 { continue; }
        let (key, ix, iy) = cosmic_text::CacheKey::new(face.id, gid, f.size, (x + cx, baseline), face.weight, cosmic_text::CacheKeyFlags::empty());
        // The mask swash gives for this key, kept and shared (the same bytes every time).
        let got = GLYPHS.with(|m| m.borrow().get(&key).cloned());
        let got = match got {
            Some(g) => g,
            None => {
                let mut sys = text_system().lock().unwrap();
                let TextSystem { fonts, cache } = &mut *sys;
                let g = cache.get_image(fonts, key).as_ref().filter(|img| img.content == cosmic_text::SwashContent::Mask).map(|img| (img.placement.left, img.placement.top, img.placement.width, img.placement.height, Arc::new(img.data.clone())));
                drop(sys);
                GLYPHS.with(|m| { let mut m = m.borrow_mut(); if m.len() >= 8192 { m.clear(); } m.insert(key, g.clone()); });
                g
            }
        };
        if let Some((left, top, w, h, data)) = got { out.push((ix + left, iy - top, w, h, data)); }
    }
    out
}

/// How many glyph masks are kept (the bench's cache line).
pub fn glyph_cache_len() -> usize { GLYPHS.with(|m| m.borrow().len()) }
type GlyphImage = Option<(i32, i32, u32, u32, Arc<Vec<u8>>)>;
thread_local! { static GLYPHS: std::cell::RefCell<HashMap<cosmic_text::CacheKey, GlyphImage>> = std::cell::RefCell::new(HashMap::new()); }

/// MeasureCharacterRanges for one range (byte offsets): one rectangle per line it is on.
pub fn char_range_rects(text: &str, f: &Font, r: RectangleF, sf: &StringFormat, start: usize, len: usize) -> Vec<RectangleF> {
    let mut out = Vec::new();
    for (o, line) in place(text, f, r, sf) {
        let mut x0 = f32::MAX;
        let mut x1 = f32::MIN;
        for &(bi, _, cx, a) in &line.chars {
            if bi >= start && bi < start + len {
                x0 = x0.min(o.x + cx);
                x1 = x1.max(o.x + cx + a);
            }
        }
        if x1 > x0 { out.push(RectangleF::new(x0, o.y, x1 - x0, f.get_height())); }
    }
    out
}
