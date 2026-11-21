//! Microfone e câmera em uso: o mesmo registro que o Windows usa para o ícone de privacidade
//! (CapabilityAccessManager\ConsentStore). Só lê nomes de programas; nada é gravado nem enviado.

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InUse { pub mic: Vec<String>, pub cam: Vec<String> }

pub fn start(repaint: impl Fn() + Send + 'static) -> Arc<Mutex<InUse>> {
    let st = Arc::new(Mutex::new(InUse::default()));
    let s2 = st.clone();
    std::thread::spawn(move || loop {
        let now = InUse { mic: in_use("microphone").into_iter().filter(|n| !n.to_lowercase().starts_with("kon")).collect(), cam: in_use("webcam") };
        let changed = { let mut g = s2.lock().unwrap(); let c = *g != now; *g = now; c };
        if changed { repaint(); }
        std::thread::sleep(std::time::Duration::from_secs(3));
    });
    st
}

fn pretty(raw: &str) -> String {
    if raw.contains('#') { let last = raw.rsplit('#').next().unwrap_or(raw); last.trim_end_matches(".exe").trim_end_matches(".EXE").to_string() }
    else { let first = raw.split('_').next().unwrap_or(raw); first.rsplit('.').next().unwrap_or(first).to_string() }
}

#[cfg(windows)]
fn in_use(cap: &str) -> Vec<String> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::System::Registry::*;
    let mut out: Vec<String> = vec![];
    let base = format!("Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\{cap}");
    for path in [base.clone(), format!("{base}\\NonPackaged")] {
        unsafe {
            let mut hk = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(path.as_str()), None, KEY_READ, &mut hk).is_err() { continue; }
            let mut i = 0u32;
            loop {
                let mut name = [0u16; 512];
                let mut len = name.len() as u32;
                if RegEnumKeyExW(hk, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None).is_err() { break; }
                i += 1;
                let sub = String::from_utf16_lossy(&name[..len as usize]);
                if sub == "NonPackaged" { continue; }
                let mut sk = HKEY::default();
                if RegOpenKeyExW(hk, &HSTRING::from(sub.as_str()), None, KEY_READ, &mut sk).is_err() { continue; }
                let q = |v: &str| -> Option<u64> {
                    let mut data = 0u64; let mut sz = 8u32;
                    RegQueryValueExW(sk, &HSTRING::from(v), None, None, Some(&mut data as *mut u64 as *mut u8), Some(&mut sz)).ok().ok().map(|_| data)
                };
                if let (Some(a), Some(b)) = (q("LastUsedTimeStart"), q("LastUsedTimeStop")) {
                    if a > 0 && b == 0 { let n = pretty(&sub); if !n.is_empty() && !out.contains(&n) { out.push(n); } }
                }
                let _ = RegCloseKey(sk);
            }
            let _ = RegCloseKey(hk);
        }
    }
    out
}
#[cfg(not(windows))]
fn in_use(_cap: &str) -> Vec<String> { vec![] }

#[cfg(test)]
mod t {
    #[test] fn nomes() {
        assert_eq!(super::pretty("C:#Program Files#Zoom#bin#Zoom.exe"), "Zoom");
        assert_eq!(super::pretty("Microsoft.WindowsCamera_8wekyb3d8bbwe"), "WindowsCamera");
    }
}
