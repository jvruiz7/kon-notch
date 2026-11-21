//! Tradução: pela IA (Gemini ou Claude, resposta em JSON) e, sem chave, um pequeno tradutor local de frases comuns.

pub const LANGS: [&str; 10] = ["Portuguese", "English", "Indonesian", "Japanese", "Spanish", "French", "German", "Chinese", "Korean", "Arabic"];
pub const TARGETS: [&str; 11] = ["English", "Portuguese", "Spanish", "French", "German", "Italian", "Japanese", "Chinese", "Korean", "Indonesian", "Arabic"];
pub const SYSTEM: &str = "You are a translation engine. Translate faithfully and naturally, keeping tone, names, numbers and line breaks. Reply with JSON only.";

/// Nome do idioma em português, para a tela (o nome em inglês é o que vai para a IA).
pub fn pt(l: &str) -> &'static str {
    match l { "Portuguese" => "Português", "English" => "Inglês", "Indonesian" => "Indonésio", "Japanese" => "Japonês", "Spanish" => "Espanhol", "French" => "Francês",
        "German" => "Alemão", "Chinese" => "Chinês", "Korean" => "Coreano", "Arabic" => "Árabe", "Italian" => "Italiano", _ => "?" }
}

pub fn prompt(src: &str, tgt: &str, text: &str) -> String {
    format!("Source language: {}. Target language: {tgt}.\nReturn JSON exactly like {{\"detected\":\"<source language name in English>\",\"translation\":\"<translated text>\"}}.\nText:\n\"\"\"\n{text}\n\"\"\"", if src == "Detect" { "auto-detect" } else { src })
}

pub fn parse(t: &str) -> Option<(Option<String>, String)> {
    let v: serde_json::Value = serde_json::from_str(t).ok().or_else(|| { let a = t.find('{')?; let b = t.rfind('}')?; serde_json::from_str(&t[a..=b]).ok() })?;
    Some((v["detected"].as_str().map(String::from), v["translation"].as_str()?.to_string()))
}

