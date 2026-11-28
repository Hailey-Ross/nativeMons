//! Runs the `measure` and `fontinfo` lines of a GdiProbe script through nm-draw's text layout.
use nm_draw::text::*;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let fonts = std::env::var("NM_FONTS").unwrap();
    let out = Path::new(&a[2]);
    std::fs::create_dir_all(out).unwrap();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 3 || !(t[0] == "measure") { continue; }
        let face = FontFace::load(&Path::new(&fonts).join(&t[2][1..])).unwrap();
        let f = Font::new(&face, t[3].parse().unwrap());
        let width: f32 = t[4].parse().unwrap();
        let wrap = t[5] == "1";
        let text = t[6..].join(" ").replace("\\n", "\n");
        let mut lines = vec![format!("height {} fontheight {} size {}", f.get_height(), f.height(), f.size)];
        let m = measure_string(&text, &f, width);
        lines.push(format!("measure {} {}", m.width, m.height));
        let l = layout(&text, &f, width, wrap);
        let mut boxes = vec![(0.0f32, 0.0f32, 0.0f32); text.len()];
        for (i, ln) in l.lines.iter().enumerate() {
            let y = (i as f32 * l.line_height).ceil();
            let end = ln.chars.iter().rposition(|c| c.1 != ' ').map_or(0, |e| e + 1);
            for (k, c) in ln.chars.iter().enumerate() {
                let w = if k >= end { 0.0 } else { c.3 };
                boxes[c.0] = (if k >= end { 0.0 } else { l.pad + c.2 }, if k >= end { 0.0 } else { y }, w);
            }
        }
        for (i, (ch_i, _)) in text.char_indices().enumerate() {
            let b = boxes[ch_i];
            lines.push(format!("{} {} {} {} {}", i, b.0, b.1, b.2, if b.2 > 0.0 { f.height() } else { 0 }));
        }
        std::fs::write(out.join(format!("{}.txt", t[1])), lines.join("\r\n")).unwrap();
    }
}
