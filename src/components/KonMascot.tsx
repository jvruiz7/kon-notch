import React, { createContext, useContext } from "react";
import { motion, AnimatePresence } from "framer-motion";

/**
 * KonTofu — o mascote do Kon. Um "tofu" branco de cantos bem arredondados, com dois olhinhos,
 * que vive DENTRO da ilha. Muda de expressão, ganha um selo colorido e um leve tom conforme o estado.
 */
export type TofuMood =
  | "idle" | "working" | "attention" | "done" | "error" | "eating"
  | "music" | "calm" | "sleep" | "listening";

const SPRING = { type: "spring", stiffness: 380, damping: 26, mass: 0.8 } as const;

const TINT: Record<TofuMood, string> = {
  idle: "#ffffff", working: "#ffffff", attention: "#FFF1DC", done: "#E3F7EE", error: "#ffffff",
  eating: "#ffffff", music: "#FFEAF3", calm: "#F1EEFF", sleep: "#EEF0F6", listening: "#FFECEC",
};
const BADGE: Partial<Record<TofuMood, { bg: string; fg: string; glyph: string }>> = {
  working: { bg: "#378ADD", fg: "#fff", glyph: "dots" },
  attention: { bg: "#EF9F27", fg: "#412402", glyph: "!" },
  done: { bg: "#1D9E75", fg: "#fff", glyph: "check" },
  error: { bg: "#7F77DD", fg: "#fff", glyph: "x" },
  music: { bg: "#D4537E", fg: "#fff", glyph: "note" },
  listening: { bg: "#E24B4A", fg: "#fff", glyph: "rec" },
};

function Arc({ s, down = false }: { s: number; down?: boolean }) {
  return (
    <svg width={4.6 * s} height={3 * s} viewBox="0 0 9 6" style={{ display: "block" }}>
      <path d={down ? "M1 1 Q4.5 6 8 1" : "M1 5 Q4.5 0 8 5"} fill="none" stroke="#111" strokeWidth={1.9} strokeLinecap="round" />
    </svg>
  );
}

function EyePair({ mood, s }: { mood: TofuMood; s: number }) {
  const gap = 5.2 * s;
  if (mood === "done" || mood === "music" || mood === "eating") {
    return <div className="flex items-center" style={{ gap }}><Arc s={s} /><Arc s={s} /></div>;
  }
  if (mood === "sleep") {
    return <div className="flex items-center" style={{ gap }}><Arc s={s} down /><Arc s={s} down /></div>;
  }
  if (mood === "error") {
    return (
      <div className="flex items-center" style={{ gap }}>
        {[0, 1].map((i) => (
          <motion.span key={i} className="block rounded-full" animate={{ rotate: 360 }} transition={{ duration: 1.2, repeat: Infinity, ease: "linear" }}
            style={{ width: 4.4 * s, height: 4.4 * s, border: `${Math.max(1.2, 0.75 * s)}px solid #111`, borderTopColor: "transparent" }} />
        ))}
      </div>
    );
  }
  const h = mood === "calm" ? 1.6 : mood === "attention" ? 3.8 : 3.3;
  const w = mood === "attention" ? 3.1 : 2.8;
  return (
    <motion.div className="flex items-center" style={{ gap }}
      animate={mood === "working" || mood === "listening" ? { x: [-0.9 * s, 0.9 * s, -0.9 * s] } : { x: 0 }}
      transition={mood === "working" || mood === "listening" ? { duration: 1.8, repeat: Infinity, ease: "easeInOut" } : SPRING}>
      {[0, 1].map((i) => (
        <motion.span key={i} className="block rounded-full" style={{ width: w * s, height: h * s, background: "#111" }}
          animate={{ scaleY: [1, 1, 0.12, 1] }}
          transition={{ duration: mood === "calm" ? 5.5 : 4.2, times: [0, 0.92, 0.95, 1], repeat: Infinity, delay: i * 0.03, ease: "easeInOut" }} />
      ))}
    </motion.div>
  );
}

function Badge({ mood, s }: { mood: TofuMood; s: number }) {
  const b = BADGE[mood];
  if (!b || s < 0.9) return null;
  const d = 7 * s;
  return (
    <motion.span initial={{ scale: 0 }} animate={{ scale: 1 }} exit={{ scale: 0 }} transition={SPRING}
      className="absolute grid place-items-center rounded-full font-bold leading-none"
      style={{ width: d, height: d, top: -2.2 * s, left: -2.2 * s, background: b.bg, color: b.fg, border: `${Math.max(1.5, 0.9 * s)}px solid #000`, fontSize: 4.4 * s }}>
      {b.glyph === "dots" ? (
        <motion.span animate={{ opacity: [0.35, 1, 0.35] }} transition={{ duration: 1, repeat: Infinity }} style={{ fontSize: 3.6 * s, letterSpacing: -0.3 * s }}>•••</motion.span>
      ) : b.glyph === "check" ? (
        <svg width={4 * s} height={4 * s} viewBox="0 0 10 10"><path d="M2 5.2 L4.2 7.4 L8 3" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" /></svg>
      ) : b.glyph === "x" ? (
        <svg width={3.6 * s} height={3.6 * s} viewBox="0 0 10 10"><path d="M2.5 2.5 L7.5 7.5 M7.5 2.5 L2.5 7.5" stroke="currentColor" strokeWidth={2} strokeLinecap="round" /></svg>
      ) : b.glyph === "note" ? (
        <span style={{ fontSize: 4.4 * s }}>♪</span>
      ) : b.glyph === "rec" ? (
        <motion.span className="block rounded-full bg-white" style={{ width: 2.6 * s, height: 2.6 * s }} animate={{ opacity: [1, 0.3, 1] }} transition={{ duration: 0.9, repeat: Infinity }} />
      ) : b.glyph}
    </motion.span>
  );
}

