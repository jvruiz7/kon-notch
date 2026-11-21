//! "Relatar problema": junta versão, sistema, telas, ajustes (sem chaves, sem link da agenda), os registros de erro
//! e, se você quiser, um print da tela, num .zip na Área de Trabalho; depois abre a página de issue do GitHub.

pub fn make(note: &str, with_shot: bool, settings: &crate::settings::Settings, extra: serde_json::Value) -> Result<(String, Option<std::path::PathBuf>), String> {
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H-%M").to_string();
    let dir = std::env::temp_dir().join(format!("kon-relatorio-{stamp}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut aj = serde_json::to_value(settings).unwrap_or_default();
    if let Some(o) = aj.as_object_mut() {
        // nada secreto vai no relatório
        for k in ["gemini_key_enc", "anthropic_key_enc", "ics_url"] { o.remove(k); }
        o.retain(|k, _| { let l = k.to_lowercase(); !(l.contains("key") || l.contains("token") || l.contains("secret") || l.contains("password")) });
    }
    let info = serde_json::json!({
        "versao": env!("CARGO_PKG_VERSION"), "app": "Kon nativo (Rust)",
        "windows": std::env::var("OS").unwrap_or_default(), "arquitetura": std::env::consts::ARCH,
        "nota": note.chars().take(2000).collect::<String>(), "ajustes": aj, "estado": extra,
    });
    std::fs::write(dir.join("info.json"), serde_json::to_string_pretty(&info).unwrap_or_default()).map_err(|e| e.to_string())?;
    let log = crate::platform::data_dir().join("erro.log");
    if let Ok(t) = std::fs::read_to_string(&log) { let lines: Vec<&str> = t.lines().collect(); let tail = lines[lines.len().saturating_sub(400)..].join("\n"); let _ = std::fs::write(dir.join("erro.log"), tail); }
    if with_shot {
        if let Some((w, h, px, _, _)) = crate::platform::capture_screen() {
            if let Some(img) = image::RgbaImage::from_raw(w, h, px) { let _ = img.save(dir.join("tela.png")); }
        }
    }
    let desk = std::env::var_os("USERPROFILE").map(|p| std::path::PathBuf::from(p).join("Desktop")).filter(|p| p.is_dir()).unwrap_or_else(std::env::temp_dir);
    let zip = desk.join(format!("Kon-relatorio-{stamp}.zip"));
    let ok = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &format!("Compress-Archive -Path '{}\\*' -DestinationPath '{}' -Force", dir.display(), zip.display())])
        .creation_flags_no_window().status().map(|s| s.success()).unwrap_or(false);
    let body = format!("**O que aconteceu:**\n{}\n\n**Versão:** Kon nativo {}\n\n(Arraste aqui o arquivo {} que está na sua Área de Trabalho.)", note, env!("CARGO_PKG_VERSION"), zip.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    let url = format!("https://github.com/jvruiz7/kon-notch/issues/new?title={}&body={}", enc(&format!("Problema no Kon nativo {}", env!("CARGO_PKG_VERSION"))), enc(&body));
    Ok((url, if ok { Some(zip) } else { None }))
}

trait NoWindow { fn creation_flags_no_window(&mut self) -> &mut Self; }
impl NoWindow for std::process::Command {
    fn creation_flags_no_window(&mut self) -> &mut Self {
        #[cfg(windows)] { use std::os::windows::process::CommandExt; self.creation_flags(0x0800_0000); }
        self
    }
}
