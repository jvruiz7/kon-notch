//! Agenda por assinatura ICS (Google Agenda, Outlook, iCloud), igual ao calendar.js do Kon atual: baixa o .ics,
//! expande recorrências simples e devolve os eventos de ontem até daqui a 7 dias, com o link da reunião.

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const DAY: i64 = 86_400_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Event { pub id: String, pub title: String, pub start: i64, pub end: i64, pub all_day: bool, pub link: Option<String> }

#[derive(Clone, Copy, Debug)]
struct D { t: i64, all_day: bool }

#[derive(Default, Clone, Debug)]
struct Raw { start: Option<D>, end: Option<D>, title: String, location: String, description: String, url: String, uid: String, status: String, rrule: Option<HashMap<String, String>>, recurrence_id: Option<D>, exdates: Vec<i64> }

fn parse_date(v: &str, tzid: Option<&str>) -> Option<D> {
    let v = v.trim();
    if v.len() < 8 { return None; }
    let (y, mo, d) = (v[0..4].parse::<i32>().ok()?, v[4..6].parse::<u32>().ok()?, v[6..8].parse::<u32>().ok()?);
    let date = NaiveDate::from_ymd_opt(y, mo, d)?;
    if v.len() < 13 || &v[8..9] != "T" {
        return Some(D { t: Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest()?.timestamp_millis(), all_day: true });
    }
    let (h, mi) = (v[9..11].parse::<u32>().ok()?, v[11..13].parse::<u32>().ok()?);
    let s = v.get(13..15).and_then(|x| x.parse::<u32>().ok()).unwrap_or(0);
    let nd = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(h, mi, s)?);
    if v.ends_with('Z') { return Some(D { t: chrono::Utc.from_utc_datetime(&nd).timestamp_millis(), all_day: false }); }
    if let Some(tz) = tzid.and_then(|t| t.parse::<chrono_tz::Tz>().ok()) {
        if let Some(x) = tz.from_local_datetime(&nd).earliest() { return Some(D { t: x.timestamp_millis(), all_day: false }); }
    }
    Some(D { t: Local.from_local_datetime(&nd).earliest()?.timestamp_millis(), all_day: false })
}

fn unesc(s: &str) -> String { s.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\") }

fn parse_ics(text: &str) -> Vec<Raw> {
    let unfolded = text.replace("\r\n ", "").replace("\r\n\t", "").replace("\n ", "").replace("\n\t", "");
    let mut out = vec![];
    let mut cur: Option<Raw> = None;
    for raw in unfolded.lines() {
        let raw = raw.trim_end_matches('\r');
        if raw == "BEGIN:VEVENT" { cur = Some(Raw::default()); continue; }
        if raw == "END:VEVENT" { if let Some(c) = cur.take() { if c.start.is_some() { out.push(c); } } continue; }
        let Some(c) = cur.as_mut() else { continue };
        let Some(i) = raw.find(':') else { continue };
        let (head, value) = (&raw[..i], &raw[i + 1..]);
        let mut parts = head.split(';');
        let name = parts.next().unwrap_or("").to_uppercase();
        let params: HashMap<String, String> = parts.filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_uppercase(), v.replace('"', ""))).collect();
        let tz = params.get("TZID").map(|s| s.as_str());
        match name.as_str() {
            "DTSTART" => c.start = parse_date(value, tz), "DTEND" => c.end = parse_date(value, tz),
            "SUMMARY" => c.title = unesc(value), "LOCATION" => c.location = unesc(value), "DESCRIPTION" => c.description = unesc(value),
            "URL" => c.url = value.into(), "UID" => c.uid = value.into(), "STATUS" => c.status = value.into(),
            "RRULE" => c.rrule = Some(value.split(';').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()),
            "RECURRENCE-ID" => c.recurrence_id = parse_date(value, tz),
            "EXDATE" => for v in value.split(',') { if let Some(d) = parse_date(v, tz) { c.exdates.push(d.t); } },
            _ => {}
        }
    }
    out
}

