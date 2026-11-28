//! hqprobe (<src.png> <w> <h> <out.png>)...: HighQualityBilinear into a transparent w x h bitmap (GDI+ comparison).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    for q in a[1..].chunks(4) {
        let src = nm_draw::Bitmap::load_png(std::path::Path::new(&q[0])).expect("png");
        let (w, h): (i32, i32) = (q[1].parse().unwrap(), q[2].parse().unwrap());
        let mut g = nm_draw::Graphics::new(nm_draw::Bitmap::new(w, h));
        g.draw_image_hq_bilinear(&src, nm_draw::RectangleF::new(0.0, 0.0, w as f32, h as f32));
        g.bmp.save_png(std::path::Path::new(&q[3])).expect("save");
    }
}
