//! System.Drawing.Graphics over a Bitmap, drawing pixels the way GDI+ does.

use crate::bitmap::Bitmap;
use crate::brush::Brush;
use crate::color::Color;
use crate::flatten::{cfg, flatten, FlattenCfg};
use crate::geom::{PointF, RectangleF};
use crate::matrix::Matrix;
use crate::path::GraphicsPath;
use crate::raster::{rasterize_aa, rasterize_aliased, Coverage, FillMode, Polys};
use crate::stroke::{nominal, widen_outline, LineCap, LineJoin, StrokeStyle};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SmoothingMode {
    None,
    AntiAlias,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelOffsetMode {
    None,
    Half,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InterpolationMode {
    Bilinear,
    NearestNeighbor,
    HighQualityBilinear,
    HighQualityBicubic,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DashStyle {
    Solid,
    Dash,
    Dot,
}

#[derive(Clone, Debug)]
pub struct Pen {
    pub color: Color,
    pub width: f32,
    pub start_cap: LineCap,
    pub end_cap: LineCap,
    pub line_join: LineJoin,
    pub dash_style: DashStyle,
    pub miter_limit: f32,
    /// Widen at the true width even under 1.5 px (glyph-like strokes), instead of GDI+'s nominal pen.
    pub true_width: bool,
}

impl Pen {
    pub fn new(color: Color, width: f32) -> Pen {
        Pen { color, width, start_cap: LineCap::Flat, end_cap: LineCap::Flat, line_join: LineJoin::Miter, dash_style: DashStyle::Solid, miter_limit: 10.0, true_width: false }
    }
    pub fn round(mut self) -> Pen {
        self.start_cap = LineCap::Round;
        self.end_cap = LineCap::Round;
        self
    }
    pub fn round_join(mut self) -> Pen {
        self.line_join = LineJoin::Round;
        self
    }
}

/// A clip in device pixels: 1 where drawing may land, kept over its tight box (all out beyond it); shared on save.
#[derive(Clone)]
pub struct Clip {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    mask: std::sync::Arc<Vec<u8>>,
    /// The clip's float bounds in device space, as GDI+ regions report them.
    pub bounds: RectangleF,
}

impl Clip {
    #[inline]
    pub fn at(&self, x: i32, y: i32) -> bool {
        let (dx, dy) = (x - self.x0, y - self.y0);
        dx >= 0 && dy >= 0 && dx < self.w && dy < self.h && self.mask[(dy * self.w + dx) as usize] != 0
    }
    /// The columns of row `y` inside the clip's box, clamped to `lo..hi`, and that row's mask from `lo`.
    #[inline]
    fn row(&self, y: i32, lo: i32, hi: i32) -> Option<(i32, i32, &[u8])> {
        let dy = y - self.y0;
        if dy < 0 || dy >= self.h { return None; }
        let (a, b) = (lo.max(self.x0), hi.min(self.x0 + self.w));
        if a >= b { return None; }
        let base = (dy * self.w) as usize;
        Some((a, b, &self.mask[base + (a - self.x0) as usize..base + (b - self.x0) as usize]))
    }
}

/// One drawing call in record mode: what it needs to draw again, the state it was made under, and the device box
/// it can change. A frame is its ops in order; flush redraws only where they differ from the last frame's.
#[derive(Clone)]
pub struct Op {
    kind: OpKind,
    transform: Matrix,
    clip: Option<Clip>,
    smoothing: SmoothingMode,
    pixel_offset: PixelOffsetMode,
    bbox: (i32, i32, i32, i32),
}

/// An op's picture, shared: the same Arc is the same picture without comparing its bytes.
#[derive(Clone)]
struct Img(std::sync::Arc<Bitmap>);
impl std::ops::Deref for Img { type Target = Bitmap; fn deref(&self) -> &Bitmap { &self.0 } }
impl PartialEq for Img {
    fn eq(&self, o: &Img) -> bool { std::sync::Arc::ptr_eq(&self.0, &o.0) || *self.0 == *o.0 }
}

#[derive(Clone, PartialEq)]
enum OpKind {
    Clear(u32),
    Fill { polys: Polys, mode: FillMode, brush: Brush },
    Glyphs { glyphs: Vec<(i32, i32, u32, u32, std::sync::Arc<Vec<u8>>)>, color: Color },
    Nearest { img: Img, r: RectangleF },
    Bilinear { img: Img, r: RectangleF },
    HqBilinear { img: Img, r: RectangleF },
    HqBicubic { img: Img, r: RectangleF },
    Unscaled { img: Img, x: i32, y: i32 },
}

impl Op {
    /// Whether the op draws the same pixels as `o`, given the same pixels under it.
    fn same(&self, o: &Op, clips: &mut std::collections::HashMap<(usize, usize), bool>) -> bool {
        if self.bbox != o.bbox || self.kind != o.kind { return false; }
        let clip_same = match (&self.clip, &o.clip) {
            (None, None) => true,
            (Some(a), Some(b)) if std::sync::Arc::ptr_eq(&a.mask, &b.mask) && (a.x0, a.y0, a.w, a.h, a.bounds) == (b.x0, b.y0, b.w, b.h, b.bounds) => true,
            (Some(a), Some(b)) => *clips.entry((std::sync::Arc::as_ptr(&a.mask) as usize, std::sync::Arc::as_ptr(&b.mask) as usize))
                .or_insert_with(|| (a.x0, a.y0, a.w, a.h, a.bounds) == (b.x0, b.y0, b.w, b.h, b.bounds) && a.mask == b.mask),
            _ => false,
        };
        if !clip_same { return false; }
        match &self.kind {
            // A solid fill's pixels follow from its device polygons; the rest also read the transform.
            OpKind::Fill { brush: Brush::Solid(_), .. } => self.smoothing == o.smoothing,
            OpKind::Fill { .. } => self.smoothing == o.smoothing && self.transform == o.transform && self.pixel_offset == o.pixel_offset,
            OpKind::Clear(_) | OpKind::Glyphs { .. } => true,
            _ => self.transform == o.transform && self.pixel_offset == o.pixel_offset,
        }
    }
}

fn debug_damage() -> bool { static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new(); *D.get_or_init(|| std::env::var("NM_DAMAGE_DEBUG").is_ok()) }
impl OpKind {
    fn name(&self) -> String {
        match self {
            OpKind::Clear(_) => "clear".into(),
            OpKind::Fill { brush, polys, .. } => format!("fill {} {} pts", match brush { Brush::Solid(c) => format!("solid {:?}", c), Brush::Linear(_) => "linear".into(), Brush::Ellipse(_) => "ellipse".into() }, polys.iter().map(|p| p.len()).sum::<usize>()),
            OpKind::Glyphs { glyphs, .. } => format!("glyphs {}", glyphs.len()),
            OpKind::Nearest { .. } => "image nearest".into(),
            OpKind::Bilinear { .. } => "image bilinear".into(),
            OpKind::HqBilinear { .. } => "image hq bilinear".into(),
            OpKind::HqBicubic { .. } => "image hq bicubic".into(),
            OpKind::Unscaled { .. } => "image unscaled".into(),
        }
    }
}
fn meet(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) { (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3)) }
fn join(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) { (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)) }
fn empty(a: (i32, i32, i32, i32)) -> bool { a.2 <= a.0 || a.3 <= a.1 }
fn area(a: (i32, i32, i32, i32)) -> i64 { if empty(a) { 0 } else { (a.2 - a.0) as i64 * (a.3 - a.1) as i64 } }

#[derive(Clone)]
pub struct GraphicsState {
    transform: Matrix,
    clip: Option<Clip>,
    smoothing: SmoothingMode,
    pixel_offset: PixelOffsetMode,
    interpolation: InterpolationMode,
}

/// Who turns paths into coverage: our GDI+ clone (default), or tiny-skia (a fallback where parity doesn't matter).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rasterizer {
    GdiPlus,
    TinySkia,
}

pub struct Graphics {
    pub bmp: Bitmap,
    pub rasterizer: Rasterizer,
    pub transform: Matrix,
    pub clip: Option<Clip>,
    pub smoothing_mode: SmoothingMode,
    pub pixel_offset_mode: PixelOffsetMode,
    pub interpolation_mode: InterpolationMode,
    flat: FlattenCfg,
    /// Known GDI+ noise the parity check accepts by name: (name, x, y, w, h) in device pixels.
    pub accepted: Vec<(&'static str, i32, i32, i32, i32)>,
    /// Pixels a gradient already painted this frame (for the gradient-stack residual).
    grad_mask: Vec<bool>,
    /// Note the accepted residuals (parity runs); the live launcher turns it off. Pixels are the same either way.
    pub track: bool,
    /// Record mode (begin_record): calls are kept as ops and drawn by flush, only where they changed.
    rec: Option<Vec<Op>>,
    /// The device box drawing may change (all of it, except while flush redraws a damaged part).
    limit: (i32, i32, i32, i32),
}

#[derive(PartialEq, Eq, Hash)]
struct ClipKey { bits: Vec<u32>, before: usize, before_box: (i32, i32, i32, i32) }
type ClipCache = (std::collections::HashMap<ClipKey, (Option<Clip>, Clip)>, usize);
thread_local! {
    /// SetClip results by their exact inputs (path, transform, offset, flattening, canvas, the clip before), so a clip
    /// set again in a later frame (a new Graphics each frame) is the same Arc, nothing rasterized. Entries keep the
    /// clip before alive, so its address can't be reused while its key exists.
    static CLIP_CACHE: std::cell::RefCell<ClipCache> = std::cell::RefCell::new((std::collections::HashMap::new(), 0));
    /// Fill coverage by its exact inputs (device polygons, fill mode, smoothing, rasterizer, the box drawing may change):
    /// a shape drawn again in a later frame isn't scan converted again.
    static COVERAGE_CACHE: std::cell::RefCell<(std::collections::HashMap<Vec<u64>, std::sync::Arc<Coverage>>, usize)> = std::cell::RefCell::new((std::collections::HashMap::new(), 0));
}

/// The shape and clip caches on this thread: (shapes, their bytes, clips, their bytes).
pub fn cache_stats() -> (usize, usize, usize, usize) {
    let (a, b) = COVERAGE_CACHE.with(|c| { let c = c.borrow(); (c.0.len(), c.1) });
    let (x, y) = CLIP_CACHE.with(|c| { let c = c.borrow(); (c.0.len(), c.1) });
    (a, b, x, y)
}

fn div255(x: u32) -> u32 { (x + 128 + ((x + 128) >> 8)) >> 8 }

/// The color at `n` of `full` samples: premultiplied, each channel scaled by n/full and rounded (probed).
pub fn blend_n(dst: u32, c: Color, n: u32, full: u32) -> u32 {
    if n >= full { return blend(dst, c, 255); }
    let sc = |v: u32| (v * n + full / 2) / full;
    let (a, r, g, b) = (c.a as u32, div255(c.r as u32 * c.a as u32), div255(c.g as u32 * c.a as u32), div255(c.b as u32 * c.a as u32));
    over(dst, sc(a), sc(r), sc(g), sc(b))
}

/// A premultiplied brush color ([a, r, g, b]) at `n` of `full` samples, over the pixel.
pub fn blend_pm_n(dst: u32, c: [u32; 4], n: u32, full: u32) -> u32 {
    if n >= full { return over(dst, c[0], c[1], c[2], c[3]); }
    let sc = |v: u32| (v * n + full / 2) / full;
    over(dst, sc(c[0]), sc(c[1]), sc(c[2]), sc(c[3]))
}

/// SourceOver as GDI+ does it: the color premultiplied, scaled by coverage, then over the pixel.
pub fn blend(dst: u32, c: Color, cov: u32) -> u32 {
    let (mut a, mut r, mut g, mut b) = (c.a as u32, div255(c.r as u32 * c.a as u32), div255(c.g as u32 * c.a as u32), div255(c.b as u32 * c.a as u32));
    if cov < 255 {
        a = div255(a * cov);
        r = div255(r * cov);
        g = div255(g * cov);
        b = div255(b * cov);
    }
    over(dst, a, r, g, b)
}

/// A premultiplied color over a straight-alpha pixel.
fn over(dst: u32, a: u32, r: u32, g: u32, b: u32) -> u32 {
    if a == 0 { return dst; }
    if a == 255 { return 0xff00_0000 | r << 16 | g << 8 | b; }
    let da = dst >> 24;
    let (dr, dg, db) = (dst >> 16 & 255, dst >> 8 & 255, dst & 255);
    if da == 255 {
        let f = |s: u32, d: u32| (s + div255(d * (255 - a))).min(255);
        return 0xff00_0000 | f(r, dr) << 16 | f(g, dg) << 8 | f(b, db);
    }
    // A translucent pixel: premultiply it, add, and take the alpha back out.
    let (pr, pg, pb) = (div255(dr * da), div255(dg * da), div255(db * da));
    let oa = a + div255(da * (255 - a));
    if oa == 0 { return 0; }
    let f = |s: u32, d: u32| (((s + div255(d * (255 - a))) * 255 + oa / 2) / oa).min(255);
    oa << 24 | f(r, pr) << 16 | f(g, pg) << 8 | f(b, pb)
}

impl Graphics {
    pub fn new(bmp: Bitmap) -> Graphics {
        let limit = (0, 0, bmp.width, bmp.height);
        Graphics {
            bmp,
            rasterizer: { static R: std::sync::OnceLock<Rasterizer> = std::sync::OnceLock::new(); *R.get_or_init(|| if std::env::var("NM_RASTERIZER").as_deref() == Ok("tiny-skia") { Rasterizer::TinySkia } else { Rasterizer::GdiPlus }) },
            transform: Matrix::IDENTITY,
            clip: None,
            smoothing_mode: SmoothingMode::None,
            pixel_offset_mode: PixelOffsetMode::None,
            interpolation_mode: InterpolationMode::Bilinear,
            flat: cfg(),
            accepted: Vec::new(),
            grad_mask: Vec::new(),
            track: true,
            rec: None,
            limit,
        }
    }
    /// Record mode: drawing calls are kept (as ops) until flush. Clips are still worked out at once.
    pub fn begin_record(&mut self) { self.rec = Some(Vec::new()); self.track = false; }

