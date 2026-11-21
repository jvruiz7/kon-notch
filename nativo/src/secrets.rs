//! Chaves de IA criptografadas pelo Windows (DPAPI, a mesma base do safeStorage do Kon atual):
//! só este usuário, neste computador, consegue abrir. A chave nunca aparece na tela nem em relatórios.

#[cfg(windows)]
pub fn protect(plain: &str) -> String {
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    if plain.is_empty() { return String::new(); }
    let mut bytes = plain.as_bytes().to_vec();
    let input = CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_mut_ptr() };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if CryptProtectData(&input, None, None, None, None, 0, &mut out).is_ok() {
            let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
            return format!("dpapi:{}", b64(&v));
        }
    }
    String::new() // sem criptografia não guarda (nunca em texto puro)
}

#[cfg(windows)]
pub fn unprotect(stored: &str) -> String {
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    let Some(rest) = stored.strip_prefix("dpapi:") else { return String::new() };
    let Some(mut bytes) = unb64(rest) else { return String::new() };
    let input = CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_mut_ptr() };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if CryptUnprotectData(&input, None, None, None, None, 0, &mut out).is_ok() {
            let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
            return String::from_utf8(v).unwrap_or_default();
        }
    }
    String::new()
}

// fora do Windows (só para testar a interface): fica em memória de teste, sem proteção real
#[cfg(not(windows))]
pub fn protect(plain: &str) -> String { if plain.is_empty() { String::new() } else { format!("test:{}", b64(plain.as_bytes())) } }
#[cfg(not(windows))]
pub fn unprotect(stored: &str) -> String { stored.strip_prefix("test:").and_then(unb64).and_then(|v| String::from_utf8(v).ok()).unwrap_or_default() }

const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn b64(d: &[u8]) -> String {
    let mut s = String::new();
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 { if i <= c.len() { s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char) } else { s.push('=') } }
    }
    s
}
fn unb64(s: &str) -> Option<Vec<u8>> {
    let mut out = vec![]; let mut buf = 0u32; let mut bits = 0;
    for ch in s.bytes().filter(|b| *b != b'=') {
        let v = A.iter().position(|a| *a == ch)? as u32;
        buf = buf << 6 | v; bits += 6;
        if bits >= 8 { bits -= 8; out.push((buf >> bits & 0xff) as u8); }
    }
    Some(out)
}

/// Formatos aceitos (iguais ao Kon atual): Gemini AIza… ou AQ.… e Anthropic sk-ant-…
pub fn valid(provider: &str, k: &str) -> bool {
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if provider == "anthropic" { return k.starts_with("sk-ant-") && k.len() >= 27 && k[7..].chars().all(ok); }
    (k.starts_with("AIza") && k.len() == 39 && k[4..].chars().all(ok)) || (k.starts_with("AQ.") && (33..=203).contains(&k.len()) && k[3..].chars().all(|c| ok(c) || c == '.'))
}

#[cfg(test)]
mod t {
    #[test] fn b64() { for s in ["", "a", "ab", "abc", "chave-ç"] { assert_eq!(super::unb64(&super::b64(s.as_bytes())).unwrap(), s.as_bytes()); } }
    #[test] fn ida_e_volta() { let k = "AIzaTESTE-nao-e-chave-de-verdade-1234567"; let e = super::protect(k); assert!(!e.is_empty() && !e.contains(k)); assert_eq!(super::unprotect(&e), k); }
    #[test] fn fmt() { assert!(super::valid("gemini", &format!("AIza{}", "x".repeat(35)))); assert!(super::valid("anthropic", "sk-ant-api03-abcdefghijklmnopqrst")); assert!(!super::valid("gemini", "oi")); }
}
