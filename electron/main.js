// Kon — processo principal do Electron
// Janela overlay transparente 820x560 colada no topo, click-through dinâmico por hit-test do cursor
// (funciona até durante arrastar-e-soltar do Explorer), IA dupla (Gemini + Claude) com chaves no
// safeStorage, gaveta de arquivos ("File Stash"), OCR nativo do Windows e helper de mídia/volume.

const {
  app, BrowserWindow, globalShortcut, ipcMain, screen, desktopCapturer, Tray, Menu,
  nativeImage, clipboard, shell, safeStorage, session,
} = require("electron");
const path = require("path");
const fs = require("fs");
const os = require("os");
const http = require("http");
const crypto = require("crypto");
const { spawn } = require("child_process");
const { computeUsage } = require("./usage");
const { fetchCalendar } = require("./calendar");
let autoUpdater = null;
try { ({ autoUpdater } = require("electron-updater")); } catch (e) { console.warn("updater", e.message); }
const UPDATE_REPO = { owner: "jvruiz7", repo: "kon-notch" };

const DEV = process.env.KON_DEV === "1";
const DEV_URL = "http://localhost:5173";
const WIN_W = 820;
const WIN_H = 560;
const CLAUDE_PORT = 47321;
const CLAUDE_URL = `http://127.0.0.1:${CLAUDE_PORT}/claude`;

let win = null;
let overlay = null;
let tray = null;
let shot = null;
let helper = null;
let helperRestarts = 0;
let hitRects = [];        // regiões clicáveis (coordenadas da janela, CSS px)
let interactive = false;
let pinned = false;       // força a janela a aceitar mouse (ex.: digitando no Ask)
let fgState = { full: false, max: false };
let hiddenForFullscreen = false;
const pendingPerms = new Map(); // pedidos de permissão do Claude Code aguardando você
const pendingOcr = new Map();
const aiControllers = new Map();

const send = (ch, payload) => { if (win && !win.isDestroyed()) win.webContents.send(ch, payload); };
const userFile = (name) => path.join(app.getPath("userData"), name);

/* ═════════ Configurações + chaves criptografadas (safeStorage) ═════════ */
const DEFAULTS = {
  provider: "gemini",
  geminiModel: "gemini-2.5-flash",
  claudeModel: "claude-haiku-5-5",
  webSearch: true,
  autoStart: false,
  devDock: false,
  openMode: "hover",        // hover | delay | click
  notchSize: "normal",      // small | normal | large
  display: "primary",       // primary | cursor
  hideFullscreen: true,     // some em jogos, vídeos e apresentações em tela cheia
  slimOverMax: true,        // vira uma linha fina sobre janelas maximizadas
  hideInCapture: true,      // não aparece em compartilhamento de tela / gravações
  approvals: true,          // aprova pedidos do Claude Code pelo notch
  claudePlan: "auto",       // auto | pro | max5 | max20
  icsUrl: "",
  clipboardOn: true,
  autoUpdate: true,
  onboarded: false,
  geminiKeyEnc: "",
  anthropicKeyEnc: "",
};
function readSettings() {
  let s;
  try { s = { ...DEFAULTS, ...JSON.parse(fs.readFileSync(userFile("settings.json"), "utf8")) }; }
  catch { s = { ...DEFAULTS }; }
  if (!s.modelsV) { if (s.claudeModel === "claude-sonnet-5-5") s.claudeModel = "claude-haiku-5-5"; s.modelsV = 2; } // Haiku 5.5 virou o padrão
  return s;
}
function writeSettings(patch) {
  const next = { ...readSettings(), ...patch };
  fs.mkdirSync(app.getPath("userData"), { recursive: true });
  fs.writeFileSync(userFile("settings.json"), JSON.stringify(next, null, 2));
  if ("autoStart" in patch) app.setLoginItemSettings({ openAtLogin: !!next.autoStart, path: process.env.PORTABLE_EXECUTABLE_FILE || process.execPath });
  return next;
}
function encryptKey(plain) {
  if (!plain) return "";
  if (safeStorage.isEncryptionAvailable()) return "enc:" + safeStorage.encryptString(plain).toString("base64");
  return "b64:" + Buffer.from(plain, "utf8").toString("base64");
}
function decryptKey(stored) {
  if (!stored) return "";
  try {
    if (stored.startsWith("enc:")) return safeStorage.decryptString(Buffer.from(stored.slice(4), "base64"));
    if (stored.startsWith("b64:")) return Buffer.from(stored.slice(4), "base64").toString("utf8");
  } catch {}
  return "";
}
function publicSettings() {
  const s = readSettings();
  const { geminiKeyEnc, anthropicKeyEnc, ...rest } = s;
  return { ...rest, hasGeminiKey: !!decryptKey(geminiKeyEnc), hasAnthropicKey: !!decryptKey(anthropicKeyEnc), secure: safeStorage.isEncryptionAvailable() };
}
ipcMain.handle("settings:get", () => publicSettings());
ipcMain.handle("settings:set", (_e, patch = {}) => {
  const clean = { ...patch };
  delete clean.geminiKeyEnc; delete clean.anthropicKeyEnc;
  writeSettings(clean);
  if ("hideInCapture" in clean) win?.setContentProtection(!!clean.hideInCapture);
  if ("display" in clean) placeWindow();
  if ("hideFullscreen" in clean) applyFullscreen();
  if ("icsUrl" in clean) calCache = null;
  if ("clipboardOn" in clean && !clean.clipboardOn) clipLast = clipboard.readText();
  return publicSettings();
});
ipcMain.handle("keys:set", (_e, { provider, key }) => {
  const field = provider === "anthropic" ? "anthropicKeyEnc" : "geminiKeyEnc";
  writeSettings({ [field]: encryptKey(String(key || "").trim()) });
  return publicSettings();
});

