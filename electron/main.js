// Kon — processo principal do Electron
// Janela overlay transparente 820x560 colada no topo, click-through dinâmico por hit-test do cursor
// (funciona até durante arrastar-e-soltar do Explorer), IA dupla (Gemini + Claude) com chaves no
// safeStorage, gaveta de arquivos ("File Stash"), OCR nativo do Windows e helper de mídia/volume.

const {
  app, BrowserWindow, globalShortcut, ipcMain, screen, desktopCapturer, Tray, Menu,
  nativeImage, clipboard, shell, safeStorage, session, Notification,
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
const pendingHelper = new Map(); // respostas do helper por id (teclado, áudio, sistema)
function helperCall(cmd, timeout = 3000) {
  return new Promise((resolve) => {
    if (!helper) return resolve(null);
    const id = crypto.randomUUID();
    const t = setTimeout(() => { pendingHelper.delete(id); resolve(null); }, timeout);
    pendingHelper.set(id, (m) => { clearTimeout(t); resolve(m); });
    try { helper.stdin.write(JSON.stringify({ ...cmd, id }) + "\n"); } catch { clearTimeout(t); resolve(null); }
  });
}
const aiControllers = new Map();

// Nenhum erro inesperado do processo principal deve virar caixa de diálogo: registra em kon-error.log
process.on("uncaughtException", (e) => {
  try { fs.appendFileSync(path.join(app.getPath("userData"), "kon-error.log"), `${new Date().toISOString()} ${e?.stack || e}\n`); } catch {}
  console.error(e);
});
process.on("unhandledRejection", (e) => { try { fs.appendFileSync(path.join(app.getPath("userData"), "kon-error.log"), `${new Date().toISOString()} rejection ${e?.stack || e}\n`); } catch {} });

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
  sysAlerts: true,          // avisa quando CPU/memória ficam no limite por 30 s
  focusDnd: true,           // liga o Não incomodar do Windows durante o Focus
  wakeWord: false,          // "Kon" abre o Perguntar já ouvindo (reconhecimento de voz do Windows)
  wakeConf: 0.6,
  accent: "aurora",
  glow: 1,
  onboarded: false,
  geminiKeyEnc: "",
  anthropicKeyEnc: "",
};
let settingsCache = null; // lido muitas vezes por segundo (hit-test, clipboard): fica em memória
function readSettings() {
  if (settingsCache) return { ...settingsCache };
  let s;
  try { s = { ...DEFAULTS, ...JSON.parse(fs.readFileSync(userFile("settings.json"), "utf8")) }; }
  catch { s = { ...DEFAULTS }; }
  if (!s.modelsV) { if (s.claudeModel === "claude-sonnet-5-5") s.claudeModel = "claude-haiku-5-5"; s.modelsV = 2; } // Haiku 5.5 virou o padrão
  settingsCache = s;
  return { ...s };
}
function writeSettings(patch) {
  const next = { ...readSettings(), ...patch };
  fs.mkdirSync(app.getPath("userData"), { recursive: true });
  fs.writeFileSync(userFile("settings.json"), JSON.stringify(next, null, 2));
  settingsCache = next;
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
  if ("wakeWord" in clean || "wakeConf" in clean) applyWake();
  if ("clipboardOn" in clean && !clean.clipboardOn) clipLast = readClipText();
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
  win.once("ready-to-show", () => { win.showInactive(); setKonFocusable(pinned); });
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
// O Kon só pega o foco do teclado quando você vai digitar (Perguntar, Ajustes, OCR, nova tarefa).
// Clicar nos botões do notch não rouba o foco da janela em que você estava trabalhando.
function setKonFocusable(on) { try { if (win && !win.isDestroyed()) win.setFocusable(!!on); } catch {} }
ipcMain.on("win:pin", (_e, on) => { pinned = !!on; setKonFocusable(pinned); if (pinned) setInteractive(true); else applyFullscreen(); });
function focusKon() { setKonFocusable(true); win?.show(); win?.focus(); }
ipcMain.on("win:focus", () => focusKon());
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
  focusKon();
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
      else if (msg.type === "wake") { if (!pinned) { focusKon(); send("shortcut", "wake"); } }
      else if (msg.id && pendingHelper.has(msg.id)) { const p = pendingHelper.get(msg.id); pendingHelper.delete(msg.id); p(msg); }
      else if (msg.type === "privacy") {
        // com a palavra de ativação ligada, o próprio Kon (via PowerShell) usa o microfone: não conta
        const wake = readSettings().wakeWord;
        const own = (n) => /^kon$/i.test(n) || (wake && /^powershell$/i.test(n));
        send("privacy:state", { mic: [].concat(msg.mic || []).filter((n) => n && !own(n)), cam: [].concat(msg.cam || []).filter(Boolean) });
      }
      else if (msg.type === "ocr") {
        const p = pendingOcr.get(msg.id);
        if (p) { pendingOcr.delete(msg.id); msg.error ? p.reject(new Error(msg.error)) : p.resolve(msg.text || ""); }
      } else if (msg.type === "ready") { helperRestarts = 0; send("helper:ready", msg); applyWake(); }
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
  const hide = fgState.full && readSettings().hideFullscreen !== false && pendingPerms.size === 0 && dueReminders === 0 && !pinned;
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
// Se você não responder em ~45 s, ou escolher "No terminal", o Kon devolve {} e o Claude Code
// mostra a pergunta normal no terminal. Os outros eventos respondem na hora.
const PERM_HOLD_MS = 45e3; // depois disso a pergunta volta para o terminal
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
  const s = String(t || "").trim();
  if (/^(sk-|sk_|pk_|rk_|AIza|ghp_|gho_|github_pat_|xox[abprs]-|glpat-|AKIA|ya29\.|eyJ[\w-]+\.[\w-]+\.)/.test(s)) return true;
  if (/-----BEGIN [A-Z ]*PRIVATE KEY-----/.test(s)) return true;
  const digits = s.replace(/[\s-]/g, "");
  if (/^\d{13,19}$/.test(digits)) return true; // número de cartão
  // uma "palavra" só, longa, misturando maiúsculas, minúsculas, números e símbolos → provável senha/token
  if (!/\s/.test(s) && s.length >= 12 && s.length <= 200 && !/^https?:\/\//.test(s) && /[a-z]/.test(s) && /[A-Z]/.test(s) && /\d/.test(s) && (/[^\w]/.test(s) || s.length >= 24)) return true;
  return false;
}
function readClipText() {
  try { const v = clipboard.readText(); return typeof v === "string" ? v : v == null ? "" : String(v); } catch { return ""; }
}
function clipTick() {
  try { clipTickUnsafe(); } catch (e) { console.warn("clipboard", e?.message || e); }
}
function clipTickUnsafe() {
  if (readSettings().clipboardOn === false || Date.now() < clipSuspendUntil) return;
  const t = readClipText();
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

/* ═════════ Ações no texto selecionado (Ctrl+Alt+R) ═════════ */
// Copia a seleção do app em primeiro plano, devolve sua área de transferência como estava e abre o menu
// de ações no notch. "Colar" escreve o resultado e manda Ctrl+V para o mesmo app (o Kon não rouba o foco).
let clipSuspendUntil = 0;
async function grabSelection() {
  if (!helper) return send("textact:open", { text: "", error: "Só funciona no Windows" });
  const before = readClipText();
  clipSuspendUntil = Date.now() + 2500;
  try { clipboard.writeText(""); } catch {}
  await helperCall({ cmd: "copysel" }, 2500);
  let text = "";
  for (let i = 0; i < 20 && !text; i++) { await new Promise((r) => setTimeout(r, 40)); text = readClipText(); }
  try { clipboard.writeText(before); } catch {}
  clipLast = before;
  send("textact:open", text.trim() ? { text: text.slice(0, 12000) } : { text: "", error: "Selecione um texto em qualquer programa e aperte Ctrl+Alt+R" });
}
ipcMain.handle("textact:paste", async (_e, text) => {
  clipSuspendUntil = Date.now() + 1500;
  clipboard.writeText(String(text || ""));
  clipLast = String(text || "");
  const r = await helperCall({ cmd: "paste" }, 2500);
  return !!r?.ok;
});

/* ═════════ Palavra de ativação "Kon" ═════════ */
let wakeState = { on: false };
async function applyWake() {
  const st = readSettings();
  const r = await helperCall({ cmd: "wake", on: !!st.wakeWord, conf: st.wakeConf || 0.6 }, 8000);
  wakeState = { on: !!(st.wakeWord && r?.ok), error: st.wakeWord && !r?.ok ? (r?.error || (r?.result === "sem-reconhecedor" ? "Nenhum reconhecedor de voz instalado no Windows" : "Não consegui ligar")) : null, lang: r?.result?.split(":")[1] || null };
  send("wake:state", wakeState);
  return wakeState;
}
ipcMain.handle("wake:apply", () => applyWake());
ipcMain.handle("wake:get", () => wakeState);

/* ═════════ Mixer: saída de áudio e volume por app ═════════ */
ipcMain.handle("mix:get", () => helperCall({ cmd: "mix" }, 5000));
ipcMain.handle("mix:setdev", (_e, id) => helperCall({ cmd: "setdev", value: id }, 5000));
ipcMain.handle("mix:appvol", (_e, { pids, volume }) => helperCall({ cmd: "appvol", pids, volume }, 5000));
ipcMain.handle("mix:appmute", (_e, { pids, mute }) => helperCall({ cmd: "appmute", pids, mute: mute ? 1 : 0 }, 5000));

/* ═════════ Relatar problema ═════════ */
// Junta registros (sem chaves nem conteúdo pessoal), um print opcional e abre uma issue pronta no GitHub.
const tail = (file, n) => { try { return fs.readFileSync(file, "utf8").split(/\r?\n/).slice(-n).join("\n"); } catch { return ""; } };
ipcMain.handle("report:make", async (_e, { note = "", withShot = false } = {}) => {
  try {
    const stamp = new Date().toISOString().replace(/[:T]/g, "-").slice(0, 16);
    const dir = path.join(os.tmpdir(), `kon-relatorio-${stamp}`);
    fs.mkdirSync(dir, { recursive: true });
    const pub = publicSettings();
    delete pub.icsUrl; // o link da agenda é secreto
    const info = {
      versao: app.getVersion(), electron: process.versions.electron, windows: os.release(), arquitetura: process.arch,
      memoriaGB: +(os.totalmem() / 1024 ** 3).toFixed(1), helper: !!helper, telaCheia: fgState, atualizacao: updateStatus,
      claudeHooks: (() => { try { return hooksState(readClaudeSettings()); } catch { return null; } })(),
      telas: screen.getAllDisplays().map((d) => ({ w: d.bounds.width, h: d.bounds.height, escala: d.scaleFactor })),
      ajustes: pub, nota: String(note).slice(0, 2000),
    };
    fs.writeFileSync(path.join(dir, "info.json"), JSON.stringify(info, null, 2));
    for (const f of ["kon-error.log", "kon-update.log"]) { const t = tail(userFile(f), 400); if (t) fs.writeFileSync(path.join(dir, f), t); }
    if (withShot) {
      try {
        const d = screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
        const src = (await desktopCapturer.getSources({ types: ["screen"], thumbnailSize: { width: d.bounds.width, height: d.bounds.height } })).find((x) => String(x.display_id) === String(d.id));
        if (src) fs.writeFileSync(path.join(dir, "tela.png"), src.thumbnail.toPNG());
      } catch {}
    }
    const desk = app.getPath("desktop");
    const zip = path.join(desk, `Kon-relatorio-${stamp}.zip`);
    if (process.platform === "win32") {
      await new Promise((res) => {
        const q = (x) => "'" + x.replace(/'/g, "''") + "'";
        spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", `Compress-Archive -Path ${q(dir + "\\*")} -DestinationPath ${q(zip)} -Force`], { windowsHide: true }).on("exit", res);
      });
    }
    const errs = tail(userFile("kon-error.log"), 12).split("\n").filter(Boolean).slice(-8).map((l) => l.slice(0, 220)).join("\n");
    const body = [
      `**O que aconteceu**`, note || "(descreva aqui)", ``,
      `**Ambiente**`, `Kon ${info.versao} · Windows ${info.windows} · ${info.telas.length} tela(s)`, ``,
      errs ? "**Últimos erros**\n```\n" + errs + "\n```" : "Sem erros registrados.", ``,
      `_Arraste para cá o arquivo **${path.basename(zip)}** que o Kon salvou na sua Área de Trabalho._`,
    ].join("\n");
    const title = `Problema: ${(note || "relatório do Kon").split(/\r?\n/)[0].slice(0, 80)}`;
    const url = `https://github.com/${UPDATE_REPO.owner}/${UPDATE_REPO.repo}/issues/new?${new URLSearchParams({ title, body: body.slice(0, 6000) })}`;
    return { ok: true, zip: fs.existsSync(zip) ? zip : null, url };
  } catch (e) { return { ok: false, error: e.message }; }
});
ipcMain.on("report:reveal", (_e, p) => shell.showItemInFolder(p));

/* ═════════ Botões rápidos ═════════ */
ipcMain.handle("quick:pick", async (_e, kind) => {
  const { dialog } = require("electron");
  focusKon();
  const r = await dialog.showOpenDialog(win, { properties: [kind === "folder" ? "openDirectory" : "openFile"], title: kind === "folder" ? "Escolha a pasta" : "Escolha o programa ou arquivo" });
  return r.canceled ? null : r.filePaths[0];
});
ipcMain.handle("quick:run", async (_e, it) => {
  try {
    if (it.kind === "site") {
      const url = /^https?:\/\//i.test(it.target) ? it.target : `https://${it.target}`;
      await shell.openExternal(url); return { ok: true };
    }
    if (it.kind === "cmd") {
      // comando escrito por você: roda numa janela própria para você ver a saída
      spawn(`start "Kon" cmd /k ${it.target}`, { shell: true, detached: true, windowsHide: true, cwd: os.homedir() }).unref();
      return { ok: true };
    }
    const err = await shell.openPath(it.target);
    return err ? { ok: false, error: err } : { ok: true };
  } catch (e) { return { ok: false, error: e.message }; }
});

/* ═════════ Foco: Não incomodar do Windows ═════════ */
let dndPrev = null;
ipcMain.handle("focus:dnd", async (_e, on) => {
  if (on) {
    const r = await helperCall({ cmd: "dnd", mode: 2 }, 3000);
    if (r?.ok && dndPrev === null) dndPrev = r.prev >= 0 ? r.prev : 0;
    return { ok: !!r?.ok };
  }
  const r = await helperCall({ cmd: "dnd", mode: dndPrev ?? 0 }, 3000);
  dndPrev = null;
  return { ok: !!r?.ok };
});

/* ═════════ Sistema: CPU, memória, disco e quem está pesando ═════════ */
let cpuPrev = os.cpus();
let cpuNow = 0;
let cpuHighSince = 0;
let lastHeavyAlert = 0;
function sampleCpu() {
  const cur = os.cpus();
  let idle = 0, total = 0;
  cur.forEach((c, i) => {
    const p = cpuPrev[i] || c;
    const t = (k) => c.times[k] - p.times[k];
    const tot = t("user") + t("nice") + t("sys") + t("idle") + t("irq");
    idle += t("idle"); total += tot;
  });
  cpuPrev = cur;
  cpuNow = total > 0 ? Math.round(100 * (1 - idle / total)) : 0;
  return cpuNow;
}
function diskInfo() {
  try {
    const root = process.platform === "win32" ? (process.env.SystemDrive || "C:") + "\\" : "/";
    const st = fs.statfsSync(root);
    return { total: st.blocks * st.bsize, free: st.bavail * st.bsize, root };
  } catch { return null; }
}
ipcMain.handle("sys:get", async (_e, withProcs) => {
  const total = os.totalmem(), free = os.freemem();
  const procs = withProcs ? await helperCall({ cmd: "procs" }, 4000) : null;
  return { ok: true, cpu: cpuNow, cores: os.cpus().length, cpuModel: (os.cpus()[0]?.model || "").replace(/\s+/g, " ").trim(), mem: { total, used: total - free }, disk: diskInfo(), uptime: os.uptime(), top: procs?.top || [], topMem: procs?.topMem?.[0] || null };
});
async function heavyCheck() {
  sampleCpu();
  const memPct = 100 * (1 - os.freemem() / os.totalmem());
  const high = cpuNow >= 90 || memPct >= 93;
  if (!high) { cpuHighSince = 0; return; }
  if (!cpuHighSince) cpuHighSince = Date.now();
  if (Date.now() - cpuHighSince < 30e3 || Date.now() - lastHeavyAlert < 10 * 60e3 || readSettings().sysAlerts === false) return;
  lastHeavyAlert = Date.now();
  const r = await helperCall({ cmd: "procs" }, 4000);
  const top = r?.top?.[0];
  const text = cpuNow >= 90
    ? `PC sobrecarregado: CPU em ${cpuNow}%${top ? ` · ${top.name} usa ${Math.round(top.cpu)}%` : ""}`
    : `Memória quase cheia (${Math.round(memPct)}%)${r?.topMem?.[0] ? ` · ${r.topMem[0].name} usa ${(r.topMem[0].mem / 1024).toFixed(1)} GB` : ""}`;
  send("toast", { kind: "error", text });
}

/* ═════════ Pergunte sobre a tela ═════════ */
ipcMain.handle("screen:capture", async () => {
  try {
    const display = screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
    const { bounds, scaleFactor } = display;
    const protectedWin = readSettings().hideInCapture !== false;
    if (!protectedWin) { win?.setOpacity(0); await new Promise((r) => setTimeout(r, 90)); }
    const sources = await desktopCapturer.getSources({ types: ["screen"], thumbnailSize: { width: Math.round(bounds.width * scaleFactor), height: Math.round(bounds.height * scaleFactor) } });
    if (!protectedWin) win?.setOpacity(1);
    const src = sources.find((x) => String(x.display_id) === String(display.id)) || sources[0];
    if (!src) return { ok: false, error: "Não consegui capturar a tela" };
    let img = src.thumbnail;
    const sz = img.getSize();
    const k = Math.min(1, 1600 / Math.max(sz.width, sz.height)); // menos tokens, ainda legível
    if (k < 1) img = img.resize({ width: Math.round(sz.width * k), height: Math.round(sz.height * k), quality: "good" });
    const file = path.join(os.tmpdir(), `kon-tela-${Date.now()}.png`);
    fs.writeFileSync(file, img.toPNG());
    setTimeout(() => fs.unlink(file, () => {}), 10 * 60e3);
    return { ok: true, path: file, name: "Captura da tela" };
  } catch (e) { return { ok: false, error: e.message }; }
});

/* ═════════ Lembretes ("me lembra em 20 min de…") ═════════ */
// Ficam no processo principal: disparam mesmo se a interface recarregar. Salvos em kon-reminders.json.
const remPath = () => userFile("kon-reminders.json");
let reminders = null;
function remList() { if (!reminders) { try { reminders = JSON.parse(fs.readFileSync(remPath(), "utf8")); } catch { reminders = []; } } return reminders; }
function remSave() { try { fs.writeFileSync(remPath(), JSON.stringify(reminders)); } catch {} send("reminders:changed", reminders); }
let dueReminders = 0;
function remTick() {
  const now = Date.now();
  let changed = false;
  for (const r of remList()) {
    if (r.fired || r.at > now) continue;
    r.fired = true; changed = true;
    dueReminders++;
    applyFullscreen();
    send("reminder:due", r);
    // se o Kon estiver escondido (tela cheia), avisa também pelo Windows
    if (hiddenForFullscreen || !win || win.isDestroyed()) { try { new Notification({ title: "Kon · lembrete", body: r.text }).show(); } catch {} }
  }
  // limpa lembretes disparados há mais de 1 dia
  const before = reminders.length;
  reminders = reminders.filter((r) => !r.fired || now - r.at < 864e5);
  if (changed || reminders.length !== before) remSave();
}
ipcMain.handle("rem:list", () => remList());
ipcMain.handle("rem:add", (_e, { text, at }) => { remList().push({ id: crypto.randomUUID(), text: String(text || "Lembrete").slice(0, 200), at: +at, fired: false }); remSave(); return reminders; });
ipcMain.handle("rem:remove", (_e, id) => { reminders = remList().filter((r) => r.id !== id); remSave(); return reminders; });
ipcMain.handle("rem:snooze", (_e, { id, minutes }) => {
  const r = remList().find((x) => x.id === id);
  if (r) { r.at = Date.now() + (minutes || 10) * 60e3; r.fired = false; }
  remSave(); return reminders;
});
ipcMain.on("rem:ack", () => { dueReminders = Math.max(0, dueReminders - 1); applyFullscreen(); });

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
let updateStatus = { state: "idle" };
let updaterReady = false;
function updLog(level, msg) {
  try { fs.appendFileSync(userFile("kon-update.log"), `${new Date().toISOString()} ${level} ${typeof msg === "string" ? msg : JSON.stringify(msg)}\n`); } catch {}
}
function setUpdStatus(st) { updateStatus = { ...st, at: Date.now() }; send("update:status", updateStatus); }
function startUpdater() {
  if (!autoUpdater || !app.isPackaged || process.platform !== "win32" || process.env.PORTABLE_EXECUTABLE_FILE) return;
  updaterReady = true;
  autoUpdater.logger = { info: (m) => updLog("info", m), warn: (m) => updLog("warn", m), error: (m) => updLog("error", m), debug: () => {} };
  // Lê direto o latest.yml da Release mais recente (o feed .atom do GitHub às vezes é bloqueado por proxy/firewall).
  // O electron-updater também exige um app-update.yml para baixar; o Kon cria o próprio em userData.
  const feedUrl = `https://github.com/${UPDATE_REPO.owner}/${UPDATE_REPO.repo}/releases/latest/download`;
  const cfgPath = userFile("app-update.yml");
  try { fs.writeFileSync(cfgPath, `provider: generic\nurl: ${feedUrl}\nupdaterCacheDirName: kon-updater\n`); autoUpdater.updateConfigPath = cfgPath; } catch (e) { updLog("error", "app-update.yml: " + e.message); }
  autoUpdater.setFeedURL({ provider: "generic", url: feedUrl });
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;
  let lastPct = -1;
  autoUpdater.on("checking-for-update", () => setUpdStatus({ state: "checking" }));
  autoUpdater.on("update-available", (info) => setUpdStatus({ state: "downloading", version: info.version, percent: 0 }));
  autoUpdater.on("update-not-available", () => setUpdStatus({ state: "latest" }));
  autoUpdater.on("download-progress", (p) => {
    const pct = Math.floor(p.percent || 0);
    if (pct === lastPct) return;
    lastPct = pct;
    setUpdStatus({ state: "downloading", version: updateStatus.version, percent: pct });
  });
  autoUpdater.on("update-downloaded", (info) => { updateInfo = { version: info.version }; setUpdStatus({ state: "ready", version: info.version }); send("update:ready", updateInfo); });
  autoUpdater.on("error", (e) => { const message = String(e?.message || e).split("\n")[0]; updLog("error", message); setUpdStatus({ state: "error", message }); });
  const check = () => { if (readSettings().autoUpdate !== false) checkAll().catch((e) => updLog("error", e.message)); };
  setTimeout(check, 20e3);
  setInterval(check, 4 * 3600e3);
}
/* Atualização pequena: quando só o app mudou (mesmo Electron), baixa o app.asar compactado (~7 MB)
   em vez do instalador inteiro (~115 MB). Quando o Electron muda, usa o instalador completo. */
const FEED = `https://github.com/${UPDATE_REPO.owner}/${UPDATE_REPO.repo}/releases/latest/download`;
const verNum = (v) => String(v || "0").split(".").map((x) => parseInt(x, 10) || 0);
const isNewer = (a, b) => { const x = verNum(a), y = verNum(b); for (let i = 0; i < 3; i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0); } return false; };
async function checkPatch() {
  const r = await fetch(`${FEED}/kon-patch.json?t=${Date.now()}`, { signal: AbortSignal.timeout(15000) });
  if (!r.ok) return null;
  const j = await r.json();
  if (!isNewer(j.version, app.getVersion())) return { latest: true };
  if (j.electron !== process.versions.electron) return { full: true, version: j.version };
  return j;
}
async function downloadPatch(j) {
  setUpdStatus({ state: "downloading", version: j.version, percent: 0, small: true });
  const res = await fetch(`${FEED}/${j.file}`, { signal: AbortSignal.timeout(10 * 60e3) });
  if (!res.ok) throw new Error(`download ${res.status}`);
  const total = +res.headers.get("content-length") || j.size || 0;
  const reader = res.body.getReader();
  const chunks = []; let got = 0, lastPct = -1;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value); got += value.length;
    const pct = total ? Math.floor((got / total) * 100) : 0;
    if (pct !== lastPct) { lastPct = pct; setUpdStatus({ state: "downloading", version: j.version, percent: pct, small: true }); }
  }
  const asar = require("zlib").gunzipSync(Buffer.concat(chunks.map((c) => Buffer.from(c))));
  const sha = crypto.createHash("sha512").update(asar).digest("base64");
  if (sha !== j.sha512) throw new Error("o arquivo baixado não confere (sha512)");
  const dir = userFile("patch");
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, "app.asar"), asar);
  updateInfo = { version: j.version, patch: true };
  updLog("info", `patch ${j.version} pronto (${(got / 1e6).toFixed(1)} MB)`);
  setUpdStatus({ state: "ready", version: j.version, small: true });
  send("update:ready", updateInfo);
}
let checking = false;
async function checkAll() {
  if (checking || updateInfo) return { ok: true, latest: updateInfo?.version, newer: !!updateInfo };
  checking = true;
  try {
    setUpdStatus({ state: "checking" });
    let p = null;
    try { p = await checkPatch(); } catch (e) { updLog("warn", "patch: " + e.message); }
    if (p?.latest) { setUpdStatus({ state: "latest" }); return { ok: true, newer: false }; }
    if (p && !p.full) { downloadPatch(p).catch((e) => { updLog("error", "patch: " + e.message); setUpdStatus({ state: "error", message: e.message }); fullCheck(); }); return { ok: true, newer: true, latest: p.version }; }
    return await fullCheck();
  } finally { checking = false; }
}
async function fullCheck() {
  const r = await autoUpdater.checkForUpdates();
  const v = r?.updateInfo?.version;
  return { ok: true, latest: v, newer: !!v && isNewer(v, app.getVersion()) };
}
let patchInstalling = false;
function installPatch(relaunch = true) {
  if (patchInstalling) return;
  patchInstalling = true;
  const exe = process.execPath;
  const dst = path.join(process.resourcesPath, "app.asar");
  const src = path.join(userFile("patch"), "app.asar");
  const ps = path.join(os.tmpdir(), `kon-patch-${Date.now()}.ps1`);
  const q = (x) => "'" + String(x).replace(/'/g, "''") + "'";
  fs.writeFileSync(ps, [
    `$ErrorActionPreference = 'SilentlyContinue'`,
    `Wait-Process -Id ${process.pid} -Timeout 20`,
    `Start-Sleep -Milliseconds 600`,
    `Get-Process -Name 'Kon' | Where-Object { $_.Path -eq ${q(exe)} } | Stop-Process -Force`,
    `Start-Sleep -Milliseconds 400`,
    `$ok = $false`,
    `for ($i = 0; $i -lt 30 -and -not $ok; $i++) { try { Copy-Item -LiteralPath ${q(dst)} -Destination (${q(dst)} + '.bak') -Force -ErrorAction Stop; Copy-Item -LiteralPath ${q(src)} -Destination ${q(dst)} -Force -ErrorAction Stop; $ok = $true } catch { Start-Sleep -Milliseconds 500 } }`,
    `if ($ok) { Set-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\app.kon.desktop' -Name DisplayVersion -Value ${q(updateInfo.version)}; Remove-Item -LiteralPath ${q(src)} -Force }`,
    `Add-Content -LiteralPath ${q(userFile("kon-update.log"))} -Value ((Get-Date -Format o) + ' patch aplicado: ' + $ok)`,
    relaunch ? `Start-Process -FilePath ${q(exe)}` : "",
    `Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force`,
  ].join("\r\n"), "utf8");
  spawn("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File", ps], { detached: true, stdio: "ignore", windowsHide: true }).unref();
  if (relaunch) app.quit();
}

