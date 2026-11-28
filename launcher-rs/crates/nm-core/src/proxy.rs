//! The proxy a request goes through, as the C#'s WebRequest.DefaultWebProxy picks it: Windows' Internet Settings
//! (manual list, bypass list, auto-detect and auto-config through WinHTTP); on Linux the usual proxy variables.

use std::io::{Read, Write};

/// An HTTP proxy: host, port and optional "user:password" for Basic auth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyServer { pub host: String, pub port: u16, pub auth: Option<String> }

/// The system's proxy for `url`, or None to go direct. Loopback is always direct, as in .NET.
pub fn for_url(url: &url::Url) -> Option<ProxyServer> {
    if is_loopback(url) { return None; }
    system(url)
}

fn is_loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(a)) => a.is_loopback(),
        Some(url::Host::Ipv6(a)) => a.is_loopback(),
        None => true,
    }
}

/// One entry of a proxy list ("host:port", "http://host:port/", "user:pass@host"); port 80 when left out.
pub fn parse_server(s: &str) -> Option<ProxyServer> {
    let s = s.trim();
    let s = match s.find("://") { Some(i) if s[..i].eq_ignore_ascii_case("http") => &s[i + 3..], Some(_) => return None, None => s };
    let s = s.trim_end_matches('/');
    let (auth, hp) = match s.rfind('@') { Some(i) => (Some(s[..i].to_string()), &s[i + 1..]), None => (None, s) };
    let (host, port) = match hp.rfind(':') {
        Some(i) if !hp[i..].contains(']') => (&hp[..i], hp[i + 1..].parse().ok()?),
        _ => (hp, 80),
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() { return None; }
    Some(ProxyServer { host: host.to_string(), port, auth })
}

/// Internet Settings' "ProxyServer": one server for all, or "http=a:1;https=b:2" per scheme (none for it: direct).
pub fn pick_ie(list: &str, scheme: &str) -> Option<ProxyServer> {
    let entries: Vec<&str> = list.split(|c: char| c == ';' || c.is_whitespace()).filter(|e| !e.is_empty()).collect();
    if entries.iter().any(|e| e.contains('=')) {
        entries.iter().filter_map(|e| e.split_once('=')).find(|(k, _)| k.trim().eq_ignore_ascii_case(scheme)).and_then(|(_, v)| parse_server(v))
    } else {
        entries.first().and_then(|e| parse_server(e))
    }
}

/// .NET's bypass list: ';'-separated, '*' wildcards, optional scheme and port, "<local>" for dotless hosts.
pub fn bypassed_ie(list: &str, url: &url::Url) -> bool {
    let host = url.host_str().unwrap_or("").trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    list.split(';').map(str::trim).filter(|e| !e.is_empty()).any(|e| {
        if e.eq_ignore_ascii_case("<local>") { return !host.contains('.') && !host.contains(':'); }
        let (scheme, rest) = match e.find("://") { Some(i) => (Some(&e[..i]), &e[i + 3..]), None => (None, e) };
        if scheme.map_or(false, |s| !s.eq_ignore_ascii_case(url.scheme())) { return false; }
        let (pat, port) = match rest.rfind(':') { Some(i) if !rest[i..].contains(']') => (&rest[..i], Some(&rest[i + 1..])), _ => (rest, None) };
        // The URL's port only counts when it isn't the default, as .NET matches "scheme://host[:port]".
        let port_ok = match (port, url.port()) { (None, _) => true, (Some(p), Some(u)) => glob(p, &u.to_string()), (Some(_), None) => false };
        port_ok && glob(&pat.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase(), &host)
    })
}

fn glob(pat: &str, s: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 { return pat == s; }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !s.starts_with(first) || s.len() < first.len() + last.len() || !s.ends_with(last) { return false; }
    let mut at = first.len();
    let end = s.len() - last.len();
    for p in &parts[1..parts.len() - 1] {
        match s[at..end].find(p) { Some(i) => at += i + p.len(), None => return false }
    }
    true
}

/// curl's no_proxy: ','-separated hosts or domain suffixes (leading '.' optional), optional ":port", '*' for all.
pub fn bypassed_env(list: &str, url: &url::Url) -> bool {
    let host = url.host_str().unwrap_or("").trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    let port = url.port_or_known_default();
    list.split(',').map(str::trim).filter(|e| !e.is_empty()).any(|e| {
        if e == "*" { return true; }
        let (name, p) = match e.rfind(':') { Some(i) if !e[i..].contains(']') && !e[..i].contains(':') => (&e[..i], e[i + 1..].parse::<u16>().ok()), _ => (e, None) };
        if p.is_some() && p != port { return false; }
        let name = name.trim_start_matches('.').trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
        host == name || host.ends_with(&format!(".{}", name))
    })
}

/// The proxy from variables as `get` reads them (https_proxy, http_proxy, all_proxy; no_proxy).
pub fn from_env(url: &url::Url, get: &dyn Fn(&str) -> Option<String>) -> Option<ProxyServer> {
    let var = |n: &str| get(n).or_else(|| get(&n.to_ascii_uppercase())).filter(|v| !v.trim().is_empty());
    // Like curl, http_proxy is only read in lowercase.
    let p = if url.scheme() == "https" { var("https_proxy") } else { get("http_proxy").filter(|v| !v.trim().is_empty()) }.or_else(|| var("all_proxy"))?;
    if var("no_proxy").map_or(false, |l| bypassed_env(&l, url)) { return None; }
    parse_server(&p)
}

#[cfg(not(windows))]
fn system(url: &url::Url) -> Option<ProxyServer> { from_env(url, &|n| std::env::var(n).ok()) }

#[cfg(windows)]
fn system(url: &url::Url) -> Option<ProxyServer> { win::ie(url) }

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::Networking::WinHttp::*;

    static DETECT_FAILED: AtomicBool = AtomicBool::new(false);

    unsafe fn take(p: *mut u16) -> Option<String> {
        if p.is_null() { return None; }
        let mut n = 0;
        while *p.add(n) != 0 { n += 1; }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
        GlobalFree(p as _);
        Some(s)
    }
    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }

    /// Internet Settings for the current user: auto-detect and the auto-config script first, then the manual list.
    pub fn ie(url: &url::Url) -> Option<ProxyServer> {
        let mut c: WINHTTP_CURRENT_USER_IE_PROXY_CONFIG = unsafe { std::mem::zeroed() };
        if unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut c) } == 0 { return None; }
        let (pac, list, bypass) = unsafe { (take(c.lpszAutoConfigUrl), take(c.lpszProxy), take(c.lpszProxyBypass)) };
        let detect = c.fAutoDetect != 0 && !DETECT_FAILED.load(Ordering::Relaxed);
        if detect || pac.is_some() {
            if let Some(found) = auto(url, detect, pac.as_deref()) { return found; }
        }
        let list = list?;
        if bypass.as_deref().map_or(false, |b| bypassed_ie(b, url)) { return None; }
        pick_ie(&list, url.scheme())
    }

    /// WinHTTP's answer from the script: Some(None) for direct, None when there's no answer (falls back to the list).
    fn auto(url: &url::Url, detect: bool, pac: Option<&str>) -> Option<Option<ProxyServer>> {
        unsafe {
            let session = WinHttpOpen(std::ptr::null(), WINHTTP_ACCESS_TYPE_NO_PROXY, std::ptr::null(), std::ptr::null(), 0);
            if session.is_null() { return None; }
            let pac_w = pac.map(wide);
            let mut o: WINHTTP_AUTOPROXY_OPTIONS = std::mem::zeroed();
            if detect { o.dwFlags |= WINHTTP_AUTOPROXY_AUTO_DETECT; o.dwAutoDetectFlags = WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A; }
            if let Some(w) = &pac_w { o.dwFlags |= WINHTTP_AUTOPROXY_CONFIG_URL; o.lpszAutoConfigUrl = w.as_ptr(); }
            o.fAutoLogonIfChallenged = 1;
            let mut info: WINHTTP_PROXY_INFO = std::mem::zeroed();
            let u = wide(url.as_str());
            let ok = WinHttpGetProxyForUrl(session, u.as_ptr(), &mut o, &mut info) != 0;
            let err = windows_sys::Win32::Foundation::GetLastError();
            WinHttpCloseHandle(session);
            if !ok {
                // As .NET, a failed detection isn't tried again by this process.
                if detect && err == ERROR_WINHTTP_AUTODETECTION_FAILED { DETECT_FAILED.store(true, Ordering::Relaxed); }
                return None;
            }
            let (list, bypass) = (take(info.lpszProxy), take(info.lpszProxyBypass));
            if info.dwAccessType != WINHTTP_ACCESS_TYPE_NAMED_PROXY { return Some(None); }
            if bypass.as_deref().map_or(false, |b| bypassed_ie(b, url)) { return Some(None); }
            Some(list.and_then(|l| pick_ie(&l, url.scheme())))
        }
    }
}