/* ═════════ Janela principal ═════════ */
function load(w, hash = "") {
  if (DEV) w.loadURL(DEV_URL + (hash ? `#${hash}` : ""));
  else w.loadFile(path.join(__dirname, "..", "dist", "index.html"), hash ? { hash } : undefined);
}
let curDisplayId = null;
function placeWindow(display) {
  if (!win || win.isDestroyed()) return;
  const d = display || (readSettings().display === "cursor" ? screen.getDisplayNearestPoint(screen.getCursorScreenPoint()) : screen.getPrimaryDisplay());
  curDisplayId = d.id;
  const { bounds } = d;
  win.setBounds({ x: Math.round(bounds.x + (bounds.width - WIN_W) / 2), y: bounds.y, width: WIN_W, height: WIN_H });
}
function createWindow() {
  const { bounds } = screen.getPrimaryDisplay();
  curDisplayId = screen.getPrimaryDisplay().id;
  win = new BrowserWindow({
    width: WIN_W,
    height: WIN_H,
    x: Math.round(bounds.x + (bounds.width - WIN_W) / 2),
    y: bounds.y,
    transparent: true,
    frame: false,
    backgroundColor: "#00000000",
    hasShadow: false,
    roundedCorners: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    focusable: true,
    alwaysOnTop: true,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false,
    },
  });
  win.setAlwaysOnTop(true, "screen-saver");
  win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
  win.setIgnoreMouseEvents(true, { forward: true });
  win.setContentProtection(readSettings().hideInCapture !== false); // fica fora de prints e de compartilhamento de tela
  load(win);
  win.once("ready-to-show", () => win.showInactive());
  win.on("blur", () => send("win:blurred"));
  win.webContents.setWindowOpenHandler(({ url }) => { if (/^https?:/.test(url)) shell.openExternal(url); return { action: "deny" }; });
  win.webContents.on("will-navigate", (e) => e.preventDefault()); // soltar arquivo fora da drop zone não navega
}

/* Click-through dinâmico: o processo principal consulta o cursor do sistema e só aceita mouse
   sobre as regiões que o renderer declarou (ilha, mascote, sheet). Funciona durante drag do Explorer. */
function setInteractive(on) {
  if (!win || on === interactive) return;
  interactive = on;
  win.setIgnoreMouseEvents(!on, { forward: true });
}
let followTick = 0;
function hitTestLoop() {
  setInterval(() => {
    // modo "seguir o cursor": troca de monitor quando o mouse muda de tela (só com o notch fechado)
    if (++followTick % 12 === 0 && win && !pinned && !interactive && readSettings().display === "cursor") {
      const d = screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
      if (d.id !== curDisplayId) placeWindow(d);
    }
    if (!win || win.isDestroyed() || !win.isVisible()) return;
    if (pinned) return setInteractive(true);
    const p = screen.getCursorScreenPoint();
    const b = win.getBounds();
    const x = p.x - b.x, y = p.y - b.y;
    const inside = hitRects.some((r) => x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h);
    setInteractive(inside);
  }, 40);
}
ipcMain.on("hit:rects", (_e, rects) => { hitRects = Array.isArray(rects) ? rects : []; });
ipcMain.on("win:pin", (_e, on) => { pinned = !!on; if (pinned) setInteractive(true); });
ipcMain.on("win:focus", () => { win?.show(); win?.focus(); });
ipcMain.on("win:blur", () => win?.blur());

/* ═════════ Smart OCR: overlay fullscreen transparente + Windows.Media.Ocr ═════════ */
async function startOcr() {
  if (overlay) return;
  const display = screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
  const { bounds, scaleFactor } = display;
  win?.setOpacity(0);
  await new Promise((r) => setTimeout(r, 90));
  let sources = [];
  try {
    sources = await desktopCapturer.getSources({
      types: ["screen"],
      thumbnailSize: { width: Math.round(bounds.width * scaleFactor), height: Math.round(bounds.height * scaleFactor) },
    });
  } catch (e) { console.warn("capture", e); }
  win?.setOpacity(1);
  const src = sources.find((s) => String(s.display_id) === String(display.id)) || sources[0];
  if (!src) return send("toast", { kind: "error", text: "Não consegui capturar a tela" });
  shot = { img: src.thumbnail, bounds };

  overlay = new BrowserWindow({
    x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height,
    transparent: true, frame: false, backgroundColor: "#00000000", hasShadow: false, roundedCorners: false,
    resizable: false, movable: false, fullscreenable: false, skipTaskbar: true, alwaysOnTop: true, show: false,
    webPreferences: { preload: path.join(__dirname, "preload.js"), contextIsolation: true, nodeIntegration: false },
  });
  overlay.setAlwaysOnTop(true, "screen-saver", 2);
  overlay.setBounds(bounds); // garante cobertura total do monitor com DPI
  load(overlay, "ocr");
  overlay.once("ready-to-show", () => { overlay.show(); overlay.focus(); });
  overlay.on("closed", () => { overlay = null; });
}
ipcMain.on("ocr:cancel", () => { overlay?.close(); shot = null; });
ipcMain.on("ocr:region", async (_e, r) => {
  if (!shot) return;
  const size = shot.img.getSize();
  const k = size.width / shot.bounds.width; // compensa o scaleFactor
  let crop = shot.img.crop({
    x: Math.max(0, Math.round(r.x * k)), y: Math.max(0, Math.round(r.y * k)),
    width: Math.max(1, Math.round(r.w * k)), height: Math.max(1, Math.round(r.h * k)),
  });
  overlay?.close();
  shot = null;
  const cs = crop.getSize();
  // upscale 2x melhora o OCR; limita ao tamanho máximo aceito pelo Windows.Media.Ocr (2600 px)
  const scale = Math.min(2, 2600 / Math.max(cs.width, cs.height));
  if (Math.abs(scale - 1) > 0.01) crop = crop.resize({ width: Math.round(cs.width * scale), height: Math.round(cs.height * scale), quality: "best" });
  const file = path.join(os.tmpdir(), `kon-ocr-${Date.now()}.png`);
  fs.writeFileSync(file, crop.toPNG());
  send("ocr:started");
  try {
    const text = await runOcr(file);
    clipboard.writeText(text);
    send("ocr:result", { text, copied: !!text.trim() });
  } catch (e) {
    send("ocr:result", { text: "", error: e.message || String(e) });
  } finally {
    fs.unlink(file, () => {});
  }
  win?.show(); win?.focus();
});
function runOcr(file) {
  return new Promise((resolve, reject) => {
    if (!helper) return reject(new Error("O OCR nativo só funciona no Windows 10/11"));
    const id = crypto.randomUUID();
    const timer = setTimeout(() => { pendingOcr.delete(id); reject(new Error("OCR demorou demais")); }, 20000);
    pendingOcr.set(id, { resolve: (t) => { clearTimeout(timer); resolve(t); }, reject: (e) => { clearTimeout(timer); reject(e); } });
    helper.stdin.write(JSON.stringify({ cmd: "ocr", id, path: file, lang: "pt-BR" }) + "\n");
  });
}

