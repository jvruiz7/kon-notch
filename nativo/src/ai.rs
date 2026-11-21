//! Perguntar ao Kon: Gemini (API v1beta, com busca do Google opcional) ou Claude, com a resposta
//! chegando aos poucos (streaming). Mesmas regras do Kon atual, inclusive o plano B quando o modelo some.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const SYSTEM: &str = "Você é o Kon, um assistente conciso que vive no notch do Windows. Responda no idioma do usuário (padrão: português do Brasil). Seja breve (até ~120 palavras), use linhas curtas ou marcadores '• '. Sem títulos markdown, tabelas ou negrito.";
const GEMINI_FALLBACK: &str = "gemini-flash-latest";

#[derive(Clone, Debug, PartialEq)]
pub enum Phase { Idle, Thinking, Streaming, Done }

#[derive(Clone, Debug)]
pub struct Msg { pub user: bool, pub text: String, pub att: Option<Att> }

/// Arquivo anexado de propósito (imagem ou PDF vão inteiros; texto entra na mensagem). Só é lido quando você pergunta.
#[derive(Clone, Debug)]
pub struct Att { pub name: String, pub mime: String, pub data: String, pub pdf: bool }

pub fn b64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity((b.len() + 2) / 3 * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(T[(n >> 18) as usize & 63] as char); o.push(T[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    o
}

const TEXT_EXT: &[&str] = &["txt", "md", "csv", "tsv", "json", "js", "jsx", "ts", "tsx", "py", "rs", "go", "java", "c", "cpp", "h", "cs", "html", "css", "xml", "yml", "yaml", "toml", "ini", "log", "sql", "sh", "ps1", "bat"];
/// Lê o anexo: Ok((texto extra para a mensagem, anexo binário)).
pub fn read_attachment(p: &std::path::Path) -> Result<(Option<String>, Option<Att>), String> {
    let meta = std::fs::metadata(p).map_err(|_| "Não encontrei o arquivo".to_string())?;
    if meta.is_dir() { return Err("Pastas não podem ser enviadas: escolha um arquivo".into()); }
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if TEXT_EXT.contains(&ext.as_str()) {
        if meta.len() > 400_000 { return Err("Arquivo de texto grande demais (máx. 400 KB)".into()); }
        let t = String::from_utf8_lossy(&std::fs::read(p).map_err(|e| e.to_string())?).to_string();
        return Ok((Some(format!("Arquivo anexado \"{name}\":\n\n{t}\n\n---\n")), None));
    }
    let mime = match ext.as_str() { "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "webp" => "image/webp", "gif" => "image/gif", "pdf" => "application/pdf", _ => return Err(format!("Não sei ler arquivos .{} (texto, imagem ou PDF)", if ext.is_empty() { "?" } else { &ext })) };
    let pdf = ext == "pdf";
    if pdf && meta.len() > 10_000_000 { return Err("PDF grande demais (máx. 10 MB)".into()); }
    if !pdf && meta.len() > 5_000_000 { return Err("Imagem grande demais (máx. 5 MB)".into()); }
    Ok((None, Some(Att { name, mime: mime.into(), data: b64(&std::fs::read(p).map_err(|e| e.to_string())?), pdf })))
}

#[derive(Clone, Debug)]
pub struct Ask {
    pub id: u64, pub phase: Phase, pub question: String, pub answer: String,
    pub error: Option<(String, String)>, // (código, provedor)
    pub sources: Vec<(String, String)>, pub fell_back: Option<(String, String)>, pub history: Vec<Msg>,
    pub new_model: Option<String>, pub tokens: u64,
}
impl Default for Ask { fn default() -> Self { Ask { id: 0, phase: Phase::Idle, question: String::new(), answer: String::new(), error: None, sources: vec![], fell_back: None, history: vec![], new_model: None, tokens: 0 } } }
pub type Shared = Arc<Mutex<Ask>>;

pub struct Req { pub provider: String, pub key: String, pub model: String, pub web: bool, pub system: String }

pub fn ask(st: &Shared, q: String, req: Req, repaint: impl Fn() + Send + 'static) { ask_with(st, q, None, None, req, repaint) }

/// Igual a `ask`, com um texto capturado da tela (OCR, conversa nova) ou um arquivo anexado.
pub fn ask_with(st: &Shared, q: String, context: Option<String>, file: Option<std::path::PathBuf>, req: Req, repaint: impl Fn() + Send + 'static) {
    let id = {
        let mut g = st.lock().unwrap();
        g.id += 1;
        let mut history = if context.is_some() { vec![] } else { g.history.clone() };
        let full = match &context { Some(c) => format!("Texto capturado da minha tela:\n\"\"\"\n{c}\n\"\"\"\n\n{q}"), None => q.clone() };
        history.push(Msg { user: true, text: full, att: None });
        *g = Ask { id: g.id, phase: Phase::Thinking, question: q, history, ..Default::default() };
        g.id
    };
    let st = st.clone();
    std::thread::spawn(move || {
        let mut history = st.lock().unwrap().history.clone();
        // o anexo só é lido agora, e vai só para a IA escolhida
        if let Some(f) = &file {
            match read_attachment(f) {
                Ok((text, att)) => if let Some(last) = history.last_mut() {
                    match (text, att) { (Some(t), _) => last.text = format!("{t}{}", last.text), (_, Some(a)) => { last.text = format!("(arquivo: {})\n{}", a.name, last.text); last.att = Some(a); } _ => {} }
                },
                Err(e) => {
                    let mut g = st.lock().unwrap();
                    if g.id == id { g.answer = e; g.error = Some(("FILE".into(), req.provider.clone())); g.history.pop(); g.phase = Phase::Done; }
                    drop(g); repaint(); return;
                }
            }
        }
        let alive = || st.lock().unwrap().id == id;
        let on_text = |t: &str| { let mut g = st.lock().unwrap(); if g.id == id { g.answer.push_str(t); g.phase = Phase::Streaming; } drop(g); repaint(); };
        let res = if req.key.is_empty() { Err(("NO_KEY".to_string(), String::new())) }
            else if req.provider == "claude" { claude(&req, &history, &on_text, &alive) }
            else { gemini_with_fallback(&req, &history, &on_text, &alive) };
        let mut g = st.lock().unwrap();
        if g.id != id { return; }
        match res {
            Ok(r) => { g.sources = r.sources; g.fell_back = r.fell_back.clone(); g.new_model = r.fell_back.map(|f| f.1); g.tokens = r.tokens; let a = g.answer.clone(); if let (Some(l), Some(h)) = (g.history.last_mut(), history.last()) { l.text = h.text.clone(); } g.history.push(Msg { user: false, text: a, att: None }); }
            Err((code, msg)) => {
                let p = if req.provider == "claude" { "Claude" } else { "Gemini" };
                g.answer = if code == "NO_KEY" { format!("Adicione sua chave {} em Configurações para usar o {p}.", if req.provider == "claude" { "ANTHROPIC_API_KEY" } else { "GEMINI_API_KEY" }) } else { format!("{p}: {msg}") };
                g.error = Some((code, req.provider.clone()));
                g.history.pop();
            }
        }
        g.phase = Phase::Done;
        drop(g); repaint();
    });
}

pub fn cancel(st: &Shared) { let mut g = st.lock().unwrap(); g.id += 1; *g = Ask { id: g.id, ..Default::default() }; }

struct Out { sources: Vec<(String, String)>, fell_back: Option<(String, String)>, tokens: u64 }
pub type E = (String, String);

fn http_err(e: ureq::Error) -> E {
    match e {
        ureq::Error::Status(code, r) => {
            let body: Value = r.into_json().unwrap_or(json!({}));
            let msg = body["error"]["message"].as_str().map(String::from).unwrap_or_else(|| format!("erro {code}"));
            (code.to_string(), msg)
        }
        ureq::Error::Transport(t) => ("NET".into(), format!("sem conexão ({})", t.kind())),
    }
}
fn is_gone(e: &E) -> bool { e.0 == "404" || { let m = e.1.to_lowercase(); m.contains("no longer available") || m.contains("not found") || m.contains("is not supported") } }

fn sse(reader: impl std::io::Read, alive: &dyn Fn() -> bool, mut on: impl FnMut(&Value)) {
    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else { break };
        if !alive() { break; } // você fez outra pergunta ou fechou: para de ler
        if let Some(d) = line.strip_prefix("data:") { if let Ok(v) = serde_json::from_str::<Value>(d.trim()) { on(&v); } }
    }
}

fn gemini(req: &Req, model: &str, h: &[Msg], on_text: &dyn Fn(&str), alive: &dyn Fn() -> bool) -> Result<Out, E> {
    let contents: Vec<Value> = h.iter().map(|m| match &m.att {
        Some(a) => json!({"role": "user", "parts": [{"inline_data": {"mime_type": a.mime, "data": a.data}}, {"text": m.text}]}),
        None => json!({"role": if m.user { "user" } else { "model" }, "parts": [{"text": m.text}]}),
    }).collect();
    let mut body = json!({"contents": contents, "systemInstruction": {"parts": [{"text": req.system}]}});
    if req.web { body["tools"] = json!([{"google_search": {}}]); }
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse");
    let r = ureq::post(&url).set("x-goog-api-key", &req.key).timeout(Duration::from_secs(90)).send_json(body).map_err(http_err)?;
    let mut sources: Vec<(String, String)> = vec![]; let mut tokens = 0;
    sse(r.into_reader(), alive, |v| {
        if let Some(t) = v["usageMetadata"]["totalTokenCount"].as_u64() { tokens = t; }
        let c = &v["candidates"][0];
        if let Some(parts) = c["content"]["parts"].as_array() { for p in parts { if let Some(t) = p["text"].as_str() { on_text(t); } } }
        if let Some(gc) = c["groundingMetadata"]["groundingChunks"].as_array() {
            for g in gc { if let Some(u) = g["web"]["uri"].as_str() { if !sources.iter().any(|s| s.0 == u) && sources.len() < 6 { sources.push((u.into(), g["web"]["title"].as_str().unwrap_or(u).into())); } } }
        }
    });
    Ok(Out { sources, fell_back: None, tokens })
}

fn gemini_with_fallback(req: &Req, h: &[Msg], on_text: &dyn Fn(&str), alive: &dyn Fn() -> bool) -> Result<Out, E> {
    match gemini(req, &req.model, h, on_text, alive) {
        Err(e) if is_gone(&e) => {
            // modelo sumiu: tenta o apelido do Flash atual e, se nem ele existir, o Flash mais novo que a chave enxerga
            if req.model != GEMINI_FALLBACK {
                match gemini(req, GEMINI_FALLBACK, h, on_text, alive) {
                    Ok(mut o) => { o.fell_back = Some((req.model.clone(), GEMINI_FALLBACK.into())); return Ok(o); }
                    Err(e2) if !is_gone(&e2) => return Err(e2),
                    _ => {}
                }
            }
            let found = discover_flash(&req.key)?;
            let mut o = gemini(req, &found, h, on_text, alive)?;
            o.fell_back = Some((req.model.clone(), found));
            Ok(o)
        }
        r => r,
    }
}

fn discover_flash(key: &str) -> Result<String, E> {
    let v: Value = ureq::get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=100").set("x-goog-api-key", key).timeout(Duration::from_secs(20)).call().map_err(http_err)?.into_json().map_err(|e| ("NET".into(), e.to_string()))?;
    let mut names: Vec<String> = v["models"].as_array().into_iter().flatten().filter_map(|m| {
        let n = m["name"].as_str()?.trim_start_matches("models/").to_string();
        let acts: Vec<&str> = m["supportedGenerationMethods"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
        let shape = n == "gemini-flash-latest" || n == "gemini-flash-lite-latest" || (n.starts_with("gemini-") && (n.ends_with("-flash") || n.ends_with("-flash-lite")) && n[7..].split('-').next().map_or(false, |v| v.chars().all(|c| c.is_ascii_digit() || c == '.')));
        (shape && (acts.is_empty() || acts.contains(&"generateContent"))).then_some(n)
    }).collect();
    let ver = |n: &str| n.strip_prefix("gemini-").and_then(|r| r.split('-').next()).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
    names.sort_by(|a, b| (a.contains("lite")).cmp(&b.contains("lite")).then(b.contains("latest").cmp(&a.contains("latest"))).then(ver(b).partial_cmp(&ver(a)).unwrap_or(std::cmp::Ordering::Equal)));
    names.into_iter().next().ok_or(("NO_MODEL".into(), "Nenhum modelo Gemini Flash disponível para esta chave".into()))
}

fn claude(req: &Req, h: &[Msg], on_text: &dyn Fn(&str), alive: &dyn Fn() -> bool) -> Result<Out, E> {
    let msgs: Vec<Value> = h.iter().map(|m| match &m.att {
        Some(a) => json!({"role": "user", "content": [{"type": if a.pdf { "document" } else { "image" }, "source": {"type": "base64", "media_type": a.mime, "data": a.data}}, {"type": "text", "text": m.text}]}),
        None => json!({"role": if m.user { "user" } else { "assistant" }, "content": m.text}),
    }).collect();
    let body = json!({"model": req.model, "max_tokens": 1200, "system": req.system, "messages": msgs, "stream": true});
    let r = ureq::post("https://api.anthropic.com/v1/messages").set("x-api-key", &req.key).set("anthropic-version", "2023-06-01")
        .timeout(Duration::from_secs(90)).send_json(body).map_err(http_err)?;
    let mut tokens = 0u64; let mut err = None;
    sse(r.into_reader(), alive, |v| match v["type"].as_str() {
        Some("content_block_delta") => if let Some(t) = v["delta"]["text"].as_str() { on_text(t) },
        Some("message_start") => { let u = &v["message"]["usage"]; tokens += ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"].iter().map(|k| u[k].as_u64().unwrap_or(0)).sum::<u64>(); }
        Some("message_delta") => tokens += v["usage"]["output_tokens"].as_u64().unwrap_or(0),
        Some("error") => err = Some(("API".to_string(), v["error"]["message"].as_str().unwrap_or("erro").to_string())),
        _ => {}
    });
    if let Some(e) = err { return Err(e); }
    Ok(Out { sources: vec![], fell_back: None, tokens })
}

/// Pedido único (tradução, ações no texto): devolve o texto inteiro numa thread e chama `done`.
pub fn once(req: Req, text: String, done: impl FnOnce(Result<String, E>, u64, Option<String>) + Send + 'static) {
    std::thread::spawn(move || {
        let h = vec![Msg { user: true, text, att: None }];
        let out = Mutex::new(String::new());
        let on = |t: &str| out.lock().unwrap().push_str(t);
        let r = if req.key.is_empty() { Err(("NO_KEY".to_string(), String::new())) } else if req.provider == "claude" { claude(&req, &h, &on, &|| true) } else { gemini_with_fallback(&req, &h, &on, &|| true) };
        match r {
            Ok(o) => done(Ok(out.into_inner().unwrap()), o.tokens, o.fell_back.map(|f| f.1)),
            Err(e) => done(Err(e), 0, None),
        }
    });
}

/// Tokens do chat do próprio Kon por dia (aparece na aba Uso do Claude Code).
pub fn add_kon_tokens(n: u64) {
    if n == 0 { return; }
    let f = crate::platform::data_dir().join("kon-usage.json");
    let mut d: serde_json::Map<String, Value> = std::fs::read_to_string(&f).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let k = chrono::Local::now().format("%Y-%m-%d").to_string();
    let cur = d.get(&k).and_then(|v| v.as_u64()).unwrap_or(0);
    d.insert(k, json!(cur + n));
    while d.len() > 14 { let first = d.keys().next().cloned().unwrap(); d.remove(&first); }
    let _ = std::fs::write(f, Value::Object(d).to_string());
}
pub fn kon_tokens_today() -> u64 {
    let f = crate::platform::data_dir().join("kon-usage.json");
    let d: Value = std::fs::read_to_string(f).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
    d[chrono::Local::now().format("%Y-%m-%d").to_string()].as_u64().unwrap_or(0)
}

#[cfg(test)]
mod t {
    use super::*;
    #[test] fn le_sse_do_gemini_e_do_claude() {
        let g = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Olá\"}]}}]}\n\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" mundo\"}]},\"groundingMetadata\":{\"groundingChunks\":[{\"web\":{\"uri\":\"https://a.com\",\"title\":\"A\"}}]}}],\"usageMetadata\":{\"totalTokenCount\":12}}\n";
        let mut out = String::new(); let mut src = 0;
        sse(g.as_bytes(), &|| true, |v| { if let Some(t) = v["candidates"][0]["content"]["parts"][0]["text"].as_str() { out.push_str(t); } if v["candidates"][0]["groundingMetadata"].is_object() { src += 1; } });
        assert_eq!(out, "Olá mundo"); assert_eq!(src, 1);
        let c = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Oi\"}}\n";
        let mut o2 = String::new();
        sse(c.as_bytes(), &|| true, |v| if let Some(t) = v["delta"]["text"].as_str() { o2.push_str(t) });
        assert_eq!(o2, "Oi");
    }
}
