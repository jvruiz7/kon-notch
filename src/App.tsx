// @ts-nocheck — componente grande e dinâmico; os tipos estritos ficam nos módulos menores.
import React, { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence, Reorder } from "framer-motion";
import {
  Asterisk, Play, Pause, SkipBack, SkipForward, Shuffle, Repeat, Volume1, Volume2, VolumeX,
  Plus, Minus, Moon, Lock, LockOpen, EyeOff, Settings, Search, Languages, X, Copy, Check,
  ChevronLeft, ChevronRight, ArrowLeftRight, Trash2, Pencil, KeyRound, ExternalLink, Power, Eye as EyeIcon, Music2,
  FolderOpen, File as FileIcon, FileText, FileImage, FileVideo, FileAudio, FileArchive, FileCode, Folder,
  Mic, Square, Globe, MapPin, Download, Bell, CheckCircle2, AlertTriangle,
  Camera, Pin, PinOff, Zap, Cpu, HardDrive, MemoryStick, AppWindow, Link2, TerminalSquare, ClipboardList, Archive, Sparkles, Video, CalendarClock, RefreshCw, Paperclip, Terminal, ShieldCheck, Monitor, MousePointerClick,
  Sun, Cloud, CloudSun, CloudMoon, CloudFog, CloudDrizzle, CloudRain, CloudSnow, CloudLightning, Droplets, Wind, Sunrise, Sunset, Thermometer,
} from "lucide-react";
import {
  ACCENTS, SPRING, FADE, COMPACT, PANEL_SIZE, PANEL_DEFAULT, PAGE_META, DEFAULT_ORDER, DURATION, STAGES, LYRICS,
  fmt, pad, DAYS_T, MONTHS_T, MONTHS_FULL, WEEK, fmtDate, fmtClock, fmtNum, MATH_RE, parseNum, calc, CUR_RE, fmtCur,
  BR_CITIES, wmo, toMin, cityNow, LANGS, TARGETS, INITIAL_TODOS, usePersist, glassBtn,
  detectLangLocal, translateLocal,
} from "./data.js";
import { Eyes, AlbumArt, Equalizer, Waveform, Seg, Ring, Ruler, Switch, PillBtn, PageRow } from "./ui.jsx";
import { KonTofu, KonTofuEating, OutfitContext, OUTFITS } from "./components/KonMascot";
import { playSound, SOUND_PACKS } from "./sounds.js";
import { bridge } from "./bridge.js";
import { convertUnits } from "./units.js";
import { parseReminder, describeWhen } from "./reminders.js";
import { getRates, RATES_TTL, getIpLocation, getWeather, getLyrics, getCoverArt } from "./services.js";

const TASK_DAYS = [0, 1, -2, 3, 6];
const WIN_W = 820;
const CHAT_SYSTEM =
  "Você é o Kon, um assistente conciso que vive no notch do Windows. Responda no idioma do usuário (padrão: português do Brasil). " +
  "Seja breve (até ~120 palavras), use linhas curtas ou marcadores '• '. Sem títulos markdown, tabelas ou negrito.";
