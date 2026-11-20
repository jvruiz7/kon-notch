import React from "react";
import { Asterisk, Music2, Timer, GlassWater, CloudSun, ListTodo, LayoutGrid, FolderOpen, ClipboardList } from "lucide-react";

/* Física & constantes */
export const SPRING = { type: "spring", stiffness: 380, damping: 30, mass: 0.8 };
export const FADE = {
  initial: { opacity: 0, scale: 0.94, filter: "blur(6px)" },
  animate: { opacity: 1, scale: 1, filter: "blur(0px)" },
  exit: { opacity: 0, scale: 0.94, filter: "blur(6px)" },
  transition: { duration: 0.32, ease: [0.32, 0.72, 0, 1] },
};
export const AURORA = "linear-gradient(90deg,#06b6d4,#2563eb,#4f46e5,#9333ea)";

export const COMPACT = { idle: [168, 32], music: [250, 36], volume: [300, 36], timer: [280, 36], claude: [300, 36], usage: [300, 36], meeting: [320, 36], slim: [130, 5] };
export const PANEL_SIZE = { weather: [600, 255], todo: [660, 255], claude: [620, 285], clip: [600, 265] };
export const PANEL_DEFAULT = [580, 255];

export const PAGE_META = {
  home: { label: "Home", nav: "HOME", Icon: LayoutGrid, sq: "bg-orange-500" },
  files: { label: "Arquivos", nav: "ARQUIVOS", Icon: FolderOpen, sq: "bg-amber-400" },
  clip: { label: "Área de transferência", nav: "CLIPS", Icon: ClipboardList, sq: "bg-teal-500" },
  music: { label: "Music", nav: "MUSIC", Icon: Music2, sq: "bg-pink-500" },
  focus: { label: "Focus", nav: "FOCUS", Icon: Timer, sq: "bg-[#ff6b5b]" },
  weather: { label: "Clima", nav: "CLIMA", Icon: CloudSun, sq: "bg-sky-500" },
  todo: { label: "To-do", nav: "TO-DO", Icon: ListTodo, sq: "bg-lime-500" },
  claude: { label: "Claude Code", nav: "CLAUDE", Icon: Asterisk, sq: "bg-purple-500" },
  water: { label: "Water", nav: "WATER", Icon: GlassWater, sq: "bg-sky-400" },
};
export const DEFAULT_ORDER = ["home", "files", "clip", "music", "weather", "focus", "todo", "claude", "water"];

export const DURATION = 195;
export const CLAUDE_STEP = 3;
export const STAGES = ["READ", "EDIT", "TEST", "DONE"];
export const BASE_LYRICS = [
  "Engine humming, midnight calling", "Neon signs in the rearview", "Windows down, the city's glowing",
  "Radio static, hearts on fire", "Chasing echoes down the highway", "Every streetlight turning gold",
  "We don't need to know where we're going", "Just keep driving, never slowing", "Pixel stars across the skyline",
  "Hold the moment, hold it tight", "Synth waves crashing on the shoreline", "Neon drive into the night",
];
export const LYRICS = Array.from({ length: 25 }, (_, i) => ({ t: i * 8, text: BASE_LYRICS[i % 12] }));
export const lyricIndex = (pos) => Math.min(LYRICS.length - 1, Math.floor(pos / 8));

