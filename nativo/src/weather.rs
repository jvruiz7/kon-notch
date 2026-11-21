//! Clima (open-meteo.com, sem chave) e localização (por IP, corrigida pelo fuso do Windows).

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct Hour { pub hour: u32, pub temp: i32, pub code: i32, pub is_day: bool, pub rain: i32 }

#[derive(Clone, Debug, Default)]
pub struct Weather {
    pub city: String,
    pub loc_id: String,
    pub temp: i32, pub feels: i32, pub humidity: i32, pub wind: i32,
    pub code: i32, pub is_day: bool, pub max: i32, pub min: i32, pub rain_max: i32,
    pub sunrise: String, pub sunset: String, pub now_iso: String,
    pub hours: Vec<Hour>,
}

/// Mesmo texto e ícone do Kon atual para cada código WMO.
pub fn wmo(code: i32, is_day: bool) -> (&'static str, char) {
    use crate::icons::*;
    match code {
        0 => if is_day { ("Céu limpo", SUN) } else { ("Noite limpa", MOON) },
        1 | 2 => (if code == 1 { "Poucas nuvens" } else { "Parcialmente nublado" }, if is_day { CLOUD_SUN } else { CLOUD_MOON }),
        3 => ("Nublado", CLOUD),
        45 | 48 => ("Neblina", CLOUD_FOG),
        51..=57 => ("Garoa", CLOUD_DRIZZLE),
        61..=67 | 80..=82 => (if code >= 80 { "Pancadas de chuva" } else if code >= 65 { "Chuva forte" } else { "Chuva" }, CLOUD_RAIN),
        71..=77 | 85 | 86 => ("Neve", CLOUD_SNOW),
        c if c >= 95 => ("Tempestade", CLOUD_LIGHTNING),
        _ => ("—", CLOUD),
    }
}

/// Capitais por fuso (a localização por IP às vezes aponta a cidade do provedor).
const TZ_CAPITAL: &[(&str, &str, f64, f64)] = &[
    ("America/Boa_Vista", "Boa Vista", 2.8235, -60.6758), ("America/Manaus", "Manaus", -3.119, -60.0217),
    ("America/Porto_Velho", "Porto Velho", -8.7612, -63.9004), ("America/Rio_Branco", "Rio Branco", -9.9747, -67.8243),
    ("America/Cuiaba", "Cuiabá", -15.601, -56.0974), ("America/Campo_Grande", "Campo Grande", -20.4697, -54.6201),
    ("America/Belem", "Belém", -1.4558, -48.4902), ("America/Fortaleza", "Fortaleza", -3.7319, -38.5267),
    ("America/Recife", "Recife", -8.0476, -34.877), ("America/Maceio", "Maceió", -9.6498, -35.7089),
    ("America/Bahia", "Salvador", -12.9777, -38.5016), ("America/Araguaina", "Palmas", -10.1844, -48.3336),
    ("America/Sao_Paulo", "São Paulo", -23.5505, -46.6333),
];

fn locate() -> (String, f64, f64) {
    let sys_tz = iana_time_zone::get_timezone().unwrap_or_default();
    let ip = ureq::get("https://get.geojs.io/v1/ip/geo.json").timeout(Duration::from_secs(8)).call().ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok());
    if let Some(d) = ip {
        let tz = d["timezone"].as_str().unwrap_or("");
        let lat: f64 = d["latitude"].as_str().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
        let lon: f64 = d["longitude"].as_str().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
        if lat.is_finite() && (tz == sys_tz || !TZ_CAPITAL.iter().any(|c| c.0 == sys_tz)) {
            return (d["city"].as_str().unwrap_or("Localização atual").to_string(), lat, lon);
        }
    }
    let c = TZ_CAPITAL.iter().find(|c| c.0 == sys_tz).unwrap_or(&TZ_CAPITAL[TZ_CAPITAL.len() - 1]);
    (c.1.to_string(), c.2, c.3)
}

