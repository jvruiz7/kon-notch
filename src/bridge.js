// Usa a ponte do Electron (window.kon). No navegador (npm run web) cai num modo compatível,
// útil para ajustar o visual — recursos nativos ficam desativados.
const LS = "kon.web.settings";
const read = () => { try { return JSON.parse(localStorage.getItem(LS) || "{}"); } catch { return {}; } };
const write = (v) => { try { localStorage.setItem(LS, JSON.stringify(v)); } catch {} };
const DEFAULTS = { provider: "gemini", geminiModel: "gemini-2.5-flash", claudeModel: "claude-haiku-5-5", webSearch: true, autoStart: false, devDock: true,
  openMode: "hover", notchSize: "normal", display: "primary", hideFullscreen: true, slimOverMax: true, hideInCapture: true, approvals: true,
  claudePlan: "auto", icsUrl: "", clipboardOn: true, autoUpdate: true, onboarded: true };
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