/* ═════════ Helper nativo (mídia GSMTC + volume + OCR) ═════════ */
function startHelper() {
  if (process.platform !== "win32") return;
  try {
    const target = userFile("kon-helper.ps1");
    fs.mkdirSync(app.getPath("userData"), { recursive: true });
    fs.writeFileSync(target, fs.readFileSync(path.join(__dirname, "kon-helper.ps1"), "utf8"), "utf8");
    helper = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", target], { windowsHide: true });
  } catch (e) { console.error("helper", e); helper = null; return; }
  let buf = "";
  helper.stdout.setEncoding("utf8");
  helper.stdout.on("data", (chunk) => {
    buf += chunk;
    const lines = buf.split(/\r?\n/);
    buf = lines.pop();
    for (const raw of lines) {
      const line = raw.replace(/^﻿/, "").trim();
      if (!line.startsWith("{")) continue;
      let msg; try { msg = JSON.parse(line); } catch { continue; }
      if (msg.type === "media") send("media:state", msg);
      else if (msg.type === "volume") send("volume:state", msg);
      else if (msg.type === "fg") onForeground(msg);
      else if (msg.type === "privacy") send("privacy:state", { mic: [].concat(msg.mic || []).filter(Boolean), cam: [].concat(msg.cam || []).filter(Boolean) });
      else if (msg.type === "ocr") {
        const p = pendingOcr.get(msg.id);
        if (p) { pendingOcr.delete(msg.id); msg.error ? p.reject(new Error(msg.error)) : p.resolve(msg.text || ""); }
      } else if (msg.type === "ready") { helperRestarts = 0; send("helper:ready", msg); }
      else if (msg.type === "error") console.warn("[kon-helper]", msg.scope, msg.message);
    }
  });
  helper.stderr.on("data", (d) => console.warn("[kon-helper:stderr]", String(d)));
  helper.on("exit", () => {
    helper = null;
    for (const p of pendingOcr.values()) p.reject(new Error("helper reiniciado"));
    pendingOcr.clear();
    if (app.isQuitting || helperRestarts > 5) return;
    helperRestarts++;
    setTimeout(startHelper, 2000 * helperRestarts);
  });
}
/* Tela cheia / maximizado: o helper informa a janela em primeiro plano */
function onForeground(m) {
  let onMine = true;
  try {
    const d = screen.getDisplayNearestPoint(screen.screenToDipPoint({ x: m.mx || 0, y: m.my || 0 }));
    onMine = d.id === curDisplayId;
  } catch {}
  const mine = m.pid && (m.pid === process.pid || app.getAppMetrics().some((x) => x.pid === m.pid));
  if (mine) return;
  const next = { full: !!m.full && onMine, max: !!m.max && onMine };
  if (next.full === fgState.full && next.max === fgState.max) return;
  fgState = next;
  send("fg:state", fgState);
  applyFullscreen();
}
function applyFullscreen() {
  if (!win || win.isDestroyed()) return;
  const hide = fgState.full && readSettings().hideFullscreen !== false && pendingPerms.size === 0 && !pinned;
  if (hide && !hiddenForFullscreen) { hiddenForFullscreen = true; win.hide(); }
  else if (!hide && hiddenForFullscreen) { hiddenForFullscreen = false; win.showInactive(); }
}
ipcMain.handle("fg:get", () => fgState);

ipcMain.on("media:cmd", (_e, cmd) => { try { helper?.stdin.write(JSON.stringify(cmd) + "\n"); } catch {} });

