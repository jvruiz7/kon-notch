//! Letras sincronizadas (lrclib.net, sem chave), como no Kon atual: procura pela faixa, guarda em cache no disco.

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct Lyrics { pub key: String, pub loading: bool, pub synced: bool, pub lines: Vec<(f64, String)> }

#[derive(Default)]
pub struct State { pub cur: Lyrics, cache: HashMap<String, Option<(bool, Vec<(f64, String)>)>>, loaded: bool, covers: HashMap<String, Option<Arc<Vec<u8>>>> }
pub type Shared = Arc<Mutex<State>>;

fn file() -> std::path::PathBuf { crate::platform::data_dir().join("lyrics-cache.json") }

/// Limpa "(Official Video)", "- Topic", "feat." etc. e separa "Artista - Música" quando vem tudo no título.
pub fn clean(title: &str, artist: &str) -> (String, String) {
    let junk = regex_lite::Regex::new(r"(?i)\s*(?:\(|\[)(?:official|video|lyrics?|lyric video|audio|hd|4k|visualizer|music video|clipe oficial|ao vivo|live)[^)\x5D]*(?:\)|\])\s*").unwrap();
    let mut t = junk.replace_all(title, " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let mut a = artist.trim().trim_end_matches(" - Topic").trim_end_matches("VEVO").trim().to_string();
    if (a.is_empty() || a.to_lowercase().contains("youtube") || a.to_lowercase().contains("chrome")) && t.contains(" - ") {
        let mut it = t.splitn(2, " - "); a = it.next().unwrap_or("").trim().into(); t = it.next().unwrap_or("").trim().into();
    }
    let feat = regex_lite::Regex::new(r"(?i)\s*(?:\(|\[)?(?:feat\.?|ft\.?)\s[^)\x5D]*(?:\)|\])?").unwrap();
    (feat.replace(&t, "").trim().to_string(), a)
}

pub fn parse_lrc(text: &str) -> Vec<(f64, String)> {
    let tag = regex_lite::Regex::new(r"\[(\d+):(\d+(?:\.\d+)?)\]").unwrap();
    let strip = regex_lite::Regex::new(r"\[[^\x5D]*\]").unwrap();
    let mut out = vec![];
    for line in text.lines() {
        let words = strip.replace_all(line, "").trim().to_string();
        for c in tag.captures_iter(line) {
            let t = c[1].parse::<f64>().unwrap_or(0.0) * 60.0 + c[2].parse::<f64>().unwrap_or(0.0);
            out.push((t, if words.is_empty() { "♪".into() } else { words.clone() }));
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

pub fn new() -> Shared { Arc::new(Mutex::new(State::default())) }

pub fn want(st: &Shared, title: &str, artist: &str, album: &str, duration: f64, repaint: impl Fn() + Send + 'static) {
    if title.is_empty() { return; }
    let (t, a) = clean(title, artist);
    let key = format!("{a}|{t}").to_lowercase();
    let mut g = st.lock().unwrap();
    if !g.loaded { g.loaded = true; if let Some(c) = std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()) { g.cache = c; } }
    if g.cur.key == key { return; }
    if let Some(hit) = g.cache.get(&key).cloned() {
        g.cur = match hit { Some((synced, lines)) => Lyrics { key, loading: false, synced, lines }, None => Lyrics { key, ..Default::default() } };
        return;
    }
    g.cur = Lyrics { key: key.clone(), loading: true, ..Default::default() };
    drop(g);
    let st = st.clone(); let album = album.to_string();
    std::thread::spawn(move || {
        let res = fetch(&t, &a, &album, duration);
        let mut g = st.lock().unwrap();
        g.cache.insert(key.clone(), res.clone());
        while g.cache.len() > 400 { let k = g.cache.keys().next().cloned().unwrap(); g.cache.remove(&k); }
        let _ = std::fs::write(file(), serde_json::to_string(&g.cache).unwrap_or_default());
        if g.cur.key == key { g.cur = match res { Some((synced, lines)) => Lyrics { key, loading: false, synced, lines }, None => Lyrics { key, ..Default::default() } }; }
        drop(g); repaint();
    });
}

fn fetch(t: &str, a: &str, album: &str, duration: f64) -> Option<(bool, Vec<(f64, String)>)> {
    let mut req = ureq::get("https://lrclib.net/api/get").query("track_name", t).query("artist_name", a).timeout(std::time::Duration::from_secs(10));
    if !album.is_empty() { req = req.query("album_name", album); }
    if duration > 0.0 { req = req.query("duration", &format!("{}", duration.round())); }
    let mut r: Option<serde_json::Value> = req.call().ok().and_then(|x| x.into_json().ok());
    let has = |v: &serde_json::Value| v["syncedLyrics"].is_string() || v["plainLyrics"].is_string();
    if !r.as_ref().map_or(false, has) {
        let q = if a.is_empty() { ureq::get("https://lrclib.net/api/search").query("q", t) } else { ureq::get("https://lrclib.net/api/search").query("track_name", t).query("artist_name", a) };
        let arr: Vec<serde_json::Value> = q.timeout(std::time::Duration::from_secs(10)).call().ok().and_then(|x| x.into_json().ok()).unwrap_or_default();
        r = arr.iter().find(|x| x["syncedLyrics"].is_string()).or_else(|| arr.iter().find(|x| x["plainLyrics"].is_string())).cloned();
    }
    let r = r?;
    if let Some(s) = r["syncedLyrics"].as_str() { let l = parse_lrc(s); if !l.is_empty() { return Some((true, l)); } }
    r["plainLyrics"].as_str().map(|s| (false, s.lines().filter(|l| !l.trim().is_empty()).enumerate().map(|(i, l)| (i as f64, l.to_string())).collect()))
}

/// Índice da linha atual (a última cujo tempo já passou).
pub fn at(lines: &[(f64, String)], pos: f64) -> usize { lines.iter().rposition(|l| l.0 <= pos + 0.25).unwrap_or(0) }

#[cfg(test)]
mod t {
    #[test] fn lrc_e_limpeza() {
        let l = super::parse_lrc("[00:01.50]Oi\n[00:03.00][00:10.00]Refrão\n[00:05.00]");
        assert_eq!(l.len(), 4); assert_eq!(l[0], (1.5, "Oi".into())); assert_eq!(l[2].1, "♪");
        assert_eq!(super::clean("Artista X - Música (Official Video)", "Canal YouTube"), ("Música".into(), "Artista X".into()));
        assert_eq!(super::at(&l, 4.0), 1);
    }
}

/// Capa pela busca do iTunes quando o Windows não entrega a imagem (como no Kon atual).
pub fn cover_fallback(st: &Shared, title: &str, artist: &str, repaint: impl Fn() + Send + 'static) -> Option<Arc<Vec<u8>>> {
    let (t, a) = clean(title, artist);
    let key = format!("{a}|{t}").to_lowercase();
    let mut g = st.lock().unwrap();
    if let Some(v) = g.covers.get(&key) { return v.clone(); }
    g.covers.insert(key.clone(), None);
    drop(g);
    let st = st.clone();
    std::thread::spawn(move || {
        let term = [a.as_str(), t.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        let d: serde_json::Value = ureq::get("https://itunes.apple.com/search").query("term", &term).query("entity", "song").query("limit", "5").query("country", "BR")
            .timeout(std::time::Duration::from_secs(5)).call().ok().and_then(|r| r.into_json().ok()).unwrap_or_default();
        let norm = |x: &serde_json::Value| x.as_str().unwrap_or("").to_lowercase();
        let tl: String = t.to_lowercase().chars().take(12).collect(); let al: String = a.to_lowercase().chars().take(8).collect();
        let res = d["results"].as_array().cloned().unwrap_or_default();
        let hit = res.iter().find(|x| norm(&x["trackName"]).contains(&tl) || (!al.is_empty() && norm(&x["artistName"]).contains(&al))).or(res.first()).cloned();
        let url = hit.and_then(|h| h["artworkUrl100"].as_str().map(|u| u.replace("100x100bb.", "600x600bb.")));
        let bytes = url.and_then(|u| ureq::get(&u).timeout(std::time::Duration::from_secs(8)).call().ok()).and_then(|r| { let mut v = vec![]; std::io::Read::read_to_end(&mut r.into_reader().take(6_000_000), &mut v).ok().map(|_| v) });
        st.lock().unwrap().covers.insert(key, bytes.map(Arc::new));
        repaint();
    });
    None
}