    fn record(&mut self, kind: OpKind) {
        let canvas = (0, 0, self.bmp.width, self.bmp.height);
        let mut b = match &kind {
            OpKind::Clear(_) => canvas,
            OpKind::Fill { polys, .. } => {
                let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                let mut odd = false;
                for q in polys.iter().flatten() { odd |= !q.0.is_finite() || !q.1.is_finite(); x0 = x0.min(q.0); y0 = y0.min(q.1); x1 = x1.max(q.0); y1 = y1.max(q.1); }
                // Coverage lands in [floor(min), floor(max) + 1) of these (already shifted) coordinates; a pixel spare.
                if odd { canvas } else if x0 > x1 { (0, 0, 0, 0) } else { (x0.floor().max(-1e6) as i32 - 1, y0.floor().max(-1e6) as i32 - 1, x1.floor().min(1e6) as i32 + 2, y1.floor().min(1e6) as i32 + 2) }
            }
            OpKind::Glyphs { glyphs, .. } => glyphs.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |a, g| join(a, (g.0, g.1, g.0 + g.2 as i32, g.1 + g.3 as i32))),
            OpKind::Nearest { r, .. } | OpKind::Bilinear { r, .. } | OpKind::HqBilinear { r, .. } | OpKind::HqBicubic { r, .. } => self.device_box(*r),
            OpKind::Unscaled { img, x, y } => {
                let o = self.transform.apply(PointF::new(*x as f32, *y as f32));
                let (ox, oy) = (o.x.round() as i32, o.y.round() as i32);
                (ox, oy, ox + img.width, oy + img.height)
            }
        };
        b = meet(b, canvas);
        if let Some(c) = &self.clip { b = meet(b, (c.x0, c.y0, c.x0 + c.w, c.y0 + c.h)); }
        if empty(b) { b = (0, 0, 0, 0); }
        let op = Op { kind, transform: self.transform, clip: self.clip.clone(), smoothing: self.smoothing_mode, pixel_offset: self.pixel_offset_mode, bbox: b };
        self.rec.as_mut().unwrap().push(op);
    }
    /// A world rectangle's device box, a pixel spare each side.
    fn device_box(&self, r: RectangleF) -> (i32, i32, i32, i32) {
        let c = [PointF::new(r.x, r.y), PointF::new(r.right(), r.y), PointF::new(r.right(), r.bottom()), PointF::new(r.x, r.bottom())].map(|q| self.transform.apply(q));
        let f = |v: f32| v.clamp(-1e6, 1e6);
        (f(c.iter().map(|q| q.x).fold(f32::MAX, f32::min)).floor() as i32 - 1, f(c.iter().map(|q| q.y).fold(f32::MAX, f32::min)).floor() as i32 - 1,
         f(c.iter().map(|q| q.x).fold(f32::MIN, f32::max)).ceil() as i32 + 1, f(c.iter().map(|q| q.y).fold(f32::MIN, f32::max)).ceil() as i32 + 1)
    }
    fn run_op(&mut self, op: &Op) {
        let kept = (self.transform, self.clip.take(), self.smoothing_mode, self.pixel_offset_mode);
        self.transform = op.transform;
        self.clip = op.clip.clone();
        self.smoothing_mode = op.smoothing;
        self.pixel_offset_mode = op.pixel_offset;
        match &op.kind {
            OpKind::Clear(v) => self.exec_clear(*v),
            OpKind::Fill { polys, mode, brush } => self.exec_fill(polys, *mode, brush),
            OpKind::Glyphs { glyphs, color } => self.exec_glyphs(glyphs, *color),
            OpKind::Nearest { img, r } => self.exec_nearest(&img.0, *r),
            OpKind::Bilinear { img, r } => self.exec_bilinear(&img.0, *r),
            OpKind::HqBilinear { img, r } => self.exec_hq_bilinear(&img.0, *r),
            OpKind::HqBicubic { img, r } => self.exec_hq_bicubic(&img.0, *r),
            OpKind::Unscaled { img, x, y } => self.exec_unscaled(&img.0, *x, *y),
        }
        (self.transform, self.clip, self.smoothing_mode, self.pixel_offset_mode) = kept;
    }
    /// Ends record mode: draws this frame's ops over the bitmap, which holds the frame `last` made, wherever they
    /// differ from `last` (all of it when `last` is empty). Pixels elsewhere are what these ops would draw there, so the
    /// bitmap ends up as a full redraw would leave it. Returns the ops (the next frame's `last`) and the parts redrawn.
    pub fn flush(&mut self, last: &[Op]) -> (Vec<Op>, Vec<(i32, i32, i32, i32)>) {
        let ops = self.rec.take().unwrap_or_default();
        self.apply(ops, last)
    }
    /// flush for ops recorded elsewhere (the damage check draws one frame's ops both ways).
    pub fn apply(&mut self, ops: Vec<Op>, last: &[Op]) -> (Vec<Op>, Vec<(i32, i32, i32, i32)>) {
        let _p = crate::prof::span("flush");
        let canvas = (0, 0, self.bmp.width, self.bmp.height);
        let mut dirty: Vec<(i32, i32, i32, i32)> = Vec::new();
        if last.is_empty() || self.rasterizer == Rasterizer::TinySkia {
            dirty.push(canvas);
        } else {
            let mut clips = std::collections::HashMap::new();
            for i in 0..ops.len().max(last.len()) {
                match (ops.get(i), last.get(i)) {
                    (Some(a), Some(b)) if a.same(b, &mut clips) => {}
                    (a, b) => {
                        if debug_damage() { eprintln!("damage: op {} {:?} / {:?}", i, a.map(|o| (o.kind.name(), o.bbox)), b.map(|o| (o.kind.name(), o.bbox))); }
                        for o in [a, b].into_iter().flatten() { if !empty(o.bbox) { dirty.push(o.bbox); } }
                    }
                }
            }
            // Overlapping or near parts become one; many, or most of the window, become the whole of it.
            let near = |a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)| a.0 <= b.2 + 8 && b.0 <= a.2 + 8 && a.1 <= b.3 + 8 && b.1 <= a.3 + 8;
            let mut merged = true;
            while merged {
                merged = false;
                'outer: for i in 0..dirty.len() {
                    for j in i + 1..dirty.len() {
                        if near(dirty[i], dirty[j]) { dirty[i] = join(dirty[i], dirty[j]); dirty.swap_remove(j); merged = true; break 'outer; }
                    }
                }
            }
            if dirty.len() > 64 || dirty.iter().map(|d| area(*d)).sum::<i64>() * 10 > area(canvas) * 7 { dirty = vec![canvas]; }
        }
        for d in &dirty {
            self.limit = *d;
            for op in &ops { if !empty(meet(op.bbox, *d)) { self.run_op(op); } }
        }
        self.limit = canvas;
        (ops, dirty)
    }

    pub fn width(&self) -> i32 { self.bmp.width }
    pub fn height(&self) -> i32 { self.bmp.height }

    pub fn save(&self) -> GraphicsState {
        let _p = crate::prof::span("save");
        GraphicsState { transform: self.transform, clip: self.clip.clone(), smoothing: self.smoothing_mode, pixel_offset: self.pixel_offset_mode, interpolation: self.interpolation_mode }
    }
    pub fn restore(&mut self, s: GraphicsState) {
        self.transform = s.transform;
        self.clip = s.clip;
        self.smoothing_mode = s.smoothing;
        self.pixel_offset_mode = s.pixel_offset;
        self.interpolation_mode = s.interpolation;
    }
    pub fn translate_transform(&mut self, x: f32, y: f32) { self.transform.translate(x, y); }
    pub fn rotate_transform(&mut self, deg: f32) { self.transform.rotate(deg); }
    pub fn scale_transform(&mut self, x: f32, y: f32) { self.transform.scale(x, y); }

    pub fn clear(&mut self, c: Color) {
        let _p = crate::prof::span("clear");
        let v = (c.a as u32) << 24 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
        if self.rec.is_some() { self.record(OpKind::Clear(v)); return; }
        self.exec_clear(v);
    }
    fn exec_clear(&mut self, v: u32) {
        let (x0, y0, x1, y1) = self.limit;
        if (x0, y0, x1, y1) == (0, 0, self.bmp.width, self.bmp.height) { for p in self.bmp.data.iter_mut() { *p = v; } return; }
        for y in y0..y1 { for p in &mut self.bmp.data[(y * self.bmp.width + x0) as usize..(y * self.bmp.width + x1) as usize] { *p = v; } }
    }

    fn offset(&self) -> f32 { if self.pixel_offset_mode == PixelOffsetMode::Half { -0.5 } else { 0.0 } }
    /// Where geometry lands for the rasterizer: the pixel offset, plus half a pixel when antialiased.
    fn raster_shift(&self) -> f32 { self.offset() + if self.smoothing_mode == SmoothingMode::AntiAlias { 0.5 } else { 0.0 } }
    fn bound(&self) -> (i32, i32, i32, i32) { (0, 0, self.bmp.width, self.bmp.height) }

    fn device_polys(&self, path: &GraphicsPath) -> Vec<(Vec<(f64, f64)>, bool)> {
        let _p = crate::prof::span("flatten");
        flatten(path, &self.transform, self.raster_shift(), &self.flat)
    }

    fn coverage(&self, polys: &Polys, mode: FillMode) -> Coverage {
        let _p = crate::prof::span("raster");
        if self.rasterizer == Rasterizer::TinySkia { return crate::raster::rasterize_tiny_skia(polys, mode, self.smoothing_mode == SmoothingMode::AntiAlias, self.limit); }
        if self.smoothing_mode == SmoothingMode::AntiAlias { rasterize_aa(polys, mode, self.limit) } else { rasterize_aliased(polys, mode, self.limit) }
    }

    /// The accepted residuals start over (a new frame).
    pub fn clear_accepted(&mut self) { self.accepted.clear(); self.grad_mask.clear(); }

    fn paint(&mut self, cov: &Coverage, brush: &Brush) {
        let _p = crate::prof::span(match brush { Brush::Solid(_) => "paint solid", Brush::Linear(_) => "paint linear", Brush::Ellipse(_) => "paint ellipse" });
        if self.track {
            self.note_wrap_rows(cov, brush);
            self.note_gradient_stack(cov, brush);
        }
        if cov.w <= 0 || cov.h <= 0 { return; }
        let full = cov.full as u32;
        let bw = self.bmp.width;
        let clip = self.clip.clone();
        // Each row's columns that may change: the coverage box, inside the clip's box.
        let span = |y: i32| -> Option<(i32, i32, Option<&[u8]>)> {
            match &clip {
                None => Some((cov.x0, cov.x0 + cov.w, None)),
                Some(c) => c.row(y, cov.x0, cov.x0 + cov.w).map(|(a, b, m)| (a, b, Some(m))),
            }
        };
        match brush {
            Brush::Solid(c) => {
                // blend_n's premultiplied, coverage-scaled color for every sample count, once.
                let (a, r, g, b) = (c.a as u32, div255(c.r as u32 * c.a as u32), div255(c.g as u32 * c.a as u32), div255(c.b as u32 * c.a as u32));
                let mut table = [[0u32; 4]; 256];
                for (n, e) in table.iter_mut().enumerate().take(full as usize + 1) {
                    let n = n as u32;
                    let sc = |v: u32| (v * n + full / 2) / full;
                    *e = if n >= full { [a, r, g, b] } else { [sc(a), sc(r), sc(g), sc(b)] };
                }
                // An opaque color fully covering a pixel is the pixel (over's a == 255); otherwise the same coverage
                // over the same pixel gives the same result, so the last one is reused.
                let opaque = if a == 255 { Some(0xff00_0000 | r << 16 | g << 8 | b) } else { None };
                for y in cov.y0..cov.y0 + cov.h {
                    let Some((x0, x1, m)) = span(y) else { continue };
                    let crow = &cov.data[((y - cov.y0) * cov.w + (x0 - cov.x0)) as usize..((y - cov.y0) * cov.w + (x1 - cov.x0)) as usize];
                    let drow = &mut self.bmp.data[(y * bw + x0) as usize..(y * bw + x1) as usize];
                    put_run(drow, crow, m, full, opaque, |d, n| { let e = &table[n as usize]; over(d, e[0], e[1], e[2], e[3]) });
                }
            }
            _ => {
                let inv = self.transform.invert();
                let off = self.offset();
                let prep = Prepared::new(brush);
                // A linear gradient (no Blend) under a transform that keeps rows level: along a row its color is
                // monotone per channel within one tile, so equal colors at two pixels hold for all between. Runs of one
                // color are found by galloping and halving, then blended; the colors are the per-pixel ones.
                let level = inv.as_ref().map_or(true, |mm| mm.0.ky == 0.0);
                if let (true, Prepared::Linear(lg, k, Some(e))) = (level, &prep) {
                    if lg.blend.is_empty() {
                        let (lg, k, e) = (*lg, *k, *e);
                        let clip_box = clip.as_ref().map(|c| (c.x0, c.y0, c.w, c.h));
                        let key = run_key(lg, k, e, inv.as_ref(), off, (cov.x0, cov.y0, cov.w, cov.h), clip_box);
                        let runs = match RUN_CACHE.with(|rc| rc.borrow().0.get(&key).cloned()) {
                            Some(r) => r,
                            None => {
                                let r = std::rc::Rc::new(linear_runs(lg, k, e, inv.as_ref(), off, cov, &span));
                                RUN_CACHE.with(|rc| {
                                    let mut rc = rc.borrow_mut();
                                    // At most 16 MB of runs (kept small for low-end PCs); then it starts again.
                                    if rc.1 + r.len() * 16 > 16 << 20 || rc.0.len() >= 4096 { rc.0.clear(); rc.1 = 0; }
                                    rc.1 += r.len() * 16;
                                    rc.0.insert(key, r.clone());
                                });
                                r
                            }
                        };
                        let mut row: Option<(i32, i32, i32, Option<&[u8]>)> = None;
                        for &(y, x, lo, packed) in runs.iter() {
                            if row.map_or(true, |r| r.0 != y) { row = span(y).map(|(a, b, m)| (y, a, b, m)); }
                            let Some((_, x0, x1, m)) = row else { continue };
                            let crow = &cov.data[((y - cov.y0) * cov.w + (x0 - cov.x0)) as usize..((y - cov.y0) * cov.w + (x1 - cov.x0)) as usize];
                            let pm = [packed >> 24, (packed >> 16) & 255, (packed >> 8) & 255, packed & 255];
                            // As for solid fills: an opaque color at full coverage is the pixel, and the same
                            // coverage over the same pixel gives the last result again.
                            let opaque = if pm[0] == 255 { Some(0xff00_0000 | pm[1] << 16 | pm[2] << 8 | pm[3]) } else { None };
                            let (k0, k1) = ((x - x0) as usize, (lo - x0) as usize + 1);
                            let drow = &mut self.bmp.data[(y * bw + x) as usize..(y * bw + lo) as usize + 1];
                            put_run(drow, &crow[k0..k1], m.map(|m| &m[k0..k1]), full, opaque, |d, n| blend_pm_n(d, pm, n as u32, full));
                        }
                        return;
                    }
                }
                let _slow = crate::prof::span(match prep { Prepared::Linear(_, _, None) => "paint linear colors", Prepared::Linear(..) => "paint linear per pixel", _ => "paint ellipse per pixel" });
                for y in cov.y0..cov.y0 + cov.h {
                    let Some((x0, x1, m)) = span(y) else { continue };
                    let crow = &cov.data[((y - cov.y0) * cov.w + (x0 - cov.x0)) as usize..((y - cov.y0) * cov.w + (x1 - cov.x0)) as usize];
                    for k in 0..crow.len() {
                        let n = crow[k];
                        if n == 0 || m.map_or(false, |m| m[k] == 0) { continue; }
                        let x = x0 + k as i32;
                        let dev = PointF::new(x as f32 - off, y as f32 - off);
                        let p = inv.as_ref().map_or(dev, |mm| mm.apply(dev));
                        let pm = prep.color(p);
                        let i = (y * bw + x) as usize;
                        self.bmp.data[i] = blend_pm_n(self.bmp.data[i], pm, n as u32, full);
                    }
                }
            }
        }
    }

    /// Where a translucent gradient lands on another gradient: GDI+ rounds the stack its own way, up to 3 (user, 2026-11-25:
    /// accepted as part of the gradient rounding, named gradient-stack; golden-diff allows 3 there and no more).
    fn note_gradient_stack(&mut self, cov: &Coverage, brush: &Brush) {
        let _p = crate::prof::span("grad stack");
        // A translucent solid fill over a gradient is marked too, but marks nothing itself (user, 2026-11-25).
        let (translucent, marks) = match brush {
            Brush::Solid(c) if c.a == 255 => { self.unmark_full(cov); return; }
            Brush::Solid(c) => (c.a < 255, false),
            Brush::Linear(lg) => (lg.c1.a < 255 || lg.c2.a < 255 || lg.colors.iter().any(|c| c.0.a < 255), true),
            Brush::Ellipse(eg) => (eg.center.a < 255 || eg.surround.a < 255, true),
        };
        if !marks && (!translucent || self.grad_mask.is_empty()) { return; }
        let n = (self.bmp.width * self.bmp.height) as usize;
        if self.grad_mask.len() != n { self.grad_mask = vec![false; n]; }
        for y in cov.y0..cov.y0 + cov.h {
            let mut run: Option<i32> = None;
            for x in cov.x0..=cov.x0 + cov.w {
                let i = (y * self.bmp.width + x.min(self.bmp.width - 1)) as usize;
                let inside = x < cov.x0 + cov.w && cov.get(x, y) > 0 && self.clip.as_ref().map_or(true, |c| c.at(x, y));
                let stacked = inside && translucent && self.grad_mask[i];
                if stacked && run.is_none() { run = Some(x); }
                if !stacked { if let Some(s) = run.take() { self.accepted.push(("gradient-stack", s, y, x - s, 1)); } }
                if inside && marks { self.grad_mask[i] = true; }
            }
        }
    }

    /// An opaque fill's fully covered pixels no longer show a gradient: a later translucent layer over them is one
    /// rounded layer, not a stack (the narrower gradient-stack, user 2026-11-25).
    fn unmark_full(&mut self, cov: &Coverage) {
        if self.grad_mask.is_empty() { return; }
        let full = cov.full;
        for y in cov.y0..cov.y0 + cov.h {
            for x in cov.x0..cov.x0 + cov.w {
                if cov.get(x, y) as i32 == full as i32 && self.clip.as_ref().map_or(true, |c| c.at(x, y)) { self.grad_mask[(y * self.bmp.width + x) as usize] = false; }
            }
        }
    }
    /// An opaque image pixel drawn over a marked one: the same.
    fn unmark_px(&mut self, i: usize) { if self.track && !self.grad_mask.is_empty() { self.grad_mask[i] = false; } }

    /// A vertical Tile gradient's exact wrap rows: where GDI+ wraps or not by its own float noise (accepted residual).
    fn note_wrap_rows(&mut self, cov: &Coverage, brush: &Brush) {
        let Brush::Linear(lg) = brush else { return };
        if lg.wrap != crate::brush::WrapMode::Tile || (lg.angle.rem_euclid(180.0) - 90.0).abs() > 1e-3 || cov.w <= 0 { return; }
        let t = self.transform.0;
        if t.kx != 0.0 || t.ky != 0.0 { return; }
        let top = lg.rect.y * t.sy + t.ty + self.offset();
        let h = lg.rect.height * t.sy;
        if h.abs() < 1.0 { return; }
        let (lo, hi) = (cov.y0 as f32, (cov.y0 + cov.h - 1) as f32);
        let k0 = ((lo - top) / h).floor() as i32;
        let k1 = ((hi - top) / h).ceil() as i32;
        for k in k0..=k1 {
            let row = (top + k as f32 * h).round();
            if row >= lo && row <= hi { self.accepted.push(("gradient-wrap-row", cov.x0, row as i32, cov.w, 1)); }
        }
    }

    pub fn fill_polys(&mut self, polys: &Polys, mode: FillMode, brush: &Brush) {
        if self.rec.is_some() { self.record(OpKind::Fill { polys: polys.clone(), mode, brush: brush.clone() }); return; }
        self.exec_fill(polys, mode, brush);
    }
    fn fill_owned(&mut self, polys: Polys, mode: FillMode, brush: &Brush) {
        if self.rec.is_some() { self.record(OpKind::Fill { polys, mode, brush: brush.clone() }); return; }
        self.exec_fill(&polys, mode, brush);
    }
    fn exec_fill(&mut self, polys: &Polys, mode: FillMode, brush: &Brush) {
        let cov = self.coverage_cached(polys, mode);
        self.paint(&cov, brush);
    }
    fn coverage_cached(&self, polys: &Polys, mode: FillMode) -> std::sync::Arc<Coverage> {
        let n: usize = polys.iter().map(|p| p.len() * 2 + 1).sum();
        let mut key = Vec::with_capacity(n + 6);
        let (x0, y0, x1, y1) = self.limit;
        key.extend([mode as u64, self.smoothing_mode as u64 | (self.rasterizer as u64) << 8 | (crate::flatten::ALIASED.with(|a| a.get()) as u64) << 16,
            x0 as u32 as u64 | (y0 as u32 as u64) << 32, x1 as u32 as u64 | (y1 as u32 as u64) << 32]);
        for p in polys { key.push(p.len() as u64); for q in p { key.push(q.0.to_bits()); key.push(q.1.to_bits()); } }
        if let Some(c) = COVERAGE_CACHE.with(|cc| cc.borrow().0.get(&key).cloned()) { return c; }
        let c = std::sync::Arc::new(self.coverage(polys, mode));
        COVERAGE_CACHE.with(|cc| {
            let mut cc = cc.borrow_mut();
            // At most 4,096 shapes or 32 MB (low-end PCs); then it starts again.
            if cc.0.len() >= 4096 || cc.1 > 32 << 20 { cc.0.clear(); cc.1 = 0; }
            cc.1 += c.data.len() + key.len() * 8;
            cc.0.insert(key, c.clone());
        });
        c
    }

    pub fn fill_path(&mut self, brush: &Brush, path: &GraphicsPath) {
        let polys: Polys = self.device_polys(path).into_iter().map(|p| p.0).collect();
        self.fill_owned(polys, path.fill_mode, brush);
    }
    pub fn fill_rectangle(&mut self, brush: &Brush, r: RectangleF) {
        let mut p = GraphicsPath::new();
        p.add_rectangle(r);
        self.fill_path(brush, &p);
    }
    pub fn fill_ellipse(&mut self, brush: &Brush, r: RectangleF) {
        let mut p = GraphicsPath::new();
        p.add_ellipse(r);
        self.fill_path(brush, &p);
    }
    pub fn fill_polygon(&mut self, brush: &Brush, pts: &[PointF]) {
        let mut p = GraphicsPath::new();
        p.add_polygon(pts);
        self.fill_path(brush, &p);
    }
    pub fn fill_pie(&mut self, brush: &Brush, x: f32, y: f32, w: f32, h: f32, start: f32, sweep: f32) {
        let mut p = GraphicsPath::new();
        p.add_pie(RectangleF::new(x, y, w, h), start, sweep);
        self.fill_path(brush, &p);
    }

    pub fn draw_path(&mut self, pen: &Pen, path: &GraphicsPath) {
        let _p = crate::prof::span("draw_path");
        let figs = crate::flatten::flatten_ex(path, &self.transform, self.raster_shift(), &self.flat, true);
        let m = &self.transform;
        let scale = ((m.m11() as f64 * m.m22() as f64 - m.m12() as f64 * m.m21() as f64).abs()).sqrt();
        // A stretched world judges the pen by its longest axis (probed, t30: 2 px under (1, 0.31) is widened).
        let (a, b, c, d) = (m.m11() as f64, m.m12() as f64, m.m21() as f64, m.m22() as f64);
        let (q, r2) = ((a * a + b * b + c * c + d * d) / 2.0, ((a * a + b * b - c * c - d * d).powi(2) / 4.0 + (a * c + b * d).powi(2)).sqrt());
        let (longest, shortest) = ((q + r2).sqrt(), (q - r2).max(0.0).sqrt());
        // A scale within 0.000595 of 1 judges the pen as unscaled (probed, t37-t41).
        let judged = if (longest - 1.0).abs() < 0.0005955 { 1.0 } else { longest };
        let nominal_pen = pen.width as f64 * judged <= 1.5 && !pen.true_width && pen.dash_style == DashStyle::Solid;
        let st = StrokeStyle {
            width: pen_width(pen.width as f64 * scale, self.smoothing_mode),
            start_cap: pen.start_cap,
            end_cap: pen.end_cap,
            join: pen.line_join,
            miter_limit: pen.miter_limit as f64,
        };
        let mut polys = Vec::new();
        // A stretched or sheared world: the pen widens in world units and its outline is transformed (as GDI+ does).
        let (c1, c2) = ((m.m11() as f64).hypot(m.m12() as f64), (m.m21() as f64).hypot(m.m22() as f64));
        let skew = (m.m11() as f64 * m.m21() as f64 + m.m12() as f64 * m.m22() as f64).abs();
        let aniso = (c1 - c2).abs() > 1e-4 * c1.max(c2) || skew > 1e-4 * c1 * c2;
        let inv = if aniso { m.invert() } else { None };
        let shift = self.raster_shift() as f64;
        let _w = crate::prof::span("widen");
        for (line, closed) in &figs {
            for piece in dashes(line, *closed, pen, st.width) {
                // Dashed pens always widen at their true width (probed, t27).
                if nominal_pen {
                    nominal(&piece.0, piece.1, &mut polys);
                } else if inv.is_some() && pen.width as f64 * shortest < 1.0 {
                    // Squeezed under a pixel on its short axis: a round one-pixel pen in device space (probed, t30/t31).
                    widen_outline(&piece.0, piece.1, &StrokeStyle { width: 1.0, ..st }, &self.flat, &mut polys);
                } else if let Some(inv) = &inv {
                    let world: Vec<(f64, f64)> = piece.0.iter().map(|q| { let w = inv.apply(PointF::new((q.0 - shift) as f32, (q.1 - shift) as f32)); (w.x as f64, w.y as f64) }).collect();
                    let wst = StrokeStyle { width: pen.width as f64, ..st };
                    let mut wp = Vec::new();
                    widen_outline(&world, piece.1, &wst, &self.flat, &mut wp);
                    for poly in wp { polys.push(poly.iter().map(|q| { let d = m.apply(PointF::new(q.0 as f32, q.1 as f32)); (d.x as f64 + shift, d.y as f64 + shift) }).collect()); }
                } else {
                    widen_outline(&piece.0, piece.1, &st, &self.flat, &mut polys);
                }
            }
        }
        drop(_w);
        let b = Brush::Solid(pen.color);
        self.fill_owned(polys, FillMode::Winding, &b);
    }
    pub fn draw_line(&mut self, pen: &Pen, a: PointF, b: PointF) {
        let mut p = GraphicsPath::new();
        p.add_line(a, b);
        self.draw_path(pen, &p);
    }
    pub fn draw_line_f(&mut self, pen: &Pen, x1: f32, y1: f32, x2: f32, y2: f32) { self.draw_line(pen, PointF::new(x1, y1), PointF::new(x2, y2)); }
    pub fn draw_lines(&mut self, pen: &Pen, pts: &[PointF]) {
        let mut p = GraphicsPath::new();
        p.add_lines(pts);
        self.draw_path(pen, &p);
    }
    pub fn draw_bezier(&mut self, pen: &Pen, a: PointF, b: PointF, c: PointF, d: PointF) {
        let mut path = GraphicsPath::new();
        path.add_bezier(a, b, c, d);
        self.draw_path(pen, &path);
    }
    /// DrawCurve(pen, points): a cardinal spline, tension 0.5; control points at tension/3 (probed, t21).
    pub fn draw_curve(&mut self, pen: &Pen, pts: &[PointF]) {
        let n = pts.len();
        if n < 2 { return; }
        let t = 0.5f32 / 3.0;
        let p = PointF::new;
        let mut path = GraphicsPath::new();
        let mut c1 = p(pts[0].x + t * (pts[1].x - pts[0].x), pts[0].y + t * (pts[1].y - pts[0].y));
        for i in 0..n - 1 {
            let c2 = if i + 2 < n {
                p(pts[i + 1].x - t * (pts[i + 2].x - pts[i].x), pts[i + 1].y - t * (pts[i + 2].y - pts[i].y))
            } else {
                p(pts[n - 1].x + t * (pts[n - 2].x - pts[n - 1].x), pts[n - 1].y + t * (pts[n - 2].y - pts[n - 1].y))
            };
            path.add_bezier(pts[i], c1, c2, pts[i + 1]);
            if i + 2 < n { c1 = p(pts[i + 1].x + t * (pts[i + 2].x - pts[i].x), pts[i + 1].y + t * (pts[i + 2].y - pts[i].y)); }
        }
        self.draw_path(pen, &path);
    }
    pub fn draw_polygon(&mut self, pen: &Pen, pts: &[PointF]) {
        let mut p = GraphicsPath::new();
        p.add_polygon(pts);
        self.draw_path(pen, &p);
    }
    pub fn draw_ellipse(&mut self, pen: &Pen, r: RectangleF) {
        let mut p = GraphicsPath::new();
        p.add_ellipse(r);
        self.draw_path(pen, &p);
    }
    pub fn draw_arc(&mut self, pen: &Pen, r: RectangleF, start: f32, sweep: f32) {
        let mut p = GraphicsPath::new();
        p.add_arc(r, start, sweep);
        self.draw_path(pen, &p);
    }
    pub fn draw_rectangle(&mut self, pen: &Pen, r: RectangleF) {
        let mut p = GraphicsPath::new();
        p.add_rectangle(r);
        self.draw_path(pen, &p);
    }

    fn clip_mask(&self, path: &GraphicsPath) -> Coverage {
        let _p = crate::prof::span("clip_mask");
        // Regions of straight lines round to the nearest 1/16 px; regions with curves snap like fills (probed).
        let all_ceil = path.pts.iter().any(|p| p.t == crate::path::PtType::Bezier);
        crate::flatten::ALIASED.with(|a| a.set(!all_ceil));
        // A curved region under a scale within 0.000595 of 1 is only offset (probed, t39).
        let m = self.transform;
        let near = all_ceil && (m.m11() - 1.0).abs() < 0.0005955 && (m.m22() - 1.0).abs() < 0.0005955 && m.m12() == 0.0 && m.m21() == 0.0;
        let tf = if near { Matrix(tiny_skia::Transform::from_translate(m.0.tx, m.0.ty)) } else { m };
        let polys: Polys = flatten(path, &tf, self.offset(), &self.flat).into_iter().map(|p| p.0).collect();
        crate::flatten::ALIASED.with(|a| a.set(false));
        let cov = if all_ceil { rasterize_aliased(&polys, path.fill_mode, self.bound()) } else { crate::raster::rasterize_region(&polys, path.fill_mode, self.bound()) };
        cov
    }
    fn intersect_clip(&mut self, cov: Coverage) {
        let _p = crate::prof::span("intersect_clip");
        // The new region over the box both share (outside either box nothing is in).
        let (mut bx0, mut by0, mut bx1, mut by1) = (cov.x0, cov.y0, cov.x0 + cov.w, cov.y0 + cov.h);
        if let Some(c) = &self.clip { bx0 = bx0.max(c.x0); by0 = by0.max(c.y0); bx1 = bx1.min(c.x0 + c.w); by1 = by1.min(c.y0 + c.h); }
        // VisibleClipBounds is the clip's pixel extent (probed with text masks): the tight box of what is left.
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
        let mut tmp = Vec::new();
        if bx1 > bx0 && by1 > by0 {
            tmp = vec![0u8; ((bx1 - bx0) * (by1 - by0)) as usize];
            let bw = (bx1 - bx0) as usize;
            for y in by0..by1 {
                let c0 = ((y - cov.y0) * cov.w + (bx0 - cov.x0)) as usize;
                let crow = &cov.data[c0..c0 + bw];
                let trow = &mut tmp[(y - by0) as usize * bw..(y - by0 + 1) as usize * bw];
                match self.clip.as_ref().and_then(|c| c.row(y, bx0, bx1)) {
                    Some((_, _, m)) => for k in 0..bw { trow[k] = (crow[k] > 0 && m[k] != 0) as u8; },
                    None if self.clip.is_some() => continue,
                    None => for k in 0..bw { trow[k] = (crow[k] > 0) as u8; },
                }
                if let Some(a) = trow.iter().position(|v| *v != 0) {
                    let b = trow.iter().rposition(|v| *v != 0).unwrap() as i32;
                    x0 = x0.min(bx0 + a as i32); x1 = x1.max(bx0 + b); y0 = y0.min(y); y1 = y1.max(y);
                }
            }
        }
        if x1 < 0 {
            self.clip = Some(Clip { x0: 0, y0: 0, w: 0, h: 0, mask: std::sync::Arc::new(Vec::new()), bounds: RectangleF::EMPTY });
            return;
        }
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut mask = Vec::with_capacity((w * h) as usize);
        for y in y0..=y1 {
            let b = ((y - by0) * (bx1 - bx0) + (x0 - bx0)) as usize;
            mask.extend_from_slice(&tmp[b..b + w as usize]);
        }
        self.clip = Some(Clip { x0, y0, w, h, mask: std::sync::Arc::new(mask), bounds: RectangleF::new(x0 as f32, y0 as f32, w as f32, h as f32) });
    }
    /// SetClip(path, CombineMode.Intersect).
    pub fn set_clip_path(&mut self, path: &GraphicsPath) {
        let key = self.clip_key(path);
        if let Some(c) = CLIP_CACHE.with(|cc| cc.borrow().0.get(&key).map(|e| e.1.clone())) { self.clip = Some(c); return; }
        let before = self.clip.clone();
        let m = self.clip_mask(path);
        self.intersect_clip(m);
        if let Some(c) = self.clip.clone() {
            CLIP_CACHE.with(|cc| {
                let mut cc = cc.borrow_mut();
                // At most 2,048 clips or 16 MB of masks; then it starts again.
                if cc.0.len() >= 2048 || cc.1 > 16 << 20 { cc.0.clear(); cc.1 = 0; }
                cc.1 += c.mask.len() + key.bits.len() * 4;
                cc.0.insert(key, (before, c));
            });
        }
    }
    /// Everything clip_mask and intersect_clip read, bit for bit.
    fn clip_key(&self, path: &GraphicsPath) -> ClipKey {
        let t = self.transform.0;
        let mut bits = Vec::with_capacity(path.pts.len() * 3 + 16);
        bits.extend([t.sx, t.kx, t.ky, t.sy, t.tx, t.ty, self.offset()].map(f32::to_bits));
        bits.extend([self.flat.unit.to_bits() as u32, (self.flat.unit.to_bits() >> 32) as u32, self.flat.tol as u32, (self.flat.tol >> 32) as u32, self.flat.adaptive as u32]);
        let (bx, by, bw, bh) = self.bound();
        bits.extend([bx as u32, by as u32, bw as u32, bh as u32, path.fill_mode as u32]);
        for p in &path.pts { bits.extend([p.p.x.to_bits(), p.p.y.to_bits(), p.t as u32 | (p.close as u32) << 8]); }
        match &self.clip {
            Some(c) => ClipKey { bits, before: std::sync::Arc::as_ptr(&c.mask) as usize, before_box: (c.x0, c.y0, c.w, c.h) },
            None => ClipKey { bits, before: 0, before_box: (0, 0, 0, 0) },
        }
    }
    /// SetClip(rect, CombineMode.Intersect).
    pub fn set_clip_rect(&mut self, r: RectangleF) {
        let mut p = GraphicsPath::new();
        p.add_rectangle(r);
        self.set_clip_path(&p);
    }
    pub fn reset_clip(&mut self) { self.clip = None; }
    /// Whether device pixel (x, y) is inside the clip (true with none).
    pub fn clip_at(&self, x: i32, y: i32) -> bool { self.clip.as_ref().map_or(true, |c| c.at(x, y)) }

    /// VisibleClipBounds, in world coordinates.
    pub fn visible_clip_bounds(&self) -> RectangleF {
        let canvas = RectangleF::new(0.0, 0.0, self.bmp.width as f32, self.bmp.height as f32);
        let r = match &self.clip { Some(c) => RectangleF::intersect(c.bounds, canvas), None => canvas };
        match self.transform.invert() {
            Some(inv) if !self.transform.is_identity() => {
                let pts = [PointF::new(r.x, r.y), PointF::new(r.right(), r.y), PointF::new(r.right(), r.bottom()), PointF::new(r.x, r.bottom())].map(|p| inv.apply(p));
                let minx = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min);
                let miny = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
                let maxx = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
                let maxy = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                RectangleF::new(minx, miny, maxx - minx, maxy - miny)
            }
            _ => r,
        }
    }
    pub fn transform_point(&self, p: PointF) -> PointF { self.transform.apply(p) }

    /// Graphics.MeasureString(s, font).
    pub fn measure_string(&self, s: &str, f: &crate::text::Font) -> crate::geom::SizeF { crate::text::measure_string(s, f, 0.0) }
    /// Graphics.MeasureString(s, font, width).
    pub fn measure_string_w(&self, s: &str, f: &crate::text::Font, width: i32) -> crate::geom::SizeF { crate::text::measure_string(s, f, width as f32) }

    /// DrawString(s, font, brush, layoutRect, format): clipped to the rect, as GDI+ does without NoClip.
    pub fn draw_string(&mut self, s: &str, f: &crate::text::Font, c: Color, r: RectangleF, sf: &crate::text::StringFormat) {
        if s.is_empty() { return; }
        let _p = crate::prof::span("draw_string");
        let kept = self.save();
        self.set_clip_rect(r);
        let ascent = f.size * f.face.ascent / f.face.upem;
        let t = self.transform.0;
        if f.face.icons {
            for (o, line) in crate::text::place(s, f, r, sf) {
                for &(_, ch, cx, _) in &line.chars {
                    // Hinted like the glyph it stands for: a whole-pixel em at the rounded size, on the pixel grid.
                    let em = f.size.round();
                    crate::icons::draw(self, ch, RectangleF::new((o.x + cx).floor(), (o.y + ascent).floor() - em, em, em), c);
                }
            }
        } else if t.sx == 1.0 && t.sy == 1.0 && t.kx == 0.0 && t.ky == 0.0 {
            // swash's hinted glyph masks, composited with GDI+'s blend inside the clip.
            let mut glyphs = Vec::new();
            // Lines wholly above or below what can be drawn (the clip, the canvas) give no pixels: 3 em of margin.
            let canvas = RectangleF::new(0.0, 0.0, self.bmp.width as f32, self.bmp.height as f32);
            let vis = self.clip.as_ref().map_or(canvas, |c| RectangleF::intersect(c.bounds, canvas));
            for (o, line) in crate::text::place(s, f, r, sf) {
                let base = (o.y + ascent).floor() + t.ty;
                if base + 3.0 * f.size < vis.y || base - 3.0 * f.size > vis.bottom() { continue; }
                glyphs.extend(crate::text::glyph_images(f, &line, o.x + t.tx, base));
            }
            if self.rec.is_some() { self.record(OpKind::Glyphs { glyphs, color: c }); } else { self.exec_glyphs(&glyphs, c); }
        } else {
            let mut path = GraphicsPath::new();
            path.fill_mode = FillMode::Winding;
            for (o, line) in crate::text::place(s, f, r, sf) {
                crate::text::glyph_path(f, &line, o.x, (o.y + ascent).floor(), &mut path);
            }
            let aa = self.smoothing_mode;
            self.smoothing_mode = SmoothingMode::AntiAlias;
            self.fill_path(&Brush::Solid(c), &path);
            self.smoothing_mode = aa;
        }
        self.restore(kept);
    }

    fn exec_glyphs(&mut self, glyphs: &[(i32, i32, u32, u32, std::sync::Arc<Vec<u8>>)], c: Color) {
        let (lx0, ly0, lx1, ly1) = self.limit;
        for (gx, gy, gw, gh, a) in glyphs {
            let (gx, gy, gw, gh) = (*gx, *gy, *gw as i32, *gh as i32);
            for j in 0..gh {
                for i in 0..gw {
                    let (x, y) = (gx + i, gy + j);
                    let cov = a[(j * gw + i) as usize] as u32;
                    if cov == 0 || x < lx0 || y < ly0 || x >= lx1 || y >= ly1 { continue; }
                    let k = (y * self.bmp.width + x) as usize;
                    if self.clip.as_ref().map_or(false, |c| !c.at(x, y)) { continue; }
                    self.bmp.data[k] = blend(self.bmp.data[k], c, cov);
                }
            }
        }
    }

    /// DrawImage(img, rect) with NearestNeighbor and PixelOffsetMode.Half: each device pixel whose center lands in
    /// the rect takes the source pixel under that center.
    pub fn draw_image_nearest(&mut self, img: &Bitmap, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::Nearest { img: Img(std::sync::Arc::new(img.clone())), r }); return; }
        self.exec_nearest(img, r);
    }
    /// draw_image_nearest of a kept picture: a recorded frame holds it without a copy.
    pub fn draw_image_nearest_shared(&mut self, img: &std::sync::Arc<Bitmap>, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::Nearest { img: Img(img.clone()), r }); return; }
        self.exec_nearest(img, r);
    }
    fn exec_nearest(&mut self, img: &Bitmap, r: RectangleF) {
        let _p = crate::prof::span("image nearest");
        let Some(inv) = self.transform.invert() else { return };
        let corners = [PointF::new(r.x, r.y), PointF::new(r.right(), r.y), PointF::new(r.right(), r.bottom()), PointF::new(r.x, r.bottom())].map(|q| self.transform.apply(q));
        let x0 = corners.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let y0 = corners.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let x1 = (corners.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as i32).min(self.bmp.width);
        let y1 = (corners.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as i32).min(self.bmp.height);
        let (x0, y0, x1, y1) = (x0.max(self.limit.0), y0.max(self.limit.1), x1.min(self.limit.2), y1.min(self.limit.3));
        let (sw, sh) = (img.width as f32, img.height as f32);
        // Axis-aligned: GDI+ steps across a row's texels in 16.16 fixed point from the first pixel center; rows map
        // exactly (probed, t42, t43).
        let t = self.transform.0;
        let dda = |o: f32, len: f32, n: f32| -> Option<(i32, i64, i64)> {
            if len <= 0.0 { return None; }
            let d0 = (o - 0.5).ceil() as i32;
            let k = n as f64 / len as f64;
            Some((d0, ((d0 as f64 + 0.5 - o as f64) * k * 65536.0).round() as i64, (k * 65536.0).round() as i64))
        };
        let ax = if t.kx == 0.0 && t.ky == 0.0 && t.sx > 0.0 && t.sy > 0.0 && r.width > 0.0 && r.height > 0.0 {
            dda(corners[0].x.min(corners[1].x), (corners[1].x - corners[0].x).abs(), sw)
        } else { None };
        // Rows: the same stepping in 4.28 fixed point (probed, nnrows 1-5, and the town's exact row boundaries).
        let ay = ax.map(|_| {
            let (o, len) = (corners[0].y.min(corners[3].y) as f64, (corners[3].y - corners[0].y).abs() as f64);
            let (d0, k) = ((o - 0.5).ceil() as i64, sh as f64 / len);
            (d0, ((d0 as f64 + 0.5 - o) * k * 268435456.0).round() as i64, (k * 268435456.0).round() as i64)
        });
        let pick = |a: &Option<(i32, i64, i64)>, d: i32, exact: f32, n: i32| -> i32 {
            match a { Some((d0, start, step)) => (((start + (d - d0) as i64 * step) >> 16) as i32).clamp(0, n - 1), None => exact as i32 }
        };
        for dy in y0..y1 {
            for dx in x0..x1 {
                let w = inv.apply(PointF::new(dx as f32 + 0.5, dy as f32 + 0.5));
                let u = (w.x - r.x) / r.width * sw;
                let v = (w.y - r.y) / r.height * sh;
                if u < 0.0 || v < 0.0 || u >= sw || v >= sh { continue; }
                let i = (dy * self.bmp.width + dx) as usize;
                if self.clip.as_ref().map_or(false, |c| !c.at(dx, dy)) { continue; }
                let src = img.get(pick(&ax, dx, u, img.width), match ay { Some((d0, start, step)) => ((start + (dy as i64 - d0) * step) >> 28).clamp(0, img.height as i64 - 1) as i32, None => v as i32 });
                if src.a == 255 { self.unmark_px(i); }
                self.bmp.data[i] = blend(self.bmp.data[i], img.get(pick(&ax, dx, u, img.width), match ay { Some((d0, start, step)) => ((start + (dy as i64 - d0) * step) >> 28).clamp(0, img.height as i64 - 1) as i32, None => v as i32 }), 255);
            }
        }
    }

    /// DrawImage(img, rect), Bilinear: each device pixel's integer point maps back to the image, texel centers on
    /// integers, no prefilter, hard edges (probed, t29).
    pub fn draw_image_bilinear(&mut self, img: &Bitmap, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::Bilinear { img: Img(std::sync::Arc::new(img.clone())), r }); return; }
        self.exec_bilinear(img, r);
    }
    pub fn draw_image_bilinear_shared(&mut self, img: &std::sync::Arc<Bitmap>, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::Bilinear { img: Img(img.clone()), r }); return; }
        self.exec_bilinear(img, r);
    }
    fn exec_bilinear(&mut self, img: &Bitmap, r: RectangleF) {
        let _p = crate::prof::span("image bilinear");
        let Some(inv) = self.transform.invert() else { return };
        let corners = [PointF::new(r.x, r.y), PointF::new(r.right(), r.y), PointF::new(r.right(), r.bottom()), PointF::new(r.x, r.bottom())].map(|q| self.transform.apply(q));
        let x0 = corners.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let y0 = corners.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let x1 = (corners.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as i32).min(self.bmp.width);
        let y1 = (corners.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as i32).min(self.bmp.height);
        let (x0, y0, x1, y1) = (x0.max(self.limit.0), y0.max(self.limit.1), x1.min(self.limit.2), y1.min(self.limit.3));
        let (sw, sh) = (img.width as f32, img.height as f32);
        // Back to world in float32 (an f64 inverse moves rows; probed, t28).
        let t = self.transform.0;
        let back = |x: f32, y: f32| -> PointF {
            if t.kx == 0.0 && t.ky == 0.0 { return PointF::new((x - t.tx) / t.sx, (y - t.ty) / t.sy); }
            inv.apply(PointF::new(x, y))
        };
        // Texels beyond the picture's edge are transparent (probed upscaling, t44).
        let px = |xx: i32, yy: i32| -> [f32; 4] {
            if xx < 0 || yy < 0 || xx >= img.width || yy >= img.height { return [0.0; 4]; }
            let c = img.get(xx, yy);
            let a = c.a as f32;
            [a, c.r as f32 * a / 255.0, c.g as f32 * a / 255.0, c.b as f32 * a / 255.0]
        };
        for dy in y0..y1 {
            for dx in x0..x1 {
                let w = back(dx as f32, dy as f32);
                let u = if r.width == sw { w.x - r.x } else { (w.x - r.x) / r.width * sw };
                let v = if r.height == sh { w.y - r.y } else { (w.y - r.y) / r.height * sh };
                if u < 0.0 || v < 0.0 || u >= sw || v >= sh { continue; }
                let i = (dy * self.bmp.width + dx) as usize;
                if self.clip.as_ref().map_or(false, |c| !c.at(dx, dy)) { continue; }
                let (fu, fv) = (u, v);
                let (ix, iy) = (fu.floor() as i32, fv.floor() as i32);
                let (ax, ay) = (fu - ix as f32, fv - iy as f32);
                let (p00, p10, p01, p11) = (px(ix, iy), px(ix + 1, iy), px(ix, iy + 1), px(ix + 1, iy + 1));
                let mut o = [0u32; 4];
                for k in 0..4 {
                    let top = p00[k] + (p10[k] - p00[k]) * ax;
                    let bot = p01[k] + (p11[k] - p01[k]) * ax;
                    o[k] = (top + (bot - top) * ay + 0.5).clamp(0.0, 255.0) as u32;
                }
                for k in 1..4 { o[k] = o[k].min(o[0]); }
                self.bmp.data[i] = blend_pm_n(self.bmp.data[i], o, 1, 1);
            }
        }
    }

    /// DrawImage(img, rect), HighQualityBilinear (probed with random pictures): a separable tent of half-width
    /// max(1, scale) around the source point of each device pixel's integer point, integrated over each texel, with
    /// transparent beyond the edges; premultiplied. A model, within 1-2 of GDI+ on opaque pixels (its rounding isn't).
    pub fn draw_image_hq_bilinear(&mut self, img: &Bitmap, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::HqBilinear { img: Img(std::sync::Arc::new(img.clone())), r }); return; }
        self.exec_hq_bilinear(img, r);
    }
    pub fn draw_image_hq_bilinear_shared(&mut self, img: &std::sync::Arc<Bitmap>, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::HqBilinear { img: Img(img.clone()), r }); return; }
        self.exec_hq_bilinear(img, r);
    }
    fn exec_hq_bilinear(&mut self, img: &Bitmap, r: RectangleF) {
        let t = self.transform.0;
        // Only scales and moves (the launcher's use); anything else draws as plain bilinear.
        if t.kx != 0.0 || t.ky != 0.0 || r.width <= 0.0 || r.height <= 0.0 { self.exec_bilinear(img, r); return; }
        let (sw, sh) = (img.width, img.height);
        let (dx0, dy0) = (r.x * t.sx + t.tx, r.y * t.sy + t.ty);
        let (dw, dh) = (r.width * t.sx, r.height * t.sy);
        let (kx, ky) = (sw as f64 / dw as f64, sh as f64 / dh as f64);
        let x0 = (dx0.ceil() as i32).max(0).max(self.limit.0);
        let y0 = (dy0.ceil() as i32).max(0).max(self.limit.1);
        let x1 = ((dx0 + dw).ceil() as i32).min(self.bmp.width).min(self.limit.2);
        let y1 = ((dy0 + dh).ceil() as i32).min(self.bmp.height).min(self.limit.3);
        if x0 >= x1 || y0 >= y1 { return; }
        let tent = |a: f64, b: f64, h: f64| -> f64 {
            let f = |x: f64| { let x = x.clamp(-h, h); if x < 0.0 { x + x * x / (2.0 * h) } else { x - x * x / (2.0 * h) } };
            f(b) - f(a)
        };
        // (first texel, weights) for each device column or row.
        let taps = |d0: i32, d1: i32, o: f32, k: f64, n: i32| -> Vec<(i32, Vec<f64>)> {
            let h = k.max(1.0);
            (d0..d1).map(|d| {
                let c = (d as f64 - o as f64) * k;
                if (k - 1.0).abs() < 1e-9 { let j = c.round() as i32; return (j, vec![if (0..n).contains(&j) { 1.0 } else { 0.0 }]); }
                let (j0, j1) = (((c - h - 0.5).floor() as i32).max(0), ((c + h + 0.5).ceil() as i32).min(n - 1));
                (j0, (j0..=j1).map(|j| tent(j as f64 - 0.5 - c, j as f64 + 0.5 - c, h) / h).collect())
            }).collect()
        };
        // Columns take their phase from x reflected about the pixel's middle (within 1/32 of a whole pixel: about that
        // pixel); rows use y as it is (probed, t45).
        let n = dx0.round();
        let ox = if (dx0 - n).abs() < 1.0 / 32.0 { 2.0 * n - dx0 } else { 2.0 * dx0.floor() + 1.0 - dx0 };
        let cols = taps(x0, x1, ox, kx, sw);
        let rows = taps(y0, y1, dy0, ky, sh);
        let pm: Vec<[f64; 4]> = img.data.iter().map(|v| { let a = (v >> 24) as f64; [a, ((v >> 16) & 255) as f64 * a / 255.0, ((v >> 8) & 255) as f64 * a / 255.0, (v & 255) as f64 * a / 255.0] }).collect();
        for (yi, dy) in (y0..y1).enumerate() {
            let (ry0, rw) = &rows[yi];
            // This row's texels summed down first, then across.
            let mut line = vec![[0f64; 4]; sw as usize];
            for (k, w) in rw.iter().enumerate() {
                let sy = ry0 + k as i32;
                if *w == 0.0 || sy < 0 || sy >= sh { continue; }
                let base = (sy * sw) as usize;
                for (x, l) in line.iter_mut().enumerate() { let p = pm[base + x]; for c in 0..4 { l[c] += p[c] * w; } }
            }
            for (xi, dx) in (x0..x1).enumerate() {
                if self.clip.as_ref().map_or(false, |c| !c.at(dx, dy)) { continue; }
                let (cx0, cw) = &cols[xi];
                let mut acc = [0f64; 4];
                for (k, w) in cw.iter().enumerate() {
                    let sx = cx0 + k as i32;
                    if *w == 0.0 || sx < 0 || sx >= sw { continue; }
                    let p = line[sx as usize];
                    for c in 0..4 { acc[c] += p[c] * w; }
                }
                let a = (acc[0] + 0.5).floor().clamp(0.0, 255.0) as u32;
                if a == 0 { continue; }
                let mut o = [a, 0, 0, 0];
                for c in 1..4 { o[c] = ((acc[c] + 0.5).floor().clamp(0.0, 255.0) as u32).min(a); }
                let i = (dy * self.bmp.width + dx) as usize;
                self.bmp.data[i] = blend_pm_n(self.bmp.data[i], o, 1, 1);
            }
        }
    }

    /// DrawImage(img, rect), HighQualityBicubic: scales and moves bit-exact with GDI+ (`bicubic::stretch`), rotations
    /// through the 4x4 model (`bicubic::sample_affine`).
    pub fn draw_image_hq_bicubic(&mut self, img: &Bitmap, r: RectangleF) {
        if self.rec.is_some() { self.record(OpKind::HqBicubic { img: Img(std::sync::Arc::new(img.clone())), r }); return; }
        self.exec_hq_bicubic(img, r);
    }
    fn exec_hq_bicubic(&mut self, img: &Bitmap, r: RectangleF) {
        let t = self.transform.0;
        if r.width <= 0.0 || r.height <= 0.0 || img.width <= 0 || img.height <= 0 { return; }
        let bounds = (self.limit.0.max(0), self.limit.1.max(0), self.limit.2.min(self.bmp.width), self.limit.3.min(self.bmp.height));
        if t.kx == 0.0 && t.ky == 0.0 && t.sx > 0.0 && t.sy > 0.0 {
            let (dx, dy) = ((r.x * t.sx + t.tx) as f64, (r.y * t.sy + t.ty) as f64);
            let (dw, dh) = ((r.width * t.sx) as f64, (r.height * t.sy) as f64);
            // Same size and next to a whole pixel: GDI+ copies the picture there, into the pixels the rect covers.
            if let (true, Some(sx), Some(sy)) = (dw == img.width as f64 && dh == img.height as f64, crate::bicubic::snap(dx), crate::bicubic::snap(dy)) {
                let (x0, y0) = ((dx.ceil() as i32).max(bounds.0), (dy.ceil() as i32).max(bounds.1));
                let (x1, y1) = (((dx + dw).ceil() as i32).min(bounds.2).min(sx + img.width), ((dy + dh).ceil() as i32).min(bounds.3).min(sy + img.height));
                for y in y0..y1 {
                    for x in x0..x1 {
                        if self.clip.as_ref().map_or(false, |c| !c.at(x, y)) { continue; }
                        let p = crate::bicubic::premul(img.data[((y - sy) * img.width + x - sx) as usize]).map(|v| v as u8);
                        self.put_pm(x, y, p);
                    }
                }
                return;
            }
            let (x0, y0, x1, y1, px) = crate::bicubic::stretch(&img.data, img.width, img.height, dx, dy, dw, dh, bounds);
            let w = (x1 - x0) as usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = px[(y - y0) as usize * w + (x - x0) as usize];
                    if self.clip.as_ref().map_or(false, |c| !c.at(x, y)) { continue; }
                    self.put_pm(x, y, p);
                }
            }
            return;
        }
        let Some(inv) = self.transform.invert() else { return };
        let (bx0, by0, bx1, by1) = meet(self.device_box(r), bounds);
        let (pw, ph) = ((r.width.round() as i32).max(1), (r.height.round() as i32).max(1));
        let pre = crate::bicubic::prescale(&img.data, img.width, img.height, pw, ph);
        for y in by0..by1 {
            for x in bx0..bx1 {
                if self.clip.as_ref().map_or(false, |c| !c.at(x, y)) { continue; }
                let q = inv.apply(PointF::new(x as f32, y as f32));
                let (u, v) = ((q.x - r.x) as f64 * pw as f64 / r.width as f64, (q.y - r.y) as f64 * ph as f64 / r.height as f64);
                if u <= -2.0 || v <= -2.0 || u >= pw as f64 + 1.0 || v >= ph as f64 + 1.0 { continue; }
                let p = crate::bicubic::sample_affine(&pre, pw, ph, u, v);
                self.put_pm(x, y, p);
            }
        }
    }
    /// A filtered image pixel (premultiplied A, R, G, B) over the bitmap: onto a transparent pixel it lands as it is.
    fn put_pm(&mut self, x: i32, y: i32, p: [u8; 4]) {
        if p[0] == 0 { return; }
        let i = (y * self.bmp.width + x) as usize;
        if p[0] == 255 { self.unmark_px(i); }
        self.bmp.data[i] = if self.bmp.data[i] >> 24 == 0 { crate::bicubic::unpremul(p) } else { blend_pm_n(self.bmp.data[i], [p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32], 1, 1) };
    }

    /// DrawImage(img, Rectangle, src, Pixel, ImageAttributes) with a ColorMatrix alpha (Matrix33): the setup leave.
    pub fn draw_image_alpha(&mut self, img: &Bitmap, r: RectangleF, alpha: f32) {
        let mut faded = img.clone();
        for px in faded.data.iter_mut() {
            let a = ((*px >> 24) as f64 * alpha as f64).round() as u32;
            *px = (*px & 0x00FF_FFFF) | (a.min(255) << 24);
        }
        self.draw_image_bilinear(&faded, r);
    }

    /// DrawImageUnscaled: a straight copy at a whole-pixel offset; scaled or at a fractional offset GDI+ samples it
    /// bilinearly at each device pixel's integer point (probed, t54).
    pub fn draw_image_unscaled(&mut self, img: &Bitmap, x: i32, y: i32) {
        if self.rec.is_some() { self.draw_image_unscaled_shared(&std::sync::Arc::new(img.clone()), x, y); return; }
        let t = self.transform.0;
        if t.sx != 1.0 || t.sy != 1.0 || t.kx != 0.0 || t.ky != 0.0 || t.tx.fract() != 0.0 || t.ty.fract() != 0.0 {
            self.draw_image_bilinear(img, RectangleF::new(x as f32, y as f32, img.width as f32, img.height as f32));
            return;
        }
        self.exec_unscaled(img, x, y);
    }
    pub fn draw_image_unscaled_shared(&mut self, img: &std::sync::Arc<Bitmap>, x: i32, y: i32) {
        let _p = crate::prof::span("image unscaled");
        let t = self.transform.0;
        if t.sx != 1.0 || t.sy != 1.0 || t.kx != 0.0 || t.ky != 0.0 || t.tx.fract() != 0.0 || t.ty.fract() != 0.0 {
            self.draw_image_bilinear_shared(img, RectangleF::new(x as f32, y as f32, img.width as f32, img.height as f32));
            return;
        }
        if self.rec.is_some() { self.record(OpKind::Unscaled { img: Img(img.clone()), x, y }); return; }
        self.exec_unscaled(img, x, y);
    }
    fn exec_unscaled(&mut self, img: &Bitmap, x: i32, y: i32) {
        let o = self.transform.apply(PointF::new(x as f32, y as f32));
        let (ox, oy) = (o.x.round() as i32, o.y.round() as i32);
        let (lx0, ly0, lx1, ly1) = self.limit;
        // The columns inside the limit, once; an opaque source pixel over anything is itself (blend at 255).
        let (sx0, sx1) = ((lx0 - ox).max(0), (lx1 - ox).min(img.width));
        if sx0 >= sx1 { return; }
        let bw = self.bmp.width;
        for sy in 0..img.height {
            let dy = oy + sy;
            if dy < ly0 || dy >= ly1 { continue; }
            let src = &img.data[(sy * img.width) as usize..((sy + 1) * img.width) as usize];
            for sx in sx0..sx1 {
                let dx = ox + sx;
                if self.clip.as_ref().map_or(false, |c| !c.at(dx, dy)) { continue; }
                let i = (dy * bw + dx) as usize;
                let v = src[sx as usize];
                if v >> 24 == 255 { self.unmark_px(i); }
                self.bmp.data[i] = if v >> 24 == 255 { v } else { blend(self.bmp.data[i], Color::from_argb((v >> 24) as i32, (v >> 16 & 255) as i32, (v >> 8 & 255) as i32, (v & 255) as i32), 255) };
            }
        }
    }
}



