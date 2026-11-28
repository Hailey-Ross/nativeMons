//! CtrHeaderFix: 3DS dumps decrypted by a tool that left their NCCH headers saying "encrypted". The fix sets the game
//! part's crypto method to 0 and its NoCrypto flag, as GodMode9 does, with the original bytes kept to undo it.

use crate::ctr3ds_saves;
use crate::library::file_name;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub at: i64,
    pub was: u8,
    pub now: u8,
}

fn backup_of(file: &str) -> PathBuf { ctr3ds_saves::root().join("header-backups").join(format!("{}.txt", file_name(file))) }
pub fn can_undo(file: Option<&str>) -> bool { file.map_or(false, |f| backup_of(f).is_file()) }
pub fn needs(file: &str) -> bool { matches!(needed(file), Ok(Some(n)) if !n.is_empty()) }
fn u32_at(b: &[u8], at: usize) -> u32 { u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) }
fn read_full(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let r = f.read(&mut buf[n..])?;
        if r == 0 { break; }
        n += r;
    }
    Ok(n)
}

/// The bytes to change; None when it isn't a cartridge dump or its game part is still encrypted.
pub fn needed(file: &str) -> std::io::Result<Option<Vec<Spot>>> {
    let mut spots = Vec::new();
    let mut f = File::open(file).map_err(|e| std::io::Error::new(e.kind(), crate::zips::missing(&e, file)))?;
    let mut h = vec![0u8; 0x200];
    if read_full(&mut f, &mut h)? < h.len() || &h[0x100..0x104] != b"NCSD" { return Ok(None); }
    let off = u32_at(&h, 0x120) as i64 * 0x200;
    if u32_at(&h, 0x124) == 0 { return Ok(None); }
    let mut n = vec![0u8; 0x200];
    f.seek(SeekFrom::Start(off as u64))?;
    if read_full(&mut f, &mut n)? < n.len() || &n[0x100..0x104] != b"NCCH" { return Ok(None); }
    let (method, flags) = (n[0x18B], n[0x18F]);
    if flags & 0x04 != 0 { return Ok(Some(spots)); }
    if !readable(&mut f, off, &n)? { return Ok(None); }
    if method != 0 { spots.push(Spot { at: off + 0x18B, was: method, now: 0 }); }
    // NoCrypto on; fixed key and seed off, as they only describe encryption.
    spots.push(Spot { at: off + 0x18F, was: flags, now: (flags | 0x04) & !0x21 });
    Ok(Some(spots))
}
/// Decrypted: the ExeFS's first file name is plain text and the RomFS starts with "IVFC".
fn readable(f: &mut File, off: i64, n: &[u8]) -> std::io::Result<bool> {
    let exefs = u32_at(n, 0x1A0) as i64 * 0x200;
    let romfs = u32_at(n, 0x1B0) as i64 * 0x200;
    if u32_at(n, 0x1A4) > 0 {
        let mut e = [0u8; 8];
        f.seek(SeekFrom::Start((off + exefs) as u64))?;
        if read_full(f, &mut e)? < 8 || e[0] < 0x20 || e[0] > 0x7E || e.iter().any(|&b| b != 0 && !(0x20..=0x7E).contains(&b)) { return Ok(false); }
    }
    if u32_at(n, 0x1B4) > 0 {
        let mut r = [0u8; 4];
        f.seek(SeekFrom::Start((off + romfs) as u64))?;
        if read_full(f, &mut r)? < 4 || &r != b"IVFC" { return Ok(false); }
    }
    Ok(true)
}
/// None when done, or why not. The backup is written before any byte changes.
pub fn fix(file: &str) -> Option<String> {
    let r = (|| -> std::io::Result<Option<String>> {
        let Some(spots) = needed(file)? else { return Ok(Some("This dump's game is still encrypted, so its header was left as it is.".into())) };
        if spots.is_empty() { return Ok(None); }
        std::fs::create_dir_all(backup_of(file).parent().unwrap())?;
        // A backup left from an earlier file of that name no longer matches: this file still needs the fix.
        let mut ls = vec![file.to_string()];
        ls.extend(spots.iter().map(|x| format!("{} {} {}", x.at, x.was, x.now)));
        crate::config::write_all_lines(&backup_of(file), &ls)?;
        write(file, &spots, false)
    })();
    r.unwrap_or_else(|e| Some(e.to_string()))
}
pub fn undo(file: &str) -> Option<String> {
    let r = (|| -> Result<Option<String>, String> {
        let text = std::fs::read_to_string(backup_of(file)).map_err(|e| crate::zips::missing(&e, &backup_of(file).to_string_lossy()))?;
        let mut spots = Vec::new();
        for l in crate::config::lines(&text).into_iter().skip(1) {
            let x: Vec<&str> = l.split(' ').collect();
            let p = |i: usize| x.get(i).copied().unwrap_or("").to_string();
            spots.push(Spot { at: p(0).parse().map_err(|_| "Input string was not in a correct format.".to_string())?, was: p(1).parse().map_err(|_| "Input string was not in a correct format.".to_string())?, now: p(2).parse().map_err(|_| "Input string was not in a correct format.".to_string())? });
        }
        let why = write(file, &spots, true).map_err(|e| e.to_string())?;
        if why.is_none() { std::fs::remove_file(backup_of(file)).map_err(|e| e.to_string())?; }
        Ok(why)
    })();
    r.unwrap_or_else(Some)
}
/// Each byte must hold what it held before; anything else stops and puts back what was written.
fn write(file: &str, spots: &[Spot], back: bool) -> std::io::Result<Option<String>> {
    let mut done: Vec<Spot> = Vec::new();
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(file).map_err(|e| std::io::Error::new(e.kind(), crate::zips::missing(&e, file)))?;
    let one = |f: &mut File, at: i64| -> std::io::Result<i32> {
        f.seek(SeekFrom::Start(at as u64))?;
        let mut b = [0u8; 1];
        Ok(if f.read(&mut b)? == 1 { b[0] as i32 } else { -1 })
    };
    for x in spots {
        let (from, to) = if back { (x.now, x.was) } else { (x.was, x.now) };
        let now = one(&mut f, x.at)?;
        if now != from as i32 {
            for d in &done {
                f.seek(SeekFrom::Start(d.at as u64))?;
                f.write_all(&[if back { d.now } else { d.was }])?;
            }
            f.sync_all()?;
            return Ok(Some("The header wasn't what was expected, so nothing was changed.".into()));
        }
        f.seek(SeekFrom::Start(x.at as u64))?;
        f.write_all(&[to])?;
        done.push(*x);
    }
    f.sync_all()?;
    for x in spots {
        if one(&mut f, x.at)? != (if back { x.was } else { x.now }) as i32 { return Ok(Some("The header couldn't be checked after writing.".into())); }
    }
    Ok(None)
}
