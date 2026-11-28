//! WebClient as the launcher uses it: a GET with the "nativeMons" user agent, https or file:// (as WebClient takes).

use std::io::Read;
use std::path::{Path, PathBuf};

fn file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file:///").or_else(|| url.strip_prefix("file://"))?;
    let p = rest.replace("%20", " ");
    Some(if cfg!(windows) { PathBuf::from(p.replace('/', "\\")) } else { PathBuf::from(format!("/{}", p.trim_start_matches('/'))) })
}

fn get(url: &str, accept: Option<&str>) -> Result<Box<dyn Read + Send + Sync>, String> {
    if let Some(p) = file_url(url) { return std::fs::File::open(&p).map(|f| Box::new(f) as Box<dyn Read + Send + Sync>).map_err(|e| e.to_string()); }
    let mut req = ureq::get(url).set("User-Agent", "nativeMons");
    if let Some(a) = accept { req = req.set("Accept", a); }
    match req.call() {
        Ok(r) => Ok(r.into_reader()),
        Err(ureq::Error::Status(code, r)) => Err(format!("The remote server returned an error: ({}) {}.", code, r.status_text())),
        Err(e) => Err(e.to_string()),
    }
}

/// DownloadString with UTF-8.
pub fn download_string(url: &str, accept: Option<&str>) -> Result<String, String> { string(url, accept, true) }
/// The same, never noted by a replay's fetch stub (the C# notes no Pokedex or sprite download).
pub fn download_string_unnoted(url: &str) -> Result<String, String> { string(url, None, false) }
fn string(url: &str, accept: Option<&str>, noted: bool) -> Result<String, String> {
    if let Some(r) = crate::test_hooks::fetch(url, noted) { return r.map(|b| String::from_utf8_lossy(&b).into_owned()); }
    let mut b = Vec::new();
    get(url, accept)?.read_to_end(&mut b).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// DownloadFile, reporting the share done (0..1) when the size is known. A download past `max` bytes stops.
pub fn download_file(url: &str, to: &Path, max: Option<u64>, progress: &mut dyn FnMut(f32)) -> Result<(), String> { file(url, to, max, progress, true) }
pub fn download_file_unnoted(url: &str, to: &Path) -> Result<(), String> { file(url, to, None, &mut |_| {}, false) }
fn file(url: &str, to: &Path, max: Option<u64>, progress: &mut dyn FnMut(f32), noted: bool) -> Result<(), String> {
    if let Some(r) = crate::test_hooks::fetch(url, noted) { return std::fs::write(to, r?).map_err(|e| e.to_string()); }
    let mut input = get(url, None)?;
    let mut out = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 81920];
    let mut got: u64 = 0;
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        got += n as u64;
        if max.map_or(false, |m| got > m) { return Err("the download is larger than nativeMons installs from the index".into()); }
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
        progress(got as f32);
    }
    Ok(())
}
