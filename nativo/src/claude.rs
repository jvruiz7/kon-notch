//! Claude Code: o mesmo servidor local de hooks HTTP do Kon atual (127.0.0.1:47321/claude).
//! PermissionRequest fica "segurado" até você responder na ilha (Negar / Permitir / Sempre).
//! Sem resposta em 45 s, ou "No terminal", devolve {} e o Claude Code pergunta no terminal.
//! Nada sai do computador: o servidor só escuta no próprio PC.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PORT: u16 = 47321;
const URL: &str = "http://127.0.0.1:47321/claude";
const EVENTS: [&str; 7] = ["UserPromptSubmit", "PreToolUse", "PermissionRequest", "Stop", "Notification", "SessionStart", "SessionEnd"];
const HOLD: Duration = Duration::from_secs(45);
pub const STAGES: [&str; 4] = ["Lendo", "Editando", "Testando", "Pronto"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status { Idle, Working, Done }

#[derive(Clone, Debug)]
pub struct Live {
    pub status: Status, pub title: String, pub file: String, pub removed: String, pub added: String,
    pub plus: usize, pub minus: usize, pub stage: usize, pub started: Option<Instant>, pub done_at: Option<Instant>, pub note: String,
}
impl Default for Live {
    fn default() -> Self { Live { status: Status::Idle, title: String::new(), file: String::new(), removed: String::new(), added: String::new(), plus: 0, minus: 0, stage: 0, started: None, done_at: None, note: String::new() } }
}

#[derive(Clone, Debug)]
pub struct Session { pub cwd: String, pub status: &'static str, pub at: Instant }

pub struct Perm { pub id: u64, pub tool: String, pub input: Value, pub cwd: String, reply: Sender<Option<&'static str>> }

#[derive(Default)]
pub struct State {
    pub live: Live,
    pub sessions: HashMap<String, Session>,
    pub perms: Vec<Perm>,
    pub listening: bool,
    pub approvals: bool,
    pub just_done: Option<Instant>,
}
pub type Shared = Arc<Mutex<State>>;

pub fn start(approvals: bool, repaint: impl Fn() + Send + Sync + 'static) -> Shared {
    let st = Arc::new(Mutex::new(State { approvals, ..Default::default() }));
    let repaint = Arc::new(repaint);
    let s2 = st.clone();
    std::thread::spawn(move || {
        let Ok(l) = TcpListener::bind(("127.0.0.1", PORT)) else { return }; // outro Kon já está escutando
        s2.lock().unwrap().listening = true;
        upgrade_hooks();
        for c in l.incoming().flatten() {
            let (s3, r3) = (s2.clone(), repaint.clone());
            std::thread::spawn(move || { let _ = handle(c, &s3, &*r3); });
        }
    });
    // as sessões paradas há mais de 6 h somem sozinhas
    st
}

fn handle(mut c: TcpStream, st: &Shared, repaint: &(dyn Fn() + Send + Sync)) -> std::io::Result<()> {
    c.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut r = BufReader::new(c.try_clone()?);
    let mut line = String::new();
    r.read_line(&mut line)?;
    let ok_path = line.starts_with("POST /claude");
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" { break; }
        if let Some((k, v)) = h.split_once(':') { if k.eq_ignore_ascii_case("content-length") { len = v.trim().parse().unwrap_or(0); } }
    }
    if !ok_path || len > 2_000_000 { return reply(&mut c, 404, ""); }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    let ev: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    let name = ev["hook_event_name"].as_str().unwrap_or("").to_string();
    if name == "PermissionRequest" && st.lock().unwrap().approvals {
        let (tx, rx) = channel();
        let id = rand_id();
        {
            let mut g = st.lock().unwrap();
            on_event(&mut g, &name, &ev);
            g.perms.push(Perm { id, tool: s(&ev["tool_name"]), input: ev["tool_input"].clone(), cwd: s(&ev["cwd"]), reply: tx });
        }
        repaint();
        let decision = rx.recv_timeout(HOLD).ok().flatten();
        st.lock().unwrap().perms.retain(|p| p.id != id);
        repaint();
        return reply(&mut c, 200, &perm_output(decision, &ev).to_string());
    }
    { let mut g = st.lock().unwrap(); on_event(&mut g, &name, &ev); }
    repaint();
    reply(&mut c, 200, "{}")
}

fn reply(c: &mut TcpStream, code: u16, body: &str) -> std::io::Result<()> {
    let t = if code == 200 { "OK" } else { "Not Found" };
    write!(c, "HTTP/1.1 {code} {t}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn perm_output(d: Option<&str>, ev: &Value) -> Value {
    match d {
        Some("allow") => json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}}),
        Some("always") => {
            let sugg = ev["permission_suggestions"].as_array().filter(|a| !a.is_empty()).map(|a| Value::Array(a.clone()))
                .unwrap_or_else(|| json!([{"type": "addRules", "rules": [{"toolName": ev["tool_name"]}], "behavior": "allow", "destination": "localSettings"}]));
            json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow", "updatedPermissions": sugg}}})
        }
        Some("deny") => json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "deny", "message": "Negado pelo usuário no Kon."}}}),
        _ => json!({}),
    }
}

