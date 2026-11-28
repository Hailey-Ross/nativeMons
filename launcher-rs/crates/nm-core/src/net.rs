//! WebClient as the launcher uses it: a GET with the "nativeMons" user agent, https or file:// (as WebClient takes).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

fn file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file:///").or_else(|| url.strip_prefix("file://"))?;
    let p = rest.replace("%20", " ");
    Some(if cfg!(windows) { PathBuf::from(p.replace('/', "\\")) } else { PathBuf::from(format!("/{}", p.trim_start_matches('/'))) })
}

/// How a request to one URL travels: through which proxy, checked against which trust store.
#[derive(Clone)]
pub struct Route { pub proxy: Option<crate::proxy::ProxyServer>, pub tls: Option<Arc<ureq::rustls::ClientConfig>> }

/// The system's route, as .NET's: Internet Settings' proxy and Windows' certificate store (Linux: env, system roots).
pub fn route(url: &url::Url) -> Route { Route { proxy: crate::proxy::for_url(url), tls: system_tls() } }

/// rustls checked by the OS verifier (on Windows the CryptoAPI chain engine, so its store and policies, as SChannel).
pub fn system_tls() -> Option<Arc<ureq::rustls::ClientConfig>> {
    static TLS: OnceLock<Option<Arc<ureq::rustls::ClientConfig>>> = OnceLock::new();
    TLS.get_or_init(|| tls_with(Vec::new())).clone()
}
/// The same with extra roots (tests: a local CA without touching the OS store).
pub fn tls_with(extra: Vec<ureq::rustls::pki_types::CertificateDer<'static>>) -> Option<Arc<ureq::rustls::ClientConfig>> {
    use ureq::rustls;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = if extra.is_empty() { rustls_platform_verifier::Verifier::new(provider.clone()) } else { rustls_platform_verifier::Verifier::new_with_extra_roots(extra, provider.clone()) }.ok()?;
    let c = rustls::ClientConfig::builder_with_provider(provider).with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13]).ok()?
        .dangerous().with_custom_certificate_verifier(Arc::new(verifier)).with_no_client_auth();
    Some(Arc::new(c))
}

/// An agent for one hop to `url` on `route`: https tunnels through the proxy with CONNECT, http uses it as a proxy.
pub fn agent(url: &url::Url, route: &Route, b: ureq::AgentBuilder) -> ureq::Agent {
    let b = b.user_agent("nativeMons");
    let port = url.port_or_known_default().unwrap_or(443);
    match (&route.proxy, url.scheme()) {
        (Some(p), "https") => b.resolver(ToProxy(format!("{}:{}", p.host, p.port))).tls_connector(Arc::new(Tunnel {
            proxy: p.clone(), target: format!("{}:{}", url.host_str().unwrap_or(""), port), tls: route.tls.clone(),
        })).build(),
        (Some(p), _) => match ureq::Proxy::new(format!("http://{}{}:{}", p.auth.as_deref().map(|a| format!("{}@", a)).unwrap_or_default(), p.host, p.port)) {
            Ok(x) => b.proxy(x).build(),
            Err(_) => b.build(),
        },
        (None, _) => match &route.tls { Some(t) => b.tls_config(t.clone()).build(), None => b.build() },
    }
}

/// Every name resolves to the proxy: the tunnel's socket goes there.
struct ToProxy(String);
impl ureq::Resolver for ToProxy {
    fn resolve(&self, _: &str) -> std::io::Result<Vec<std::net::SocketAddr>> { std::net::ToSocketAddrs::to_socket_addrs(&self.0).map(|a| a.collect()) }
}
/// CONNECT on the proxy's socket, then TLS to the target inside, so requests stay as they are direct.
struct Tunnel { proxy: crate::proxy::ProxyServer, target: String, tls: Option<Arc<ureq::rustls::ClientConfig>> }
impl ureq::TlsConnector for Tunnel {
    fn connect(&self, dns_name: &str, mut io: Box<dyn ureq::ReadWrite>) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
        crate::proxy::connect(&mut io, &self.proxy, &self.target)?;
        match &self.tls { Some(t) => ureq::TlsConnector::connect(t, dns_name, io), None => Err(std::io::Error::other("no TLS configuration").into()) }
    }
}

/// The refusal of a proxy tunnel, when that's why a request failed.
pub fn proxy_refusal(t: &ureq::Transport) -> Option<String> {
    let io = std::error::Error::source(t)?.downcast_ref::<std::io::Error>()?;
    io.get_ref()?.downcast_ref::<crate::proxy::ProxyRefused>().map(|r| r.0.clone())
}

