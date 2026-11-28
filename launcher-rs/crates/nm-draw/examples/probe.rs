//! Runs a GdiProbe script through nm-draw (`probe run <script> <out>`), or compares two output folders
//! (`probe diff <gdi> <ours> [detail]`): per canvas, differing pixels and the largest channel difference.

use nm_draw::*;
use std::path::Path;

fn f(s: &str) -> f32 { s.parse().unwrap() }
fn col(a: &[&str], i: usize) -> Color { Color::from_argb(a[i].parse().unwrap(), a[i + 1].parse().unwrap(), a[i + 2].parse().unwrap(), a[i + 3].parse().unwrap()) }
fn rect(a: &[&str], i: usize) -> RectangleF { RectangleF::new(f(a[i]), f(a[i + 1]), f(a[i + 2]), f(a[i + 3])) }
fn pts(a: &[&str], i: usize) -> Vec<PointF> {
    let n: usize = a[i].parse().unwrap();
    (0..n).map(|k| PointF::new(f(a[i + 1 + 2 * k]), f(a[i + 2 + 2 * k]))).collect()
}
fn round(r: RectangleF, rad: f32) -> GraphicsPath {
    let mut p = GraphicsPath::new();
    let d = (rad * 2.0).min(r.width.min(r.height));
    if d <= 0.0 {
        p.add_rectangle(r);
        return p;
    }
    p.add_arc(RectangleF::new(r.x, r.y, d, d), 180.0, 90.0);
    p.add_arc(RectangleF::new(r.right() - d, r.y, d, d), 270.0, 90.0);
    p.add_arc(RectangleF::new(r.right() - d, r.bottom() - d, d, d), 0.0, 90.0);
    p.add_arc(RectangleF::new(r.x, r.bottom() - d, d, d), 90.0, 90.0);
    p.close_figure();
    p
}
fn cap(s: &str) -> LineCap {
    match s { "round" => LineCap::Round, "square" => LineCap::Square, _ => LineCap::Flat }
}
fn join(s: &str) -> LineJoin {
    match s { "round" => LineJoin::Round, "bevel" => LineJoin::Bevel, _ => LineJoin::Miter }
}