/* ═════════ File Stash (sem IA: só referências em kon-stash.json) ═════════ */
const stashPath = () => userFile("kon-stash.json");
function readStash() { try { return JSON.parse(fs.readFileSync(stashPath(), "utf8")); } catch { return []; } }
function writeStash(list) { fs.writeFileSync(stashPath(), JSON.stringify(list, null, 2)); return list; }
ipcMain.handle("stash:list", () => readStash().map((it) => ({ ...it, missing: !fs.existsSync(it.path) })));
ipcMain.handle("stash:add", (_e, paths = []) => {
  const list = readStash();
  const added = [];
  for (const p of paths) {
    if (!p || list.some((it) => it.path === p)) continue;
    let st = null; try { st = fs.statSync(p); } catch {}
    const item = {
      id: crypto.randomUUID(),
      name: path.basename(p),
      path: p,
      size: st && st.isFile() ? st.size : 0,
      ext: st && st.isDirectory() ? "folder" : path.extname(p).replace(".", "").toLowerCase(),
      addedAt: Date.now(),
    };
    list.unshift(item);
    added.push(item);
  }
  writeStash(list);
  return { list, added };
});
ipcMain.handle("stash:remove", (_e, id) => writeStash(readStash().filter((it) => it.id !== id)));
ipcMain.handle("stash:open", async (_e, p) => (await shell.openPath(p)) || "ok");
ipcMain.on("stash:reveal", (_e, p) => shell.showItemInFolder(p));
ipcMain.handle("stash:icon", async (_e, p) => {
  try { return (await app.getFileIcon(p, { size: "normal" })).toDataURL(); } catch { return null; }
});
ipcMain.on("stash:drag", async (e, p) => {
  if (!p || !fs.existsSync(p)) return;
  let icon;
  try { icon = await app.getFileIcon(p, { size: "normal" }); } catch {}
  if (!icon || icon.isEmpty()) icon = nativeImage.createFromPath(path.join(__dirname, "tray.png"));
  e.sender.startDrag({ file: p, icon });
});

/* ═════════ IA dupla: Gemini (@google/genai) + Claude (@anthropic-ai/sdk) ═════════ */
let GoogleGenAI = null;
let Anthropic = null;
try { ({ GoogleGenAI } = require("@google/genai")); } catch (e) { console.warn("genai sdk", e.message); }
try { Anthropic = require("@anthropic-ai/sdk").Anthropic; } catch (e) { console.warn("anthropic sdk", e.message); }
const GEMINI_FALLBACK = "gemini-flash-latest";

function keys() {
  const s = readSettings();
  return { gemini: decryptKey(s.geminiKeyEnc), anthropic: decryptKey(s.anthropicKeyEnc), s };
}
const isGone = (e) => /404|no longer available|not found|is not supported/i.test(String(e?.message || e));

