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
        // a janela do Kon tem o título "Kon" e é da nossa classe (winit)
        unsafe { FindWindowW(None, w!("Kon")).ok() }
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
    pub fn set_focusable(_on: bool) {}
    pub fn data_dir() -> std::path::PathBuf { std::env::temp_dir().join("kon-nativo") }
}

pub use imp::*;
