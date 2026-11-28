//! Submit: shared packages to the submission service (Zakuro hint lists), signed with this install's own P-256 key,
//! kept encrypted for the user, each on a one-time challenge, in the one layout the server accepts. Ported from the
//! C#. The network send is a `Transport`: `Https` in the launcher, `Stubbed` in replays.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// POST: (path, content type, body, signature) -> the reply's body, or why not.
pub trait Transport { fn post(&self, path: &str, content_type: &str, body: &[u8], signature: Option<&str>) -> Result<String, String>; }

/// Stands in where the C#'s TestHooks.Todo does (replays): nothing leaves the PC.
pub struct Stubbed;
impl Transport for Stubbed {
    fn post(&self, _: &str, _: &str, _: &[u8], _: Option<&str>) -> Result<String, String> { Err("a replay sends nothing".into()) }
}

/// The C#'s Post: HTTPS (rustls) to NATIVEMONS_SUBMIT_URL or submit.hails.app, 30 s, no redirects.
pub struct Https { pub base: String }
impl Https {
    pub fn from_env() -> Https { Https { base: std::env::var("NATIVEMONS_SUBMIT_URL").unwrap_or_else(|_| "https://submit.hails.app".into()).trim_end_matches('/').to_string() } }
}
impl Transport for Https {
    fn post(&self, path: &str, content_type: &str, body: &[u8], signature: Option<&str>) -> Result<String, String> {
        let agent = ureq::AgentBuilder::new().redirects(0).timeout(std::time::Duration::from_secs(30)).build();
        let mut req = agent.post(&format!("{}{}", self.base, path)).set("Content-Type", content_type).set("User-Agent", "nativeMons");
        if let Some(s) = signature { req = req.set("X-NM-Signature", s); }
        match req.send_bytes(body) {
            Ok(r) => r.into_string().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(code, r)) => {
                let text = r.status_text().to_string();
                let why = r.into_string().ok().and_then(|b| regex::Regex::new("\"error\":\"([a-z ]{1,40})\"").unwrap().captures(&b).map(|m| m[1].to_string()));
                Err(match why { Some(w) => format!("the server said: {}", w), None => format!("The remote server returned an error: ({}) {}.", code, text) })
            }
            Err(ureq::Error::Transport(t)) => Err(transport_message(&t)),
        }
    }
}
/// WebException's wording for the common failures.
fn transport_message(t: &ureq::Transport) -> String {
    let io = std::error::Error::source(t).and_then(|e| e.downcast_ref::<std::io::Error>()).map(|e| e.kind());
    match (t.kind(), io) {
        (_, Some(std::io::ErrorKind::TimedOut)) | (_, Some(std::io::ErrorKind::WouldBlock)) => "The operation has timed out".into(),
        (ureq::ErrorKind::ConnectionFailed, _) => "Unable to connect to the remote server".into(),
        (ureq::ErrorKind::Dns, _) => format!("The remote name could not be resolved: '{}'", t.url().and_then(|u| u.host_str()).unwrap_or("")),
        _ => t.to_string(),
    }
}

const ENTROPY: &[u8] = b"nativeMons submit key v1";
fn key_file() -> PathBuf { crate::config::state().join("submit").join("install.key") }

/// The install's key: read back from its protected file, else made and kept (96 bytes: D, X, Y).
fn load_key() -> Result<p256::ecdsa::SigningKey, String> {
    let f = key_file();
    if let Ok(data) = std::fs::read(&f) {
        if let Some(raw) = unprotect(&data) {
            if raw.len() == 96 { if let Ok(k) = p256::ecdsa::SigningKey::from_slice(&raw[..32]) { return Ok(k); } }
        }
    }
    let key = p256::ecdsa::SigningKey::random(&mut rand_core::OsRng);
    let mut blob = key.to_bytes().to_vec();
    blob.extend_from_slice(&public_point(&key)[1..]);
    std::fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&f, protect(&blob)?).map_err(|e| e.to_string())?;
    Ok(key)
}
/// The public key, uncompressed (0x04, X, Y).
fn public_point(key: &p256::ecdsa::SigningKey) -> Vec<u8> { key.verifying_key().to_encoded_point(false).as_bytes().to_vec() }

