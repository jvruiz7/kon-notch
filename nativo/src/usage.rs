//! Uso do Claude Code: lê os transcripts locais (~/.claude/projects/**/*.jsonl), soma tokens por dia,
//! modelo e sessão, estima custo e a janela de 5 h. Mesma conta do Kon atual. Nada sai do computador.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DAY: i64 = 86_400_000;
const WINDOW: i64 = 5 * 3_600_000;

#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub found: bool, pub plan: String, pub limit: u64,
    pub window: Option<(u64, u32, i64)>, // usado, %, renova em (ms)
    pub today: (u64, f64), pub week: (u64, f64),
    pub days: Vec<(i64, u64)>,
    pub models: Vec<(String, u64, u32)>,
    pub sessions_today: usize,
}

#[derive(Clone)]
struct Entry { key: String, t: i64, model: String, session: String, input: u64, output: u64, cw: u64, cr: u64 }
#[derive(Default)]
struct FileCache { size: u64, offset: u64, rest: Vec<u8>, entries: Vec<Entry> }

#[derive(Default)]
pub struct State { pub usage: Option<Usage>, busy: bool, cache: HashMap<PathBuf, FileCache> }
pub type Shared = Arc<Mutex<State>>;

pub fn new() -> Shared { Arc::new(Mutex::new(State::default())) }

/// Recalcula numa thread (os históricos podem ter centenas de MB; o desenho nunca espera).
pub fn refresh(st: &Shared, plan: String, repaint: impl Fn() + Send + 'static) {
    { let mut g = st.lock().unwrap(); if g.busy { return; } g.busy = true; }
    let st = st.clone();
    std::thread::spawn(move || {
        let mut cache = std::mem::take(&mut st.lock().unwrap().cache);
        let u = compute(&plan, &mut cache);
        let mut g = st.lock().unwrap();
        g.cache = cache; g.usage = Some(u); g.busy = false;
        drop(g); repaint();
    });
}

fn price(m: &str) -> (f64, f64) {
    let m = m.to_lowercase();
    if m.contains("opus-4-0") || m.contains("opus-4-1") || m.contains("opus-4-2025") || m.contains("claude-3-opus") { (15.0, 75.0) }
    else if m.contains("opus") { (5.0, 25.0) }
    else if m.contains("haiku") { if m.contains("3-5") || m.contains("3-haiku") { (0.8, 4.0) } else { (1.0, 5.0) } }
    else { (3.0, 15.0) }
}
fn family(m: &str) -> &'static str { let m = m.to_lowercase(); if m.contains("opus") { "Opus" } else if m.contains("haiku") { "Haiku" } else if m.contains("sonnet") { "Sonnet" } else { "Outros" } }
fn plan_limit(p: &str) -> Option<u64> { match p { "pro" => Some(19_000), "max5" => Some(88_000), "max20" => Some(220_000), _ => None } }

fn roots() -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(env) = std::env::var("CLAUDE_CONFIG_DIR") { for p in env.split([';', ',']) { out.push(PathBuf::from(p.trim()).join("projects")); } }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    out.push(home.join(".claude").join("projects"));
    out.push(home.join(".config").join("claude").join("projects"));
    let mut seen = HashSet::new();
    out.into_iter().filter(|p| p.is_dir() && seen.insert(p.clone())).collect()
}
fn list(dir: &Path, since: i64, acc: &mut Vec<(PathBuf, u64)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() { list(&p, since, acc); }
        else if p.extension().map_or(false, |x| x == "jsonl") {
            let mt = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
            if mt >= since { acc.push((p, md.len())); }
        }
    }
}
fn parse_line(line: &[u8], fallback_session: &str, since: i64, out: &mut Vec<Entry>) {
    if !line.windows(7).any(|w| w == b"\"usage\"") { return; }
    let Ok(o) = serde_json::from_slice::<Value>(line) else { return };
    let msg = &o["message"]; let u = &msg["usage"];
    let model = msg["model"].as_str().unwrap_or("");
    if !u.is_object() || o["type"] != "assistant" || model.is_empty() || model == "<synthetic>" { return; }
    let Some(t) = o["timestamp"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.timestamp_millis()) else { return };
    if t < since { return; }
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    out.push(Entry {
        key: format!("{}:{}", msg["id"].as_str().unwrap_or(""), o["requestId"].as_str().unwrap_or("")), t, model: model.into(),
        session: o["sessionId"].as_str().unwrap_or(fallback_session).into(),
        input: n("input_tokens"), output: n("output_tokens"), cw: n("cache_creation_input_tokens"), cr: n("cache_read_input_tokens"),
    });
}
/// Lê só o que foi acrescentado ao arquivo desde a última vez, em blocos de 8 MB.
fn parse_file(p: &Path, size: u64, since: i64, cache: &mut HashMap<PathBuf, FileCache>) -> Vec<Entry> {
    let c = cache.entry(p.to_path_buf()).or_default();
    if c.size == size && size > 0 { return c.entries.clone(); }
    if size < c.offset { *c = FileCache::default(); }
    let ps = p.to_string_lossy().to_string();
    if let Ok(mut f) = std::fs::File::open(p) {
        if f.seek(SeekFrom::Start(c.offset)).is_ok() {
            let mut buf = vec![0u8; 8 * 1024 * 1024];
            while c.offset < size {
                let Ok(n) = f.read(&mut buf) else { break };
                if n == 0 { break; }
                c.offset += n as u64;
                let mut chunk = std::mem::take(&mut c.rest);
                chunk.extend_from_slice(&buf[..n]);
                let last = chunk.iter().rposition(|&b| b == b'\n');
                let (done, rest) = match last { Some(i) => (chunk[..i].to_vec(), chunk[i + 1..].to_vec()), None => (vec![], chunk) };
                c.rest = rest; // linha ainda sendo escrita
                for line in done.split(|&b| b == b'\n') { parse_line(line, &ps, since, &mut c.entries); }
            }
        }
    }
    c.entries.retain(|e| e.t >= since);
    c.size = size;
    c.entries.clone()
}