fn fetch(city: &str, lat: f64, lon: f64) -> Option<Weather> {
    let u = format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
        &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m\
        &hourly=temperature_2m,precipitation_probability,weather_code,is_day\
        &daily=sunrise,sunset,temperature_2m_max,temperature_2m_min,precipitation_probability_max&forecast_days=2&timezone=auto");
    let d: serde_json::Value = ureq::get(&u).timeout(Duration::from_secs(10)).call().ok()?.into_json().ok()?;
    let c = &d["current"];
    let f = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0).round() as i32;
    let now_iso = c["time"].as_str().unwrap_or("").to_string();
    let times = d["hourly"]["time"].as_array().cloned().unwrap_or_default();
    let start = times.iter().position(|t| t.as_str().map_or(false, |s| s[..13.min(s.len())] > now_iso[..13.min(now_iso.len())])).unwrap_or(0);
    let hours = (start..(start + 6).min(times.len())).map(|i| Hour {
        hour: times[i].as_str().and_then(|s| s.get(11..13)).and_then(|h| h.parse().ok()).unwrap_or(0),
        temp: f(&d["hourly"]["temperature_2m"][i]), code: f(&d["hourly"]["weather_code"][i]),
        is_day: d["hourly"]["is_day"][i].as_i64() == Some(1), rain: f(&d["hourly"]["precipitation_probability"][i]),
    }).collect();
    Some(Weather {
        city: city.to_string(), loc_id: String::new(),
        temp: f(&c["temperature_2m"]), feels: f(&c["apparent_temperature"]), humidity: f(&c["relative_humidity_2m"]),
        wind: f(&c["wind_speed_10m"]), code: f(&c["weather_code"]), is_day: c["is_day"].as_i64() == Some(1),
        max: f(&d["daily"]["temperature_2m_max"][0]), min: f(&d["daily"]["temperature_2m_min"][0]),
        rain_max: f(&d["daily"]["precipitation_probability_max"][0]),
        sunrise: d["daily"]["sunrise"][0].as_str().unwrap_or("").to_string(), sunset: d["daily"]["sunset"][0].as_str().unwrap_or("").to_string(),
        now_iso, hours,
    })
}

/// Cidades fixas (mesma lista do Kon atual), além da localização atual.
pub const BR_CITIES: [(&str, &str, f64, f64, &str); 6] = [
    ("porto-velho", "Porto Velho", -8.7612, -63.9004, "America/Porto_Velho"), ("sao-paulo", "São Paulo", -23.5505, -46.6333, "America/Sao_Paulo"),
    ("brasilia", "Brasília", -15.7939, -47.8828, "America/Sao_Paulo"), ("rio", "Rio de Janeiro", -22.9068, -43.1729, "America/Sao_Paulo"),
    ("manaus", "Manaus", -3.119, -60.0217, "America/Manaus"), ("curitiba", "Curitiba", -25.4284, -49.2733, "America/Sao_Paulo"),
];

/// Busca o clima do lugar escolhido (`loc`: "auto" ou o id de uma cidade); troca na hora quando você muda de cidade.
pub fn start(loc: Arc<Mutex<String>>, repaint: impl Fn() + Send + 'static) -> Arc<Mutex<Option<Weather>>> {
    let st = Arc::new(Mutex::new(None));
    let s2 = st.clone();
    std::thread::spawn(move || {
        let mut auto: Option<(String, f64, f64)> = None;
        loop {
            let id = loc.lock().unwrap().clone();
            let (city, lat, lon) = match BR_CITIES.iter().find(|c| c.0 == id) {
                Some(c) => (c.1.to_string(), c.2, c.3),
                None => auto.get_or_insert_with(locate).clone(),
            };
            let ok = if let Some(mut w) = fetch(&city, lat, lon) { w.loc_id = id.clone(); *s2.lock().unwrap() = Some(w); repaint(); true } else { false };
            // espera 15 min (ou 1 min se falhou), mas acorda na hora se você trocar de cidade
            let wait = if ok { 900 } else { 60 };
            for _ in 0..wait { std::thread::sleep(Duration::from_secs(1)); if *loc.lock().unwrap() != id { break; } }
        }
    });
    st
}
