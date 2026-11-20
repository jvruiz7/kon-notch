// @ts-nocheck
import React, { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { Sparkles } from "lucide-react";
import { bridge } from "./bridge.js";

// Janela temporária fullscreen TRANSPARENTE aberta pelo Ctrl+Alt+T: cobre 100% do monitor ativo e deixa
// desenhar o recorte sobre a tela real. A captura já foi feita antes (sem a ilha) e é recortada no main.
export default function OcrOverlay() {
  const [rect, setRect] = useState(null);
  const start = useRef(null);

  useEffect(() => {
    document.documentElement.style.background = "transparent";
    const onKey = (e) => { if (e.key === "Escape") bridge.ocrCancel(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const finish = () => {
    const r = rect;
    start.current = null;
    if (!r || r.w < 6 || r.h < 6) { setRect(null); return; }
    bridge.ocrRegion(r);
  };

  return (
    <div
      className="fixed inset-0 cursor-crosshair select-none overflow-hidden"
      onMouseDown={(e) => { start.current = { x: e.clientX, y: e.clientY }; setRect({ x: e.clientX, y: e.clientY, w: 0, h: 0 }); }}
      onMouseMove={(e) => {
        const s = start.current;
        if (!s) return;
        setRect({ x: Math.min(s.x, e.clientX), y: Math.min(s.y, e.clientY), w: Math.abs(e.clientX - s.x), h: Math.abs(e.clientY - s.y) });
      }}
      onMouseUp={finish}
    >
      {/* escurecimento fora do recorte */}
      <div className="absolute inset-0 bg-black/30 pointer-events-none"
        style={rect ? { clipPath: `polygon(0 0,100% 0,100% 100%,0 100%,0 0,${rect.x}px ${rect.y}px,${rect.x}px ${rect.y + rect.h}px,${rect.x + rect.w}px ${rect.y + rect.h}px,${rect.x + rect.w}px ${rect.y}px,${rect.x}px ${rect.y}px)` } : undefined} />
      <motion.div initial={{ opacity: 0, y: -8 }} animate={{ opacity: 1, y: 0 }}
        className="absolute top-14 left-0 right-0 mx-auto w-fit h-9 px-4 rounded-full bg-black/85 text-[12.5px] font-medium text-white/90 flex items-center gap-2 ring-1 ring-white/10 shadow-2xl font-sans">
        <Sparkles size={14} className="text-indigo-300" /> Arraste sobre o texto para capturar · Esc cancela
      </motion.div>
      {rect && (
        <div className="absolute border-2 border-indigo-400 shadow-[0_0_20px_rgba(99,102,241,0.5)] rounded-xl pointer-events-none"
          style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }} />
      )}
    </div>
  );
}
