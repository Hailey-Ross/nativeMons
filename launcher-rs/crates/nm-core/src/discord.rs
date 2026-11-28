//! DiscordPresence, line by line from the C#: Rich Presence over Discord's local pipe (Windows) or socket (Linux),
//! no library. NATIVEMONS_TEST_DISCORD=<file>: the frames go to that file as "op <n> <json>" lines, never to Discord.

use crate::config;
use crate::game_entry::GameEntry;
use std::io::{Read, Write};
use std::sync::Mutex;

// Registered at discord.com/developers. A file in state overrides it, for testing.
const APP_ID: &str = "1566980838325747914";
const IMAGE_KEY: &str = "nativemons";
const HOME: &str = "https://github.com/Hailey-Ross/nativeMons";

struct State { pipe: Option<std::fs::File>, generation: u64 }
static GATE: Mutex<State> = Mutex::new(State { pipe: None, generation: 0 });

pub fn enabled() -> bool { config::read("discord.txt", "on").trim() != "off" }
pub fn set_enabled(on: bool) { config::write("discord.txt", if on { "on" } else { "off" }); if !on { stop(); } }
fn id() -> String { let own = config::read("discord-app.txt", "").trim().to_string(); if own.is_empty() { APP_ID.into() } else { own } }
pub fn available() -> bool { !id().is_empty() }

/// JavaScriptSerializer's string form: quotes, backslash, < > ' & and control characters escaped, the rest as is.
pub fn js_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\u{8}' => o.push_str("\\b"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\u{c}' => o.push_str("\\f"),
            '\r' => o.push_str("\\r"),
            '<' | '>' | '\'' | '&' => o.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
/// The two frames' bodies: the handshake, and SET_ACTIVITY for the game (details, button to GitHub, start time).
pub fn handshake(id: &str) -> String { format!("{{\"v\":1,\"client_id\":{}}}", js_string(id)) }
pub fn activity(title: &str, started: i64, pid: u32, nonce: &str) -> String {
    let details = match title.strip_prefix("Pokemon ") { Some(r) => format!("Pokémon {}", r), None => title.to_string() };
    format!("{{\"cmd\":\"SET_ACTIVITY\",\"nonce\":{},\"args\":{{\"pid\":{},\"activity\":{{\"details\":{},\"details_url\":{},\"timestamps\":{{\"start\":{}}},\"assets\":{{\"large_image\":{},\"large_text\":\"nativeMons launcher\",\"large_url\":{}}},\"buttons\":[{{\"label\":\"nativeMons on GitHub\",\"url\":{}}}]}}}}}}",
        js_string(nonce), pid, js_string(&details), js_string(HOME), started, js_string(IMAGE_KEY), js_string(HOME), js_string(HOME))
}
fn guid() -> String {
    let h = crate::proc::new_guid();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn playing(g: Option<&GameEntry>) {
    let Some(g) = g else { return };
    if !enabled() || !available() { return; }
    let (title, id) = (g.title.clone(), id());
    let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mine = { let mut s = GATE.lock().unwrap(); s.generation += 1; s.generation };
    if let Ok(file) = std::env::var("NATIVEMONS_TEST_DISCORD") {
        let body = format!("op 0 {}\r\nop 1 {}\r\n", handshake(&id), activity(&title, started, std::process::id(), &guid()));
        if let Ok(mut o) = std::fs::OpenOptions::new().create(true).append(true).open(file) { let _ = o.write_all(body.as_bytes()); }
        return;
    }
    std::thread::spawn(move || {
        let Some(mut p) = connect(&id) else { return };
        if send(&mut p, 1, &activity(&title, started, std::process::id(), &guid())).is_err() { return; }
        let reply = receive(&mut p);
        if let Ok(log) = std::env::var("NATIVEMONS_TEST_LOG") {
            if let Ok(mut o) = std::fs::OpenOptions::new().create(true).append(true).open(log) { let _ = write!(o, "discord: {}\r\n", reply.unwrap_or_default()); }
        }
        let mut s = GATE.lock().unwrap();
        // The game may have closed while this was connecting.
        if mine != s.generation { return; }
        s.pipe = Some(p);
    });
}
pub fn stop() { let mut s = GATE.lock().unwrap(); s.generation += 1; s.pipe = None; }

#[cfg(windows)]
fn open(i: u32) -> Option<std::fs::File> { std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{}", i)).ok() }
#[cfg(unix)]
fn open(i: u32) -> Option<std::fs::File> {
    use std::os::fd::{FromRawFd, IntoRawFd};
    let dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().find_map(|v| std::env::var(v).ok()).unwrap_or_else(|| "/tmp".into());
    let s = std::os::unix::net::UnixStream::connect(format!("{}/discord-ipc-{}", dir, i)).ok()?;
    Some(unsafe { std::fs::File::from_raw_fd(s.into_raw_fd()) })
}
fn connect(id: &str) -> Option<std::fs::File> {
    for i in 0..10 {
        let Some(mut p) = open(i) else { continue };
        // Discord answers the handshake with READY, or closes the pipe for an unknown application.
        if send(&mut p, 0, &handshake(id)).is_ok() && receive(&mut p).is_some() { return Some(p); }
    }
    None
}
fn send(s: &mut std::fs::File, op: i32, body: &str) -> std::io::Result<()> {
    let data = body.as_bytes();
    let mut frame = Vec::with_capacity(8 + data.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(data.len() as i32).to_le_bytes());
    frame.extend_from_slice(data);
    s.write_all(&frame)?;
    s.flush()
}
fn receive(s: &mut std::fs::File) -> Option<String> {
    let mut head = [0u8; 8];
    s.read_exact(&mut head).ok()?;
    let op = i32::from_le_bytes(head[0..4].try_into().ok()?);
    let len = i32::from_le_bytes(head[4..8].try_into().ok()?);
    if !(0..=1 << 20).contains(&len) { return None; }
    let mut body = vec![0u8; len as usize];
    s.read_exact(&mut body).ok()?;
    // Op 2 is Discord closing the connection, with its reason.
    if op == 2 { None } else { Some(String::from_utf8_lossy(&body).into_owned()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payloads() {
        assert_eq!(js_string("a<b>'&\"\\\u{1}é"), "\"a\\u003cb\\u003e\\u0027\\u0026\\\"\\\\\\u0001é\"");
        let a = activity("Pokemon Red", 5, 9, "n");
        assert!(a.starts_with("{\"cmd\":\"SET_ACTIVITY\",\"nonce\":\"n\",\"args\":{\"pid\":9,\"activity\":{\"details\":\"Pokémon Red\""));
    }
}