pub fn decide(st: &Shared, id: u64, d: Option<&'static str>) {
    let mut g = st.lock().unwrap();
    if let Some(i) = g.perms.iter().position(|p| p.id == id) { let p = g.perms.remove(i); let _ = p.reply.send(d); }
}

fn s(v: &Value) -> String { v.as_str().unwrap_or("").to_string() }
fn rand_id() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) }
pub fn first_line(t: &str) -> String { t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string() }
pub fn base_name(p: &str) -> String { p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("").to_string() }
fn clip(t: String, n: usize) -> String { t.chars().take(n).collect() }
fn lines_of(t: &str) -> usize { t.lines().filter(|l| !l.trim().is_empty()).count() }

fn is_test(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    const W: [&str; 20] = ["test", "jest", "vitest", "pytest", "mocha", "playwright", "cypress", "cargo test", "go test", "npm run test", "npm run lint", "npm run build", "npm run typecheck", "pnpm test", "pnpm build", "yarn test", "yarn build", "tsc", "eslint", "pnpm lint"];
    c.split(|ch: char| !ch.is_alphanumeric()).any(|w| W.contains(&w) || w == "tests") || W.iter().any(|w| w.contains(' ') && c.contains(w))
}

fn on_event(g: &mut State, name: &str, ev: &Value) {
    let now = Instant::now();
    if let Some(sid) = ev["session_id"].as_str() {
        if name == "SessionEnd" { g.sessions.remove(sid); } else {
            let cwd = ev["cwd"].as_str().map(String::from);
            let e = g.sessions.entry(sid.to_string()).or_insert(Session { cwd: String::new(), status: "idle", at: now });
            if let Some(c) = cwd { e.cwd = c; }
            e.status = match name { "Stop" => "done", "Notification" | "PermissionRequest" => "waiting", "SessionStart" => "idle", _ => "working" };
            e.at = now;
        }
    }
    g.sessions.retain(|_, x| now - x.at < Duration::from_secs(6 * 3600));
    if matches!(name, "SessionStart" | "SessionEnd" | "PermissionRequest") { return; }
    let l = &mut g.live;
    let fresh = |title: String| Live { status: Status::Working, title, started: Some(now), ..Default::default() };
    if l.status != Status::Working && matches!(name, "PreToolUse" | "Notification") { let t = if l.title.is_empty() { "Trabalhando…".into() } else { l.title.clone() }; *l = fresh(t); }
    match name {
        "UserPromptSubmit" => { let t = clip(first_line(ev["prompt"].as_str().unwrap_or("")), 90); *l = fresh(if t.is_empty() { "Trabalhando…".into() } else { t }); }
        "PreToolUse" => {
            l.note.clear();
            let tool = ev["tool_name"].as_str().unwrap_or(""); let i = &ev["tool_input"];
            if matches!(tool, "Edit" | "MultiEdit" | "Write" | "NotebookEdit") {
                let e0 = if tool == "MultiEdit" { &i["edits"][0] } else { i };
                let (old, new) = match tool { "Write" => ("", i["content"].as_str().unwrap_or("")), "NotebookEdit" => ("", i["new_source"].as_str().unwrap_or("")), _ => (e0["old_string"].as_str().unwrap_or(""), e0["new_string"].as_str().unwrap_or("")) };
                l.file = base_name(i["file_path"].as_str().or(i["notebook_path"].as_str()).unwrap_or("")).to_uppercase();
                l.removed = clip(first_line(old), 70); l.added = clip(first_line(new), 70);
                l.plus += lines_of(new); l.minus += lines_of(old); l.stage = l.stage.max(1);
            } else if tool == "Bash" && is_test(i["command"].as_str().unwrap_or("")) { l.stage = l.stage.max(2); }
        }
        "Notification" => { l.note = ev["message"].as_str().filter(|m| !m.is_empty()).unwrap_or("O Claude precisa de você").to_string(); }
        "Stop" => if l.status != Status::Idle { l.status = Status::Done; l.stage = 3; l.done_at = Some(now); l.note.clear(); g.just_done = Some(now); },
        _ => {}
    }
}

/// Título e detalhe do pedido de permissão, iguais aos do Kon atual.
pub fn perm_text(p: &Perm) -> (String, String) {
    let t = p.tool.as_str(); let i = &p.input;
    let g = |k: &str| i[k].as_str().unwrap_or("").to_string();
    match t {
        "Bash" | "PowerShell" => ("Claude quer rodar um comando".into(), clip(first_line(&g("command")), 120)),
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
            let f = if g("file_path").is_empty() { g("notebook_path") } else { g("file_path") };
            let b = base_name(&f);
            (format!("Claude quer {} {}", if t == "Write" { "criar" } else { "editar" }, if b.is_empty() { "um arquivo".into() } else { b }), f)
        }
        "WebFetch" => ("Claude quer abrir um site".into(), g("url")),
        "WebSearch" => ("Claude quer pesquisar na web".into(), g("query")),
        "ExitPlanMode" => ("Claude quer sair do modo plano".into(), "e começar a executar o plano".into()),
        _ if t.starts_with("mcp__") => { let parts: Vec<&str> = t.split("__").collect(); (format!("Claude quer usar {}", parts.get(2).unwrap_or(&t)), format!("conector {}", parts.get(1).unwrap_or(&""))) }
        _ => (format!("Claude quer usar {}", if t.is_empty() { "uma ferramenta" } else { t }), clip(i.to_string(), 120)),
    }
}