ipcMain.handle("update:get", () => ({ version: app.getVersion(), ready: updateInfo, status: updateStatus, portable: !!process.env.PORTABLE_EXECUTABLE_FILE }));
ipcMain.on("update:install", () => {
  if (!updateInfo || !autoUpdater) return;
  updLog("info", `instalando ${updateInfo.version}${updateInfo.patch ? " (pequena)" : ""}`);
  app.isQuitting = true;
  try { helper?.stdin.end(); helper?.kill(); } catch {}
  if (updateInfo.patch) return installPatch();
  setImmediate(() => autoUpdater.quitAndInstall(true, true));
});
ipcMain.handle("update:check", async () => {
  if (!autoUpdater || !app.isPackaged) return { ok: false, error: "Só no app instalado" };
  if (process.env.PORTABLE_EXECUTABLE_FILE) return { ok: false, error: "A versão portátil não se atualiza sozinha — use o instalador" };
  if (!updaterReady) return { ok: false, error: "Atualizador indisponível" };
  if (updateInfo) return { ok: true, latest: updateInfo.version, newer: true, ready: true };
  if (updateStatus.state === "downloading") return { ok: true, latest: updateStatus.version, newer: true, downloading: true };
  try { return await checkAll(); }
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
    globalShortcut.register("Control+Alt+Space", () => { focusKon(); send("shortcut", "ask"); }),
    globalShortcut.register("Control+Alt+T", () => startOcr()),
    globalShortcut.register("Control+Alt+E", () => send("shortcut", "panel")),
    globalShortcut.register("Control+Alt+V", () => { win?.show(); send("shortcut", "clip"); }),
    globalShortcut.register("Control+Alt+R", () => { grabSelection().catch(() => {}); }),
  ];
  if (ok.includes(false)) console.warn("Algum atalho global já está em uso por outro programa");
}
function createTray() {
  tray = new Tray(nativeImage.createFromPath(path.join(__dirname, "tray.png")));
  tray.setToolTip("Kon");
  const s = readSettings();
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: "Perguntar ao Kon      Ctrl+Alt+Space", click: () => { focusKon(); send("shortcut", "ask"); } },
    { label: "Smart OCR             Ctrl+Alt+T", click: () => startOcr() },
    { label: "Abrir painel          Ctrl+Alt+E", click: () => send("shortcut", "panel") },
    { label: "Área de transferência  Ctrl+Alt+V", click: () => send("shortcut", "clip") },
    { label: "Ações no texto selecionado  Ctrl+Alt+R", click: () => grabSelection().catch(() => {}) },
    { label: "Configurações", click: () => { focusKon(); send("shortcut", "settings"); } },
    { label: "Relatar problema", click: () => { focusKon(); send("shortcut", "report"); } },
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
    clipLast = readClipText();
    setInterval(clipTick, 800);
    setInterval(() => { try { remTick(); } catch (e) { console.warn("lembretes", e); } }, 5000);
    setInterval(() => { heavyCheck().catch(() => {}); }, 5000);
    // mantém a opção "Iniciar com o Windows" em sincronia com o que o instalador marcou
    try { const on = app.getLoginItemSettings().openAtLogin; if (on !== !!readSettings().autoStart) writeSettings({ autoStart: on }); } catch {}
    const replace = () => placeWindow();
    screen.on("display-metrics-changed", replace);
    screen.on("display-added", replace);
    screen.on("display-removed", replace);
  });
  app.on("before-quit", () => {
    // atualização pequena baixada e você fechou o Kon: aplica ao sair, sem reabrir
    if (updateInfo?.patch && !patchInstalling) { try { installPatch(false); } catch {} }
    app.isQuitting = true; try { helper?.stdin.end(); helper?.kill(); } catch {} });
  app.on("will-quit", () => globalShortcut.unregisterAll());
  app.on("window-all-closed", () => app.quit());
}
