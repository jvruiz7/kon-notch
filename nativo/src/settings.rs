//! Ajustes salvos em %APPDATA%\KonNativo\settings.json (separado do Kon antigo).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub open_mode: String,
    pub approvals: bool,
    pub focus_minutes: u32,
    pub accent: [u8; 3],
    pub page: String,
    pub clipboard_on: bool,
    pub water_day: String,
    pub water_count: u32,
    pub sys_alerts: bool,
    pub claude_plan: String,
    pub display: String,
    pub dnd: bool,
    pub locked: bool,
    pub privacy: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { open_mode: "hover".into(), approvals: true, focus_minutes: 25, accent: [99, 102, 241], page: "home".into(), clipboard_on: true, water_day: String::new(), water_count: 0, sys_alerts: true, claude_plan: "auto".into(), display: "primary".into(), dnd: false, locked: false, privacy: false }
    }
}

impl Settings {
    fn path() -> std::path::PathBuf { crate::platform::data_dir().join("settings.json") }
    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        let p = Self::path();
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            // grava num temporário e troca: nunca deixa o arquivo pela metade
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() { let _ = std::fs::rename(&tmp, &p); }
        }
    }
}
