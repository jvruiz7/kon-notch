//! Histórico da área de transferência, igual ao Kon atual: só texto, máx. 40 itens,
//! fica só neste computador (clipboard.json) e ignora o que parece senha, chave ou cartão.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX: usize = 40;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip { pub id: u64, pub text: String, pub at: i64, pub pinned: bool }

#[derive(Default)]
pub struct State { pub list: Vec<Clip>, pub on: bool, last: String, skip_next: Option<String> }

pub type Shared = Arc<Mutex<State>>;

fn path() -> std::path::PathBuf { crate::platform::data_dir().join("clipboard.json") }

fn save(list: &[Clip]) {
    let p = path();
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    if let Ok(s) = serde_json::to_string(list) {
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, s).is_ok() { let _ = std::fs::rename(&tmp, &p); }
    }
}

pub fn looks_secret(t: &str) -> bool {
    let s = t.trim();
    const PREFIX: [&str; 16] = ["sk-", "sk_", "pk_", "rk_", "AIza", "AQ.Ab", "ghp_", "gho_", "github_pat_", "xoxa-", "xoxb-", "xoxp-", "xoxr-", "xoxs-", "glpat-", "AKIA"];
    if PREFIX.iter().any(|p| s.starts_with(p)) || s.starts_with("ya29.") { return true; }
    if s.starts_with("eyJ") && s.matches('.').count() >= 2 { return true; } // JWT
    if s.contains("-----BEGIN") && s.contains("PRIVATE KEY-----") { return true; }
    let digits: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if (13..=19).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit()) { return true; } // cartão
    let n = s.chars().count();
    if !s.contains(char::is_whitespace) && (12..=200).contains(&n) && !s.starts_with("http://") && !s.starts_with("https://")
        && s.chars().any(|c| c.is_ascii_lowercase()) && s.chars().any(|c| c.is_ascii_uppercase()) && s.chars().any(|c| c.is_ascii_digit())
        && (s.chars().any(|c| !c.is_alphanumeric() && c != '_') || n >= 24) { return true; }
    false
}

pub fn start(on: bool, repaint: impl Fn() + Send + 'static) -> Shared {
    let list: Vec<Clip> = std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let st = Arc::new(Mutex::new(State { list, on, ..Default::default() }));
    let s2 = st.clone();
    std::thread::spawn(move || {
        let mut cb = None;
        loop {
            std::thread::sleep(Duration::from_millis(700));
            if cb.is_none() { cb = arboard::Clipboard::new().ok(); }
            let Some(c) = cb.as_mut() else { continue };
            let Ok(t) = c.get_text() else { continue };
            let mut g = s2.lock().unwrap();
            if t == g.last { continue; }
            g.last = t.clone();
            if g.skip_next.take().as_deref() == Some(t.as_str()) { continue; }
            if !g.on || t.trim().is_empty() || t.len() > 20000 || looks_secret(&t) { continue; }
            g.list.retain(|x| x.text != t);
            let id = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
            g.list.insert(0, Clip { id, text: t, at: chrono::Utc::now().timestamp_millis(), pinned: false });
            let (mut pin, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut g.list).into_iter().partition(|x| x.pinned);
            pin.extend(rest); pin.truncate(MAX);
            g.list = pin; save(&g.list);
            drop(g); repaint();
        }
    });
    st
}

pub fn copy(st: &Shared, id: u64) -> bool {
    let mut g = st.lock().unwrap();
    let Some(t) = g.list.iter().find(|x| x.id == id).map(|x| x.text.clone()) else { return false };
    g.last = t.clone();
    arboard::Clipboard::new().and_then(|mut c| c.set_text(t)).is_ok()
}
pub fn remove(st: &Shared, id: u64) { let mut g = st.lock().unwrap(); g.list.retain(|x| x.id != id); save(&g.list); }
pub fn pin(st: &Shared, id: u64) {
    let mut g = st.lock().unwrap();
    for x in g.list.iter_mut() { if x.id == id { x.pinned = !x.pinned; } }
    let (mut p, r): (Vec<_>, Vec<_>) = std::mem::take(&mut g.list).into_iter().partition(|x| x.pinned);
    p.extend(r); g.list = p; save(&g.list);
}
pub fn clear(st: &Shared) { let mut g = st.lock().unwrap(); g.list.retain(|x| x.pinned); save(&g.list); }
pub fn set_on(st: &Shared, on: bool) {
    let mut g = st.lock().unwrap(); g.on = on;
    // ao religar, não grava o que já estava copiado enquanto estava pausado
    if on { if let Ok(t) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) { g.last = t; } }
}

#[cfg(test)]
mod t {
    use super::looks_secret as s;
    #[test] fn secretos() {
        assert!(s("sk-ant-abc123")); assert!(s("AIzaSyD-xxxxxxxx")); assert!(s("4111 1111 1111 1111"));
        assert!(s("Senha#Forte2024")); assert!(s("eyJhbGc.eyJzdWI.sig"));
        assert!(!s("olá mundo")); assert!(!s("https://Exemplo.com/A1b2c3d4e5")); assert!(!s("12345")); assert!(!s("reunião às 15h"));
    }
}
