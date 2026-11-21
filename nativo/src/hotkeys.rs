//! Atalhos globais do Kon atual: Ctrl+Alt+Espaço (Perguntar), Ctrl+Alt+E (painel), Ctrl+Alt+V (Área de transferência),
//! Ctrl+Alt+R (ações no texto selecionado).

use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Copy, Debug)]
pub enum Hot { Ask, Panel, Clip, Text }

pub fn start(repaint: impl Fn() + Send + 'static) -> Receiver<Hot> {
    let (tx, rx) = channel();
    #[cfg(windows)]
    std::thread::spawn(move || unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, VK_SPACE};
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
        let m = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
        // se outro programa (ou o Kon antigo) já usa a combinação, o registro falha e ela só não funciona aqui
        let _ = RegisterHotKey(None, 1, m, VK_SPACE.0 as u32);
        let _ = RegisterHotKey(None, 2, m, 'E' as u32);
        let _ = RegisterHotKey(None, 3, m, 'V' as u32);
        let _ = RegisterHotKey(None, 4, m, 'R' as u32);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                let h = match msg.wParam.0 { 1 => Hot::Ask, 2 => Hot::Panel, 3 => Hot::Clip, _ => Hot::Text };
                if tx.send(h).is_err() { break; }
                repaint();
            }
        }
    });
    #[cfg(not(windows))]
    { let _ = (tx, repaint); }
    rx
}
