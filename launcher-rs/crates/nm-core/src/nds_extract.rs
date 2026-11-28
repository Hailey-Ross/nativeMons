//! NdsExtract: unpacks a DS ROM as the Platinum port does on first launch, so its file picker never shows.

use std::io::Write;
use std::path::Path;

fn u32_at(b: &[u8], at: u32) -> u32 { u32::from_le_bytes(b[at as usize..at as usize + 4].try_into().unwrap()) }
fn u16_at(b: &[u8], at: u32) -> u16 { u16::from_le_bytes(b[at as usize..at as usize + 2].try_into().unwrap()) }

fn put(file: &Path, rom: &[u8], offset: u32, size: u32) -> Result<(), String> {
    if file.exists() { return Ok(()); }
    if offset as u64 + size as u64 > rom.len() as u64 { return Err("That ROM looks cut short.".into()); }
    if let Some(d) = file.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(file).map_err(|e| e.to_string())?;
    f.write_all(&rom[offset as usize..(offset + size) as usize]).map_err(|e| e.to_string())
}

/// Path.GetInvalidFileNameChars on Windows: control characters and " < > | : * ? \ /.
fn invalid_name(name: &str) -> bool { name.chars().any(|c| (c as u32) < 32 || "\"<>|:*?\\/".contains(c)) }

pub fn run(rom_file: &Path, dir: &Path) -> Result<(), String> {
    let rom = std::fs::read(rom_file).map_err(|e| e.to_string())?;
    let (arm9, arm9_size, arm7, arm7_size) = (u32_at(&rom, 0x20), u32_at(&rom, 0x2C), u32_at(&rom, 0x30), u32_at(&rom, 0x3C));
    let (fnt, fnt_size, fat, fat_size) = (u32_at(&rom, 0x40), u32_at(&rom, 0x44), u32_at(&rom, 0x48), u32_at(&rom, 0x4C));
    let (ovt9, ovt9_size, ovt7, ovt7_size) = (u32_at(&rom, 0x50), u32_at(&rom, 0x54), u32_at(&rom, 0x58), u32_at(&rom, 0x5C));
    let (icon, rsa) = (u32_at(&rom, 0x68), u32_at(&rom, 0x80));
    put(&dir.join("header.bin"), &rom, 0, 0x4000)?;
    put(&dir.join("arm9.bin"), &rom, arm9, arm9_size)?;
    put(&dir.join("arm7.bin"), &rom, arm7, arm7_size)?;
    put(&dir.join("arm9ovt.bin"), &rom, ovt9, ovt9_size)?;
    put(&dir.join("arm7ovt.bin"), &rom, ovt7, ovt7_size)?;
    if icon != 0 {
        let kind = u16_at(&rom, icon);
        put(&dir.join("banner.bin"), &rom, icon, match kind { 2 => 0x940, 3 => 0xA40, 0x103 => 0x23C0, _ => 0x840 })?;
    }
    put(&dir.join("fnt.bin"), &rom, fnt, fnt_size)?;
    put(&dir.join("fat.bin"), &rom, fat, fat_size)?;
    put(&dir.join("rsasig.bin"), &rom, rsa, 136)?;
    for i in 0..ovt9_size / 32 {
        overlay(&rom, &dir.join("overlay9").join(format!("overlay9_{}.bin", u32_at(&rom, ovt9 + i * 32))), u16_at(&rom, ovt9 + i * 32 + 24), fat)?;
    }
    for i in 0..ovt7_size / 32 {
        overlay(&rom, &dir.join("overlay7").join(format!("overlay7_{}.bin", u32_at(&rom, ovt7 + i * 32))), u16_at(&rom, ovt7 + i * 32 + 24), fat)?;
    }
    tree(&rom, fnt, fat, 0xF000, dir)
}

fn overlay(rom: &[u8], file: &Path, id: u16, fat: u32) -> Result<(), String> {
    let start = u32_at(rom, fat + id as u32 * 8);
    put(file, rom, start, u32_at(rom, fat + id as u32 * 8 + 4).wrapping_sub(start))
}

fn tree(rom: &[u8], fnt: u32, fat: u32, dir_id: u32, path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let entry = fnt + (dir_id & 0xFFF) * 8;
    let mut at = fnt + u32_at(rom, entry);
    let mut file_id = u16_at(rom, entry + 4);
    loop {
        let head = rom[at as usize];
        at += 1;
        if head == 0 { break; }
        let length = (head & 0x7F) as u32;
        // Encoding.ASCII: bytes over 127 become '?'.
        let name: String = rom[at as usize..(at + length) as usize].iter().map(|&b| if b < 128 { b as char } else { '?' }).collect();
        if invalid_name(&name) || name == ".." { return Err("That ROM has a bad file name in it.".into()); }
        at += length;
        if head & 0x80 != 0 {
            let child = u16_at(rom, at) as u32;
            at += 2;
            tree(rom, fnt, fat, child, &path.join(&name))?;
        } else {
            let start = u32_at(rom, fat + file_id as u32 * 8);
            put(&path.join(&name), rom, start, u32_at(rom, fat + file_id as u32 * 8 + 4).wrapping_sub(start))?;
            file_id = file_id.wrapping_add(1);
        }
    }
    Ok(())
}