/// DPAPI for the Windows user, with the C#'s entropy (so either launcher reads the other's key).
#[cfg(windows)]
fn dpapi(data: &[u8], seal: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let inp = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let ent = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // CRYPTPROTECT_UI_FORBIDDEN, as .NET's ProtectedData passes.
    let ok = unsafe {
        if seal { CryptProtectData(&inp, std::ptr::null(), &ent, std::ptr::null(), std::ptr::null(), 1, &mut out) }
        else { CryptUnprotectData(&inp, std::ptr::null_mut(), &ent, std::ptr::null(), std::ptr::null(), 1, &mut out) }
    };
    if ok == 0 || out.pbData.is_null() { return None; }
    let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
    unsafe { windows_sys::Win32::Foundation::LocalFree(out.pbData as _); }
    Some(v)
}
#[cfg(windows)]
fn protect(raw: &[u8]) -> Result<Vec<u8>, String> { dpapi(raw, true).ok_or_else(|| "the key couldn't be protected".into()) }
#[cfg(windows)]
fn unprotect(data: &[u8]) -> Option<Vec<u8>> { dpapi(data, false) }
// No DPAPI on Linux: the key file is the user's alone (0600) instead.
#[cfg(not(windows))]
fn protect(raw: &[u8]) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::PermissionsExt;
    let f = key_file();
    if f.exists() { let _ = std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)); }
    Ok(raw.to_vec())
}
#[cfg(not(windows))]
fn unprotect(data: &[u8]) -> Option<Vec<u8>> { Some(data.to_vec()) }

pub fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 { o.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' }); }
    }
    o
}

/// Clean: only the characters the server takes.
pub fn clean(s: &str, max: usize) -> String {
    let t: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || " .,()-".contains(c) { c } else { ' ' }).collect();
    let mut t = t.trim().to_string();
    if t.chars().count() > max { t = t.chars().take(max).collect::<String>().trim().to_string(); }
    if t.is_empty() { "unknown".into() } else { t }
}
fn zakuro_build() -> String {
    let f = crate::config::zakuro().parent().map(|d| d.join("NATIVEMONS-BUILD.txt")).unwrap_or_default();
    std::fs::read_to_string(f).ok().and_then(|t| regex::Regex::new(r"/tree/([0-9a-f]{9})").unwrap().captures(&t).map(|m| m[1].to_string())).unwrap_or_else(|| "unknown".into())
}

/// The addresses a list holds (hex, 0x optional), in the range Zakuro's code covers, sorted, at most 65,536.
pub fn addresses(lines: &[String]) -> BTreeSet<u32> {
    let re = regex::Regex::new("^(0x)?([0-9A-Fa-f]{1,8})$").unwrap();
    let mut addrs: BTreeSet<u32> = BTreeSet::new();
    for l in lines {
        let Some(m) = re.captures(l.trim()) else { continue };
        let a = u32::from_str_radix(&m[2], 16).unwrap_or(0);
        if (0x0010_0000..0x1000_0000).contains(&a) { addrs.insert(a); }
    }
    if addrs.len() > 65536 { addrs = addrs.into_iter().take(65536).collect(); }
    addrs
}

/// The package, exactly as the C# writes it.
pub fn package(code_id: &str, addrs: &BTreeSet<u32>, public_key: &str, nonce: &str, created: i64, launcher: &str, zakuro: &str) -> String {
    let body = format!("{{\"code\":\"{}\",\"addresses\":[{}]}}", code_id, addrs.iter().map(|a| format!("\"{:08X}\"", a)).collect::<Vec<_>>().join(","));
    format!("{{\"v\":1,\"kind\":\"zakuro-hints\",\"key\":\"{}\",\"nonce\":\"{}\",\"created\":{},\"launcher\":\"{}\",\"zakuro\":\"{}\",\"body\":{}}}", public_key, nonce, created, clean(launcher, 80), zakuro, body)
}