/// Opens the tunnel to `target` ("host:port") through the proxy on `io`, as .NET's CONNECT does.
pub fn connect(io: &mut dyn ReadWriteBox, proxy: &ProxyServer, target: &str) -> std::io::Result<()> {
    let auth = proxy.auth.as_deref().map(|a| format!("Proxy-Authorization: Basic {}\r\n", crate::submit::base64(a.as_bytes()))).unwrap_or_default();
    io.write_all(format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n{}Proxy-Connection: Keep-Alive\r\n\r\n", target, target, auth).as_bytes())?;
    io.flush()?;
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if io.read(&mut b)? == 0 || head.len() > 65536 { return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, ProxyRefused("The underlying connection was closed: The connection was closed unexpectedly.".into()))); }
        head.push(b[0]);
    }
    let line = String::from_utf8_lossy(&head);
    let mut parts = line.lines().next().unwrap_or("").splitn(3, ' ').skip(1);
    let code: u16 = parts.next().and_then(|c| c.parse().ok()).unwrap_or(0);
    if code == 200 { return Ok(()); }
    let text = parts.next().unwrap_or("").trim();
    Err(std::io::Error::other(ProxyRefused(format!("The remote server returned an error: ({}) {}.", code, text))))
}

/// The tunnel's refusal, in WebException's wording.
#[derive(Debug)]
pub struct ProxyRefused(pub String);
impl std::fmt::Display for ProxyRefused { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for ProxyRefused {}

/// Read + Write, for the raw socket ureq hands over.
pub trait ReadWriteBox: Read + Write {}
impl<T: Read + Write + ?Sized> ReadWriteBox for T {}

#[cfg(test)]
mod tests {
    use super::*;
    fn u(s: &str) -> url::Url { url::Url::parse(s).unwrap() }
    fn srv(h: &str, p: u16) -> Option<ProxyServer> { Some(ProxyServer { host: h.into(), port: p, auth: None }) }
    #[test]
    fn ie_lists() {
        assert_eq!(pick_ie("proxy.corp:8080", "https"), srv("proxy.corp", 8080));
        assert_eq!(pick_ie("http=a:1;https=b:2;ftp=c:3", "https"), srv("b", 2));
        assert_eq!(pick_ie("http=a:1", "https"), None);
        assert_eq!(pick_ie("HTTP=a", "http"), srv("a", 80));
        assert_eq!(pick_ie("http://p.corp:3128/", "http"), srv("p.corp", 3128));
        assert_eq!(pick_ie("a:1; b:2", "https"), srv("a", 1));
        assert_eq!(pick_ie("socks=s:1080", "https"), None);
    }
    #[test]
    fn ie_bypass() {
        let l = "*.corp.example;<local>;10.*;https://only-https.test;port.test:8443";
        assert!(bypassed_ie(l, &u("https://a.corp.example/x")));
        assert!(!bypassed_ie(l, &u("https://corp.example/x")));
        assert!(bypassed_ie(l, &u("http://intranet/")));
        assert!(!bypassed_ie(l, &u("http://intranet.example/")));
        assert!(bypassed_ie(l, &u("http://10.1.2.3/")));
        assert!(bypassed_ie(l, &u("https://ONLY-https.test/")));
        assert!(!bypassed_ie(l, &u("http://only-https.test/")));
        assert!(bypassed_ie(l, &u("https://port.test:8443/")));
        assert!(!bypassed_ie(l, &u("https://port.test/")));
        assert!(!bypassed_ie("", &u("https://submit.hails.app/")));
    }
    #[test]
    fn loopback_is_direct() {
        assert!(is_loopback(&u("http://127.0.0.1:5/")) && is_loopback(&u("http://[::1]/")) && is_loopback(&u("https://LOCALHOST/")));
        assert!(!is_loopback(&u("https://submit.hails.app/")));
    }
    #[test]
    fn env_vars() {
        let vars = |m: &'static [(&'static str, &'static str)]| move |n: &str| m.iter().find(|(k, _)| *k == n).map(|(_, v)| v.to_string());
        let e = vars(&[("https_proxy", "http://user:pw@p.test:3128"), ("HTTP_PROXY", "http://upper:1"), ("no_proxy", ".skip.test,exact.test:8443")]);
        assert_eq!(from_env(&u("https://x.test/"), &e), Some(ProxyServer { host: "p.test".into(), port: 3128, auth: Some("user:pw".into()) }));
        assert_eq!(from_env(&u("http://x.test/"), &e), None);
        assert_eq!(from_env(&u("https://a.skip.test/"), &e), None);
        assert_eq!(from_env(&u("https://skip.test/"), &e), None);
        assert!(from_env(&u("https://exact.test/"), &e).is_some());
        assert_eq!(from_env(&u("https://exact.test:8443/"), &e), None);
        let a = vars(&[("ALL_PROXY", "a.test:8080"), ("NO_PROXY", "*")]);
        assert_eq!(from_env(&u("https://x.test/"), &a), None);
        let a = vars(&[("ALL_PROXY", "a.test:8080"), ("http_proxy", "socks5://s:1")]);
        assert_eq!(from_env(&u("https://x.test/"), &a), srv("a.test", 8080));
        assert_eq!(from_env(&u("http://x.test/"), &a), None);
    }
}
