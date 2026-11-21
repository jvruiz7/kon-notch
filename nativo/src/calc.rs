//! Respostas instantâneas no Perguntar (iguais ao Kon atual): conta ("12 × 3,5 ="), unidades ("10 kN em kgf=")
//! e câmbio ("100 usd em brl=", cotações da ExchangeRate-API com cache de 45 min).

use std::sync::{Arc, Mutex};

pub fn parse_num(s: &str) -> Option<f64> {
    let s = s.trim();
    let t = if s.contains(',') { s.replace('.', "").replacen(',', ".", 1) }
        else if is_thousands(s) { s.replace('.', "") }
        else { s.to_string() };
    // parseFloat: pega o maior prefixo numérico
    let mut end = 0; let mut dot = false;
    for (i, c) in t.char_indices() {
        if c.is_ascii_digit() || (i == 0 && c == '-') { end = i + 1; }
        else if c == '.' && !dot { dot = true; end = i + 1; }
        else { break; }
    }
    t[..end].trim_end_matches('.').parse().ok()
}
fn is_thousands(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() > 1 && (1..=3).contains(&parts[0].len()) && parts[0].chars().all(|c| c.is_ascii_digit()) && parts[1..].iter().all(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
}

/// "12 + 10% =" → 13,2. Só aceita números e + - × x ÷ / % ( ).
pub fn calc(input: &str) -> Option<f64> {
    let src = input.trim_end().trim_end_matches('=').replace(['×', 'x', 'X'], "*").replace('÷', "/");
    #[derive(Clone, Copy, PartialEq)] enum T { N(f64), Op(char) }
    let mut toks = vec![];
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() { i += 1; continue; }
        if c.is_ascii_digit() {
            let mut j = i; while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.' || cs[j] == ',') { j += 1; }
            toks.push(T::N(parse_num(&cs[i..j].iter().collect::<String>())?)); i = j; continue;
        }
        if "+-*/%()".contains(c) { toks.push(T::Op(c)); i += 1; continue; }
        return None;
    }
    struct P { t: Vec<T>, p: usize }
    impl P {
        fn peek(&self) -> Option<T> { self.t.get(self.p).copied() }
        fn post(&mut self, v: f64) -> (f64, bool) { if self.peek() == Some(T::Op('%')) { self.p += 1; (v / 100.0, true) } else { (v, false) } }
        fn factor(&mut self) -> Option<(f64, bool)> {
            match self.peek()? {
                T::Op('-') => { self.p += 1; let (v, pc) = self.factor()?; Some((-v, pc)) }
                T::Op('(') => { self.p += 1; let v = self.expr()?; if self.peek() != Some(T::Op(')')) { return None; } self.p += 1; Some(self.post(v)) }
                T::N(v) => { self.p += 1; Some(self.post(v)) }
                _ => None,
            }
        }
        fn term(&mut self) -> Option<(f64, bool)> {
            let mut l = self.factor()?;
            while let Some(T::Op(op @ ('*' | '/'))) = self.peek() { self.p += 1; let r = self.factor()?; l = (if op == '*' { l.0 * r.0 } else { l.0 / r.0 }, false); }
            Some(l)
        }
        fn expr(&mut self) -> Option<f64> {
            let mut l = self.term()?.0;
            while let Some(T::Op(op @ ('+' | '-'))) = self.peek() { self.p += 1; let r = self.term()?; let rv = if r.1 { l * r.0 } else { r.0 }; l = if op == '+' { l + rv } else { l - rv }; }
            Some(l)
        }
    }
    let mut p = P { t: toks, p: 0 };
    let v = p.expr()?;
    (p.p == p.t.len() && v.is_finite()).then_some(v)
}

