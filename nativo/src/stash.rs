//! Gaveta de arquivos (File Stash), igual ao Kon atual: guarda só a referência (caminho, nome,
//! tamanho) em stash.json. Nunca lê o conteúdo dos arquivos e nada é enviado para IA.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item { pub id: u64, pub name: String, pub path: String, pub size: u64, pub ext: String, pub added_at: i64, #[serde(skip)] pub missing: bool }

#[derive(Default)]
pub struct Stash { pub list: Vec<Item> }

fn file() -> std::path::PathBuf { crate::platform::data_dir().join("stash.json") }

impl Stash {
    pub fn load() -> Self {
        let mut list: Vec<Item> = std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        for it in &mut list { it.missing = !Path::new(&it.path).exists(); }
        Stash { list }
    }
    fn save(&self) {
        let p = file();
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        if let Ok(s) = serde_json::to_string_pretty(&self.list) { let t = p.with_extension("tmp"); if std::fs::write(&t, s).is_ok() { let _ = std::fs::rename(&t, &p); } }
    }
    pub fn refresh_missing(&mut self) { for it in &mut self.list { it.missing = !Path::new(&it.path).exists(); } }
    pub fn add(&mut self, paths: &[std::path::PathBuf]) -> usize {
        let mut n = 0;
        for p in paths {
            let ps = p.to_string_lossy().to_string();
            if ps.is_empty() || self.list.iter().any(|i| i.path == ps) { continue; }
            let md = std::fs::metadata(p).ok();
            let dir = md.as_ref().map_or(false, |m| m.is_dir());
            let it = Item {
                id: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64 + n as u64,
                name: p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(ps.clone()),
                size: md.as_ref().filter(|m| m.is_file()).map_or(0, |m| m.len()),
                ext: if dir { "folder".into() } else { p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default() },
                path: ps, added_at: chrono::Utc::now().timestamp_millis(), missing: false,
            };
            self.list.insert(0, it); n += 1;
        }
        if n > 0 { self.save(); }
        n
    }
    pub fn remove(&mut self, id: u64) { self.list.retain(|i| i.id != id); self.save(); }
}

pub fn open(path: &str) { let _ = std::process::Command::new("explorer").arg(path).spawn(); }
pub fn reveal(path: &str) { let _ = std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn(); }

pub fn icon(ext: &str) -> char {
    use crate::icons as ic;
    let any = |l: &[&str]| l.contains(&ext);
    if ext == "folder" { ic::FOLDER }
    else if any(&["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "ico"]) { ic::FILE_IMAGE }
    else if any(&["mp4", "mov", "mkv", "avi", "webm"]) { ic::FILE_VIDEO }
    else if any(&["mp3", "wav", "flac", "m4a", "ogg"]) { ic::FILE_AUDIO }
    else if any(&["zip", "rar", "7z", "tar", "gz"]) { ic::FILE_ARCHIVE }
    else if any(&["js", "ts", "tsx", "jsx", "py", "json", "html", "css", "cs", "cpp", "java", "go", "rs", "ps1", "sql"]) { ic::FILE_CODE }
    else if any(&["pdf", "doc", "docx", "txt", "md", "xls", "xlsx", "ppt", "pptx", "csv", "dwg", "rtf"]) { ic::FILE_TEXT }
    else { ic::FILE }
}

pub fn fmt_size(b: u64) -> String {
    if b == 0 { return String::new(); }
    let b = b as f64;
    if b < 1024.0 { format!("{b:.0} B") } else if b < 1048576.0 { format!("{:.0} KB", b / 1024.0) } else if b < 1073741824.0 { format!("{:.1} MB", b / 1048576.0) } else { format!("{:.1} GB", b / 1073741824.0) }
}