type Run = (i32, i32, i32, u32);
thread_local! {
    /// A linear gradient's runs of one color (row, first and last pixel, premultiplied color) by every input that
    /// decides them, so a fill drawn again isn't searched again. At most 16 MB of runs.
    static RUN_CACHE: std::cell::RefCell<(std::collections::HashMap<Vec<u64>, std::rc::Rc<Vec<Run>>>, usize)> = std::cell::RefCell::new((std::collections::HashMap::new(), 0));
}
fn run_key(lg: &crate::brush::LinearGradient, k: (f64, f64, f64, f64), e: ([f32; 4], [f32; 4]), inv: Option<&Matrix>, off: f32, cov: (i32, i32, i32, i32), clip: Option<(i32, i32, i32, i32)>) -> Vec<u64> {
    let f = |v: f32| v.to_bits() as u64;
    let c = |c: Color| (c.a as u64) << 24 | (c.r as u64) << 16 | (c.g as u64) << 8 | c.b as u64;
    let mut key = vec![f(lg.rect.x), f(lg.rect.y), f(lg.rect.width), f(lg.rect.height), c(lg.c1), c(lg.c2), f(lg.angle), lg.wrap as u64,
        k.0.to_bits(), k.1.to_bits(), k.2.to_bits(), k.3.to_bits(), f(off), cov.0 as u64, cov.1 as u64, cov.2 as u64, cov.3 as u64];
    key.extend(e.0.iter().chain(e.1.iter()).map(|v| f(*v)));
    key.push(lg.colors.len() as u64);
    for (col, pos) in &lg.colors { key.push(c(*col)); key.push(f(*pos)); }
    match inv { Some(m) => { let t = m.0; key.extend([1, f(t.sx), f(t.kx), f(t.ky), f(t.sy), f(t.tx), f(t.ty)]); } None => key.push(0) }
    match clip { Some((a, b, w, h)) => key.extend([1, a as u64, b as u64, w as u64, h as u64]), None => key.push(0) }
    key
}
/// The runs of one color along each row a linear gradient (no Blend, rows level) paints: along a row its color is
/// monotone per channel within one tile, so equal colors at two pixels hold for all between. Found by galloping and
/// halving from a guess at the next rounding step; the colors are the per-pixel ones.
fn linear_runs<'c>(lg: &crate::brush::LinearGradient, k: (f64, f64, f64, f64), e: ([f32; 4], [f32; 4]), inv: Option<&Matrix>, off: f32, cov: &Coverage, span: &impl Fn(i32) -> Option<(i32, i32, Option<&'c [u8]>)>) -> Vec<Run> {
    let mut out = Vec::new();
    for y in cov.y0..cov.y0 + cov.h {
        let Some((x0, x1, _)) = span(y) else { continue };
        let tparam = |x: i32| -> f64 {
            let dev = PointF::new(x as f32 - off, y as f32 - off);
            lg.param_with(k, inv.map_or(dev, |mm| mm.apply(dev)))
        };
        let key_t = |t: f64| -> ([u32; 4], i64) {
            let tile = if lg.wrap == crate::brush::WrapMode::Clamp { 0 } else if t.is_finite() { t.floor() as i64 } else { i64::MIN };
            (lg.color_at_pm(t, e), tile)
        };
        let key = |x: i32| key_t(tparam(x));
        // The parameter's step per pixel along this row (only for guessing where a run ends).
        let slope = if x1 - 1 > x0 { (tparam(x1 - 1) - tparam(x0)) / (x1 - 1 - x0) as f64 } else { 0.0 };
        let guess = |x: i32, t: f64| -> Option<i32> { run_guess(lg, e, t, slope).map(|d| x.saturating_add(d)) };
        let mut x = x0;
        while x < x1 {
            let tx = tparam(x);
            let c = key_t(tx);
            // A NaN parameter (no tile) is taken pixel by pixel.
            let mut lo = x;
            if c.1 != i64::MIN {
                let mut hi = x1;
                // A guess at the run's last pixel from the colors' next rounding step, checked: the run is still
                // found by its colors (equal at two pixels of a tile: equal between).
                if let Some(g) = guess(x, tx) {
                    let g = g.min(x1 - 1);
                    if g > x { if key(g) == c { lo = g; } else { hi = g; } }
                }
                let mut step = 1;
                while lo + step < hi { if key(lo + step) == c { lo += step; step *= 2; } else { hi = lo + step; break; } }
                while hi - lo > 1 { let mid = lo + (hi - lo) / 2; if key(mid) == c { lo = mid; } else { hi = mid; } }
            }
            let pm = c.0;
            out.push((y, x, lo, pm[0] << 24 | pm[1] << 16 | pm[2] << 8 | pm[3]));
            x = lo + 1;
        }
    }
    out
}