/// Número no formato brasileiro, com até `max` casas.
pub fn fmt_br(v: f64, max: usize) -> String {
    let s = format!("{:.*}", max, v.abs());
    let (int, frac) = s.split_once('.').map(|(a, b)| (a.to_string(), b.trim_end_matches('0').to_string())).unwrap_or((s.clone(), String::new()));
    let mut g = String::new();
    for (i, c) in int.chars().enumerate() { if i > 0 && (int.len() - i) % 3 == 0 { g.push('.'); } g.push(c); }
    let neg = v < 0.0 && (int != "0" || !frac.is_empty());
    format!("{}{}{}", if neg { "-" } else { "" }, g, if frac.is_empty() { String::new() } else { format!(",{frac}") })
}
fn fmt_en(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v.abs());
    let (int, frac) = s.split_once('.').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((s.clone(), String::new()));
    let mut g = String::new();
    for (i, c) in int.chars().enumerate() { if i > 0 && (int.len() - i) % 3 == 0 { g.push(','); } g.push(c); }
    format!("{}{}{}", if v < 0.0 { "-" } else { "" }, g, if frac.is_empty() { String::new() } else { format!(".{frac}") })
}

/// Só números e operadores, terminando com "=" e com pelo menos um operador.
pub fn is_math(q: &str) -> bool {
    let t = q.trim_end();
    t.ends_with('=') && t.chars().all(|c| c.is_ascii_digit() || c.is_whitespace() || ".,+-*×xX÷/%()=".contains(c))
        && t.trim_end_matches(|c: char| c == '=' || c.is_whitespace()).chars().any(|c| "+-*×xX÷/%".contains(c))
}

/* ───────── unidades ───────── */
const U: &[(&str, &[(&str, f64)])] = &[
    ("comprimento", &[("mm", 1e-3), ("cm", 1e-2), ("dm", 0.1), ("m", 1.0), ("km", 1e3), ("in", 0.0254), ("pol", 0.0254), ("polegada", 0.0254), ("polegadas", 0.0254), ("ft", 0.3048), ("pe", 0.3048), ("pes", 0.3048), ("yd", 0.9144), ("mi", 1609.344), ("milha", 1609.344), ("milhas", 1609.344)]),
    ("area", &[("mm²", 1e-6), ("cm²", 1e-4), ("dm²", 1e-2), ("m²", 1.0), ("km²", 1e6), ("ha", 1e4), ("hectare", 1e4), ("hectares", 1e4), ("in²", 0.00064516), ("ft²", 0.09290304)]),
    ("volume", &[("mm³", 1e-9), ("cm³", 1e-6), ("ml", 1e-6), ("dm³", 1e-3), ("l", 1e-3), ("litro", 1e-3), ("litros", 1e-3), ("m³", 1.0), ("ft³", 0.028316846592), ("gal", 0.003785411784), ("galao", 0.003785411784), ("galoes", 0.003785411784)]),
    ("massa", &[("mg", 1e-6), ("g", 1e-3), ("kg", 1.0), ("t", 1e3), ("ton", 1e3), ("tonelada", 1e3), ("toneladas", 1e3), ("lb", 0.45359237), ("oz", 0.028349523125)]),
    ("forca", &[("n", 1.0), ("kn", 1e3), ("mn", 1e6), ("kgf", 9.80665), ("tf", 9806.65), ("lbf", 4.4482216152605), ("kip", 4448.2216152605)]),
    ("pressao", &[("pa", 1.0), ("kpa", 1e3), ("mpa", 1e6), ("gpa", 1e9), ("bar", 1e5), ("mbar", 100.0), ("atm", 101325.0), ("psi", 6894.757293168), ("ksi", 6894757.293168), ("kgf/cm²", 98066.5), ("kgf/m²", 9.80665), ("tf/m²", 9806.65), ("kn/m²", 1e3), ("n/m²", 1.0), ("n/mm²", 1e6), ("kn/cm²", 1e7), ("mca", 9806.65), ("mmhg", 133.322387415)]),
    ("momento", &[("n·m", 1.0), ("kn·m", 1e3), ("kgf·m", 9.80665), ("tf·m", 9806.65), ("kgf·cm", 0.0980665), ("kn·cm", 10.0)]),
    ("carga", &[("n/m", 1.0), ("kn/m", 1e3), ("kgf/m", 9.80665), ("tf/m", 9806.65)]),
    ("velocidade", &[("m/s", 1.0), ("km/h", 1.0 / 3.6), ("mph", 0.44704)]),
    ("energia", &[("j", 1.0), ("kj", 1e3), ("cal", 4.184), ("kcal", 4184.0), ("wh", 3600.0), ("kwh", 3.6e6)]),
    ("potencia", &[("w", 1.0), ("kw", 1e3), ("cv", 735.49875), ("hp", 745.699872)]),
];
const PRETTY: &[(&str, &str)] = &[("l", "L"), ("litro", "L"), ("litros", "L"), ("n", "N"), ("kn", "kN"), ("mn", "MN"), ("pa", "Pa"), ("kpa", "kPa"), ("mpa", "MPa"), ("gpa", "GPa"), ("kn/m²", "kN/m²"), ("n/m²", "N/m²"), ("n/mm²", "N/mm²"), ("kn/cm²", "kN/cm²"), ("kn·m", "kN·m"), ("n·m", "N·m"), ("kn·cm", "kN·cm"), ("kn/m", "kN/m"), ("n/m", "N/m"), ("j", "J"), ("kj", "kJ"), ("wh", "Wh"), ("kwh", "kWh"), ("w", "W"), ("kw", "kW"), ("mca", "m.c.a."), ("mmhg", "mmHg"), ("ml", "mL")];

