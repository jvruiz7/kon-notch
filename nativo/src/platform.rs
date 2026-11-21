//! Tudo que fala direto com o Windows. Em outros sistemas há versões vazias, só para compilar e testar a interface.

/// Retângulo em pixels físicos.
#[derive(Clone, Copy, Debug, Default)]
pub struct PxRect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

#[cfg(windows)]
mod imp {
    use super::PxRect;
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, GetMonitorInfoW, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};

    fn hwnd() -> Option<HWND> {
        // a janela do Kon nativo tem o título "KonNativo" (o Kon antigo, em Electron, usa "Kon") e é da nossa classe (winit)
        unsafe { FindWindowW(None, w!("KonNativo")).ok() }
    }

    /// Cursor e janela, os dois em pixels físicos (o processo é "per-monitor DPI aware", então não há mistura de escalas).
    pub fn cursor_in_window() -> Option<(i32, i32)> {
        let h = hwnd()?;
        let mut p = POINT::default();
        let mut r = RECT::default();
        unsafe {
            GetCursorPos(&mut p).ok()?;
            GetWindowRect(h, &mut r).ok()?;
        }
        Some((p.x - r.left, p.y - r.top))
    }

    /// Área do monitor principal (sem a barra de tarefas), em pixels físicos.
    pub fn primary_work_area() -> PxRect {
        unsafe {
            let m = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if GetMonitorInfoW(m, &mut mi).as_bool() {
                let r = mi.rcMonitor;
                return PxRect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top };
            }
        }
        PxRect { x: 0, y: 0, w: 1920, h: 1080 }
    }

    /// Centraliza o Kon no topo do monitor escolhido: o principal ou o que está com o mouse.
    /// Tudo em pixels físicos (cada monitor pode ter uma escala diferente). Devolve true se moveu.
    pub fn place(follow_cursor: bool) -> bool {
        let Some(h) = hwnd() else { return false };
        unsafe {
            let mut pt = POINT::default();
            if follow_cursor { let _ = GetCursorPos(&mut pt); }
            let m = MonitorFromPoint(pt, if follow_cursor { windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST } else { MONITOR_DEFAULTTOPRIMARY });
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if !GetMonitorInfoW(m, &mut mi).as_bool() { return false; }
            let mut r = RECT::default();
            if GetWindowRect(h, &mut r).is_err() { return false; }
            let mr = mi.rcMonitor;
            let w = r.right - r.left;
            let x = mr.left + ((mr.right - mr.left) - w) / 2;
            let y = mr.top;
            if (r.left - x).abs() <= 1 && r.top == y { return false; }
            let _ = SetWindowPos(h, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            true
        }
    }

    /// Manda Ctrl+<tecla> para o programa em primeiro plano (o Kon não pega o foco). Como o Kon atual: espera você
    /// soltar Ctrl/Alt/Shift/Win (até 1,2 s), solta o que ainda estiver preso e então manda Ctrl+tecla.
    pub fn send_ctrl(key: u8) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let down = |vk: VIRTUAL_KEY| unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 };
        let mut waited = 0;
        while waited < 1200 && [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN].iter().any(|k| down(*k)) { std::thread::sleep(std::time::Duration::from_millis(15)); waited += 15; }
        let k = |vk: VIRTUAL_KEY, up: bool| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, time: 0, dwExtraInfo: 0 } } };
        let mut rel = vec![];
        for vk in [VK_MENU, VK_SHIFT, VK_LWIN] { if down(vk) { rel.push(k(vk, true)); } }
        unsafe {
            if !rel.is_empty() { SendInput(&rel, std::mem::size_of::<INPUT>() as i32); }
            let c = VIRTUAL_KEY(key as u16);
            SendInput(&[k(VK_CONTROL, false), k(c, false)], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(20));
            SendInput(&[k(c, true), k(VK_CONTROL, true)], std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// "Não incomodar" do Windows (Assistente de foco) pelo mesmo estado WNF que o Kon atual usa.
    /// 0 = desligado, 1 = só prioridade, 2 = só alarmes. Devolve o modo anterior.
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn ZwUpdateWnfStateData(name: *const u64, buf: *const u8, len: u32, ty: *const u8, scope: *const u8, matchd: u32, check: u32) -> i32;
        fn ZwQueryWnfStateData(name: *const u64, ty: *const u8, scope: *const u8, stamp: *mut u32, buf: *mut u8, size: *mut u32) -> i32;
    }
    const QUIET: u64 = 0x0D83063EA3BF1C75;
    pub fn dnd_get() -> i32 {
        let mut b = [0u8; 4]; let mut size = 4u32; let mut stamp = 0u32;
        let r = unsafe { ZwQueryWnfStateData(&QUIET, std::ptr::null(), std::ptr::null(), &mut stamp, b.as_mut_ptr(), &mut size) };
        if r == 0 { i32::from_le_bytes(b) } else { -1 }
    }
    pub fn dnd_set(mode: i32) -> bool {
        let b = mode.to_le_bytes();
        unsafe { ZwUpdateWnfStateData(&QUIET, b.as_ptr(), 4, std::ptr::null(), std::ptr::null(), 0, 0) == 0 }
    }

    /// Fora de prints, gravações e compartilhamento de tela (Teams, Meet, Discord...).
    pub fn exclude_from_capture(on: bool) {
        if let Some(h) = hwnd() { unsafe { let _ = SetWindowDisplayAffinity(h, if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE }); } }
    }

    /// O que está na frente: (tela cheia — jogo, vídeo, apresentação; janela maximizada no monitor do Kon).
    pub fn foreground_state() -> (bool, bool) {
        // igual ao Kon atual: olha a janela em primeiro plano. Tela cheia = ela cobre o monitor inteiro
        // (sem estar maximizada) ou o Windows diz que há jogo D3D/apresentação. "Ocupado" (QUNS_BUSY) não conta:
        // sobreposições transparentes (NVIDIA, Game Bar) deixam o Windows nesse estado o tempo todo.
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
        unsafe {
            let fg = GetForegroundWindow();
            if fg.0.is_null() || !IsWindowVisible(fg).as_bool() { return (false, false); }
            let Some(k) = hwnd() else { return (false, false) };
            if fg == k { return (false, false); }
            let mut fpid = 0u32; GetWindowThreadProcessId(fg, Some(&mut fpid));
            if fpid == std::process::id() { return (false, false); }
            let mut cls = [0u16; 64];
            let n = GetClassNameW(fg, &mut cls);
            let c = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
            if c == "Progman" || c == "WorkerW" || c == "Shell_TrayWnd" || c == "Shell_SecondaryTrayWnd" { return (false, false); }
            let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
            // só importa o que está no monitor do Kon
            if mon != MonitorFromWindow(k, MONITOR_DEFAULTTONEAREST) { return (false, false); }
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            let _ = GetMonitorInfoW(mon, &mut mi);
            let mut r = RECT::default();
            let _ = GetWindowRect(fg, &mut r);
            let m = mi.rcMonitor;
            let covers = r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom;
            let zoomed = IsZoomed(fg).as_bool();
            let q = SHQueryUserNotificationState().ok();
            let full = (covers && !zoomed) || q == Some(QUNS_RUNNING_D3D_FULL_SCREEN) || q == Some(QUNS_PRESENTATION_MODE);
            (full, !full && zoomed)
        }
    }

    /// Esconde/mostra a janela do Kon (tela cheia).
    pub fn set_visible(on: bool) { if let Some(h) = hwnd() { unsafe { let _ = ShowWindow(h, if on { SW_SHOWNOACTIVATE } else { SW_HIDE }); } } }

    /// Iniciar com o Windows (HKCU\...\Run).
    pub fn set_autostart(on: bool) -> bool {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::*;
        let path = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        unsafe {
            let mut hk = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, &path, None, KEY_SET_VALUE, &mut hk).is_err() { return false; }
            let r = if on {
                let exe = std::env::current_exe().map(|p| format!("\"{}\"", p.display())).unwrap_or_default();
                let w: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = std::slice::from_raw_parts(w.as_ptr() as *const u8, w.len() * 2);
                RegSetValueExW(hk, &HSTRING::from("KonNativo"), None, REG_SZ, Some(bytes)).is_ok()
            } else { let _ = RegDeleteValueW(hk, &HSTRING::from("KonNativo")); true };
            let _ = RegCloseKey(hk);
            r
        }
    }

    /// Volume do Windows (0–100) e mudo, pelo dispositivo de saída padrão.
    pub fn master_volume() -> Option<(u32, bool)> {
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, Endpoints::IAudioEndpointVolume};
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let dev = en.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            let vol: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).ok()?;
            let l = vol.GetMasterVolumeLevelScalar().ok()?;
            let m = vol.GetMute().ok()?.as_bool();
            Some(((l * 100.0).round() as u32, m))
        }
    }

    /// Fotografa o monitor onde está o mouse (pixels físicos, RGBA). Devolve (largura, altura, pixels, x, y do monitor).
    pub fn capture_screen() -> Option<(u32, u32, Vec<u8>, i32, i32)> {
        use windows::Win32::Graphics::Gdi::*;
        unsafe {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let m = MonitorFromPoint(pt, windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if !GetMonitorInfoW(m, &mut mi).as_bool() { return None; }
            let r = mi.rcMonitor;
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp.into());
            let _ = BitBlt(mem, 0, 0, w, h, Some(screen), r.left, r.top, SRCCOPY | CAPTUREBLT);
            let mut bi = BITMAPINFO { bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() }, ..Default::default() };
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(mem, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            if lines == 0 { return None; }
            for px in buf.chunks_exact_mut(4) { px.swap(0, 2); px[3] = 255; } // BGRA → RGBA
            Some((w as u32, h as u32, buf, r.left, r.top))
        }
    }

    /// Põe uma janela nossa (pelo título) exatamente sobre um retângulo em pixels físicos, no topo e com foco.
    pub fn cover_rect(title: &str, x: i32, y: i32, w: i32, h: i32) -> bool {
        let t: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        unsafe {
            match FindWindowW(None, windows::core::PCWSTR(t.as_ptr())) {
                Ok(h2) => { let _ = SetWindowPos(h2, Some(HWND_TOPMOST), x, y, w, h, SWP_SHOWWINDOW); let _ = SetForegroundWindow(h2); true }
                Err(_) => false,
            }
        }
    }

    /// Ditado em português (Windows.Media.SpeechRecognition): ouve uma frase e devolve o texto.
    pub fn dictate() -> Result<String, String> {
        use windows::Media::SpeechRecognition::{SpeechRecognizer, SpeechRecognitionResultStatus};
        use windows::Globalization::Language;
        unsafe { let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED); }
        let msg = |e: windows::core::Error| -> String {
            match e.code().0 as u32 {
                0x80045509 => "Ative \"Reconhecimento de fala online\" em Configurações > Privacidade > Fala".into(),
                0x80070005 => "O Windows bloqueou o microfone: libere em Configurações > Privacidade > Microfone".into(),
                0x8004503A => "Nenhum microfone encontrado".into(),
                _ => format!("Não consegui ouvir ({})", e.message()),
            }
        };
        let rec = Language::CreateLanguage(&windows::core::HSTRING::from("pt-BR")).and_then(|l| SpeechRecognizer::Create(&l)).or_else(|_| SpeechRecognizer::new()).map_err(msg)?;
        let _ = rec.Timeouts().and_then(|t| { t.SetInitialSilenceTimeout(windows::Foundation::TimeSpan { Duration: 60_000_000 })?; t.SetEndSilenceTimeout(windows::Foundation::TimeSpan { Duration: 12_000_000 }) });
        rec.CompileConstraintsAsync().map_err(msg)?.get().map_err(msg)?;
        let r = rec.RecognizeAsync().map_err(msg)?.get().map_err(msg)?;
        let ok = r.Status().map(|s| s == SpeechRecognitionResultStatus::Success).unwrap_or(false);
        if !ok { return Err("Não entendi: tente de novo, mais perto do microfone".into()); }
        Ok(r.Text().map(|t| t.to_string()).unwrap_or_default())
    }

    /// OCR nativo do Windows (Windows.Media.Ocr): português do Brasil se instalado, senão os idiomas do usuário.
    pub fn ocr(w: u32, h: u32, rgba: &[u8]) -> Result<String, String> {
        use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
        use windows::Media::Ocr::OcrEngine;
        use windows::Storage::Streams::DataWriter;
        use windows::Globalization::Language;
        let e = |x: windows::core::Error| x.message().to_string();
        unsafe { let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED); }
        let mut bgra = rgba.to_vec();
        for p in bgra.chunks_exact_mut(4) { p.swap(0, 2); p[3] = 255; }
        let dw = DataWriter::new().map_err(e)?;
        dw.WriteBytes(&bgra).map_err(e)?;
        let buf = dw.DetachBuffer().map_err(e)?;
        let bmp = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(&buf, BitmapPixelFormat::Bgra8, w as i32, h as i32, BitmapAlphaMode::Premultiplied).map_err(e)?;
        let engine = Language::CreateLanguage(&windows::core::HSTRING::from("pt-BR")).ok()
            .filter(|l| OcrEngine::IsLanguageSupported(l).unwrap_or(false))
            .and_then(|l| OcrEngine::TryCreateFromLanguage(&l).ok())
            .or_else(|| OcrEngine::TryCreateFromUserProfileLanguages().ok())
            .ok_or_else(|| "Nenhum idioma de OCR instalado no Windows (Configurações > Idioma > adicionar Português (Brasil))".to_string())?;
        let res = engine.RecognizeAsync(&bmp).map_err(e)?.get().map_err(e)?;
        let lines = res.Lines().map_err(e)?;
        let mut out = vec![];
        for i in 0..lines.Size().map_err(e)? { if let Ok(l) = lines.GetAt(i) { out.push(l.Text().map(|t| t.to_string()).unwrap_or_default()); } }
        Ok(out.join("\n"))
    }

    /// Esconde o Kon da barra de tarefas e do Alt+Tab, e o mantém acima de tudo.
    pub fn make_tool_window() {
        if let Some(h) = hwnd() {
            unsafe {
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(h, GWL_EXSTYLE, ex | (WS_EX_TOOLWINDOW.0 as isize) | (WS_EX_NOACTIVATE.0 as isize));
                let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
    }

    /// Permite (ou não) que o Kon receba o teclado. Fica "sem foco" por padrão para não roubar a janela em que você está.
    pub fn set_focusable(on: bool) {
        if let Some(h) = hwnd() {
            unsafe {
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                let na = WS_EX_NOACTIVATE.0 as isize;
                SetWindowLongPtrW(h, GWL_EXSTYLE, if on { ex & !na } else { ex | na });
                if on { let _ = SetForegroundWindow(h); }
            }
        }
    }

    pub fn data_dir() -> std::path::PathBuf {
        let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("KonNativo")
    }
}

#[cfg(not(windows))]
mod imp {
    use super::PxRect;
    pub fn cursor_in_window() -> Option<(i32, i32)> { None }
    pub fn primary_work_area() -> PxRect { PxRect { x: 0, y: 0, w: 1920, h: 1080 } }
    pub fn make_tool_window() {}
    pub fn place(_follow_cursor: bool) -> bool { false }
    pub fn send_ctrl(_key: u8) {}
    pub fn dnd_get() -> i32 { -1 }
    pub fn dnd_set(_m: i32) -> bool { false }
    pub fn exclude_from_capture(_on: bool) {}
    pub fn foreground_state() -> (bool, bool) { (false, std::env::var("KON_MAX").is_ok()) }
    pub fn set_visible(_on: bool) {}
    pub fn set_autostart(_on: bool) -> bool { false }
    pub fn master_volume() -> Option<(u32, bool)> { None }
    pub fn capture_screen() -> Option<(u32, u32, Vec<u8>, i32, i32)> {
        // Linux (testes): uma imagem sintética para exercitar o recorte
        if std::env::var("KON_OCR_FAKE").is_err() { return None; }
        let (w, h) = (1280u32, 720u32);
        let mut v = vec![0u8; (w * h * 4) as usize];
        for (i, p) in v.chunks_exact_mut(4).enumerate() { let x = (i as u32 % w) as u8; p.copy_from_slice(&[30 + x / 8, 40, 70, 255]); }
        Some((w, h, v, 0, 0))
    }
    pub fn dictate() -> Result<String, String> { Err("O ditado só funciona no Windows".into()) }
    pub fn cover_rect(_t: &str, _x: i32, _y: i32, _w: i32, _h: i32) -> bool { false }
    pub fn ocr(_w: u32, _h: u32, _rgba: &[u8]) -> Result<String, String> {
        if std::env::var("KON_OCR_FAKE").is_ok() { return Ok("Texto de teste lido pelo OCR\nsegunda linha".into()); }
        Err("O OCR nativo só funciona no Windows 10/11".into())
    }
    pub fn set_focusable(_on: bool) {}
    pub fn data_dir() -> std::path::PathBuf { std::env::temp_dir().join("kon-nativo") }
}

pub use imp::*;