/// Sends a Zakuro hint list; the receipt, or why not.
pub fn zakuro_hints(code_id: &str, lines: &[String]) -> Result<String, String> {
    if crate::test_hooks::replay() { zakuro_hints_via(&Stubbed, code_id, lines) } else { zakuro_hints_via(&Https::from_env(), code_id, lines) }
}
pub fn zakuro_hints_via(t: &dyn Transport, code_id: &str, lines: &[String]) -> Result<String, String> {
    let addrs = addresses(lines);
    if addrs.is_empty() { return Err("there are no findings to send".into()); }
    let key = load_key()?;
    let public = base64(&public_point(&key));
    let ch = t.post("/v1/challenge", "application/json", format!("{{\"key\":\"{}\"}}", public).as_bytes(), None)?;
    let since = std::time::Instant::now();
    let nonce = regex::Regex::new("\"nonce\":\"([A-Za-z0-9+/]{43}=)\"").unwrap().captures(&ch).map(|m| m[1].to_string()).unwrap_or_default();
    let now = regex::Regex::new("\"now\":([1-9][0-9]{9})[,}]").unwrap().captures(&ch).map(|m| m[1].to_string());
    let Some(now) = now.filter(|_| !nonce.is_empty()) else { return Err("the server's challenge wasn't readable".into()) };
    // The server's clock, never this PC's: a wrong PC clock would get the package refused.
    let created = now.parse::<i64>().unwrap_or(0) + since.elapsed().as_secs() as i64;
    let pkg = package(code_id, &addrs, &public, &nonce, created, &format!("nativeMons {}", crate::updater::installed_label()), &zakuro_build());
    use p256::ecdsa::signature::Signer;
    let sig: p256::ecdsa::Signature = key.sign(pkg.as_bytes());
    let done = t.post("/v1/submit/zakuro-hints", "application/vnd.nativemons.package+json", pkg.as_bytes(), Some(&base64(&sig.to_bytes())))?;
    regex::Regex::new("\"receipt\":\"([0-9]{8}-[0-9a-f]{16})\"").unwrap().captures(&done).map(|m| m[1].to_string()).ok_or_else(|| "the server didn't give a receipt".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    /// A local stand-in for the service: answers the challenge, checks the signature, gives a receipt.
    struct Fake { sent: RefCell<Vec<(String, Vec<u8>, Option<String>)>> }
    impl Transport for Fake {
        fn post(&self, path: &str, _: &str, body: &[u8], sig: Option<&str>) -> Result<String, String> {
            self.sent.borrow_mut().push((path.into(), body.to_vec(), sig.map(String::from)));
            if path == "/v1/challenge" { return Ok("{\"nonce\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\",\"now\":1790000000}".into()); }
            Ok("{\"ok\":true,\"receipt\":\"20261127-0123456789abcdef\"}".into())
        }
    }
    #[test]
    fn base64_and_clean() {
        assert_eq!(base64(b""), ""); assert_eq!(base64(b"f"), "Zg=="); assert_eq!(base64(b"fo"), "Zm8="); assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(clean("nativeMons 0.0.8d (dev, abc123)", 80), "nativeMons 0.0.8d (dev, abc123)");
        assert_eq!(clean("a/b\u{e9}", 80), "a b");
        assert_eq!(clean("\u{e9}\u{e9}", 80), "unknown");
    }
    #[test]
    fn package_layout() {
        let a = addresses(&["0x00100000".into(), "1234567".into(), "fffff".into(), "zz".into(), "0x10000000".into()]);
        assert_eq!(a.iter().copied().collect::<Vec<_>>(), vec![0x0010_0000, 0x0123_4567]);
        assert_eq!(package("0004000000055D00-1", &a, "BKEY", "N=", 1790000003, "nativeMons dev", "abcdef012"),
            "{\"v\":1,\"kind\":\"zakuro-hints\",\"key\":\"BKEY\",\"nonce\":\"N=\",\"created\":1790000003,\"launcher\":\"nativeMons dev\",\"zakuro\":\"abcdef012\",\"body\":{\"code\":\"0004000000055D00-1\",\"addresses\":[\"00100000\",\"01234567\"]}}");
    }
    #[test]
    fn signs_and_keeps_its_key() {
        let dir = std::env::temp_dir().join(format!("nm-submit-{}", std::process::id()));
        std::env::set_var("NATIVEMONS_STATE", &dir);
        let fake = Fake { sent: RefCell::new(Vec::new()) };
        let r = zakuro_hints_via(&fake, "0004000000055D00-1", &["0x00200000".into()]);
        assert_eq!(r.as_deref(), Ok("20261127-0123456789abcdef"));
        let sent = fake.sent.borrow();
        let (_, pkg, sig) = &sent[1];
        // The signature checks against the key the challenge named; the key file reads back as the same key.
        let key = load_key().unwrap();
        use p256::ecdsa::signature::Verifier;
        let raw = decode64(sig.as_deref().unwrap());
        let sig = p256::ecdsa::Signature::from_slice(&raw).unwrap();
        assert!(key.verifying_key().verify(pkg, &sig).is_ok());
        assert!(String::from_utf8_lossy(&sent[0].1).contains(&base64(&public_point(&key))));
        assert!(String::from_utf8_lossy(pkg).contains("\"created\":17900000"));
        // The whole send through the real transport to a local fake: the same two requests, the receipt.
        let (base, h) = serve(vec![reply("200 OK", "{\"nonce\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\",\"now\":1790000000}"), reply("200 OK", "{\"ok\":true,\"receipt\":\"20261127-0123456789abcdef\"}")]);
        assert_eq!(zakuro_hints_via(&Https { base }, "0004000000055D00-1", &["0x00200000".into()]).as_deref(), Ok("20261127-0123456789abcdef"));
        let raw = h.join().unwrap();
        let (c, _) = as_csharp(&raw[0]);
        assert!(c.ends_with(&format!("\r\n\r\n{{\"key\":\"{}\"}}", base64(&public_point(&key)))));
        let (p, _) = as_csharp(&raw[1]);
        let line = p.lines().find(|x| x.starts_with("X-NM-Signature: ")).unwrap();
        let body = p.split("\r\n\r\n").nth(1).unwrap();
        let s2 = p256::ecdsa::Signature::from_slice(&decode64(&line[16..])).unwrap();
        assert!(key.verifying_key().verify(body.as_bytes(), &s2).is_ok());
        assert!(body.starts_with("{\"v\":1,\"kind\":\"zakuro-hints\",\"key\":\"") && body.ends_with("\"body\":{\"code\":\"0004000000055D00-1\",\"addresses\":[\"00200000\"]}}"));
        let _ = std::fs::remove_dir_all(&dir);
        // Nothing to send: refused before any request.
        assert!(zakuro_hints("x", &["hello".into()]).is_err());
    }
    /// A fake service on 127.0.0.1: keeps each raw request, answers with the canned replies in turn.
    fn serve(replies: Vec<String>) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let h = std::thread::spawn(move || replies.into_iter().map(|reply| {
            let (mut c, _) = l.accept().unwrap();
            let mut raw = Vec::new();
            let mut b = [0u8; 4096];
            loop {
                let n = c.read(&mut b).unwrap(); raw.extend_from_slice(&b[..n]);
                let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else { if n == 0 { break } else { continue } };
                let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                let len: usize = head.lines().find_map(|x| x.strip_prefix("content-length: ")).map_or(0, |v| v.trim().parse().unwrap());
                if raw.len() >= end + 4 + len || n == 0 { break; }
            }
            c.write_all(reply.as_bytes()).unwrap();
            raw
        }).collect());
        (base, h)
    }
    fn reply(status: &str, body: &str) -> String { format!("HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body) }
    /// The request as the C#'s HttpWebRequest writes the parts it sets: request line, the headers Post names, the body.
    fn as_csharp(raw: &[u8]) -> (String, Vec<String>) {
        let end = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&raw[..end]).to_string();
        let mut lines = head.split("\r\n");
        let mut out = format!("{}\r\n", lines.next().unwrap());
        let (mut set, mut other): (Vec<&str>, Vec<String>) = (Vec::new(), Vec::new());
        for x in lines {
            let name = x.split(':').next().unwrap().to_ascii_lowercase();
            if ["content-type", "user-agent", "x-nm-signature", "host", "content-length"].contains(&name.as_str()) { set.push(x); } else { other.push(name); }
        }
        set.sort_by_key(|x| ["content-type", "user-agent", "x-nm-signature", "host", "content-length"].iter().position(|n| x.to_ascii_lowercase().starts_with(n)));
        for x in set { out.push_str(x); out.push_str("\r\n"); }
        out.push_str("\r\n");
        out.push_str(&String::from_utf8_lossy(&raw[end + 4..]));
        (out, other)
    }
    #[test]
    fn https_posts_as_the_csharp() {
        let sig = "MEUCIQ+/=";
        let (base, h) = serve(vec![reply("200 OK", "{\"nonce\":\"x\"}"), reply("200 OK", "{\"receipt\":\"r\"}")]);
        let t = Https { base: format!("{}/", base).trim_end_matches('/').to_string() };
        assert_eq!(t.post("/v1/challenge", "application/json", b"{\"key\":\"BKEY\"}", None).as_deref(), Ok("{\"nonce\":\"x\"}"));
        assert_eq!(t.post("/v1/submit/zakuro-hints", "application/vnd.nativemons.package+json", b"{\"v\":1}", Some(sig)).as_deref(), Ok("{\"receipt\":\"r\"}"));
        let raw = h.join().unwrap();
        let host = base.trim_start_matches("http://");
        let (a, extra) = as_csharp(&raw[0]);
        assert_eq!(a, format!("POST /v1/challenge HTTP/1.1\r\nContent-Type: application/json\r\nUser-Agent: nativeMons\r\nHost: {}\r\nContent-Length: 14\r\n\r\n{{\"key\":\"BKEY\"}}", host));
        // Only transport headers besides (the C# adds Expect and Connection; ureq Accept and Accept-Encoding).
        assert!(extra.iter().all(|n| n == "accept" || n == "accept-encoding"), "{:?}", extra);
        let (b, _) = as_csharp(&raw[1]);
        assert_eq!(b, format!("POST /v1/submit/zakuro-hints HTTP/1.1\r\nContent-Type: application/vnd.nativemons.package+json\r\nUser-Agent: nativeMons\r\nX-NM-Signature: {}\r\nHost: {}\r\nContent-Length: 7\r\n\r\n{{\"v\":1}}", sig, host));
    }
    #[test]
    fn https_errors_as_the_csharp() {
        let (base, h) = serve(vec![
            reply("400 Bad Request", "{\"ok\":false,\"error\":\"bad signature\"}"),
            reply("404 Not Found", "nope"),
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        ]);
        let t = Https { base: base.clone() };
        assert_eq!(t.post("/v1/challenge", "application/json", b"{}", None), Err("the server said: bad signature".into()));
        assert_eq!(t.post("/v1/challenge", "application/json", b"{}", None), Err("The remote server returned an error: (404) Not Found.".into()));
        // No redirect followed: the 302's empty body comes back, and the challenge then isn't readable.
        assert_eq!(t.post("/v1/challenge", "application/json", b"{}", None), Ok(String::new()));
        h.join().unwrap();
        let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let gone = Https { base: format!("http://{}", dead.local_addr().unwrap()) };
        drop(dead);
        assert_eq!(gone.post("/v1/challenge", "application/json", b"{}", None), Err("Unable to connect to the remote server".into()));
    }
    fn decode64(s: &str) -> Vec<u8> {
        const T: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let v: Vec<u32> = s.chars().filter(|c| *c != '=').map(|c| T.find(c).unwrap() as u32).collect();
        let mut o = Vec::new();
        for c in v.chunks(4) {
            let n = c.iter().enumerate().fold(0u32, |n, (i, x)| n | x << (18 - 6 * i));
            for i in 0..c.len() - 1 { o.push((n >> (16 - 8 * i)) as u8); }
        }
        o
    }
}
