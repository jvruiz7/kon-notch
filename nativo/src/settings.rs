//! Ajustes salvos em %APPDATA%\KonNativo\settings.json (separado do Kon antigo).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub open_on_hover: bool,
    pub focus_minutes: u32,
    pub accent: [u8; 3],
    pub page: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { open_on_hover: true, focus_minutes: 25, accent: [99, 102, 241], page: "home".into() }
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
