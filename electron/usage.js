// Uso do Claude Code: lê os transcripts locais (~/.claude/projects/**/*.jsonl), soma tokens por dia,
// modelo e sessão, estima custo e a janela de 5 h. Nada sai do computador.
const fs = require("fs");
const path = require("path");
const os = require("os");

const DAY = 864e5;
const WINDOW = 5 * 3600e3;
// Limites aproximados de tokens (entrada + saída, sem cache) por janela de 5 h. A Anthropic não publica
// números oficiais; estes são os usados por monitores populares da comunidade e servem só de referência.
const PLAN_LIMIT = { pro: 19000, max5: 88000, max20: 220000 };
// US$ por milhão de tokens (entrada, saída). Escrita de cache = 1,25× entrada; leitura = 0,1× entrada.
function price(model) {
  const m = String(model || "").toLowerCase();
  if (/opus-4-(0|1)|opus-4-2025|claude-3-opus/.test(m)) return [15, 75];
  if (m.includes("opus")) return [5, 25];
  if (m.includes("haiku")) return /3-5|3-haiku/.test(m) ? [0.8, 4] : [1, 5];
  return [3, 15]; // sonnet e desconhecidos
}
const family = (m) => { m = String(m || "").toLowerCase(); return m.includes("opus") ? "Opus" : m.includes("haiku") ? "Haiku" : m.includes("sonnet") ? "Sonnet" : "Outros"; };

function roots() {
  const out = [];
  const env = process.env.CLAUDE_CONFIG_DIR;
  if (env) for (const p of env.split(/[;,]/)) out.push(path.join(p.trim(), "projects"));
  out.push(path.join(os.homedir(), ".claude", "projects"), path.join(os.homedir(), ".config", "claude", "projects"));
  return [...new Set(out)].filter((p) => { try { return fs.statSync(p).isDirectory(); } catch { return false; } });
}
function listFiles(dir, since, acc = []) {
  let ents = [];
  try { ents = fs.readdirSync(dir, { withFileTypes: true }); } catch { return acc; }
  for (const e of ents) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) listFiles(p, since, acc);
    else if (e.name.endsWith(".jsonl")) { try { const st = fs.statSync(p); if (st.mtimeMs >= since) acc.push({ p, size: st.size, mtime: st.mtimeMs }); } catch {} }
  }
  return acc;
}

// cache por arquivo: só relê o que mudou
const fileCache = new Map();
function parseFile(f, since) {
  const c = fileCache.get(f.p);
  if (c && c.size === f.size && c.mtime === f.mtime) return c.entries;
  const entries = [];
  let text = "";
  try { text = fs.readFileSync(f.p, "utf8"); } catch { return entries; }
  for (const line of text.split("\n")) {
    if (!line.includes('"usage"')) continue;
    let o; try { o = JSON.parse(line); } catch { continue; }
    const msg = o.message;
    const u = msg && msg.usage;
    if (!u || o.type !== "assistant" || !msg.model || msg.model === "<synthetic>") continue;
    const t = Date.parse(o.timestamp);
    if (!t || t < since) continue;
    entries.push({
      key: `${msg.id || ""}:${o.requestId || ""}`, t, model: msg.model, session: o.sessionId || f.p,
      input: u.input_tokens || 0, output: u.output_tokens || 0,
      cw: u.cache_creation_input_tokens || 0, cr: u.cache_read_input_tokens || 0,
    });
  }
  fileCache.set(f.p, { size: f.size, mtime: f.mtime, entries });
  return entries;
}

function startOfDay(t) { const d = new Date(t); d.setHours(0, 0, 0, 0); return d.getTime(); }

function computeUsage({ plan = "auto", konTokens = {} } = {}) {
  const now = Date.now();
  const today0 = startOfDay(now);
  const since = today0 - 7 * DAY;
  const dirs = roots();
  const seen = new Set();
  const all = [];
  for (const dir of dirs) for (const f of listFiles(dir, since)) for (const e of parseFile(f, since)) {
    if (e.key !== ":" && seen.has(e.key)) continue;
    seen.add(e.key);
    all.push(e);
  }
  all.sort((a, b) => a.t - b.t);

  const days = Array.from({ length: 7 }, (_, i) => ({ day: today0 - (6 - i) * DAY, tokens: 0, cost: 0 }));
  const models = {};
  const sessionsToday = new Set();
  let today = { tokens: 0, cost: 0 }, week = { tokens: 0, cost: 0 };
  // janelas de 5 h: começam na hora cheia da primeira mensagem depois que a anterior acabou
  const blocks = [];
  for (const e of all) {
    const [pi, po] = price(e.model);
    const cost = (e.input * pi + e.output * po + e.cw * pi * 1.25 + e.cr * pi * 0.1) / 1e6;
    const tokens = e.input + e.output + e.cw + e.cr;
    const di = Math.floor((startOfDay(e.t) - days[0].day) / DAY);
    if (di >= 0 && di < 7) { days[di].tokens += tokens; days[di].cost += cost; week.tokens += tokens; week.cost += cost; }
    if (e.t >= today0) { today.tokens += tokens; today.cost += cost; sessionsToday.add(e.session); }
    if (di >= 0) { const fam = family(e.model); models[fam] = (models[fam] || 0) + tokens; }
    const last = blocks[blocks.length - 1];
    if (!last || e.t >= last.end || e.t - last.lastAt >= WINDOW) {
      const start = Math.floor(e.t / 3600e3) * 3600e3;
      blocks.push({ start, end: start + WINDOW, lastAt: e.t, used: 0 });
    }
    const b = blocks[blocks.length - 1];
    b.used += e.input + e.output; b.lastAt = e.t;
  }
  const cur = blocks.length && now < blocks[blocks.length - 1].end ? blocks[blocks.length - 1] : null;
  const pastMax = Math.max(0, ...blocks.filter((b) => b !== cur).map((b) => b.used));
  const limit = PLAN_LIMIT[plan] || Math.max(PLAN_LIMIT.pro, pastMax);
  const famTotal = Object.values(models).reduce((a, b) => a + b, 0) || 1;
  const konToday = konTokens[new Date(now).toDateString()] || 0;

  return {
    ok: true,
    found: dirs.length > 0,
    at: now,
    plan, limit, estimated: true,
    window: cur ? { used: cur.used, pct: Math.min(100, Math.round((cur.used / limit) * 100)), resetsAt: cur.end, startedAt: cur.start } : null,
    today, week,
    days: days.map((d) => ({ day: d.day, tokens: d.tokens })),
    models: Object.entries(models).sort((a, b) => b[1] - a[1]).map(([name, tokens]) => ({ name, tokens, pct: Math.round((tokens / famTotal) * 100) })),
    sessionsToday: sessionsToday.size,
    konToday,
  };
}

module.exports = { computeUsage, PLAN_LIMIT };
