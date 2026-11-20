// Sons do Kon, gerados na hora (sem arquivos): três pacotes, seis eventos. Mesmo desenho do esboço aprovado.
let ac = null;
const ctx = () => {
  if (!ac) { const A = window.AudioContext || window.webkitAudioContext; if (!A) return null; ac = new A(); }
  if (ac.state === "suspended") ac.resume().catch(() => {});
  return ac;
};

export const SOUND_PACKS = { suave: "Suave", cristal: "Cristal", retro: "Retrô" };
export const SOUND_EVENTS = {
  done: "Concluído",
  attention: "Precisa de você",
  reminder: "Lembrete",
  meeting: "Reunião",
  file: "Arquivo guardado",
  error: "Erro",
};
const PACK = { suave: { type: "sine", v: 0.22, d: 1 }, cristal: { type: "triangle", v: 0.18, d: 1.3, sh: true }, retro: { type: "square", v: 0.06, d: 0.45 } };
const NOTES = {
  done: [[523, 0], [784, 0.12]],
  attention: [[659, 0], [659, 0.18]],
  reminder: [[392, 0], [523, 0.11], [659, 0.22]],
  meeting: [[440, 0], [880, 0, "bell"]],
  file: [[900, 0, "pop"]],
  error: [[294, 0], [220, 0.14]],
};

function tone(a, out, f, t0, dur, type, vol, f2) {
  const o = a.createOscillator(), g = a.createGain();
  const t = a.currentTime + t0;
  o.type = type;
  o.frequency.setValueAtTime(f, t);
  if (f2) o.frequency.exponentialRampToValueAtTime(f2, t + dur);
  g.gain.setValueAtTime(0, t);
  g.gain.linearRampToValueAtTime(vol, t + 0.01);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  o.connect(g).connect(out);
  o.start(t);
  o.stop(t + dur + 0.05);
}

const lastAt = {};
/** Toca um evento. volume 0–1 (multiplica o volume do pacote). */
export function playSound(event, pack = "retro", volume = 0.6) {
  const notes = NOTES[event];
  const a = notes && ctx();
  if (!a || volume <= 0) return;
  const now = Date.now();
  if (now - (lastAt[event] || 0) < 350) return; // evita som repetido em rajada
  lastAt[event] = now;
  const p = PACK[pack] || PACK.retro;
  const out = a.createGain();
  out.gain.value = Math.min(1, volume * 1.4);
  out.connect(a.destination);
  for (const [f, t, k] of notes) {
    if (k === "pop") { tone(a, out, f, t, 0.12, p.type, p.v, 180); continue; }
    const d = (k === "bell" ? 1.4 : 0.35) * p.d;
    tone(a, out, f, t, d, p.type, k === "bell" ? p.v * 0.5 : p.v);
    if (p.sh) tone(a, out, f * 2, t, d * 0.6, "sine", p.v * 0.25);
  }
}
