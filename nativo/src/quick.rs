//! Atalhos: programas, pastas, sites e comandos que você abre todo dia (quick.json).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quick { pub id: u64, pub kind: String, pub label: String, pub target: String }

fn file() -> std::path::PathBuf { crate::platform::data_dir().join("quick.json") }

pub fn load() -> Vec<Quick> { std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() }
pub fn save(list: &[Quick]) {
    let p = file();
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    if let Ok(s) = serde_json::to_string_pretty(list) { let t = p.with_extension("tmp"); if std::fs::write(&t, s).is_ok() { let _ = std::fs::rename(&t, &p); } }
}

/// Nome automático quando você não escreve um: nome do arquivo/pasta, domínio do site ou 1ª palavra do comando.
pub fn auto_label(kind: &str, target: &str) -> String {
    let t = target.trim().trim_matches('"');
    match kind {
        "site" => t.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").split('/').next().unwrap_or(t).to_string(),
        "cmd" => t.split_whitespace().next().unwrap_or(t).to_string(),
        _ => { let b = crate::claude::base_name(t); let b = b.strip_suffix(".exe").or(b.strip_suffix(".EXE")).map(String::from).unwrap_or(b); if b.is_empty() { t.to_string() } else { b } }
    }
}

pub fn run(q: &Quick) -> Result<(), String> {
    let t = q.target.trim();
    if t.is_empty() { return Err("Atalho vazio".into()); }
    match q.kind.as_str() {
        "site" => {
            let url = if t.starts_with("http://") || t.starts_with("https://") { t.to_string() } else { format!("https://{t}") };
            open_shell(&url)
        }
        "cmd" => {
            // comando escrito por você: abre numa janela própria do Prompt para você ver a saída
            let home = std::env::var_os("USERPROFILE").unwrap_or_default();
            std::process::Command::new("cmd").args(["/c", "start", "Kon", "cmd", "/k", t]).current_dir(home).spawn().map(|_| ()).map_err(|e| e.to_string())
        }
        _ => {
            if !std::path::Path::new(t.trim_matches('"')).exists() { return Err("Não encontrei esse caminho".into()); }
            open_shell(t.trim_matches('"'))
        }
    }
}

fn open_shell(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let r = unsafe { ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
        if r.0 as isize > 32 { Ok(()) } else { Err("Não consegui abrir".into()) }
    }
    #[cfg(not(windows))]
    { std::process::Command::new("xdg-open").arg(target).spawn().map(|_| ()).map_err(|e| e.to_string()) }
}

pub fn pick(folder: bool) -> Option<String> {
    #[cfg(windows)]
    {
        let d = rfd::FileDialog::new().set_title(if folder { "Escolha a pasta" } else { "Escolha o programa ou arquivo" });
        let r = if folder { d.pick_folder() } else { d.pick_file() };
        r.map(|p| p.to_string_lossy().to_string())
    }
    #[cfg(not(windows))]
    { let _ = folder; None }
}

#[cfg(test)]
mod t {
    use super::auto_label as a;
    #[test] fn nomes() {
        assert_eq!(a("site", "https://www.sinapi.caixa.gov.br/x"), "sinapi.caixa.gov.br");
        assert_eq!(a("app", "C:\\Program Files\\Revit\\Revit.exe"), "Revit");
        assert_eq!(a("cmd", "code C:\\proj"), "code");
    }
}