const PHRASES: &[&[(&str, &str)]] = &[
    &[("English", "Good morning! See you at the meeting at 3 PM."), ("Portuguese", "Bom dia! Nos vemos na reunião às 15h."), ("Spanish", "¡Buenos días! Nos vemos en la reunión a las 3 p. m."), ("French", "Bonjour ! À tout à l'heure à la réunion de 15 h."), ("Indonesian", "Selamat pagi! Sampai jumpa di rapat jam 3 sore."), ("Japanese", "おはようございます！午後3時の会議でお会いしましょう。")],
    &[("English", "Good morning!"), ("Portuguese", "Bom dia!"), ("Spanish", "¡Buenos días!"), ("French", "Bonjour !"), ("Indonesian", "Selamat pagi!"), ("Japanese", "おはようございます！")],
    &[("English", "Good night!"), ("Portuguese", "Boa noite!"), ("Spanish", "¡Buenas noches!"), ("French", "Bonne nuit !"), ("Indonesian", "Selamat malam!"), ("Japanese", "おやすみなさい！")],
    &[("English", "Thank you very much!"), ("Portuguese", "Muito obrigado!"), ("Spanish", "¡Muchas gracias!"), ("French", "Merci beaucoup !"), ("Indonesian", "Terima kasih banyak!"), ("Japanese", "どうもありがとうございます！")],
    &[("English", "How are you?"), ("Portuguese", "Como você está?"), ("Spanish", "¿Cómo estás?"), ("French", "Comment ça va ?"), ("Indonesian", "Apa kabar?"), ("Japanese", "お元気ですか？")],
    &[("English", "Where is the train station?"), ("Portuguese", "Onde fica a estação de trem?"), ("Spanish", "¿Dónde está la estación de tren?"), ("French", "Où est la gare ?"), ("Indonesian", "Di mana stasiun kereta?"), ("Japanese", "駅はどこですか？")],
    &[("English", "I'll send the report tomorrow."), ("Portuguese", "Vou enviar o relatório amanhã."), ("Spanish", "Enviaré el informe mañana."), ("French", "J'enverrai le rapport demain."), ("Indonesian", "Saya akan mengirim laporannya besok."), ("Japanese", "明日レポートを送ります。")],
    &[("English", "Can we reschedule the meeting?"), ("Portuguese", "Podemos remarcar a reunião?"), ("Spanish", "¿Podemos reprogramar la reunión?"), ("French", "Pouvons-nous reporter la réunion ?"), ("Indonesian", "Bisakah kita menjadwal ulang rapat?"), ("Japanese", "会議の日程を変更できますか？")],
];
const WORDS: &[(&str, &[&str])] = &[
    ("Portuguese", &["bom", "dia", "voce", "reuniao", "obrigado", "obrigada", "ola", "nao", "ate", "vemos", "onde", "fica", "estacao", "amanha", "vou", "muito", "esta", "boa", "noite", "relatorio", "podemos", "que", "uma", "para", "com"]),
    ("Spanish", &["buenos", "dias", "gracias", "reunion", "hola", "donde", "estacion", "manana", "muchas", "estas", "enviare", "buenas", "noches", "informe", "que", "una", "para", "con", "el"]),
    ("English", &["the", "good", "morning", "see", "you", "at", "meeting", "thank", "how", "are", "where", "is", "i", "will", "tomorrow", "hello", "night", "send", "can", "we", "and"]),
    ("French", &["bonjour", "merci", "beaucoup", "comment", "ou", "est", "la", "le", "les", "et", "nous", "reunion", "demain"]),
    ("Indonesian", &["selamat", "pagi", "sampai", "jumpa", "rapat", "terima", "kasih", "apa", "kabar", "saya", "akan", "besok", "mana"]),
];
fn norm(s: &str) -> String {
    let mut o = String::new();
    for c in s.to_lowercase().chars() {
        let c = match c { 'á' | 'à' | 'â' | 'ã' | 'ä' => 'a', 'é' | 'è' | 'ê' | 'ë' => 'e', 'í' | 'ì' | 'î' | 'ï' => 'i', 'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o', 'ú' | 'ù' | 'û' | 'ü' => 'u', 'ç' => 'c', 'ñ' => 'n', c => c };
        if c.is_alphanumeric() || c.is_whitespace() { o.push(c); }
    }
    o.split_whitespace().collect::<Vec<_>>().join(" ")
}
pub fn detect(t: &str) -> &'static str {
    if t.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c)) { return "Japanese"; }
    let n = norm(t); let words: Vec<&str> = n.split(' ').collect();
    let mut best = ("English", 0);
    for (l, list) in WORDS { let sc = words.iter().filter(|w| list.contains(w)).count(); if sc > best.1 { best = (l, sc); } }
    best.0
}
pub fn local(text: &str, src: &str, tgt: &str) -> Option<String> {
    if src == tgt { return Some(text.to_string()); }
    fn get(ph: &[(&'static str, &'static str)], l: &str) -> Option<&'static str> { ph.iter().find(|x| x.0 == l).map(|x| x.1) }
    let whole = norm(text);
    for ph in PHRASES { if ph.iter().any(|(_, s)| norm(s) == whole) { if let Some(t) = get(ph, tgt) { return Some(t.into()); } } }
    let mut matched = 0; let mut out = vec![];
    let mut cur = String::new();
    let mut sentences = vec![];
    for c in text.chars() { cur.push(c); if ".!?。！？\n".contains(c) { sentences.push(std::mem::take(&mut cur)); } }
    if !cur.trim().is_empty() { sentences.push(cur); }
    for s in sentences {
        let n = norm(&s);
        if let Some(t) = PHRASES.iter().find(|ph| ph.iter().any(|(_, x)| norm(x) == n)).and_then(|ph| get(ph, tgt)) { matched += 1; out.push(t.to_string()); } else if !s.trim().is_empty() { out.push(s.trim().to_string()); }
    }
    (matched > 0).then(|| out.join(" "))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test] fn local_e_json() {
        assert_eq!(detect("Bom dia, você vem à reunião?"), "Portuguese");
        assert_eq!(local("bom dia!", "Portuguese", "English").unwrap(), "Good morning!");
        assert!(local("texto qualquer sem frase", "Portuguese", "English").is_none());
        assert_eq!(parse("```json\n{\"detected\":\"English\",\"translation\":\"Olá\"}\n```").unwrap().1, "Olá");
    }
}