/// One run of a fill's pixels: skipped where uncovered (or outside the clip mask), the opaque color itself where fully
/// covered, else `blend` of the pixel and its coverage, the last result reused for the same pixel and coverage.
#[inline]
fn put_run(drow: &mut [u32], crow: &[u8], m: Option<&[u8]>, full: u32, opaque: Option<u32>, blend: impl Fn(u32, u8) -> u32) {
    let mut memo: (u8, u32, u32) = (0, 0, 0);
    let mut one = |d: &mut u32, n: u8| {
        if n == 0 { return; }
        if let (Some(px), true) = (opaque, n as u32 >= full) { *d = px; return; }
        if memo.0 == n && memo.1 == *d { *d = memo.2; return; }
        let out = blend(*d, n);
        memo = (n, *d, out);
        *d = out;
    };
    match m {
        None => for (d, &n) in drow.iter_mut().zip(crow) { one(d, n) },
        Some(m) => for ((d, &n), &mm) in drow.iter_mut().zip(crow).zip(m) { if mm != 0 { one(d, n) } },
    }
}

/// How many pixels on a linear gradient's color stays the same along a row, roughly: the nearest next rounding step
/// of any channel (or the tile's or clamp's edge). Only a guess; the caller checks it against the real colors.
fn run_guess(lg: &crate::brush::LinearGradient, (pa, pb): ([f32; 4], [f32; 4]), t: f64, s: f64) -> Option<i32> {
    use crate::brush::WrapMode;
    if s == 0.0 || !s.is_finite() || !t.is_finite() { return None; }
    let (k, dk, mut dx) = match lg.wrap {
        WrapMode::Clamp => {
            if t <= 0.0 { return Some(if s > 0.0 { ((0.0 - t) / s).floor().min(1e9) as i32 } else { i32::MAX }); }
            if t >= 1.0 { return Some(if s < 0.0 { ((1.0 - t) / s).floor().min(1e9) as i32 } else { i32::MAX }); }
            (t, s, if s > 0.0 { (1.0 - t) / s } else { -t / s })
        }
        _ => {
            let fl = t.floor();
            let f = t - fl;
            let odd = matches!(lg.wrap, WrapMode::TileFlipX | WrapMode::TileFlipXY) && (fl as i64).rem_euclid(2) == 1;
            (if odd { 1.0 - f } else { f }, if odd { -s } else { s }, if s > 0.0 { (1.0 - f) / s } else { -f / s })
        }
    };
    for i in 0..4 {
        let d = (pb[i] - pa[i]) as f64;
        if d == 0.0 { continue; }
        let v = pa[i] as f64 + d * k + 0.5;
        let n = v.floor();
        let target = if (dk > 0.0) == (d > 0.0) { n + 1.0 } else { n };
        let kt = (target - 0.5 - pa[i] as f64) / d;
        dx = dx.min((kt - k) / dk);
    }
    if !dx.is_finite() { return Some(i32::MAX); }
    Some(dx.max(0.0).floor().min(1e9) as i32)
}

