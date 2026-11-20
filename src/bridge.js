// Usa a ponte do Electron (window.kon). No navegador (npm run web) cai num modo compatível,
// útil para ajustar o visual — recursos nativos ficam desativados.
const LS = "kon.web.settings";
const read = () => { try { return JSON.parse(localStorage.getItem(LS) || "{}"); } catch { return {}; } };
const write = (v) => { try { localStorage.setItem(LS, JSON.stringify(v)); } catch {} };
const DEFAULTS = { provider: "gemini", geminiModel: "gemini-flash-latest", claudeModel: "claude-haiku-5-5", webSearch: true, autoStart: false, devDock: true,
  openMode: "hover", notchSize: "normal", display: "primary", hideFullscreen: true, slimOverMax: true, hideInCapture: true, approvals: true,
  claudePlan: "auto", icsUrl: "", clipboardOn: true, autoUpdate: true, onboarded: true, outfit: "auto", soundsOn: true, soundPack: "retro", soundVol: 0.6 };
const DAY = 864e5;
const day0 = () => { const d = new Date(); d.setHours(0, 0, 0, 0); return d.getTime(); };
// dados de exemplo para ver a página de uso no navegador
const demoUsage = () => ({
  ok: true, found: true, at: Date.now(), plan: "auto", limit: 88000, estimated: true,
  window: { used: 64000, pct: 73, resetsAt: Date.now() + 102 * 60e3, startedAt: Date.now() - 198 * 60e3 },
  today: { tokens: 2.4e6, cost: 6.8 }, week: { tokens: 11.9e6, cost: 31.2 },
  days: [0.3, 0.55, 0.4, 0.85, 0.6, 0.35, 0.7].map((f, i) => ({ day: day0() - (6 - i) * DAY, tokens: Math.round(f * 3.4e6) })),
  models: [{ name: "Opus", tokens: 7.4e6, pct: 62 }, { name: "Sonnet", tokens: 3.7e6, pct: 31 }, { name: "Haiku", tokens: 0.8e6, pct: 7 }],
  sessionsToday: 3, konToday: 48000,
});
const pub = () => { const s = { ...DEFAULTS, ...read() }; return { ...s, hasGeminiKey: false, hasAnthropicKey: false, secure: false }; };

const webBridge = {
  desktop: false,
  setHitRects: () => {}, pin: () => {}, focus: () => {}, blur: () => {}, quit: () => {},
  getSettings: async () => pub(),
  setSettings: async (patch) => { write({ ...read(), ...patch }); return pub(); },
  setKey: async () => pub(),
  keyFromClipboard: async () => ({ ok: false, error: "Só no app desktop" }),
  aiStream: () => {}, aiAbort: () => {},
  aiOnce: async () => ({ ok: false, code: "NO_KEY" }),
  transcribe: async () => ({ ok: false, code: "NO_KEY" }),
  startOcr: () => {}, ocrRegion: () => {}, ocrCancel: () => {},
  pathForFile: () => "",
  stashList: async () => [], stashAdd: async () => ({ list: [], added: [] }), stashRemove: async () => [],
  stashOpen: async () => "", stashReveal: () => {}, stashIcon: async () => null, stashDrag: () => {},
  mediaCmd: () => {},
  claudeHooks: async () => ({ ok: false, installed: false, error: "Só no app desktop" }),
  cacheGet: async (k) => { try { return JSON.parse(localStorage.getItem("kon.cache." + k) || "null"); } catch { return null; } },
  cacheSet: async (k, v) => { try { localStorage.setItem("kon.cache." + k, JSON.stringify(v)); } catch {} return true; },
  claudeDecide: () => {},
  ...(() => { let mix = { ok: true, devices: [{ id: "a", name: "Alto-falantes (Realtek)", default: true }, { id: "b", name: "Fone JBL Tune 520BT", default: false }, { id: "c", name: "Monitor LG (HDMI)", default: false }], apps: [{ name: "Spotify", title: "Spotify", pids: [1], volume: 80, muted: false }, { name: "chrome", title: "Google Chrome", pids: [2, 3], volume: 55, muted: false }, { name: "Discord", title: "Discord", pids: [4], volume: 100, muted: true }] };
    return {
      mixGet: async () => mix,
      mixSetDevice: async (id) => { mix = { ...mix, devices: mix.devices.map((d) => ({ ...d, default: d.id === id })) }; return mix; },
      mixAppVolume: async (pids, volume) => { mix = { ...mix, apps: mix.apps.map((a) => (a.pids[0] === pids[0] ? { ...a, volume } : a)) }; return mix; },
      mixAppMute: async (pids, muted) => { mix = { ...mix, apps: mix.apps.map((a) => (a.pids[0] === pids[0] ? { ...a, muted } : a)) }; return mix; },
    }; })(),
  textPaste: async () => false,
  wakeGet: async () => ({ on: false }),
  report: async () => ({ ok: true, zip: null, url: "https://github.com/jvruiz7/kon-notch/issues/new" }),
  reportReveal: () => {},
  sysInfo: async () => ({ ok: true, cpu: 23 + Math.round(Math.random() * 10), cores: 12, cpuModel: "Intel Core i7-12700H", mem: { total: 16e9, used: 10.4e9 }, disk: { total: 512e9, free: 183e9, root: "C:\\" }, uptime: 3 * 3600 + 1200, top: [{ name: "chrome", cpu: 12.4, mem: 2100 }, { name: "Code", cpu: 6.1, mem: 1200 }, { name: "Spotify", cpu: 2.3, mem: 380 }], topMem: { name: "chrome", mem: 2100 } }),
  focusDnd: async () => ({ ok: false }),
  quickPick: async () => null,
  quickRun: async (it) => { if (it.kind === "site") window.open(/^https?:/.test(it.target) ? it.target : "https://" + it.target, "_blank"); return { ok: it.kind === "site" }; },
  captureScreen: async () => ({ ok: false, error: "Só no app desktop" }),
  ...(() => { let rems = []; const t = new Set(); return {
    remList: async () => rems,
    remAdd: async (r) => { const it = { id: String(Date.now()), fired: false, ...r }; rems = [...rems, it]; const ms = Math.max(0, r.at - Date.now()); t.add(setTimeout(() => { it.fired = true; window.dispatchEvent(new CustomEvent("kon-web-reminder", { detail: it })); }, ms)); return rems; },
    remRemove: async (id) => (rems = rems.filter((x) => x.id !== id)),
    remSnooze: async (id, m) => { const it = rems.find((x) => x.id === id); if (it) { it.at = Date.now() + m * 60e3; it.fired = false; } return rems; },
    remAck: () => {},
  }; })(),
  usage: async () => demoUsage(),
  calendar: async () => ({ ok: true, events: [], empty: true }),
  clipList: async () => [], clipCopy: async () => true, clipRemove: async () => [], clipPin: async () => [], clipClear: async () => [],
  zip: async () => ({ ok: false, error: "Só no app desktop" }),
  fgState: async () => ({ full: false, max: false }),
  updateInfo: async () => ({ version: "web", ready: null }),
  updateCheck: async () => ({ ok: false, error: "Só no app instalado" }),
  updateInstall: () => {},
  copy: async (text) => { try { await navigator.clipboard.writeText(text); return true; } catch { return false; } },
  openExternal: (url) => window.open(url, "_blank", "noopener"),
  on: () => () => {},
};

export const bridge = typeof window !== "undefined" && window.kon ? window.kon : webBridge;