async function geminiStream({ key, model, system, messages, webSearch, signal, onText }) {
  const ai = new GoogleGenAI({ apiKey: key });
  const contents = messages.map((m) => ({ role: m.role === "assistant" ? "model" : "user", parts: m.parts || [{ text: m.text }] }));
  const config = { systemInstruction: system, abortSignal: signal };
  if (webSearch) config.tools = [{ googleSearch: {} }];
  const stream = await ai.models.generateContentStream({ model, contents, config });
  const sources = new Map();
  let full = "";
  let tokens = 0;
  for await (const chunk of stream) {
    if (chunk.usageMetadata?.totalTokenCount) tokens = chunk.usageMetadata.totalTokenCount;
    const t = chunk.text;
    if (t) { full += t; onText(t); }
    const gm = chunk.candidates?.[0]?.groundingMetadata;
    for (const g of gm?.groundingChunks || []) if (g.web?.uri) sources.set(g.web.uri, { url: g.web.uri, title: g.web.title || new URL(g.web.uri).hostname });
  }
  return { full, tokens, sources: [...sources.values()].slice(0, 6) };
}
async function claudeStream({ key, model, system, messages, signal, onText }) {
  const client = new Anthropic({ apiKey: key });
  const stream = client.messages.stream(
    { model, max_tokens: 1200, system, messages: messages.map((m) => ({ role: m.role === "assistant" ? "assistant" : "user", content: m.blocks || m.text })) },
    { signal },
  );
  stream.on("text", (t) => onText(t));
  const final = await stream.finalMessage();
  const full = final.content.filter((c) => c.type === "text").map((c) => c.text).join("");
  const u = final.usage || {};
  return { full, sources: [], tokens: (u.input_tokens || 0) + (u.output_tokens || 0) + (u.cache_creation_input_tokens || 0) + (u.cache_read_input_tokens || 0) };
}
async function runAI({ provider, system, messages, webSearch, signal, onText }) {
  const { gemini, anthropic, s } = keys();
  if (provider === "claude") {
    if (!anthropic) throw Object.assign(new Error("NO_KEY"), { code: "NO_KEY", provider });
    if (!Anthropic) throw new Error("Anthropic SDK indisponível");
    return { ...(await claudeStream({ key: anthropic, model: s.claudeModel, system, messages, signal, onText })), model: s.claudeModel };
  }
  if (!gemini) throw Object.assign(new Error("NO_KEY"), { code: "NO_KEY", provider: "gemini" });
  if (!GoogleGenAI) throw new Error("Gemini SDK indisponível");
  try {
    return { ...(await geminiStream({ key: gemini, model: s.geminiModel, system, messages, webSearch, signal, onText })), model: s.geminiModel };
  } catch (e) {
    if (!isGone(e) || s.geminiModel === GEMINI_FALLBACK) throw e;
    const r = await geminiStream({ key: gemini, model: GEMINI_FALLBACK, system, messages, webSearch, signal, onText });
    return { ...r, model: GEMINI_FALLBACK, fellBack: s.geminiModel };
  }
}
/* Arquivo anexado de propósito (zona "Perguntar à IA"): só é lido aqui, quando você pede */
const TEXT_EXT = /^(txt|md|csv|tsv|json|js|jsx|ts|tsx|py|rs|go|java|c|cpp|h|cs|html|css|xml|yml|yaml|toml|ini|log|sql|sh|ps1|bat)$/i;
const IMG_MIME = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp", gif: "image/gif" };
function readAttachment(p) {
  const st = fs.statSync(p);
  const ext = path.extname(p).slice(1).toLowerCase();
  const name = path.basename(p);
  if (TEXT_EXT.test(ext)) {
    if (st.size > 400e3) throw new Error("Arquivo de texto grande demais (máx. 400 KB)");
    return { kind: "text", name, text: fs.readFileSync(p, "utf8") };
  }
  if (IMG_MIME[ext]) { if (st.size > 5e6) throw new Error("Imagem grande demais (máx. 5 MB)"); return { kind: "image", name, mime: IMG_MIME[ext], data: fs.readFileSync(p).toString("base64") }; }
  if (ext === "pdf") { if (st.size > 10e6) throw new Error("PDF grande demais (máx. 10 MB)"); return { kind: "pdf", name, mime: "application/pdf", data: fs.readFileSync(p).toString("base64") }; }
  throw new Error(`Não sei ler arquivos .${ext || "?"} (texto, imagem ou PDF)`);
}
function attach(messages, file) {
  const a = readAttachment(file);
  const msgs = messages.map((m) => ({ ...m }));
  const last = msgs[msgs.length - 1];
  const q = last.text;
  if (a.kind === "text") {
    last.text = `Arquivo anexado "${a.name}":\n\n${a.text}\n\n---\n${q}`;
  } else {
    last.parts = [{ inlineData: { mimeType: a.mime, data: a.data } }, { text: `(arquivo: ${a.name})\n${q}` }];
    last.blocks = [a.kind === "image" ? { type: "image", source: { type: "base64", media_type: a.mime, data: a.data } } : { type: "document", source: { type: "base64", media_type: a.mime, data: a.data } }, { type: "text", text: `(arquivo: ${a.name})\n${q}` }];
  }
  return msgs;
}
/* tokens gastos pelo chat do próprio Kon (para a página de uso) */
function addKonTokens(n) {
  if (!n) return;
  const f = userFile("kon-usage.json");
  let d = {}; try { d = JSON.parse(fs.readFileSync(f, "utf8")); } catch {}
  const k = new Date().toDateString();
  d[k] = (d[k] || 0) + n;
  const keep = Object.fromEntries(Object.entries(d).slice(-14));
  try { fs.writeFileSync(f, JSON.stringify(keep)); } catch {}
}
ipcMain.on("ai:stream", async (e, { id, provider, system, messages, webSearch, file }) => {
  const ac = new AbortController();
  aiControllers.set(id, ac);
  const out = (type, data) => { if (!e.sender.isDestroyed()) e.sender.send("ai:event", { id, type, ...data }); };
  try {
    if (file) messages = attach(messages, file);
    const r = await runAI({ provider, system, messages, webSearch, signal: ac.signal, onText: (text) => out("chunk", { text }) });
    addKonTokens(r.tokens);
    out("done", { sources: r.sources, model: r.model, fellBack: r.fellBack || null });
  } catch (err) {
    if (ac.signal.aborted) return;
    out("error", { code: err.code || null, provider: err.provider || provider, message: String(err.message || err) });
  } finally { aiControllers.delete(id); }
});
ipcMain.on("ai:abort", (_e, id) => { aiControllers.get(id)?.abort(); aiControllers.delete(id); });
ipcMain.handle("ai:once", async (_e, { provider, system, messages }) => {
  let full = "";
  try {
    const r = await runAI({ provider, system, messages, webSearch: false, onText: (t) => { full += t; } });
    addKonTokens(r.tokens);
    return { ok: true, text: r.full || full };
  } catch (err) { return { ok: false, code: err.code || null, message: String(err.message || err) }; }
});
ipcMain.handle("ai:transcribe", async (_e, { base64, mime }) => {
  const { gemini, s } = keys();
  if (!gemini || !GoogleGenAI) return { ok: false, code: "NO_KEY" };
  const ai = new GoogleGenAI({ apiKey: gemini });
  const req = (model) => ai.models.generateContent({
    model,
    contents: [{ role: "user", parts: [{ inlineData: { mimeType: mime || "audio/webm", data: base64 } }, { text: "Transcreva este áudio em português do Brasil. Responda só com a transcrição." }] }],
  });
  try {
    let r;
    try { r = await req(s.geminiModel); } catch (e) { if (!isGone(e)) throw e; r = await req(GEMINI_FALLBACK); }
    return { ok: true, text: (r.text || "").trim() };
  } catch (e) { return { ok: false, message: String(e.message || e) }; }
});

