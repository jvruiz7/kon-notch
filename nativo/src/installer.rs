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
        if std::env::var_os("KON_CI").is_some() { return false; }
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
        // instalado pelo KonSetup.exe: o instalador cuida do registro e da desinstalação
        if dir().join("Desinstalar.exe").exists() { return; }
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

    /// O Kon antigo (Electron) aberto ao mesmo tempo disputa os atalhos e o topo da tela: oferece fechar
    /// e tirar ele do início do Windows.
    pub fn close_old_kon() {
        use windows::core::{w, PWSTR};
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE};
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};
        if std::env::var_os("KON_PORTABLE").is_some() || std::env::var_os("KON_CI").is_some() { return; }
        unsafe {
            let Ok(h) = FindWindowW(w!("Chrome_WidgetWin_1"), w!("Kon")) else { return };
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == 0 || pid == std::process::id() { return; }
            let Ok(ph) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE, false, pid) else { return };
            let mut buf = [0u16; 1024]; let mut len = buf.len() as u32;
            let path = if QueryFullProcessImageNameW(ph, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok() { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() };
            if message("O Kon antigo está aberto e usa os mesmos atalhos e o mesmo lugar no topo da tela.\n\nFechar o Kon antigo e não abrir mais ele com o Windows?", true) {
                let _ = TerminateProcess(ph, 0);
                // fecha também os outros processos do Electron dele
                if !path.is_empty() {
                    let p = path.replace('\'', "''");
                    let ps = format!("Get-Process | Where-Object {{ $_.Path -eq '{p}' }} | Stop-Process -Force -ErrorAction SilentlyContinue");
                    let _ = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &ps]).creation_flags(NO_WINDOW).status();
                    remove_run_entries_for(&path);
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            let _ = CloseHandle(ph);
        }
    }
    fn remove_run_entries_for(exe: &str) {
        let key = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        unsafe {
            let mut k = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, &key, None, KEY_READ | KEY_SET_VALUE, &mut k).is_err() { return; }
            let mut del = vec![];
            for i in 0..256u32 {
                let mut name = [0u16; 512]; let mut nl = name.len() as u32;
                let mut data = [0u8; 4096]; let mut dl = data.len() as u32;
                if RegEnumValueW(k, i, Some(windows::core::PWSTR(name.as_mut_ptr())), &mut nl, None, None, Some(data.as_mut_ptr()), Some(&mut dl)).is_err() { break; }
                let n = String::from_utf16_lossy(&name[..nl as usize]);
                let wide: Vec<u16> = data[..dl as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                let v = String::from_utf16_lossy(&wide).to_lowercase();
                if n != "KonNativo" && v.contains(&exe.to_lowercase()) { del.push(n); }
            }
            for n in del { let _ = RegDeleteValueW(k, &HSTRING::from(n)); }
            let _ = RegCloseKey(k);
        }
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
    pub fn close_old_kon() {}
    pub fn message(_t: &str, _ask: bool) -> bool { false }
}

pub use imp::*;