fn run(script: &str, out: &Path) {
    std::fs::create_dir_all(out).unwrap();
    let mut g: Option<Graphics> = None;
    let mut brush = Brush::Solid(Color::BLACK);
    let mut pen = Pen::new(Color::BLACK, 1.0);
    let mut saved: Vec<GraphicsState> = Vec::new();
    let mut img: Option<Bitmap> = None;
    for line in std::fs::read_to_string(script).unwrap().lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        let a: Vec<&str> = l.split_whitespace().collect();
        if a[0] == "canvas" {
            g = Some(Graphics::new(Bitmap::new(a[1].parse().unwrap(), a[2].parse().unwrap())));
            saved.clear();
            continue;
        }
        let gr = match g.as_mut() { Some(x) => x, None => continue };
        match a[0] {
            "clear" => gr.clear(col(&a, 1)),
            "smooth" => gr.smoothing_mode = if a[1] == "1" { SmoothingMode::AntiAlias } else { SmoothingMode::None },
            "pixoff" => gr.pixel_offset_mode = if a[1] == "half" { PixelOffsetMode::Half } else { PixelOffsetMode::None },
            "solid" => brush = Brush::Solid(col(&a, 1)),
            "linear" => {
                let mut lg = LinearGradient::new(rect(&a, 1), col(&a, 6), col(&a, 10), f(a[5]));
                let mut i = 14;
                if a.len() > i && a[i] == "wrap" {
                    lg.wrap = match a[i + 1] { "Clamp" => WrapMode::Clamp, "TileFlipX" => WrapMode::TileFlipX, "TileFlipXY" => WrapMode::TileFlipXY, _ => WrapMode::Tile };
                    i += 2;
                }
                if a.len() > i && a[i] == "blend" {
                    let n: usize = a[i + 1].parse().unwrap();
                    lg.blend = (0..n).map(|k| (f(a[i + 2 + k]), f(a[i + 2 + n + k]))).collect();
                }
                if a.len() > i && a[i] == "colors" {
                    let n: usize = a[i + 1].parse().unwrap();
                    lg.colors = (0..n).map(|k| (col(&a, i + 2 + 4 * k), f(a[i + 2 + 4 * n + k]))).collect();
                }
                brush = Brush::Linear(lg);
            }
            "pathellipse" => brush = Brush::Ellipse(EllipseGradient { rect: rect(&a, 1), center: col(&a, 5), surround: col(&a, 9) }),
            "pen" => {
                pen = Pen::new(col(&a, 1), f(a[5]));
                if a.len() > 8 {
                    pen.start_cap = cap(a[6]);
                    pen.end_cap = cap(a[7]);
                    pen.line_join = join(a[8]);
                }
                if a.len() > 10 && a[9] == "dash" {
                    pen.dash_style = match a[10] { "dash" => DashStyle::Dash, "dot" => DashStyle::Dot, _ => DashStyle::Solid };
                }
            }
            "fillrect" => gr.fill_rectangle(&brush, rect(&a, 1)),
            "fillellipse" => gr.fill_ellipse(&brush, rect(&a, 1)),
            "fillpie" => gr.fill_pie(&brush, f(a[1]), f(a[2]), f(a[3]), f(a[4]), f(a[5]), f(a[6])),
            "fillround" => gr.fill_path(&brush, &round(rect(&a, 1), f(a[5]))),
            "drawround" => gr.draw_path(&pen, &round(rect(&a, 1), f(a[5]))),
            "fillpoly" => gr.fill_polygon(&brush, &pts(&a, 1)),
            "drawrect" => gr.draw_rectangle(&pen, rect(&a, 1)),
            "drawellipse" => gr.draw_ellipse(&pen, rect(&a, 1)),
            "drawline" => gr.draw_line_f(&pen, f(a[1]), f(a[2]), f(a[3]), f(a[4])),
            "drawlines" => gr.draw_lines(&pen, &pts(&a, 1)),
            "drawcurve" => gr.draw_curve(&pen, &pts(&a, 1)),
            "drawpoly" => gr.draw_polygon(&pen, &pts(&a, 1)),
            "drawarc" => gr.draw_arc(&pen, rect(&a, 1), f(a[5]), f(a[6])),
            "drawbezier" => {
                let mut p = GraphicsPath::new();
                p.add_bezier(PointF::new(f(a[1]), f(a[2])), PointF::new(f(a[3]), f(a[4])), PointF::new(f(a[5]), f(a[6])), PointF::new(f(a[7]), f(a[8])));
                gr.draw_path(&pen, &p);
            }
            "cliprect" => gr.set_clip_rect(rect(&a, 1)),
            "clipround" => gr.set_clip_path(&round(rect(&a, 1), f(a[5]))),
            "clipellipse" => {
                let mut p = GraphicsPath::new();
                p.add_ellipse(rect(&a, 1));
                gr.set_clip_path(&p);
            }
            "resetclip" => gr.reset_clip(),
            "translate" => gr.translate_transform(f(a[1]), f(a[2])),
            "rotate" => gr.rotate_transform(f(a[1])),
            "scale" => gr.scale_transform(f(a[1]), f(a[2])),
            "save" => saved.push(gr.save()),
            "restore" => {
                let s = saved.pop().unwrap();
                gr.restore(s);
            }
            "text" => {
                let dir = std::env::var("NM_FONTS").unwrap_or_default();
                if let Some(f) = nm_draw::text::FontFace::load(&std::path::Path::new(&dir).join("NotoSansSymbols2-Regular.ttf")) { nm_draw::text::add_fallback(f); }
                let face = if a[1].starts_with('@') { nm_draw::text::FontFace::load(&std::path::Path::new(&dir).join(&a[1][1..])).unwrap() } else { nm_draw::text::FontFace::icon_face() };
                let f = nm_draw::text::Font::new(&face, f(a[2]));
                let al = |v: &str| match v { "1" => nm_draw::StringAlignment::Center, "2" => nm_draw::StringAlignment::Far, _ => nm_draw::StringAlignment::Near };
                let sf = nm_draw::StringFormat { alignment: al(a[12]), line_alignment: al(a[13]), no_wrap: a[11] != "1", line_limit: a[11] == "1", ellipsis: true };
                gr.draw_string(&a[14..].join(" "), &f, col(&a, 3), rect(&a, 7), &sf);
            }
            "grab" => img = Some(gr.bmp.clone()),
            "drawunscaled" => gr.draw_image_unscaled(img.as_ref().unwrap(), a[1].parse().unwrap(), a[2].parse().unwrap()),
            "drawimage" => gr.draw_image_bilinear(img.as_ref().unwrap(), rect(&a, 1)),
            "drawimagehq" => gr.draw_image_hq_bilinear(img.as_ref().unwrap(), rect(&a, 1)),
            "drawimagebc" => gr.draw_image_hq_bicubic(img.as_ref().unwrap(), rect(&a, 1)),
            // drawimagebcrot x y w h tx ty deg alpha: as CapturePc's swirl (the ColorMatrix alpha on the picture first).
            "drawimagebcrot" => {
                let mut faded = img.as_ref().unwrap().clone();
                for p in faded.data.iter_mut() { let al = ((*p >> 24) as f64 * f(a[8]) as f64).round() as u32; *p = (*p & 0x00FF_FFFF) | al.min(255) << 24; }
                let st = gr.save();
                gr.translate_transform(f(a[5]), f(a[6]));
                gr.rotate_transform(f(a[7]));
                gr.draw_image_hq_bicubic(&faded, RectangleF::new(a[1].parse::<i32>().unwrap() as f32, a[2].parse::<i32>().unwrap() as f32, a[3].parse::<i32>().unwrap() as f32, a[4].parse::<i32>().unwrap() as f32));
                gr.restore(st);
            }
            // noise seed w h opaque|alpha: System.Random pixels, as GdiProbe makes them.
            "noise" => {
                let (w, h): (i32, i32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
                let mut r = NetRandom::new(a[1].parse().unwrap());
                let mut b = Bitmap::new(w, h);
                for yy in 0..h { for xx in 0..w {
                    let al = if a[4] == "alpha" { r.next(256) } else { 255 };
                    let (cr, cg, cb) = (r.next(256), r.next(256), r.next(256));
                    b.set(xx, yy, Color::from_argb(al, cr, cg, cb));
                } }
                img = Some(b);
            }
            // loadbin <file>: a raw canvas as "out" writes it.
            "loadbin" => {
                let d = std::fs::read(a[1]).unwrap();
                let (w, h) = (i32::from_le_bytes(d[0..4].try_into().unwrap()), i32::from_le_bytes(d[4..8].try_into().unwrap()));
                let mut b = Bitmap::new(w, h);
                for (i, px) in b.data.iter_mut().enumerate() { *px = u32::from_le_bytes(d[8 + 4 * i..12 + 4 * i].try_into().unwrap()); }
                img = Some(b);
            }
            "drawimagenn" => gr.draw_image_nearest(img.as_ref().unwrap(), rect(&a, 1)),
            "drawimageattr" => {
                let r = RectangleF::new(f(a[1]), f(a[2]), f(a[3]), f(a[4]));
                gr.draw_image_alpha(img.as_ref().unwrap(), r, f(a[5]));
            }
            "grabsub" => {
                let (x, y, w, h): (i32, i32, i32, i32) = (a[1].parse().unwrap(), a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap());
                let mut b = Bitmap::new(w, h);
                for yy in 0..h { for xx in 0..w { b.set(xx, yy, gr.bmp.get(x + xx, y + yy)); } }
                img = Some(b);
            }
            "out" => gr.bmp.save_raw(&out.join(format!("{}.bin", a[1]))).unwrap(),
            _ => {}
        }
    }
}

