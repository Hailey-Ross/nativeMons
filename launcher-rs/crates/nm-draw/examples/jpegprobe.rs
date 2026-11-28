//! jpegprobe <in.jpg> <out.png>...: decodes with nm-draw's JPEG loader (for the GDI+ comparison).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    for pair in a[1..].chunks(2) {
        let b = nm_draw::Bitmap::load_jpeg(std::path::Path::new(&pair[0])).expect("decode");
        b.save_png(std::path::Path::new(&pair[1])).expect("save");
    }
}
