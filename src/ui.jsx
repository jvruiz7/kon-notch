import React from "react";
import { motion, AnimatePresence, Reorder, useDragControls } from "framer-motion";
import { GripHorizontal } from "lucide-react";
import { SPRING, PAGE_META } from "./data.js";
import { KonTofu } from "./components/KonMascot";

/* ═════════════════════════ Mascote ═════════════════════════ */
const MOOD_MAP = { happy: "music", reading: "working", love: "done", sparkles: "working", rest: "done" };
/** Mantém a API antiga (<Eyes mood scale>) mas desenha o mascote tofu do Kon. */
export function Eyes({ mood = "idle", scale = 1 }) {
  const m = MOOD_MAP[mood] || mood;
  return <KonTofu mood={m} size={scale >= 2 ? scale * 0.85 : scale * 0.82} />;
}

/* ═════════════════════════ Peças visuais ═════════════════════════ */
export function AlbumArt({ id, className = "" }) {
  return (
    <svg viewBox="0 0 100 100" preserveAspectRatio="xMidYMid slice" className={className}>
      <defs>
        <linearGradient id={`${id}-sky`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#0a0f45" /><stop offset=".42" stopColor="#4a1772" />
          <stop offset=".7" stopColor="#d63a86" /><stop offset="1" stopColor="#ff8fb4" />
        </linearGradient>
        <linearGradient id={`${id}-sun`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff6a8" /><stop offset=".6" stopColor="#ffd166" /><stop offset="1" stopColor="#ff9f5a" />
        </linearGradient>
        <clipPath id={`${id}-clip`}>
          <rect x="0" y="30" width="100" height="27" /><rect x="0" y="58.5" width="100" height="3" />
          <rect x="0" y="63" width="100" height="2.4" /><rect x="0" y="66.8" width="100" height="1.8" /><rect x="0" y="70" width="100" height="1.2" />
        </clipPath>
      </defs>
      <rect width="100" height="100" fill={`url(#${id}-sky)`} />
      {[[12, 10], [28, 18], [70, 8], [86, 20], [52, 14], [8, 30], [92, 34]].map(([x, y], i) => (
        <circle key={i} cx={x} cy={y} r={i % 2 ? 0.6 : 0.9} fill="#fff" opacity=".7" />
      ))}
      <g clipPath={`url(#${id}-clip)`}><circle cx="50" cy="56" r="24" fill={`url(#${id}-sun)`} /></g>
      <polygon points="0,100 0,76 10,66 22,74 34,60 48,72 60,56 74,68 88,58 100,66 100,100" fill="#c2307a" opacity=".9" />
      <polygon points="0,100 0,80 14,70 26,77 40,64 54,75 66,68 80,77 92,71 100,74 100,100" fill="#6a1b6e" />
      <rect x="0" y="82" width="100" height="18" fill="#130f3d" />
      <g stroke="#ff5fa2" strokeWidth=".45" opacity=".55">
        {[-60, -35, -15, 0, 15, 35, 60].map((dx, i) => <line key={i} x1="50" y1="82" x2={50 + dx * 1.6} y2="100" />)}
        {[84, 87, 91, 96].map((y) => <line key={y} x1="0" y1={y} x2="100" y2={y} />)}
      </g>
    </svg>
  );
}

export function Equalizer({ playing }) {
  const colors = ["#ff5fa2", "#ffd166", "#ff7ab8", "#ffe08a", "#ff4f93"];
  const seqs = [[4, 14, 7, 12, 4], [10, 5, 14, 6, 10], [6, 12, 4, 14, 6], [12, 6, 10, 4, 12], [5, 10, 14, 7, 5]];
  return (
    <div className="flex items-center gap-[2.5px] h-[16px]">
      {colors.map((c, i) => (
        <motion.span key={i} className="w-[3px] rounded-full" style={{ background: c, boxShadow: `0 0 4px ${c}88` }}
          animate={{ height: playing ? seqs[i] : 3 }}
          transition={playing ? { duration: 0.9 + i * 0.07, repeat: Infinity, ease: "easeInOut" } : { duration: 0.3 }} />
      ))}
    </div>
  );
}

/* Onda sonora do notch compacto: barras na cor da capa do álbum */
export function Waveform({ playing, color }) {
  const c = color || "#ff7ab8";
  const seqs = [[3, 9, 5, 12, 3], [6, 14, 8, 5, 6], [10, 5, 15, 7, 10], [5, 12, 6, 14, 5], [12, 7, 10, 4, 12], [4, 10, 13, 6, 4], [7, 4, 9, 12, 7]];
  return (
    <div className="flex items-center gap-[2px] h-[18px]">
      {seqs.map((seq, i) => (
        <motion.span key={i} className="w-[2.5px] rounded-full" style={{ background: c, boxShadow: `0 0 5px ${c}` }}
          animate={{ height: playing ? seq : 3, opacity: playing ? 1 : 0.5 }}
          transition={playing ? { duration: 0.8 + i * 0.09, repeat: Infinity, ease: "easeInOut" } : { duration: 0.3 }} />
      ))}
    </div>
  );
}

/* Seletor segmentado padrão do Kon */
export function Seg({ value, options, onChange, id, small }) {
  return (
    <div className={`flex ${small ? "h-[20px] text-[10px]" : "h-7 text-[11px]"} rounded-full bg-white/[0.08] p-[2px] font-medium shrink-0`}>
      {options.map(([v, label]) => (
        <button key={v} onClick={() => onChange(v)} className={`relative px-2.5 rounded-full whitespace-nowrap ${value === v ? "text-black" : "text-white/60 hover:text-white/85"}`}>
          {value === v && <motion.span layoutId={`seg-${id}`} className="absolute inset-0 rounded-full bg-white" transition={SPRING} />}
          <span className="relative">{label}</span>
        </button>
      ))}
    </div>
  );
}

export function Ring({ progress, size = 20, stroke = 2.6, color = "#a855f7", track = "rgba(168,85,247,.25)" }) {
  const r = (size - stroke) / 2, c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} className="-rotate-90 shrink-0">
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={track} strokeWidth={stroke} />
      <motion.circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth={stroke} strokeLinecap="round"
        strokeDasharray={c} initial={false} animate={{ strokeDashoffset: c * (1 - Math.max(0, Math.min(1, progress))) }}
        transition={{ duration: 0.8, ease: "easeOut" }} style={{ filter: `drop-shadow(0 0 3px ${color})` }} />
    </svg>
  );
}