const MOTION: Partial<Record<TofuMood, any>> = {
  idle: { animate: { y: [0, -0.6, 0] }, transition: { duration: 3.2, repeat: Infinity, ease: "easeInOut" } },
  working: { animate: { rotate: [-6, 6, -6] }, transition: { duration: 2.2, repeat: Infinity, ease: "easeInOut" } },
  done: { animate: { y: [0, -3, 0], scaleY: [1, 0.94, 1] }, transition: { duration: 0.6, repeat: 2, ease: "easeOut" } },
  music: { animate: { rotate: [-4, 4, -4], y: [0, -1, 0] }, transition: { duration: 1.2, repeat: Infinity, ease: "easeInOut" } },
  attention: { animate: { x: [0, -1.2, 1.2, -1.2, 0] }, transition: { duration: 0.5, repeat: Infinity, repeatDelay: 1.6 } },
  error: { animate: { rotate: [-3, 3, -3] }, transition: { duration: 0.8, repeat: Infinity } },
  sleep: { animate: { scale: [1, 1.03, 1] }, transition: { duration: 4, repeat: Infinity, ease: "easeInOut" } },
  listening: { animate: { scale: [1, 1.05, 1] }, transition: { duration: 1.1, repeat: Infinity, ease: "easeInOut" } },
};

/* ─── Roupas ───
   Desenhadas num quadro 80×60 onde o corpo do tofu ocupa x 14–66 e y 16–58 (mesma escala do esboço aprovado). */
export type Outfit = "none" | "helmet" | "headphones" | "sunglasses" | "santa" | "cap" | "bow" | "scarf" | "crown";
export const OUTFITS: { id: Outfit; label: string; hint: string }[] = [
  { id: "helmet", label: "Capacete de obra", hint: "Claude trabalhando" },
  { id: "headphones", label: "Fone de ouvido", hint: "tocando música" },
  { id: "sunglasses", label: "Óculos escuros", hint: "modo foco" },
  { id: "santa", label: "Gorro de Natal", hint: "em dezembro" },
  { id: "cap", label: "Boné", hint: "dia a dia" },
  { id: "bow", label: "Laço", hint: "fofura" },
  { id: "scarf", label: "Cachecol", hint: "dias frios" },
  { id: "crown", label: "Coroa", hint: "tarefas do dia 100%" },
];
const OUTFIT_SVG: Record<Exclude<Outfit, "none">, React.ReactNode> = {
  helmet: (<><path d="M12 22 Q40 -4 68 22 Z" fill="#F5B82E" /><rect x="8" y="20" width="64" height="6" rx="3" fill="#E0A11B" /><rect x="37" y="4" width="6" height="16" rx="3" fill="#E0A11B" /></>),
  headphones: (<><path d="M14 32 Q14 6 40 6 Q66 6 66 32" stroke="#333" strokeWidth="4" fill="none" /><rect x="7" y="28" width="11" height="16" rx="5" fill="#E8508F" /><rect x="62" y="28" width="11" height="16" rx="5" fill="#E8508F" /></>),
  sunglasses: (<><rect x="22" y="31" width="16" height="10" rx="4" fill="#111" /><rect x="42" y="31" width="16" height="10" rx="4" fill="#111" /><rect x="37" y="34" width="6" height="2.5" fill="#111" /><rect x="25" y="33" width="5" height="2" rx="1" fill="#555" /></>),
  santa: (<><path d="M16 20 Q34 -6 62 12 L66 20 Z" fill="#D9363E" /><rect x="12" y="17" width="58" height="7" rx="3.5" fill="#fff" /><circle cx="64" cy="10" r="5" fill="#fff" /></>),
  cap: (<><path d="M16 22 Q40 2 64 22 Z" fill="#3A6FD8" /><rect x="40" y="18" width="34" height="6" rx="3" fill="#2B57B0" /><circle cx="40" cy="9" r="2.5" fill="#2B57B0" /></>),
  bow: (<><path d="M40 16 L26 8 L26 24 Z" fill="#F06AA8" /><path d="M40 16 L54 8 L54 24 Z" fill="#F06AA8" /><circle cx="40" cy="16" r="4" fill="#D84F90" /></>),
  scarf: (<><rect x="12" y="48" width="56" height="9" rx="4" fill="#2BA88A" /><rect x="50" y="50" width="9" height="13" rx="3" fill="#228E74" /><rect x="18" y="50" width="4" height="5" fill="#fff" opacity=".6" /><rect x="28" y="50" width="4" height="5" fill="#fff" opacity=".6" /></>),
  crown: (<><path d="M20 20 L24 6 L32 14 L40 4 L48 14 L56 6 L60 20 Z" fill="#F2C744" /><rect x="20" y="18" width="40" height="5" rx="2" fill="#D9AE2C" /><circle cx="40" cy="12" r="2.5" fill="#E2483D" /></>),
};
/** Roupa atual do tofu (o App decide: automática pelo contexto ou fixa nos Ajustes). */
export const OutfitContext = createContext<Outfit>("none");

