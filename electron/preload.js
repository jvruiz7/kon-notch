// Ponte segura entre o renderer (React) e o processo principal do Kon.
const { contextBridge, ipcRenderer, webUtils } = require("electron");

const CHANNELS = [
  "shortcut", "win:blurred", "media:state", "volume:state", "claude:event", "helper:ready",
  "ocr:started", "ocr:result", "ai:event", "toast", "fg:state", "privacy:state", "claude:perm-closed",
  "clip:changed", "update:ready", "update:status", "reminder:due", "reminders:changed", "textact:open", "wake:state", "settings:changed", "key:detected",
];

window.addEventListener("resize", () => ipcRenderer.send("hit:viewport", { vw: window.innerWidth, vh: window.innerHeight, dpr: window.devicePixelRatio }));

contextBridge.exposeInMainWorld("kon", {
  desktop: true,
  // janela
  // regiões clicáveis + tamanho real da página: o main converte coordenadas da janela para CSS px
  // (corrige escala dupla em monitores com DPI diferentes e qualquer zoom)
  setHitRects: (rects) => ipcRenderer.send("hit:rects", { rects, vw: window.innerWidth, vh: window.innerHeight, dpr: window.devicePixelRatio }),
  pin: (on) => ipcRenderer.send("win:pin", !!on),
  focus: () => ipcRenderer.send("win:focus"),
  blur: () => ipcRenderer.send("win:blur"),
  cursor: (x, y) => ipcRenderer.send("hit:cursor", x == null ? null : [x, y]),
  diag: (e) => ipcRenderer.send("diag", e), // diagnóstico de cliques (vai só para o relatório de problemas)
  quit: () => ipcRenderer.send("app:quit"),
  // configurações e chaves (as chaves nunca voltam para o renderer)
  getSettings: () => ipcRenderer.invoke("settings:get"),
  setSettings: (patch) => ipcRenderer.invoke("settings:set", patch),
  setKey: (provider, key) => ipcRenderer.invoke("keys:set", { provider, key }),
  keyFromClipboard: (provider) => ipcRenderer.invoke("keys:fromClipboard", provider),
  // IA
  aiStream: (req) => ipcRenderer.send("ai:stream", req),
  aiAbort: (id) => ipcRenderer.send("ai:abort", id),
  aiOnce: (req) => ipcRenderer.invoke("ai:once", req),
  transcribe: (base64, mime) => ipcRenderer.invoke("ai:transcribe", { base64, mime }),
  // OCR
  startOcr: () => ipcRenderer.send("ocr:start"),
  ocrRegion: (r) => ipcRenderer.send("ocr:region", r),
  ocrCancel: () => ipcRenderer.send("ocr:cancel"),
  // stash
  pathForFile: (file) => { try { return webUtils.getPathForFile(file); } catch { return ""; } },
  stashList: () => ipcRenderer.invoke("stash:list"),
  stashAdd: (paths) => ipcRenderer.invoke("stash:add", paths),
  stashRemove: (id) => ipcRenderer.invoke("stash:remove", id),
  stashOpen: (p) => ipcRenderer.invoke("stash:open", p),
  stashReveal: (p) => ipcRenderer.send("stash:reveal", p),
  stashIcon: (p) => ipcRenderer.invoke("stash:icon", p),
  stashDrag: (p) => ipcRenderer.send("stash:drag", p),
  // mídia, Claude Code, cache, diversos
  mediaCmd: (cmd, value) => ipcRenderer.send("media:cmd", { cmd, value }),
  claudeHooks: (action) => ipcRenderer.invoke("claude:hooks", action),
  cacheGet: (key) => ipcRenderer.invoke("cache:get", key),
  cacheSet: (key, value) => ipcRenderer.invoke("cache:set", key, value),
  claudeDecide: (id, decision) => ipcRenderer.send("claude:decide", { id, decision }),
  usage: () => ipcRenderer.invoke("usage:get"),
  calendar: (force) => ipcRenderer.invoke("cal:get", !!force),
  clipList: () => ipcRenderer.invoke("clip:list"),
  clipCopy: (id) => ipcRenderer.invoke("clip:copy", id),
  clipRemove: (id) => ipcRenderer.invoke("clip:remove", id),
  clipPin: (id) => ipcRenderer.invoke("clip:pin", id),
  clipClear: () => ipcRenderer.invoke("clip:clear"),
  zip: (paths) => ipcRenderer.invoke("zip:make", paths),
  fgState: () => ipcRenderer.invoke("fg:get"),
  updateInfo: () => ipcRenderer.invoke("update:get"),
  updateCheck: () => ipcRenderer.invoke("update:check"),
  updateInstall: () => ipcRenderer.send("update:install"),
  wakeGet: () => ipcRenderer.invoke("wake:get"),
  report: (opts) => ipcRenderer.invoke("report:make", opts),
  reportReveal: (p) => ipcRenderer.send("report:reveal", p),
  sysInfo: (withProcs) => ipcRenderer.invoke("sys:get", !!withProcs),
  focusDnd: (on) => ipcRenderer.invoke("focus:dnd", !!on),
  quickPick: (kind) => ipcRenderer.invoke("quick:pick", kind),
  quickRun: (it) => ipcRenderer.invoke("quick:run", it),
  mixGet: () => ipcRenderer.invoke("mix:get"),
  mixSetDevice: (id) => ipcRenderer.invoke("mix:setdev", id),
  mixAppVolume: (pids, volume) => ipcRenderer.invoke("mix:appvol", { pids, volume }),
  mixAppMute: (pids, mute) => ipcRenderer.invoke("mix:appmute", { pids, mute }),
  textPaste: (text) => ipcRenderer.invoke("textact:paste", text),
  captureScreen: () => ipcRenderer.invoke("screen:capture"),
  remList: () => ipcRenderer.invoke("rem:list"),
  remAdd: (r) => ipcRenderer.invoke("rem:add", r),
  remRemove: (id) => ipcRenderer.invoke("rem:remove", id),
  remSnooze: (id, minutes) => ipcRenderer.invoke("rem:snooze", { id, minutes }),
  remAck: () => ipcRenderer.send("rem:ack"),
  copy: (text) => ipcRenderer.invoke("clipboard:write", text),
  openExternal: (url) => ipcRenderer.send("open:external", url),
  on: (channel, cb) => {
    if (!CHANNELS.includes(channel)) return () => {};
    const handler = (_e, payload) => cb(payload);
    ipcRenderer.on(channel, handler);
    return () => ipcRenderer.removeListener(channel, handler);
  },
});
