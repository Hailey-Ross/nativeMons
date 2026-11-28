//! PlatinumSave: the Platinum port's save.bin, a DS save with 64-bit padding (notes/launcher.md, Saves).

const MAIN_DS: usize = 0xCF2C;
const BOXES_DS: usize = 0x121E4;
const FOOTER_SIZE: usize = 20;
const HALF_SIZE: usize = 0x40000;
const SIZE: usize = 0x80000;
const SIGNATURE: u32 = 0x20060623;
/// Where, in DS offsets, 4 bytes are added.
const MAIN_PADS: [usize; 5] = [0x34, 0x48, 0x64, 0x1740, 0x182C];
const BOXES_PADS: [usize; 2] = [0x4, BOXES_DS - FOOTER_SIZE];
const MAIN_PORT: usize = MAIN_DS + MAIN_PADS.len() * 4;
const BOXES_PORT: usize = BOXES_DS + BOXES_PADS.len() * 4;

fn u32_at(d: &[u8], at: usize) -> u32 { u32::from_le_bytes(d[at..at + 4].try_into().unwrap()) }
fn u16_at(d: &[u8], at: usize) -> u16 { u16::from_le_bytes(d[at..at + 2].try_into().unwrap()) }

fn crc(d: &[u8], at: usize, n: usize) -> u16 {
    let mut c: u16 = 0xFFFF;
    for i in 0..n {
        c ^= (d[at + i] as u16) << 8;
        for _ in 0..8 { c = if c & 0x8000 != 0 { (c << 1) ^ 0x1021 } else { c << 1 }; }
    }
    c
}

fn valid(d: &[u8], at: usize, size: usize, id: u8) -> bool {
    let f = at + size - FOOTER_SIZE;
    f + FOOTER_SIZE <= d.len() && u32_at(d, f + 8) == size as u32 && u32_at(d, f + 12) == SIGNATURE && d[f + 16] == id && u16_at(d, f + 18) == crc(d, at, size - FOOTER_SIZE)
}

/// One block's body, with padding added or removed. Removed padding has to be zero, or it wasn't padding.
fn body(src: &[u8], from: usize, dst: &mut [u8], to: usize, ds_body: usize, pads: &[usize], to_port: bool) -> Result<(), String> {
    let (mut ds, mut port) = (0usize, 0usize);
    for &pad in pads {
        let run = pad - ds;
        if to_port {
            dst[to + port..to + port + run].copy_from_slice(&src[from + ds..from + ds + run]);
            dst[to + port + run..to + port + run + 4].fill(0);
        } else {
            dst[to + ds..to + ds + run].copy_from_slice(&src[from + port..from + port + run]);
            for k in 0..4 {
                if src[from + port + run + k] != 0 { return Err("The port's save has data where padding was expected, so it wasn't converted.".into()); }
            }
        }
        ds += run;
        port += run + 4;
    }
    let rest = ds_body - ds;
    if to_port { dst[to + port..to + port + rest].copy_from_slice(&src[from + ds..from + ds + rest]); } else { dst[to + ds..to + ds + rest].copy_from_slice(&src[from + port..from + port + rest]); }
    Ok(())
}

fn footer(src: &[u8], src_footer: usize, dst: &mut [u8], at: usize, size: usize) {
    let f = at + size - FOOTER_SIZE;
    dst[f..f + FOOTER_SIZE].copy_from_slice(&src[src_footer..src_footer + FOOTER_SIZE]);
    dst[f + 8..f + 12].copy_from_slice(&(size as u32).to_le_bytes());
    let c = crc(dst, at, size - FOOTER_SIZE);
    dst[f + 18..f + 20].copy_from_slice(&c.to_le_bytes());
}

fn convert(input: &[u8], to_port: bool) -> Result<Vec<u8>, String> {
    let mut src = vec![0xFFu8; SIZE];
    let n = input.len().min(SIZE);
    src[..n].copy_from_slice(&input[..n]);
    let mut dst = src.clone();
    let (main_from, boxes_from) = if to_port { (MAIN_DS, BOXES_DS) } else { (MAIN_PORT, BOXES_PORT) };
    let (main_to, boxes_to) = if to_port { (MAIN_PORT, BOXES_PORT) } else { (MAIN_DS, BOXES_DS) };
    let mut converted = 0;
    for half in [0, HALF_SIZE] {
        // Both blocks are always rewritten together, and blanked first so nothing old shows past a shorter block.
        let main = valid(&src, half, main_from, 0);
        let boxes = valid(&src, half + main_from, boxes_from, 1);
        let area = (MAIN_DS + BOXES_DS).max(MAIN_PORT + BOXES_PORT);
        dst[half..half + area].fill(0xFF);
        if main {
            body(&src, half, &mut dst, half, MAIN_DS - FOOTER_SIZE, &MAIN_PADS, to_port)?;
            footer(&src, half + main_from - FOOTER_SIZE, &mut dst, half, main_to);
            converted += 1;
        }
        if boxes {
            body(&src, half + main_from, &mut dst, half + main_to, BOXES_DS - FOOTER_SIZE, &BOXES_PADS, to_port)?;
            footer(&src, half + main_from + boxes_from - FOOTER_SIZE, &mut dst, half + main_to, boxes_to);
            converted += 1;
        }
    }
    if converted == 0 { return Err("No valid save data was found to convert.".into()); }
    // The port fills space it hasn't used with zeros. On a cartridge unused space reads as erased.
    if !to_port {
        for sector in (0..SIZE).step_by(0x1000) {
            if dst[sector..sector + 0x1000].iter().all(|b| *b == 0) { dst[sector..sector + 0x1000].fill(0xFF); }
        }
    }
    Ok(dst)
}

/// Ports up to v0.1.13 padded the save; v0.2.0 on use the DS format. An old save has a footer signature at 0xCF38.
pub fn is_old_port(d: &[u8]) -> bool { [0, HALF_SIZE].iter().any(|&half| d.len() >= half + MAIN_PORT && u32_at(d, half + MAIN_PORT - 8) == SIGNATURE) }
pub fn to_port(ds: &[u8]) -> Result<Vec<u8>, String> { convert(ds, true) }
pub fn to_ds(port: &[u8]) -> Result<Vec<u8>, String> { convert(port, false) }
