use crate::color::Color;

/// A 32bpp ARGB bitmap (straight alpha, like GDI+'s Format32bppArgb), one u32 per pixel as 0xAARRGGBB.
#[derive(Clone, PartialEq)]
pub struct Bitmap {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u32>,
}

impl Bitmap {
    pub fn new(width: i32, height: i32) -> Bitmap {
        Bitmap { width, height, data: vec![0; (width.max(0) * height.max(0)) as usize] }
    }
    pub fn get(&self, x: i32, y: i32) -> Color {
        let v = self.data[(y * self.width + x) as usize];
        Color::from_argb((v >> 24) as i32, (v >> 16 & 255) as i32, (v >> 8 & 255) as i32, (v & 255) as i32)
    }
    pub fn set(&mut self, x: i32, y: i32, c: Color) {
        self.data[(y * self.width + x) as usize] = (c.a as u32) << 24 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
    }

    /// RGBA8 rows, straight alpha, as a PNG wants them.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() * 4);
        for v in &self.data {
            out.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, *v as u8, (v >> 24) as u8]);
        }
        out
    }
    /// A tiny-skia Pixmap of this bitmap (premultiplied, as tiny-skia stores pixels).
    pub fn to_pixmap(&self) -> tiny_skia::Pixmap {
        let mut pm = tiny_skia::Pixmap::new(self.width.max(1) as u32, self.height.max(1) as u32).unwrap();
        for (d, v) in pm.pixels_mut().iter_mut().zip(self.data.iter()) {
            let c = tiny_skia::ColorU8::from_rgba((v >> 16) as u8, (v >> 8) as u8, *v as u8, (v >> 24) as u8);
            *d = c.premultiply();
        }
        pm
    }
    pub fn from_pixmap(pm: &tiny_skia::Pixmap) -> Bitmap {
        let mut b = Bitmap::new(pm.width() as i32, pm.height() as i32);
        for (d, p) in b.data.iter_mut().zip(pm.pixels().iter()) {
            let c = p.demultiply();
            *d = (c.alpha() as u32) << 24 | (c.red() as u32) << 16 | (c.green() as u32) << 8 | c.blue() as u32;
        }
        b
    }
    /// PNG through tiny-skia.
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> { self.to_pixmap().save_png(path).map_err(std::io::Error::other) }
    pub fn load_png(path: &std::path::Path) -> std::io::Result<Bitmap> { tiny_skia::Pixmap::load_png(path).map(|p| Bitmap::from_pixmap(&p)).map_err(std::io::Error::other) }
    /// A JPEG as GDI+'s decoder gives it (libjpeg's islow IDCT and fancy upsampling, checked byte for byte).
    pub fn load_jpeg(path: &std::path::Path) -> std::io::Result<Bitmap> {
        let data = std::fs::read(path)?;
        let rgb = std::panic::catch_unwind(|| -> std::io::Result<(usize, usize, Vec<[u8; 3]>)> {
            let d = mozjpeg::Decompress::new_mem(&data).map_err(std::io::Error::other)?;
            let mut s = d.rgb().map_err(std::io::Error::other)?;
            let (w, h) = (s.width(), s.height());
            let px: Vec<[u8; 3]> = s.read_scanlines().map_err(std::io::Error::other)?;
            s.finish().map_err(std::io::Error::other)?;
            Ok((w, h, px))
        }).map_err(|_| std::io::Error::other("not a JPEG it can read"))??;
        let (w, h, px) = rgb;
        let mut b = Bitmap::new(w as i32, h as i32);
        for (d, p) in b.data.iter_mut().zip(px.iter()) { *d = 0xFF00_0000 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32; }
        Ok(b)
    }
    /// An uncompressed 24 or 32 bit BMP (as the engines write their shots), bottom-up or top-down.
    pub fn load_bmp(path: &std::path::Path) -> std::io::Result<Bitmap> {
        let d = std::fs::read(path)?;
        let bad = || std::io::Error::other("not a BMP it can read");
        if d.len() < 54 || &d[0..2] != b"BM" { return Err(bad()); }
        let u32_at = |i: usize| u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
        let (off, w, h) = (u32_at(10) as usize, u32_at(18) as i32, u32_at(22) as i32);
        let (bpp, comp) = (u16::from_le_bytes([d[28], d[29]]), u32_at(30));
        if w <= 0 || h == 0 || !(bpp == 24 || bpp == 32) || !(comp == 0 || (comp == 3 && bpp == 32)) { return Err(bad()); }
        let (rows, down) = (h.abs(), h < 0);
        let stride = ((w as usize * bpp as usize / 8) + 3) & !3;
        if d.len() < off + stride * rows as usize { return Err(bad()); }
        let mut b = Bitmap::new(w, rows);
        for y in 0..rows {
            let src = off + stride * (if down { y } else { rows - 1 - y }) as usize;
            for x in 0..w as usize {
                let p = src + x * (bpp as usize / 8);
                b.data[(y * w) as usize + x] = 0xFF00_0000 | (d[p + 2] as u32) << 16 | (d[p + 1] as u32) << 8 | d[p] as u32;
            }
        }
        Ok(b)
    }
    /// new Bitmap(file): PNG, JPEG or BMP by its content.
    pub fn load_any(path: &std::path::Path) -> std::io::Result<Bitmap> {
        let mut head = [0u8; 4];
        std::io::Read::read_exact(&mut std::fs::File::open(path)?, &mut head)?;
        if head.starts_with(&[0x89, b'P', b'N', b'G']) { Bitmap::load_png(path) } else if head.starts_with(&[0xFF, 0xD8]) { Bitmap::load_jpeg(path) } else { Bitmap::load_bmp(path) }
    }
    /// Bitmap.Clone(rect): the pixels inside it.
    pub fn crop(&self, x: i32, y: i32, w: i32, h: i32) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for r in 0..h { for c in 0..w { b.data[(r * w + c) as usize] = self.data[((y + r) * self.width + x + c) as usize]; } }
        b
    }
    /// Image.Save(file, jpeg, Quality q): libjpeg v6's defaults (Annex K tables, 4:2:0, islow DCT, baseline), as GDI+'s
    /// encoder writes them. Alpha is dropped (GDI+ writes the colors as they are).
    pub fn encode_jpeg(&self, quality: u8) -> std::io::Result<Vec<u8>> { self.encode_jpeg_exif(quality, None) }
    /// The same for a picture saved as it was loaded: GDI+ writes the file's property items as an Exif APP1 (exif_of).
    pub fn encode_jpeg_exif(&self, quality: u8, exif: Option<Vec<u8>>) -> std::io::Result<Vec<u8>> {
        let rgb: Vec<u8> = self.data.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
        let (w, h) = (self.width as usize, self.height as usize);
        std::panic::catch_unwind(move || -> std::io::Result<Vec<u8>> {
            let mut c = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
            c.set_fastest_defaults();
            c.set_size(w, h);
            // GDI+'s quality q is libjpeg's q - 1 (its tables, probed at 85, 88 and 90); 96 dpi in the JFIF header.
            c.set_quality(quality.saturating_sub(1).max(1) as f32);
            c.set_pixel_density(mozjpeg::PixelDensity { unit: mozjpeg::PixelDensityUnit::Inches, x: 96, y: 96 });
            c.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
            let mut s = c.start_compress(Vec::new())?;
            if let Some(x) = &exif { s.write_marker(mozjpeg::Marker::APP(1), x); }
            s.write_scanlines(&rgb)?;
            s.finish()
        }).map_err(|_| std::io::Error::other("the picture couldn't be encoded"))?
    }
    /// The probe's raw format: width, height (i32 LE), then BGRA bytes.
    pub fn save_raw(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut out = Vec::with_capacity(8 + self.data.len() * 4);
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        for v in &self.data { out.extend_from_slice(&v.to_le_bytes()); }
        std::fs::write(path, out)
    }
}

