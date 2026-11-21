//! Tarefas do dia, salvas em %APPDATA%\KonNativo\tasks.json.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task { pub id: u64, pub text: String, pub done: bool }

#[derive(Default)]
pub struct Tasks { pub list: Vec<Task> }

impl Tasks {
    fn path() -> std::path::PathBuf { crate::platform::data_dir().join("tasks.json") }
    pub fn load() -> Self { Tasks { list: std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() } }
    pub fn save(&self) {
        let p = Self::path();
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        if let Ok(s) = serde_json::to_string_pretty(&self.list) { let t = p.with_extension("tmp"); if std::fs::write(&t, s).is_ok() { let _ = std::fs::rename(&t, &p); } }
    }
    pub fn add(&mut self, text: &str) {
        let t = text.trim(); if t.is_empty() { return; }
        let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        self.list.push(Task { id, text: t.to_string(), done: false }); self.save();
    }
    pub fn toggle(&mut self, id: u64) { if let Some(t) = self.list.iter_mut().find(|t| t.id == id) { t.done = !t.done; } self.save(); }
    pub fn remove(&mut self, id: u64) { self.list.retain(|t| t.id != id); self.save(); }
    pub fn done(&self) -> usize { self.list.iter().filter(|t| t.done).count() }
}