/* ═════════ Claude Code: servidor local para hooks HTTP ═════════ */
// PermissionRequest fica "segurado" até você responder no notch (Negar / Permitir / Sempre).
// Se você não responder em ~100 s, ou escolher "No terminal", o Kon devolve {} e o Claude Code
// mostra a pergunta normal no terminal. Os outros eventos respondem na hora.
const PERM_HOLD_MS = 100e3;
function permOutput(decision, ev) {
  if (decision === "allow") return { hookSpecificOutput: { hookEventName: "PermissionRequest", decision: { behavior: "allow" } } };
  if (decision === "always") {
    const sugg = Array.isArray(ev.permission_suggestions) && ev.permission_suggestions.length ? ev.permission_suggestions : null;
    const updatedPermissions = sugg || [{ type: "addRules", rules: [{ toolName: ev.tool_name }], behavior: "allow", destination: "localSettings" }];
    return { hookSpecificOutput: { hookEventName: "PermissionRequest", decision: { behavior: "allow", updatedPermissions } } };
  }
  if (decision === "deny") return { hookSpecificOutput: { hookEventName: "PermissionRequest", decision: { behavior: "deny", message: "Negado pelo usuário no Kon." } } };
  return {};
}
function resolvePerm(id, decision) {
  const p = pendingPerms.get(id);
  if (!p) return;
  pendingPerms.delete(id);
  clearTimeout(p.timer);
  try { p.res.writeHead(200, { "Content-Type": "application/json" }); p.res.end(JSON.stringify(permOutput(decision, p.ev))); } catch {}
  send("claude:perm-closed", { id, decision });
  applyFullscreen();
}
ipcMain.on("claude:decide", (_e, { id, decision }) => resolvePerm(id, decision));
function startClaudeServer() {
  const server = http.createServer((req, res) => {
    if (req.method !== "POST" || !req.url.startsWith("/claude")) { res.writeHead(404); return res.end(); }
    let body = "";
    req.on("data", (c) => { body += c; if (body.length > 2e6) req.destroy(); });
    req.on("end", () => {
      let ev = {};
      try { ev = JSON.parse(body || "{}"); } catch {}
      if (ev.hook_event_name === "PermissionRequest" && readSettings().approvals !== false && win && !win.isDestroyed()) {
        const id = crypto.randomUUID();
        const timer = setTimeout(() => resolvePerm(id, null), PERM_HOLD_MS);
        pendingPerms.set(id, { res, ev, timer });
        // o Claude Code desistiu (você respondeu no terminal ou fechou a sessão)
        res.on("close", () => { if (pendingPerms.has(id) && !res.writableEnded) { pendingPerms.delete(id); clearTimeout(timer); send("claude:perm-closed", { id, decision: "gone" }); applyFullscreen(); } });
        applyFullscreen();
        send("claude:event", { ...ev, kon_perm_id: id });
        return;
      }
      send("claude:event", ev);
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end("{}");
    });
  });
  server.on("error", (e) => console.warn("claude server", e.message));
  server.listen(CLAUDE_PORT, "127.0.0.1");
}
const claudeSettingsPath = () => path.join(os.homedir(), ".claude", "settings.json");
const HOOK_EVENTS = ["UserPromptSubmit", "PreToolUse", "PermissionRequest", "Stop", "Notification", "SessionStart", "SessionEnd"];
const HOOK_TIMEOUT = { PermissionRequest: 120 };
const isKonHook = (h) => h && h.type === "http" && typeof h.url === "string" && h.url.includes(`127.0.0.1:${CLAUDE_PORT}/claude`);
function readClaudeSettings() {
  try { return JSON.parse(fs.readFileSync(claudeSettingsPath(), "utf8")); } catch (e) {
    if (e.code === "ENOENT") return {};
    throw new Error("Não consegui ler ~/.claude/settings.json: " + e.message);
  }
}
function stripKon(cfg) {
  const hooks = cfg.hooks || {};
  for (const ev of Object.keys(hooks)) {
    hooks[ev] = (hooks[ev] || []).map((g) => ({ ...g, hooks: (g.hooks || []).filter((h) => !isKonHook(h)) })).filter((g) => g.hooks.length);
    if (!hooks[ev].length) delete hooks[ev];
  }
  if (Object.keys(hooks).length) cfg.hooks = hooks; else delete cfg.hooks;
  return cfg;
}
function writeClaudeSettings(cfg) {
  const p = claudeSettingsPath();
  fs.mkdirSync(path.dirname(p), { recursive: true });
  if (fs.existsSync(p)) fs.copyFileSync(p, p + ".kon-backup");
  fs.writeFileSync(p, JSON.stringify(cfg, null, 2));
}
function installHooks() {
  const cfg = stripKon(readClaudeSettings());
  cfg.hooks = cfg.hooks || {};
  for (const ev of HOOK_EVENTS) {
    const group = { hooks: [{ type: "http", url: CLAUDE_URL, timeout: HOOK_TIMEOUT[ev] || 2 }] };
    if (ev === "PreToolUse" || ev === "PermissionRequest") group.matcher = "";
    cfg.hooks[ev] = [...(cfg.hooks[ev] || []), group];
  }
  writeClaudeSettings(cfg);
}
const hooksState = (cfg) => {
  const has = (ev) => (cfg.hooks?.[ev] || []).some((g) => (g.hooks || []).some(isKonHook));
  return { any: HOOK_EVENTS.some(has), all: HOOK_EVENTS.every(has) };
};
ipcMain.handle("claude:hooks", (_e, action) => {
  try {
    if (action === "install") installHooks();
    else if (action === "uninstall") writeClaudeSettings(stripKon(readClaudeSettings()));
    return { ok: true, installed: hooksState(readClaudeSettings()).any };
  } catch (e) { return { ok: false, installed: false, error: e.message }; }
});
// quem conectou numa versão antiga ganha os eventos novos (permissões, sessões) sem precisar fazer nada
function upgradeHooks() { try { const st = hooksState(readClaudeSettings()); if (st.any && !st.all) installHooks(); } catch {} }

/* ═════════ Uso do Claude Code (transcripts locais) ═════════ */
ipcMain.handle("usage:get", () => {
  try {
    let konTokens = {}; try { konTokens = JSON.parse(fs.readFileSync(userFile("kon-usage.json"), "utf8")); } catch {}
    return computeUsage({ plan: readSettings().claudePlan, konTokens });
  } catch (e) { return { ok: false, error: e.message }; }
});

