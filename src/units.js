// Conversão de unidades instantânea no Perguntar: "10 kN em kgf=", "25 MPa em kgf/cm²=", "3,5 m³ em litros="
import { parseNum } from "./data.js";

// fator para a unidade base de cada grandeza
const U = {
  comprimento: { mm: 1e-3, cm: 1e-2, dm: 0.1, m: 1, km: 1e3, in: 0.0254, pol: 0.0254, polegada: 0.0254, polegadas: 0.0254, ft: 0.3048, pe: 0.3048, pes: 0.3048, yd: 0.9144, mi: 1609.344, milha: 1609.344, milhas: 1609.344 },
  area: { "mm²": 1e-6, "cm²": 1e-4, "dm²": 1e-2, "m²": 1, "km²": 1e6, ha: 1e4, hectare: 1e4, hectares: 1e4, "in²": 0.00064516, "ft²": 0.09290304 },
  volume: { "mm³": 1e-9, "cm³": 1e-6, ml: 1e-6, "dm³": 1e-3, l: 1e-3, litro: 1e-3, litros: 1e-3, "m³": 1, "ft³": 0.028316846592, gal: 0.003785411784, galao: 0.003785411784, galoes: 0.003785411784 },
  massa: { mg: 1e-6, g: 1e-3, kg: 1, t: 1e3, ton: 1e3, tonelada: 1e3, toneladas: 1e3, lb: 0.45359237, oz: 0.028349523125 },
  forca: { n: 1, kn: 1e3, mn: 1e6, kgf: 9.80665, tf: 9806.65, lbf: 4.4482216152605, kip: 4448.2216152605 },
  pressao: { pa: 1, kpa: 1e3, mpa: 1e6, gpa: 1e9, bar: 1e5, mbar: 100, atm: 101325, psi: 6894.757293168, ksi: 6894757.293168, "kgf/cm²": 98066.5, "kgf/m²": 9.80665, "tf/m²": 9806.65, "kn/m²": 1e3, "n/m²": 1, "n/mm²": 1e6, "kn/cm²": 1e7, mca: 9806.65, mmhg: 133.322387415 },
  momento: { "n·m": 1, "kn·m": 1e3, "kgf·m": 9.80665, "tf·m": 9806.65, "kgf·cm": 0.0980665, "kn·cm": 10 },
  carga: { "n/m": 1, "kn/m": 1e3, "kgf/m": 9.80665, "tf/m": 9806.65 },
  velocidade: { "m/s": 1, "km/h": 1 / 3.6, mph: 0.44704 },
  energia: { j: 1, kj: 1e3, cal: 4.184, kcal: 4184, wh: 3600, kwh: 3.6e6 },
  potencia: { w: 1, kw: 1e3, cv: 735.49875, hp: 745.699872 },
};
const TEMP = { c: "°C", "°c": "°C", f: "°F", "°f": "°F", k: "K" };
// nome bonito para mostrar no resultado
const PRETTY = { l: "L", litro: "L", litros: "L", n: "N", kn: "kN", mn: "MN", pa: "Pa", kpa: "kPa", mpa: "MPa", gpa: "GPa", "kn/m²": "kN/m²", "n/m²": "N/m²", "n/mm²": "N/mm²", "kn/cm²": "kN/cm²", "kn·m": "kN·m", "n·m": "N·m", "kn·cm": "kN·cm", "kn/m": "kN/m", "n/m": "N/m", j: "J", kj: "kJ", wh: "Wh", kwh: "kWh", w: "W", kw: "kW", mca: "m.c.a.", mmhg: "mmHg", ml: "mL" };

function norm(u) {
  return String(u || "").trim().toLowerCase()
    .replace(/\s+/g, "")
    .replace(/\^?2(?![\d])/g, "²").replace(/\^?3(?![\d])/g, "³")
    .replace(/[*.x×]/g, "·")
    .replace(/^m·c·a·?$/, "mca")
    .replace(/ã/g, "a").replace(/õ/g, "o").replace(/é/g, "e");
}
function find(u) {
  const n = norm(u);
  if (TEMP[n]) return { dim: "temp", key: TEMP[n] };
  for (const [dim, table] of Object.entries(U)) {
    if (n in table) return { dim, key: n, f: table[n] };
    // "kN.m" e "kNm" também valem como momento
    const m = n.replace(/·/g, "");
    if (dim === "momento") for (const k of Object.keys(table)) if (k.replace("·", "") === m) return { dim, key: k, f: table[k] };
  }
  return null;
}
const fmt = (v) => {
  const a = Math.abs(v);
  const digits = a === 0 ? 0 : a >= 1000 ? 2 : a >= 1 ? 4 : Math.min(8, 3 - Math.floor(Math.log10(a)));
  return new Intl.NumberFormat("pt-BR", { maximumFractionDigits: Math.max(0, digits) }).format(v);
};
const RE = /^\s*(-?[\d.,]+)\s*([^\d\s=][^=]*?)\s+(?:em|para|to|in|->|→)\s+([^=]+?)\s*=\s*$/i;

export function convertUnits(query) {
  const m = String(query || "").match(RE);
  if (!m) return null;
  const from = find(m[2]), to = find(m[3]);
  if (!from || !to) return null;
  const v = parseNum(m[1]);
  if (v === null || Number.isNaN(v)) return null;
  if (from.dim !== to.dim) return { text: "grandezas diferentes", error: true };
  let out;
  if (from.dim === "temp") {
    const c = from.key === "°C" ? v : from.key === "°F" ? (v - 32) * 5 / 9 : v - 273.15;
    out = to.key === "°C" ? c : to.key === "°F" ? c * 9 / 5 + 32 : c + 273.15;
    return { text: `${fmt(out)} ${to.key}` };
  }
  out = (v * from.f) / to.f;
  return { text: `${fmt(out)} ${PRETTY[to.key] || to.key}` };
}