fn norm(u: &str) -> String {
    let mut s: String = u.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    // ^2 / 2 → ², ^3 / 3 → ³ (quando não seguidos de outro dígito)
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new(); let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '^' && i + 1 < cs.len() && (cs[i + 1] == '2' || cs[i + 1] == '3') && cs.get(i + 2).map_or(true, |d| !d.is_ascii_digit()) { out.push(if cs[i + 1] == '2' { '²' } else { '³' }); i += 2; continue; }
        if (c == '2' || c == '3') && cs.get(i + 1).map_or(true, |d| !d.is_ascii_digit()) && i > 0 && !cs[i - 1].is_ascii_digit() { out.push(if c == '2' { '²' } else { '³' }); i += 1; continue; }
        out.push(match c { '*' | '.' | 'x' | '×' => '·', 'ã' => 'a', 'õ' => 'o', 'é' => 'e', o => o });
        i += 1;
    }
    s = out;
    if s.trim_end_matches('·') == "m·c·a" { s = "mca".into(); }
    s
}
enum Unit { Temp(&'static str), Lin(&'static str, &'static str, f64) }
fn find(u: &str) -> Option<Unit> {
    let n = norm(u);
    match n.as_str() { "c" | "°c" => return Some(Unit::Temp("°C")), "f" | "°f" => return Some(Unit::Temp("°F")), "k" => return Some(Unit::Temp("K")), _ => {} }
    for (dim, table) in U {
        if let Some((k, f)) = table.iter().find(|(k, _)| *k == n) { return Some(Unit::Lin(dim, k, *f)); }
        if *dim == "momento" { let m = n.replace('·', ""); if let Some((k, f)) = table.iter().find(|(k, _)| k.replace('·', "") == m) { return Some(Unit::Lin(dim, k, *f)); } }
    }
    None
}
fn fmt_u(v: f64) -> String {
    let a = v.abs();
    let d = if a == 0.0 { 0 } else if a >= 1000.0 { 2 } else if a >= 1.0 { 4 } else { (3 - a.log10().floor() as i32).min(8).max(0) as usize };
    fmt_br(v, d)
}
/// Separa "N unidade em|para|to|in|->|→ unidade =".
fn split_conv(q: &str) -> Option<(String, String, String)> {
    let t = q.trim();
    let t = t.strip_suffix('=')?.trim_end();
    let num_end = t.char_indices().find(|(i, c)| !(c.is_ascii_digit() || *c == '.' || *c == ',' || (*i == 0 && *c == '-'))).map(|x| x.0)?;
    if num_end == 0 { return None; }
    let (num, rest) = t.split_at(num_end);
    let rest = rest.trim_start();
    if rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_digit()) { return None; }
    for sep in [" em ", " para ", " to ", " in ", " -> ", " → "] {
        let lower = rest.to_lowercase();
        if let Some(i) = lower.find(sep) {
            let (a, b) = (rest[..i].trim(), rest[i + sep.len()..].trim());
            if !a.is_empty() && !b.is_empty() { return Some((num.to_string(), a.to_string(), b.to_string())); }
        }
    }
    None
}
/// (texto, é erro)
pub fn convert_units(q: &str) -> Option<(String, bool)> {
    let (n, a, b) = split_conv(q)?;
    let (from, to) = (find(&a)?, find(&b)?);
    let v = parse_num(&n)?;
    match (from, to) {
        (Unit::Temp(f), Unit::Temp(t)) => {
            let c = match f { "°C" => v, "°F" => (v - 32.0) * 5.0 / 9.0, _ => v - 273.15 };
            let out = match t { "°C" => c, "°F" => c * 9.0 / 5.0 + 32.0, _ => c + 273.15 };
            Some((format!("{} {t}", fmt_u(out)), false))
        }
        (Unit::Lin(d1, _, f1), Unit::Lin(d2, k2, f2)) if d1 == d2 => {
            let pretty = PRETTY.iter().find(|(k, _)| *k == k2).map_or(k2, |x| x.1);
            Some((format!("{} {pretty}", fmt_u(v * f1 / f2)), false))
        }
        _ => Some(("grandezas diferentes".into(), true)),
    }
}

/* ───────── câmbio ───────── */
/// "100 usd em brl =" → (100, USD, BRL)
pub fn parse_currency(q: &str) -> Option<(f64, String, String)> {
    let (n, a, b) = split_conv(q)?;
    let ok = |s: &str| s.len() == 3 && s.chars().all(|c| c.is_ascii_alphabetic());
    if !ok(&a) || !ok(&b) || [" -> ", " → "].iter().any(|s| q.contains(s.trim()) && false) { return None; }
    Some((parse_num(&n)?, a.to_uppercase(), b.to_uppercase()))
}
pub fn fmt_cur(v: f64, c: &str) -> String {
    match c {
        "BRL" => format!("R$ {}", { let s = fmt_br(v, 2); if s.contains(',') { let (a, b) = s.split_once(',').unwrap(); format!("{a},{b:0<2}") } else { format!("{s},00") } }),
        "USD" => format!("${}", fmt_en(v, 2)),
        "EUR" => format!("€{}", fmt_en(v, 2)),
        "GBP" => format!("£{}", fmt_en(v, 2)),
        "JPY" => format!("¥{}", fmt_en(v, 0)),
        "IDR" => format!("Rp{}", fmt_br(v.round(), 0)),
        "CAD" => format!("${}", fmt_en(v, 2)),
        "AUD" => format!("${}", fmt_en(v, 2)),
        "INR" => format!("₹{}", fmt_en(v, 2)),
        "CNY" => format!("¥{}", fmt_en(v, 2)),
        "KRW" => format!("₩{}", fmt_en(v, 0)),
        "MXN" | "ARS" => format!("${}", fmt_en(v, 2)),
        "CHF" => format!("CHF {}", fmt_en(v, 2)),
        _ => format!("{} {c}", fmt_en(v, 2)),
    }
}

#[derive(Default, Clone)]
pub struct Rates { pub at: i64, pub rates: std::collections::HashMap<String, f64>, pub loading: bool }
pub type SharedRates = Arc<Mutex<Rates>>;
const TTL_MS: i64 = 45 * 60 * 1000;
pub fn rates_new() -> SharedRates {
    let r: Rates = std::fs::read_to_string(crate::platform::data_dir().join("rates.json")).ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .map(|v| Rates { at: v["at"].as_i64().unwrap_or(0), rates: v["rates"].as_object().map(|o| o.iter().filter_map(|(k, x)| Some((k.clone(), x.as_f64()?))).collect()).unwrap_or_default(), loading: false })
        .unwrap_or_default();
    Arc::new(Mutex::new(r))
}
pub fn rates_stale(r: &Rates) -> bool { chrono::Utc::now().timestamp_millis() - r.at > TTL_MS * 3 / 2 }
/// Busca as cotações se estiverem velhas (no máximo uma busca por vez).
pub fn rates_refresh(st: &SharedRates, repaint: impl Fn() + Send + 'static) {
    {
        let mut g = st.lock().unwrap();
        if g.loading || chrono::Utc::now().timestamp_millis() - g.at < TTL_MS { return; }
        g.loading = true;
    }
    let st = st.clone();
    std::thread::spawn(move || {
        let r: Option<serde_json::Value> = ureq::get("https://open.er-api.com/v6/latest/USD").timeout(std::time::Duration::from_secs(12)).call().ok().and_then(|r| r.into_json().ok());
        let mut g = st.lock().unwrap();
        g.loading = false;
        if let Some(v) = r.filter(|v| v["result"] == "success") {
            g.rates = v["rates"].as_object().map(|o| o.iter().filter_map(|(k, x)| Some((k.clone(), x.as_f64()?))).collect()).unwrap_or_default();
            g.at = chrono::Utc::now().timestamp_millis();
            let d = crate::platform::data_dir(); let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join("rates.json"), serde_json::json!({"at": g.at, "rates": g.rates}).to_string());
        } else if g.at == 0 { g.at = 1; } // sem rede: para de tentar a cada quadro (tenta de novo em 45 min)
        drop(g); repaint();
    });
}