/// A gradient brush's per-fill constants, worked out once instead of per pixel (same arithmetic).
enum Prepared<'a> {
    Solid([u32; 4]),
    Linear(&'a crate::brush::LinearGradient, (f64, f64, f64, f64), Option<([f32; 4], [f32; 4])>),
    Ellipse(&'a crate::brush::EllipseGradient, std::rc::Rc<Vec<(f64, f64)>>, (f64, f64)),
}
impl<'a> Prepared<'a> {
    fn new(brush: &'a Brush) -> Prepared<'a> {
        match brush {
            Brush::Solid(c) => Prepared::Solid(crate::brush::premul(*c).map(|v| v as u32)),
            Brush::Linear(lg) => Prepared::Linear(lg, lg.param_prep(), lg.ends_pm()),
            Brush::Ellipse(eg) => {
                // The center is the mean of the path's points (13 for an ellipse, its start twice): probed, t19.
                let mut path = GraphicsPath::new();
                path.add_ellipse(eg.rect);
                let n = path.pts.len() as f64;
                let c = (path.pts.iter().map(|q| q.p.x as f64).sum::<f64>() / n, path.pts.iter().map(|q| q.p.y as f64).sum::<f64>() / n);
                Prepared::Ellipse(eg, ellipse_poly(eg.rect), c)
            }
        }
    }
    #[inline]
    fn color(&self, p: PointF) -> [u32; 4] {
        match self {
            Prepared::Solid(c) => *c,
            Prepared::Linear(lg, k, Some(e)) => lg.color_at_pm(lg.param_with(*k, p), *e),
            Prepared::Linear(lg, k, None) => lg.color_at(lg.param_with(*k, p)),
            Prepared::Ellipse(eg, poly, c) => crate::brush::lerp_pm(eg.center, eg.surround, ray_ratio(poly, *c, (p.x as f64, p.y as f64)) as f32),
        }
    }
}

/// GDI+ widens antialiased pens of under a pixel to a pixel.
fn pen_width(w: f64, _m: SmoothingMode) -> f64 { w }

/// The dashes of a pen as open polylines (or the whole line, solid).
fn dashes(line: &[(f64, f64)], closed: bool, pen: &Pen, width: f64) -> Vec<(Vec<(f64, f64)>, bool)> {
    let pattern: &[f64] = match pen.dash_style {
        DashStyle::Solid => return vec![(line.to_vec(), closed)],
        DashStyle::Dash => &[3.0, 1.0],
        DashStyle::Dot => &[1.0, 1.0],
    };
    let mut pts = line.to_vec();
    // A closed figure dashes from its last distinct point, through the closing segment first (probed, t24-t26).
    if closed && pts.len() > 1 {
        let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
        while pts.len() > 1 && close(pts[pts.len() - 1], pts[0]) { pts.pop(); }
        let last = pts[pts.len() - 1];
        pts.insert(0, last);
    }
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let (mut k, mut left, mut on) = (0usize, pattern[0] * width, true);
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[i], pts[i + 1]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let mut t = 0.0;
        if on && cur.is_empty() { cur.push(a); }
        while len - t > left {
            t += left;
            let p = (a.0 + (b.0 - a.0) * t / len, a.1 + (b.1 - a.1) * t / len);
            if on {
                cur.push(p);
                out.push((std::mem::take(&mut cur), false));
            } else {
                cur = vec![p];
            }
            on = !on;
            k = (k + 1) % pattern.len();
            left = pattern[k] * width;
        }
        left -= len - t;
        if on { cur.push(b); }
    }
    if on && cur.len() > 1 { out.push((cur, false)); }
    out
}


/// A path gradient's ellipse as GDI+ flattens it: four beziers cracked to a polygon (world coordinates).
fn ellipse_poly(r: RectangleF) -> std::rc::Rc<Vec<(f64, f64)>> {
    thread_local! { static LAST: std::cell::RefCell<Option<(RectangleF, std::rc::Rc<Vec<(f64, f64)>>)>> = const { std::cell::RefCell::new(None) }; }
    if let Some(v) = LAST.with(|l| l.borrow().as_ref().filter(|(k, _)| *k == r).map(|(_, v)| v.clone())) { return v; }
    let v = std::rc::Rc::new(ellipse_poly_new(r));
    LAST.with(|l| *l.borrow_mut() = Some((r, v.clone())));
    v
}
fn ellipse_poly_new(r: RectangleF) -> Vec<(f64, f64)> {
    let mut path = GraphicsPath::new();
    path.add_ellipse(r);
    let pts: Vec<(f64, f64)> = path.pts.iter().map(|q| (q.p.x as f64, q.p.y as f64)).collect();
    let mut out = vec![pts[0]];
    let c = crate::flatten::cfg();
    for k in 0..4 { out.extend(crate::flatten::crack_f64([pts[3 * k], pts[3 * k + 1], pts[3 * k + 2], pts[3 * k + 3]], &c)); }
    out
}
/// How far along the ray from the center to the polygon's edge a point lies (0 center, 1 edge, clamped).
fn ray_ratio(poly: &[(f64, f64)], c: (f64, f64), p: (f64, f64)) -> f64 {
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    if dx == 0.0 && dy == 0.0 { return 0.0; }
    let mut best = f64::MAX;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let den = dx * ey - dy * ex;
        if den.abs() < 1e-12 { continue; }
        let (ax, ay) = (a.0 - c.0, a.1 - c.1);
        let s = (ax * ey - ay * ex) / den;
        let u = (ax * dy - ay * dx) / den;
        if s > 0.0 && (-1e-9..=1.0 + 1e-9).contains(&u) && s < best { best = s; }
    }
    if best == f64::MAX { 1.0 } else { (1.0 / best).min(1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    // GDI+'s pixels for DrawImageUnscaled through a fractional translate (probe t55): bilinear at each device pixel's
    // integer point, hard edges, transparent beyond the picture.
    #[test]
    fn unscaled_at_a_fractional_offset_matches_gdiplus() {
        let mut img = Bitmap::new(3, 2);
        for y in 0..2 { img.data[(y * 3) as usize..(y * 3 + 3) as usize].copy_from_slice(&[0xFFC82800, 0xFF0078F0, 0xFFFAFAFA]); }
        let run = |tx: f32, ty: f32| {
            let mut g = Graphics::new(Bitmap::new(8, 4));
            g.clear(Color::from_argb(255, 10, 20, 30));
            g.translate_transform(tx, ty);
            g.draw_image_unscaled(&img, 0, 0);
            g.bmp.data
        };
        let (bg, f40, f75) = (0xFF0A141E, run(2.4, 1.0), run(2.75, 1.5));
        let row = |d: &Vec<u32>, y: usize| d[y * 8..y * 8 + 8].to_vec();
        assert_eq!(row(&f40, 0), vec![bg; 8]);
        for y in [1, 2] { assert_eq!(row(&f40, y), vec![bg, bg, bg, 0xFF505890, 0xFF96C6F6, 0xFF6A7076, bg, bg]); }
        assert_eq!(row(&f40, 3), vec![bg; 8]);
        assert_eq!(row(&f75, 1), vec![bg; 8]);
        assert_eq!(row(&f75, 2), vec![bg, bg, bg, 0xFF963C3C, 0xFF3F99F3, 0xFFBFC1C4, bg, bg]);
        assert_eq!(row(&f75, 3), vec![bg, bg, bg, 0xFF50282D, 0xFF245688, 0xFF646A71, bg, bg]);
    }
}