/* ═════════════════════════ Formatação ═════════════════════════ */
export const fmt = (s) => { s = Math.max(0, Math.floor(s)); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`; };
export const pad = (n) => String(n).padStart(2, "0");
export const DAYS = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
export const DAYS_T = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
export const MONTHS = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
export const MONTHS_T = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const MONTHS_FULL = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
export const WEEK = ["S", "M", "T", "W", "T", "F", "S"];
export const fmtDate = (d) => `${DAYS[d.getDay()]} ${pad(d.getDate())} ${MONTHS[d.getMonth()]}`;
export const fmtClock = (d) => d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
export const fmtNum = (v) => new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 4 }).format(v);

/* ═════════════════════════ Calculadora offline ═════════════════════════ */
export const MATH_RE = /^[\d\s.,+\-*×x÷/%()]+=\s*$/i;
export function parseNum(s) {
  if (s.includes(",")) return parseFloat(s.replace(/\./g, "").replace(",", "."));
  if (/^\d{1,3}(\.\d{3})+$/.test(s)) return parseFloat(s.replace(/\./g, ""));
  return parseFloat(s);
}
export function calc(input) {
  const src = input.replace(/=\s*$/, "").replace(/[×x]/gi, "*").replace(/÷/g, "/");
  const toks = [];
  for (let i = 0; i < src.length;) {
    const c = src[i];
    if (/\s/.test(c)) { i++; continue; }
    if (/\d/.test(c)) {
      let j = i;
      while (j < src.length && /[\d.,]/.test(src[j])) j++;
      toks.push({ t: "n", v: parseNum(src.slice(i, j)) });
      i = j; continue;
    }
    if ("+-*/%()".includes(c)) { toks.push({ t: c }); i++; continue; }
    return null;
  }
  let p = 0;
  const post = (f) => (toks[p]?.t === "%" ? (p++, { v: f.v / 100, pct: true }) : f);
  const factor = () => {
    const tk = toks[p];
    if (!tk) throw 0;
    if (tk.t === "-") { p++; const f = factor(); return { v: -f.v, pct: f.pct }; }
    if (tk.t === "(") { p++; const v = expr(); if (toks[p]?.t !== ")") throw 0; p++; return post({ v, pct: false }); }
    if (tk.t === "n") { p++; return post({ v: tk.v, pct: false }); }
    throw 0;
  };
  const term = () => {
    let l = factor();
    while (toks[p] && (toks[p].t === "*" || toks[p].t === "/")) {
      const op = toks[p++].t; const r = factor();
      l = { v: op === "*" ? l.v * r.v : l.v / r.v, pct: false };
    }
    return l;
  };
  const expr = () => {
    let l = term().v;
    while (toks[p] && (toks[p].t === "+" || toks[p].t === "-")) {
      const op = toks[p++].t; const r = term();
      const rv = r.pct ? l * r.v : r.v; // 200 + 10% = 220
      l = op === "+" ? l + rv : l - rv;
    }
    return l;
  };
  try { const v = expr(); return p === toks.length && isFinite(v) ? v : null; } catch { return null; }
}

export const CUR_RE = /^\s*([\d.,]+)\s*([a-z]{3})\s+(?:to|em|para|in|->)\s+([a-z]{3})\s*=\s*$/i;
const CUR_LOCALE = { USD: "en-US", BRL: "pt-BR", EUR: "en-IE", GBP: "en-GB", SAR: "en-US", INR: "en-IN", CNY: "zh-CN", KRW: "ko-KR", MXN: "es-MX", ARS: "es-AR", CAD: "en-CA", AUD: "en-AU", CHF: "de-CH" };
export function fmtCur(v, c) {
  if (c === "IDR") return "Rp" + new Intl.NumberFormat("id-ID", { maximumFractionDigits: 0 }).format(v);
  if (c === "JPY") return "¥" + new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(v);
  try { return new Intl.NumberFormat(CUR_LOCALE[c] || "en-US", { style: "currency", currency: c }).format(v); }
  catch { return `${new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(v)} ${c}`; }
}

/* Localizações no Brasil (horários de oração via AlAdhan method=3 e clima via Open-Meteo) */
/* Códigos WMO do Open-Meteo → descrição em português + tipo de ícone */
export function wmo(code, isDay = true) {
  const c = Number(code);
  if (c === 0) return { text: isDay ? "Céu limpo" : "Noite limpa", icon: isDay ? "sun" : "moon" };
  if (c === 1 || c === 2) return { text: c === 1 ? "Poucas nuvens" : "Parcialmente nublado", icon: isDay ? "sun-cloud" : "moon-cloud" };
  if (c === 3) return { text: "Nublado", icon: "cloud" };
  if (c === 45 || c === 48) return { text: "Neblina", icon: "fog" };
  if (c >= 51 && c <= 57) return { text: "Garoa", icon: "drizzle" };
  if ((c >= 61 && c <= 67) || (c >= 80 && c <= 82)) return { text: c >= 80 ? "Pancadas de chuva" : c >= 65 ? "Chuva forte" : "Chuva", icon: "rain" };
  if ((c >= 71 && c <= 77) || c === 85 || c === 86) return { text: "Neve", icon: "snow" };
  if (c >= 95) return { text: "Tempestade", icon: "storm" };
  return { text: "—", icon: "cloud" };
}
export const BR_CITIES = [
  { id: "porto-velho", city: "Porto Velho", lat: -8.7612, lon: -63.9004, tz: "America/Porto_Velho" },
  { id: "sao-paulo", city: "São Paulo", lat: -23.5505, lon: -46.6333, tz: "America/Sao_Paulo" },
  { id: "brasilia", city: "Brasília", lat: -15.7939, lon: -47.8828, tz: "America/Sao_Paulo" },
  { id: "rio", city: "Rio de Janeiro", lat: -22.9068, lon: -43.1729, tz: "America/Sao_Paulo" },
  { id: "manaus", city: "Manaus", lat: -3.119, lon: -60.0217, tz: "America/Manaus" },
  { id: "curitiba", city: "Curitiba", lat: -25.4284, lon: -49.2733, tz: "America/Sao_Paulo" },
];
export const toMin = (t) => { const [h, m] = t.split(":").map(Number); return h * 60 + m; };
export function cityNow(tz, date) {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-US", { timeZone: tz, weekday: "short", day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit", hourCycle: "h23" })
      .formatToParts(date).map((p) => [p.type, p.value]),
  );
  const h = +parts.hour % 24, m = +parts.minute;
  return { label: `${parts.weekday} ${parts.day} ${parts.month} ${pad(h)}:${pad(m)}`, mins: h * 60 + m, dateKey: `${parts.day}-${parts.month}` };
}

export const LANGS = ["Portuguese", "English", "Indonesian", "Japanese", "Spanish", "French", "German", "Chinese", "Korean", "Arabic"];
export const TARGETS = ["English", "Portuguese", "Spanish", "French", "German", "Italian", "Japanese", "Chinese", "Korean", "Indonesian", "Arabic"];

export const INITIAL_TODOS = [
  { id: 1, type: "event", title: "Design review", time: "15:00" },
  { id: 2, type: "task", title: "Plan the sprint", done: false },
  { id: 3, type: "task", title: "Weekly update", done: true },
  { id: 4, type: "task", title: "Reply to Sherty", done: true },
  { id: 5, type: "event", title: "Team sync", time: "18:00" },
  { id: 6, type: "task", title: "Morning workout", done: true },
  { id: 7, type: "task", title: "Prepare the demo", done: false },
];

/* Persistência local simples */
export function usePersist(key, initial) {
  const [v, setV] = React.useState(() => {
    try { const raw = localStorage.getItem("nusa." + key); return raw ? JSON.parse(raw) : initial; } catch { return initial; }
  });
  React.useEffect(() => { try { localStorage.setItem("nusa." + key, JSON.stringify(v)); } catch {} }, [key, v]);
  return [v, setV];
}
export const glassBtn = "backdrop-blur-xl bg-white/[0.06] hover:bg-white/[0.12] transition-colors";

/* Tradutor local (fallback sem chave): frases comuns + glossário */
export const PHRASES = [
  { English: "Good morning! See you at the meeting at 3 PM.", Portuguese: "Bom dia! Nos vemos na reunião às 15h.", Spanish: "¡Buenos días! Nos vemos en la reunión a las 3 p. m.", French: "Bonjour ! À tout à l'heure à la réunion de 15 h.", Indonesian: "Selamat pagi! Sampai jumpa di rapat jam 3 sore.", Japanese: "おはようございます！午後3時の会議でお会いしましょう。" },
  { English: "Good morning!", Portuguese: "Bom dia!", Spanish: "¡Buenos días!", French: "Bonjour !", Indonesian: "Selamat pagi!", Japanese: "おはようございます！" },
  { English: "Good night!", Portuguese: "Boa noite!", Spanish: "¡Buenas noches!", French: "Bonne nuit !", Indonesian: "Selamat malam!", Japanese: "おやすみなさい！" },
  { English: "Thank you very much!", Portuguese: "Muito obrigado!", Spanish: "¡Muchas gracias!", French: "Merci beaucoup !", Indonesian: "Terima kasih banyak!", Japanese: "どうもありがとうございます！" },
  { English: "How are you?", Portuguese: "Como você está?", Spanish: "¿Cómo estás?", French: "Comment ça va ?", Indonesian: "Apa kabar?", Japanese: "お元気ですか？" },
  { English: "Where is the train station?", Portuguese: "Onde fica a estação de trem?", Spanish: "¿Dónde está la estación de tren?", French: "Où est la gare ?", Indonesian: "Di mana stasiun kereta?", Japanese: "駅はどこですか？" },
  { English: "I'll send the report tomorrow.", Portuguese: "Vou enviar o relatório amanhã.", Spanish: "Enviaré el informe mañana.", French: "J'enverrai le rapport demain.", Indonesian: "Saya akan mengirim laporannya besok.", Japanese: "明日レポートを送ります。" },
  { English: "Can we reschedule the meeting?", Portuguese: "Podemos remarcar a reunião?", Spanish: "¿Podemos reprogramar la reunión?", French: "Pouvons-nous reporter la réunion ?", Indonesian: "Bisakah kita menjadwal ulang rapat?", Japanese: "会議の日程を変更できますか？" },
];
const LANG_WORDS = {
  Portuguese: ["bom", "dia", "voce", "reuniao", "obrigado", "obrigada", "ola", "nao", "ate", "vemos", "onde", "fica", "estacao", "amanha", "vou", "muito", "esta", "boa", "noite", "relatorio", "podemos", "que", "uma", "para", "com"],
  Spanish: ["buenos", "dias", "gracias", "reunion", "hola", "donde", "estacion", "manana", "muchas", "estas", "enviare", "buenas", "noches", "informe", "que", "una", "para", "con", "el"],
  English: ["the", "good", "morning", "see", "you", "at", "meeting", "thank", "how", "are", "where", "is", "i", "will", "tomorrow", "hello", "night", "send", "can", "we", "and"],
  French: ["bonjour", "merci", "beaucoup", "comment", "ou", "est", "la", "le", "les", "et", "nous", "reunion", "demain"],
  Indonesian: ["selamat", "pagi", "sampai", "jumpa", "rapat", "terima", "kasih", "apa", "kabar", "saya", "akan", "besok", "mana"],
};
export const normText = (s) => s.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/[^\p{L}\p{N}\s]/gu, "").replace(/\s+/g, " ").trim();
export function detectLangLocal(t) {
  if (!t.trim()) return null;
  if (/[\u3040-\u30ff\u4e00-\u9fff]/.test(t)) return "Japanese";
  const words = normText(t).split(" ");
  let best = "English", score = 0;
  for (const [lang, list] of Object.entries(LANG_WORDS)) {
    const sc = words.filter((w) => list.includes(w)).length;
    if (sc > score) { score = sc; best = lang; }
  }
  return best;
}
export function translateLocal(text, src, tgt) {
  if (src === tgt) return text;
  const sentences = text.match(/[^.!?。！？\n]+[.!?。！？]*/g) || [text];
  const whole = normText(text);
  for (const ph of PHRASES) for (const l of Object.keys(ph)) if (normText(ph[l]) === whole && ph[tgt]) return ph[tgt];
  let matched = 0;
  const out = sentences.map((s) => {
    const n = normText(s);
    for (const ph of PHRASES) for (const l of Object.keys(ph)) if (normText(ph[l]) === n && ph[tgt]) { matched++; return ph[tgt]; }
    return s.trim();
  });
  return matched ? out.join(" ") : null;
}
