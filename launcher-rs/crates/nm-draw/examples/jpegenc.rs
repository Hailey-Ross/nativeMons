//! jpegenc <in.png> <out.jpg> <quality>: JPEG encoding as GDI+'s encoder writes it (comparison probe).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = nm_draw::Bitmap::load_png(std::path::Path::new(&a[1])).expect("png");
    let q: u8 = a[3].parse().unwrap();
    std::fs::write(&a[2], b.encode_jpeg(q).expect("encode")).expect("write");
}
