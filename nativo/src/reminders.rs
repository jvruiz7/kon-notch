//! Lembretes falados no Perguntar, iguais ao Kon atual:
//! "me lembra em 20 min de ligar pro cliente", "me lembra às 15h30 da reunião",
//! "lembrete amanhã às 9h pagar o boleto", "me lembre daqui a 2 horas de tirar o bolo".

use chrono::{DateTime, Duration, Local, TimeZone, Timelike};
use regex_lite::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rem { pub id: u64, pub text: String, pub at: i64, pub fired: bool }

pub fn parse(input: &str, now: DateTime<Local>) -> Option<(String, i64)> {
    let start = Regex::new(r"(?i)^\s*(?:me\s+)?(?:lembr(?:a|e|ar|ete)(?:-me)?|avis(?:a|e)(?:-me)?)\b[\s:,-]*").unwrap();
    let rel = Regex::new(r"(?i)\b(?:em|daqui\s+a|daqui|dentro\s+de)\s+(\d+(?:[.,]\d+)?|meia)\s*(min(?:uto)?s?|m|h(?:ora)?s?|seg(?:undo)?s?|s)\b(?:\s+e\s+(\d+)\s*(min(?:uto)?s?|m)\b)?").unwrap();
    let abs = Regex::new(r"(?i)(?:^|\s)(amanh[ãa]\s+|hoje\s+)?(?:às|as|ao|a)\s+(\d{1,2})(?:\s*[:h]\s*(\d{2}))?\s*(?:hs|h|horas?)?\b(?:\s*(?:da|de)\s+(manh[ãa]|tarde|noite))?").unwrap();
    let dayword = Regex::new(r"(?i)\b(amanh[ãa])\b").unwrap();
    let m = start.find(input)?;
    let mut rest = input[m.end()..].to_string();
    let at;
    if let Some(c) = rel.captures(&rest) {
        let n: f64 = if c[1].eq_ignore_ascii_case("meia") { 0.5 } else { c[1].replace(',', ".").parse().ok()? };
        let unit = c[2].to_lowercase();
        let mut ms = if unit.starts_with('h') { n * 3_600_000.0 } else if unit.starts_with('s') { n * 1000.0 } else { n * 60_000.0 };
        if let Some(extra) = c.get(3) { ms += extra.as_str().parse::<f64>().ok()? * 60_000.0; }
        at = now.timestamp_millis() + ms as i64;
        let whole = c[0].to_string();
        rest = rest.replacen(&whole, " ", 1);
    } else if let Some(c) = abs.captures(&rest) {
        let mut h: u32 = c[2].parse().ok()?;
        let mi: u32 = c.get(3).map_or(Some(0), |x| x.as_str().parse().ok())?;
        let period = c.get(4).map_or(String::new(), |x| x.as_str().to_lowercase());
        if (period == "tarde" || period == "noite") && h < 12 { h += 12; }
        if h > 23 || mi > 59 { return None; }
        let mut d = now.with_hour(h)?.with_minute(mi)?.with_second(0)?.with_nanosecond(0)?;
        let tomorrow = c.get(1).map_or(false, |x| x.as_str().to_lowercase().starts_with("amanh")) || dayword.is_match(&rest);
        if tomorrow || d <= now { d = d + Duration::days(1); }
        at = d.timestamp_millis();
        let whole = c[0].to_string();
        rest = dayword.replace(&rest.replacen(&whole, " ", 1), " ").to_string();
    } else { return None; }
    let lead = Regex::new(r"(?i)^\s*(?:de|que|da|do|pra|para|a|o)\s+").unwrap();
    let ws = Regex::new(r"\s{2,}").unwrap();
    let mut t = ws.replace_all(&rest, " ").to_string();
    t = lead.replace(&t, "").to_string();
    let t = t.trim_end_matches(|c: char| c.is_whitespace() || ".,;:!?-".contains(c)).trim().to_string();
    let t = if t.is_empty() { "Lembrete".to_string() } else { let mut ch = t.chars(); ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or(t) };
    Some((t, at))
}

pub fn describe(at: i64, now: DateTime<Local>) -> String {
    let d = Local.timestamp_millis_opt(at).single().unwrap_or(now);
    let hm = d.format("%H:%M").to_string();
    let mins = ((at - now.timestamp_millis()) as f64 / 60_000.0).round() as i64;
    if mins < 60 { return format!("em {} min (às {hm})", mins.max(1)); }
    if d.date_naive() == now.date_naive() { format!("hoje às {hm}") } else { format!("amanhã às {hm}") }
}

fn file() -> std::path::PathBuf { crate::platform::data_dir().join("reminders.json") }
pub fn load() -> Vec<Rem> { std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() }
pub fn save(l: &[Rem]) {
    let p = file();
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    if let Ok(s) = serde_json::to_string(l) { let t = p.with_extension("tmp"); if std::fs::write(&t, s).is_ok() { let _ = std::fs::rename(&t, &p); } }
}

#[cfg(test)]
mod t {
    use super::*;
    fn now() -> DateTime<Local> { Local.with_ymd_and_hms(2026, 11, 21, 10, 0, 0).unwrap() }
    #[test] fn frases() {
        let (t, at) = parse("me lembra em 20 min de ligar pro cliente", now()).unwrap();
        assert_eq!((t.as_str(), at - now().timestamp_millis()), ("Ligar pro cliente", 20 * 60_000));
        let (t, at) = parse("me lembra às 15h30 da reunião", now()).unwrap();
        assert_eq!(t, "Reunião"); assert_eq!(Local.timestamp_millis_opt(at).unwrap().format("%d %H:%M").to_string(), "21 15:30");
        let (t, at) = parse("lembrete amanhã às 9h pagar o boleto", now()).unwrap();
        assert_eq!(t, "Pagar o boleto"); assert_eq!(Local.timestamp_millis_opt(at).unwrap().format("%d %H:%M").to_string(), "22 09:00");
        let (_, at) = parse("me lembre daqui a 2 horas de tirar o bolo", now()).unwrap();
        assert_eq!(at - now().timestamp_millis(), 2 * 3_600_000);
        let (_, at) = parse("me avisa às 8 da noite de regar", now()).unwrap();
        assert_eq!(Local.timestamp_millis_opt(at).unwrap().format("%H:%M").to_string(), "20:00");
        assert!(parse("quanto é 2+2", now()).is_none());
        assert!(parse("me lembra de comprar pão", now()).is_none());
    }
}