fn add_days_local(t: i64, days: i64, months: i32, years: i32) -> Option<i64> {
    let d = Local.timestamp_millis_opt(t).single()?;
    let (mut y, mut m) = (d.year() + years, d.month() as i32 + months);
    while m > 12 { m -= 12; y += 1; }
    while m < 1 { m += 12; y -= 1; }
    let date = NaiveDate::from_ymd_opt(y, m as u32, d.day())? + chrono::Duration::days(days);
    Local.from_local_datetime(&date.and_hms_opt(d.hour(), d.minute(), 0)?).earliest().map(|x| x.timestamp_millis())
}

fn occurrences(ev: &Raw, from: i64, to: i64) -> (Vec<i64>, i64) {
    let s = ev.start.unwrap();
    let dur = ev.end.map_or(if s.all_day { DAY } else { 30 * 60_000 }, |e| e.t - s.t);
    let mut out = vec![];
    let push = |out: &mut Vec<i64>, t: i64| { if t + dur >= from && t <= to && !ev.exdates.contains(&t) { out.push(t); } };
    let Some(r) = &ev.rrule else { push(&mut out, s.t); return (out, dur) };
    let freq = r.get("FREQ").map(|s| s.as_str()).unwrap_or("");
    let interval: i64 = r.get("INTERVAL").and_then(|x| x.parse().ok()).unwrap_or(1);
    let count: u32 = r.get("COUNT").and_then(|x| x.parse().ok()).unwrap_or(u32::MAX);
    let until = r.get("UNTIL").and_then(|u| parse_date(u, None)).map_or(i64::MAX, |d| d.t);
    let wd = |c: &str| ["SU", "MO", "TU", "WE", "TH", "FR", "SA"].iter().position(|x| c.ends_with(x)).map(|x| x as i64);
    let days: Option<Vec<i64>> = r.get("BYDAY").map(|b| b.split(',').filter_map(wd).collect());
    let sd = Local.timestamp_millis_opt(s.t).single().unwrap();
    let s_wd = sd.weekday().num_days_from_sunday() as i64;
    let mut n = 0u32;
    'outer: for k in 0..2000i64 {
        if n >= count { break; }
        let cands: Vec<i64> = match freq {
            "DAILY" => add_days_local(s.t, k * interval, 0, 0).into_iter().collect(),
            "WEEKLY" => { let mut v: Vec<i64> = days.clone().unwrap_or(vec![s_wd]).iter().filter_map(|w| add_days_local(s.t, -s_wd + k * 7 * interval + w, 0, 0)).collect(); v.sort(); v }
            "MONTHLY" => add_days_local(s.t, 0, (k * interval) as i32, 0).into_iter().collect(),
            "YEARLY" => add_days_local(s.t, 0, 0, (k * interval) as i32).into_iter().collect(),
            _ => { push(&mut out, s.t); break 'outer; }
        };
        for t in cands {
            if t < s.t { continue; }
            if t > until || t > to || n >= count { break 'outer; }
            n += 1; push(&mut out, t);
        }
    }
    (out, dur)
}

fn meet_link(hay: &str) -> Option<String> {
    let re = regex_lite::Regex::new(r#"(?i)https?://(?:meet\.google\.com/[a-z0-9-]+|[\w.-]*zoom\.us/(?:j|my|w)/[^\s"<>\\]+|teams\.microsoft\.com/l/meetup-join/[^\s"<>\\]+|teams\.live\.com/meet/[^\s"<>\\]+|[\w.-]*webex\.com/[^\s"<>\\]+|whereby\.com/[^\s"<>\\]+)"#).ok()?;
    re.find(hay).map(|m| m.as_str().to_string())
}

pub fn events_from(text: &str, now: i64) -> Vec<Event> {
    let (from, to) = (now - DAY, now + 8 * DAY);
    let evs = parse_ics(text);
    let overrides: std::collections::HashSet<String> = evs.iter().filter_map(|e| e.recurrence_id.map(|r| format!("{}|{}", e.uid, r.t))).collect();
    let mk = |e: &Raw, t: i64, dur: i64| Event {
        id: format!("{}|{t}", if e.uid.is_empty() { &e.title } else { &e.uid }), title: if e.title.is_empty() { "(sem título)".into() } else { e.title.clone() },
        start: t, end: t + dur, all_day: e.start.map_or(false, |s| s.all_day), link: meet_link(&[e.location.as_str(), e.url.as_str(), e.description.as_str()].join("\n")),
    };
    let mut list = vec![];
    for e in &evs {
        if e.status == "CANCELLED" { continue; }
        if e.recurrence_id.is_some() { let mut e2 = e.clone(); e2.rrule = None; let (l, d) = occurrences(&e2, from, to); for t in l { list.push(mk(e, t, d)); } continue; }
        let (l, d) = occurrences(e, from, to);
        for t in l { if !overrides.contains(&format!("{}|{t}", e.uid)) { list.push(mk(e, t, d)); } }
    }
    list.sort_by_key(|e| e.start);
    list.truncate(200);
    list
}

#[derive(Default, Clone)]
pub struct State { pub events: Vec<Event>, pub status: String, pub at: Option<std::time::Instant>, pub busy: bool }
pub type Shared = Arc<Mutex<State>>;

/// Baixa a agenda numa thread (cache de 10 min, a não ser que `force`).
pub fn refresh(st: &Shared, url: &str, force: bool, repaint: impl Fn() + Send + 'static) {
    let url = url.trim().to_string();
    {
        let mut g = st.lock().unwrap();
        if url.is_empty() { g.events.clear(); g.status.clear(); return; }
        if g.busy || (!force && g.at.map_or(false, |t| t.elapsed().as_secs() < 600)) { return; }
        g.busy = true;
        if force { g.status = "Atualizando…".into(); }
    }
    let st = st.clone();
    std::thread::spawn(move || {
        let u = if url.to_lowercase().starts_with("webcal") { format!("https{}", &url[url.find("://").unwrap_or(0)..]) } else { url.clone() };
        let res: Result<Vec<Event>, String> = (|| {
            if !(u.starts_with("http://") || u.starts_with("https://")) { return Err("Endereço ICS inválido".into()); }
            let r = ureq::get(&u).set("User-Agent", "Kon").timeout(std::time::Duration::from_secs(20)).call().map_err(|e| match e { ureq::Error::Status(c, _) => format!("A agenda respondeu {c}"), _ => "sem conexão".into() })?;
            let text = r.into_string().map_err(|e| e.to_string())?;
            if !text.contains("BEGIN:VCALENDAR") { return Err("Esse link não é um arquivo .ics".into()); }
            Ok(events_from(&text, chrono::Utc::now().timestamp_millis()))
        })();
        let mut g = st.lock().unwrap();
        g.busy = false; g.at = Some(std::time::Instant::now());
        match res {
            Ok(ev) => { let n = ev.iter().filter(|e| e.start > chrono::Utc::now().timestamp_millis() - DAY).count(); g.events = ev; g.status = format!("Agenda conectada · {n} eventos nos próximos 7 dias"); }
            Err(e) => g.status = format!("Erro: {e}"),
        }
        drop(g); repaint();
    });
}

#[cfg(test)]
mod t {
    #[test] fn ics() {
        let now = chrono::Utc::now().timestamp_millis();
        let d = chrono::Utc::now() + chrono::Duration::hours(2);
        let ics = format!("BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:a\nDTSTART:{}\nDTEND:{}\nSUMMARY:Daily com o time\nDESCRIPTION:Entrar: https://meet.google.com/abc-defg-hij\nEND:VEVENT\nBEGIN:VEVENT\nUID:b\nDTSTART:{}\nRRULE:FREQ=DAILY;COUNT=3\nSUMMARY:Rotina\nEND:VEVENT\nEND:VCALENDAR\n",
            d.format("%Y%m%dT%H%M%SZ"), (d + chrono::Duration::minutes(30)).format("%Y%m%dT%H%M%SZ"), d.format("%Y%m%dT%H%M%SZ"));
        let ev = super::events_from(&ics, now);
        assert_eq!(ev.iter().filter(|e| e.title == "Rotina").count(), 3);
        let daily = ev.iter().find(|e| e.title == "Daily com o time").unwrap();
        assert_eq!(daily.link.as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        assert_eq!(daily.end - daily.start, 30 * 60_000);
    }
}