fn get(url: &str, accept: Option<&str>) -> Result<Box<dyn Read + Send + Sync>, String> { get_via(url, accept, &route) }
/// GET as WebClient: redirects followed (up to 50), each hop on its own route.
fn get_via(url: &str, accept: Option<&str>, routes: &dyn Fn(&url::Url) -> Route) -> Result<Box<dyn Read + Send + Sync>, String> {
    if let Some(p) = file_url(url) { return std::fs::File::open(&p).map(|f| Box::new(f) as Box<dyn Read + Send + Sync>).map_err(|e| e.to_string()); }
    let mut at = url::Url::parse(url).map_err(|e| e.to_string())?;
    for _ in 0..=50 {
        let a = agent(&at, &routes(&at), ureq::AgentBuilder::new().redirects(0));
        let mut req = a.request_url("GET", &at).set("User-Agent", "nativeMons");
        if let Some(a) = accept { req = req.set("Accept", a); }
        match req.call() {
            Ok(r) if [301, 302, 303, 307, 308].contains(&r.status()) && r.header("Location").is_some() => {
                at = at.join(r.header("Location").unwrap_or("")).map_err(|e| e.to_string())?;
            }
            Ok(r) => return Ok(r.into_reader()),
            Err(ureq::Error::Status(code, r)) => return Err(format!("The remote server returned an error: ({}) {}.", code, r.status_text())),
            Err(ureq::Error::Transport(t)) => return Err(proxy_refusal(&t).unwrap_or_else(|| ureq::Error::Transport(t).to_string())),
        }
    }
    Err("Too many automatic redirections were attempted.".into())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use ureq::rustls;

    /// A test CA and a leaf for submit.test and 127.0.0.1 it signs (never put in any OS store).
    fn pki() -> (rustls::pki_types::CertificateDer<'static>, rustls::pki_types::CertificateDer<'static>, Vec<u8>) {
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let mut ca = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca.distinguished_name.push(rcgen::DnType::CommonName, "nativeMons test CA");
        ca.key_usages = vec![rcgen::KeyUsagePurpose::KeyCertSign, rcgen::KeyUsagePurpose::CrlSign, rcgen::KeyUsagePurpose::DigitalSignature];
        ca.not_before = rcgen::date_time_ymd(2025, 1, 1);
        ca.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let ca = ca.self_signed(&ca_key).unwrap();
        let key = rcgen::KeyPair::generate().unwrap();
        let mut leaf = rcgen::CertificateParams::new(vec!["submit.test".to_string(), "127.0.0.1".to_string()]).unwrap();
        leaf.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        leaf.not_before = rcgen::date_time_ymd(2025, 1, 1);
        leaf.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let leaf = leaf.signed_by(&key, &ca, &ca_key).unwrap();
        (ca.der().clone(), leaf.der().clone(), key.serialize_der())
    }

    /// An HTTPS fake on 127.0.0.1: each connection gets the next canned reply; the raw requests come back.
    fn https_serve(leaf: rustls::pki_types::CertificateDer<'static>, key: Vec<u8>, replies: Vec<String>) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = Arc::new(rustls::ServerConfig::builder_with_provider(provider).with_safe_default_protocol_versions().unwrap().with_no_client_auth()
            .with_single_cert(vec![leaf], rustls::pki_types::PrivateKeyDer::Pkcs8(key.into())).unwrap());
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || replies.into_iter().map(|reply| {
            let (c, _) = l.accept().unwrap();
            let mut s = rustls::StreamOwned::new(rustls::ServerConnection::new(cfg.clone()).unwrap(), c);
            let mut raw = Vec::new();
            let mut b = [0u8; 4096];
            loop {
                let n = match s.read(&mut b) { Ok(n) => n, Err(_) => break };
                raw.extend_from_slice(&b[..n]);
                let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else { if n == 0 { break } else { continue } };
                let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                let len: usize = head.lines().find_map(|x| x.strip_prefix("content-length: ")).map_or(0, |v| v.trim().parse().unwrap());
                if raw.len() >= end + 4 + len || n == 0 { break; }
            }
            let _ = s.write_all(reply.as_bytes());
            s.conn.send_close_notify();
            let _ = s.flush();
            String::from_utf8_lossy(&raw).into_owned()
        }).collect());
        (port, h)
    }

    /// A CONNECT proxy on 127.0.0.1 tunneling every target to `to` (or answering `refuse`); the CONNECT heads come back.
    fn proxy(to: u16, conns: usize, refuse: Option<&'static str>) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || (0..conns).map(|_| {
            let (mut c, _) = l.accept().unwrap();
            let mut head = Vec::new();
            let mut b = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") { if c.read(&mut b).unwrap() == 0 { break; } head.push(b[0]); }
            if let Some(r) = refuse { c.write_all(r.as_bytes()).unwrap(); return String::from_utf8_lossy(&head).into_owned(); }
            let up = TcpStream::connect(("127.0.0.1", to)).unwrap();
            c.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").unwrap();
            let (mut a, mut b2) = (c.try_clone().unwrap(), up.try_clone().unwrap());
            let t = std::thread::spawn(move || { let _ = std::io::copy(&mut a, &mut b2); let _ = b2.shutdown(std::net::Shutdown::Write); });
            let (mut up2, mut c2) = (up, c);
            let _ = std::io::copy(&mut up2, &mut c2);
            let _ = c2.shutdown(std::net::Shutdown::Both);
            let _ = t.join();
            String::from_utf8_lossy(&head).into_owned()
        }).collect());
        (port, h)
    }
    fn reply(status: &str, extra: &str, body: &str) -> String { format!("HTTP/1.1 {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}", status, extra, body.len(), body) }
    fn via(p: u16, tls: Option<Arc<rustls::ClientConfig>>) -> Route { Route { proxy: Some(crate::proxy::ProxyServer { host: "127.0.0.1".into(), port: p, auth: Some("u:p".into()) }), tls } }

    #[test]
    fn https_post_through_a_proxy_on_the_os_verifier() {
        let (ca, leaf, key) = pki();
        let (sport, server) = https_serve(leaf, key, vec![reply("200 OK", "", "{\"receipt\":\"r\"}")]);
        let (pport, prox) = proxy(sport, 1, None);
        let route = via(pport, tls_with(vec![ca]));
        let base = format!("https://submit.test:{}", sport);
        let got = crate::submit::post_on(&base, "/v1/challenge", "application/json", b"{\"key\":\"BKEY\"}", None, &|_| route.clone());
        assert_eq!(got.as_deref(), Ok("{\"receipt\":\"r\"}"));
        let connect = &prox.join().unwrap()[0];
        assert_eq!(connect, &format!("CONNECT submit.test:{0} HTTP/1.1\r\nHost: submit.test:{0}\r\nProxy-Authorization: Basic dTpw\r\nProxy-Connection: Keep-Alive\r\n\r\n", sport));
        // Inside the tunnel the request is as it is direct: origin-form, the same headers and body.
        let req = &server.join().unwrap()[0];
        assert!(req.starts_with("POST /v1/challenge HTTP/1.1\r\n"), "{}", req);
        assert!(req.contains(&format!("\r\nHost: submit.test:{}\r\n", sport)) && req.contains("\r\nUser-Agent: nativeMons\r\n") && req.contains("\r\nContent-Type: application/json\r\n"));
        assert!(req.ends_with("\r\n\r\n{\"key\":\"BKEY\"}") && !req.to_ascii_lowercase().contains("proxy-"));
    }
    #[test]
    fn get_follows_redirects_each_hop_routed() {
        let (ca, leaf, key) = pki();
        let (sport, server) = https_serve(leaf, key, vec![reply("302 Found", "Location: /real.json\r\n", ""), reply("200 OK", "", "[1]")]);
        let (pport, prox) = proxy(sport, 2, None);
        let route = via(pport, tls_with(vec![ca.clone()]));
        let mut b = String::new();
        get_via(&format!("https://submit.test:{}/index.json", sport), Some("application/json"), &|_| route.clone()).unwrap().read_to_string(&mut b).unwrap();
        assert_eq!(b, "[1]");
        assert_eq!(prox.join().unwrap().len(), 2);
        let reqs = server.join().unwrap();
        assert!(reqs[0].starts_with("GET /index.json HTTP/1.1\r\n") && reqs[1].starts_with("GET /real.json HTTP/1.1\r\n"));
        assert!(reqs.iter().all(|r| r.contains("\r\nUser-Agent: nativeMons\r\n") && r.contains("\r\nAccept: application/json\r\n")));
        // Direct on the same verifier: trusted with its CA, refused with another.
        let (ca2, leaf, key) = pki();
        let (sport, server) = https_serve(leaf, key, vec![reply("200 OK", "", "ok"), String::new()]);
        let direct = |tls| Route { proxy: None, tls };
        let mut b = String::new();
        get_via(&format!("https://127.0.0.1:{}/", sport), None, &|_| direct(tls_with(vec![ca2.clone()]))).unwrap().read_to_string(&mut b).unwrap();
        assert_eq!(b, "ok");
        let e = get_via(&format!("https://127.0.0.1:{}/", sport), None, &|_| direct(tls_with(vec![ca.clone()]))).err().unwrap();
        assert!(e.to_lowercase().contains("certificate") || e.to_lowercase().contains("issuer"), "{}", e);
        let _ = server.join();
    }
    #[test]
    fn proxy_refusals_as_webexception() {
        let (pport, prox) = proxy(0, 1, Some("HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n"));
        let route = via(pport, system_tls());
        let got = crate::submit::post_on("https://submit.test", "/v1/challenge", "application/json", b"{}", None, &|_| route.clone());
        assert_eq!(got, Err("The remote server returned an error: (407) Proxy Authentication Required.".into()));
        assert!(prox.join().unwrap()[0].starts_with("CONNECT submit.test:443 HTTP/1.1\r\n"));
        // Plain http uses the proxy as a proxy (absolute-form, as .NET).
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || { let (mut c, _) = l.accept().unwrap(); let mut b = [0u8; 2048]; let n = c.read(&mut b).unwrap(); c.write_all(reply("200 OK", "", "hi").as_bytes()).unwrap(); String::from_utf8_lossy(&b[..n]).into_owned() });
        let mut b = String::new();
        get_via("http://plain.test/a?b=1", None, &|_| via(p, None)).unwrap().read_to_string(&mut b).unwrap();
        assert_eq!(b, "hi");
        assert!(h.join().unwrap().starts_with("GET http://plain.test/a?b=1 HTTP/1.1\r\n"));
    }
    #[test]
    fn system_tls_builds() { assert!(system_tls().is_some()); }
}