fn load(p: &Path) -> (i32, i32, Vec<u8>) {
    let d = std::fs::read(p).unwrap();
    let w = i32::from_le_bytes(d[0..4].try_into().unwrap());
    let h = i32::from_le_bytes(d[4..8].try_into().unwrap());
    (w, h, d[8..].to_vec())
}

fn diff(a: &Path, b: &Path, detail: Option<&str>) {
    let mut names: Vec<_> = std::fs::read_dir(a).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().into_string().unwrap()).filter(|n| n.ends_with(".bin")).collect();
    names.sort();
    for n in names {
        if let Some(d) = detail {
            if !n.starts_with(d) { continue; }
        }
        let pb = b.join(&n);
        if !pb.exists() { continue; }
        let (w, h, x) = load(&a.join(&n));
        let (_, _, y) = load(&pb);
        let (mut cnt, mut max, mut sum) = (0, 0i32, 0i64);
        let mut hist = [0usize; 9];
        let mut first = Vec::new();
        for i in 0..(w * h) as usize {
            let mut m = 0;
            for c in 0..4 { m = m.max((x[i * 4 + c] as i32 - y[i * 4 + c] as i32).abs()); }
            if m > 0 {
                cnt += 1;
                sum += m as i64;
                hist[(m as usize).min(255).next_power_of_two().trailing_zeros() as usize] += 1;
                if first.len() < 12 {
                    let p = |v: &Vec<u8>| format!("{},{},{},{}", v[i * 4 + 3], v[i * 4 + 2], v[i * 4 + 1], v[i * 4]);
                    first.push(format!("({},{}) gdi {} ours {}", i as i32 % w, i as i32 / w, p(&x), p(&y)));
                }
            }
            max = max.max(m);
        }
        println!("{:24} {:6} px differ of {:7}, max {:3}, mean {:.1}, hist(1,2,4,..256) {:?}", n, cnt, w * h, max, if cnt > 0 { sum as f64 / cnt as f64 } else { 0.0 }, hist);
        if detail.is_some() {
            for l in first { println!("    {}", l); }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a[1].as_str() {
        "run" => run(&a[2], Path::new(&a[3])),
        "diff" => diff(Path::new(&a[2]), Path::new(&a[3]), a.get(4).map(|s| s.as_str())),
        _ => eprintln!("probe run <script> <out> | probe diff <gdi> <ours> [prefix]"),
    }
}
