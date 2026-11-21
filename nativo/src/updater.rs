//! Atualização automática do Kon nativo pelos releases do GitHub (tags "nativo-vX.Y.Z" com o Kon.exe).
//! Baixa em segundo plano; "Reiniciar" troca o exe (um exe em uso pode ser renomeado no Windows) e reabre.

use std::sync::{Arc, Mutex};

const REPO: &str = "jvruiz7/kon-notch";

#[derive(Clone, Debug, Default, PartialEq)]
pub enum St { #[default] Idle, Checking, Downloading(u32), Ready(String), Latest, Error(String) }

#[derive(Default)]
pub struct State { pub st: St, pub version: String, pub file: Option<std::path::PathBuf>, last: Option<std::time::Instant> }
pub type Shared = Arc<Mutex<State>>;

pub fn new() -> Shared { Arc::new(Mutex::new(State::default())) }

fn ver(s: &str) -> Vec<u32> { s.trim_start_matches(|c: char| !c.is_ascii_digit()).split('.').map(|x| x.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0)).collect() }
pub fn newer(remote: &str, local: &str) -> bool { ver(remote) > ver(local) }

/// Procura uma versão nova (no máximo a cada 4 h, a não ser que `force`) e já baixa.
pub fn check(st: &Shared, force: bool, repaint: impl Fn() + Send + 'static) {
    {
        let mut g = st.lock().unwrap();
        if matches!(g.st, St::Checking | St::Downloading(_) | St::Ready(_)) { return; }
        if !force && g.last.map_or(false, |t| t.elapsed().as_secs() < 4 * 3600) { return; }
        g.last = Some(std::time::Instant::now());
        g.st = St::Checking;
    }
    let st = st.clone();
    std::thread::spawn(move || {
        let set = |s: St| { st.lock().unwrap().st = s; };
        let list: Result<serde_json::Value, String> = ureq::get(&format!("https://api.github.com/repos/{REPO}/releases?per_page=30"))
            .set("User-Agent", "Kon").set("Accept", "application/vnd.github+json").timeout(std::time::Duration::from_secs(15))
            .call().map_err(|e| e.to_string()).and_then(|r| r.into_json().map_err(|e| e.to_string()));
        let list = match list { Ok(l) => l, Err(e) => { set(St::Error(e)); repaint(); return; } };
        let best = list.as_array().into_iter().flatten()
            .filter(|r| !r["draft"].as_bool().unwrap_or(false) && r["tag_name"].as_str().map_or(false, |t| t.starts_with("nativo-v")))
            .max_by(|a, b| ver(a["tag_name"].as_str().unwrap_or("")).cmp(&ver(b["tag_name"].as_str().unwrap_or(""))));
        let Some(rel) = best else { set(St::Latest); repaint(); return };
        let tag = rel["tag_name"].as_str().unwrap_or("").to_string();
        let v = tag.trim_start_matches("nativo-v").to_string();
        if !newer(&v, env!("CARGO_PKG_VERSION")) { set(St::Latest); repaint(); return; }
        let Some(url) = rel["assets"].as_array().into_iter().flatten().find(|a| a["name"].as_str().map_or(false, |n| n.eq_ignore_ascii_case("Kon.exe"))).and_then(|a| a["browser_download_url"].as_str()).map(String::from)
            else { set(St::Error("a versão nova não tem o Kon.exe".into())); repaint(); return };
        set(St::Downloading(0)); repaint();
        let dir = crate::platform::data_dir().join("update");
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join(format!("Kon-{v}.exe"));
        let res: Result<(), String> = (|| {
            let r = ureq::get(&url).set("User-Agent", "Kon").timeout(std::time::Duration::from_secs(300)).call().map_err(|e| e.to_string())?;
            let total: u64 = r.header("Content-Length").and_then(|x| x.parse().ok()).unwrap_or(0);
            let mut rd = r.into_reader();
            let tmp = dest.with_extension("part");
            let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 256 * 1024]; let mut got = 0u64;
            loop {
                let n = std::io::Read::read(&mut rd, &mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                std::io::Write::write_all(&mut f, &buf[..n]).map_err(|e| e.to_string())?;
                got += n as u64;
                if total > 0 { set(St::Downloading((got * 100 / total) as u32)); }
            }
            drop(f);
            if got < 1_000_000 || (total > 0 && got != total) { return Err("download incompleto".into()); }
            // confere que é mesmo um executável do Windows antes de aceitar
            let head = std::fs::read(&tmp).map_err(|e| e.to_string())?;
            if head.get(0..2) != Some(b"MZ") { return Err("arquivo baixado inválido".into()); }
            std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
        })();
        let mut g = st.lock().unwrap();
        match res { Ok(()) => { g.st = St::Ready(v.clone()); g.version = v; g.file = Some(dest); } Err(e) => g.st = St::Error(e) }
        drop(g); repaint();
    });
}

/// Troca o exe pela versão baixada e abre a nova (o Kon atual fecha logo depois).
pub fn install(st: &Shared) -> Result<(), String> {
    let file = st.lock().unwrap().file.clone().ok_or("nada para instalar")?;
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = me.with_extension("old.exe");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&me, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::copy(&file, &me) { let _ = std::fs::rename(&old, &me); return Err(e.to_string()); }
    std::process::Command::new(&me).arg("--atualizado").spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Ao abrir depois de uma atualização: apaga a cópia antiga e os downloads.
pub fn cleanup() {
    if let Ok(me) = std::env::current_exe() { let _ = std::fs::remove_file(me.with_extension("old.exe")); }
    let _ = std::fs::remove_dir_all(crate::platform::data_dir().join("update"));
}

#[cfg(test)]
mod t { #[test] fn versoes() { assert!(super::newer("2.0.1", "2.0.0")); assert!(!super::newer("2.0.0", "2.0.0")); assert!(super::newer("2.10.0", "2.9.9")); } }
