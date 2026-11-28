//! Windows (MSVC): the exe gets the C# launcher's icon (launcher/nativemons.ico, also SDL's window icon) and the
//! same asInvoker manifest and version block csc embeds, as a .res written here (no rc.exe) that link.exe takes as an input.

use std::path::{Path, PathBuf};

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="MyApplication.app"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v2">
    <security>
      <requestedPrivileges xmlns="urn:schemas-microsoft-com:asm.v3">
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#;

fn u16le(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
fn u32le(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }

/// One RES entry: header (ordinal type and name), then the data padded to 4 bytes.
fn entry(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    u32le(out, data.len() as u32);
    u32le(out, 32);
    u16le(out, 0xFFFF);
    u16le(out, kind);
    u16le(out, 0xFFFF);
    u16le(out, name);
    u32le(out, 0);
    u16le(out, flags);
    u16le(out, if kind == 0 || kind == 16 { 0 } else { 0x0409 });
    u32le(out, 0);
    u32le(out, 0);
    out.extend_from_slice(data);
    while out.len() % 4 != 0 { out.push(0); }
}

fn pad4(v: &mut Vec<u8>) { while v.len() % 4 != 0 { v.push(0); } }

/// One VS_VERSIONINFO node: length, value length, type, key, value, then the children, each 4-byte aligned.
fn vnode(key: &str, kind: u16, value: &[u8], value_len: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![0, 0];
    u16le(&mut v, value_len);
    u16le(&mut v, kind);
    for c in key.encode_utf16().chain([0]) { u16le(&mut v, c); }
    pad4(&mut v);
    v.extend_from_slice(value);
    for c in children { pad4(&mut v); v.extend_from_slice(c); }
    // csc counts the closing padding in each node's length.
    pad4(&mut v);
    let n = v.len() as u16;
    v[0..2].copy_from_slice(&n.to_le_bytes());
    v
}

fn text(key: &str, s: &str) -> Vec<u8> {
    let mut b = Vec::new();
    for c in s.encode_utf16().chain([0]) { u16le(&mut b, c); }
    vnode(key, 1, &b, (b.len() / 2) as u16, &[])
}

/// The version block csc gives the C# exe (no assembly attributes), byte for byte.
fn version_info() -> Vec<u8> {
    let mut fixed = Vec::new();
    for x in [0xFEEF04BDu32, 0x10000, 0, 0, 0, 0, 0x3F, 0, 4, 1, 0, 0, 0] { u32le(&mut fixed, x); }
    let strings = vnode("000004b0", 1, &[], 0, &[
        text("FileDescription", " "),
        text("FileVersion", "0.0.0.0"),
        text("InternalName", "NativeMonsLauncher.exe"),
        text("LegalCopyright", " "),
        text("OriginalFilename", "NativeMonsLauncher.exe"),
        text("ProductVersion", "0.0.0.0"),
        text("Assembly Version", "0.0.0.0"),
    ]);
    let var = vnode("VarFileInfo", 1, &[], 0, &[vnode("Translation", 0, &[0, 0, 0xB0, 0x04], 4, &[])]);
    vnode("VS_VERSION_INFO", 0, &fixed, 52, &[var, vnode("StringFileInfo", 1, &[], 0, &[strings])])
}

fn res(ico: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    entry(&mut out, 0, 0, 0, &[]);
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let mut group = Vec::new();
    u16le(&mut group, 0);
    u16le(&mut group, 1);
    u16le(&mut group, count as u16);
    for i in 0..count {
        let e = &ico[6 + 16 * i..6 + 16 * (i + 1)];
        let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let at = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        // RT_ICON, moveable + discardable as rc.exe writes them.
        entry(&mut out, 3, (i + 1) as u16, 0x1010, &ico[at..at + size]);
        group.extend_from_slice(&e[0..12]);
        u16le(&mut group, (i + 1) as u16);
    }
    entry(&mut out, 14, 1, 0x1030, &group);
    entry(&mut out, 16, 1, 0x0030, &version_info());
    entry(&mut out, 24, 1, 0x1030, MANIFEST.as_bytes());
    out
}

fn main() {
    let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../launcher/nativemons.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    println!("cargo:rerun-if-changed=build.rs");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !windows || !msvc { return; }
    let Ok(data) = std::fs::read(&ico) else { println!("cargo:warning=no icon at {}", ico.display()); return };
    let file = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("nativemons.res");
    std::fs::write(&file, res(&data)).unwrap();
    println!("cargo:rustc-link-arg-bins={}", file.display());
}
