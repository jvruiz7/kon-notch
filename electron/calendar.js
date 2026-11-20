// Agenda por assinatura ICS (Google Agenda, Outlook, iCloud): baixa o .ics, expande recorrências
// simples e devolve os eventos de ontem até daqui a 7 dias, com o link da reunião quando houver.
const DAY = 864e5;
const MEET_RE = /https?:\/\/(?:meet\.google\.com\/[a-z0-9-]+|[\w.-]*zoom\.us\/(?:j|my|w)\/[^\s"<>\\]+|teams\.microsoft\.com\/l\/meetup-join\/[^\s"<>\\]+|teams\.live\.com\/meet\/[^\s"<>\\]+|[\w.-]*webex\.com\/[^\s"<>\\]+|whereby\.com\/[^\s"<>\\]+)/i;

function tzOffset(utcMs, tz) {
  try {
    const f = new Intl.DateTimeFormat("en-US", { timeZone: tz, hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" });
    const p = Object.fromEntries(f.formatToParts(new Date(utcMs)).map((x) => [x.type, x.value]));
    return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour % 24, +p.minute, +p.second) - utcMs;
  } catch { return null; }
}
function parseDate(value, params = {}) {
  const m = /^(\d{4})(\d{2})(\d{2})(?:T(\d{2})(\d{2})(\d{2})?(Z)?)?/.exec(value || "");
  if (!m) return null;
  const [, y, mo, d, h, mi, s, z] = m;
  if (!h) return { t: new Date(+y, +mo - 1, +d).getTime(), allDay: true };
  const guess = Date.UTC(+y, +mo - 1, +d, +h, +mi, +(s || 0));
  if (z) return { t: guess, allDay: false };
  if (params.TZID) {
    let off = tzOffset(guess, params.TZID);
    if (off !== null) { const t1 = guess - off; off = tzOffset(t1, params.TZID); return { t: guess - off, allDay: false }; }
  }
  return { t: new Date(+y, +mo - 1, +d, +h, +mi, +(s || 0)).getTime(), allDay: false }; // hora local
}
const unesc = (s) => String(s || "").replace(/\\n/gi, "\n").replace(/\\([,;\\])/g, "$1");

function parseICS(text) {
  const lines = String(text).replace(/\r\n[ \t]/g, "").replace(/\n[ \t]/g, "").split(/\r?\n/);
  const events = [];
  let cur = null;
  for (const raw of lines) {
    if (raw === "BEGIN:VEVENT") { cur = { exdates: [] }; continue; }
    if (raw === "END:VEVENT") { if (cur && cur.start) events.push(cur); cur = null; continue; }
    if (!cur) continue;
    const i = raw.indexOf(":");
    if (i < 0) continue;
    const [name, ...ps] = raw.slice(0, i).split(";");
    const params = Object.fromEntries(ps.map((p) => p.split("=")).map(([k, v]) => [k.toUpperCase(), (v || "").replace(/"/g, "")]));
    const value = raw.slice(i + 1);
    switch (name.toUpperCase()) {
      case "DTSTART": cur.start = parseDate(value, params); break;
      case "DTEND": cur.end = parseDate(value, params); break;
      case "SUMMARY": cur.title = unesc(value); break;
      case "LOCATION": cur.location = unesc(value); break;
      case "DESCRIPTION": cur.description = unesc(value); break;
      case "URL": cur.url = value; break;
      case "UID": cur.uid = value; break;
      case "STATUS": cur.status = value; break;
      case "RRULE": cur.rrule = Object.fromEntries(value.split(";").map((p) => p.split("="))); break;
      case "RECURRENCE-ID": cur.recurrenceId = parseDate(value, params); break;
      case "EXDATE": for (const v of value.split(",")) { const d = parseDate(v, params); if (d) cur.exdates.push(d.t); } break;
    }
  }
  return events;
}

const BYDAY = { SU: 0, MO: 1, TU: 2, WE: 3, TH: 4, FR: 5, SA: 6 };
function occurrences(ev, from, to) {
  const dur = ev.end ? ev.end.t - ev.start.t : ev.start.allDay ? DAY : 30 * 60e3;
  const out = [];
  const push = (t) => { if (t + dur >= from && t <= to && !ev.exdates.includes(t)) out.push(t); };
  const r = ev.rrule;
  if (!r) { push(ev.start.t); return { list: out, dur }; }
  const freq = r.FREQ, interval = +(r.INTERVAL || 1), count = r.COUNT ? +r.COUNT : Infinity;
  const until = r.UNTIL ? parseDate(r.UNTIL)?.t ?? Infinity : Infinity;
  const days = r.BYDAY ? r.BYDAY.split(",").map((d) => BYDAY[d.slice(-2)]).filter((x) => x !== undefined) : null;
  const s = new Date(ev.start.t);
  let n = 0;
  for (let k = 0; k < 2000 && n < count; k++) {
    let cands = [];
    if (freq === "DAILY") cands = [new Date(s.getFullYear(), s.getMonth(), s.getDate() + k * interval, s.getHours(), s.getMinutes()).getTime()];
    else if (freq === "WEEKLY") {
      const base = new Date(s.getFullYear(), s.getMonth(), s.getDate() - s.getDay() + k * 7 * interval, s.getHours(), s.getMinutes());
      cands = (days || [s.getDay()]).map((wd) => new Date(base.getFullYear(), base.getMonth(), base.getDate() + wd, s.getHours(), s.getMinutes()).getTime()).sort((a, b) => a - b);
    } else if (freq === "MONTHLY") cands = [new Date(s.getFullYear(), s.getMonth() + k * interval, s.getDate(), s.getHours(), s.getMinutes()).getTime()];
    else if (freq === "YEARLY") cands = [new Date(s.getFullYear() + k * interval, s.getMonth(), s.getDate(), s.getHours(), s.getMinutes()).getTime()];
    else { push(ev.start.t); break; }
    let stop = false;
    for (const t of cands) {
      if (t < ev.start.t) continue;
      if (t > until || t > to || n >= count) { stop = true; break; }
      n++; push(t);
    }
    if (stop) break;
  }
  return { list: out, dur };
}

async function fetchCalendar(url) {
  const u = String(url || "").trim().replace(/^webcals?:\/\//i, "https://");
  if (!/^https?:\/\//i.test(u)) throw new Error("Endereço ICS inválido");
  const res = await fetch(u, { headers: { "User-Agent": "Kon" } });
  if (!res.ok) throw new Error(`A agenda respondeu ${res.status}`);
  const text = await res.text();
  if (!text.includes("BEGIN:VCALENDAR")) throw new Error("Esse link não é um arquivo .ics");
  const now = Date.now();
  const from = now - DAY, to = now + 8 * DAY;
  const evs = parseICS(text);
  const overrides = new Map();
  for (const e of evs) if (e.recurrenceId) overrides.set(`${e.uid}|${e.recurrenceId.t}`, e);
  const list = [];
  for (const e of evs) {
    if (e.status === "CANCELLED") continue;
    if (e.recurrenceId) { const { list: l, dur } = occurrences({ ...e, rrule: null }, from, to); l.forEach((t) => list.push(mk(e, t, dur))); continue; }
    const { list: l, dur } = occurrences(e, from, to);
    for (const t of l) if (!overrides.has(`${e.uid}|${t}`)) list.push(mk(e, t, dur));
  }
  return list.sort((a, b) => a.start - b.start).slice(0, 200);
}
function mk(e, t, dur) {
  const hay = [e.location, e.url, e.description].filter(Boolean).join("\n");
  const link = (hay.match(MEET_RE) || [])[0] || null;
  return { id: `${e.uid || e.title}|${t}`, title: e.title || "(sem título)", start: t, end: t + dur, allDay: !!e.start.allDay, location: e.location || "", link };
}

module.exports = { fetchCalendar, parseICS };