/* ── hooks no ~/.claude/settings.json (mesmo formato do Kon atual, então os dois são compatíveis) ── */
fn settings_path() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default();
    home.join(".claude").join("settings.json")
}
fn read_cfg() -> Result<Value, String> {
    match std::fs::read_to_string(settings_path()) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("Não consegui ler ~/.claude/settings.json: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("Não consegui ler ~/.claude/settings.json: {e}")),
    }
}
fn is_kon(h: &Value) -> bool { h["type"] == "http" && h["url"].as_str().map_or(false, |u| u.contains("127.0.0.1:47321/claude")) }
fn strip(mut cfg: Value) -> Value {
    if let Some(hooks) = cfg.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_, groups) in hooks.iter_mut() {
            if let Some(arr) = groups.as_array_mut() {
                for g in arr.iter_mut() { if let Some(hs) = g.get_mut("hooks").and_then(|x| x.as_array_mut()) { hs.retain(|h| !is_kon(h)); } }
                arr.retain(|g| g["hooks"].as_array().map_or(false, |h| !h.is_empty()));
            }
        }
        hooks.retain(|_, g| g.as_array().map_or(false, |a| !a.is_empty()));
        if hooks.is_empty() { cfg.as_object_mut().map(|o| o.remove("hooks")); }
    }
    cfg
}
fn write_cfg(cfg: &Value) -> Result<(), String> {
    let p = settings_path();
    if let Some(d) = p.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
    if p.exists() { let _ = std::fs::copy(&p, p.with_extension("json.kon-backup")); }
    std::fs::write(&p, serde_json::to_string_pretty(cfg).unwrap_or_default()).map_err(|e| e.to_string())
}
pub fn hooks_state() -> (bool, bool) {
    let Ok(cfg) = read_cfg() else { return (false, false) };
    let has = |ev: &str| cfg["hooks"][ev].as_array().map_or(false, |a| a.iter().any(|g| g["hooks"].as_array().map_or(false, |h| h.iter().any(is_kon))));
    (EVENTS.iter().any(|e| has(e)), EVENTS.iter().all(|e| has(e)))
}
pub fn install_hooks() -> Result<(), String> {
    let mut cfg = strip(read_cfg()?);
    if !cfg.is_object() { cfg = json!({}); }
    if !cfg["hooks"].is_object() { cfg["hooks"] = json!({}); }
    for ev in EVENTS {
        let mut g = json!({"hooks": [{"type": "http", "url": URL, "timeout": if ev == "PermissionRequest" { 120 } else { 2 }}]});
        if matches!(ev, "PreToolUse" | "PermissionRequest") { g["matcher"] = json!(""); }
        let arr = cfg["hooks"][ev].as_array().cloned().unwrap_or_default();
        let mut arr = arr; arr.push(g);
        cfg["hooks"][ev] = Value::Array(arr);
    }
    write_cfg(&cfg)
}
pub fn uninstall_hooks() -> Result<(), String> { write_cfg(&strip(read_cfg()?)) }
fn upgrade_hooks() { let (any, all) = hooks_state(); if any && !all { let _ = install_hooks(); } }

#[cfg(test)]
mod t {
    use super::*;
    #[test] fn strip_keeps_others() {
        let cfg = json!({"model": "x", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "a"}, {"type": "http", "url": URL}]}], "PreToolUse": [{"matcher": "", "hooks": [{"type": "http", "url": URL}]}]}});
        let s = strip(cfg);
        assert_eq!(s["model"], "x");
        assert_eq!(s["hooks"]["Stop"][0]["hooks"].as_array().unwrap().len(), 1);
        assert!(s["hooks"].get("PreToolUse").is_none());
    }
    #[test] fn testes() { assert!(is_test("npm run test")); assert!(is_test("cargo test --release")); assert!(!is_test("ls -la")); }
    #[test] fn fluxo() {
        let mut g = State::default();
        on_event(&mut g, "UserPromptSubmit", &json!({"prompt": "\nArruma o login\nmais", "session_id": "a"}));
        assert_eq!(g.live.title, "Arruma o login");
        on_event(&mut g, "PreToolUse", &json!({"tool_name": "Edit", "tool_input": {"file_path": "C:\\x\\app.ts", "old_string": "a\nb", "new_string": "c"}}));
        assert_eq!((g.live.file.as_str(), g.live.plus, g.live.minus, g.live.stage), ("APP.TS", 1, 2, 1));
        on_event(&mut g, "Stop", &json!({"session_id": "a"}));
        assert_eq!(g.live.status, Status::Done);
    }
}
