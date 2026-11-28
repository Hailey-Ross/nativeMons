//! Infinite Fusion's Pokedex, from its saves and Data\species.dat, through a read-only Ruby Marshal 4.8 reader
//! (RPG Maker's save format). Ported from the C#'s RubyMarshal and InfiniteFusionDex; anything unknown stops it.

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Sym(String),
    Bytes(Vec<u8>),
    List(Rc<Vec<V>>),
    Hash(Rc<Vec<(V, V)>>),
    Obj(Rc<(String, Vec<(String, V)>)>),
}
impl V {
    /// An object's instance variable (RObj[ivar]).
    pub fn ivar(&self, name: &str) -> Option<&V> { match self { V::Obj(o) => o.1.iter().rev().find(|x| x.0 == name).map(|x| &x.1), _ => None } }
    pub fn get(&self, key: &V) -> Option<&V> { match self { V::Hash(h) => h.iter().rev().find(|x| &x.0 == key).map(|x| &x.1), _ => None } }
}

struct Reader<'a> { b: &'a [u8], p: usize, syms: Vec<String>, objs: Vec<V> }

pub fn load(data: &[u8]) -> Result<V, String> {
    if data.len() < 2 || data[0] != 4 || data[1] != 8 { return Err("Not Ruby Marshal 4.8 data.".into()); }
    Reader { b: data, p: 2, syms: Vec::new(), objs: Vec::new() }.read()
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, String> { let v = *self.b.get(self.p).ok_or("Marshal data cut short.")?; self.p += 1; Ok(v) }
    fn long(&mut self) -> Result<i64, String> {
        let c = self.byte()? as i8 as i32;
        if c == 0 { return Ok(0); }
        if c > 4 { return Ok((c - 5) as i64); }
        if c < -4 { return Ok((c + 5) as i64); }
        let mut v: i64 = 0;
        if c > 0 { for i in 0..c { v |= (self.byte()? as i64) << (8 * i); } return Ok(v); }
        v = -1;
        for i in 0..-c { v &= !(0xFFi64 << (8 * i)); v |= (self.byte()? as i64) << (8 * i); }
        Ok(v)
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.long()?;
        if n < 0 || self.p as i64 + n > self.b.len() as i64 { return Err("Marshal data cut short.".into()); }
        let v = self.b[self.p..self.p + n as usize].to_vec();
        self.p += n as usize;
        Ok(v)
    }
    fn text(&mut self) -> Result<String, String> { Ok(String::from_utf8_lossy(&self.bytes()?).into_owned()) }
    fn sym(&mut self) -> Result<String, String> {
        match self.byte()? {
            b':' => { let s = self.text()?; self.syms.push(s.clone()); Ok(s) }
            b';' => { let i = self.long()?; self.syms.get(i as usize).cloned().ok_or_else(|| "Marshal: a symbol was expected.".into()) }
            b'I' => { let s = self.sym()?; let n = self.long()?; for _ in 0..n { self.sym()?; self.read()?; } Ok(s) }
            _ => Err("Marshal: a symbol was expected.".into()),
        }
    }
    /// A slot in the object table, taken before the children are read (their links count after it).
    fn slot(&mut self) -> usize { self.objs.push(V::Nil); self.objs.len() - 1 }
    fn read(&mut self) -> Result<V, String> {
        let t = self.byte()?;
        Ok(match t {
            b'0' => V::Nil,
            b'T' => V::Bool(true),
            b'F' => V::Bool(false),
            b'i' => V::Int(self.long()?),
            b':' | b';' => { self.p -= 1; V::Sym(self.sym()?) }
            b'@' => { let i = self.long()?; self.objs.get(i as usize).cloned().ok_or("Marshal: a link to nothing.")? }
            b'I' => { let v = self.read()?; let n = self.long()?; for _ in 0..n { self.sym()?; self.read()?; } v }
            b'"' => { let v = V::Str(self.text()?); self.objs.push(v.clone()); v }
            b'f' => {
                let txt = String::from_utf8_lossy(&self.bytes()?).into_owned();
                // .NET's double.TryParse takes neither "inf" nor "nan": 0 there.
                let v = V::Float(if txt.chars().any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E') { 0.0 } else { txt.parse().unwrap_or(0.0) });
                self.objs.push(v.clone());
                v
            }
            b'l' => { self.byte()?; let n = self.long()? * 2; self.p = (self.p as i64 + n).max(0) as usize; self.objs.push(V::Int(0)); V::Int(0) }
            b'[' => {
                let at = self.slot();
                let n = self.long()?;
                let mut a = Vec::new();
                for _ in 0..n { a.push(self.read()?); }
                let v = V::List(Rc::new(a));
                self.objs[at] = v.clone();
                v
            }
            b'{' | b'}' => {
                let at = self.slot();
                let n = self.long()?;
                let mut h: Vec<(V, V)> = Vec::new();
                for _ in 0..n {
                    let k = self.read()?;
                    let v = self.read()?;
                    if k == V::Nil { continue; }
                    match h.iter_mut().find(|x| x.0 == k) { Some(x) => x.1 = v, None => h.push((k, v)) }
                }
                if t == b'}' { self.read()?; }
                let v = V::Hash(Rc::new(h));
                self.objs[at] = v.clone();
                v
            }
            b'o' | b'S' => {
                let class = self.sym()?;
                let at = self.slot();
                let n = self.long()?;
                let mut iv: Vec<(String, V)> = Vec::new();
                for _ in 0..n { let k = self.sym()?; let v = self.read()?; match iv.iter_mut().find(|x| x.0 == k) { Some(x) => x.1 = v, None => iv.push((k, v)) } }
                let v = V::Obj(Rc::new((class, iv)));
                self.objs[at] = v.clone();
                v
            }
            b'u' => { let class = self.sym()?; let d = self.bytes()?; let v = V::Obj(Rc::new((class, vec![("_dump".into(), V::Bytes(d))]))); self.objs.push(v.clone()); v }
            b'U' => {
                let class = self.sym()?;
                let at = self.slot();
                let d = self.read()?;
                let v = V::Obj(Rc::new((class, vec![("_data".into(), d)])));
                self.objs[at] = v.clone();
                v
            }
            b'e' | b'C' => { self.sym()?; self.read()? }
            b'c' | b'm' => { let v = V::Str(self.text()?); self.objs.push(v.clone()); v }
            b'/' => { let d = self.bytes()?; self.byte()?; let v = V::Bytes(d); self.objs.push(v.clone()); v }
            _ => return Err(format!("Marshal: unknown kind '{}' at byte {}.", t as char, self.p - 1)),
        })
    }
}

const KEY: [u8; 16] = [0x4A, 0x8F, 0x2C, 0xE1, 0x73, 0xB5, 0x96, 0x0D, 0x5E, 0xA2, 0x3F, 0xC7, 0x81, 0x14, 0x6B, 0xD9];

/// Plain: lower case, the gender signs as f/m, accents dropped (Unicode's canonical decomposition, for the Latin
/// letters the names use), only ASCII letters and digits kept.
pub fn plain(name: &str) -> String {
    const FROM: &str = "àáâãäåāăąçćĉċčďèéêëēĕėęěĝğġģĥìíîïĩīĭįĵķĺļľñńņňòóôõöōŏőŕŗřśŝşšţťùúûüũūŭůűųŵýÿŷźżž";
    const TO: &str = "aaaaaaaaacccccdeeeeeeeeegggghiiiiiiiijklllnnnnooooooooorrrssssttuuuuuuuuuuwyyyzzz";
    let mut o = String::new();
    for c in name.to_lowercase().replace('\u{2640}', "f").replace('\u{2642}', "m").chars() {
        let c = match FROM.chars().position(|x| x == c) { Some(i) => TO.as_bytes()[i] as char, None => c };
        if c.is_ascii_alphanumeric() { o.push(c); }
    }
    o
}

static SPECIES: std::sync::Mutex<Option<(String, HashMap<i32, String>)>> = std::sync::Mutex::new(None);

/// Its species list (Data\species.dat, XOR-masked unless it's plain Marshal), kept until the file changes.
fn species(game_dir: &std::path::Path) -> Result<HashMap<i32, String>, String> {
    let file = game_dir.join("Data").join("species.dat");
    let m = std::fs::metadata(&file).map_err(|e| e.to_string())?;
    let print = format!("{}|{:?}", file.display(), m.modified().ok());
    let mut kept = SPECIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, d)) = kept.as_ref() { if *p == print { return Ok(d.clone()); } }
    let mut raw = std::fs::read(&file).map_err(|e| e.to_string())?;
    if !(raw.len() > 1 && raw[0] == 4 && raw[1] == 8) { for (i, b) in raw.iter_mut().enumerate() { *b ^= KEY[i % KEY.len()]; } }
    let all = load(&raw)?;
    let V::Hash(h) = all else { return Err("Its species list isn't in the form expected.".into()) };
    let mut d = HashMap::new();
    for (k, v) in h.iter() {
        let (V::Int(k), V::Obj(_)) = (k, v) else { continue };
        let n = match v.ivar("real_name") { Some(V::Str(s)) => Some(s), _ => match v.ivar("@real_name") { Some(V::Str(s)) => Some(s), _ => None } };
        if let Some(n) = n { d.insert(*k as i32, n.clone()); }
    }
    *kept = Some((print, d.clone()));
    Ok(d)
}

