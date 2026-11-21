//! Ajustes salvos em %APPDATA%\KonNativo\settings.json (separado do Kon antigo).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub open_mode: String,
    pub approvals: bool,
    pub focus_minutes: u32,
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
    pub page_order: Vec<String>,
    pub pages_off: Vec<String>,
    pub provider: String,
    pub gemini_model: String,
    pub claude_model: String,
    pub web_search: bool,
    pub gemini_key_enc: String,
    pub anthropic_key_enc: String,
    pub outfit: String,
    pub accent: String,
    pub glow: f32,
    pub notch_size: String,
    pub hide_fullscreen: bool,
    pub slim_over_max: bool,
    pub hide_in_capture: bool,
    pub sounds_on: bool,
    pub sound_pack: String,
    pub sound_vol: f32,
    pub focus_dnd: bool,
    pub ics_url: String,
    pub onboarded: bool,
    pub auto_update: bool,
    pub loc_id: String,
    pub lyrics_on: bool,
    pub focus_log: Vec<(i64, u32)>,
    pub auto_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { open_mode: "hover".into(), approvals: true, focus_minutes: 25, page: "home".into(), clipboard_on: true, water_day: String::new(), water_count: 0, sys_alerts: true, claude_plan: "auto".into(), display: "primary".into(), dnd: false, locked: false, privacy: false, page_order: ["home","files","clip","music","weather","focus","todo","claude","quick","system","water"].iter().map(|s| s.to_string()).collect(), pages_off: vec![], provider: "gemini".into(), gemini_model: "gemini-flash-latest".into(), claude_model: "claude-haiku-5-5".into(), web_search: true, gemini_key_enc: String::new(), anthropic_key_enc: String::new(), outfit: "auto".into(), accent: "aurora".into(), glow: 1.0, notch_size: "normal".into(), hide_fullscreen: true, slim_over_max: true, hide_in_capture: true, sounds_on: true, sound_pack: "retro".into(), sound_vol: 0.6, focus_dnd: true, ics_url: String::new(), onboarded: false, auto_update: true, loc_id: "auto".into(), lyrics_on: true, focus_log: vec![], auto_start: true }
    }
}

impl Settings {
    fn path() -> std::path::PathBuf { crate::platform::data_dir().join("settings.json") }
    pub fn load() -> Self {
        // campo a campo: um valor estranho (de uma versão antiga) não apaga os outros ajustes
        let Some(loaded) = std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) else { return Self::default() };
        let mut cur = serde_json::to_value(Self::default()).unwrap_or_default();
        if let (Some(obj), Some(map)) = (loaded.as_object(), cur.as_object().cloned()) {
            let _ = map;
            for (k, v) in obj {
                let mut cand = cur.clone();
                cand[k] = v.clone();
                if serde_json::from_value::<Self>(cand.clone()).is_ok() { cur = cand; }
            }
        }
        serde_json::from_value(cur).unwrap_or_default()
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

#[cfg(test)]
mod t {
    #[test] fn valor_antigo_nao_apaga_o_resto() {
        let v: serde_json::Value = serde_json::json!({"accent": [1, 2, 3], "provider": "claude", "glow": 0.5});
        let mut cur = serde_json::to_value(super::Settings::default()).unwrap();
        for (k, x) in v.as_object().unwrap() { let mut c = cur.clone(); c[k] = x.clone(); if serde_json::from_value::<super::Settings>(c.clone()).is_ok() { cur = c; } }
        let s: super::Settings = serde_json::from_value(cur).unwrap();
        assert_eq!(s.provider, "claude"); assert_eq!(s.accent, "aurora"); assert_eq!(s.glow, 0.5);
    }
}
