// Dados externos com stale-while-revalidate: devolve na hora o que está em RAM/disco (userData)
// e atualiza em segundo plano quando o TTL expira. Sem rede, o último valor salvo continua valendo.
import { bridge } from "./bridge.js";

const mem = new Map();
async function swr(key, ttlMs, fetcher, onUpdate) {
  let cached = mem.get(key) ?? (await bridge.cacheGet(key));
  if (cached) mem.set(key, cached);
  const fresh = cached && Date.now() - cached.at < ttlMs;
  if (!fresh) {
    const p = fetcher()
      .then(async (value) => {
        const rec = { value, at: Date.now() };
        mem.set(key, rec);
        await bridge.cacheSet(key, rec);
        onUpdate?.(rec);
        return rec;
      })
      .catch(() => null);
    if (!cached) return (await p) || null;
  }
  return cached;
}

/* ───────── Câmbio — https://www.exchangerate-api.com (open access; atribuição obrigatória) ───────── */
export const RATES_TTL = 45 * 60e3;
export function getRates(onUpdate) {
  return swr("rates", RATES_TTL, async () => {
    const d = await (await fetch("https://open.er-api.com/v6/latest/USD")).json();
    if (d.result !== "success") throw new Error("rates");
    return { rates: d.rates, source: d.time_last_update_unix * 1000 };
  }, onUpdate);
}

/* ───────── Localização atual (por IP, sem permissão) — https://www.geojs.io ───────── */
export function getIpLocation() {
  return swr("iploc", 6 * 3600e3, async () => {
    const d = await (await fetch("https://get.geojs.io/v1/ip/geo.json")).json();
    const lat = parseFloat(d.latitude), lon = parseFloat(d.longitude);
    if (!Number.isFinite(lat)) throw new Error("geo");
    return { city: d.city || d.region || "Localização atual", region: d.region, country: d.country_code, lat, lon, tz: d.timezone };
  });
}

/* ───────── Clima — https://open-meteo.com (atual + próximas horas + nascer/pôr do sol) ───────── */
export function getWeather(loc, onUpdate) {
  if (!loc) return Promise.resolve(null);
  const key = `wx2.${loc.lat.toFixed(2)},${loc.lon.toFixed(2)}`;
  return swr(key, 15 * 60e3, async () => {
    const u = `https://api.open-meteo.com/v1/forecast?latitude=${loc.lat}&longitude=${loc.lon}` +
      `&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m` +
      `&hourly=temperature_2m,precipitation_probability,weather_code,is_day` +
      `&daily=sunrise,sunset,temperature_2m_max,temperature_2m_min,precipitation_probability_max&forecast_days=2&timezone=auto`;
    const d = await (await fetch(u)).json();
    if (!d.current) throw new Error("wx");
    return {
      temp: Math.round(d.current.temperature_2m), feels: Math.round(d.current.apparent_temperature),
      humidity: d.current.relative_humidity_2m, wind: Math.round(d.current.wind_speed_10m),
      code: d.current.weather_code, isDay: !!d.current.is_day, nowIso: d.current.time,
      sunrise: d.daily?.sunrise?.[0], sunset: d.daily?.sunset?.[0],
      max: Math.round(d.daily?.temperature_2m_max?.[0]), min: Math.round(d.daily?.temperature_2m_min?.[0]),
      rainMax: d.daily?.precipitation_probability_max?.[0] ?? null,
      hourly: (d.hourly?.time || []).map((t, i) => ({
        t, temp: Math.round(d.hourly.temperature_2m[i]), rain: d.hourly.precipitation_probability?.[i] ?? 0,
        code: d.hourly.weather_code[i], isDay: !!d.hourly.is_day?.[i],
      })),
    };
  }, onUpdate);
}

/* ───────── Letras sincronizadas — https://lrclib.net ───────── */
const JUNK = /\s*[([](official|video|lyrics?|lyric video|audio|hd|4k|visualizer|music video|clipe oficial|ao vivo|live)[^)\]]*[)\]]\s*/gi;
function cleanTrack(title = "", artist = "") {
  let t = title.replace(JUNK, " ").replace(/\s{2,}/g, " ").trim();
  let a = artist.replace(/\s*-\s*Topic$/i, "").replace(/VEVO$/i, "").trim();
  if ((!a || /youtube|chrome/i.test(a)) && t.includes(" - ")) { const [x, ...rest] = t.split(" - "); a = x.trim(); t = rest.join(" - ").trim(); }
  t = t.replace(/\s*[([]?(feat\.?|ft\.?)\s[^)\]]*[)\]]?/i, "").trim();
  return { t, a };
}
function parseLrc(text) {
  const out = [];
  for (const line of String(text).split(/\r?\n/)) {
    const tags = [...line.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (!tags.length) continue;
    const words = line.replace(/\[[^\]]*\]/g, "").trim();
    for (const m of tags) out.push({ t: +m[1] * 60 + +m[2], text: words || "♪" });
  }
  return out.sort((x, y) => x.t - y.t);
}
export async function getLyrics({ title, artist, album, duration }) {
  if (!title) return null;
  const { t, a } = cleanTrack(title, artist);
  const key = `lyrics.${a}|${t}`.toLowerCase();
  const rec = await swr(key, 30 * 864e5, async () => {
    let r = null;
    const qs = new URLSearchParams({ track_name: t, artist_name: a });
    if (album) qs.set("album_name", album);
    if (duration > 0) qs.set("duration", String(Math.round(duration)));
    try { const res = await fetch(`https://lrclib.net/api/get?${qs}`); if (res.ok) r = await res.json(); } catch {}
    if (!r || (!r.syncedLyrics && !r.plainLyrics)) {
      const res = await fetch(`https://lrclib.net/api/search?${new URLSearchParams(a ? { track_name: t, artist_name: a } : { q: t })}`);
      const arr = res.ok ? await res.json() : [];
      r = arr.find((x) => x.syncedLyrics) || arr.find((x) => x.plainLyrics) || null;
    }
    if (r?.syncedLyrics) { const lines = parseLrc(r.syncedLyrics); if (lines.length) return { synced: true, lines }; }
    if (r?.plainLyrics) return { synced: false, lines: r.plainLyrics.split(/\r?\n/).filter((l) => l.trim()).map((text, i) => ({ t: i, text })) };
    return { none: true };
  });
  return rec?.value && !rec.value.none ? rec.value : null;
}

/* ───────── Capa do álbum (fallback quando o Windows não entrega a imagem) — iTunes Search API ───────── */
export async function getCoverArt(title, artist, album) {
  if (!title) return null;
  const { t, a } = cleanTrack(title, artist);
  const rec = await swr(`cover.${a}|${t}`.toLowerCase(), 30 * 864e5, async () => {
    const term = [a, t].filter(Boolean).join(" ");
    const r = await fetch(`https://itunes.apple.com/search?${new URLSearchParams({ term, entity: "song", limit: "5", country: "BR" })}`, { signal: AbortSignal.timeout(5000) });
    const d = r.ok ? await r.json() : { results: [] };
    const norm = (x) => String(x || "").toLowerCase();
    const hit = (d.results || []).find((x) => norm(x.trackName).includes(norm(t).slice(0, 12)) || norm(x.artistName).includes(norm(a).slice(0, 8))) || d.results?.[0];
    const url = hit?.artworkUrl100 ? hit.artworkUrl100.replace(/\/\d+x\d+bb\./, "/600x600bb.") : null;
    return { url };
  });
  return rec?.value?.url || null;
}