/* ═════════ Agenda (ICS) ═════════ */
let calCache = null;
ipcMain.handle("cal:get", async (_e, force) => {
  const url = readSettings().icsUrl;
  if (!url) return { ok: true, events: [], empty: true };
  if (!force && calCache && Date.now() - calCache.at < 10 * 60e3) return calCache;
  try { calCache = { ok: true, events: await fetchCalendar(url), at: Date.now() }; }
  catch (e) { return calCache ? { ...calCache, stale: true, error: e.message } : { ok: false, events: [], error: e.message }; }
  return calCache;
});

/* ═════════ Histórico da área de transferência ═════════ */
// Só texto, máx. 40 itens, guardado em kon-clipboard.json. Ignora o que parece senha/chave/cartão.
const CLIP_MAX = 40;
let clipLast = "";
let clipList = null;
const clipPath = () => userFile("kon-clipboard.json");
function clipRead() { if (!clipList) { try { clipList = JSON.parse(fs.readFileSync(clipPath(), "utf8")); } catch { clipList = []; } } return clipList; }
function clipSave() { try { fs.writeFileSync(clipPath(), JSON.stringify(clipList)); } catch {} }
function looksSecret(t) {
  const s = t.trim();
  if (/^(sk-|sk_|pk_|rk_|AIza|ghp_|gho_|github_pat_|xox[abprs]-|glpat-|AKIA|ya29\.|eyJ[\w-]+\.[\w-]+\.)/.test(s)) return true;
  if (/-----BEGIN [A-Z ]*PRIVATE KEY-----/.test(s)) return true;
  const digits = s.replace(/[\s-]/g, "");
  if (/^\d{13,19}$/.test(digits)) return true; // número de cartão
  // uma "palavra" só, longa, misturando maiúsculas, minúsculas, números e símbolos → provável senha/token
  if (!/\s/.test(s) && s.length >= 12 && s.length <= 200 && !/^https?:\/\//.test(s) && /[a-z]/.test(s) && /[A-Z]/.test(s) && /\d/.test(s) && (/[^\w]/.test(s) || s.length >= 24)) return true;
  return false;
}
function clipTick() {
  if (readSettings().clipboardOn === false) return;
  let t = "";
  try { t = clipboard.readText(); } catch { return; }
  if (!t || t === clipLast) return;
  clipLast = t;
  if (!t.trim() || t.length > 20000 || looksSecret(t)) return;
  const list = clipRead().filter((x) => x.text !== t);
  list.unshift({ id: crypto.randomUUID(), text: t, at: Date.now(), pinned: false });
  const pinnedItems = list.filter((x) => x.pinned);
  clipList = [...pinnedItems, ...list.filter((x) => !x.pinned)].slice(0, CLIP_MAX);
  clipSave();
  send("clip:changed", clipList);
}
ipcMain.handle("clip:list", () => clipRead());
ipcMain.handle("clip:copy", (_e, id) => { const it = clipRead().find((x) => x.id === id); if (it) { clipboard.writeText(it.text); clipLast = it.text; } return !!it; });
ipcMain.handle("clip:remove", (_e, id) => { clipList = clipRead().filter((x) => x.id !== id); clipSave(); return clipList; });
ipcMain.handle("clip:pin", (_e, id) => { clipList = clipRead().map((x) => (x.id === id ? { ...x, pinned: !x.pinned } : x)); clipSave(); return clipList; });
ipcMain.handle("clip:clear", () => { clipList = clipRead().filter((x) => x.pinned); clipSave(); return clipList; });

/* ═════════ Compactar (zona de soltar) ═════════ */
ipcMain.handle("zip:make", (_e, paths = []) => new Promise((resolve) => {
  const list = paths.filter((p) => p && fs.existsSync(p));
  if (!list.length) return resolve({ ok: false, error: "Nada para compactar" });
  if (process.platform !== "win32") return resolve({ ok: false, error: "Compactar só funciona no Windows" });
  const dir = path.dirname(list[0]);
  const base = list.length === 1 ? path.basename(list[0], path.extname(list[0])) : "Arquivos";
  let dest = path.join(dir, `${base}.zip`);
  for (let i = 2; fs.existsSync(dest); i++) dest = path.join(dir, `${base} (${i}).zip`);
  const q = (x) => "'" + x.replace(/'/g, "''") + "'";
  const cmd = `Compress-Archive -LiteralPath ${list.map(q).join(",")} -DestinationPath ${q(dest)} -CompressionLevel Optimal`;
  const ps = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", cmd], { windowsHide: true });
  let err = "";
  ps.stderr.on("data", (d) => { err += d; });
  ps.on("exit", (code) => resolve(code === 0 && fs.existsSync(dest) ? { ok: true, path: dest } : { ok: false, error: err.trim().split("\n")[0] || "Falhou ao compactar" }));
}));

/* ═════════ Atualização automática (GitHub Releases) ═════════ */
let updateInfo = null;
function startUpdater() {
  if (!autoUpdater || !app.isPackaged || process.platform !== "win32" || process.env.PORTABLE_EXECUTABLE_FILE) return;
  autoUpdater.setFeedURL({ provider: "github", owner: UPDATE_REPO.owner, repo: UPDATE_REPO.repo });
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;
  autoUpdater.on("update-downloaded", (info) => { updateInfo = { version: info.version }; send("update:ready", updateInfo); });
  autoUpdater.on("error", (e) => console.warn("updater", e?.message || e));
  const check = () => { if (readSettings().autoUpdate !== false) autoUpdater.checkForUpdates().catch(() => {}); };
  setTimeout(check, 20e3);
  setInterval(check, 4 * 3600e3);
}
ipcMain.handle("update:get", () => ({ version: app.getVersion(), ready: updateInfo, portable: !!process.env.PORTABLE_EXECUTABLE_FILE }));
ipcMain.on("update:install", () => { if (updateInfo && autoUpdater) { app.isQuitting = true; autoUpdater.quitAndInstall(true, true); } });
ipcMain.handle("update:check", async () => {
  if (!autoUpdater || !app.isPackaged) return { ok: false, error: "Só no app instalado" };
  if (process.env.PORTABLE_EXECUTABLE_FILE) return { ok: false, error: "A versão portátil não se atualiza sozinha — use o instalador" };
  try { const r = await autoUpdater.checkForUpdates(); const v = r?.updateInfo?.version; return { ok: true, latest: v, newer: v && v !== app.getVersion() }; }
  catch (e) { return { ok: false, error: String(e?.message || e).split("\n")[0] }; }
});

/* ═════════ Cache em disco (stale-while-revalidate) ═════════ */
const cachePath = () => userFile("kon-cache.json");
let cacheMem = null;
function cacheAll() { if (!cacheMem) { try { cacheMem = JSON.parse(fs.readFileSync(cachePath(), "utf8")); } catch { cacheMem = {}; } } return cacheMem; }
ipcMain.handle("cache:get", (_e, key) => cacheAll()[key] ?? null);
ipcMain.handle("cache:set", (_e, key, value) => {
  cacheAll()[key] = value;
  clearTimeout(cacheAll.t);
  cacheAll.t = setTimeout(() => { try { fs.writeFileSync(cachePath(), JSON.stringify(cacheMem)); } catch {} }, 400);
  return true;
});

/* ═════════ Diversos ═════════ */
ipcMain.handle("clipboard:write", (_e, text) => { clipboard.writeText(String(text ?? "")); return true; });
ipcMain.on("open:external", (_e, url) => { if (/^https?:\/\//.test(url)) shell.openExternal(url); });
ipcMain.on("ocr:start", () => startOcr());
ipcMain.on("app:quit", () => app.quit());

function registerShortcuts() {
  const ok = [
    globalShortcut.register("Control+Alt+Space", () => { win?.show(); win?.focus(); send("shortcut", "ask"); }),
    globalShortcut.register("Control+Alt+T", () => startOcr()),
    globalShortcut.register("Control+Alt+E", () => send("shortcut", "panel")),
    globalShortcut.register("Control+Alt+V", () => { win?.show(); send("shortcut", "clip"); }),
  ];
  if (ok.includes(false)) console.warn("Algum atalho global já está em uso por outro programa");
}
function createTray() {
  tray = new Tray(nativeImage.createFromPath(path.join(__dirname, "tray.png")));
  tray.setToolTip("Kon");
  const s = readSettings();
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: "Perguntar ao Kon      Ctrl+Alt+Space", click: () => { win?.show(); win?.focus(); send("shortcut", "ask"); } },
    { label: "Smart OCR             Ctrl+Alt+T", click: () => startOcr() },
    { label: "Abrir painel          Ctrl+Alt+E", click: () => send("shortcut", "panel") },
    { label: "Área de transferência  Ctrl+Alt+V", click: () => send("shortcut", "clip") },
    { label: "Configurações", click: () => { win?.show(); win?.focus(); send("shortcut", "settings"); } },
    { type: "separator" },
    { label: "Iniciar com o Windows", type: "checkbox", checked: !!s.autoStart, click: (i) => writeSettings({ autoStart: i.checked }) },
    { label: "Recarregar", click: () => win?.reload() },
    { type: "separator" },
    { label: "Sair do Kon", click: () => app.quit() },
  ]));
  tray.on("click", () => send("shortcut", "panel"));
}

/* ═════════ Ciclo de vida ═════════ */
if (process.argv.includes("--remove-hooks")) {
  // chamado pelo desinstalador: tira só os hooks do Kon do ~/.claude/settings.json e sai
  try { const cfg = readClaudeSettings(); if (cfg.hooks) writeClaudeSettings(stripKon(cfg)); } catch {}
  app.exit(0);
} else if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", () => send("shortcut", "panel"));
  app.whenReady().then(() => {
    if (process.platform === "win32") app.setAppUserModelId("app.kon.desktop");
    session.defaultSession.setPermissionRequestHandler((_wc, permission, cb) => cb(["media", "clipboard-sanitized-write", "geolocation"].includes(permission)));
    createWindow();
    hitTestLoop();
    registerShortcuts();
    createTray();
    startHelper();
    startClaudeServer();
    upgradeHooks();
    startUpdater();
    clipLast = (() => { try { return clipboard.readText(); } catch { return ""; } })();
    setInterval(clipTick, 800);
    // mantém a opção "Iniciar com o Windows" em sincronia com o que o instalador marcou
    try { const on = app.getLoginItemSettings().openAtLogin; if (on !== !!readSettings().autoStart) writeSettings({ autoStart: on }); } catch {}
    const replace = () => placeWindow();
    screen.on("display-metrics-changed", replace);
    screen.on("display-added", replace);
    screen.on("display-removed", replace);
  });
  app.on("before-quit", () => { app.isQuitting = true; try { helper?.stdin.end(); helper?.kill(); } catch {} });
  app.on("will-quit", () => globalShortcut.unregisterAll());
  app.on("window-all-closed", () => app.quit());
}
