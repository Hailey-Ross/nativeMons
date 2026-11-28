//! ModIndex: the community list of Gen1Recomp mods (a JSON feed, cached six hours), and installing one from it.

use crate::config;
use serde_json::Value;

pub const FEED: &str = "https://bryanthaboi.github.io/gen1recomp-mod-index/data/index.json";
const MAX_ZIP: i64 = 300 * 1024 * 1024;

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub version: Option<String>,
    pub zip_url: Option<String>,
    pub games: Vec<String>,
    pub needs: Vec<String>,
    pub clashes: Vec<String>,
    pub experimental: bool,
    pub zip_size: i64,
    pub downloads: i64,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub repo: Option<String>,
    pub license: Option<String>,
    pub folder: Option<String>,
    pub categories: Vec<String>,
    pub permissions: Vec<String>,
}
fn cache_file() -> std::path::PathBuf { config::state().join("mod-index.json") }
fn s(d: Option<&Value>, k: &str) -> Option<String> { d.and_then(|d| d.get(k)).and_then(|v| v.as_str()).map(String::from) }
fn strs(d: &Value, k: &str) -> Vec<String> { d.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() }
fn num(v: Option<&Value>) -> Option<i64> { v.and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f.round() as i64)).or_else(|| x.as_str().and_then(|t| t.trim().parse().ok()))) }
fn spec_ids(d: &Value, k: &str) -> Vec<String> { strs(d, k).iter().map(|spec| spec.split(['@', '#']).next().unwrap_or("").to_string()).filter(|id| !id.is_empty()).collect() }

fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    if num(root.get("schema_version")) != Some(1) { return Err("The mod index is in a newer form than this launcher reads. Update nativeMons.".into()); }
    let mut list = Vec::new();
    for m in root.get("mods").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        if !m.is_object() { continue; }
        let mut e = Entry { id: s(Some(&m), "id").unwrap_or_default(), title: s(Some(&m), "title").unwrap_or_default(), version: s(Some(&m), "version"), downloads: -1, ..Default::default() };
        if e.id.is_empty() || e.title.is_empty() { continue; }
        e.games = strs(&m, "games");
        e.author = s(Some(&m), "author");
        e.summary = s(Some(&m), "summary");
        e.repo = s(Some(&m), "repo");
        e.license = s(Some(&m), "license");
        e.folder = s(Some(&m), "folder");
        e.categories = strs(&m, "categories");
        e.permissions = strs(&m, "permissions");
        // Its required mods, by id (a spec string is id, then @range, then #repo).
        e.needs = spec_ids(&m, "dependencies");
        e.clashes = spec_ids(&m, "conflicts");
        e.experimental = m.get("experimental").and_then(|v| v.as_bool()).unwrap_or(false);
        let latest = m.get("latest").filter(|v| v.is_object());
        let zip = latest.and_then(|l| l.get("zip")).filter(|v| v.is_object());
        if let Some(v) = s(latest, "version").filter(|v| !v.is_empty()) { e.version = Some(v); }
        e.zip_url = s(zip, "url").or_else(|| s(Some(&m), "downloadURL"));
        if let Some(n) = num(zip.and_then(|z| z.get("size"))) { e.zip_size = n; }
        if let Some(n) = num(m.get("downloads").filter(|v| v.is_object()).and_then(|d| d.get("total"))) { e.downloads = n; }
        // Nothing installable without an https zip.
        if !e.zip_url.as_deref().map_or(false, |u| u.to_lowercase().starts_with("https://")) { continue; }
        list.push(e);
    }
    list.sort_by(|a, b| b.downloads.cmp(&a.downloads).then_with(|| crate::save_guard::ordinal(&a.title.to_uppercase(), &b.title.to_uppercase())));
    Ok(list)
}
/// CachedCount: how many the kept copy lists (0 without one).
pub fn cached_count() -> usize { std::fs::read_to_string(cache_file()).ok().and_then(|t| parse(&t).ok()).map_or(0, |l| l.len()) }
/// The list, from the feed; a copy kept six hours, and used whenever the feed can't be reached. (list, note)
pub fn load() -> Result<(Vec<Entry>, Option<String>), String> {
    let mut note = None;
    let mut text: Option<String> = None;
    let f = cache_file();
    let fresh = std::fs::metadata(&f).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(false, |a| a.as_secs() <= 6 * 3600);
    if !fresh {
        let got = crate::net::download_string(FEED, None).and_then(|t| { parse(&t)?; Ok(t) });
        match got {
            Ok(t) => {
                let _ = std::fs::create_dir_all(config::state());
                let part = std::path::PathBuf::from(format!("{}.part", f.display()));
                // File.WriteAllText with Encoding.UTF8 starts with a BOM.
                std::fs::write(&part, format!("\u{feff}{}", t)).map_err(|e| e.to_string())?;
                if f.is_file() { std::fs::remove_file(&f).map_err(|e| e.to_string())?; }
                std::fs::rename(&part, &f).map_err(|e| e.to_string())?;
                text = Some(t);
            }
            Err(x) => {
                if !f.is_file() { return Err(format!("The mod index couldn't be reached. {}", x)); }
                let when: chrono::DateTime<chrono::Local> = std::fs::metadata(&f).and_then(|m| m.modified()).map(chrono::DateTime::from).unwrap_or_else(|_| chrono::Local::now());
                note = Some(format!("The mod index couldn't be reached, so this is the list from {}.", when.format("%-d %b, %-I:%M %p")));
            }
        }
    }
    let text = match text { Some(t) => t, None => std::fs::read_to_string(&f).map_err(|e| crate::zips::missing(&e, &f.to_string_lossy()))? };
    Ok((parse(&text)?, note))
}
fn generation(version: &str) -> &'static str {
    match version { "red" | "blue" | "yellow" => "gen1", "gold" | "silver" | "crystal" => "gen2", _ => "gen3" }
}
/// A mod that names no games is offered everywhere.
pub fn fits(e: &Entry, version: Option<&str>) -> bool {
    let v = version.unwrap_or("");
    e.games.is_empty() || e.games.iter().any(|x| x.eq_ignore_ascii_case(v) || x.eq_ignore_ascii_case(generation(v)))
}
/// Downloads the release .zip as <id>.zip and installs it.
pub fn install(e: &Entry) -> String {
    if e.zip_size > MAX_ZIP { return format!("{} is {} MB, larger than nativeMons installs from the index.", e.title, e.zip_size / (1024 * 1024)); }
    install_from(e.zip_url.as_deref(), &e.id, &e.title)
}
/// Any https link to a mod's .zip, checked to be the mod named id before it goes in.
pub fn install_from(url: Option<&str>, id: &str, title: &str) -> String {
    let dir = config::state().join("mod-downloads");
    let zip = dir.join(format!("{}.zip", id));
    let part = std::path::PathBuf::from(format!("{}.part", zip.display()));
    let _ = std::fs::create_dir_all(&dir);
    let r = (|| -> Result<String, String> {
        let Some(url) = url.filter(|u| u.to_lowercase().starts_with("https://")) else { return Ok("Only https:// links are used.".into()) };
        crate::net::download_file(url, &part, Some(MAX_ZIP as u64), &mut |_| {})?;
        if zip.is_file() { std::fs::remove_file(&zip).map_err(|e| e.to_string())?; }
        std::fs::rename(&part, &zip).map_err(|e| e.to_string())?;
        crate::mods::extract_mod(&zip.to_string_lossy(), Some(id))
    })();
    let _ = std::fs::remove_file(&part);
    let _ = std::fs::remove_file(&zip);
    r.unwrap_or_else(|x| format!("Couldn't install {}: {}", title, x))
}
/// GithubZip: the newest release of a repository with a .zip (one named "<id>-..." first), prereleases last.
/// (url, size, version)
pub fn github_zip(repo: &str, id: &str) -> Result<Option<(String, i64, String)>, String> {
    let text = crate::net::download_string(&format!("https://api.github.com/repos/{}/releases?per_page=10", repo), Some("application/vnd.github+json"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut releases: Vec<&Value> = v.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    releases.sort_by_key(|r| if r.get("prerelease").and_then(|x| x.as_bool()) == Some(true) { 1 } else { 0 });
    for r in releases {
        if r.get("draft").and_then(|x| x.as_bool()) == Some(true) { continue; }
        let assets: Vec<&Value> = r.get("assets").and_then(|a| a.as_array()).map(|a| a.iter().filter(|x| x.is_object()).collect()).unwrap_or_default();
        let name = |x: &Value| x.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let zips: Vec<&Value> = assets.into_iter().filter(|x| name(x).to_lowercase().ends_with(".zip")).collect();
        let pick = zips.iter().find(|x| name(x).to_lowercase().starts_with(&format!("{}-", id.to_lowercase()))).or_else(|| zips.first());
        let Some(pick) = pick else { continue };
        let size = pick.get("size").and_then(|x| x.as_i64()).unwrap_or(0);
        let version = r.get("tag_name").and_then(|x| x.as_str()).unwrap_or("").trim_start_matches(['v', 'V']).to_string();
        return Ok(pick.get("browser_download_url").and_then(|x| x.as_str()).map(|u| (u.to_string(), size, version)));
    }
    Ok(None)
}
