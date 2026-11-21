//! Sons do Kon, gerados na hora (sem arquivos), iguais ao sounds.js do Kon atual: três pacotes, seis eventos.

pub const PACKS: [(&str, &str); 3] = [("suave", "Suave"), ("cristal", "Cristal"), ("retro", "Retrô")];

fn notes(ev: &str) -> &'static [(f32, f32, &'static str)] {
    match ev {
        "done" => &[(523.0, 0.0, ""), (784.0, 0.12, "")],
        "attention" => &[(659.0, 0.0, ""), (659.0, 0.18, "")],
        "reminder" => &[(392.0, 0.0, ""), (523.0, 0.11, ""), (659.0, 0.22, "")],
        "meeting" => &[(440.0, 0.0, ""), (880.0, 0.0, "bell")],
        "file" => &[(900.0, 0.0, "pop")],
        "error" => &[(294.0, 0.0, ""), (220.0, 0.14, "")],
        _ => &[],
    }
}

#[derive(Clone, Copy)]
enum Wave { Sine, Triangle, Square }
fn osc(w: Wave, ph: f32) -> f32 {
    let x = ph.fract();
    match w { Wave::Sine => (x * std::f32::consts::TAU).sin(), Wave::Triangle => 4.0 * (x - 0.5).abs() - 1.0, Wave::Square => if x < 0.5 { 1.0 } else { -1.0 } }
}

/// Gera o som em PCM 16 bits mono, 44,1 kHz (mesmo envelope do original: ataque 10 ms, queda exponencial).
pub fn render(ev: &str, pack: &str, volume: f32) -> Vec<i16> {
    let (w, v, d, sh) = match pack { "suave" => (Wave::Sine, 0.22, 1.0, false), "cristal" => (Wave::Triangle, 0.18, 1.3, true), _ => (Wave::Square, 0.06, 0.45, false) };
    let sr = 44_100.0;
    let mut tones: Vec<(f32, f32, f32, Wave, f32, Option<f32>)> = vec![]; // freq, início, duração, onda, volume, freq final
    for (f, t, k) in notes(ev) {
        if *k == "pop" { tones.push((*f, *t, 0.12, w, v, Some(180.0))); continue; }
        let dur = if *k == "bell" { 1.4 } else { 0.35 } * d;
        tones.push((*f, *t, dur, w, if *k == "bell" { v * 0.5 } else { v }, None));
        if sh { tones.push((f * 2.0, *t, dur * 0.6, Wave::Sine, v * 0.25, None)); }
    }
    let total = tones.iter().map(|t| t.1 + t.2 + 0.05).fold(0.0, f32::max);
    let n = (total * sr) as usize;
    let mut buf = vec![0f32; n];
    for (f, t0, dur, wv, vol, f2) in tones {
        let s0 = (t0 * sr) as usize;
        let mut ph = 0.0f32;
        for i in 0..((dur + 0.05) * sr) as usize {
            let t = i as f32 / sr;
            let freq = match f2 { Some(e) => f * (e / f).powf((t / dur).min(1.0)), None => f };
            ph += freq / sr;
            let env = if t < 0.01 { t / 0.01 * vol } else if t <= dur { vol * (0.0001f32 / vol).powf((t - 0.01) / (dur - 0.01)) } else { 0.0 };
            if s0 + i < n { buf[s0 + i] += osc(wv, ph) * env; }
        }
    }
    let g = (volume * 1.4).min(1.0);
    buf.iter().map(|x| (x * g).clamp(-1.0, 1.0) * 32000.0).map(|x| x as i16).collect()
}

fn wav(pcm: &[i16]) -> Vec<u8> {
    let data = pcm.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + data as usize);
    v.extend_from_slice(b"RIFF"); v.extend_from_slice(&(36 + data).to_le_bytes()); v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes()); v.extend_from_slice(&1u16.to_le_bytes()); v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&44_100u32.to_le_bytes()); v.extend_from_slice(&88_200u32.to_le_bytes()); v.extend_from_slice(&2u16.to_le_bytes()); v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data"); v.extend_from_slice(&data.to_le_bytes());
    for s in pcm { v.extend_from_slice(&s.to_le_bytes()); }
    v
}

static LAST: std::sync::Mutex<Vec<(String, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());

/// Toca um evento (ignora repetições em menos de 350 ms, como o original).
pub fn play(ev: &str, pack: &str, volume: f32) {
    if volume <= 0.0 || notes(ev).is_empty() { return; }
    {
        let mut l = LAST.lock().unwrap();
        if let Some(x) = l.iter_mut().find(|x| x.0 == ev) { if x.1.elapsed().as_millis() < 350 { return; } x.1 = std::time::Instant::now(); } else { l.push((ev.into(), std::time::Instant::now())); }
    }
    let data = wav(&render(ev, pack, volume));
    std::thread::spawn(move || {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_SYNC, SND_NODEFAULT};
            let _ = PlaySoundW(windows::core::PCWSTR(data.as_ptr() as *const u16), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT);
        }
        #[cfg(not(windows))]
        { let _ = data; }
    });
}

#[cfg(test)]
mod t {
    #[test] fn gera() { for p in ["suave", "cristal", "retro"] { for e in ["done", "attention", "reminder", "meeting", "file", "error"] { let v = super::render(e, p, 0.6); assert!(v.len() > 1000 && v.iter().any(|x| *x != 0), "{p}/{e}"); } } }
}