function OutfitLayer({ outfit, s, front }: { outfit: Outfit; s: number; front: boolean }) {
  if (!outfit || outfit === "none") return null;
  const u = s / 10; // 1 unidade do desenho
  return (
    <motion.svg key={outfit} initial={{ opacity: 0, y: -2 * u }} animate={{ opacity: 1, y: 0 }} transition={SPRING}
      width={80 * u} height={64 * u} viewBox="0 0 80 64" className="absolute pointer-events-none"
      style={{ left: -14 * u, top: -16 * u, overflow: "visible", zIndex: front ? 2 : 0 }}>
      {OUTFIT_SVG[outfit as Exclude<Outfit, "none">]}
    </motion.svg>
  );
}

/** size = escala: 1 → tofu de ~26×21 px (notch fechado); 3 → ~78×63 px (páginas grandes). */
export function KonTofu({ mood = "idle", size = 1, outfit }: { mood?: TofuMood; size?: number; outfit?: Outfit }) {
  const ctxOutfit = useContext(OutfitContext);
  const wear = size >= 0.6 && mood !== "eating" ? (outfit ?? ctxOutfit) : "none";
  const s = size * 5;
  const w = 5.2 * s, h = 4.2 * s;
  const m = MOTION[mood] || {};
  return (
    <motion.div className="relative shrink-0" style={{ width: w, height: h }} animate={m.animate || {}} transition={m.transition}>
      <motion.div className="absolute inset-0 grid place-items-center overflow-visible"
        initial={false} animate={{ backgroundColor: TINT[mood] }} transition={{ duration: 0.4 }}
        style={{ zIndex: 1, borderRadius: 1.55 * s, boxShadow: `inset 0 ${-0.5 * s}px ${1.2 * s}px rgba(0,0,0,.08)` }}>
        {mood === "eating" && (
          <motion.span className="absolute rounded-full bg-[#111]" style={{ top: 0.55 * s, left: 0.9 * s, right: 0.9 * s }}
            initial={{ height: 0 }} animate={{ height: [0.6 * s, 1.0 * s, 0.6 * s] }} transition={{ duration: 0.9, repeat: Infinity }} />
        )}
        <div style={{ marginTop: mood === "eating" ? 1.1 * s : 0 }}><EyePair mood={mood} s={size * 1.25} /></div>
        {mood === "error" && (
          <>
            <span className="absolute rounded-full" style={{ width: 1.5 * s, height: 0.75 * s, background: "#F4C0D1", bottom: 0.9 * s, left: 0.75 * s }} />
            <span className="absolute rounded-full" style={{ width: 1.5 * s, height: 0.75 * s, background: "#F4C0D1", bottom: 0.9 * s, right: 0.75 * s }} />
          </>
        )}
      </motion.div>
      <AnimatePresence>{wear !== "none" && <OutfitLayer key={wear} outfit={wear} s={s} front />}</AnimatePresence>
      {mood === "sleep" && size >= 0.9 && (
        <motion.span className="absolute font-semibold text-indigo-200" style={{ fontSize: 2 * s, right: -1.6 * s, top: -1.6 * s }}
          animate={{ opacity: [0, 1, 0], y: [0.6 * s, -0.4 * s, -1 * s] }} transition={{ duration: 2.4, repeat: Infinity }}>z</motion.span>
      )}
      <AnimatePresence>{BADGE[mood] && <Badge key={mood} mood={mood} s={size * 1.25} />}</AnimatePresence>
    </motion.div>
  );
}

/** Tofu engolindo um arquivo (drop zone da gaveta). */
export function KonTofuEating({ size = 1.6 }: { size?: number }) {
  const s = size * 5;
  return (
    <div className="relative" style={{ width: 5.2 * s, height: 4.2 * s }}>
      <motion.div className="absolute rounded-[3px] bg-white z-0" style={{ width: 2.1 * s, height: 2.5 * s, left: 1.55 * s }}
        animate={{ y: [-3.6 * s, 0.4 * s, 0.8 * s], opacity: [1, 1, 0] }} transition={{ duration: 1.5, repeat: Infinity, ease: "easeIn", times: [0, 0.7, 1] }}>
        <span className="absolute rounded-sm bg-red-500" style={{ top: 1.1 * s, left: 0.4 * s, right: 0.4 * s, height: 0.32 * s }} />
      </motion.div>
      <div className="absolute inset-0 z-10"><KonTofu mood="eating" size={size} /></div>
    </div>
  );
}

export default KonTofu;
