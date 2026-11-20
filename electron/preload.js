// Ponte segura entre o renderer (React) e o processo principal do Kon.
const { contextBridge, ipcRenderer, webUtils } = require("electron");

const CHANNELS = [
  "shortcut", "win:blurred", "media:state", "volume:state", "claude:event", "helper:ready",
  "ocr:started", "ocr:result", "ai:event", "toast", "fg:state", "privacy:state", "claude:perm-closed",
  "clip:changed", "update:ready",
];

contextBridge.exposeInMainWorld("kon", {
  desktop: true,
  // janela
  setHitRects: (rects) => ipcRenderer.send("hit:rects", rects),
  pin: (on) => ipcRenderer.send("win:pin", !!on),
  focus: () => ipcRenderer.send("win:focus"),
  blur: () => ipcRenderer.send("win:blur"),
  quit: () => ipcRenderer.send("app:quit"),
  // configurações e chaves (as chaves nunca voltam para o renderer)
  getSettings: () => ipcRenderer.invoke("settings:get"),
  setSettings: (patch) => ipcRenderer.invoke("settings:set", patch),
  setKey: (provider, key) => ipcRenderer.invoke("keys:set", { provider, key }),
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
  copy: (text) => ipcRenderer.invoke("clipboard:write", text),
  openExternal: (url) => ipcRenderer.send("open:external", url),
  on: (channel, cb) => {
    if (!CHANNELS.includes(channel)) return () => {};
    const handler = (_e, payload) => cb(payload);
    ipcRenderer.on(channel, handler);
    return () => ipcRenderer.removeListener(channel, handler);
  },
});
