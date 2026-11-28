//! LinkPattern: web addresses in a body.
use regex::Regex;
use std::sync::OnceLock;

/// (byte offset, byte length) of each address, as the C# regex finds them.
pub fn find(text: &str) -> Vec<(usize, usize)> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"(?:https?://)?(?:[a-z0-9-]+\.)+(?:com|app|co|org|io|net|cc)(?:/[A-Za-z0-9._~%/-]*[A-Za-z0-9_/-])?").unwrap());
    r.find_iter(text).map(|m| (m.start(), m.len())).collect()
}