/// The Exif APP1 GDI+ writes when it saves a PNG it loaded (probed, scratch png probes 2026-11-25): its decoder's
/// property items in tag order, big-endian. gAMA (Gamma, 100000 / gamma), cHRM (WhitePoint, PrimaryChromaticities),
/// sRGB (rendering intent), pHYs (unit, pixels per unit; unit 1 and 0 x 0 when there's none: so always written), tIME
/// (DateTime), tEXt Title, Author, Description, Copyright, Software, Source (Model) and Comment (UserComment, UNICODE, in
/// an Exif IFD). zTXt, iTXt, other keys, bKGD and sBIT are dropped; iCCP isn't handled (its description isn't known).
pub fn exif_of(path: &std::path::Path) -> Option<Vec<u8>> {
    let d = std::fs::read(path).ok()?;
    if !d.starts_with(&[0x89, b'P', b'N', b'G']) { return None; }
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let (mut gamma, mut intent, mut phys, mut chrm, mut time) = (None, None, None, None, None);
    let mut texts: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut comment: Option<Vec<u8>> = None;
    let mut i = 8;
    while i + 8 <= d.len() {
        let len = be(&d[i..]) as usize;
        let (kind, body) = (&d[i + 4..i + 8], d.get(i + 8..i + 8 + len)?);
        match kind {
            b"gAMA" if len == 4 => gamma = Some(be(body)),
            b"sRGB" if len == 1 => intent = Some(body[0]),
            b"pHYs" if len == 9 => phys = Some((be(body), be(&body[4..]), body[8])),
            b"cHRM" if len == 32 => chrm = Some((0..8).map(|k| be(&body[4 * k..])).collect::<Vec<u32>>()),
            b"tIME" if len == 7 => time = Some(format!("{:04}:{:02}:{:02} {:02}:{:02}:{:02}", u16::from_be_bytes([body[0], body[1]]), body[2], body[3], body[4], body[5], body[6])),
            b"tEXt" => {
                if let Some(z) = body.iter().position(|b| *b == 0) {
                    let (key, text) = (&body[..z], &body[z + 1..]);
                    let tag = match key { b"Title" => 0x0320, b"Author" => 0x013B, b"Description" => 0x010E, b"Copyright" => 0x8298, b"Software" => 0x0131, b"Source" => 0x0110, _ => 0 };
                    if key == b"Comment" { if comment.is_none() { comment = Some(text.to_vec()); } }
                    else if tag != 0 && !texts.iter().any(|t| t.0 == tag) { texts.push((tag, text.to_vec())); }
                }
            }
            b"IEND" => break,
            _ => {}
        }
        i += 12 + len;
    }
    // (tag, type, count, inline value or data); data goes after the directory, in tag order, padded to even.
    enum V { Inline([u8; 4]), Data(Vec<u8>), SubIfd }
    let mut entries: Vec<(u16, u16, u32, V)> = Vec::new();
    let ascii = |t: &[u8]| { let mut v = t.to_vec(); v.push(0); v };
    let rat = |pairs: &[(u32, u32)]| { let mut v = Vec::new(); for (n, dd) in pairs { v.extend_from_slice(&n.to_be_bytes()); v.extend_from_slice(&dd.to_be_bytes()); } v };
    if let Some(c) = &chrm {
        entries.push((0x013E, 5, 2, V::Data(rat(&[(c[0], 100000), (c[1], 100000)]))));
        entries.push((0x013F, 5, 6, V::Data(rat(&(2..8).map(|k| (c[k], 100000)).collect::<Vec<_>>()))));
    }
    if let Some(g) = gamma { entries.push((0x0301, 5, 1, V::Data(rat(&[(100000, g)])))); }
    if let Some(r) = intent { entries.push((0x0303, 1, 1, V::Inline([r, 0, 0, 0]))); }
    if let Some(t) = &time { entries.push((0x0132, 2, 20, V::Data(ascii(t.as_bytes())))); }
    for (tag, t) in &texts { let v = ascii(t); entries.push((*tag, 2, v.len() as u32, if v.len() <= 4 { let mut b = [0u8; 4]; b[..v.len()].copy_from_slice(&v); V::Inline(b) } else { V::Data(v) })); }
    let (x, y, unit) = phys.unwrap_or((0, 0, 1));
    entries.push((0x5110, 1, 1, V::Inline([unit, 0, 0, 0])));
    entries.push((0x5111, 4, 1, V::Inline(x.to_be_bytes())));
    entries.push((0x5112, 4, 1, V::Inline(y.to_be_bytes())));
    if comment.is_some() { entries.push((0x8769, 4, 1, V::SubIfd)); }
    entries.sort_by_key(|e| e.0);
    let mut out = vec![b'E', b'x', b'i', b'f', 0, 0, b'M', b'M', 0, 0x2a, 0, 0, 0, 8];
    let mut extra: Vec<u8> = Vec::new();
    let data_at = 8 + 2 + 12 * entries.len() as u32 + 4;
    let data_len: u32 = entries.iter().map(|e| if let V::Data(v) = &e.3 { (v.len() as u32 + 1) & !1 } else { 0 }).sum();
    out.extend_from_slice(&(entries.len() as u16).to_be_bytes());
    for (tag, kind, count, v) in &entries {
        out.extend_from_slice(&tag.to_be_bytes()); out.extend_from_slice(&kind.to_be_bytes()); out.extend_from_slice(&count.to_be_bytes());
        match v {
            V::Inline(b) => out.extend_from_slice(b),
            V::Data(b) => { out.extend_from_slice(&(data_at + extra.len() as u32).to_be_bytes()); extra.extend_from_slice(b); if b.len() % 2 == 1 { extra.push(0); } }
            V::SubIfd => out.extend_from_slice(&(data_at + data_len).to_be_bytes()),
        }
    }
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&extra);
    if let Some(c) = comment {
        // The Exif IFD: UserComment as "UNICODE\0" and the text in UTF-16BE (no terminator).
        let mut uc = b"UNICODE\0".to_vec();
        for b in &c { uc.extend_from_slice(&(*b as u16).to_be_bytes()); }
        let at = data_at + data_len;
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&0x9286u16.to_be_bytes()); out.extend_from_slice(&7u16.to_be_bytes()); out.extend_from_slice(&(uc.len() as u32).to_be_bytes());
        out.extend_from_slice(&(at + 18).to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&uc);
        // Two zero bytes more when its length is a multiple of 4 (probed: 1 to 5 characters).
        if uc.len() % 4 == 0 { out.extend_from_slice(&[0, 0]); }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    // The Exif GDI+ wrote re-saving PNGs with these chunks as JPEG (probed 2026-11-25), byte for byte.
    fn png_with(chunks: &[(&[u8], Vec<u8>)]) -> std::path::PathBuf {
        let mut d = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10];
        let mut put = |t: &[u8], b: &[u8]| { d.extend_from_slice(&(b.len() as u32).to_be_bytes()); d.extend_from_slice(t); d.extend_from_slice(b); d.extend_from_slice(&[0; 4]); };
        put(b"IHDR", &[0, 0, 0, 16, 0, 0, 0, 16, 8, 2, 0, 0, 0]);
        for (t, b) in chunks { put(t, b); }
        put(b"IDAT", &[]);
        put(b"IEND", &[]);
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("nm-exif-{}-{}.png", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::write(&p, d).unwrap();
        p
    }
    fn hex(s: &str) -> Vec<u8> { (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect() }
    fn text(k: &str, v: &str) -> Vec<u8> { let mut b = k.as_bytes().to_vec(); b.push(0); b.extend_from_slice(v.as_bytes()); b }
    fn check(chunks: &[(&[u8], Vec<u8>)], want: &str) {
        let p = png_with(chunks);
        let got = super::exif_of(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, hex(want));
    }
    #[test]
    fn exif_matches_gdiplus() {
        check(&[], "4578696600004d4d002a00000008000351100001000000010100000051110004000000010000000051120004000000010000000000000000");
        check(&[(b"tEXt", text("Copyright", "(c) 2024"))], "4578696600004d4d002a00000008000451100001000000010100000051110004000000010000000051120004000000010000000082980002000000090000003e0000000028632920323032340000");
        check(&[(b"tEXt", text("Comment", "note"))], "4578696600004d4d002a00000008000451100001000000010100000051110004000000010000000051120004000000010000000087690004000000010000003e00000000000192860007000000100000005000000000554e49434f444500006e006f007400650000");
        check(&[(b"tEXt", text("Comment", "ab"))], "4578696600004d4d002a00000008000451100001000000010100000051110004000000010000000051120004000000010000000087690004000000010000003e000000000001928600070000000c0000005000000000554e49434f444500006100620000");
        check(&[(b"tEXt", text("Comment", "abc"))], "4578696600004d4d002a00000008000451100001000000010100000051110004000000010000000051120004000000010000000087690004000000010000003e000000000001928600070000000e0000005000000000554e49434f444500006100620063");
        let be = |v: &[u32]| v.iter().flat_map(|x| x.to_be_bytes()).collect::<Vec<u8>>();
        let mut phys = be(&[3780, 3780]); phys.push(1);
        check(&[(b"gAMA", be(&[45455])), (b"cHRM", be(&[31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000])), (b"sRGB", vec![0]), (b"pHYs", phys),
                (b"sBIT", vec![8, 8, 8]), (b"bKGD", vec![0, 255, 0, 255, 0, 255]), (b"tIME", vec![0x07, 0xE8, 5, 17, 13, 45, 9]),
                (b"tEXt", text("Title", "T")), (b"tEXt", text("Author", "A")), (b"tEXt", text("Software", "S")), (b"tEXt", text("Description", "D")),
                (b"tEXt", text("Copyright", "C")), (b"tEXt", text("Comment", "K"))],
            "4578696600004d4d002a00000008000e010e000200000002440000000131000200000002530000000132000200000014000000b6013b00020000000241000000013e000500000002000000ca013f000500000006000000da03010005000000010000010a030300010000000100000000032000020000000254000000511000010000000101000000511100040000000100000ec4511200040000000100000ec482980002000000024300000087690004000000010000011200000000323032343a30353a31372031333a34353a30390000007a26000186a000008084000186a00000fa00000186a0000080e8000186a000007530000186a00000ea60000186a000003a98000186a000001770000186a0000186a00000b18f0001928600070000000a0000012400000000554e49434f444500004b");
    }
}
