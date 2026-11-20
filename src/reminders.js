// Lembretes em português no Perguntar:
// "me lembra em 20 min de ligar pro cliente", "me lembra às 15h30 da reunião",
// "lembrete amanhã às 9h pagar o boleto", "me lembre daqui a 2 horas de tirar o bolo"
const START = /^\s*(?:me\s+)?(?:lembr(?:a|e|ar|ete)(?:-me)?|avis(?:a|e)(?:-me)?)\b[\s:,-]*/i;
const REL = /\b(?:em|daqui\s+a|daqui|dentro\s+de)\s+(\d+(?:[.,]\d+)?|meia)\s*(min(?:uto)?s?|m|h(?:ora)?s?|seg(?:undo)?s?|s)\b(?:\s+e\s+(\d+)\s*(min(?:uto)?s?|m)\b)?/i;
const ABS = /(?:^|\s)(amanh[ãa]\s+|hoje\s+)?(?:às|as|ao|a)\s+(\d{1,2})(?:\s*[:h]\s*(\d{2}))?\s*(?:hs|h|horas?)?(?![\w])(?:\s*(?:da|de)\s+(manh[ãa]|tarde|noite))?/i;
const DAYWORD = /\b(amanh[ãa])\b/i;

function cleanText(t) {
  return t.replace(/\s{2,}/g, " ").replace(/^\s*(?:de|que|da|do|pra|para|a|o)\s+/i, "").replace(/[\s.,;:!?-]+$/, "").trim();
}

export function parseReminder(input, now = new Date()) {
  const s = String(input || "");
  const st = s.match(START);
  if (!st) return null;
  let rest = s.slice(st[0].length);
  let at = null;
  const r = rest.match(REL);
  if (r) {
    const n = r[1].toLowerCase() === "meia" ? 0.5 : parseFloat(r[1].replace(",", "."));
    const unit = r[2].toLowerCase();
    let ms = unit.startsWith("h") ? n * 3600e3 : unit.startsWith("s") ? n * 1e3 : n * 60e3;
    if (r[3]) ms += parseInt(r[3], 10) * 60e3;
    at = now.getTime() + ms;
    rest = rest.replace(r[0], " ");
  } else {
    const a = rest.match(ABS);
    if (a) {
      let h = parseInt(a[2], 10);
      const mi = a[3] ? parseInt(a[3], 10) : 0;
      const period = (a[4] || "").toLowerCase();
      if ((period === "tarde" || period === "noite") && h < 12) h += 12;
      if (h > 23 || mi > 59) return null;
      const d = new Date(now);
      d.setHours(h, mi, 0, 0);
      const tomorrow = /amanh/i.test(a[1] || "") || DAYWORD.test(rest);
      if (tomorrow) d.setDate(d.getDate() + 1);
      else if (d.getTime() <= now.getTime()) d.setDate(d.getDate() + 1);
      at = d.getTime();
      rest = rest.replace(a[0], " ").replace(DAYWORD, " ");
    }
  }
  if (!at) return null;
  const text = cleanText(rest) || "Lembrete";
  return { text: text.charAt(0).toUpperCase() + text.slice(1), at };
}

export function describeWhen(at, now = new Date()) {
  const d = new Date(at);
  const hm = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  const mins = Math.round((at - now.getTime()) / 60e3);
  const sameDay = d.toDateString() === now.toDateString();
  if (mins < 60) return `em ${Math.max(1, mins)} min (às ${hm})`;
  return sameDay ? `hoje às ${hm}` : `amanhã às ${hm}`;
}