export function Ruler({ remaining, onPick }) {
  const pct = (Math.min(remaining / 60, 60) / 60) * 100;
  return (
    <div className="relative h-[44px] mx-1 cursor-pointer select-none"
      onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); onPick(Math.max(1, Math.round(((e.clientX - r.left) / r.width) * 60))); }}>
      <motion.div className="absolute left-0 top-0 h-[22px] rounded-sm bg-gradient-to-r from-purple-500/0 to-purple-500/20"
        initial={false} animate={{ width: `${pct}%` }} transition={{ duration: 0.9, ease: "linear" }} />
      <div className="absolute inset-x-0 top-0 h-[22px] flex justify-between items-start">
        {Array.from({ length: 61 }, (_, i) => <span key={i} className={`w-px rounded-full ${i % 10 === 0 ? "h-[18px] bg-white/60" : "h-[9px] bg-white/20"}`} />)}
      </div>
      {[0, 10, 20, 30, 40, 50, 60].map((n) => (
        <span key={n} className="absolute top-[25px] -translate-x-1/2 text-[10px] font-medium text-white/40 tabular-nums font-mono" style={{ left: `${(n / 60) * 100}%` }}>{n}</span>
      ))}
      <motion.div className="absolute -top-[5px] h-[30px] w-[3px] rounded-full bg-gradient-to-b from-white to-purple-400"
        style={{ x: "-50%", boxShadow: "0 0 10px rgba(168,85,247,.9)" }} initial={false} animate={{ left: `${pct}%` }} transition={{ duration: 0.9, ease: "linear" }} />
    </div>
  );
}

export function Switch({ on, onChange, color = "#34c759" }) {
  return (
    <button onClick={(e) => { e.stopPropagation(); onChange(!on); }}
      className="relative w-[38px] h-[22px] rounded-full shrink-0 transition-colors duration-300"
      style={{ background: on ? color : "rgba(255,255,255,.15)" }}>
      <motion.span className="absolute top-[2px] left-[2px] w-[18px] h-[18px] rounded-full bg-white shadow-[0_2px_6px_rgba(0,0,0,.35)]"
        initial={false} animate={{ x: on ? 16 : 0 }} transition={SPRING} />
    </button>
  );
}

export function PillBtn({ children, onClick, className = "", primary = false }) {
  const tone = primary ? "text-black bg-white hover:bg-white/90 font-semibold" : "text-white/85 bg-white/[0.08] hover:bg-white/[0.14]";
  return (
    <motion.button whileTap={{ scale: 0.92 }} onClick={onClick}
      className={`h-7 px-3 rounded-full flex items-center gap-1.5 text-[11.5px] font-medium whitespace-nowrap transition-colors ${/\bbg-/.test(className) ? className : `${tone} ${className}`}`}>
      {children}
    </motion.button>
  );
}

/* Linha do modal Pages (precisa do hook de drag por item) */
export function PageRow({ id, on, onToggle }) {
  const controls = useDragControls();
  const meta = PAGE_META[id];
  return (
    <Reorder.Item value={id} dragListener={false} dragControls={controls}
      whileDrag={{ scale: 1.03, backgroundColor: "rgba(255,255,255,0.1)", boxShadow: "0 12px 30px rgba(0,0,0,.45)" }}
      className="relative flex items-center gap-3 h-[40px] pl-1.5 pr-2 rounded-xl bg-white/[0.04] select-none">
      <span onPointerDown={(e) => controls.start(e)} className="w-6 h-8 grid place-items-center cursor-grab active:cursor-grabbing touch-none text-white/35 hover:text-white/70">
        <GripHorizontal size={16} />
      </span>
      <span className={`w-7 h-7 rounded-[8px] grid place-items-center ${meta.sq}`}><meta.Icon size={15} className="text-white" strokeWidth={2.3} /></span>
      <span className={`flex-1 text-[13.5px] font-medium transition-colors ${on ? "text-white" : "text-white/40"}`}>{meta.label}</span>
      <Switch on={on} onChange={onToggle} color="#10b981" />
    </Reorder.Item>
  );
}