fn start_of_day(t: i64) -> i64 {
    use chrono::{Local, TimeZone};
    let d = Local.timestamp_millis_opt(t).single().unwrap_or_else(Local::now).date_naive();
    Local.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap()).earliest().map_or(t, |x| x.timestamp_millis())
}

fn compute(plan: &str, cache: &mut HashMap<PathBuf, FileCache>) -> Usage {
    let now = chrono::Utc::now().timestamp_millis();
    let today0 = start_of_day(now);
    let since = today0 - 7 * DAY;
    let dirs = roots();
    let mut seen = HashSet::new();
    let mut all = vec![];
    for d in &dirs {
        let mut files = vec![]; list(d, since, &mut files);
        for (p, size) in files { for e in parse_file(&p, size, since, cache) { if e.key != ":" && !seen.insert(e.key.clone()) { continue; } all.push(e); } }
    }
    all.sort_by_key(|e| e.t);
    let mut days: Vec<(i64, u64, f64)> = (0..7).map(|i| (start_of_day(today0 - (6 - i) * DAY + DAY / 2), 0, 0.0)).collect();
    let mut models: HashMap<&str, u64> = HashMap::new();
    let mut sessions = HashSet::new();
    let (mut today, mut week) = ((0u64, 0f64), (0u64, 0f64));
    struct B { start: i64, end: i64, last: i64, used: u64 }
    let mut blocks: Vec<B> = vec![];
    for e in &all {
        let (pi, po) = price(&e.model);
        let cost = (e.input as f64 * pi + e.output as f64 * po + e.cw as f64 * pi * 1.25 + e.cr as f64 * pi * 0.1) / 1e6;
        let tokens = e.input + e.output + e.cw + e.cr;
        let sd = start_of_day(e.t);
        if let Some(d) = days.iter_mut().find(|d| d.0 == sd) { d.1 += tokens; d.2 += cost; week.0 += tokens; week.1 += cost; }
        if e.t >= today0 { today.0 += tokens; today.1 += cost; sessions.insert(e.session.clone()); }
        if sd >= days[0].0 { *models.entry(family(&e.model)).or_default() += tokens; }
        let new_block = match blocks.last() { None => true, Some(b) => e.t >= b.end || e.t - b.last >= WINDOW };
        if new_block { let s = e.t / 3_600_000 * 3_600_000; blocks.push(B { start: s, end: s + WINDOW, last: e.t, used: 0 }); }
        let b = blocks.last_mut().unwrap(); b.used += e.input + e.output; b.last = e.t;
    }
    let cur = blocks.last().filter(|b| now < b.end).map(|b| (b.start, b.used, b.end));
    let past_max = blocks.iter().filter(|b| cur.map_or(true, |c| c.0 != b.start)).map(|b| b.used).max().unwrap_or(0);
    let limit = plan_limit(plan).unwrap_or(19_000u64.max(past_max));
    let fam_total = models.values().sum::<u64>().max(1);
    let mut ms: Vec<_> = models.into_iter().map(|(k, v)| (k.to_string(), v, ((v as f64 / fam_total as f64) * 100.0).round() as u32)).collect();
    ms.sort_by(|a, b| b.1.cmp(&a.1));
    Usage {
        found: !dirs.is_empty(), plan: plan.into(), limit,
        window: cur.map(|(_, used, end)| (used, ((used as f64 / limit as f64) * 100.0).round().min(100.0) as u32, end)),
        today, week, days: days.iter().map(|d| (d.0, d.1)).collect(), models: ms, sessions_today: sessions.len(),
    }
}

pub fn fmt_tok(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1} M", n as f64 / 1e6).replace('.', ",") } else if n >= 1000 { format!("{} mil", (n as f64 / 1000.0).round()) } else { n.to_string() }
}
pub fn fmt_usd(x: f64) -> String { format!("US$ {:.2}", x).replace('.', ",") }
pub fn fmt_left(ms: i64) -> String {
    if ms <= 0 { return "agora".into(); }
    let m = (ms as f64 / 60_000.0).ceil() as i64;
    if m >= 60 { format!("{}h{:02}", m / 60, m % 60) } else { format!("{m} min") }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test] fn conta() {
        let dir = std::env::temp_dir().join("kon-usage-test/projects/p");
        let _ = std::fs::create_dir_all(&dir);
        let ts = chrono::Utc::now().to_rfc3339();
        let l = format!(r#"{{"type":"assistant","timestamp":"{ts}","sessionId":"s","requestId":"r1","message":{{"id":"m1","model":"claude-opus-4-5","usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":2000}}}}}}"#);
        std::fs::write(dir.join("a.jsonl"), format!("{l}\n{l}\n")).unwrap();
        std::env::set_var("CLAUDE_CONFIG_DIR", std::env::temp_dir().join("kon-usage-test"));
        let u = compute("pro", &mut HashMap::new());
        assert!(u.found);
        assert_eq!(u.today.0, 3500); // linha repetida conta uma vez só
        assert_eq!(u.window.unwrap().0, 1500);
        assert_eq!(u.models[0].0, "Opus");
        assert_eq!(fmt_tok(3500), "4 mil"); assert_eq!(fmt_left(61 * 60_000), "1h01");
    }
}
