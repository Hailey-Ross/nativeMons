//! Zip extraction as System.IO.Compression does it: each file keeps its entry's time (DOS time, read as local).
//! Also .NET's wording for a missing file, where the C# shows an exception's message.

use chrono::TimeZone;
use std::io::{Read, Seek};
use std::path::Path;

fn entry_time(e: &zip::read::ZipFile<'_>) -> Option<std::time::SystemTime> {
    let t = e.last_modified();
    let local = chrono::Local.with_ymd_and_hms(t.year() as i32, t.month() as u32, t.day() as u32, t.hour() as u32, t.minute() as u32, t.second() as u32).earliest()?;
    Some(std::time::SystemTime::from(local))
}
/// ExtractToFile: the entry's bytes, then its time on the file.
pub fn extract_entry<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, i: usize, to: &Path) -> Result<(), String> {
    let mut e = archive.by_index(i).map_err(|x| x.to_string())?;
    let when = entry_time(&e);
    let mut out = std::fs::File::create(to).map_err(|x| x.to_string())?;
    std::io::copy(&mut e, &mut out).map_err(|x| x.to_string())?;
    if let Some(w) = when { let _ = out.set_modified(w); }
    Ok(())
}
/// ExtractToDirectory: every entry under `to`, folders made as needed.
pub fn extract_all(zip: &str, to: &Path) -> Result<(), String> {
    let mut a = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|x| missing(&x, zip))?).map_err(|x| x.to_string())?;
    for i in 0..a.len() {
        let (name, dir) = {
            let e = a.by_index(i).map_err(|x| x.to_string())?;
            (e.enclosed_name().map(|p| p.to_path_buf()), e.is_dir())
        };
        let Some(name) = name else { continue };
        let target = to.join(name);
        if dir { std::fs::create_dir_all(&target).map_err(|x| x.to_string())?; continue; }
        if let Some(p) = target.parent() { std::fs::create_dir_all(p).map_err(|x| x.to_string())?; }
        extract_entry(&mut a, i, &target)?;
    }
    Ok(())
}
/// An io error in .NET's words when it's a missing file ("Could not find file '<full path>'.").
pub fn missing(e: &std::io::Error, path: &str) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        let full = crate::paths::full_path(Path::new(path));
        let parent_ok = full.parent().map_or(true, |p| p.is_dir());
        if parent_ok { format!("Could not find file '{}'.", full.display()) } else { format!("Could not find a part of the path '{}'.", full.display()) }
    } else {
        e.to_string()
    }
}
