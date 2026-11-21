//! Instalação sem instalador: na primeira vez que o Kon.exe baixado é aberto, ele se copia para
//! %LOCALAPPDATA%\Programs\KonNativo, cria o atalho no Menu Iniciar, aparece em "Aplicativos instalados"
//! (com Desinstalar) e reabre de lá. Também garante uma única cópia aberta por vez.

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::*;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MB_SETFOREGROUND};

    const NO_WINDOW: u32 = 0x0800_0000;
    const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\KonNativo";

    pub fn dir() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        base.join("Programs").join("KonNativo")
    }
    pub fn installed_exe() -> PathBuf { dir().join("Kon.exe") }
    fn shortcut() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        base.join("Microsoft\\Windows\\Start Menu\\Programs\\Kon (nativo).lnk")
    }
    fn same(a: &std::path::Path, b: &std::path::Path) -> bool {
        a.to_string_lossy().to_lowercase().trim_start_matches(r"\\?\") == b.to_string_lossy().to_lowercase().trim_start_matches(r"\\?\")
    }
    pub fn is_installed_copy() -> bool { std::env::current_exe().map_or(false, |me| same(&me, &installed_exe())) }

    pub fn message(text: &str, ask: bool) -> bool {
        let r = unsafe { MessageBoxW(None, &HSTRING::from(text), &HSTRING::from("Kon"), if ask { MB_YESNO | MB_ICONQUESTION | MB_SETFOREGROUND } else { MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND }) };
        r == IDYES
    }

    fn set_sz(k: HKEY, name: &str, val: &str) {
        let w: Vec<u16> = val.encode_utf16().chain(Some(0)).collect();
        let bytes = unsafe { std::slice::from_raw_parts(w.as_ptr() as *const u8, w.len() * 2) };
        unsafe { let _ = RegSetValueExW(k, &HSTRING::from(name), None, REG_SZ, Some(bytes)); }
    }
    fn set_dword(k: HKEY, name: &str, val: u32) {
        unsafe { let _ = RegSetValueExW(k, &HSTRING::from(name), None, REG_DWORD, Some(&val.to_le_bytes())); }
    }

    /// Registra em "Aplicativos instalados" (Configurações do Windows), com a versão atual.
    pub fn register() {
        let exe = installed_exe();
        unsafe {
            let mut k = HKEY::default();
            if RegCreateKeyExW(HKEY_CURRENT_USER, &HSTRING::from(UNINSTALL_KEY), None, None, REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut k, None).is_err() { return; }
            set_sz(k, "DisplayName", "Kon (nativo)");
            set_sz(k, "DisplayVersion", env!("CARGO_PKG_VERSION"));
            set_sz(k, "Publisher", "jvruiz7");
            set_sz(k, "DisplayIcon", &exe.display().to_string());
            set_sz(k, "InstallLocation", &dir().display().to_string());
            set_sz(k, "UninstallString", &format!("\"{}\" --uninstall", exe.display()));
            set_dword(k, "NoModify", 1);
            set_dword(k, "NoRepair", 1);
            set_dword(k, "EstimatedSize", (std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0) / 1024) as u32);
            let _ = RegCloseKey(k);
        }
    }

    fn make_shortcut() {
        let ps = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Description='Kon';$s.Save()",
            shortcut().display().to_string().replace('\'', "''"), installed_exe().display().to_string().replace('\'', "''"), dir().display().to_string().replace('\'', "''"));
        let _ = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &ps]).creation_flags(NO_WINDOW).status();
    }

    /// Instala se este exe não for a cópia instalada. Devolve true quando a cópia instalada já foi aberta (este processo deve sair).
    pub fn ensure_installed() -> bool {
        if std::env::var_os("KON_PORTABLE").is_some() || std::env::var_os("KON_DEMO").is_some() || std::env::args().any(|a| a == "--portable") { return false; }
        let Ok(me) = std::env::current_exe() else { return false };
        if same(&me, &installed_exe()) { register(); return false; }
        let res: Result<(), String> = (|| {
            std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
            let dest = installed_exe();
            if dest.exists() {
                // uma versão anterior instalada (fechada): troca pelo exe novo
                let old = dest.with_extension("old.exe");
                let _ = std::fs::remove_file(&old);
                let _ = std::fs::rename(&dest, &old);
            }
            std::fs::copy(&me, &dest).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(e) = res {
            // sem conseguir instalar, roda daqui mesmo (portátil)
            let d = crate::platform::data_dir(); let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join("erro.log"), format!("instalação: {e}\n"));
            return false;
        }
        make_shortcut();
        register();
        std::process::Command::new(installed_exe()).arg("--instalado").spawn().is_ok()
    }

    /// "Desinstalar" (Configurações do Windows ou Kon.exe --uninstall). Os ajustes ficam guardados.
    pub fn uninstall() {
        if !message("Desinstalar o Kon deste computador?\n\nSeus ajustes ficam guardados caso você instale de novo.", true) { return; }
        crate::platform::set_autostart(false);
        let _ = std::fs::remove_file(shortcut());
        unsafe { let _ = RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(UNINSTALL_KEY)); }
        let path = installed_exe().display().to_string().replace('\'', "''");
        let d = dir().display().to_string().replace('\'', "''");
        // fecha o Kon aberto (só o desta pasta) e apaga a pasta depois que este processo sair
        let ps = format!("Start-Sleep -Milliseconds 800; Get-Process | Where-Object {{ $_.Path -eq '{path}' }} | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 2; Remove-Item -LiteralPath '{d}' -Recurse -Force -ErrorAction SilentlyContinue");
        let _ = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &ps]).creation_flags(NO_WINDOW).spawn();
        message("O Kon foi desinstalado.", false);
    }

    /// Uma cópia por vez. Ao reabrir depois de atualizar/instalar, espera a anterior fechar.
    pub fn single_instance() -> bool {
        use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;
        let wait = std::env::args().any(|a| a == "--atualizado" || a == "--instalado");
        for _ in 0..if wait { 60 } else { 1 } {
            unsafe {
                if let Ok(h) = CreateMutexW(None, true, &HSTRING::from("Local\\KonNativo")) {
                    if GetLastError() != ERROR_ALREADY_EXISTS { std::mem::forget(h); return true; }
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                }
            }
            if wait { std::thread::sleep(std::time::Duration::from_millis(250)); }
        }
        if !wait { message("O Kon já está aberto: procure a ilha no topo da tela ou o ícone perto do relógio.", false); }
        false
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn ensure_installed() -> bool { false }
    pub fn uninstall() {}
    pub fn single_instance() -> bool { true }
    pub fn is_installed_copy() -> bool { false }
}

pub use imp::*;