fn count(table: Option<&V>) -> i32 {
    let Some(V::List(rows)) = table else { return 0 };
    rows.iter().map(|r| match r { V::List(r) => r.iter().filter(|x| **x == V::Bool(true)).count() as i32, _ => 0 }).sum()
}

/// What IfDex reads: caught and seen by National Dex number, fusions, and later Pokemon by their own numbers.
#[derive(Default, Debug, PartialEq)]
pub struct IfRead { pub caught: BTreeSet<i32>, pub seen: BTreeSet<i32>, pub fusions_caught: i32, pub fusions_seen: i32, pub later_caught: BTreeSet<i32>, pub later_seen: BTreeSet<i32> }

/// Every save of the game, read together. national: a Pokemon's plain name to its National Dex number.
pub fn read(game_dir: &std::path::Path, saves: &[std::path::PathBuf], national: &HashMap<String, i32>) -> Result<IfRead, String> {
    let mut r = IfRead::default();
    let species = species(game_dir)?;
    for file in saves {
        let save = load(&std::fs::read(file).map_err(|e| e.to_string())?)?;
        let Some(pl) = save.get(&V::Sym("player".into())) else { continue };
        let Some(dex @ V::Obj(_)) = pl.ivar("@pokedex") else { continue };
        for key in ["@owned_standard", "@seen_standard"] {
            let Some(V::List(list)) = dex.ivar(key) else { continue };
            let owned = key == "@owned_standard";
            for i in 1..list.len() {
                if list[i] != V::Bool(true) { continue; }
                let nat = if i <= 251 { Some(i as i32) } else { species.get(&(i as i32)).and_then(|n| national.get(&plain(n)).copied()) };
                match nat {
                    Some(n) => { if owned { r.caught.insert(n); } else { r.seen.insert(n); } }
                    // A later Pokemon than the collection lists (Oricorio, Carbink): kept by its own number.
                    None => { if owned { r.later_caught.insert(i as i32); } else { r.later_seen.insert(i as i32); } }
                }
            }
        }
        r.fusions_caught = r.fusions_caught.max(count(dex.ivar("@owned_fusion")));
        r.fusions_seen = r.fusions_seen.max(count(dex.ivar("@seen_fusion")));
    }
    let c = r.caught.clone(); r.seen.extend(c);
    let c = r.later_caught.clone(); r.later_seen.extend(c);
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Marshal bytes written by hand: Ruby's own encodings of these values.
    fn sym(s: &str) -> Vec<u8> { let mut v = vec![b':', (s.len() + 5) as u8]; v.extend(s.bytes()); v }
    #[test]
    fn longs_and_links() {
        assert_eq!(load(&[4, 8, b'i', 0]).unwrap(), V::Int(0));
        assert_eq!(load(&[4, 8, b'i', 6]).unwrap(), V::Int(1));
        assert_eq!(load(&[4, 8, b'i', 0xFA]).unwrap(), V::Int(-1));
        assert_eq!(load(&[4, 8, b'i', 1, 200]).unwrap(), V::Int(200));
        assert_eq!(load(&[4, 8, b'i', 0xFF, 0x38]).unwrap(), V::Int(-200));
        assert_eq!(load(&[4, 8, b'i', 2, 0xE8, 0x03]).unwrap(), V::Int(1000));
        // ["a", @1]: the string is object 1 (the array is 0).
        let v = load(&[4, 8, b'[', 7, b'I', b'"', 6, b'a', 6, b':', 6, b'E', b'T', b'@', 6]).unwrap();
        assert_eq!(v, V::List(Rc::new(vec![V::Str("a".into()), V::Str("a".into())])));
        assert!(load(&[4, 8, b'[', 7]).is_err());
        assert!(load(&[4, 9]).is_err());
    }
    #[test]
    fn plain_names() {
        assert_eq!(plain("Nidoran\u{2640}"), "nidoranf");
        assert_eq!(plain("Flab\u{e9}b\u{e9}"), "flabebe");
        assert_eq!(plain("Mr. Mime"), "mrmime");
        assert_eq!(plain("Farfetch\u{2019}d"), "farfetchd");
        assert_eq!(plain("Type: Null"), "typenull");
    }
    #[test]
    fn reads_a_save() {
        let dir = std::env::temp_dir().join(format!("nm-ifdex-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Data")).unwrap();
        // species.dat: {252 => Species(@real_name "Oricorio"), 253 => Species(@real_name "Treecko")}, XOR-masked.
        let mut sp = vec![4u8, 8, b'{', 7, b'i', 1, 252];
        sp.extend([b'o']); sp.extend(sym("Species")); sp.extend([6]); sp.extend(sym("@real_name")); sp.extend([b'"', 13]); sp.extend(b"Oricorio");
        sp.extend([b'i', 1, 253, b'o', b';', 0, 6, b';', 6, b'"', 12]); sp.extend(b"Treecko");
        let masked: Vec<u8> = sp.iter().enumerate().map(|(i, b)| b ^ KEY[i % 16]).collect();
        std::fs::write(dir.join("Data").join("species.dat"), masked).unwrap();
        // A save: {:player => Player(@pokedex => Pokedex(owned [nil,T,F,..,T at 252,T at 253], seen, fusions))}.
        let mut owned = vec![b'[', 2, 254, 0, b'0'];
        for i in 1..254 { owned.push(if i == 1 || i == 252 || i == 253 { b'T' } else { b'F' }); }
        let mut s = vec![4u8, 8, b'{', 6];
        s.extend(sym("player")); s.push(b'o'); s.extend(sym("Player")); s.push(6); s.extend(sym("@pokedex"));
        s.push(b'o'); s.extend(sym("Pokedex")); s.push(9);
        s.extend(sym("@owned_standard")); s.extend(&owned);
        s.extend(sym("@seen_standard")); s.extend([b'[', 8, b'0', b'F', b'T']);
        s.extend(sym("@owned_fusion")); s.extend([b'[', 7, b'[', 7, b'T', b'F', b'[', 6, b'T']);
        s.extend(sym("@seen_fusion")); s.extend([b'[', 6, b'[', 8, b'T', b'T', b'T']);
        std::fs::write(dir.join("a.rxdata"), &s).unwrap();
        let national: HashMap<String, i32> = [("treecko".to_string(), 252)].into_iter().collect();
        let r = read(&dir, &[dir.join("a.rxdata")], &national).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(r.caught.iter().copied().collect::<Vec<_>>(), vec![1, 252]);
        assert_eq!(r.seen.iter().copied().collect::<Vec<_>>(), vec![1, 2, 252]);
        assert_eq!(r.later_caught.iter().copied().collect::<Vec<_>>(), vec![252]);
        assert_eq!((r.fusions_caught, r.fusions_seen), (2, 3));
    }
}