pub enum Inline { Unit(String, bool), Currency(String), Math(String) }
impl Inline { pub fn text(&self) -> &str { match self { Inline::Unit(t, _) | Inline::Currency(t) | Inline::Math(t) => t } } }
/// O que aparece ao lado do campo enquanto você digita.
pub fn inline(q: &str, rates: &SharedRates, repaint: impl Fn() + Send + 'static) -> Option<Inline> {
    if let Some((t, e)) = convert_units(q) { return Some(Inline::Unit(t, e)); }
    if let Some((v, from, to)) = parse_currency(q) {
        rates_refresh(rates, repaint);
        let g = rates.lock().unwrap();
        return Some(Inline::Currency(match (g.rates.get(&from), g.rates.get(&to)) {
            (Some(a), Some(b)) => fmt_cur(v / a * b, &to),
            _ if g.rates.is_empty() => "carregando…".into(),
            _ => "moeda desconhecida".into(),
        }));
    }
    if is_math(q) { return calc(q).map(|v| Inline::Math(fmt_br(v, 4))); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contas() {
        assert_eq!(calc("200 + 10% =").map(|v| fmt_br(v, 4)), Some("220".into()));
        assert_eq!(calc("12 × 3,5 =").map(|v| fmt_br(v, 4)), Some("42".into()));
        assert_eq!(calc("1.000 / 3 =").map(|v| fmt_br(v, 4)), Some("333,3333".into()));
        assert_eq!(calc("(2+3)*4="), Some(20.0));
        assert!(is_math("2+2=") && !is_math("2+2") && !is_math("22="));
    }
    #[test]
    fn unidades() {
        assert_eq!(convert_units("10 kN em kgf=").unwrap().0, "1.019,72 kgf");
        assert_eq!(convert_units("25 MPa em kgf/cm²=").unwrap().0, "254,9291 kgf/cm²");
        assert_eq!(convert_units("3,5 m³ em litros=").unwrap().0, "3.500 L");
        assert_eq!(convert_units("100 c em f=").unwrap().0, "212 °F");
        assert!(convert_units("1 kg em m=").unwrap().1);
    }
    #[test]
    fn cambio() {
        assert_eq!(parse_currency("100 usd em brl="), Some((100.0, "USD".into(), "BRL".into())));
        assert_eq!(fmt_cur(1234.5, "BRL"), "R$ 1.234,50");
        assert_eq!(fmt_cur(1234.5, "USD"), "$1,234.50");
    }
}