const TRANSLATE_SYSTEM = "You are a translation engine. Translate faithfully and naturally, keeping tone, names, numbers and line breaks. Reply with JSON only.";
const TEST_RE = /\b(test|tests|jest|vitest|pytest|mocha|playwright|cypress|cargo test|go test|npm run (test|lint|build|typecheck)|pnpm (test|lint|build)|yarn (test|lint|build)|tsc|eslint)\b/i;
const STAGE_LABEL = ["Reading", "Editing", "Testing", "Done"];
const PROVIDER_NAME = { gemini: "Gemini", claude: "Claude" };
const WX_ICON = { sun: Sun, moon: Moon, "sun-cloud": CloudSun, "moon-cloud": CloudMoon, cloud: Cloud, fog: CloudFog, drizzle: CloudDrizzle, rain: CloudRain, snow: CloudSnow, storm: CloudLightning };
function locIsoHour(tz, date) {
  const p = Object.fromEntries(new Intl.DateTimeFormat("en-CA", { timeZone: tz, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", hourCycle: "h23" }).formatToParts(date).map((x) => [x.type, x.value]));
  return `${p.year}-${p.month}-${p.day}T${String(+p.hour % 24).padStart(2, "0")}`;
}
const linesOf = (x) => String(x || "").split(/\r?\n/);
const firstLine = (x) => (linesOf(x).find((l) => l.trim()) || "").trim();
const baseName = (f) => String(f || "").split(/[\\/]/).filter(Boolean).pop() || "";
const uid = () => (crypto.randomUUID ? crypto.randomUUID() : String(Date.now() + Math.random()));
const cleanLine = (l) => l.replace(/\*\*(.+?)\*\*/g, "$1").replace(/^#+\s*/, "").replace(/^\s*[-*]\s+/, "• ");
const fmtSize = (b) => (!b ? "" : b < 1024 ? `${b} B` : b < 1048576 ? `${(b / 1024).toFixed(0)} KB` : b < 1073741824 ? `${(b / 1048576).toFixed(1)} MB` : `${(b / 1073741824).toFixed(2)} GB`);
const fmtHM = (ts) => new Date(ts).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
const extIcon = (ext) =>
  ext === "folder" ? Folder
    : /^(png|jpe?g|gif|webp|bmp|svg|heic|ico)$/.test(ext) ? FileImage
      : /^(mp4|mov|mkv|avi|webm)$/.test(ext) ? FileVideo
        : /^(mp3|wav|flac|m4a|ogg)$/.test(ext) ? FileAudio
          : /^(zip|rar|7z|tar|gz)$/.test(ext) ? FileArchive
            : /^(js|ts|tsx|jsx|py|json|html|css|cs|cpp|java|go|rs|ps1|sql)$/.test(ext) ? FileCode
              : /^(pdf|docx?|txt|md|xlsx?|pptx?|csv|dwg|rtf)$/.test(ext) ? FileText : FileIcon;
function diffOf(tool, inp) {
  let oldS = "", newS = "";
  if (tool === "Edit") { oldS = inp.old_string; newS = inp.new_string; }
  else if (tool === "MultiEdit") { oldS = inp.edits?.[0]?.old_string; newS = inp.edits?.[0]?.new_string; }
  else if (tool === "Write") { newS = inp.content; }
  else if (tool === "NotebookEdit") { newS = inp.new_source; }
  return {
    file: baseName(inp.file_path || inp.notebook_path).toUpperCase(),
    removed: firstLine(oldS).slice(0, 70), added: firstLine(newS).slice(0, 70),
    plus: newS ? linesOf(newS).length : 0, minus: oldS ? linesOf(oldS).length : 0,
  };
}
const appName = (id = "") => {
  const s = id.toLowerCase();
  if (s.includes("spotify")) return "Spotify";
  if (s.includes("chrome")) return "Chrome";
  if (s.includes("msedge")) return "Microsoft Edge";
  if (s.includes("firefox")) return "Firefox";
  if (s.includes("applemusic") || s.includes("itunes")) return "Apple Music";
  if (s.includes("zunemusic") || s.includes("media")) return "Media Player";
  return id.split(/[!._]/)[0] || "Media";
};
const lyricAt = (lines, t) => { let i = 0; for (let k = 0; k < lines.length; k++) { if (lines[k].t <= t + 0.3) i = k; else break; } return i; };
const parseJSONish = (t) => { try { return JSON.parse(t); } catch { const m = String(t).match(/\{[\s\S]*\}/); return m ? JSON.parse(m[0]) : null; } };

/* Texto do card de permissão do Claude Code */
function permText(p) {
  const t = p.tool || "", i = p.input || {};
  const project = baseName(p.cwd);
  let title = `Claude quer usar ${t || "uma ferramenta"}`, sub = "";
  if (t === "Bash" || t === "PowerShell") { title = "Claude quer rodar um comando"; sub = firstLine(i.command || "").slice(0, 120); }
  else if (/^(Edit|MultiEdit|Write|NotebookEdit)$/.test(t)) { title = `Claude quer ${t === "Write" ? "criar" : "editar"} ${baseName(i.file_path || i.notebook_path) || "um arquivo"}`; sub = i.file_path || i.notebook_path || ""; }
  else if (t === "WebFetch") { title = "Claude quer abrir um site"; sub = i.url || ""; }
  else if (t === "WebSearch") { title = "Claude quer pesquisar na web"; sub = i.query || ""; }
  else if (t === "ExitPlanMode") { title = "Claude quer sair do modo plano"; sub = "e começar a executar o plano"; }
  else if (t.startsWith("mcp__")) { const parts = t.split("__"); title = `Claude quer usar ${parts[2] || t}`; sub = `conector ${parts[1] || ""}`; }
  else sub = JSON.stringify(i).slice(0, 120);
  // prévia do que muda: até 2 linhas que saem e 2 que entram
  const L = (x) => String(x || "").split(/\r?\n/).map((l) => l.replace(/\t/g, "  ")).filter((l) => l.trim());
  let diff = null, more = 0;
  const e0 = t === "MultiEdit" ? (i.edits || [])[0] || {} : i;
  if (t === "Edit" || t === "MultiEdit") {
    const rm = L(e0.old_string), add = L(e0.new_string);
    diff = [...rm.slice(0, 2).map((x) => ["-", x]), ...add.slice(0, 2).map((x) => ["+", x])];
    more = Math.max(0, rm.length - 2) + Math.max(0, add.length - 2) + (t === "MultiEdit" ? Math.max(0, (i.edits || []).length - 1) : 0);
  } else if (t === "Write") {
    const add = L(i.content);
    diff = add.slice(0, 3).map((x) => ["+", x]);
    more = Math.max(0, add.length - 3);
  } else if (t === "NotebookEdit") {
    const add = L(i.new_source);
    diff = add.slice(0, 3).map((x) => ["+", x]);
    more = Math.max(0, add.length - 3);
  }
  return { title, sub, project, diff: diff && diff.length ? diff : null, more };
}
const fmtLeft = (ms) => {
  if (!ms || ms <= 0) return "agora";
  const m = Math.ceil(ms / 60e3);
  return m >= 60 ? `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}` : `${m} min`;
};
const fmtTok = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1).replace(".", ",")} M` : n >= 1e3 ? `${Math.round(n / 1e3)} mil` : `${n || 0}`);
const fmtUsd = (n) => `US$ ${(n || 0).toFixed(2).replace(".", ",")}`;
const TEXT_ACTIONS = [
  ["resumir", "Resumir", "Resuma o texto abaixo em português, em até 3 frases curtas."],
  ["traduzir", "Traduzir", "Se o texto abaixo estiver em português, traduza para o inglês; se estiver em outro idioma, traduza para o português do Brasil. Mantenha o tom e a formatação."],
  ["corrigir", "Corrigir", "Corrija ortografia, gramática e pontuação do texto abaixo, mantendo o idioma, o tom e a formatação."],
  ["formal", "Mais formal", "Reescreva o texto abaixo em tom mais formal e profissional, no mesmo idioma."],
  ["curto", "Mais curto", "Reescreva o texto abaixo mais curto e direto, no mesmo idioma, sem perder o sentido."],
  ["explicar", "Explicar", "Explique em português, de forma simples, o que o texto abaixo quer dizer."],
];
const PLAN_LABEL = { auto: "Automático", pro: "Pro", max5: "Max 5×", max20: "Max 20×" };

export default function KonIsland() {
  /* ─── Estado principal ─── */
  const [view, setView] = useState("compact"); // compact | panel | sheet | ask | translate | ocr
  const [hover, setHover] = useState(false);
  const [tab, setTab] = usePersist("tab.v2", "home");
  const [pageOrder, setPageOrder] = usePersist("pages.v2", DEFAULT_ORDER);
  const [pageOn, setPageOn] = usePersist("pagesOn.v2", Object.fromEntries(DEFAULT_ORDER.map((k) => [k, true])));
  const [sheetTab, setSheetTab] = useState("pages");
  const [settings, setSettings] = useState({ provider: "gemini", geminiModel: "gemini-2.5-flash", claudeModel: "claude-haiku-5-5", webSearch: true, autoStart: false, devDock: !bridge.desktop, hasGeminiKey: false, hasAnthropicKey: false,
    openMode: "hover", notchSize: "normal", display: "primary", hideFullscreen: true, slimOverMax: true, hideInCapture: true, approvals: true, claudePlan: "auto", icsUrl: "", clipboardOn: true, autoUpdate: true, onboarded: true, outfit: "auto", soundsOn: true, soundPack: "retro", soundVol: 0.6, loaded: false });
  const [obStep, setObStep] = useState(0);
  const [fg, setFg] = useState({ full: false, max: false });
  const [priv, setPriv] = useState({ mic: [], cam: [] });
  const [perms, setPerms] = useState([]);          // pedidos de permissão do Claude Code esperando você
  const [sessions, setSessions] = useState({});    // sessões do Claude Code vistas pelos hooks
  const [claudeView, setClaudeView] = usePersist("claudeView", "live");
  const [usage, setUsage] = useState(null);
  const [calEvents, setCalEvents] = useState([]);
  const [calStatus, setCalStatus] = useState("");
  const [clips, setClips] = useState([]);
  const [update, setUpdate] = useState(null);
  const [updStatus, setUpdStatus] = useState({ state: "idle" });
  const [appVersion, setAppVersion] = useState("");
  const [dismissed, setDismissed] = useState({});  // cards de ação dispensados
  const [askFile, setAskFile] = useState(null);    // arquivo anexado de propósito (zona "Perguntar à IA")
  const [dropZone, setDropZone] = useState("stash");
  const [reminders, setReminders] = useState([]);
  const [mixOpen, setMixOpen] = useState(false);
  const [quick, setQuick] = usePersist("quick.v1", []);
  const [qForm, setQFormState] = useState(null);  // { kind, label, target } enquanto adiciona
  const setQForm = (v) => setQFormState((cur) => { const n = typeof v === "function" ? v(cur) : v; return n; });
  const [qEdit, setQEdit] = useState(false);
  const [qIcons, setQIcons] = useState({});
  const [sys, setSys] = useState(null);
  const [rep, setRep] = useState({ note: "", shot: true, status: "idle", result: null });
  const [wake, setWake] = useState({ on: false });
  const [focusLog, setFocusLog] = usePersist("focusLog.v1", []);
  const [mix, setMix] = useState(null);
  const mixSend = useRef({});
  const [ta, setTa] = useState({ text: "", status: "idle", action: "", result: "", error: "" }); // ações no texto selecionado
  const [dueRems, setDueRems] = useState([]);       // lembretes que tocaram e esperam você
  const [keyDraft, setKeyDraft] = useState({ gemini: "", anthropic: "" });
  const [showKey, setShowKey] = useState(false);

  const REAL = !!bridge.desktop;
  const [dPlaying, setDPlaying] = useState(false);
  const [dPos, setDPos] = useState(44);
  const [dShuffle, setDShuffle] = useState(false);
  const [dRepeat, setDRepeat] = useState(true);
  const [media, setMedia] = useState({ active: false });
  const [art, setArt] = useState(null);
  const [artColor, setArtColor] = useState(null);
  const [lyrics, setLyrics] = useState({ key: "", lines: null, status: "idle" });
  const [volume, setVolume] = useState(62);
  const [muted, setMuted] = useState(false);
  const [hud, setHud] = useState(false);
  const [hooksOn, setHooksOn] = useState(false);
  const [hooksMsg, setHooksMsg] = useState("");

  const [timerTotal, setTimerTotal] = useState(1500);
  const [remaining, setRemaining] = useState(1500);
  const [timerRunning, setTimerRunning] = useState(false);

  const [claude, setClaude] = useState({ status: "idle", title: "", file: "", removed: "", added: "", plus: 0, minus: 0, stage: 0, startedAt: 0, doneAt: 0, note: "" });

  const [dnd, setDnd] = usePersist("dnd", false);
  const [sleepLeft, setSleepLeft] = useState(null);
  const [locked, setLocked] = usePersist("locked", false);
  const [privacy, setPrivacy] = usePersist("privacy", false);

  const [now, setNow] = useState(new Date());
  const [showLyric, setShowLyric] = useState(true);
  const [weather, setWeather] = useState(null);

  const [waterLog, setWaterLog] = usePersist("water", { day: "", count: 0 });
  const todayKey = now.toDateString();
  const water = waterLog.day === todayKey ? waterLog.count : 0;
  const setWater = (fnOrVal) => setWaterLog((w) => {
    const cur = w.day === todayKey ? w.count : 0;
    return { day: todayKey, count: typeof fnOrVal === "function" ? fnOrVal(cur) : fnOrVal };
  });

  /* Localização (Brasil) */
  const [locId, setLocId] = usePersist("loc", "auto");
  const [ipLoc, setIpLoc] = useState(null);

  const [todos, setTodos] = usePersist("todos", INITIAL_TODOS);
  const [adding, setAdding] = useState(false);
  const [newTask, setNewTask] = useState("");
  const [editing, setEditing] = useState(false);
  const [calView, setCalView] = useState("week");

  /* Arquivos (stash) */
  const [stash, setStash] = useState([]);
  const [icons, setIcons] = useState({});
  const [dragging, setDragging] = useState(false);
  const dragDepth = useRef(0);
  const draggingOut = useRef(false);

  /* Ask */
  const [query, setQuery] = useState("");
  const [askPhase, setAskPhase] = useState("idle"); // idle | thinking | streaming | done
  const [askQ, setAskQ] = useState("");
  const [answer, setAnswer] = useState("");
  const [askError, setAskError] = useState(null);
  const [sources, setSources] = useState([]);
  const [answerMeta, setAnswerMeta] = useState(null);
  const [messages, setMessages] = useState([]);
  const [copied, setCopied] = useState(null);
  const [rates, setRates] = useState(null);
  const [listening, setListening] = useState(false);

  /* Translate */
  const [tText, setTText] = useState("");
  const [tSrc, setTSrc] = useState("Detect");
  const [tTgt, setTTgt] = usePersist("tTgt.v2", "English");
  const [tOut, setTOut] = useState(null);
  const [tDetected, setTDetected] = useState(null);
  const [tStatus, setTStatus] = useState("idle");
  const [tEngine, setTEngine] = useState("");

  /* OCR */
  const [ocrPhase, setOcrPhase] = useState("reading");
  const [ocrText, setOcrText] = useState("");
  const [ocrError, setOcrError] = useState("");
  const [ocrCopied, setOcrCopied] = useState(false);
  const [ocrQ, setOcrQ] = useState("");

  /* Notificações flutuantes + mascote */
  const [toasts, setToasts] = useState([]);
  const [success, setSuccess] = useState(false);
  const [devMood, setDevMood] = useState(null);

  /* ─── Música unificada (real no Windows, demo no navegador) ─── */
  const playing = REAL ? media.active && media.status === 4 : dPlaying;
  const duration = REAL ? Math.max(0, media.duration || 0) : DURATION;
  const pos = REAL
    ? (media.active ? Math.max(0, Math.min(duration || 1e9, (media.position || 0) + (playing && media.updated ? Math.max(0, now.getTime() - media.updated) / 1000 : 0))) : 0)
    : dPos;
  const shuffle = REAL ? !!media.shuffle : dShuffle;
  const repeat = REAL ? media.repeat > 0 : dRepeat;
  const trackTitle = REAL ? (media.active ? media.title || "Sem título" : "Nada tocando") : "Neon Drive";
  const trackArtist = REAL ? (media.active ? media.artist || appName(media.app) : "Toque música em qualquer app") : "Pixel Kids";
  const lyricLines = REAL ? lyrics.lines : LYRICS;
  const setPos = (sec) => (REAL ? media.canSeek && bridge.mediaCmd("seek", sec) : setDPos(sec));
  const setShuffle = (f) => { const v = typeof f === "function" ? f(shuffle) : f; REAL ? bridge.mediaCmd("shuffle", v) : setDShuffle(v); };
  const setRepeat = (f) => { const v = typeof f === "function" ? f(repeat) : f; REAL ? bridge.mediaCmd("repeat", v ? 2 : 0) : setDRepeat(v); };
  const setPlaying = (f) => { const v = typeof f === "function" ? f(playing) : f; REAL ? bridge.mediaCmd(v ? "play" : "pause") : setDPlaying(v); };
  const nextTrack = () => (REAL ? bridge.mediaCmd("next") : setDPos(0));
  const prevTrack = () => (REAL ? bridge.mediaCmd("prev") : setDPos(0));

  /* ─── Claude Code (hooks) ─── */
  const claudeStatus = claude.status;
  const claudeRunning = claude.status === "working";
  const done = claude.status === "done";
  const claudeElapsed = claude.startedAt ? Math.max(0, Math.floor(((done ? claude.doneAt : now.getTime()) - claude.startedAt) / 1000)) : 0;
  const claudeRecent = done && now.getTime() - claude.doneAt < 12000;
  const claudeLabel = done ? "Done" : claude.note ? "Needs you" : claudeRunning ? STAGE_LABEL[claude.stage] : "Idle";

  /* ─── Localização ativa ─── */
  const loc = locId === "auto"
    ? (ipLoc && ipLoc.country === "BR" ? { ...ipLoc, id: "auto", auto: true } : { ...BR_CITIES[1], id: "auto", auto: true, approx: true })
    : BR_CITIES.find((c) => c.id === locId) || BR_CITIES[1];
  const locLabel = loc.auto ? (ipLoc?.country === "BR" ? `${ipLoc.city} · atual` : "São Paulo · padrão") : loc.city;
  const LOC_IDS = ["auto", ...BR_CITIES.map((c) => c.id)];

  /* Modo compacto derivado do que está acontecendo de verdade */
  const nowMs = now.getTime();
  const meetingSoon = calEvents.find((e) => !e.allDay && e.start - nowMs <= 10 * 60e3 && nowMs < e.start + 5 * 60e3) || null;
  const usageHigh = !!(usage?.window && usage.window.pct >= 80);
  const mode = hud ? "volume" : claudeRunning || claudeRecent ? "claude" : meetingSoon ? "meeting" : timerRunning ? "timer" : playing ? "music" : usageHigh ? "usage" : "idle";
  const dndRef = useRef(dnd); dndRef.current = dnd;
  const settingsRef = useRef(settings); settingsRef.current = settings;
  const askFileRef = useRef(null);
  const volSeen = useRef(false);
  const hudTimer = useRef(null);

  const askInputRef = useRef(null);
  const tAreaRef = useRef(null);
  const ocrInputRef = useRef(null);
  const addInputRef = useRef(null);
  const hoverTimer = useRef(null);
  const timers = useRef([]);
  const aiReq = useRef(null);
  const answerRef = useRef("");
  const recRef = useRef(null);
  const recorderRef = useRef(null);
  const voiceFallback = useRef(false);
  const viewRef = useRef(view); viewRef.current = view;
  const tabRef = useRef(tab); tabRef.current = tab;
  const leaveTimer = useRef(null);
  const addingRef = useRef(false); addingRef.current = adding;
  const qFormRef = useRef(null); qFormRef.current = qForm;
  const qInputRef = useRef(null);
  const islandBoxRef = useRef(null);

  const timerTotalRef = useRef(timerTotal); timerTotalRef.current = timerTotal;
  const devSoundIdx = useRef(-1);
  const later = (fn, ms) => { const id = setTimeout(fn, ms); timers.current.push(id); return id; };
  const clearTimers = () => { timers.current.forEach(clearTimeout); timers.current = []; };
  const copy = async (text, key) => {
    const ok = await bridge.copy(text);
    if (ok && key) { setCopied(key); setTimeout(() => setCopied((c) => (c === key ? null : c)), 1500); }
    return ok;
  };
  /* Sons (respeitam o Não incomodar do Kon) */
  const sfx = (ev) => { const st = settingsRef.current; if (st.soundsOn === false || dndRef.current) return; playSound(ev, st.soundPack || "retro", st.soundVol ?? 0.6); };
  const toast = (text, kind = "info", icon = null) => {
    if (kind === "error") sfx("error");
    const id = uid();
    setToasts((t) => [...t.slice(-2), { id, text, kind, icon }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 2800);
  };
  const flashSuccess = () => { setSuccess(true); setTimeout(() => setSuccess(false), 1300); };

  /* ─── Configurações ─── */
  useEffect(() => { bridge.getSettings().then((st) => setSettings({ ...st, loaded: true })); }, []);
  const saveSettings = async (patch) => { setSettings((cur) => ({ ...cur, ...patch })); const st = await bridge.setSettings(patch); setSettings({ ...st, loaded: true }); };
  const saveKey = async (provider) => {
    const k = keyDraft[provider].trim();
    if (!k) return;
    setSettings({ ...(await bridge.setKey(provider, k)), loaded: true });
    setKeyDraft((d) => ({ ...d, [provider]: "" }));
    toast(`Chave ${provider === "anthropic" ? "Claude" : "Gemini"} salva com segurança`, "success");
  };
  const providerHasKey = (p) => (p === "claude" ? settings.hasAnthropicKey : settings.hasGeminiKey);

  /* Primeira vez: boas-vindas em 3 passos */
  useEffect(() => {
    if (!settings.loaded || settings.onboarded) return;
    const t = setTimeout(() => { setObStep(0); openSheet("welcome"); }, 1400);
    return () => clearTimeout(t);
  }, [settings.loaded]); // eslint-disable-line

  /* ─── Relógios ─── */
  useEffect(() => { const t = setInterval(() => setNow(new Date()), 1000); return () => clearInterval(t); }, []);
  useEffect(() => {
    if (REAL || !dPlaying) return;
    const t = setInterval(() => setDPos((p) => {
      if (p + 1 >= DURATION) { if (dRepeat) return 0; setDPlaying(false); return DURATION; }
      return p + 1;
    }), 1000);
    return () => clearInterval(t);
  }, [REAL, dPlaying, dRepeat]);
  useEffect(() => {
    if (!timerRunning) return;
    const t = setInterval(() => setRemaining((r) => {
      if (r <= 1) {
        setTimerRunning(false); toast("Foco concluído — hora de uma pausa", "success", "bell"); flashSuccess();
        setFocusLog((l) => [...l.slice(-300), { at: Date.now(), minutes: Math.max(1, Math.round(timerTotalRef.current / 60)) }]);
        return 0;
      }
      return r - 1;
    }), 1000);
    return () => clearInterval(t);
  }, [timerRunning]);
  useEffect(() => {
    if (sleepLeft === null) return;
    if (sleepLeft <= 0) { setPlaying(false); setSleepLeft(null); return; }
    const t = setTimeout(() => setSleepLeft((s) => (s === null ? null : s - 1)), 1000);
    return () => clearTimeout(t);
  }, [sleepLeft]);
  useEffect(() => {
    if (mode !== "music") return;
    const t = setInterval(() => setShowLyric((v) => !v), 4000);
    return () => clearInterval(t);
  }, [mode]);

  /* ─── Mídia do Windows (GSMTC) ─── */
  /* Capa: a do Windows (exata) vence; enquanto ela não chega, a do catálogo entra em ~0,3 s.
     Capas já vistas ficam em memória, então voltar a uma música mostra a capa na hora. */
  const thumbCache = useRef(new Map());
  const nativeArt = useRef(new Set());
  useEffect(() => bridge.on("media:state", (m) => {
    const key = `${m.title}|${m.artist}`;
    if (m.thumb?.data) {
      const url = `data:${m.thumb.mime || "image/png"};base64,${m.thumb.data}`;
      thumbCache.current.set(key, url);
      if (thumbCache.current.size > 40) thumbCache.current.delete(thumbCache.current.keys().next().value);
      nativeArt.current.add(key);
      setArt(url);
    } else if (m.trackChanged || !m.active) setArt(m.active ? thumbCache.current.get(key) || null : null);
    setMedia(m);
  }), []);
  useEffect(() => {
    if (!art) { setArtColor(null); return; }
    const img = new Image();
    if (!art.startsWith("data:")) img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const c = document.createElement("canvas"); c.width = c.height = 12;
        const ctx = c.getContext("2d"); ctx.drawImage(img, 0, 0, 12, 12);
        const d = ctx.getImageData(0, 0, 12, 12).data;
        let r = 0, g = 0, b = 0, n = 0;
        for (let i = 0; i < d.length; i += 4) { const sat = Math.max(d[i], d[i + 1], d[i + 2]) - Math.min(d[i], d[i + 1], d[i + 2]); const wgt = 1 + sat / 40; r += d[i] * wgt; g += d[i + 1] * wgt; b += d[i + 2] * wgt; n += wgt; }
        setArtColor(`rgb(${Math.round(r / n)},${Math.round(g / n)},${Math.round(b / n)})`);
      } catch { setArtColor(null); }
    };
    img.src = art;
  }, [art]);
  const lyricKey = REAL && media.active ? `${media.title}|${media.artist}` : "";
  useEffect(() => {
    if (!lyricKey) { setLyrics({ key: "", lines: null, status: "idle" }); return; }
    let alive = true;
    setLyrics({ key: lyricKey, lines: null, status: "loading" });
    getLyrics({ title: media.title, artist: media.artist, album: media.album, duration: media.duration })
      .then((r) => alive && setLyrics({ key: lyricKey, lines: r?.lines || null, synced: !!r?.synced, status: r ? "ok" : "none" }))
      .catch(() => alive && setLyrics({ key: lyricKey, lines: null, status: "none" }));
    return () => { alive = false; };
  }, [lyricKey]); // eslint-disable-line

  /* ─── Volume do Windows ─── */
  useEffect(() => bridge.on("helper:ready", () => { volSeen.current = false; }), []);
  useEffect(() => bridge.on("volume:state", (v) => {
    setVolume(v.level); setMuted(!!v.muted);
    if (!volSeen.current) { volSeen.current = true; return; }
    if (dndRef.current) return;
    setHud(true);
    clearTimeout(hudTimer.current);
    hudTimer.current = setTimeout(() => setHud(false), 1600);
  }), []);

  /* ─── Claude Code ─── */
  useEffect(() => { bridge.claudeHooks("status").then((r) => setHooksOn(!!r?.installed)); }, []);
  useEffect(() => bridge.on("claude:event", (ev) => {
    const name = ev?.hook_event_name;
    const t = Date.now();
    const sid = ev?.session_id;
    if (sid) {
      setSessions((m) => {
        if (name === "SessionEnd") { const n = { ...m }; delete n[sid]; return n; }
        const cur = m[sid] || { id: sid, cwd: ev.cwd || "", title: "", status: "idle", at: t };
        const status = name === "Stop" ? "done" : name === "Notification" || name === "PermissionRequest" ? "waiting" : name === "SessionStart" ? "idle" : "working";
        return { ...m, [sid]: { ...cur, cwd: ev.cwd || cur.cwd, title: name === "UserPromptSubmit" ? firstLine(ev.prompt).slice(0, 80) : cur.title, status, at: t } };
      });
    }
    if (name === "PermissionRequest" && ev.kon_perm_id) {
      setPerms((list) => [...list, { id: ev.kon_perm_id, tool: ev.tool_name || "", input: ev.tool_input || {}, cwd: ev.cwd || "", session: sid, at: t }]);
      return;
    }
    if (name === "SessionStart" || name === "SessionEnd") return;
    if (name === "Stop") { sfx("done"); toast("Claude Code terminou a tarefa", "success", "check"); setTimeout(() => refreshUsage(), 1500); }
    if (name === "Notification") sfx("attention");
    setClaude((c) => {
      const fresh = { status: "working", title: c.title || "Working…", file: "", removed: "", added: "", plus: 0, minus: 0, stage: 0, startedAt: t, doneAt: 0, note: "" };
      const base = c.status === "working" ? c : fresh;
      if (name === "UserPromptSubmit") return { ...fresh, title: firstLine(ev.prompt).slice(0, 90) || "Working…" };
      if (name === "PreToolUse") {
        const tool = ev.tool_name || "", inp = ev.tool_input || {};
        if (/^(Edit|MultiEdit|Write|NotebookEdit)$/.test(tool)) {
          const d = diffOf(tool, inp);
          return { ...base, note: "", stage: Math.max(base.stage, 1), ...d, plus: base.plus + d.plus, minus: base.minus + d.minus };
        }
        if (tool === "Bash" && TEST_RE.test(inp.command || "")) return { ...base, note: "", stage: Math.max(base.stage, 2) };
        return { ...base, note: "" };
      }
      if (name === "Notification") return { ...base, note: ev.message || "Claude needs your input" };
      if (name === "Stop") return c.status === "idle" ? c : { ...base, status: "done", stage: 3, doneAt: t, note: "" };
      return c;
    });
  }), []);
  useEffect(() => bridge.on("claude:perm-closed", ({ id }) => setPerms((list) => list.filter((p) => p.id !== id))), []);
  const decidePerm = (id, decision) => {
    setPerms((list) => list.filter((p) => p.id !== id));
    bridge.claudeDecide(id, decision);
    if (decision === "allow" || decision === "always") flashSuccess();
  };
  /* Uso do Claude Code (transcripts locais) */
  const refreshUsage = async () => { const u = await bridge.usage(); if (u?.ok) setUsage(u); };
  useEffect(() => { refreshUsage(); const t = setInterval(refreshUsage, 60e3); return () => clearInterval(t); }, [settings.claudePlan]); // eslint-disable-line
  /* Agenda (ICS) */
  const refreshCal = async (force) => {
    const r = await bridge.calendar(force);
    if (r?.events) setCalEvents(r.events);
    setCalStatus(r?.empty ? "" : r?.error ? `Erro: ${r.error}` : r?.at ? `Atualizada às ${fmtHM(r.at)}` : "");
  };
  useEffect(() => { refreshCal(); const t = setInterval(() => refreshCal(), 10 * 60e3); return () => clearInterval(t); }, [settings.icsUrl]); // eslint-disable-line
  useEffect(() => { bridge.wakeGet().then(setWake); return bridge.on("wake:state", setWake); }, []);
  /* Avisos vindos do processo principal (PC sobrecarregado, falha de captura etc.) */
  useEffect(() => bridge.on("toast", (t) => t?.text && toast(t.text, t.kind || "info", t.icon || null)), []);
  /* Lembretes */
  useEffect(() => {
    bridge.remList().then(setReminders);
    const onDue = (r) => { sfx("reminder"); setDueRems((l) => (l.some((x) => x.id === r.id) ? l : [...l, r])); bridge.remList().then(setReminders); };
    const offA = bridge.on("reminder:due", onDue);
    const offB = bridge.on("reminders:changed", setReminders);
    const web = (e) => onDue(e.detail);
    window.addEventListener("kon-web-reminder", web);
    return () => { offA(); offB(); window.removeEventListener("kon-web-reminder", web); };
  }, []);
  /* Focus de verdade: liga o Não incomodar do Windows enquanto o timer roda */
  const dndOnRef = useRef(false);
  useEffect(() => {
    const want = timerRunning && settings.focusDnd !== false && REAL;
    if (want === dndOnRef.current) return;
    dndOnRef.current = want;
    bridge.focusDnd(want);
  }, [timerRunning, settings.focusDnd]); // eslint-disable-line
  /* Sistema: atualiza a cada 2 s com a página aberta (processos a cada 4 s) */
  useEffect(() => {
    if (view !== "panel" || tab !== "system") return;
    let alive = true, n = 0;
    const load = () => bridge.sysInfo(n++ % 2 === 0).then((r) => alive && r && setSys((cur) => ({ ...r, top: r.top?.length ? r.top : cur?.top || [], topMem: r.topMem || cur?.topMem || null })));
    load();
    const t = setInterval(load, 2000);
    return () => { alive = false; clearInterval(t); };
  }, [view, tab]);
  /* Atalhos: ícones de apps e arquivos */
  useEffect(() => {
    quick.forEach((q) => {
      if (qIcons[q.id] !== undefined || q.kind === "cmd") return;
      setQIcons((m) => ({ ...m, [q.id]: null }));
      if (q.kind === "site") { const host = q.target.replace(/^https?:\/\//, "").split("/")[0]; setQIcons((m) => ({ ...m, [q.id]: `https://www.google.com/s2/favicons?domain=${host}&sz=64` })); return; }
      bridge.stashIcon(q.target).then((url) => url && setQIcons((m) => ({ ...m, [q.id]: url })));
    });
  }, [quick]); // eslint-disable-line
  const addQuick = () => {
    if (!qForm?.target?.trim()) return;
    const t = qForm.target.trim();
    const label = qForm.label?.trim() || (qForm.kind === "site" ? t.replace(/^https?:\/\//, "").split("/")[0] : qForm.kind === "cmd" ? t.split(" ")[0] : t.split(/[\\/]/).filter(Boolean).pop().replace(/\.(exe|lnk|url)$/i, ""));
    setQuick((l) => [...l, { id: uid(), kind: qForm.kind, label, target: t }]);
    setQForm(null);
  };
  /* Mixer (página Música): atualiza a cada 3 s enquanto aberto */
  useEffect(() => {
    if (!mixOpen || view !== "panel" || tab !== "music") return;
    let alive = true;
    const load = () => bridge.mixGet().then((m) => alive && m && setMix(m));
    load();
    const t = setInterval(load, 3000);
    return () => { alive = false; clearInterval(t); };
  }, [mixOpen, view, tab]);
  const setAppVol = (app, v) => {
    setMix((m) => ({ ...m, apps: m.apps.map((a) => (a.name === app.name ? { ...a, volume: v } : a)) }));
    const now = Date.now(), last = mixSend.current[app.name] || 0;
    clearTimeout(mixSend.current[app.name + ":t"]);
    const fire = () => { mixSend.current[app.name] = Date.now(); bridge.mixAppVolume(app.pids, v); };
    if (now - last > 120) fire(); else mixSend.current[app.name + ":t"] = setTimeout(fire, 120);
  };
  /* Ações no texto selecionado (Ctrl+Alt+R) */
  useEffect(() => bridge.on("textact:open", ({ text, error }) => {
    clearTimers();
    setTa({ text: text || "", error: error || "", status: "idle", action: "", result: "" });
    setView("textact");
  }), []);
  const runTextAction = async (id) => {
    const a = TEXT_ACTIONS.find((x) => x[0] === id);
    const provider = providerHasKey(settings.provider) ? settings.provider : settings.hasGeminiKey ? "gemini" : settings.hasAnthropicKey ? "claude" : null;
    if (!provider) { setTa((t) => ({ ...t, action: id, status: "error", result: "Adicione uma chave do Gemini ou do Claude em Ajustes." })); return; }
    setTa((t) => ({ ...t, action: id, status: "working", result: "" }));
    const r = await bridge.aiOnce({ provider, system: "Você edita textos para o usuário. Responda somente com o texto final, sem aspas, sem comentários e sem explicar o que fez (exceto quando pedirem uma explicação).", messages: [{ role: "user", text: `${a[2]}\n\n"""\n${ta.text}\n"""` }] });
    if (r.ok) { setTa((t) => ({ ...t, status: "done", result: String(r.text || "").trim().replace(/^"""|"""$/g, "").trim() })); flashSuccess(); }
    else setTa((t) => ({ ...t, status: "error", result: r.code === "NO_KEY" ? "Adicione sua chave em Ajustes." : r.message || "Não deu certo." }));
  };
  const closeReminder = async (r, snooze) => {
    setDueRems((l) => l.filter((x) => x.id !== r.id));
    bridge.remAck();
    if (snooze) { setReminders(await bridge.remSnooze(r.id, snooze)); toast(`Adiado ${snooze} min`, "info", "bell"); }
  };
  /* Área de transferência, tela cheia, privacidade, atualizações */
  useEffect(() => { bridge.clipList().then(setClips); return bridge.on("clip:changed", setClips); }, []);
  useEffect(() => { bridge.fgState().then(setFg); return bridge.on("fg:state", setFg); }, []);
  useEffect(() => bridge.on("privacy:state", (p) => setPriv({ mic: p.mic || [], cam: p.cam || [] })), []);
  useEffect(() => {
    bridge.updateInfo().then((u) => { setAppVersion(u?.version || ""); if (u?.ready) setUpdate(u.ready); if (u?.status) setUpdStatus(u.status); });
    const off = bridge.on("update:status", setUpdStatus);
    const off2 = bridge.on("update:ready", setUpdate);
    return () => { off(); off2(); };
  }, []);
  const toggleHooks = async () => {
    setHooksMsg("");
    const r = await bridge.claudeHooks(hooksOn ? "uninstall" : "install");
    setHooksOn(!!r?.installed);
    if (!r?.ok) setHooksMsg(r?.error || "Não consegui alterar as configurações do Claude Code");
    else setHooksMsg(r.installed ? "Conectado. Reabra as sessões do Claude Code." : "Desconectado.");
  };

  /* ─── Dados híbridos (cache instantâneo + atualização em segundo plano) ─── */
  useEffect(() => {
    const load = () => getRates((rec) => setRates(rec)).then((rec) => rec && setRates(rec));
    load();
    const t = setInterval(load, 5 * 60e3);
    return () => clearInterval(t);
  }, []);
  useEffect(() => { getIpLocation().then((r) => r?.value && setIpLoc(r.value)); }, []);
  useEffect(() => {
    let alive = true;
    const load = () => getWeather(loc, (rec) => alive && setWeather(rec.value)).then((rec) => alive && rec && setWeather(rec.value));
    load();
    const t = setInterval(load, 10 * 60e3);
    return () => { alive = false; clearInterval(t); };
  }, [loc.lat, loc.lon]); // eslint-disable-line
  const cn = cityNow(loc.tz || Intl.DateTimeFormat().resolvedOptions().timeZone, now);
  const locTz = loc.tz || Intl.DateTimeFormat().resolvedOptions().timeZone;
  const nowHourIso = locIsoHour(locTz, now);
  const rainToastKey = useRef("");
  useEffect(() => {
    if (!weather?.hourly) return;
    const idx = weather.hourly.findIndex((x) => x.t.slice(0, 13) === nowHourIso);
    if (idx < 0) return;
    const next = weather.hourly.slice(idx + 1, idx + 3).find((x) => x.rain >= 60);
    if (next && rainToastKey.current !== next.t) {
      rainToastKey.current = next.t;
      toast(`Chuva provável às ${+next.t.slice(11, 13)}h · ${next.rain}%`, "info", "rain");
    }
  }, [weather, nowHourIso]); // eslint-disable-line

  /* Migração: Prayer saiu, Clima entrou */
  useEffect(() => {
    if (pageOrder.includes("prayer") || !pageOrder.includes("weather")) {
      setPageOrder((o) => { const n = o.filter((x) => x !== "prayer" && PAGE_META[x]); return n.includes("weather") ? n : [...n.slice(0, 3), "weather", ...n.slice(3)]; });
      setPageOn((o) => ({ ...o, weather: o.weather ?? true }));
    }
    if (!PAGE_META[tab] && !["sleep", "lock"].includes(tab)) setTab("home");
    // páginas novas (ex.: Área de transferência) entram na posição padrão
    const missing = DEFAULT_ORDER.filter((id) => !pageOrder.includes(id) && id !== "weather");
    if (missing.length) {
      setPageOrder((o) => { const n = [...o]; for (const id of missing) { const at = DEFAULT_ORDER.indexOf(id); n.splice(Math.min(at, n.length), 0, id); } return n; });
      setPageOn((o) => ({ ...o, ...Object.fromEntries(missing.map((id) => [id, true])) }));
    }
  }, []); // eslint-disable-line

  /* Catálogo do iTunes em paralelo, já na troca de faixa (fica em cache por 30 dias) */
  useEffect(() => {
    if (!REAL || !media.active || !media.title) return;
    const key = `${media.title}|${media.artist}`;
    let alive = true;
    const t0 = Date.now();
    getCoverArt(media.title, media.artist, media.album).then((url) => {
      if (!alive || !url) return;
      // dá 300 ms de vantagem para a capa do Windows, que é a exata
      const img = new Image(); // pré-carrega para não piscar um quadro vazio
      img.onload = img.onerror = () => setTimeout(() => { if (alive && !nativeArt.current.has(key)) setArt((cur) => cur || url); }, Math.max(0, 300 - (Date.now() - t0)));
      img.src = url;
    }).catch(() => {});
    return () => { alive = false; };
  }, [REAL, media.active, media.title, media.artist]); // eslint-disable-line

  /* ─── Arquivos (File Stash — sem IA) ─── */
  const refreshStash = async () => setStash(await bridge.stashList());
  useEffect(() => { refreshStash(); }, []);
  useEffect(() => {
    stash.forEach((it) => {
      if (icons[it.id] !== undefined) return;
      setIcons((m) => ({ ...m, [it.id]: null }));
      bridge.stashIcon(it.path).then((url) => url && setIcons((m) => ({ ...m, [it.id]: url })));
    });
  }, [stash]); // eslint-disable-line
  const removeStash = async (id) => { setStash(await bridge.stashRemove(id)); };
  useEffect(() => {
    const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes("Files");
    const onEnter = (e) => {
      if (!hasFiles(e) || draggingOut.current) return;
      e.preventDefault();
      dragDepth.current++;
      setDragging(true);
    };
    const onOver = (e) => {
      if (!hasFiles(e) || draggingOut.current) return;
      e.preventDefault(); e.dataTransfer.dropEffect = "copy";
      const z = e.target?.closest?.("[data-zone]")?.dataset.zone;
      if (z) setDropZone(z);
    };
    const onLeave = (e) => {
      if (!hasFiles(e) || draggingOut.current) return;
      dragDepth.current = Math.max(0, dragDepth.current - 1);
      if (!dragDepth.current) setDragging(false);
    };
    const onDrop = async (e) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      dragDepth.current = 0;
      setDragging(false);
      if (draggingOut.current) return;
      const zone = e.target?.closest?.("[data-zone]")?.dataset.zone || "stash";
      setDropZone("stash");
      const paths = [...e.dataTransfer.files].map((f) => bridge.pathForFile(f)).filter(Boolean);
      if (!paths.length) { toast(REAL ? "Não consegui ler esse item" : "Arquivos só funcionam no app desktop", "error"); return; }
      if (zone === "ask") {
        // você escolheu enviar: só agora o arquivo é lido, e só para a IA selecionada
        const p0 = paths[0];
        openAsk("", { path: p0, name: p0.split(/[\\/]/).pop() });
        if (paths.length > 1) toast("Anexei só o primeiro arquivo", "info");
        return;
      }
      if (zone === "zip") {
        toast("Compactando…", "info", "file");
        const z = await bridge.zip(paths);
        if (!z.ok) { toast(z.error || "Não consegui compactar", "error"); return; }
        const r2 = await bridge.stashAdd([z.path]);
        setStash(r2.list); flashSuccess();
        toast(`Compactado: ${z.path.split(/[\\/]/).pop()}`, "success", "file");
        return;
      }
      const r = await bridge.stashAdd(paths);
      setStash(r.list);
      flashSuccess();
      if (r.added.length) sfx("file");
      toast(r.added.length === 1 ? `Guardado na ilha: ${r.added[0].name}` : r.added.length ? `${r.added.length} itens guardados na ilha` : "Esses itens já estavam na ilha", "success", "file");
    };
    window.addEventListener("dragenter", onEnter);
    window.addEventListener("dragover", onOver);
    window.addEventListener("dragleave", onLeave);
    window.addEventListener("drop", onDrop);
    return () => {
      window.removeEventListener("dragenter", onEnter);
      window.removeEventListener("dragover", onOver);
      window.removeEventListener("dragleave", onLeave);
      window.removeEventListener("drop", onDrop);
    };
  }, [REAL]); // eslint-disable-line
  const dragOut = (e, it) => {
    e.preventDefault();
    draggingOut.current = true;
    bridge.stashDrag(it.path);
    setTimeout(() => { draggingOut.current = false; }, 1500);
  };

  /* ─── Páginas visíveis ─── */
  const visiblePages = pageOrder.filter((id) => pageOn[id] && PAGE_META[id]);
  useEffect(() => { if (PAGE_META[tab] && !pageOn[tab]) setTab(visiblePages[0]); }, [pageOn, tab]); // eslint-disable-line

  /* ─── Ações ─── */
  const toCompact = () => { clearTimers(); setView("compact"); };
  // resposta imediata no botão; o estado real do Windows chega em seguida e confirma
  const togglePlay = () => { if (REAL) { setMedia((m) => (m.active ? { ...m, status: m.status === 4 ? 5 : 4, position: pos, updated: Date.now() } : m)); return bridge.mediaCmd("toggle"); } if (dPos >= DURATION) setDPos(0); setDPlaying((p) => !p); };
  const startTimer = () => { if (remaining <= 0) setRemaining(timerTotal); setTimerRunning(true); };
  const toggleTimer = () => (timerRunning ? setTimerRunning(false) : startTimer());
  const stopTimer = () => { setTimerRunning(false); setTimerTotal(1500); setRemaining(1500); };
  const addFive = () => { setRemaining((r) => Math.min(r + 300, 3600)); setTimerTotal((t) => Math.min(t + 300, 3600)); };
  const pickMinutes = (m) => { setTimerTotal(m * 60); setRemaining(m * 60); };
  const openTab = (t) => setTab(t);
  const openPanel = (page) => { clearTimers(); if (page) openTab(page); setView("panel"); };
  const togglePage = (id, v) => { if (!v && visiblePages.length <= 1) return; setPageOn((o) => ({ ...o, [id]: v })); };
  const openSheet = (which = "pages") => { clearTimers(); setSheetTab(which); setView("sheet"); bridge.focus(); };

  const openAsk = (text = "", file = null) => {
    clearTimers();
    setAskFile(file);
    askFileRef.current = file;
    if (aiReq.current) bridge.aiAbort(aiReq.current);
    aiReq.current = null;
    setAskPhase("idle"); setAnswer(""); setAskQ(""); setAskError(null); setSources([]); setAnswerMeta(null); setMessages([]); setQuery(text);
    setView("ask"); bridge.focus();
    later(() => askInputRef.current?.focus({ preventScroll: true }), 60);
  };
  const submitAsk = (q, contextText) => {
    clearTimers();
    if (aiReq.current) bridge.aiAbort(aiReq.current);
    const rem = !contextText && !askFileRef.current ? parseReminder(q) : null;
    if (rem) {
      bridge.remAdd(rem).then(setReminders);
      setAskQ(q); setQuery(""); setAskError(null); setSources([]); setAnswerMeta(null); setMessages([]);
      setAnswer(`Combinado: ${describeWhen(rem.at)} eu te aviso no notch.\n${rem.text}`);
      setAskPhase("done"); setView("ask"); flashSuccess();
      return;
    }
    const provider = settingsRef.current.provider;
    const userText = contextText ? `Texto capturado da minha tela:\n"""\n${contextText}\n"""\n\n${q}` : q;
    const history = [...(contextText ? [] : messages), { role: "user", text: userText }];
    const id = uid();
    aiReq.current = id;
    answerRef.current = "";
    setMessages(history); setAskQ(q); setQuery(""); setAnswer(""); setAskError(null); setSources([]); setAnswerMeta(null);
    setAskPhase("thinking"); setView("ask");
    bridge.focus();
    later(() => askInputRef.current?.focus({ preventScroll: true }), 60);
    if (!REAL) {
      later(() => { setAnswer("O chat com Gemini/Claude funciona no app desktop do Kon."); setAskError({ code: "WEB" }); setAskPhase("done"); }, 600);
      return;
    }
    const file = askFileRef.current;
    if (file) { askFileRef.current = null; setAskFile((f) => (f ? { ...f, sent: true } : f)); }
    bridge.aiStream({ id, provider, system: CHAT_SYSTEM, messages: history, webSearch: provider === "gemini" && settingsRef.current.webSearch, file: file && !file.sent ? file.path : undefined });
  };
  useEffect(() => bridge.on("ai:event", (ev) => {
    if (ev.id !== aiReq.current) return;
    if (ev.type === "chunk") { answerRef.current += ev.text; setAnswer(answerRef.current); setAskPhase("streaming"); }
    else if (ev.type === "done") {
      setSources(ev.sources || []);
      setAnswerMeta({ model: ev.model, fellBack: ev.fellBack });
      setMessages((h) => [...h, { role: "assistant", text: answerRef.current }]);
      setAskPhase("done");
      flashSuccess();
      aiReq.current = null;
    } else if (ev.type === "error") {
      const p = PROVIDER_NAME[ev.provider] || "IA";
      setAnswer(ev.code === "NO_KEY"
        ? `Adicione sua chave ${ev.provider === "claude" ? "ANTHROPIC_API_KEY" : "GEMINI_API_KEY"} em Configurações para usar o ${p}.`
        : `${p}: ${ev.message}`);
      setAskError(ev);
      setAskPhase("done");
      aiReq.current = null;
    }
  }), []);
  const switchProvider = (p) => saveSettings({ provider: p || (settings.provider === "gemini" ? "claude" : "gemini") });

  /* ─── Voz (pt-BR): Web Speech; se indisponível no Electron, grava e transcreve com Gemini ─── */
  const stopVoice = () => {
    try { recRef.current?.stop(); } catch {}
    try { if (recorderRef.current?.state === "recording") recorderRef.current.stop(); } catch {}
    setListening(false);
  };
  const recordFallback = async () => {
    if (!settings.hasGeminiKey) { toast("Para ditar, adicione a chave do Gemini em Configurações", "error"); return; }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const rec = new MediaRecorder(stream);
      const chunks = [];
      rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
      rec.onstop = async () => {
        stream.getTracks().forEach((t) => t.stop());
        setListening(false);
        const blob = new Blob(chunks, { type: rec.mimeType || "audio/webm" });
        const b64 = await new Promise((res) => { const fr = new FileReader(); fr.onload = () => res(String(fr.result).split(",")[1]); fr.readAsDataURL(blob); });
        setQuery("Transcrevendo…");
        const r = await bridge.transcribe(b64, blob.type);
        setQuery(r.ok ? r.text : "");
        if (!r.ok) toast(r.code === "NO_KEY" ? "Adicione a chave do Gemini para ditar" : "Não consegui transcrever o áudio", "error");
        askInputRef.current?.focus({ preventScroll: true });
      };
      recorderRef.current = rec;
      rec.start();
      setListening(true);
      setTimeout(() => { if (rec.state === "recording") rec.stop(); }, 12000);
    } catch { setListening(false); toast("Microfone indisponível", "error"); }
  };
  const toggleVoiceRef = useRef(null);
  const toggleVoice = () => {
    if (listening) return stopVoice();
    const SR = window.webkitSpeechRecognition || window.SpeechRecognition;
    if (!SR || voiceFallback.current) return recordFallback();
    const rec = new SR();
    rec.lang = "pt-BR"; rec.interimResults = true; rec.continuous = false;
    let gotResult = false;
    rec.onresult = (e) => { gotResult = true; setQuery([...e.results].map((r) => r[0].transcript).join("")); };
    rec.onerror = (e) => {
      setListening(false);
      if (["network", "service-not-allowed", "not-allowed", "language-not-supported"].includes(e.error)) { voiceFallback.current = true; recordFallback(); }
    };
    rec.onend = () => { setListening(false); if (gotResult) askInputRef.current?.focus({ preventScroll: true }); };
    try { rec.start(); recRef.current = rec; setListening(true); } catch { voiceFallback.current = true; recordFallback(); }
  };

  toggleVoiceRef.current = toggleVoice;
  /* ─── Tradutor híbrido: IA quando há chave; dicionário local quando não há ─── */
  const tReq = useRef(0);
  const doTranslate = async () => {
    const text = tText;
    if (!text.trim()) return;
    const my = ++tReq.current;
    setTStatus("translating");
    const provider = providerHasKey(settings.provider) ? settings.provider : settings.hasGeminiKey ? "gemini" : settings.hasAnthropicKey ? "claude" : null;
    if (provider && REAL) {
      const r = await bridge.aiOnce({
        provider, system: TRANSLATE_SYSTEM,
        messages: [{ role: "user", text: `Source language: ${tSrc === "Detect" ? "auto-detect" : tSrc}. Target language: ${tTgt}.\nReturn JSON exactly like {"detected":"<source language name in English>","translation":"<translated text>"}.\nText:\n"""\n${text}\n"""` }],
      });
      if (my !== tReq.current) return;
      const j = r.ok ? parseJSONish(r.text) : null;
      if (j?.translation != null) { setTDetected(j.detected || null); setTOut(j.translation); setTEngine(PROVIDER_NAME[provider]); setTStatus("done"); return; }
    }
    if (my !== tReq.current) return;
    const det = tSrc === "Detect" ? detectLangLocal(text) : tSrc;
    const out = translateLocal(text, det, tTgt);
    setTDetected(det);
    setTEngine("local");
    if (out) { setTOut(out); setTStatus("done"); }
    else { setTOut("Sem tradução offline para este texto. Adicione uma chave do Gemini ou do Claude em Configurações para traduzir qualquer coisa."); setTStatus("error"); }
  };
  useEffect(() => {
    if (view !== "translate") return;
    if (!tText.trim()) { tReq.current++; setTOut(null); setTDetected(null); setTStatus("idle"); return; }
    setTStatus("translating");
    const id = setTimeout(doTranslate, 500);
    return () => clearTimeout(id);
  }, [tText, tSrc, tTgt, view]); // eslint-disable-line
  const openTranslate = (text = "") => {
    clearTimers();
    setTText(text); setTSrc("Detect"); setTOut(null); setTDetected(null); setTStatus("idle");
    setView("translate"); bridge.focus();
    later(() => tAreaRef.current?.focus({ preventScroll: true }), 80);
  };
  const cycle = (list, cur, dir) => list[(list.indexOf(cur) + dir + list.length) % list.length];
  const swapLangs = () => {
    const from = tSrc === "Detect" ? tDetected || "English" : tSrc;
    if (tStatus === "done" && tOut) setTText(tOut);
    setTSrc(tTgt);
    setTTgt(TARGETS.includes(from) ? from : "English");
  };

  /* ─── Smart OCR nativo ─── */
  const startOcr = () => { clearTimers(); setView("compact"); bridge.startOcr(); };
  useEffect(() => bridge.on("ocr:started", () => {
    clearTimers();
    setOcrText(""); setOcrError(""); setOcrQ(""); setOcrCopied(false); setOcrPhase("reading"); setView("ocr");
  }), []);
  useEffect(() => bridge.on("ocr:result", ({ text, error, copied: c }) => {
    setOcrText(text || ""); setOcrError(error || ""); setOcrCopied(!!c); setOcrPhase("result"); setView("ocr");
    if (c) { flashSuccess(); toast("Texto copiado para a área de transferência", "success", "check"); }
    setTimeout(() => ocrInputRef.current?.focus({ preventScroll: true }), 80);
  }), []);

  /* ─── Atalhos globais, blur e teclado ─── */
  useEffect(() => bridge.on("shortcut", (name) => {
    if (name === "ask") viewRef.current === "ask" ? toCompact() : openAsk();
    if (name === "panel") viewRef.current === "panel" ? toCompact() : openPanel();
    if (name === "settings") openSheet("settings");
    if (name === "wake") { openAsk(); setTimeout(() => toggleVoiceRef.current?.(), 250); }
    if (name === "report") { setRep({ note: "", shot: true, status: "idle", result: null }); openSheet("report"); }
    if (name === "clip") viewRef.current === "panel" && tabRef.current === "clip" ? toCompact() : openPanel("clip");
  }));
  useEffect(() => bridge.on("win:blurred", () => {
    if (["ask", "translate", "ocr", "sheet"].includes(viewRef.current)) toCompact();
    else if (viewRef.current === "panel" && !adding && !qFormRef.current) setView("compact");
  }));
  useEffect(() => {
    const onKey = (e) => {
      const k = e.key.toLowerCase();
      if (!REAL && e.ctrlKey && e.altKey && e.code === "Space") { e.preventDefault(); view === "ask" ? toCompact() : openAsk(); return; }
      if (!REAL && e.ctrlKey && e.altKey && k === "e") { e.preventDefault(); view === "panel" ? toCompact() : openPanel(); return; }
      if (e.key === "Escape") {
        if (listening) return stopVoice();
        if (view === "sheet") return setView("panel");
        if (view !== "compact") toCompact();
      }
    };
    const onDown = (e) => {
      if (islandBoxRef.current?.contains(e.target) || e.target.closest?.("[data-keep]")) return;
      if (view !== "compact") toCompact();
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onDown);
    return () => { window.removeEventListener("keydown", onKey); document.removeEventListener("mousedown", onDown); };
  });

  /* ─── Hover ─── */
  /* Abrir de propósito: passar o mouse (rápido), passar o mouse com atraso, ou só com clique */
  const onEnter = () => {
    clearTimeout(leaveTimer.current); // voltou antes de fechar: mantém aberto
    if (view !== "compact" || locked || dragging) return;
    clearTimeout(hoverTimer.current);
    setHover(true);
    if (actionRef.current || settings.openMode === "click") return;
    hoverTimer.current = setTimeout(() => { setView("panel"); setHover(false); }, settings.openMode === "delay" ? 450 : 110);
  };
  const onIslandClick = (e) => {
    if (view !== "compact" || dragging || actionRef.current || e.target.closest?.("button")) return;
    clearTimeout(hoverTimer.current);
    setHover(false);
    openPanel();
  };
  // fecha com uma pequena folga: passar rápido pela borda ou o painel mudar de tamanho não fecha nada
  const onLeave = () => {
    clearTimeout(hoverTimer.current);
    setHover(false);
    clearTimeout(leaveTimer.current);
    leaveTimer.current = setTimeout(() => { if (viewRef.current === "panel" && !addingRef.current && !qFormRef.current) setView("compact"); }, 280);
  };

  /* ─── Detecção inline (math 0 ms / câmbio com cache) ─── */
  let inline = null;
  const um = convertUnits(query);
  const cm = !um && query.match(CUR_RE);
  if (um) inline = { kind: "unit", text: um.text, error: um.error };
  else if (cm) {
    const from = cm[2].toUpperCase(), to = cm[3].toUpperCase();
    const R = rates?.value?.rates;
    if (R?.[from] && R?.[to]) inline = { kind: "currency", text: fmtCur((parseNum(cm[1]) / R[from]) * R[to], to) };
    else if (!rates) inline = { kind: "currency", text: "carregando…" };
    else inline = { kind: "currency", text: "moeda desconhecida" };
  } else if (MATH_RE.test(query) && /[+\-*×x÷/%]/i.test(query.replace(/=\s*$/, ""))) {
    const v = calc(query);
    if (v !== null) inline = { kind: "math", text: fmtNum(v) };
  }
  const ratesStale = rates && Date.now() - rates.at > RATES_TTL * 1.5;
  const ratesLabel = rates ? `${fmtHM(rates.at)}${ratesStale ? " · offline" : ""}` : "";
  const onQueryChange = (v) => setQuery(/^[\d\s.,+\-*×x÷/%()=]*$/i.test(v) ? v.replace(/\*/g, "×") : v);

  /* ─── Derivados ─── */
  const processing =
    dragging ||
    (view === "ask" && (askPhase === "thinking" || askPhase === "streaming")) ||
    (view === "ocr" && ocrPhase === "reading") ||
    (view === "translate" && tStatus === "translating") ||
    (view === "compact" && mode === "claude" && claudeRunning);
  const compactMood = devMood || (success ? "done" : claude.note ? "attention" : { idle: dnd ? "sleep" : "idle", music: "music", volume: "idle", timer: "calm", usage: "attention", meeting: "attention", claude: done ? "done" : "working" }[mode]);
  const panelMood = tab === "music" ? "happy" : tab === "focus" ? "calm" : tab === "claude" ? (done ? "love" : claudeRunning ? "reading" : "idle")
    : tab === "sleep" ? (dnd ? "sleep" : "calm") : compactMood;

  const answerLines = answer ? answer.split(/\r?\n/).filter((l, i, a) => l.trim() || (i > 0 && a[i - 1].trim())).map(cleanLine) : [];
  const askOpen = askPhase !== "idle" && (askPhase !== "thinking" || answer);
  const ocrLineCount = Math.max(1, (ocrError || ocrText || "").split(/\r?\n/).reduce((n, l) => n + Math.max(1, Math.ceil(l.length / 62)), 0));
  /* Card de ação padrão (mascote grande à esquerda, título curto, até 2 botões) */
  const perm = perms[0];
  const action = view !== "compact" || dragging ? null
    : perm ? { kind: "perm", key: perm.id, mood: "attention", ...permText(perm) }
      : dueRems[0] ? { kind: "reminder", key: `rem-${dueRems[0].id}`, mood: "attention", title: dueRems[0].text, sub: `Lembrete das ${fmtHM(dueRems[0].at)}${dueRems.length > 1 ? ` · +${dueRems.length - 1}` : ""}` }
      : update && !dismissed[`upd-${update.version}`] ? { kind: "update", key: `upd-${update.version}`, mood: "done", title: `Kon ${update.version} pronto para instalar`, sub: "Baixado em segundo plano · reinicia em segundos" }
        : meetingSoon && meetingSoon.start - nowMs <= 2 * 60e3 && !dismissed[`mt-${meetingSoon.id}`] ? { kind: "meeting", key: `mt-${meetingSoon.id}`, mood: "attention", title: meetingSoon.title, sub: meetingSoon.start > nowMs ? `Começa às ${fmtHM(meetingSoon.start)}${meetingSoon.link ? " · link pronto" : ""}` : `Começou às ${fmtHM(meetingSoon.start)}` }
          : null;
  const actionRef = useRef(null); actionRef.current = action;
  useEffect(() => { if (perm?.id) sfx("attention"); }, [perm?.id]); // eslint-disable-line
  const meetKey = action?.kind === "meeting" ? action.key : null;
  useEffect(() => { if (meetKey) sfx("meeting"); }, [meetKey]); // eslint-disable-line
  /* Roupa do tofu: automática pelo que está acontecendo, ou fixa nos Ajustes */
  const allTasksDone = todos.some((t) => t.type === "task") && todos.filter((t) => t.type === "task").every((t) => t.done);
  const outfit = settings.outfit && settings.outfit !== "auto" ? settings.outfit
    : claudeRunning ? "helmet" : playing ? "headphones" : timerRunning ? "sunglasses" : allTasksDone ? "crown" : now.getMonth() === 11 ? "santa" : "none";
  const activeToast = view === "compact" && !dragging && !action ? toasts[toasts.length - 1] : null;
  const slim = view === "compact" && !dragging && !action && !activeToast && !hover && mode === "idle" && settings.slimOverMax && fg.max;
  const nScale = view === "compact" && !dragging ? ({ small: 0.88, large: 1.14 }[settings.notchSize] || 1) : 1;
  let size, docked = true;
  if (dragging) size = [470, 112];
  else if (view === "panel") size = PANEL_SIZE[tab] || PANEL_DEFAULT;
  else if (view === "sheet") size = [440, 470];
  else if (view === "ask") { docked = false; size = [540, askOpen ? Math.min(360, Math.max(160, 46 + 44 + 24 + answerLines.length * 18 + (sources.length ? 30 : 0))) : 46]; }
  else if (view === "translate") { docked = false; size = [560, 250]; }
  else if (view === "textact") { docked = false; size = [540, ta.error ? 52 : ta.status === "idle" ? 98 : ta.status === "working" ? 120 : Math.min(250, 128 + Math.min(6, Math.ceil(ta.result.length / 70) + (ta.result.match(/\n/g) || []).length) * 18)]; }
  else if (view === "ocr") { docked = false; size = ocrPhase === "reading" ? [280, 44] : [540, 46 + 42 + Math.min(180, ocrLineCount * 21 + 20)]; }
  else if (action) size = [action.diff ? 520 : 470, action.kind === "perm" ? (action.diff ? 100 + 16 + action.diff.length * 17 + (action.more ? 14 : 0) : 100) : 84];
  else if (activeToast) size = [Math.min(420, Math.max(260, 90 + activeToast.text.length * 6.6)), 40];
  else if (slim) size = COMPACT.slim;
  else size = COMPACT[mode];
  const [w, h] = size;
  const floatR = Math.min(26, h / 2);
  const topR = docked ? 0 : floatR;
  const botR = docked ? (view === "panel" || view === "sheet" || dragging ? 24 : Math.min(24, h / 2)) : floatR;
  const islandTop = docked ? 0 : 12;
  /* Brilho por estado: âmbar = precisa de você · verde = concluído/soltar arquivo · roxo = erro */
  const tone = dragging ? "green"
    : devMood === "error" || (view === "ask" && askError && !["NO_KEY", "WEB"].includes(askError.code)) || (view === "ocr" && ocrError) || activeToast?.kind === "error" ? "purple"
      : devMood === "attention" || claude.note || action?.kind === "perm" || action?.kind === "meeting" || action?.kind === "reminder" || (view === "ask" && askError?.code === "NO_KEY") ? "amber"
        : success || devMood === "done" || action?.kind === "update" || (view === "compact" && claudeRecent) ? "green" : null;
  const accentStops = (ACCENTS[settings.accent] || ACCENTS.aurora).stops;
  const glowK = Math.max(0, Math.min(1.5, settings.glow ?? 1));
  const TONE_COLOR = { amber: "rgba(239,159,39,.55)", green: "rgba(29,158,117,.5)", purple: "rgba(127,119,221,.55)" };

  /* Regiões clicáveis para o click-through dinâmico (o main consulta o cursor do sistema) */
  useEffect(() => {
    // margem pequena no notch fechado (não engole cliques nas abas do navegador ao lado); maior com o painel aberto
    const mx = view === "compact" && !dragging ? 8 : 18;
    const rw = (w + mx * 2) * nScale;
    const rects = [{ x: (WIN_W - rw) / 2, y: 0, w: rw, h: (islandTop + h) * nScale + (slim ? 12 : view === "compact" ? 6 : 10) }];
    if (settings.devDock) rects.push({ x: 10, y: 560 - 100, w: WIN_W - 20, h: 96 });
    bridge.setHitRects(rects);
  }, [w, h, islandTop, settings.devDock, nScale, slim, view, dragging]);
  useEffect(() => {
    bridge.pin(["ask", "translate", "ocr", "sheet"].includes(view) || adding || !!qForm);
  }, [view, adding, qForm]);

  const li = lyricLines && (REAL ? lyrics.synced : true) ? lyricAt(lyricLines, pos) : 0;
  const currentLyric = lyricLines?.[li]?.text || `${trackTitle} — ${trackArtist}`;
  const lyricsView = lyricLines || [{ t: 0, text: REAL ? (lyrics.status === "loading" ? "Procurando a letra…" : media.active ? trackTitle : "Nada tocando") : "" }, { t: 1, text: REAL && media.active ? (lyrics.status === "loading" ? "" : "Sem letra sincronizada") : "" }];
  const stageIdx = done ? 4 : claudeRunning ? claude.stage : -1;
  const stageFrac = 0.55;
  const cover = (id, cls = "w-full h-full") => (!REAL ? <AlbumArt id={id} className={cls} /> : art
    ? <img src={art} alt="" draggable={false} className={`${cls} object-cover`} />
    : <div className={`${cls} grid place-items-center bg-gradient-to-br from-indigo-900 via-purple-900 to-pink-800`}><Music2 className="w-1/3 h-1/3 text-white/60" /></div>);
  const doneAt = fmtClock(new Date(now.getTime() + remaining * 1000));
  const VolIcon = muted || volume === 0 ? VolumeX : volume < 50 ? Volume1 : Volume2;
  const tasks = todos.filter((t) => t.type === "task");
  const tasksDone = tasks.filter((t) => t.done).length;
  const temp = weather?.temp ?? null;
  /* ═════════ Conteúdo compacto (notch) ═════════ */
  const renderCompact = () => {
    const [cw, ch] = COMPACT[mode];
    const box = "absolute top-0 left-0 right-0 mx-auto flex items-center";
    const center = <div className="absolute inset-0 flex items-center justify-center pointer-events-none"><Eyes mood={compactMood} /></div>;
    switch (mode) {
      case "music":
        return (
          <div className={`${box} justify-between px-[7px]`} style={{ width: cw, height: ch }}>
            <motion.div className="w-[23px] h-[23px] rounded-md overflow-hidden shrink-0" animate={{ scale: playing ? 1 : 0.88 }} transition={SPRING}>
              {cover("cmp")}
            </motion.div>
            {center}
            <div className="pr-[6px]"><Waveform playing={playing} color={artColor} /></div>
          </div>
        );
      case "usage": {
        const pct = usage?.window?.pct ?? 0;
        return (
          <div className={`${box} px-[11px] gap-2.5`} style={{ width: cw, height: ch }} title="Estimativa da janela de 5 h do Claude Code">
            <KonTofu mood="attention" size={0.62} />
            <div className="flex-1 h-[4px] rounded-full bg-white/20 overflow-hidden">
              <motion.div className={`h-full rounded-full ${pct >= 95 ? "bg-red-400" : "bg-orange-400"}`} initial={false} animate={{ width: `${pct}%` }} transition={SPRING} />
            </div>
            <span className="text-[11.5px] font-medium tabular-nums text-white/90 pr-0.5">{pct}% · {fmtLeft(usage?.window?.resetsAt - nowMs)}</span>
          </div>
        );
      }
      case "meeting": {
        const m = meetingSoon;
        const mins = m ? Math.ceil((m.start - nowMs) / 60e3) : 0;
        return (
          <div className={`${box} px-[10px] gap-2`} style={{ width: cw, height: ch }}>
            <CalendarClock size={15} className="text-amber-300 shrink-0" />
            <span className="flex-1 min-w-0 text-[12px] font-medium truncate">{m && (mins > 0 ? `Em ${mins} min · ${m.title}` : `Agora · ${m.title}`)}</span>
            {m?.link && <button onClick={() => bridge.openExternal(m.link)} className="h-[22px] px-2.5 rounded-full bg-white text-black text-[11px] font-semibold shrink-0">Entrar</button>}
          </div>
        );
      }
      case "volume":
        return (
          <div className={`${box} px-4 gap-3`} style={{ width: cw, height: ch }}>
            <VolIcon size={16} className="text-white shrink-0" strokeWidth={2.2} />
            <div className="flex-1 h-[4px] rounded-full bg-white/20 overflow-hidden">
              <motion.div className="h-full rounded-full bg-white" initial={false} animate={{ width: `${volume}%` }} transition={SPRING} />
            </div>
          </div>
        );
      case "timer":
        return (
          <div className={`${box} justify-between px-[9px]`} style={{ width: cw, height: ch }}>
            <Ring progress={timerTotal ? remaining / timerTotal : 0} />
            {center}
            <span className="pr-1.5 font-mono tabular-nums text-[13px] font-semibold text-purple-300">{fmt(remaining)}</span>
          </div>
        );
      case "claude":
        return (
          <div className={`${box} justify-between px-[11px]`} style={{ width: cw, height: ch }}>
            <div className="flex items-center gap-1.5">
              <motion.span animate={done ? { rotate: 0, scale: [1, 1.15, 1] } : { rotate: 360 }}
                transition={done ? { duration: 1.2, repeat: Infinity } : { duration: 2.4, repeat: Infinity, ease: "linear" }}
                className="text-orange-400 flex" style={{ filter: "drop-shadow(0 0 4px rgba(251,146,60,.8))" }}>
                <Asterisk size={15} strokeWidth={2.6} />
              </motion.span>
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.span key={claudeLabel} {...FADE} className="text-[12.5px] font-semibold tracking-tight text-white">{claudeLabel}</motion.span>
              </AnimatePresence>
            </div>
            {center}
            <span className="font-mono tabular-nums text-[12.5px] text-white/75">{fmt(claudeElapsed)}</span>
          </div>
        );
      default:
        if (slim) return <div className={box} style={{ width: COMPACT.slim[0], height: COMPACT.slim[1] }} />;
        return <div className={`${box} justify-center`} style={{ width: cw, height: ch }}><Eyes mood={compactMood} /></div>;
    }
  };

  /* ═════════ Páginas ═════════ */
  const card = "relative rounded-2xl border border-white/10 overflow-hidden p-3";

  const HomeTab = (
    <div className="grid grid-cols-[1fr_1.55fr_1fr] gap-2.5 h-full">
      <button onClick={() => openTab("claude")}
        className={`${card} text-left flex flex-col justify-between bg-[radial-gradient(ellipse_at_top_left,var(--tw-gradient-stops))] from-amber-950/60 to-neutral-950 hover:border-white/20 transition-colors`}>
        <span className="text-[10px] font-semibold tracking-[0.12em] text-amber-200/70">CLAUDE</span>
        <div>
          <div className="font-mono tabular-nums text-[26px] font-semibold tracking-tight leading-none">{fmt(claudeElapsed)}</div>
          <div className="text-[12px] text-white/50 mt-1">{claudeLabel}</div>
        </div>
      </button>
      <div className="relative rounded-2xl overflow-hidden border border-white/10">
        <motion.div className="absolute inset-0" animate={{ scale: playing ? 1.06 : 1 }} transition={{ duration: 1.2, ease: "easeOut" }}>
          {cover("home")}
        </motion.div>
        <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/20 to-transparent" />
        <div className="absolute inset-x-0 bottom-0 p-3 flex items-end justify-between">
          <div className="min-w-0 pr-2"><div className="text-[17px] font-semibold tracking-tight leading-tight truncate">{trackTitle}</div><div className="text-[12px] text-white/60 truncate">{trackArtist}</div></div>
          <motion.button whileTap={{ scale: 0.88 }} onClick={togglePlay} className="w-9 h-9 rounded-full bg-white text-black grid place-items-center shadow-lg">
            {playing ? <Pause size={15} fill="currentColor" /> : <Play size={15} fill="currentColor" className="ml-0.5" />}
          </motion.button>
        </div>
      </div>
      <div className={`${card} flex flex-col justify-between bg-[radial-gradient(ellipse_at_top_left,var(--tw-gradient-stops))] from-purple-950/60 to-neutral-950`}>
        <span className="text-[10px] font-semibold tracking-[0.12em] text-purple-200/70">FOCUS</span>
        <div className="flex items-end justify-between">
          <div>
            <div className="font-mono tabular-nums text-[26px] font-semibold tracking-tight leading-none">{fmt(remaining)}</div>
            <div className="text-[12px] text-white/50 mt-1">{timerRunning ? "Running" : remaining === timerTotal ? "Ready" : "Paused"}</div>
          </div>
          <motion.button whileTap={{ scale: 0.88 }} onClick={toggleTimer} className={`w-7 h-7 rounded-full grid place-items-center text-white ${glassBtn} bg-white/[0.12]`}>
            {timerRunning ? <Pause size={12} fill="currentColor" /> : <Play size={12} fill="currentColor" className="ml-0.5" />}
          </motion.button>
        </div>
      </div>
    </div>
  );

  const MusicTab = (
    <div className="relative h-full flex items-center gap-5 px-1">
      <div className="pointer-events-none absolute -left-10 -top-10 w-56 h-56 rounded-full bg-pink-500/25 blur-3xl transition-colors duration-700" style={artColor ? { background: artColor, opacity: 0.35 } : undefined} />
      <div className="pointer-events-none absolute right-0 -bottom-16 w-64 h-48 rounded-full bg-indigo-600/25 blur-3xl" />
      <motion.div className="relative w-[132px] h-[132px] rounded-2xl overflow-hidden shrink-0 shadow-[0_12px_30px_rgba(214,58,134,0.35)]" animate={{ scale: playing ? 1 : 0.9 }} transition={SPRING}>
        {cover("mus")}
      </motion.div>
      <button onClick={() => setMixOpen((v) => !v)} title="Saída de áudio e volume por app"
        className={`absolute top-0 right-0 z-10 h-6 px-2 rounded-full flex items-center gap-1 text-[10.5px] font-medium ${mixOpen ? "bg-white text-black" : "bg-white/[0.08] text-white/65 hover:bg-white/[0.14]"}`}>
        <Volume2 size={11} /> {mixOpen ? "Voltar" : "Áudio"}
      </button>
      {mixOpen ? (
        <div className="relative flex-1 min-w-0 h-full flex flex-col gap-1.5 pt-0.5 pr-0.5 overflow-hidden">
          <div className="text-[10px] font-semibold tracking-[0.1em] text-white/40">SAÍDA</div>
          <div className="flex flex-wrap gap-1 pr-16">
            {(mix?.devices || []).map((d) => (
              <button key={d.id} onClick={async () => { setMix((m) => ({ ...m, devices: m.devices.map((x) => ({ ...x, default: x.id === d.id })) })); const r = await bridge.mixSetDevice(d.id); if (r?.ok) setMix(r); }}
                className={`h-[24px] max-w-[190px] px-2.5 rounded-full text-[11px] truncate ${d.default ? "bg-white text-black font-semibold" : "bg-white/[0.07] text-white/75 hover:bg-white/[0.12]"}`}>{d.name}</button>
            ))}
            {!mix && <span className="text-[11px] text-white/40">{REAL ? "Lendo os dispositivos…" : ""}</span>}
            {mix && !mix.ok && <span className="text-[11px] text-amber-300/80">Não consegui ler o áudio do Windows</span>}
          </div>
          <div className="text-[10px] font-semibold tracking-[0.1em] text-white/40 mt-0.5">APPS</div>
          <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-1 [scrollbar-width:none]">
            {(mix?.apps || []).map((a) => (
              <div key={a.name} className="flex items-center gap-2 h-[24px]">
                <button onClick={async () => { setMix((m) => ({ ...m, apps: m.apps.map((x) => (x.name === a.name ? { ...x, muted: !x.muted } : x)) })); bridge.mixAppMute(a.pids, !a.muted); }}
                  className={`w-6 h-6 grid place-items-center rounded-full shrink-0 ${a.muted ? "text-red-300" : "text-white/60 hover:text-white"}`}>{a.muted ? <VolumeX size={13} /> : <Volume2 size={13} />}</button>
                <span className="w-[110px] text-[11.5px] truncate" title={a.title}>{a.title}</span>
                <input type="range" min="0" max="100" value={a.volume} onChange={(e) => setAppVol(a, +e.target.value)} className="kon-range flex-1" />
                <span className="w-7 text-right text-[10.5px] tabular-nums text-white/50">{a.volume}</span>
              </div>
            ))}
            {mix?.ok && !mix.apps?.length && <span className="text-[11px] text-white/40">Nenhum app tocando som agora</span>}
          </div>
        </div>
      ) : (
      <div className="relative flex-1 min-w-0 flex flex-col">
        <div className="text-[12px] text-white/60 truncate pr-16">{trackTitle} — {trackArtist}</div>
        <div className="relative h-[52px] overflow-hidden mt-1.5" style={{ maskImage: "linear-gradient(black 70%, transparent)" }}>
          <motion.div animate={{ y: -li * 26 }} transition={SPRING}>
            {(privacy ? [{ t: 0, text: "Lyrics hidden" }] : lyricsView).map((l, i) => (
              <motion.div key={i} className="h-[26px] leading-[26px] text-[17px] font-semibold tracking-tight truncate cursor-pointer"
                animate={{ color: i === li ? "rgba(255,247,222,1)" : i === li + 1 ? "rgba(255,255,255,.3)" : "rgba(255,255,255,.12)",
                  textShadow: i === li ? "0 0 14px rgba(255,214,140,.55)" : "0 0 0 rgba(0,0,0,0)" }}
                transition={{ duration: 0.5 }} onClick={() => setPos(l.t)}>{l.text || "\u00a0"}</motion.div>
            ))}
          </motion.div>
        </div>
        <div className="mt-2.5">
          <div className="h-1 rounded-full bg-white/20 cursor-pointer" onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setPos(Math.round(((e.clientX - r.left) / r.width) * duration)); }}>
            <motion.div className="h-full rounded-full bg-white" initial={false} animate={{ width: `${duration ? (pos / duration) * 100 : 0}%` }} transition={{ duration: 0.9, ease: "linear" }} />
          </div>
          <div className="flex justify-between mt-1 font-mono tabular-nums text-[10.5px] text-white/45"><span>{fmt(pos)}</span><span>-{fmt(Math.max(0, duration - pos))}</span></div>
        </div>
        <div className="flex items-center justify-center gap-6 mt-1">
          <button onClick={() => setShuffle((s) => !s)} className={shuffle ? "text-white" : "text-white/40 hover:text-white/70"}><Shuffle size={15} /></button>
          <motion.button whileTap={{ scale: 0.85 }} onClick={prevTrack} className="text-white"><SkipBack size={18} fill="currentColor" /></motion.button>
          <motion.button whileTap={{ scale: 0.88 }} onClick={togglePlay} className="w-9 h-9 rounded-full bg-white text-black grid place-items-center">
            {playing ? <Pause size={16} fill="currentColor" /> : <Play size={16} fill="currentColor" className="ml-0.5" />}
          </motion.button>
          <motion.button whileTap={{ scale: 0.85 }} onClick={nextTrack} className="text-white"><SkipForward size={18} fill="currentColor" /></motion.button>
          <button onClick={() => setRepeat((s) => !s)} className={repeat ? "text-white" : "text-white/40 hover:text-white/70"}><Repeat size={15} /></button>
        </div>
      </div>
      )}
    </div>
  );

  const FocusTab = (
    <div className="h-full flex flex-col justify-between px-2 pt-1">
      <div className="flex items-start justify-between">
        <div>
          <div className="text-4xl font-semibold tabular-nums font-mono tracking-tight leading-none">{fmt(remaining)}</div>
          <div className="text-[12px] text-white/45 mt-2">Termina às {doneAt}{settings.focusDnd !== false && REAL ? " · Não incomodar ligado" : ""}</div>
          {(() => {
            const d0 = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 6);
            const week = focusLog.filter((f) => f.at >= d0.getTime());
            const mins = week.reduce((a, f) => a + f.minutes, 0);
            const per = Array.from({ length: 7 }, (_, i) => { const a = d0.getTime() + i * 864e5; return week.filter((f) => f.at >= a && f.at < a + 864e5).reduce((x, f) => x + f.minutes, 0); });
            const mx = Math.max(25, ...per);
            return (
              <div className="flex items-end gap-2 mt-2" title="Sessões de foco concluídas nos últimos 7 dias">
                <div className="flex items-end gap-[3px] h-[18px]">{per.map((m, i) => <span key={i} className={`w-[5px] rounded-[2px] ${i === 6 ? "bg-purple-400" : "bg-white/25"}`} style={{ height: `${Math.max(12, (m / mx) * 100)}%`, opacity: m ? 1 : 0.4 }} />)}</div>
                <span className="text-[11px] text-white/50 tabular-nums">{week.length} {week.length === 1 ? "sessão" : "sessões"} na semana · {Math.floor(mins / 60)}h{String(mins % 60).padStart(2, "0")}</span>
              </div>
            );
          })()}
        </div>
        <div className="flex gap-2">
          <motion.button whileTap={{ scale: 0.92 }} onClick={addFive} className="h-8 px-3.5 rounded-full bg-white/10 hover:bg-white/15 text-white text-[12.5px]">+5 min</motion.button>
          <motion.button whileTap={{ scale: 0.92 }} onClick={stopTimer} className="h-8 px-3.5 rounded-full bg-white/10 hover:bg-white/15 text-white text-[12.5px]">Parar</motion.button>
          <motion.button whileTap={{ scale: 0.92 }} onClick={toggleTimer} className="h-8 px-4 rounded-full bg-white text-black font-medium text-[12.5px] min-w-[72px]">{timerRunning ? "Pausar" : "Começar"}</motion.button>
        </div>
      </div>
      <Ruler remaining={remaining} onPick={pickMinutes} />
    </div>
  );

  const WaterTab = (
    <div className="relative h-full flex items-center justify-between px-3">
      <div className="pointer-events-none absolute -left-12 -top-12 w-60 h-60 rounded-full bg-sky-500/20 blur-3xl" />
      <div className="relative">
        <div className="text-[10px] font-semibold tracking-[0.12em] text-sky-200/70">WATER</div>
        <div className="flex items-baseline gap-1.5 mt-1">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span key={water} {...FADE} className="text-[44px] font-semibold tabular-nums leading-none tracking-tight">{water}</motion.span>
          </AnimatePresence>
          <span className="text-white/45 text-[15px]">/ 8 glasses</span>
        </div>
        <div className="text-[12px] text-white/45 mt-1 tabular-nums">{fmtNum(water * 250)} ml of 2.000 ml</div>
        <div className="flex gap-1.5 mt-3">
          {Array.from({ length: 8 }, (_, i) => (
            <button key={i} onClick={() => setWater(water === i + 1 ? i : i + 1)}
              className="relative w-[22px] h-[30px] rounded-b-[7px] rounded-t-[3px] border border-sky-300/40 overflow-hidden">
              <motion.span className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-sky-500 to-cyan-300" initial={false}
                animate={{ height: i < water ? "100%" : "0%" }} transition={{ ...SPRING, delay: i * 0.02 }} style={{ boxShadow: "0 0 10px rgba(56,189,248,.6)" }} />
            </button>
          ))}
        </div>
      </div>
      <div className="relative flex flex-col gap-2">
        <motion.button whileTap={{ scale: 0.88 }} onClick={() => setWater((x) => Math.min(8, x + 1))} className="w-12 h-12 rounded-full bg-sky-400 text-black grid place-items-center shadow-[0_0_20px_rgba(56,189,248,.5)]"><Plus size={20} strokeWidth={2.6} /></motion.button>
        <motion.button whileTap={{ scale: 0.88 }} onClick={() => setWater((x) => Math.max(0, x - 1))} className="w-12 h-12 rounded-full bg-white/10 text-white grid place-items-center"><Minus size={20} strokeWidth={2.6} /></motion.button>
      </div>
    </div>
  );

  /* Clima */
  const wxNow = wmo(weather?.code, weather?.isDay ?? true);
  const WxIcon = WX_ICON[wxNow.icon] || Cloud;
  const hm = (iso) => (iso ? iso.slice(11, 16) : "--:--");
  const riseM = weather?.sunrise ? toMin(hm(weather.sunrise)) : 6 * 60;
  const setM = weather?.sunset ? toMin(hm(weather.sunset)) : 18 * 60;
  const sunT = (cn.mins - riseM) / Math.max(1, setM - riseM);
  const isDay = sunT >= 0 && sunT <= 1;
  const arcY = (t) => 70 - Math.sin(Math.PI * Math.max(0, Math.min(1, t))) * 54;
  const arcX = (t) => 18 + Math.max(0, Math.min(1, t)) * 284;
  const arcPath = Array.from({ length: 41 }, (_, i) => `${i ? "L" : "M"}${arcX(i / 40).toFixed(1)} ${arcY(i / 40).toFixed(1)}`).join(" ");
  const hourIdx = weather?.hourly ? Math.max(0, weather.hourly.findIndex((x) => x.t.slice(0, 13) === nowHourIso)) : 0;
  const nextHours = weather?.hourly ? weather.hourly.slice(hourIdx + 1, hourIdx + 7) : [];
  const dayLeft = isDay ? setM - cn.mins : null;

  const WeatherTab = (
    <div className="relative h-full grid grid-cols-[40fr_60fr] gap-2 px-7">
      <div className="flex flex-col justify-center min-w-0">
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div key={`${locId}-${weather?.temp}-${weather?.code}`} {...FADE}>
            <div className="flex items-center gap-2.5">
              <WxIcon size={30} strokeWidth={1.8} className={isDay ? "text-amber-300" : "text-indigo-200"} />
              <span className="text-[44px] font-semibold tracking-tight leading-none tabular-nums">{weather ? `${weather.temp}°` : "--°"}</span>
            </div>
            <div className="text-[13px] font-medium mt-1.5 truncate">{weather ? wxNow.text : "Carregando o clima…"}</div>
            <div className="text-[11.5px] text-white/45 mt-0.5 tabular-nums">{weather ? `Máx ${weather.max}° · Mín ${weather.min}°` : "\u00a0"}</div>
          </motion.div>
        </AnimatePresence>
        <div className="flex flex-wrap gap-1 mt-2.5">
          {[
            [Thermometer, weather ? `${weather.feels}°` : "—", "Sensação"],
            [Droplets, weather ? `${weather.humidity}%` : "—", "Umidade"],
            [Wind, weather ? `${weather.wind} km/h` : "—", "Vento"],
            [CloudRain, weather?.rainMax != null ? `${weather.rainMax}%` : "—", "Chuva hoje"],
          ].map(([I, v, l]) => (
            <span key={l} title={l} className="h-[22px] px-2 rounded-full bg-white/[0.07] text-[10.5px] text-white/75 flex items-center gap-1 tabular-nums"><I size={11} className="text-white/45" />{v}</span>
          ))}
        </div>
      </div>
      <div className="flex flex-col justify-center min-w-0">
        <svg viewBox="0 0 320 84" className="w-full h-[78px] overflow-visible">
          <defs>
            <linearGradient id="sunArc" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stopColor="#f59e0b" /><stop offset=".5" stopColor="#fde68a" /><stop offset="1" stopColor="#f97316" /></linearGradient>
            <linearGradient id="sunFill" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#fbbf24" stopOpacity=".2" /><stop offset="1" stopColor="#fbbf24" stopOpacity="0" /></linearGradient>
            <radialGradient id="sunGlow"><stop offset="0" stopColor="#fff" /><stop offset=".45" stopColor="#fde68a" /><stop offset="1" stopColor="#fbbf24" stopOpacity="0" /></radialGradient>
          </defs>
          <path d={`${arcPath} L302 70 L18 70 Z`} fill="url(#sunFill)" />
          <line x1="0" y1="70" x2="320" y2="70" stroke="rgba(255,255,255,.18)" strokeDasharray="3 4" />
          <path d={arcPath} fill="none" stroke="url(#sunArc)" strokeWidth="2.2" strokeLinecap="round" opacity={isDay ? 1 : 0.4} />
          <motion.circle r="13" fill={isDay ? "url(#sunGlow)" : "rgba(199,210,254,.22)"} initial={false} animate={{ cx: arcX(sunT), cy: isDay ? arcY(sunT) : 70 }} transition={SPRING} />
          <motion.circle r="5" fill={isDay ? "#fff7d6" : "#e0e7ff"} initial={false} animate={{ cx: arcX(sunT), cy: isDay ? arcY(sunT) : 70 }} transition={SPRING}
            style={{ filter: `drop-shadow(0 0 6px ${isDay ? "#fbbf24" : "#a5b4fc"})` }} />
          <text x="18" y="83" fill="rgba(255,255,255,.45)" fontSize="9.5" textAnchor="middle">{hm(weather?.sunrise)}</text>
          <text x="302" y="83" fill="rgba(255,255,255,.45)" fontSize="9.5" textAnchor="middle">{hm(weather?.sunset)}</text>
          {dayLeft != null && <text x="160" y="83" fill="rgba(255,255,255,.55)" fontSize="9.5" textAnchor="middle">{`${Math.floor(dayLeft / 60)}h${pad(dayLeft % 60)} de sol`}</text>}
        </svg>
        <div className="grid grid-cols-6 mt-1.5 text-center">
          {(nextHours.length ? nextHours : Array.from({ length: 6 }, () => null)).map((hr, i) => {
            const info = hr ? wmo(hr.code, hr.isDay) : null;
            const HI = info ? WX_ICON[info.icon] || Cloud : Cloud;
            return (
              <div key={i} className="flex flex-col items-center gap-0.5">
                <span className="text-[10px] text-white/45 tabular-nums">{hr ? `${+hr.t.slice(11, 13)}h` : "--"}</span>
                <HI size={14} strokeWidth={1.8} className={hr && hr.rain >= 50 ? "text-sky-300" : "text-white/70"} />
                <span className="text-[12px] font-semibold tabular-nums">{hr ? `${hr.temp}°` : "--"}</span>
                <span className={`text-[9.5px] tabular-nums ${hr && hr.rain >= 20 ? "text-sky-300/90" : "text-transparent"}`}>{hr ? `${hr.rain}%` : "0"}</span>
              </div>
            );
          })}
        </div>
      </div>
      <button onClick={() => setLocId(cycle(LOC_IDS, locId, -1))} className="absolute left-0 top-1/2 -translate-y-1/2 w-6 h-10 grid place-items-center text-white/40 hover:text-white"><ChevronLeft size={18} /></button>
      <button onClick={() => setLocId(cycle(LOC_IDS, locId, 1))} className="absolute right-0 top-1/2 -translate-y-1/2 w-6 h-10 grid place-items-center text-white/40 hover:text-white"><ChevronRight size={18} /></button>
    </div>
  );
  const WeatherHeader = (
    <div className="relative h-[42px] shrink-0 flex items-center px-5 pt-1">
      <div className="flex items-center gap-1.5 text-[13px] min-w-0">
        <span className="font-semibold text-white">Clima</span>
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span key={locId + locLabel} {...FADE} className="text-white/45 truncate">🇧🇷 {locLabel}</motion.span>
        </AnimatePresence>
      </div>
      <div className="absolute right-5 top-0 bottom-0 flex items-center gap-1.5 pt-1">
        {LOC_IDS.map((id) => (
          <button key={id} onClick={() => setLocId(id)} title={id === "auto" ? "Localização atual" : BR_CITIES.find((c) => c.id === id)?.city}
            className={`h-1.5 rounded-full transition-all ${id === locId ? "w-5 bg-sky-400" : "w-1.5 bg-white/25 hover:bg-white/50"}`} />
        ))}
        <span className="ml-2 text-[12px] font-medium tabular-nums text-white/85">{cn.label.slice(-5)}</span>
      </div>
    </div>
  );

  /* To-do */
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const dayKey = (d) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  const icsOn = !!settings.icsUrl;
  const taskDaySet = new Set(icsOn ? calEvents.map((e) => dayKey(new Date(e.start))) : TASK_DAYS.map((o) => dayKey(new Date(today.getTime() + o * 864e5))));
  const todayEvents = calEvents.filter((e) => dayKey(new Date(e.start)) === dayKey(today))
    .map((e) => ({ id: e.id, type: "event", cal: true, title: e.title, time: e.allDay ? "dia todo" : fmtHM(e.start), allDay: e.allDay, link: e.link, past: e.end < nowMs }));
  const remItems = reminders.filter((r) => !r.fired && r.at - nowMs < 36 * 3600e3).sort((a, b) => a.at - b.at)
    .map((r) => ({ id: `rem-${r.id}`, remId: r.id, type: "event", rem: true, title: r.text, time: new Date(r.at).toDateString() === now.toDateString() ? fmtHM(r.at) : `amanhã ${fmtHM(r.at)}`, allDay: true }));
  const listTodos = [...remItems, ...(icsOn ? [...todayEvents, ...todos.filter((t) => t.type === "task")] : todos)];
  const weekStart = new Date(today); weekStart.setDate(today.getDate() - today.getDay());
  const monthFirst = new Date(today.getFullYear(), today.getMonth(), 1);
  const daysInMonth = new Date(today.getFullYear(), today.getMonth() + 1, 0).getDate();
  const nowPct = ((now.getHours() * 60 + now.getMinutes()) / 1440) * 100;
  const addTask = () => {
    const t = newTask.trim();
    if (t) setTodos((l) => [{ id: Date.now(), type: "task", title: t, done: false }, ...l]);
    setNewTask(""); setAdding(false);
  };
  const startAdding = () => { setAdding(true); bridge.focus(); setTimeout(() => addInputRef.current?.focus({ preventScroll: true }), 80); };

  const TodoTab = (
    <div className="h-full grid grid-cols-[22fr_43fr_35fr] gap-3 px-4 pt-3 pb-1">
      <div className="flex flex-col min-w-0">
        <div className="text-[15px] font-bold tracking-tight">Todo<span className="text-[#22c55e]" style={{ textShadow: "0 0 10px rgba(34,197,94,.6)" }}>List</span></div>
        <div className="flex items-center gap-2 mt-2">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span key={`${tasksDone}/${tasks.length}`} {...FADE} className="text-[30px] font-semibold tabular-nums tracking-tight leading-none">{tasksDone}/{tasks.length}</motion.span>
          </AnimatePresence>
          <Ring progress={tasks.length ? tasksDone / tasks.length : 0} size={26} stroke={3.2} color="#22c55e" track="rgba(34,197,94,.2)" />
        </div>
        <div className="text-[12px] text-white/45 mt-1">{DAYS_T[now.getDay()]}, {now.getDate()} {MONTHS_T[now.getMonth()]}</div>
        <div className="mt-auto flex flex-col gap-1.5">
          <PillBtn onClick={startAdding} className="justify-center bg-[#22c55e]/15 text-[#4ade80] hover:bg-[#22c55e]/25"><Plus size={12} strokeWidth={2.6} /> Add Task</PillBtn>
          <PillBtn onClick={() => setEditing((e) => !e)} className={`justify-center ${editing ? "bg-white text-black hover:bg-white" : ""}`}><Pencil size={11} /> {editing ? "Done" : "Edit Task"}</PillBtn>
        </div>
      </div>
      <div className="flex flex-col min-h-0">
        <div className="text-center text-[11.5px] font-medium tabular-nums text-white/70 mb-1.5">
          {DAYS_T[now.getDay()]} {now.getDate()} {MONTHS_T[now.getMonth()]} {pad(now.getHours())}:{pad(now.getMinutes())}
        </div>
        <div className="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-1 [scrollbar-width:none]">
          <AnimatePresence initial={false}>
            {adding && (
              <motion.div key="adder" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
                className="flex items-center gap-2 bg-white/[0.08] rounded-lg px-3 py-1.5 ring-1 ring-[#22c55e]/40">
                <span className="w-[14px] h-[14px] rounded-full border-[1.5px] border-[#22c55e]/70 shrink-0" />
                <input ref={addInputRef} value={newTask} onChange={(e) => setNewTask(e.target.value)} placeholder="New task, Enter to add"
                  onKeyDown={(e) => { if (e.key === "Enter") addTask(); if (e.key === "Escape") { e.stopPropagation(); setAdding(false); } }}
                  onBlur={addTask} className="flex-1 min-w-0 bg-transparent outline-none text-[12.5px] placeholder:text-white/35" />
              </motion.div>
            )}
            {icsOn && todayEvents.length === 0 && <div className="text-[11px] text-white/35 px-1">Nenhum evento hoje na agenda</div>}
            {listTodos.map((t) => (
              <motion.div key={t.id} layout initial={{ opacity: 0, y: -6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, x: 20 }} transition={SPRING}
                onClick={() => { if (t.rem) return; if (t.cal) { if (t.link) bridge.openExternal(t.link); return; } if (t.type === "task" && !editing) setTodos((l) => l.map((x) => (x.id === t.id ? { ...x, done: !x.done } : x))); }}
                title={t.link ? "Abrir a reunião" : undefined}
                className={`flex items-center gap-2 bg-white/[0.05] rounded-lg px-3 py-1.5 ${t.past ? "opacity-45" : ""} ${(t.type === "task" && !editing) || t.link ? "cursor-pointer hover:bg-white/[0.08]" : ""}`}>
                {t.rem ? (
                  <Bell size={13} className="text-amber-300 shrink-0" />
                ) : t.type === "event" ? (
                  <span className="w-[13px] h-[13px] rounded-[3px] bg-purple-500 shrink-0 shadow-[0_0_6px_rgba(168,85,247,.7)]" />
                ) : (
                  <motion.span className="w-[14px] h-[14px] rounded-full grid place-items-center shrink-0" initial={false}
                    animate={{ backgroundColor: t.done ? "#22c55e" : "rgba(0,0,0,0)", borderColor: t.done ? "#22c55e" : "#3b82f6" }}
                    style={{ borderWidth: 1.5, borderStyle: "solid" }} transition={{ duration: 0.25 }}>
                    {t.done && <Check size={9} strokeWidth={3.5} className="text-black" />}
                  </motion.span>
                )}
                <span className={`flex-1 min-w-0 truncate text-[12.5px] transition-colors ${t.done ? "line-through text-white/40" : "text-white/90"}`}>{t.title}</span>
                {t.link && <Video size={11} className="text-purple-300/80 shrink-0" />}
                {t.type === "event" && <span className="text-[11px] tabular-nums text-white/50">{t.time}</span>}
                {editing && !t.cal && (
                  <button onClick={async (e) => { e.stopPropagation(); if (t.rem) { setReminders(await bridge.remRemove(t.remId)); return; } setTodos((l) => l.filter((x) => x.id !== t.id)); }} className="text-white/40 hover:text-red-400"><Trash2 size={12} /></button>
                )}
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      </div>
      <div className="flex flex-col min-w-0">
        <div className="flex items-center justify-between">
          <span className="text-[12.5px] font-semibold">{MONTHS_FULL[now.getMonth()]} {now.getFullYear()}</span>
          <div className="relative flex h-5 rounded-full bg-white/[0.08] p-[2px] text-[10px] font-medium">
            {["week", "month"].map((v) => (
              <button key={v} onClick={() => setCalView(v)} className={`relative px-2 rounded-full capitalize ${calView === v ? "text-black" : "text-white/55"}`}>
                {calView === v && <motion.span layoutId="calPill" className="absolute inset-0 rounded-full bg-white" transition={SPRING} />}
                <span className="relative">{v}</span>
              </button>
            ))}
          </div>
        </div>
        <AnimatePresence mode="popLayout" initial={false}>
          {calView === "week" ? (
            <motion.div key="week" {...FADE} className="flex-1 flex flex-col">
              <div className="grid grid-cols-7 mt-2.5 text-center">
                {Array.from({ length: 7 }, (_, i) => {
                  const d = new Date(weekStart); d.setDate(weekStart.getDate() + i);
                  const isToday = dayKey(d) === dayKey(today);
                  return (
                    <div key={i} className="flex flex-col items-center gap-1">
                      <span className="text-[9.5px] font-semibold text-white/35">{WEEK[i]}</span>
                      <span className={`w-[24px] h-[24px] grid place-items-center rounded-full text-[11.5px] tabular-nums ${isToday ? "bg-emerald-500 text-black font-bold shadow-[0_0_12px_rgba(16,185,129,.6)]" : "text-white/75"}`}>{d.getDate()}</span>
                      <span className={`w-1 h-1 rounded-full ${taskDaySet.has(dayKey(d)) ? (isToday ? "bg-emerald-400" : "bg-white/40") : "bg-transparent"}`} />
                    </div>
                  );
                })}
              </div>
              <div className="mt-auto mb-1">
                <div className="relative h-[8px] rounded-full bg-white/[0.06]">
                  {listTodos.filter((t) => t.type === "event" && !t.allDay).map((ev) => (
                    <span key={ev.id} className="absolute top-0 h-full rounded-full bg-purple-500/70" style={{ left: `${(toMin(ev.time) / 1440) * 100}%`, width: "4%" }} />
                  ))}
                  <span className="absolute -top-[4px] h-[16px] w-[2px] rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,.9)]" style={{ left: `${nowPct}%` }} />
                </div>
                <div className="flex justify-between mt-1 text-[9px] font-mono tabular-nums text-white/35">{["00", "06", "12", "18", "24"].map((x) => <span key={x}>{x}</span>)}</div>
              </div>
            </motion.div>
          ) : (
            <motion.div key="month" {...FADE} className="grid grid-cols-7 gap-y-[1px] mt-1.5 text-center">
              {WEEK.map((d, i) => <span key={i} className="text-[8.5px] font-semibold text-white/35">{d}</span>)}
              {Array.from({ length: monthFirst.getDay() }, (_, i) => <span key={`e${i}`} />)}
              {Array.from({ length: daysInMonth }, (_, i) => {
                const d = new Date(today.getFullYear(), today.getMonth(), i + 1);
                const isToday = i + 1 === today.getDate();
                return (
                  <span key={i} className="relative grid place-items-center h-[18px]">
                    <span className={`w-[17px] h-[17px] grid place-items-center rounded-full text-[9.5px] tabular-nums ${isToday ? "bg-emerald-500 text-black font-bold" : "text-white/70"}`}>{i + 1}</span>
                    {taskDaySet.has(dayKey(d)) && !isToday && <span className="absolute bottom-0 w-[3px] h-[3px] rounded-full bg-white/40" />}
                  </span>
                );
              })}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );

  const ClaudeLive = (
    <div className="relative h-full grid grid-cols-[32%_68%]">
      <div className="pointer-events-none absolute -left-16 -top-16 w-60 h-60 rounded-full bg-amber-500/20 blur-3xl" />
      <div className="relative flex flex-col items-center justify-between py-1">
        <Eyes mood={perm ? "attention" : done ? "love" : claudeRunning ? "reading" : "idle"} scale={2.7} />
        <div className="font-mono tabular-nums text-[24px] font-semibold tracking-tight leading-none">{fmt(claudeElapsed)}</div>
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div key={claudeLabel} {...FADE} className={`flex items-center gap-1.5 h-7 px-3 rounded-full text-[12px] font-medium ${done ? "bg-pink-500/15 text-pink-200" : claude.note ? "bg-amber-500/15 text-amber-200" : claudeRunning ? "bg-orange-500/15 text-orange-200" : "bg-white/[0.08] text-white/60"}`}>
            <motion.span className={`w-1.5 h-1.5 rounded-full ${done ? "bg-pink-400" : "bg-orange-400"}`}
              animate={!claudeRunning ? { opacity: 1 } : { opacity: [1, 0.3, 1], scale: [1, 1.4, 1] }} transition={{ duration: 1.2, repeat: claudeRunning ? Infinity : 0 }}
              style={{ boxShadow: done ? "0 0 6px #f472b6" : claudeRunning ? "0 0 6px #fb923c" : "none" }} />
            {claudeLabel}
          </motion.div>
        </AnimatePresence>
      </div>
      <div className="relative flex flex-col justify-between pl-2 pr-1">
        <div className="text-[14px] font-semibold tracking-tight text-white truncate" title={claude.note || claude.title}>{claude.note || claude.title || (hooksOn ? "Waiting for Claude Code…" : REAL ? "Connect Claude Code in Settings" : "Claude Code monitor (desktop app)")}</div>
        <div className="relative bg-black/60 border border-white/10 rounded-xl overflow-hidden">
          {privacy && <div className="absolute inset-0 z-10 flex items-center justify-center gap-1.5 text-[11px] font-medium text-white/60 backdrop-blur-md bg-black/30"><EyeOff size={13} /> Hidden by Privacy mode</div>}
          <div className="flex items-center justify-between px-3 h-6 border-b border-white/[0.06] text-[10px] font-semibold tracking-[0.1em]">
            <span className="text-white/50 truncate pr-2">{claude.file || "NO FILE YET"}</span>
            <span className="font-mono tabular-nums shrink-0"><span className="text-emerald-400">+{claude.plus}</span> <span className="text-red-400">-{claude.minus}</span></span>
          </div>
          <div className="py-1 font-mono tabular-nums text-[12px] leading-[20px]">
            <div className="flex px-3 bg-red-500/15 text-red-200/90 truncate"><span className="w-5 shrink-0 text-red-300/50">-</span><span className="truncate">{claude.removed || "\u00a0"}</span></div>
            <motion.div className="flex px-3 bg-emerald-500/15 text-emerald-200" animate={{ opacity: claude.added ? 1 : 0.35 }} transition={{ duration: 0.4 }}>
              <span className="w-5 shrink-0 text-emerald-300/50">+</span><span className="truncate">{claude.added || "\u00a0"}</span>
            </motion.div>
          </div>
        </div>
        <div className="grid grid-cols-4 gap-1.5">
          {STAGES.map((s, i) => {
            const fill = i < stageIdx ? 1 : i === stageIdx ? Math.max(0.06, stageFrac) : 0;
            const active = i === stageIdx || (done && i === 3);
            return (
              <div key={s}>
                <div className="h-[3px] rounded-full bg-white/10 overflow-hidden">
                  <motion.div className={`h-full rounded-full ${done ? "bg-pink-400" : "bg-orange-400"}`} initial={false} animate={{ width: `${fill * 100}%` }} transition={{ duration: 1, ease: "linear" }} />
                </div>
                <div className={`mt-1.5 text-[9.5px] font-semibold tracking-[0.12em] ${active ? "text-white" : i < stageIdx ? "text-white/55" : "text-white/30"}`}>{s}</div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );


  /* Uso: estimativa local a partir dos transcripts do Claude Code */
  const u = usage;
  const winPct = u?.window?.pct ?? 0;
  const ringC = 2 * Math.PI * 26;
  const maxDay = Math.max(1, ...(u?.days || []).map((d) => d.tokens));
  const MODEL_COLOR = { Opus: "#a855f7", Sonnet: "#34d399", Haiku: "#60a5fa", Outros: "#a3a3a3" };
  const ucard = "relative rounded-[14px] bg-white/[0.05] border border-white/10 p-2.5";
  const ClaudeUsage = !u ? (
    <div className="h-full grid place-items-center text-[12px] text-white/45">Lendo o histórico do Claude Code…</div>
  ) : !u.found ? (
    <div className="h-full grid place-items-center text-center px-10">
      <div><div className="text-[13px] text-white/75">Nenhum histórico do Claude Code neste computador</div>
        <div className="text-[11px] text-white/40 mt-1">O Kon lê os registros locais em ~/.claude/projects. Use o Claude Code e o uso aparece aqui.</div></div>
    </div>
  ) : (
    <div className="relative h-full grid grid-cols-[29%_minmax(0,1fr)_31%] gap-[7px]">
      <div className="pointer-events-none absolute -left-10 -top-12 w-48 h-44 rounded-full bg-amber-700/30 blur-3xl" />
      <div className={`${ucard} flex flex-col items-center justify-center gap-1`} title={`Estimativa: ${fmtTok(u.window?.used || 0)} de ~${fmtTok(u.limit)} tokens (entrada + saída) · plano ${PLAN_LABEL[u.plan] || u.plan}`}>
        <svg width="70" height="70" viewBox="0 0 64 64">
          <circle cx="32" cy="32" r="26" fill="none" stroke="rgba(255,255,255,.1)" strokeWidth="5" />
          <motion.circle cx="32" cy="32" r="26" fill="none" stroke={winPct >= 90 ? "#f87171" : "#fb923c"} strokeWidth="5" strokeLinecap="round"
            strokeDasharray={ringC} initial={false} animate={{ strokeDashoffset: ringC * (1 - winPct / 100) }} transition={SPRING} transform="rotate(-90 32 32)" />
          <text x="32" y="37" textAnchor="middle" fill="#fff" fontSize="15" fontWeight="600">{u.window ? `${winPct}%` : "—"}</text>
        </svg>
        <span className="text-[10.5px] text-white/45 text-center leading-tight">janela de 5 h<br />{u.window ? `renova em ${fmtLeft(u.window.resetsAt - nowMs)}` : "sem uso agora"}</span>
      </div>
      <div className="flex flex-col gap-[7px] min-w-0">
        <div className="grid grid-cols-2 gap-[7px]">
          {[["HOJE", u.today], ["SEMANA", u.week]].map(([l, v]) => (
            <div key={l} className={ucard}>
              <div className="text-[10px] text-white/45 tracking-[0.06em]">{l}</div>
              <div className="text-[17px] font-semibold tabular-nums tracking-tight leading-tight">{fmtTok(v.tokens)}</div>
              <div className="text-[10.5px] text-white/45 tabular-nums">≈ {fmtUsd(v.cost)}</div>
            </div>
          ))}
        </div>
        <div className={`${ucard} flex-1 py-2 flex flex-col`}>
          <div className="flex-1 flex items-end gap-[5px] min-h-[24px]">
            {u.days.map((d, i) => (
              <motion.div key={d.day} title={`${new Date(d.day).toLocaleDateString("pt-BR", { weekday: "short", day: "2-digit" })} · ${fmtTok(d.tokens)}`}
                className={`flex-1 rounded-[3px] ${i === 6 ? "bg-orange-400" : "bg-white/20"}`}
                initial={false} animate={{ height: `${Math.max(4, (d.tokens / maxDay) * 100)}%` }} transition={SPRING} />
            ))}
          </div>
          <div className="flex justify-between mt-1 text-[9.5px] text-white/40">
            {u.days.map((d, i) => <span key={d.day} className={i === 6 ? "text-white" : ""}>{i === 6 ? "hoje" : "DSTQQSS"[new Date(d.day).getDay()]}</span>)}
          </div>
        </div>
      </div>
      <div className={`${ucard} flex flex-col gap-1.5`}>
        <div className="text-[10px] text-white/45 tracking-[0.06em]">MODELOS</div>
        <div className="flex h-[5px] rounded-full overflow-hidden bg-white/10">
          {u.models.map((m) => <span key={m.name} style={{ width: `${m.pct}%`, background: MODEL_COLOR[m.name] }} />)}
        </div>
        <div className="flex flex-col gap-[3px] text-[11px]">
          {u.models.length ? u.models.slice(0, 4).map((m) => (
            <span key={m.name} className="flex items-center gap-1.5 tabular-nums"><span className="w-1.5 h-1.5 rounded-full" style={{ background: MODEL_COLOR[m.name] }} />{m.name} {m.pct}%</span>
          )) : <span className="text-white/40">sem uso na semana</span>}
        </div>
        <div className="mt-auto text-[10px] text-white/45 leading-tight">{u.sessionsToday} {u.sessionsToday === 1 ? "sessão" : "sessões"} hoje · chat do Kon {fmtTok(u.konToday)}</div>
      </div>
    </div>
  );
  const liveSessions = Object.values(sessions).filter((x) => nowMs - x.at < 6 * 3600e3).sort((a2, b2) => b2.at - a2.at);
  const ClaudeTab = (
    <div className="h-full flex flex-col">
      <div className="h-[24px] shrink-0 flex items-center justify-between gap-2 mb-1.5">
        <div className="flex items-center gap-1 min-w-0 overflow-hidden">
          {claudeView === "live" && liveSessions.slice(0, 4).map((x) => (
            <span key={x.id} title={x.title || x.cwd} className="h-[20px] px-2 rounded-full bg-white/[0.07] text-[10.5px] text-white/70 flex items-center gap-1.5 shrink-0 max-w-[130px]">
              <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${x.status === "waiting" ? "bg-amber-400" : x.status === "working" ? "bg-orange-400" : x.status === "done" ? "bg-emerald-400" : "bg-white/30"}`} />
              <span className="truncate">{baseName(x.cwd) || "sessão"}</span>
            </span>
          ))}
          {claudeView === "usage" && <span className="text-[10.5px] text-white/35 truncate">Estimativa local · plano {PLAN_LABEL[settings.claudePlan] || "Automático"} · nada sai do PC</span>}
        </div>
        <Seg id="claudeView" small value={claudeView} onChange={(v) => { setClaudeView(v); if (v === "usage") refreshUsage(); }} options={[["live", "Ao vivo"], ["usage", "Uso"]]} />
      </div>
      <div className="relative flex-1 min-h-0">
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div key={claudeView} {...FADE} className="h-full">{claudeView === "usage" ? ClaudeUsage : ClaudeLive}</motion.div>
        </AnimatePresence>
      </div>
    </div>
  );

  /* Área de transferência: histórico local (só texto) */
  const ClipTab = (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-baseline gap-2 min-w-0">
          <span className="text-[15px] font-semibold tracking-tight">Área de transferência</span>
          <span className="text-[11px] text-white/40 truncate">{settings.clipboardOn ? `${clips.length} ${clips.length === 1 ? "item" : "itens"} · senhas e chaves são ignoradas` : "pausado"}</span>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          <span className="text-[10.5px] text-white/45">Gravar</span>
          <Switch on={!!settings.clipboardOn} onChange={(v) => saveSettings({ clipboardOn: v })} color="#14b8a6" />
          {clips.some((c) => !c.pinned) && <PillBtn onClick={async () => setClips(await bridge.clipClear())}><Trash2 size={11} /> Limpar</PillBtn>}
        </div>
      </div>
      {clips.length === 0 ? (
        <div className="flex-1 rounded-2xl border border-dashed border-white/15 grid place-items-center text-center px-6">
          <div>
            <ClipboardList size={20} className="mx-auto text-white/35 mb-1.5" />
            <div className="text-[13px] text-white/70">{REAL ? "Copie qualquer texto e ele aparece aqui" : "O histórico funciona no app desktop"}</div>
            <div className="text-[11px] text-white/35 mt-0.5">Ctrl+Alt+V abre esta página · fica só neste computador</div>
          </div>
        </div>
      ) : (
        <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-1 pr-0.5 [scrollbar-width:none]">
          <AnimatePresence initial={false}>
            {clips.map((c) => (
              <motion.div key={c.id} layout initial={{ opacity: 0, y: -6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, x: 20 }} transition={SPRING}
                onClick={async () => { if (await bridge.clipCopy(c.id)) { toast("Copiado", "success", "check"); } }}
                className="group flex items-center gap-2.5 min-h-[34px] px-3 py-1.5 rounded-xl bg-white/[0.05] border border-white/[0.06] hover:bg-white/[0.09] cursor-pointer">
                {c.pinned && <Pin size={11} className="text-teal-300 shrink-0" />}
                <span className={`flex-1 min-w-0 text-[12.5px] leading-[17px] line-clamp-2 break-all ${privacy ? "blur-[5px]" : ""}`}>{c.text}</span>
                <span className="text-[10px] text-white/35 tabular-nums shrink-0">{fmtHM(c.at)}</span>
                <div className="flex items-center opacity-0 group-hover:opacity-100 transition-opacity">
                  <button onClick={async (e) => { e.stopPropagation(); setClips(await bridge.clipPin(c.id)); }} title={c.pinned ? "Desafixar" : "Fixar"} className="w-6 h-6 grid place-items-center rounded-md text-white/60 hover:text-white hover:bg-white/10">{c.pinned ? <PinOff size={12} /> : <Pin size={12} />}</button>
                  <button onClick={async (e) => { e.stopPropagation(); setClips(await bridge.clipRemove(c.id)); }} title="Apagar" className="w-6 h-6 grid place-items-center rounded-md text-white/60 hover:text-red-300 hover:bg-white/10"><Trash2 size={12} /></button>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}
    </div>
  );

  const SleepTab = (
    <div className="grid grid-cols-[1fr_1.25fr] gap-2.5 h-full">
      <div className={`${card} flex flex-col justify-between bg-[radial-gradient(ellipse_at_top_left,var(--tw-gradient-stops))] from-indigo-950/70 to-neutral-950`}>
        <div className="flex items-center justify-between">
          <motion.div className="w-9 h-9 rounded-full grid place-items-center" animate={{ background: dnd ? "rgba(129,140,248,.9)" : "rgba(255,255,255,.08)" }}>
            <Moon size={17} fill={dnd ? "currentColor" : "none"} className="text-white" />
          </motion.div>
          <Switch on={dnd} onChange={setDnd} color="#6366f1" />
        </div>
        <div>
          <div className="text-[16px] font-semibold tracking-tight">Do Not Disturb</div>
          <div className="text-[12px] text-white/50 mt-0.5 leading-snug">{dnd ? "On — volume HUD is silenced" : "Off — Live Activities can pop up"}</div>
        </div>
      </div>
      <div className={`${card} flex flex-col justify-between bg-white/[0.03]`}>
        <div className="flex items-center justify-between">
          <span className="text-[10px] font-semibold tracking-[0.12em] text-indigo-200/70">SLEEP TIMER</span>
          <span className="text-[11px] text-white/40">Pauses the music</span>
        </div>
        <div className="font-mono tabular-nums text-[30px] font-semibold tracking-tight leading-none">{sleepLeft === null ? <span className="text-white/35">Off</span> : fmt(sleepLeft)}</div>
        <div className="flex gap-1.5">
          {[null, 15, 30, 45, 60].map((m) => {
            const left = sleepLeft === null ? null : Math.ceil(sleepLeft / 60);
            const active = m === null ? left === null : left !== null && left <= m && left > m - 15;
            return (
              <motion.button key={String(m)} whileTap={{ scale: 0.9 }} onClick={() => setSleepLeft(m === null ? null : m * 60)}
                className={`flex-1 h-7 rounded-full text-[11.5px] font-medium transition-colors ${active ? "bg-white text-black" : "bg-white/10 text-white/80 hover:bg-white/15"}`}>
                {m === null ? "Off" : `${m}m`}
              </motion.button>
            );
          })}
        </div>
      </div>
    </div>
  );

  const LockTab = (
    <div className="grid grid-cols-2 gap-2.5 h-full">
      <div onClick={() => setLocked((l) => !l)} className={`${card} cursor-pointer flex flex-col justify-between bg-[radial-gradient(ellipse_at_top_left,var(--tw-gradient-stops))] ${locked ? "from-amber-900/50" : "from-white/[0.06]"} to-neutral-950`}>
        <div className="flex items-center justify-between">
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.div key={String(locked)} initial={{ rotate: -25, opacity: 0, scale: 0.7 }} animate={{ rotate: 0, opacity: 1, scale: 1 }} exit={{ rotate: 25, opacity: 0, scale: 0.7 }} transition={SPRING}
              className={`w-9 h-9 rounded-full grid place-items-center ${locked ? "bg-amber-400 text-black" : "bg-white/[0.08] text-white"}`}>
              {locked ? <Lock size={16} strokeWidth={2.4} /> : <LockOpen size={16} strokeWidth={2.4} />}
            </motion.div>
          </AnimatePresence>
          <Switch on={locked} onChange={setLocked} color="#f59e0b" />
        </div>
        <div>
          <div className="text-[16px] font-semibold tracking-tight">{locked ? "Island locked" : "Lock island"}</div>
          <div className="text-[12px] text-white/50 mt-0.5 leading-snug">{locked ? "Hover won't expand. Use Ctrl+Alt+E or the tray icon." : "Stop hover from expanding the panel"}</div>
        </div>
      </div>
      <div onClick={() => setPrivacy((v) => !v)} className={`${card} cursor-pointer flex flex-col justify-between bg-white/[0.03]`}>
        <div className="flex items-center justify-between">
          <div className={`w-9 h-9 rounded-full grid place-items-center transition-colors ${privacy ? "bg-sky-400 text-black" : "bg-white/[0.08] text-white"}`}><EyeOff size={16} strokeWidth={2.4} /></div>
          <Switch on={privacy} onChange={setPrivacy} color="#0ea5e9" />
        </div>
        <div>
          <div className="text-[16px] font-semibold tracking-tight">Privacy mode</div>
          <div className="text-[12px] text-white/50 mt-0.5 leading-snug">Hide lyrics and code from Live Activities</div>
        </div>
      </div>
    </div>
  );

  const FilesTab = (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-baseline gap-2">
          <span className="text-[15px] font-semibold tracking-tight">Arquivos</span>
          <span className="text-[11.5px] text-white/40">{stash.length ? `${stash.length} ${stash.length === 1 ? "item guardado" : "itens guardados"}` : "gaveta vazia"}</span>
        </div>
        <span className="text-[10.5px] text-white/35">Arraste para fora para usar · nada é enviado para IA</span>
      </div>
      {stash.length === 0 ? (
        <div className="flex-1 rounded-2xl border border-dashed border-white/15 grid place-items-center text-center px-6">
          <div>
            <Download size={20} className="mx-auto text-white/35 mb-1.5" />
            <div className="text-[13px] text-white/70">Arraste arquivos ou pastas até a ilha</div>
            <div className="text-[11px] text-white/35 mt-0.5">Eles ficam guardados aqui até você remover</div>
          </div>
        </div>
      ) : (
        <div className="flex-1 min-h-0 overflow-y-auto grid grid-cols-2 gap-1.5 content-start pr-0.5">
          <AnimatePresence initial={false}>
            {stash.map((it) => {
              const Ico = extIcon(it.ext);
              return (
                <motion.div key={it.id} layout initial={{ opacity: 0, scale: 0.95 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 0.9 }} transition={SPRING}
                  draggable={!it.missing} onDragStart={(e) => dragOut(e, it)} onDoubleClick={() => bridge.stashOpen(it.path)}
                  title={it.path}
                  className={`group flex items-center gap-2.5 h-[46px] pl-2 pr-1.5 rounded-xl bg-white/[0.05] border border-white/[0.06] hover:bg-white/[0.09] ${it.missing ? "opacity-45" : "cursor-grab active:cursor-grabbing"}`}>
                  <span className="w-8 h-8 rounded-lg bg-white/[0.06] grid place-items-center shrink-0 overflow-hidden">
                    {icons[it.id] ? <img src={icons[it.id]} alt="" className="w-6 h-6" draggable={false} /> : <Ico size={16} className="text-white/70" />}
                  </span>
                  <div className="flex-1 min-w-0">
                    <div className="text-[12.5px] font-medium truncate">{it.name}</div>
                    <div className="text-[10.5px] text-white/40 truncate">{it.missing ? "não encontrado" : [it.ext && it.ext !== "folder" ? it.ext.toUpperCase() : it.ext === "folder" ? "Pasta" : "", fmtSize(it.size)].filter(Boolean).join(" · ")}</div>
                  </div>
                  <div className="flex items-center opacity-60 group-hover:opacity-100 transition-opacity">
                    <button onClick={() => bridge.stashOpen(it.path)} title="Abrir" className="w-6 h-6 grid place-items-center rounded-md text-white/70 hover:text-white hover:bg-white/10"><ExternalLink size={12} /></button>
                    <button onClick={() => bridge.stashReveal(it.path)} title="Mostrar na pasta" className="w-6 h-6 grid place-items-center rounded-md text-white/70 hover:text-white hover:bg-white/10"><FolderOpen size={12} /></button>
                    <button onClick={() => removeStash(it.id)} title="Remover da ilha" className="w-6 h-6 grid place-items-center rounded-md text-white/60 hover:text-red-300 hover:bg-white/10"><Trash2 size={12} /></button>
                  </div>
                </motion.div>
              );
            })}
          </AnimatePresence>
        </div>
      )}
    </div>
  );

  const QK = { app: [AppWindow, "Programa"], folder: [FolderOpen, "Pasta"], site: [Link2, "Site"], cmd: [TerminalSquare, "Comando"] };
  const QuickTab = (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-baseline gap-2"><span className="text-[15px] font-semibold tracking-tight">Atalhos</span><span className="text-[11px] text-white/40">{quick.length ? "um clique para abrir" : "seus apps, pastas, sites e comandos"}</span></div>
        <div className="flex gap-1.5">
          {quick.length > 0 && <PillBtn onClick={() => setQEdit((v) => !v)} primary={qEdit}><Pencil size={11} /> {qEdit ? "Pronto" : "Editar"}</PillBtn>}
          {!qForm && <PillBtn onClick={() => { setQForm({ kind: "app", label: "", target: "" }); bridge.focus(); }}><Plus size={11} /> Adicionar</PillBtn>}
        </div>
      </div>
      {qForm ? (
        <div className="flex-1 flex flex-col gap-2">
          <Seg id="qkind" small value={qForm.kind} onChange={(k) => { setQForm((f) => ({ ...f, kind: k })); setTimeout(() => qInputRef.current?.focus({ preventScroll: true }), 30); }} options={Object.entries(QK).map(([k, v]) => [k, v[1]])} />
          <div className="flex gap-1.5">
            <input ref={qInputRef} autoFocus value={qForm.target} onChange={(e) => setQForm((f) => ({ ...f, target: e.target.value }))} onKeyDown={(e) => { if (e.key === "Enter") addQuick(); if (e.key === "Escape") { e.stopPropagation(); setQForm(null); } }}
              placeholder={{ app: "C:\\Program Files\\…\\app.exe", folder: "C:\\Users\\você\\Obras", site: "sinapi.caixa.gov.br", cmd: "code C:\\Projetos\\kon" }[qForm.kind]}
              className="flex-1 min-w-0 h-8 px-2.5 rounded-lg bg-white/[0.06] border border-white/[0.08] outline-none text-[12px] text-white placeholder:text-white/30 focus:border-indigo-400/60" />
            {(qForm.kind === "app" || qForm.kind === "folder") && REAL && <PillBtn onClick={async () => { const p = await bridge.quickPick(qForm.kind); if (p) setQForm((f) => ({ ...f, target: p })); }}>Escolher…</PillBtn>}
          </div>
          <input value={qForm.label} onChange={(e) => setQForm((f) => ({ ...f, label: e.target.value }))} onKeyDown={(e) => e.key === "Enter" && addQuick()} placeholder="Nome (opcional)"
            className="h-8 px-2.5 rounded-lg bg-white/[0.06] border border-white/[0.08] outline-none text-[12px] text-white placeholder:text-white/30 focus:border-indigo-400/60" />
          <div className="flex justify-between items-center mt-auto">
            <span className="text-[10.5px] text-white/35">{qForm.kind === "cmd" ? "O comando abre numa janela do Prompt para você ver a saída." : "Enter para salvar · Esc cancela"}</span>
            <div className="flex gap-1.5"><PillBtn onClick={() => setQForm(null)}>Cancelar</PillBtn><PillBtn primary onClick={addQuick}>Salvar</PillBtn></div>
          </div>
        </div>
      ) : quick.length === 0 ? (
        <div className="flex-1 rounded-2xl border border-dashed border-white/15 grid place-items-center text-center px-6">
          <div><Zap size={20} className="mx-auto text-white/35 mb-1.5" /><div className="text-[13px] text-white/70">Adicione o que você abre todo dia</div><div className="text-[11px] text-white/35 mt-0.5">Programas, pastas de obra, sites e até comandos</div></div>
        </div>
      ) : (
        <div className="flex-1 min-h-0 overflow-y-auto grid grid-cols-5 gap-1.5 content-start [scrollbar-width:none]">
          {quick.map((q) => {
            const [I] = QK[q.kind] || QK.app;
            return (
              <motion.div key={q.id} layout className="relative">
                <button onClick={async () => { if (qEdit) return; const r = await bridge.quickRun(q); if (!r.ok) toast(r.error || "Não consegui abrir", "error"); else toCompact(); }}
                  title={q.target} className={`w-full h-[72px] rounded-xl bg-white/[0.05] border border-white/[0.06] hover:bg-white/[0.1] flex flex-col items-center justify-center gap-1.5 px-1.5 ${qEdit ? "animate-pulse" : ""}`}>
                  <span className="w-8 h-8 rounded-lg bg-white/[0.06] grid place-items-center overflow-hidden">{qIcons[q.id] ? <img src={qIcons[q.id]} alt="" className="w-6 h-6" draggable={false} onError={() => setQIcons((m) => ({ ...m, [q.id]: "" }))} /> : <I size={16} className="text-white/75" />}</span>
                  <span className="text-[11px] truncate max-w-full">{q.label}</span>
                </button>
                {qEdit && <button onClick={() => setQuick((l) => l.filter((x) => x.id !== q.id))} className="absolute -top-1 -right-1 w-5 h-5 rounded-full bg-red-500 text-white grid place-items-center"><X size={11} /></button>}
              </motion.div>
            );
          })}
        </div>
      )}
    </div>
  );

  const GB = (b) => (b / 1024 ** 3).toFixed(1).replace(".", ",");
  const memPct = sys ? Math.round((sys.mem.used / sys.mem.total) * 100) : 0;
  const diskPct = sys?.disk ? Math.round(((sys.disk.total - sys.disk.free) / sys.disk.total) * 100) : 0;
  const barC = (p) => (p >= 90 ? "bg-red-400" : p >= 75 ? "bg-amber-400" : "bg-emerald-400");
  const SystemTab = !sys ? <div className="h-full grid place-items-center text-[12px] text-white/45">Lendo o sistema…</div> : (
    <div className="h-full grid grid-cols-[1fr_1fr] gap-2.5">
      <div className="grid grid-cols-2 gap-2">
        <div className={`${card} flex flex-col items-center justify-center bg-white/[0.03]`} title={sys.cpuModel}>
          <svg width="64" height="64" viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" fill="none" stroke="rgba(255,255,255,.1)" strokeWidth="5" />
            <motion.circle cx="32" cy="32" r="26" fill="none" stroke={sys.cpu >= 90 ? "#f87171" : sys.cpu >= 70 ? "#fbbf24" : "#34d399"} strokeWidth="5" strokeLinecap="round" strokeDasharray={163.4} initial={false} animate={{ strokeDashoffset: 163.4 * (1 - sys.cpu / 100) }} transition={SPRING} transform="rotate(-90 32 32)" />
            <text x="32" y="37" textAnchor="middle" fill="#fff" fontSize="14" fontWeight="600">{sys.cpu}%</text></svg>
          <span className="text-[10.5px] text-white/45 mt-0.5">CPU · {sys.cores} núcleos</span>
        </div>
        <div className="flex flex-col gap-2">
          {[[MemoryStick, "Memória", memPct, `${GB(sys.mem.used)} de ${GB(sys.mem.total)} GB`], [HardDrive, `Disco ${sys.disk?.root?.replace(/\\$/, "") || ""}`, diskPct, sys.disk ? `${GB(sys.disk.free)} GB livres` : "—"]].map(([I, l, p, sub]) => (
            <div key={l} className={`${card} flex-1 bg-white/[0.03] py-2`}>
              <div className="flex items-center justify-between text-[10.5px] text-white/50"><span className="flex items-center gap-1"><I size={11} />{l}</span><span className="tabular-nums text-white/80">{p}%</span></div>
              <div className="h-[4px] rounded-full bg-white/10 mt-1.5 overflow-hidden"><motion.div className={`h-full rounded-full ${barC(p)}`} initial={false} animate={{ width: `${p}%` }} transition={SPRING} /></div>
              <div className="text-[10.5px] text-white/40 mt-1 tabular-nums">{sub}</div>
            </div>
          ))}
        </div>
      </div>
      <div className={`${card} bg-white/[0.03] flex flex-col`}>
        <div className="flex items-center justify-between text-[10px] font-semibold tracking-[0.1em] text-white/40"><span>MAIS PESADOS AGORA</span><span className="font-normal tracking-normal tabular-nums">ligado há {Math.floor(sys.uptime / 3600)}h{String(Math.floor((sys.uptime % 3600) / 60)).padStart(2, "0")}</span></div>
        <div className="flex flex-col gap-1 mt-2">
          {(sys.top || []).slice(0, 4).map((p) => (
            <div key={p.name} className="flex items-center gap-2 text-[11.5px]">
              <span className="flex-1 truncate">{p.name}</span>
              <span className="w-14 text-right tabular-nums text-white/50">{p.mem >= 1024 ? `${(p.mem / 1024).toFixed(1).replace(".", ",")} GB` : `${p.mem} MB`}</span>
              <span className={`w-12 text-right tabular-nums ${p.cpu >= 50 ? "text-red-300" : p.cpu >= 20 ? "text-amber-300" : "text-white/80"}`}>{String(p.cpu).replace(".", ",")}%</span>
            </div>
          ))}
          {!(sys.top || []).length && <span className="text-[11px] text-white/40">{REAL ? "Calculando…" : "—"}</span>}
        </div>
        <div className="mt-auto flex items-center justify-between gap-2">
          <span className="text-[10px] text-white/35 leading-tight">Avisa no notch se a CPU ou a memória ficarem no limite por 30 s</span>
          <Switch on={settings.sysAlerts !== false} onChange={(v) => saveSettings({ sysAlerts: v })} color="#10b981" />
        </div>
      </div>
    </div>
  );

  const TABS = { home: HomeTab, files: FilesTab, clip: ClipTab, quick: QuickTab, system: SystemTab, music: MusicTab, focus: FocusTab, water: WaterTab, weather: WeatherTab, todo: TodoTab, claude: ClaudeTab, sleep: SleepTab, lock: LockTab };
  const UTIL = [
    { id: "sleep", nav: "SLEEP", Icon: Moon, on: dnd || sleepLeft !== null, color: "text-indigo-300" },
    { id: "lock", nav: "LOCK", Icon: locked ? Lock : LockOpen, on: locked || privacy, color: "text-amber-300" },
  ];
  const navButton = ({ id, nav, Icon, on, color }) => {
    const active = tab === id;
    return (
      <button key={id} onClick={() => openTab(id)} className="relative h-7 px-2 rounded-full flex items-center gap-1.5 text-[10.5px] font-semibold tracking-[0.1em]">
        {active && <motion.span layoutId="activeNavPill" className="absolute inset-0 rounded-full bg-white/15" transition={SPRING} />}
        <Icon size={13} strokeWidth={2.3} className={`relative transition-colors ${active ? "text-white" : on ? color : "text-white/40"}`} />
        <AnimatePresence initial={false}>
          {active && (
            <motion.span initial={{ opacity: 0, width: 0 }} animate={{ opacity: 1, width: "auto" }} exit={{ opacity: 0, width: 0 }} transition={SPRING}
              className="relative overflow-hidden whitespace-nowrap text-white">{nav}</motion.span>
          )}
        </AnimatePresence>
      </button>
    );
  };

  const hasOwnHeader = tab === "weather" || tab === "todo";
  const [pw, ph] = PANEL_SIZE[tab] || PANEL_DEFAULT;
  const Panel = (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col" style={{ width: pw, height: ph }}>
      {tab === "weather" ? WeatherHeader : !hasOwnHeader && (
        <div className="relative h-[42px] shrink-0 flex items-center justify-between px-5 pt-1">
          <div className="flex items-center gap-1.5 text-[11px] font-semibold tracking-[0.08em]"><span className="text-white/45">{fmtDate(now)}</span><span className="text-white">{temp ?? "–"}°</span></div>
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none pt-1"><Eyes mood={panelMood} /></div>
          <div className="flex items-center gap-2">
            {priv.mic.length > 0 && <span title="Microfone em uso" className="flex items-center gap-1 text-[10.5px] text-orange-300"><Mic size={11} />{priv.mic.slice(0, 2).join(", ")}</span>}
            {priv.cam.length > 0 && <span title="Câmera em uso" className="flex items-center gap-1 text-[10.5px] text-emerald-300"><Camera size={11} />{priv.cam.slice(0, 2).join(", ")}</span>}
            <span className="text-[12px] font-medium tabular-nums text-white/90">{fmtClock(now)}</span>
          </div>
        </div>
      )}
      <div className={`relative flex-1 min-h-0 ${hasOwnHeader ? "" : "px-4"}`}>
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div key={tab} {...FADE} className="h-full">{TABS[tab]}</motion.div>
        </AnimatePresence>
      </div>
      <div className="h-[46px] shrink-0 flex items-center justify-between px-4">
        <button onClick={() => { openTab("todo"); startAdding(); }} title="Quick add task" className={`w-7 h-7 rounded-full grid place-items-center text-white/70 ${glassBtn}`}><Plus size={14} /></button>
        <div className="flex items-center gap-0.5">
          {visiblePages.map((id) => navButton({ id, ...PAGE_META[id] }))}
          <span className="w-px h-4 bg-white/10 mx-1" />
          {UTIL.map(navButton)}
          <button onClick={() => openSheet("pages")} title="Pages e configurações" className="w-7 h-7 grid place-items-center rounded-full transition-colors text-white/40 hover:text-white/80"><Settings size={13} /></button>
        </div>
        <div className="flex items-center gap-1.5">
          <motion.span className={`w-1.5 h-1.5 rounded-full ${playing ? "bg-emerald-400" : timerRunning ? "bg-purple-400" : "bg-white/25"}`}
            animate={playing || timerRunning ? { opacity: [1, 0.4, 1] } : { opacity: 1 }} transition={{ duration: 1.4, repeat: playing || timerRunning ? Infinity : 0 }} />
          <button onClick={() => openAsk()} title="Perguntar ao Kon (Ctrl+Alt+Space)" className={`w-7 h-7 rounded-full grid place-items-center text-white/70 ${glassBtn}`}><Search size={13} /></button>
        </div>
      </div>
    </div>
  );

  /* ═════════ Ask Kon ═════════ */
  const askMood = listening ? "listening" : askPhase === "thinking" || askPhase === "streaming" ? "working"
    : askPhase === "done" ? (askError ? (askError.code === "NO_KEY" ? "attention" : "error") : "done") : "idle";
  const canAsk = (query.trim() || (askFile && !askFile.sent)) && !inline && !/=\s*$/.test(query);
  const defaultFileQ = askFile?.screen ? "O que está na minha tela? Resuma o que importa." : "Resuma este arquivo.";
  const provider = settings.provider;
  const AskView = (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col" style={{ width: 540, height: h }}>
      <div className="h-[46px] shrink-0 flex items-center gap-2 pl-4 pr-2.5">
        <motion.div initial={{ x: 200, opacity: 0 }} animate={{ x: 0, opacity: 1 }} transition={SPRING} className="w-7 flex justify-center shrink-0"><Eyes mood={askMood} /></motion.div>
        {askFile && !inline && (
          <span title={askFile.path} className={`h-6 max-w-[150px] pl-1.5 pr-1 rounded-full text-[10.5px] flex items-center gap-1 shrink-0 ${askFile.sent ? "bg-white/[0.06] text-white/45" : "bg-violet-500/20 text-violet-100"}`}>
            <Paperclip size={11} className="shrink-0" /><span className="truncate">{askFile.name}</span>
            {!askFile.sent && <button onClick={() => { setAskFile(null); askFileRef.current = null; }} className="w-4 h-4 grid place-items-center rounded-full hover:bg-white/15"><X size={10} /></button>}
          </span>
        )}
        <div className={inline ? "relative inline-grid shrink-0 ml-1" : "relative grid flex-1 min-w-0"}>
          {inline && <span className="invisible whitespace-pre col-start-1 row-start-1 text-[14.5px] tracking-tight pr-1">{query}</span>}
          <input ref={askInputRef} size={1} value={query} onChange={(e) => onQueryChange(e.target.value)}
            placeholder={listening ? "Ouvindo… fale em português" : askFile && !askFile.sent ? (askFile.screen ? "Pergunte sobre a tela" : `Pergunte sobre ${askFile.name}`) : askPhase === "idle" ? "Pergunte ao Kon" : "Pergunte de novo"}
            onKeyDown={(e) => {
              if (e.key === "Tab") { e.preventDefault(); openTranslate(query); }
              else if (e.key === "Enter") {
                if (canAsk) submitAsk(query.trim() || defaultFileQ);
                else if (inline && !/carregando|desconhecida|grandezas/.test(inline.text)) copy(inline.text.replace(/ /g, " "), "inline");
              }
            }}
            className="col-start-1 row-start-1 w-full min-w-0 bg-transparent outline-none text-[14.5px] text-white placeholder:text-white/35 tracking-tight" />
        </div>
        <AnimatePresence mode="popLayout" initial={false}>
          {inline && (
            <motion.button key={inline.kind + inline.text} {...FADE} onClick={() => copy(inline.text.replace(/ /g, " "), "inline")} title="Clique para copiar"
              className="flex items-center gap-2.5 shrink-0 group">
              <span className="w-px h-5 bg-white/20 shrink-0" />
              <span className="text-[15px] font-bold text-white tabular-nums whitespace-nowrap" style={{ textShadow: "0 0 12px rgba(255,255,255,.55)" }}>{inline.text}</span>
              <span className="w-5 h-5 grid place-items-center text-white/45 group-hover:text-white">
                {copied === "inline" ? <Check size={13} className="text-emerald-400" /> : <Copy size={12} />}
              </span>
            </motion.button>
          )}
        </AnimatePresence>
        <div className="flex-1" />
        {inline?.kind === "currency" && rates && (
          <button onClick={() => bridge.openExternal("https://www.exchangerate-api.com")} title="Rates By Exchange Rate API"
            className={`text-[10.5px] whitespace-nowrap ${ratesStale ? "text-amber-300/80" : "text-violet-300/70"} hover:text-violet-200`}>ExchangeRate-API · {ratesLabel}</button>
        )}
        {!inline && (
          <>
            <button onClick={() => switchProvider()} title="Alternar IA" className="h-6 px-2 rounded-full text-[10.5px] font-semibold bg-white/[0.08] text-white/70 hover:bg-white/15 shrink-0">
              {PROVIDER_NAME[provider]}
            </button>
            <button onClick={toggleVoice} title="Ditar (pt-BR)" className={`w-7 h-7 rounded-full grid place-items-center shrink-0 transition-colors ${listening ? "bg-red-500/90 text-white" : "text-white/45 hover:text-white hover:bg-white/10"}`}>
              {listening ? <Square size={11} fill="currentColor" /> : <Mic size={15} />}
            </button>
            <button onClick={async () => {
              const r = await bridge.captureScreen();
              if (!r.ok) { toast(r.error || "Não consegui capturar a tela", "error"); return; }
              const f = { path: r.path, name: r.name, screen: true };
              setAskFile(f); askFileRef.current = f; askInputRef.current?.focus({ preventScroll: true });
            }} title="Perguntar sobre a tela" className="w-7 h-7 rounded-full grid place-items-center text-white/45 hover:text-white hover:bg-white/10 shrink-0"><Monitor size={15} /></button>
            <button onClick={() => openTranslate(query)} title="Traduzir (Tab)" className="w-7 h-7 rounded-full grid place-items-center text-white/45 hover:text-white hover:bg-white/10 shrink-0"><Languages size={15} /></button>
          </>
        )}
        <button onClick={() => (canAsk ? submitAsk(query.trim() || defaultFileQ) : askInputRef.current?.focus({ preventScroll: true }))} className="w-7 h-7 rounded-full grid place-items-center text-white/70 hover:text-white hover:bg-white/10 shrink-0"><Search size={15} /></button>
      </div>
      {askOpen && (
        <>
          <div className="flex-1 min-h-0 overflow-y-auto px-5 pb-1">
            <div className="text-[11.5px] text-white/40 mb-1 truncate">{askQ}</div>
            {answerLines.map((l, i) => (
              <motion.div key={i} initial={{ opacity: 0, y: 4, filter: "blur(4px)" }} animate={{ opacity: 1, y: 0, filter: "blur(0px)" }} transition={{ duration: 0.3 }}
                className={`text-[12.5px] leading-[18px] select-text break-words ${askError ? "text-amber-200/90" : i === 0 ? "text-white font-medium" : "text-white/85"} ${l.trim() ? "" : "h-2"}`}>{l}</motion.div>
            ))}
            {askPhase === "streaming" && <motion.span className="inline-block w-1.5 h-3.5 bg-indigo-300/80 rounded-sm align-middle" animate={{ opacity: [1, 0, 1] }} transition={{ duration: 0.8, repeat: Infinity }} />}
            {sources.length > 0 && (
              <div className="flex flex-wrap gap-1 mt-2">
                {sources.map((s, i) => (
                  <button key={s.url} onClick={() => bridge.openExternal(s.url)} title={s.url}
                    className="h-[22px] max-w-[160px] px-2 rounded-full bg-white/[0.07] hover:bg-white/[0.14] text-[10.5px] text-white/70 flex items-center gap-1">
                    <span className="text-sky-300/80 tabular-nums">{i + 1}</span><span className="truncate">{s.title}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="h-[42px] shrink-0 flex items-center justify-between px-4 gap-2">
            <span className="text-[11px] text-white/40 truncate">
              {PROVIDER_NAME[provider]}{provider === "gemini" && settings.webSearch ? " + Google Search" : ""}
              {answerMeta?.fellBack ? ` · ${answerMeta.fellBack} indisponível, usei ${answerMeta.model}` : " · Enter para continuar"}
            </span>
            <div className="flex gap-1.5 shrink-0">
              {askError?.code === "NO_KEY" ? (
                <PillBtn onClick={() => openSheet("settings")}><KeyRound size={11} /> Configurar</PillBtn>
              ) : (
                <PillBtn onClick={() => copy(answer, "ans")}>{copied === "ans" ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />} Copiar</PillBtn>
              )}
              <PillBtn onClick={() => openAsk()}><Plus size={11} /> Novo chat</PillBtn>
            </div>
          </div>
        </>
      )}
    </div>
  );

  /* ═════════ Translate ═════════ */
  const srcLabel = tSrc === "Detect" ? (tDetected && tText.trim() ? `Detectar · ${tDetected}` : "Detectar idioma") : tSrc;
  const TranslateView = (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col px-4" style={{ width: 560, height: 250 }}>
      <div className="h-[44px] shrink-0 flex items-center justify-between">
        <div className="flex items-center gap-2.5"><Eyes mood={tStatus === "translating" ? "working" : tStatus === "done" ? "done" : tStatus === "error" ? "attention" : "idle"} /><span className="text-[14px] font-semibold tracking-tight">Translate</span></div>
        <div className="flex items-center gap-1.5">
          <span className="w-7 h-7 rounded-full grid place-items-center bg-blue-500 text-white shadow-[0_0_12px_rgba(59,130,246,.7)]"><Languages size={14} /></span>
          <button onClick={toCompact} className="w-7 h-7 rounded-full grid place-items-center text-white/50 hover:text-white hover:bg-white/10"><X size={15} /></button>
        </div>
      </div>
      <div className="h-[30px] shrink-0 grid grid-cols-[1fr_auto_1fr] items-center gap-2">
        {[[srcLabel, (d) => setTSrc(cycle(["Detect", ...LANGS], tSrc, d))], null, [tTgt, (d) => setTTgt(cycle(TARGETS, tTgt, d))]].map((cell, i) => cell === null ? (
          <motion.button key="swap" whileTap={{ rotate: 180, scale: 0.9 }} onClick={swapLangs} className="w-7 h-7 rounded-full grid place-items-center bg-white/[0.08] text-white/80 hover:bg-white/15"><ArrowLeftRight size={13} /></motion.button>
        ) : (
          <div key={i} className="h-[28px] flex items-center justify-between rounded-lg bg-white/[0.05] border border-white/[0.06] px-1">
            <button onClick={() => cell[1](-1)} className="w-5 h-5 grid place-items-center text-white/40 hover:text-white"><ChevronLeft size={14} /></button>
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.span key={cell[0]} {...FADE} className="text-[12px] font-medium text-white/90 truncate">{cell[0]}</motion.span>
            </AnimatePresence>
            <button onClick={() => cell[1](1)} className="w-5 h-5 grid place-items-center text-white/40 hover:text-white"><ChevronRight size={14} /></button>
          </div>
        ))}
      </div>
      <div className="flex-1 min-h-0 grid grid-cols-2 gap-2.5 mt-2.5">
        <div className="relative bg-white/[0.04] border border-white/[0.08] rounded-xl p-3">
          <textarea ref={tAreaRef} value={tText} maxLength={800} onChange={(e) => setTText(e.target.value)} placeholder="Digite ou cole um texto"
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); doTranslate(); } }}
            className="w-full h-full resize-none bg-transparent outline-none text-[13px] leading-[19px] text-white placeholder:text-white/30" />
          <span className="absolute right-2.5 bottom-2 text-[10px] font-mono tabular-nums text-white/35">{tText.length}/800</span>
        </div>
        <div className="relative bg-white/[0.04] border border-white/[0.08] rounded-xl p-3 overflow-y-auto">
          <AnimatePresence mode="popLayout" initial={false}>
            {tStatus === "translating" ? (
              <motion.div key="t" {...FADE} className="text-[13px] text-white/45 flex items-center gap-0.5">
                Traduzindo{[0, 1, 2].map((i) => <motion.span key={i} animate={{ opacity: [0.2, 1, 0.2] }} transition={{ duration: 0.9, repeat: Infinity, delay: i * 0.15 }}>.</motion.span>)}
              </motion.div>
            ) : tStatus === "done" || tStatus === "error" ? (
              <motion.div key={`o-${tOut}`} {...FADE} className={`text-[13px] leading-[19px] whitespace-pre-wrap break-words select-text pb-5 ${tStatus === "error" ? "text-amber-200/90" : "text-white"}`}>{tOut}</motion.div>
            ) : (
              <motion.div key="e" {...FADE} className="text-[13px] text-white/25">Tradução</motion.div>
            )}
          </AnimatePresence>
          {tStatus === "done" && tOut && (
            <button onClick={() => copy(tOut, "tr")} className="absolute right-2 bottom-2 h-6 px-2 rounded-md flex items-center gap-1 text-[11px] text-white/60 hover:text-white bg-black/40 hover:bg-white/10">
              {copied === "tr" ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />} Copiar
            </button>
          )}
        </div>
      </div>
      <div className="h-[34px] shrink-0 flex items-center justify-center">
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span key={tStatus === "done" ? "d" + tEngine : "h"} {...FADE} className="text-[11px] text-white/45">
            {tStatus === "done"
              ? (tEngine === "local" ? "Tradução local (offline) · Ctrl+Z desfaz" : `Traduzido por ${tEngine} · Ctrl+Z desfaz`)
              : "Enter traduz · Shift+Enter nova linha · Esc fecha"}
          </motion.span>
        </AnimatePresence>
      </div>
    </div>
  );

  /* ═════════ Smart OCR (Windows.Media.Ocr) ═════════ */
  const OcrView = ocrPhase === "reading" ? (
    <div className="absolute top-0 left-0 right-0 mx-auto flex items-center justify-center gap-2.5" style={{ width: 280, height: 44 }}>
      <Eyes mood="working" />
      <span className="text-[13.5px] font-medium tracking-tight text-white">Lendo texto</span>
      <span className="flex">{[0, 1, 2].map((i) => <motion.span key={i} className="text-white" animate={{ opacity: [0.2, 1, 0.2] }} transition={{ duration: 0.9, repeat: Infinity, delay: i * 0.15 }}>.</motion.span>)}</span>
    </div>
  ) : (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col" style={{ width: 540, height: h }}>
      <div className="h-[46px] shrink-0 flex items-center gap-3 pl-4 pr-2.5 border-b border-white/[0.06]">
        <div className="w-7 flex justify-center"><Eyes mood={ocrError ? "error" : "done"} /></div>
        <input ref={ocrInputRef} value={ocrQ} onChange={(e) => setOcrQ(e.target.value)} placeholder="Pergunte sobre este texto"
          onKeyDown={(e) => { if (e.key === "Enter" && ocrQ.trim() && ocrText) submitAsk(ocrQ.trim(), ocrText); }}
          className="flex-1 min-w-0 bg-transparent outline-none text-[14.5px] text-white placeholder:text-white/35 tracking-tight" />
        <button onClick={() => ocrQ.trim() && ocrText && submitAsk(ocrQ.trim(), ocrText)} className="w-7 h-7 rounded-full grid place-items-center text-white/70 hover:text-white hover:bg-white/10"><Search size={15} /></button>
      </div>
      <div className="flex-1 min-h-0 px-5 py-2.5">
        <div className={`w-full text-left whitespace-pre-wrap break-words max-h-[180px] overflow-y-auto font-mono text-[13px] leading-[21px] select-text ${ocrError ? "text-amber-200/90" : "text-white"}`}>
          {ocrError ? `Não consegui ler o texto: ${ocrError}` : ocrText || "(nenhum texto encontrado na seleção)"}
        </div>
      </div>
      <div className="h-[42px] shrink-0 flex items-center justify-between px-4">
        <span className="text-[11px] text-white/40">Smart OCR · Windows · {ocrCopied ? "Copiado para a área de transferência" : "Pronto para copiar"}</span>
        <div className="flex gap-1.5">
          <PillBtn onClick={async () => setOcrCopied(await copy(ocrText, "ocr"))}>{copied === "ocr" ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />} Copiar</PillBtn>
          <PillBtn onClick={toCompact}><X size={11} /> Fechar</PillBtn>
        </div>
      </div>
    </div>
  );

  /* ═════════ Drop zone: Guardar · Perguntar à IA · Compactar ═════════ */
  const ZONES = [
    ["stash", Download, "Guardar", "só a referência · nada é lido", "emerald"],
    ["ask", Sparkles, "Perguntar à IA", `envia o arquivo para o ${PROVIDER_NAME[settings.provider]}`, "violet"],
    ["zip", Archive, "Compactar", "cria um .zip ao lado", "sky"],
  ];
  const ZC = { emerald: ["border-emerald-300/70 bg-emerald-400/10", "text-emerald-300"], violet: ["border-violet-300/70 bg-violet-400/10", "text-violet-300"], sky: ["border-sky-300/70 bg-sky-400/10", "text-sky-300"] };
  const DropView = (
    <div className="absolute top-0 left-0 right-0 mx-auto p-2.5 pt-2 grid grid-cols-[auto_1fr_1fr_1fr] gap-2 items-stretch" style={{ width: 470, height: 112 }}>
      <div className="grid place-items-center px-1"><KonTofuEating size={1.35} /></div>
      {ZONES.map(([id, I, label, sub, c]) => {
        const on = dropZone === id;
        return (
          <div key={id} data-zone={id} onDragEnter={() => setDropZone(id)}
            className={`rounded-[16px] border-2 border-dashed flex flex-col items-center justify-center text-center px-1.5 transition-colors ${on ? ZC[c][0] : "border-white/15"}`}>
            <I size={17} className={`pointer-events-none ${on ? ZC[c][1] : "text-white/50"}`} />
            <div className="pointer-events-none text-[12px] font-semibold tracking-tight mt-1">{label}</div>
            <div className="pointer-events-none text-[9.5px] text-white/45 leading-tight mt-0.5">{sub}</div>
          </div>
        );
      })}
    </div>
  );

  /* ═════════ Ações no texto selecionado ═════════ */
  const taMood = ta.error || ta.status === "error" ? "attention" : ta.status === "working" ? "working" : ta.status === "done" ? "done" : "idle";
  const TextActView = (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col px-4 py-2.5" style={{ width: 540, height: h }}>
      <div className="flex items-center gap-2.5 h-[30px] shrink-0">
        <KonTofu mood={taMood} size={0.8} />
        <div className="flex-1 min-w-0">
          <div className="text-[12.5px] font-semibold tracking-tight">{ta.error ? ta.error : "Texto selecionado"}</div>
          {!ta.error && <div className={`text-[11px] text-white/45 truncate ${privacy ? "blur-[4px]" : ""}`}>{ta.text.replace(/\s+/g, " ")}</div>}
        </div>
        <button onClick={toCompact} className="w-6 h-6 rounded-full grid place-items-center text-white/50 hover:text-white hover:bg-white/10 shrink-0"><X size={13} /></button>
      </div>
      {!ta.error && (
        <div className="flex flex-wrap gap-1.5 mt-2 shrink-0">
          {TEXT_ACTIONS.map(([id, label]) => (
            <button key={id} onClick={() => runTextAction(id)} disabled={ta.status === "working"}
              className={`h-[26px] px-2.5 rounded-full text-[11.5px] font-medium transition-colors ${ta.action === id ? "bg-white text-black" : "bg-white/[0.08] text-white/80 hover:bg-white/[0.14]"}`}>{label}</button>
          ))}
        </div>
      )}
      {ta.status === "working" && <div className="mt-2.5 text-[12px] text-white/45 flex items-center gap-0.5">Escrevendo{[0, 1, 2].map((i) => <motion.span key={i} animate={{ opacity: [0.2, 1, 0.2] }} transition={{ duration: 0.9, repeat: Infinity, delay: i * 0.15 }}>.</motion.span>)}</div>}
      {(ta.status === "done" || ta.status === "error") && (<>
        <div className={`flex-1 min-h-0 overflow-y-auto mt-2 text-[12.5px] leading-[18px] whitespace-pre-wrap select-text ${ta.status === "error" ? "text-amber-200/90" : "text-white"}`}>{ta.result}</div>
        {ta.status === "done" && (
          <div className="flex justify-end gap-1.5 shrink-0 pt-1.5">
            <PillBtn onClick={async () => { await bridge.copy(ta.result); toast("Copiado", "success", "check"); toCompact(); }}><Copy size={11} /> Copiar</PillBtn>
            {ta.action !== "explicar" && <PillBtn primary onClick={async () => { toCompact(); const ok = await bridge.textPaste(ta.result); if (!ok) toast("Copiei o texto; cole com Ctrl+V", "info"); }}><Check size={11} /> Colar no lugar</PillBtn>}
          </div>
        )}
      </>)}
    </div>
  );

  /* ═════════ Card de ação padrão ═════════ */
  const ActionView = action && (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col justify-center pl-4 pr-3.5" style={{ width: w, height: h }}>
      <div className="flex items-center gap-3">
      <div className="w-[46px] grid place-items-center shrink-0"><KonTofu mood={action.mood} size={1.45} /></div>
      <div className="flex-1 min-w-0">
        <div className="text-[13.5px] font-semibold tracking-tight truncate">{action.title}</div>
        {action.sub && <div className={`text-[11px] text-white/55 truncate mt-0.5 ${action.kind === "perm" ? "font-mono" : ""} ${privacy && action.kind === "perm" ? "blur-[4px]" : ""}`}>{action.sub}</div>}
        {action.kind === "perm" && (
          <div className="flex items-center gap-2.5 mt-1 text-[10.5px] text-white/40 whitespace-nowrap">
            {action.project && <span className="truncate max-w-[90px]">{action.project}</span>}
            <button onClick={() => decidePerm(perm.id, "always")} className="text-sky-300/80 hover:text-sky-200">Sempre permitir</button>
            <button onClick={() => decidePerm(perm.id, null)} className="hover:text-white/70 flex items-center gap-1"><Terminal size={10} /> No terminal</button>
            {perms.length > 1 && <span className="text-amber-300/80">+{perms.length - 1}</span>}
          </div>
        )}
      </div>
      <div className="flex gap-1.5 shrink-0">
        {action.kind === "perm" ? (<>
          <PillBtn onClick={() => decidePerm(perm.id, "deny")}>Negar</PillBtn>
          <PillBtn onClick={() => decidePerm(perm.id, "allow")} primary>Permitir</PillBtn>
        </>) : action.kind === "reminder" ? (<>
          <PillBtn onClick={() => closeReminder(dueRems[0], 10)}>Adiar 10 min</PillBtn>
          <PillBtn primary onClick={() => closeReminder(dueRems[0])}><Check size={11} /> Ok</PillBtn>
        </>) : action.kind === "update" ? (<>
          <PillBtn onClick={() => setDismissed((d) => ({ ...d, [action.key]: true }))}>Depois</PillBtn>
          <PillBtn onClick={() => bridge.updateInstall()} primary><RefreshCw size={11} /> Reiniciar</PillBtn>
        </>) : (<>
          <PillBtn onClick={() => setDismissed((d) => ({ ...d, [action.key]: true }))}>Ok</PillBtn>
          {meetingSoon?.link && <PillBtn onClick={() => { bridge.openExternal(meetingSoon.link); setDismissed((d) => ({ ...d, [action.key]: true })); }} primary><Video size={11} /> Entrar</PillBtn>}
        </>)}
      </div>
      </div>
      {action.diff && (
        <div className={`mt-2 ml-[58px] rounded-lg bg-white/[0.04] border border-white/[0.06] py-1 font-mono text-[11px] leading-[17px] overflow-hidden ${privacy ? "blur-[4px]" : ""}`}>
          {action.diff.map(([k, line], idx) => (
            <div key={idx} className={`flex px-2 ${k === "-" ? "bg-red-500/[0.12] text-red-200/90" : "bg-emerald-500/[0.12] text-emerald-200/90"}`}>
              <span className="w-3.5 shrink-0 opacity-60">{k}</span><span className="truncate whitespace-pre">{line}</span>
            </div>
          ))}
          {action.more > 0 && <div className="px-2 text-[10px] text-white/35 leading-[14px]">+{action.more} {action.more === 1 ? "linha" : "linhas"}</div>}
        </div>
      )}
    </div>
  );

  /* ═════════ Notificação flutuante (no próprio notch) ═════════ */
  const ToastView = activeToast && (
    <div className="absolute top-0 left-0 right-0 mx-auto flex items-center gap-2.5 px-4" style={{ width: w, height: 40 }}>
      <KonTofu mood={activeToast.kind === "error" ? "error" : activeToast.kind === "success" ? "done" : activeToast.icon === "rain" || activeToast.icon === "bell" ? "attention" : "idle"} size={0.78} />
      <span className="text-[12.5px] font-medium tracking-tight truncate">{activeToast.text}</span>
    </div>
  );

  /* ═════════ Sheet: Pages + Settings (dentro da ilha) ═════════ */
  const KeyField = ({ provider: p, label, placeholder, link, has }) => (
    <div>
      <div className="flex items-center justify-between mb-1">
        <span className="text-[11.5px] font-semibold text-white/75 flex items-center gap-1.5"><KeyRound size={11} /> {label}</span>
        <button onClick={() => bridge.openExternal(link)} className="text-[10.5px] text-sky-300/80 hover:text-sky-200 flex items-center gap-1">Obter chave <ExternalLink size={9} /></button>
      </div>
      <div className="flex items-center gap-1.5 h-8 px-2.5 rounded-lg bg-white/[0.06] border border-white/[0.08] focus-within:border-indigo-400/60">
        <input type={showKey ? "text" : "password"} value={keyDraft[p]} onChange={(e) => setKeyDraft((d) => ({ ...d, [p]: e.target.value }))}
          placeholder={has ? "•••••••• salva — cole para trocar" : placeholder}
          onKeyDown={(e) => { if (e.key === "Enter") saveKey(p); }} onBlur={() => saveKey(p)}
          className="flex-1 min-w-0 bg-transparent outline-none text-[12px] font-mono text-white placeholder:text-white/30" />
        <button onClick={() => setShowKey((v) => !v)} className="text-white/40 hover:text-white">{showKey ? <EyeOff size={12} /> : <EyeIcon size={12} />}</button>
        {has && <Check size={12} className="text-emerald-400" />}
      </div>
    </div>
  );
  const SwitchRow = (I, label, key, sub) => (
    <div key={key} className="flex items-center gap-2.5 min-h-[36px] py-1 px-2.5 rounded-xl bg-white/[0.04]">
      <I size={13} className="text-white/50 shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="text-[11.5px] font-medium">{label}</div>
        {sub && <div className="text-[10px] text-white/35 leading-tight truncate">{sub}</div>}
      </div>
      <Switch on={settings[key] !== false} onChange={(v) => saveSettings({ [key]: v })} color="#10b981" />
    </div>
  );
  /* Boas-vindas em 3 passos: IA, Claude Code, o que mostrar */
  const finishOnboarding = () => { saveSettings({ onboarded: true }); setView("compact"); toast(settings.openMode === "click" ? "Tudo pronto · clique no notch para abrir" : "Tudo pronto · passe o mouse no notch", "success", "check"); };
  const OB_STEPS = [
    ["Escolha sua IA", "Gemini pesquisa na web; Claude é ótimo para texto e código. Dá para pular e configurar depois."],
    ["Conecte o Claude Code", "O Kon mostra o progresso, deixa você aprovar pedidos pelo notch e acompanha seu uso."],
    ["O que mostrar", "Escolha as páginas e como o notch abre. Tudo pode mudar depois em Ajustes."],
  ];
  const Onboarding = (
    <div className="flex flex-col gap-3 h-full">
      <div className="flex items-center gap-3">
        <KonTofu mood={["idle", "working", "done"][obStep]} size={1.4} />
        <div className="min-w-0">
          <div className="text-[10.5px] font-semibold tracking-[0.1em] text-white/40">PASSO {obStep + 1} DE 3</div>
          <div className="text-[15px] font-semibold tracking-tight">{OB_STEPS[obStep][0]}</div>
          <div className="text-[11.5px] text-white/50 leading-snug">{OB_STEPS[obStep][1]}</div>
        </div>
      </div>
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.div key={obStep} {...FADE} className="flex flex-col gap-2.5">
          {obStep === 0 && (<>
            <div className="flex items-center justify-between">
              <span className="text-[12px] font-medium text-white/70">IA padrão</span>
              <Seg id="obProv" value={provider} onChange={(v) => switchProvider(v)} options={[["gemini", "Gemini"], ["claude", "Claude"]]} />
            </div>
            {provider === "gemini"
              ? KeyField({ provider: "gemini", label: "GEMINI_API_KEY", placeholder: "AIza…", link: "https://aistudio.google.com/apikey", has: settings.hasGeminiKey })
              : KeyField({ provider: "anthropic", label: "ANTHROPIC_API_KEY", placeholder: "sk-ant-…", link: "https://console.anthropic.com/settings/keys", has: settings.hasAnthropicKey })}
            <div className="text-[10px] text-white/35">A chave fica criptografada pelo Windows, só neste computador.</div>
          </>)}
          {obStep === 1 && (<>
            {[[Asterisk, "Ao vivo no notch", "lendo, editando, testando, concluído"], [ShieldCheck, "Aprovar pelo notch", "Negar, Permitir ou Sempre, sem trocar de janela"], [Sparkles, "Seu uso", "janela de 5 h, tokens do dia e da semana"]].map(([I, t, d]) => (
              <div key={t} className="flex items-center gap-2.5 h-10 px-2.5 rounded-xl bg-white/[0.04]">
                <I size={14} className="text-purple-300 shrink-0" />
                <div className="min-w-0"><div className="text-[12px] font-medium">{t}</div><div className="text-[10px] text-white/40 truncate">{d}</div></div>
              </div>
            ))}
            <div className="flex items-center gap-2">
              <PillBtn onClick={toggleHooks} className={hooksOn ? "" : "bg-purple-500/25 text-purple-100 hover:bg-purple-500/35"}>{hooksOn ? <><Check size={11} className="text-emerald-400" /> Conectado</> : "Conectar o Claude Code"}</PillBtn>
              <span className="text-[10px] text-white/40 truncate">{hooksMsg || "Adiciona hooks em ~/.claude/settings.json (com backup)"}</span>
            </div>
          </>)}
          {obStep === 2 && (<>
            <div className="grid grid-cols-3 gap-1.5">
              {pageOrder.filter((id) => PAGE_META[id]).map((id) => {
                const M = PAGE_META[id];
                return (
                  <button key={id} onClick={() => togglePage(id, !pageOn[id])}
                    className={`h-8 px-2 rounded-lg flex items-center gap-1.5 text-[11px] font-medium border transition-colors ${pageOn[id] ? "bg-white/[0.1] border-white/20 text-white" : "bg-transparent border-white/[0.08] text-white/40"}`}>
                    <M.Icon size={12} className="shrink-0" /><span className="truncate">{M.label}</span>
                  </button>
                );
              })}
            </div>
            <div className="flex items-center justify-between gap-2">
              <span className="text-[12px] font-medium text-white/70">Abrir o painel</span>
              <Seg id="obOpen" small value={settings.openMode} onChange={(v) => saveSettings({ openMode: v })} options={[["hover", "Passar o mouse"], ["delay", "Com atraso"], ["click", "Clique"]]} />
            </div>
          </>)}
        </motion.div>
      </AnimatePresence>
      <div className="mt-auto flex items-center justify-between">
        <div className="flex gap-1.5">{[0, 1, 2].map((i) => <span key={i} className={`h-1.5 rounded-full transition-all ${i === obStep ? "w-5 bg-white" : "w-1.5 bg-white/25"}`} />)}</div>
        <div className="flex gap-1.5">
          {obStep > 0 ? <PillBtn onClick={() => setObStep((x) => x - 1)}>Voltar</PillBtn> : <PillBtn onClick={finishOnboarding}>Pular</PillBtn>}
          <PillBtn onClick={() => (obStep < 2 ? setObStep((x) => x + 1) : finishOnboarding())} primary>{obStep < 2 ? "Continuar" : "Concluir"}</PillBtn>
        </div>
      </div>
    </div>
  );
  const ReportPane = (
    <div className="flex flex-col gap-2.5 h-full">
      {rep.status !== "done" ? (<>
        <textarea autoFocus value={rep.note} onChange={(e) => setRep((r) => ({ ...r, note: e.target.value }))} maxLength={2000}
          placeholder="O que aconteceu? Ex.: cliquei em Música e o painel fechou sozinho"
          className="h-[120px] resize-none rounded-xl bg-white/[0.06] border border-white/[0.08] focus:border-indigo-400/60 outline-none p-3 text-[12.5px] leading-[18px] text-white placeholder:text-white/30" />
        <div className="flex items-center gap-2.5 h-9 px-2.5 rounded-xl bg-white/[0.04]">
          <Camera size={13} className="text-white/50" />
          <span className="flex-1 text-[11.5px] font-medium">Incluir um print da tela</span>
          <Switch on={rep.shot} onChange={(v) => setRep((r) => ({ ...r, shot: v }))} color="#10b981" />
        </div>
        <div className="text-[10.5px] text-white/40 leading-snug">O relatório leva a versão, os registros de erro e de atualização e seus ajustes, sem chaves de API, sem o link da agenda e sem o conteúdo dos seus arquivos ou da área de transferência.</div>
        <div className="mt-auto flex justify-end gap-1.5">
          <PillBtn onClick={() => setSheetTab("settings")}>Cancelar</PillBtn>
          <PillBtn primary onClick={async () => { setRep((r) => ({ ...r, status: "working" })); const r = await bridge.report({ note: rep.note, withShot: rep.shot }); setRep((x) => ({ ...x, status: r.ok ? "done" : "idle", result: r })); if (!r.ok) toast(r.error || "Não consegui gerar o relatório", "error"); }}>
            {rep.status === "working" ? "Gerando…" : "Gerar relatório"}
          </PillBtn>
        </div>
      </>) : (
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-3"><KonTofu mood="done" size={1.3} /><div><div className="text-[14px] font-semibold">Relatório pronto</div><div className="text-[11.5px] text-white/50">{rep.result?.zip ? "Salvei o arquivo .zip na sua Área de Trabalho." : "Abra a página para enviar."}</div></div></div>
          <div className="text-[11.5px] text-white/60 leading-snug">Abra a página no GitHub, arraste o arquivo para lá e clique em <b>Submit new issue</b>. Também pode me mandar o arquivo direto aqui no chat.</div>
          <div className="flex gap-1.5">
            <PillBtn primary onClick={() => bridge.openExternal(rep.result.url)}><ExternalLink size={11} /> Abrir no GitHub</PillBtn>
            {rep.result?.zip && <PillBtn onClick={() => bridge.reportReveal(rep.result.zip)}><FolderOpen size={11} /> Mostrar arquivo</PillBtn>}
          </div>
        </div>
      )}
    </div>
  );
  const SettingsPane = (
    <div className="flex flex-col gap-2.5">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-medium text-white/70">IA padrão</span>
        <div className="flex h-7 rounded-full bg-white/[0.08] p-[2px] text-[11px] font-medium">
          {["gemini", "claude"].map((p) => (
            <button key={p} onClick={() => switchProvider(p)} className={`relative px-3 rounded-full ${provider === p ? "text-black" : "text-white/60"}`}>
              {provider === p && <motion.span layoutId="provPill" className="absolute inset-0 rounded-full bg-white" transition={SPRING} />}
              <span className="relative">{PROVIDER_NAME[p]}</span>
            </button>
          ))}
        </div>
      </div>
      {KeyField({ provider: "gemini", label: "GEMINI_API_KEY", placeholder: "AIza…", link: "https://aistudio.google.com/apikey", has: settings.hasGeminiKey })}
      {KeyField({ provider: "anthropic", label: "ANTHROPIC_API_KEY", placeholder: "sk-ant-…", link: "https://console.anthropic.com/settings/keys", has: settings.hasAnthropicKey })}
      <div className="text-[10px] text-white/35 -mt-1">{settings.secure === false && REAL ? "Criptografia do sistema indisponível — chaves salvas localmente." : "Chaves criptografadas pelo Windows (safeStorage), só neste computador."}</div>
      <div className="grid grid-cols-2 gap-2">
        {[["geminiModel", "Modelo Gemini", ["gemini-2.5-flash", "gemini-flash-latest", "gemini-3.5-flash", "gemini-3.6-flash"]], ["claudeModel", "Modelo Claude", ["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"]]].map(([k, label, opts]) => (
          <label key={k} className="flex flex-col gap-1">
            <span className="text-[10.5px] font-medium text-white/55">{label}</span>
            <input list={`dl-${k}`} defaultValue={settings[k]} key={`${k}-${settings[k]}`} onBlur={(e) => e.target.value.trim() && saveSettings({ [k]: e.target.value.trim() })}
              onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              className="h-8 px-2.5 rounded-lg bg-white/[0.06] border border-white/[0.08] outline-none text-[11.5px] font-mono text-white" />
            <datalist id={`dl-${k}`}>{opts.map((m) => <option key={m} value={m} />)}</datalist>
          </label>
        ))}
      </div>
      <div className="flex items-center gap-3 h-9 px-2.5 rounded-xl bg-white/[0.04]">
        <Globe size={14} className="text-sky-300/80" />
        <span className="flex-1 text-[12px] font-medium">Busca Google no Gemini (com fontes)</span>
        <Switch on={!!settings.webSearch} onChange={(v) => saveSettings({ webSearch: v })} color="#10b981" />
      </div>
      <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
        <MapPin size={14} className="text-emerald-300/80" />
        <span className="text-[12px] font-medium">Local</span>
        <select value={locId} onChange={(e) => setLocId(e.target.value)} className="flex-1 min-w-0 h-7 rounded-md bg-white/[0.06] border border-white/[0.08] text-[11.5px] text-white outline-none px-1.5">
          <option value="auto" className="bg-neutral-900">📍 Atual ({ipLoc?.country === "BR" ? ipLoc.city : "detectando / São Paulo"})</option>
          {BR_CITIES.map((c) => <option key={c.id} value={c.id} className="bg-neutral-900">🇧🇷 {c.city}</option>)}
        </select>
      </div>
      {REAL && (
        <div className="flex items-center gap-3 min-h-[40px] py-1 px-2.5 rounded-xl bg-white/[0.04]">
          <span className="w-6 h-6 rounded-[7px] grid place-items-center bg-purple-500 shrink-0"><Asterisk size={13} className="text-white" strokeWidth={2.4} /></span>
          <div className="flex-1 min-w-0">
            <div className="text-[12px] font-medium">Claude Code</div>
            <div className={`text-[10px] leading-tight ${hooksMsg && !hooksOn && hooksMsg !== "Desconectado." ? "text-amber-300/80" : "text-white/40"}`}>{hooksMsg || (hooksOn ? "Conectado · ao vivo na página Claude" : "Adiciona hooks em ~/.claude/settings.json")}</div>
          </div>
          <PillBtn onClick={toggleHooks} className={hooksOn ? "" : "bg-purple-500/25 text-purple-100 hover:bg-purple-500/35"}>{hooksOn ? "Desconectar" : "Conectar"}</PillBtn>
        </div>
      )}
      {REAL && hooksOn && (
        <div className="flex flex-col gap-1.5 -mt-1 pl-3 border-l border-white/10 ml-2">
          {SwitchRow(ShieldCheck, "Aprovar pedidos do Claude pelo notch", "approvals", "Negar / Permitir / Sempre, sem ir ao terminal")}
          <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
            <span className="flex-1 text-[11.5px] font-medium">Plano (para a janela de 5 h)</span>
            <Seg id="plan" small value={settings.claudePlan || "auto"} onChange={(v) => saveSettings({ claudePlan: v })} options={[["auto", "Auto"], ["pro", "Pro"], ["max5", "Max 5×"], ["max20", "Max 20×"]]} />
          </div>
        </div>
      )}
      <div className="text-[10.5px] font-semibold tracking-[0.1em] text-white/40 mt-1">NOTCH</div>
      <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
        <MousePointerClick size={14} className="text-white/50" />
        <span className="flex-1 text-[11.5px] font-medium">Abrir o painel</span>
        <Seg id="open" small value={settings.openMode} onChange={(v) => saveSettings({ openMode: v })} options={[["hover", "Passar o mouse"], ["delay", "Com atraso"], ["click", "Clique"]]} />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
          <span className="flex-1 text-[11.5px] font-medium">Tamanho</span>
          <Seg id="size" small value={settings.notchSize} onChange={(v) => saveSettings({ notchSize: v })} options={[["small", "P"], ["normal", "M"], ["large", "G"]]} />
        </div>
        <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
          <Monitor size={13} className="text-white/50" />
          <span className="flex-1 text-[11.5px] font-medium">Tela</span>
          <Seg id="disp" small value={settings.display} onChange={(v) => saveSettings({ display: v })} options={[["primary", "Principal"], ["cursor", "Do mouse"]]} />
        </div>
      </div>
      {SwitchRow(EyeOff, "Esconder em tela cheia", "hideFullscreen", "jogos, vídeos e apresentações")}
      {SwitchRow(Minus, "Linha fina sobre janelas maximizadas", "slimOverMax", "passe o mouse para ver o notch")}
      {SwitchRow(ShieldCheck, "Fora de prints e compartilhamento de tela", "hideInCapture", "Teams, Meet, Discord e gravações não mostram o Kon")}
      <div className="flex items-center gap-2 min-h-[36px] py-1 px-2.5 rounded-xl bg-white/[0.04]">
        <span className="text-[11.5px] font-medium shrink-0">Cor</span>
        <div className="flex gap-1.5 flex-1 justify-end">
          {Object.entries(ACCENTS).map(([k, a]) => (
            <button key={k} onClick={() => saveSettings({ accent: k })} title={a.label}
              className={`w-[22px] h-[22px] rounded-full ring-2 transition ${(settings.accent || "aurora") === k ? "ring-white" : "ring-transparent hover:ring-white/30"}`}
              style={{ background: `linear-gradient(135deg,${a.stops[0]},${a.stops[3]})` }} />
          ))}
        </div>
      </div>
      <div className="flex items-center gap-2.5 h-9 px-2.5 rounded-xl bg-white/[0.04]">
        <span className="text-[11.5px] font-medium shrink-0">Brilho</span>
        <input type="range" min="0" max="150" step="5" value={Math.round((settings.glow ?? 1) * 100)} onChange={(e) => setSettings((st) => ({ ...st, glow: +e.target.value / 100 }))}
          onMouseUp={(e) => saveSettings({ glow: +e.currentTarget.value / 100 })} className="kon-range flex-1" />
        <span className="w-9 text-right text-[10.5px] tabular-nums text-white/50">{Math.round((settings.glow ?? 1) * 100)}%</span>
      </div>
      {SwitchRow(Moon, "Não incomodar do Windows durante o Focus", "focusDnd", "silencia notificações enquanto o timer roda")}
      {REAL && (
        <div className="flex items-center gap-2.5 min-h-[36px] py-1 px-2.5 rounded-xl bg-white/[0.04]">
          <Mic size={13} className="text-white/50 shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-[11.5px] font-medium">Chamar pela voz: diga "Kon"</div>
            <div className={`text-[10px] leading-tight truncate ${wake.error ? "text-amber-300/80" : "text-white/35"}`}>{wake.error || (wake.on ? `Ouvindo só a palavra "Kon" · offline${wake.lang ? ` (${wake.lang})` : ""}` : "Reconhecimento de voz do Windows, offline; nada é gravado")}</div>
          </div>
          <Switch on={settings.wakeWord === true} onChange={(v) => saveSettings({ wakeWord: v })} color="#10b981" />
        </div>
      )}
      <div className="text-[10.5px] font-semibold tracking-[0.1em] text-white/40 mt-1">MASCOTE E SONS</div>
      <div className="grid grid-cols-5 gap-1.5">
        {[{ id: "auto", label: "Automática", hint: "muda com o que está acontecendo" }, { id: "none", label: "Sem roupa", hint: "" }, ...OUTFITS].map((o) => {
          const on = (settings.outfit || "auto") === o.id;
          return (
            <button key={o.id} onClick={() => saveSettings({ outfit: o.id })} title={o.hint || o.label}
              className={`h-[62px] rounded-xl flex flex-col items-center justify-end gap-1 pb-1.5 border transition-colors ${on ? "bg-white/[0.1] border-white/40" : "bg-white/[0.03] border-white/[0.06] hover:bg-white/[0.07]"}`}>
              <div className="h-[30px] grid place-items-end"><KonTofu mood="idle" size={1.05} outfit={o.id === "auto" ? outfit : o.id} /></div>
              <span className="text-[9.5px] text-white/70 leading-none truncate max-w-full px-1">{o.label}</span>
            </button>
          );
        })}
      </div>
      <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
        <span className="flex-1 text-[11.5px] font-medium">Sons</span>
        <Seg id="pack" small value={settings.soundPack || "retro"} onChange={(v) => { saveSettings({ soundPack: v }); playSound("done", v, settings.soundVol ?? 0.6); }} options={Object.entries(SOUND_PACKS)} />
        <Switch on={settings.soundsOn !== false} onChange={(v) => saveSettings({ soundsOn: v })} color="#10b981" />
      </div>
      {settings.soundsOn !== false && (
        <div className="flex items-center gap-2.5 h-9 px-2.5 rounded-xl bg-white/[0.04]">
          <span className="text-[11.5px] font-medium shrink-0">Volume</span>
          <input type="range" min="5" max="100" step="5" value={Math.round((settings.soundVol ?? 0.6) * 100)} onChange={(e) => setSettings((st) => ({ ...st, soundVol: +e.target.value / 100 }))}
            onMouseUp={(e) => { const v = +e.currentTarget.value / 100; saveSettings({ soundVol: v }); playSound("attention", settings.soundPack || "retro", v); }} className="kon-range flex-1" />
          <span className="w-9 text-right text-[10.5px] tabular-nums text-white/50">{Math.round((settings.soundVol ?? 0.6) * 100)}%</span>
        </div>
      )}
      <div className="text-[10.5px] font-semibold tracking-[0.1em] text-white/40 mt-1">AGENDA</div>
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-1.5 h-8 px-2.5 rounded-lg bg-white/[0.06] border border-white/[0.08] focus-within:border-indigo-400/60">
          <CalendarClock size={13} className="text-white/45 shrink-0" />
          <input defaultValue={settings.icsUrl} key={`ics-${settings.loaded}`} placeholder="Link .ics da sua agenda (Google, Outlook, iCloud)"
            onBlur={(e) => { const v = e.target.value.trim(); if (v !== settings.icsUrl) saveSettings({ icsUrl: v }); }}
            onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
            className="flex-1 min-w-0 bg-transparent outline-none text-[11.5px] text-white placeholder:text-white/30" />
          {settings.icsUrl && <button onClick={() => refreshCal(true)} title="Atualizar agora" className="text-white/40 hover:text-white"><RefreshCw size={12} /></button>}
        </div>
        <div className={`text-[10px] leading-tight ${calStatus.startsWith("Erro") ? "text-amber-300/80" : "text-white/35"}`}>
          {calStatus || "No Google Agenda: Configurações › sua agenda › Endereço secreto em formato iCal. O Kon avisa 2 min antes e mostra o botão Entrar."}
        </div>
      </div>
      <div className="text-[10.5px] font-semibold tracking-[0.1em] text-white/40 mt-1">GERAL</div>
      <div className="grid grid-cols-2 gap-2">
        {REAL && (
          <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
            <span className="flex-1 text-[11.5px] font-medium">Iniciar com o Windows</span>
            <Switch on={!!settings.autoStart} onChange={(v) => saveSettings({ autoStart: v })} color="#10b981" />
          </div>
        )}
        <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
          <span className="flex-1 text-[11.5px] font-medium">Dock de testes</span>
          <Switch on={!!settings.devDock} onChange={(v) => saveSettings({ devDock: v })} color="#10b981" />
        </div>
      </div>
      {REAL && (
        <div className="flex items-center gap-2 h-9 px-2.5 rounded-xl bg-white/[0.04]">
          <RefreshCw size={13} className="text-white/50" />
          <div className="flex-1 min-w-0">
            <div className="text-[11.5px] font-medium">Kon {appVersion} · atualizar sozinho</div>
            <div className={`text-[10px] leading-tight truncate ${updStatus.state === "error" ? "text-amber-300/80" : updStatus.state === "ready" ? "text-emerald-300" : "text-white/40"}`}>
              {{ checking: "Procurando…", downloading: `Baixando o Kon ${updStatus.version || ""} · ${updStatus.percent || 0}%${updStatus.small ? " · atualização pequena" : ""}`, ready: `Kon ${updStatus.version} pronto para instalar`, latest: "Você tem a versão mais nova", error: `Erro: ${updStatus.message || ""}` }[updStatus.state] || "Procura sozinho ao abrir e a cada 4 h"}
            </div>
          </div>
          {update ? (
            <PillBtn primary onClick={() => bridge.updateInstall()}><RefreshCw size={11} /> Reiniciar</PillBtn>
          ) : (
            <PillBtn onClick={async () => {
              setUpdStatus({ state: "checking" });
              const r = await bridge.updateCheck();
              if (!r.ok) { setUpdStatus({ state: "error", message: r.error }); return; }
              if (!r.newer) setUpdStatus({ state: "latest" });
            }}>Procurar</PillBtn>
          )}
          <Switch on={settings.autoUpdate !== false} onChange={(v) => saveSettings({ autoUpdate: v })} color="#10b981" />
        </div>
      )}
      <div className="flex items-center gap-4">
        <button onClick={() => { setRep({ note: "", shot: true, status: "idle", result: null }); setSheetTab("report"); }} className="text-left text-[10.5px] text-amber-300/80 hover:text-amber-200 flex items-center gap-1"><AlertTriangle size={10} /> Relatar problema</button>
        <button onClick={() => { setObStep(0); setSheetTab("welcome"); }} className="text-left text-[10.5px] text-sky-300/70 hover:text-sky-200">Rever as boas-vindas</button>
      </div>
      <div className="flex items-center justify-between text-[10px] text-white/35 pt-0.5">
        <span>Ctrl+Alt+Space Perguntar · Ctrl+Alt+T OCR · Ctrl+Alt+E Painel · Ctrl+Alt+V Clips</span>
        {REAL && <button onClick={() => bridge.quit()} className="flex items-center gap-1 hover:text-red-300"><Power size={10} /> Sair</button>}
      </div>
    </div>
  );
  const SheetView = (
    <div className="absolute top-0 left-0 right-0 mx-auto flex flex-col p-4 pt-3" style={{ width: 440, height: 470 }}>
      <div className="flex items-start justify-between shrink-0">
        <div>
          <div className="text-[17px] font-bold tracking-tight">{sheetTab === "welcome" ? "Bem-vindo ao Kon" : sheetTab === "report" ? "Relatar problema" : sheetTab === "pages" ? "Pages" : "Configurações"}</div>
          <div className="text-[12px] text-white/50 mt-0.5">{sheetTab === "report" ? "Junte as informações para eu conseguir corrigir." : sheetTab === "welcome" ? "Três passos e pronto." : sheetTab === "pages" ? "Arraste para reordenar. Desligue o que não usa." : "IA, notch, Claude Code, agenda e atualizações."}</div>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          <div className={`flex h-6 rounded-full bg-white/[0.08] p-[2px] text-[10.5px] font-medium ${sheetTab === "welcome" || sheetTab === "report" ? "hidden" : ""}`}>
            {[["pages", "Pages"], ["settings", "Ajustes"]].map(([t, l]) => (
              <button key={t} onClick={() => setSheetTab(t)} className={`relative px-2.5 rounded-full ${sheetTab === t ? "text-black" : "text-white/55"}`}>
                {sheetTab === t && <motion.span layoutId="sheetPill" className="absolute inset-0 rounded-full bg-white" transition={SPRING} />}
                <span className="relative">{l}</span>
              </button>
            ))}
          </div>
          <button onClick={() => (sheetTab === "welcome" ? finishOnboarding() : setView("panel"))} className="w-6 h-6 rounded-full grid place-items-center text-white/50 hover:text-white hover:bg-white/10"><X size={13} /></button>
        </div>
      </div>
      <div className="relative flex-1 min-h-0 mt-3 overflow-y-auto pr-0.5">
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div key={sheetTab} {...FADE} className={sheetTab === "welcome" || sheetTab === "report" ? "h-full" : ""}>
            {sheetTab === "welcome" ? Onboarding : sheetTab === "report" ? ReportPane : sheetTab === "pages" ? (
              <Reorder.Group axis="y" values={pageOrder} onReorder={setPageOrder} className="flex flex-col gap-1">
                {pageOrder.filter((id) => PAGE_META[id]).map((id) => <PageRow key={id} id={id} on={!!pageOn[id]} onToggle={(v) => togglePage(id, v)} />)}
              </Reorder.Group>
            ) : SettingsPane}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );

  const contentKey = dragging ? "drop" : view === "compact" ? (action ? `act-${action.key}` : activeToast ? `toast-${activeToast.id}` : slim ? "slim" : `c-${mode}`) : view === "ocr" ? `ocr-${ocrPhase}` : view;
  const content = dragging ? DropView
    : view === "panel" ? Panel : view === "sheet" ? SheetView : view === "ask" ? AskView : view === "translate" ? TranslateView : view === "ocr" ? OcrView : view === "textact" ? TextActView
      : action ? ActionView : activeToast ? ToastView : renderCompact();

  /* Dock de testes (opcional, em Configurações) */
  const devButtons = [
    ["Idle", () => { setDevMood(null); setDragging(false); toCompact(); }],
    ["Arrastar", () => setDragging((d) => !d)],
    ["Trabalhando", () => setDevMood((m) => (m === "working" ? null : "working"))],
    ["Atenção", () => setDevMood((m) => (m === "attention" ? null : "attention"))],
    ["Concluído", () => { setDevMood(null); flashSuccess(); }],
    ["Erro", () => setDevMood((m) => (m === "error" ? null : "error"))],
    ["Toast", () => toast("Guardado na ilha: orçamento.xlsx", "success", "file")],
    ["Ask", () => openAsk()],
    ["Câmbio", () => openAsk("100usd em brl=")],
    ["OCR", () => { if (REAL) startOcr(); else { setOcrText("TODAY:\nIced coffee latte 18K\nMatcha milk 24K"); setOcrError(""); setOcrPhase("result"); setView("ocr"); } }],
    ["Pages", () => openSheet("pages")],
    ["Permissão", () => setPerms((l) => [...l, l.length % 2
      ? { id: uid(), tool: "Bash", input: { command: "npm run build && npm test" }, cwd: "C:/Projetos/kon-notch", at: Date.now() }
      : { id: uid(), tool: "Edit", input: { file_path: "C:/Projetos/kon-notch/src/orcamento.ts", old_string: "const bdi = 0.25;\nconst total = custo * (1 + bdi);", new_string: "const bdi = calcularBdi(obra);\nconst total = arredondar(custo * (1 + bdi), 2);\nlog(total);" }, cwd: "C:/Projetos/kon-notch", at: Date.now() }])],
    ["Som", () => { const evs = ["done", "attention", "reminder", "meeting", "file", "error"]; const i = (devSoundIdx.current = (devSoundIdx.current + 1) % evs.length); playSound(evs[i], settings.soundPack || "retro", settings.soundVol ?? 0.6); toast(`Som: ${evs[i]}`); }],
    ["Texto", () => { setTa({ text: "oi pessoal, segue o orçamento atualizado da obra, qualquer coisa me avisa", status: "idle", action: "", result: "", error: "" }); setView("textact"); }],
    ["Reunião", () => { const t = Date.now() + 90e3; setCalEvents([{ id: "demo|" + t, title: "Daily com o time", start: t, end: t + 30 * 60e3, allDay: false, link: "https://meet.google.com/abc-defg-hij" }]); }],
    ["Uso 84%", () => setUsage((u) => u && ({ ...u, window: { ...(u.window || {}), pct: u.window?.pct === 84 ? 73 : 84, resetsAt: Date.now() + 38 * 60e3 } }))],
    ["Maximizado", () => setFg((f) => ({ ...f, max: !f.max }))],
    ["Mic", () => setPriv((p) => (p.mic.length ? { mic: [], cam: [] } : { mic: ["Discord"], cam: ["Teams"] }))],
    ["Boas-vindas", () => { setObStep(0); openSheet("welcome"); }],
  ];

  return (
    <OutfitContext.Provider value={outfit}>
    <div className="fixed inset-0 overflow-clip text-white font-sans antialiased select-none">
      {/* ───── Notch / Dynamic Island ───── */}
      <motion.div className="absolute inset-x-0 top-0 z-50 flex justify-center pointer-events-none" initial={false} animate={{ y: islandTop }} transition={SPRING}>
        <motion.div ref={islandBoxRef} className="relative pointer-events-auto" initial={false} animate={{ scale: nScale }} transition={SPRING} style={{ originY: 0 }}>
          {/* Orelhas côncavas reais (SVG) */}
          {[0, 1].map((side) => (
            <motion.svg key={side} width="16" height="16" viewBox="0 0 16 16" className="absolute top-0 z-20 pointer-events-none"
              style={{ [side ? "right" : "left"]: -16, transformOrigin: side ? "left top" : "right top" }}
              initial={false} animate={{ opacity: docked && !slim ? 1 : 0, scale: docked && !slim ? 1 : 0.3 }} transition={SPRING}>
              <path d={side ? "M16 0 H0 V16 A16 16 0 0 1 16 0 Z" : "M0 0 H16 V16 A16 16 0 0 0 0 0 Z"} fill="#000000" />
            </motion.svg>
          ))}
          <motion.div
            onMouseEnter={onEnter} onMouseLeave={onLeave} onClick={onIslandClick}
            initial={false}
            animate={{
              width: w, height: h,
              borderTopLeftRadius: topR, borderTopRightRadius: topR,
              borderBottomLeftRadius: botR, borderBottomRightRadius: botR,
              scale: hover && view === "compact" ? 1.05 : 1,
            }}
            transition={SPRING}
            style={{ originY: 0, background: "#000000" }}
            className={`relative z-10 overflow-clip shadow-[0_22px_55px_rgba(0,0,0,0.55)] ${docked ? "border-x border-b border-t-0 border-white/[0.08]" : "border border-white/[0.12]"}`}
          >
            {/* Brilho do estado (âmbar / verde / roxo) vindo da base */}
            <motion.div className="pointer-events-none absolute inset-0 z-0" initial={false}
              animate={{ opacity: tone ? Math.min(1, 0.4 + 0.6 * glowK) : 0 }} transition={{ duration: 0.5 }}
              style={{ background: `radial-gradient(120% 90% at 30% 115%, ${TONE_COLOR[tone] || "transparent"} 0%, transparent 62%)` }} />
            {/* Aurora: linha neon ultrafina na base */}
            <motion.div className="pointer-events-none absolute inset-x-0 bottom-0 z-0" initial={false}
              animate={processing ? { opacity: [0.6 * glowK, glowK, 0.6 * glowK] } : { opacity: 0.55 * glowK }}
              transition={processing ? { duration: 1.2, repeat: Infinity, ease: "easeInOut" } : { duration: 0.5 }}>
              <motion.div className="absolute left-[10%] right-[10%] bottom-0 rounded-full blur-xl" animate={{ height: processing ? 18 : 8, opacity: processing ? 0.6 : 0.3 }} transition={SPRING}
                style={{ background: `linear-gradient(90deg,${accentStops.join(",")})` }} />
              <motion.div className="absolute bottom-0 h-[1.5px]" animate={{ left: processing ? "6%" : "22%", right: processing ? "6%" : "22%" }} transition={SPRING}
                style={{ background: `linear-gradient(90deg,transparent,${accentStops.slice(0, 3).join(",")},transparent)` }} />
            </motion.div>
            <div className="relative z-10 w-full h-full">
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.div key={contentKey} {...FADE} className="absolute inset-0">{content}</motion.div>
              </AnimatePresence>
            </div>
            {/* Microfone / câmera em uso (como no iPhone) */}
            {view === "compact" && !slim && !dragging && (priv.mic.length > 0 || priv.cam.length > 0) && (
              <div className="absolute top-[4px] right-[9px] z-20 flex gap-[3px] pointer-events-none">
                {priv.mic.length > 0 && <span className="w-[5px] h-[5px] rounded-full bg-orange-400 shadow-[0_0_5px_#fb923c]" />}
                {priv.cam.length > 0 && <span className="w-[5px] h-[5px] rounded-full bg-emerald-400 shadow-[0_0_5px_#34d399]" />}
              </div>
            )}
          </motion.div>
        </motion.div>
      </motion.div>

      {/* ───── Dock de testes ───── */}
      <AnimatePresence>
        {settings.devDock && (
          <motion.div data-keep key="dev" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 10 }} transition={SPRING}
            className="absolute bottom-2 inset-x-0 mx-auto w-fit max-w-[800px] flex flex-wrap justify-center items-center gap-0.5 p-1 rounded-2xl bg-black/70 backdrop-blur-xl border border-white/10 shadow-2xl">
            {devButtons.map(([label, fn]) => (
              <button key={label} onClick={fn} className={`h-7 px-2.5 rounded-xl text-[11.5px] font-medium transition-colors ${({ working: "Trabalhando", attention: "Atenção", error: "Erro" })[devMood] === label || (label === "Arrastar" && dragging) ? "bg-white text-black" : "text-white/75 hover:text-white hover:bg-white/10"}`}>{label}</button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
    </OutfitContext.Provider>
  );
}
