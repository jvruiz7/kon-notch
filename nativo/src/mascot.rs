//! O mascote do Kon (o "tofu"), igual ao KonMascot.tsx do Kon atual: expressão, tom, selo, movimento e roupas.

use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood { Idle, Working, Attention, Done, Error, Eating, Music, Calm, Sleep, Listening }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outfit { None, Helmet, Headphones, Sunglasses, Santa, Cap, Bow, Scarf, Crown }

impl Outfit {
    pub const ALL: [(Outfit, &'static str, &'static str, &'static str); 8] = [
        (Outfit::Helmet, "helmet", "Capacete de obra", "Claude trabalhando"), (Outfit::Headphones, "headphones", "Fone de ouvido", "tocando música"),
        (Outfit::Sunglasses, "sunglasses", "Óculos escuros", "modo foco"), (Outfit::Santa, "santa", "Gorro de Natal", "em dezembro"),
        (Outfit::Cap, "cap", "Boné", "dia a dia"), (Outfit::Bow, "bow", "Laço", "fofura"), (Outfit::Scarf, "scarf", "Cachecol", "dias frios"),
        (Outfit::Crown, "crown", "Coroa", "tarefas do dia 100%"),
    ];
    pub fn from_id(id: &str) -> Outfit { Self::ALL.iter().find(|o| o.1 == id).map_or(Outfit::None, |o| o.0) }
}

fn hex(h: u32) -> Color32 { Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8) }
fn fade(c: Color32, a: f32) -> Color32 { c.gamma_multiply(a.clamp(0.0, 1.0)) }
const INK: Color32 = Color32::from_rgb(17, 17, 17);

fn tint(m: Mood) -> Color32 {
    match m { Mood::Attention => hex(0xFFF1DC), Mood::Done => hex(0xE3F7EE), Mood::Music => hex(0xFFEAF3), Mood::Calm => hex(0xF1EEFF), Mood::Sleep => hex(0xEEF0F6), Mood::Listening => hex(0xFFECEC), _ => Color32::WHITE }
}

/// Mesma piscada do Kon atual: 140 ms a cada ~4,2 s (5,5 s no modo calmo). `blink` vem do relógio do app.
/// `size`: 1 → tofu de 26×21 px. `t`: segundos desde o início (para os movimentos).
pub fn draw(p: &egui::Painter, center: Pos2, size: f32, mood: Mood, outfit: Outfit, blink: bool, t: f32, a: f32) {
    let s = size * 5.0;
    let (w, h) = (5.2 * s, 4.2 * s);
    // movimentos (iguais aos do framer-motion do original)
    let mut c = center;
    let mut rot = 0.0f32;
    let mut sc = 1.0f32;
    match mood {
        Mood::Working => rot = (t * std::f32::consts::TAU / 2.2).sin() * 6f32.to_radians(),
        Mood::Attention => { let k = (t % 2.1) / 0.5; if k < 1.0 { c.x += (k * std::f32::consts::TAU * 2.0).sin() * 1.2; } }
        Mood::Error => rot = (t * std::f32::consts::TAU / 0.8).sin() * 3f32.to_radians(),
        Mood::Listening => sc = 1.0 + 0.05 * (t * std::f32::consts::TAU / 1.1).sin().abs(),
        _ => {}
    }
    let wear = if size >= 0.6 && mood != Mood::Eating { outfit } else { Outfit::None };
    let body = Rect::from_center_size(c, vec2(w * sc, h * sc));
    // roupas que ficam atrás? (no original todas ficam na frente; desenhamos depois do corpo)
    let mut shapes: Vec<Shape> = vec![];
    if rot.abs() < 0.0001 {
        shapes.push(Shape::rect_filled(body, CornerRadius::same((1.55 * s * sc) as u8), fade(tint(mood), a)));
    } else {
        shapes.push(rotated_rr(body, 1.55 * s * sc, rot, fade(tint(mood), a)));
    }
    // sombra interna leve na base
    let base = Rect::from_min_max(pos2(body.left() + 1.55 * s * 0.6, body.bottom() - 1.0 * s), pos2(body.right() - 1.55 * s * 0.6, body.bottom() - 0.15 * s));
    let _ = base;
    // olhos
    let es = size * 1.25;
    let gap = 5.2 * es;
    let rotp = |pt: Pos2| -> Pos2 { if rot == 0.0 { pt } else { let d = pt - c; let (sn, cs) = rot.sin_cos(); pos2(c.x + d.x * cs - d.y * sn, c.y + d.x * sn + d.y * cs) } };
    let ink = fade(INK, a);
    match mood {
        Mood::Done | Mood::Music | Mood::Eating | Mood::Sleep => {
            let down = mood == Mood::Sleep;
            let (aw, ah) = (4.6 * es, 3.0 * es);
            for k in [-1.0f32, 1.0] {
                let cx = c.x + k * (gap / 2.0 + aw / 2.0);
                let y0 = c.y - ah / 2.0 + if mood == Mood::Eating { 1.1 * s / 2.0 } else { 0.0 };
                // caminho do SVG: M1 5 Q4.5 0 8 5 (ou M1 1 Q4.5 6 8 1), num quadro 9×6
                let (p0, ctl, p1) = if down { ((1.0, 1.0), (4.5, 6.0), (8.0, 1.0)) } else { ((1.0, 5.0), (4.5, 0.0), (8.0, 5.0)) };
                let map = |(x, y): (f32, f32)| pos2(cx - aw / 2.0 + x / 9.0 * aw, y0 + y / 6.0 * ah);
                let pts: Vec<Pos2> = quad(map(p0), map(ctl), map(p1), 10).into_iter().map(rotp).collect();
                shapes.push(Shape::line(pts, Stroke::new(1.9 / 9.0 * aw, ink)));
            }
        }
        Mood::Error => {
            let d = 4.4 * es;
            for k in [-1.0f32, 1.0] {
                let cc = rotp(pos2(c.x + k * (gap / 2.0 + d / 2.0), c.y));
                let st = t * std::f32::consts::TAU / 1.2;
                let pts: Vec<Pos2> = (0..=20).map(|i| { let an = st + i as f32 / 20.0 * std::f32::consts::TAU * 0.75; pos2(cc.x + d / 2.0 * 0.82 * an.cos(), cc.y + d / 2.0 * 0.82 * an.sin()) }).collect();
                shapes.push(Shape::line(pts, Stroke::new((0.75 * s).max(1.2) * 0.9, ink)));
            }
            for k in [-1.0f32, 1.0] {
                let ch = Rect::from_center_size(pos2(c.x + k * (w / 2.0 - 0.75 * s - 0.75 * s), body.bottom() - 0.9 * s - 0.375 * s), vec2(1.5 * s, 0.75 * s));
                shapes.push(Shape::rect_filled(ch, CornerRadius::same(255), fade(hex(0xF4C0D1), a)));
            }
        }
        _ => {
            let eh = match mood { Mood::Calm => 1.6, Mood::Attention => 3.8, _ => 3.3 } * es;
            let ew = if mood == Mood::Attention { 3.1 } else { 2.8 } * es;
            let shift = if matches!(mood, Mood::Working | Mood::Listening) { (t * std::f32::consts::TAU / 1.8).sin() * 0.9 * es } else { 0.0 };
            let eh = if blink { eh * 0.12 } else { eh };
            for k in [-1.0f32, 1.0] {
                let ec = rotp(pos2(c.x + shift + k * (gap / 2.0 + ew / 2.0), c.y));
                shapes.push(Shape::rect_filled(Rect::from_center_size(ec, vec2(ew, eh.max(0.6))), CornerRadius::same(255), ink));
            }
        }
    }
    if mood == Mood::Eating {
        let m = Rect::from_min_max(pos2(body.left() + 0.9 * s, body.top() + 0.55 * s), pos2(body.right() - 0.9 * s, body.top() + 0.55 * s + (0.6 + 0.4 * (t * std::f32::consts::TAU / 0.9).sin().abs()) * s));
        shapes.push(Shape::rect_filled(m, CornerRadius::same(255), ink));
    }
    for sh in shapes { p.add(sh); }
    if wear != Outfit::None { draw_outfit(p, body.min, s, wear, a); }
    if mood == Mood::Sleep && size >= 0.9 {
        let k = (t % 2.4) / 2.4;
        let op = if k < 0.5 { k * 2.0 } else { (1.0 - k) * 2.0 };
        p.text(pos2(body.right() + 1.6 * s - 1.0 * s, body.top() - 1.6 * s + 1.0 * s + (0.6 - 1.6 * k) * s), egui::Align2::CENTER_CENTER, "z",
            egui::FontId::new(2.0 * s, egui::FontFamily::Name("bold".into())), fade(Color32::from_rgb(199, 210, 254), a * op));
    }
    badge(p, body.min, es, mood, t, a);
}

/// Pede quadros enquanto o mascote anima (para não repintar à toa quando ele está parado).
pub fn animates(m: Mood) -> bool { matches!(m, Mood::Working | Mood::Attention | Mood::Error | Mood::Listening | Mood::Eating | Mood::Sleep) }

fn badge(p: &egui::Painter, tl: Pos2, es: f32, mood: Mood, t: f32, a: f32) {
    let (bg, fg, glyph) = match mood {
        Mood::Working => (hex(0x378ADD), Color32::WHITE, "dots"), Mood::Attention => (hex(0xEF9F27), hex(0x412402), "!"),
        Mood::Done => (hex(0x1D9E75), Color32::WHITE, "check"), Mood::Error => (hex(0x7F77DD), Color32::WHITE, "x"),
        Mood::Music => (hex(0xD4537E), Color32::WHITE, "note"), Mood::Listening => (hex(0xE24B4A), Color32::WHITE, "rec"),
        _ => return,
    };
    if es / 1.25 < 0.9 { return; }
    let d = 7.0 * es;
    let c = pos2(tl.x - 2.2 * es + d / 2.0, tl.y - 2.2 * es + d / 2.0);
    let bw = (0.9 * es).max(1.5);
    p.circle_filled(c, d / 2.0, fade(Color32::BLACK, a));
    p.circle_filled(c, d / 2.0 - bw, fade(bg, a));
    let fg = fade(fg, a);
    match glyph {
        "dots" => { let op = 0.35 + 0.65 * (t * std::f32::consts::TAU).sin().abs(); for k in [-1.0f32, 0.0, 1.0] { p.circle_filled(pos2(c.x + k * 1.2 * es, c.y), 0.42 * es, fade(fg, op)); } }
        "check" => { let u = 4.0 * es / 10.0; let o = pos2(c.x - 2.0 * es, c.y - 2.0 * es); p.add(Shape::line(vec![pos2(o.x + 2.0 * u, o.y + 5.2 * u), pos2(o.x + 4.2 * u, o.y + 7.4 * u), pos2(o.x + 8.0 * u, o.y + 3.0 * u)], Stroke::new(2.0 * u, fg))); }
        "x" => { let u = 3.6 * es / 10.0; let o = pos2(c.x - 1.8 * es, c.y - 1.8 * es); p.line_segment([pos2(o.x + 2.5 * u, o.y + 2.5 * u), pos2(o.x + 7.5 * u, o.y + 7.5 * u)], Stroke::new(2.0 * u, fg)); p.line_segment([pos2(o.x + 7.5 * u, o.y + 2.5 * u), pos2(o.x + 2.5 * u, o.y + 7.5 * u)], Stroke::new(2.0 * u, fg)); }
        "note" => { p.text(c, egui::Align2::CENTER_CENTER, "♪", egui::FontId::new(4.4 * es, egui::FontFamily::Name("bold".into())), fg); }
        "rec" => { let op = 0.3 + 0.7 * (t * std::f32::consts::TAU / 0.9).cos().abs(); p.circle_filled(c, 1.3 * es, fade(Color32::WHITE, op * a)); }
        g => { p.text(c, egui::Align2::CENTER_CENTER, g, egui::FontId::new(4.4 * es, egui::FontFamily::Name("bold".into())), fg); }
    }
}

fn quad(a: Pos2, c: Pos2, b: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n).map(|i| { let t = i as f32 / n as f32; let u = 1.0 - t; pos2(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y) }).collect()
}

fn rotated_rr(r: Rect, rad: f32, ang: f32, col: Color32) -> Shape {
    let c = r.center();
    let mut pts = vec![];
    for (cx, cy, st) in [(r.right() - rad, r.bottom() - rad, 0.0f32), (r.left() + rad, r.bottom() - rad, 90.0), (r.left() + rad, r.top() + rad, 180.0), (r.right() - rad, r.top() + rad, 270.0)] {
        for i in 0..=6 { let a = (st + i as f32 * 15.0).to_radians(); pts.push(pos2(cx + rad * a.cos(), cy + rad * a.sin())); }
    }
    let (sn, cs) = ang.sin_cos();
    let pts = pts.into_iter().map(|p| { let d = p - c; pos2(c.x + d.x * cs - d.y * sn, c.y + d.x * sn + d.y * cs) }).collect();
    Shape::convex_polygon(pts, col, Stroke::NONE)
}

/// Polígono simples (côncavo ou não) preenchido por "ear clipping".
fn fill_poly(p: &egui::Painter, pts: &[Pos2], col: Color32) {
    let n = pts.len();
    if n < 3 { return; }
    let area: f32 = (0..n).map(|i| { let (a, b) = (pts[i], pts[(i + 1) % n]); a.x * b.y - b.x * a.y }).sum();
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 { idx.reverse(); }
    let mut mesh = egui::Mesh::default();
    for q in pts { mesh.colored_vertex(*q, col); }
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let inside = |p: Pos2, a: Pos2, b: Pos2, c: Pos2| cross(a, b, p) > 0.0 && cross(b, c, p) > 0.0 && cross(c, a, p) > 0.0;
    let mut guard = 0;
    while idx.len() > 3 && guard < 500 {
        guard += 1;
        let m = idx.len();
        let mut cut = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 0.0 { continue; }
            if idx.iter().any(|&j| j != ia && j != ib && j != ic && inside(pts[j], a, b, c)) { continue; }
            mesh.add_triangle(ia as u32, ib as u32, ic as u32);
            idx.remove(i);
            cut = true;
            break;
        }
        if !cut { break; }
    }
    if idx.len() == 3 { mesh.add_triangle(idx[0] as u32, idx[1] as u32, idx[2] as u32); }
    p.add(Shape::mesh(mesh));
}

/// Roupas desenhadas no mesmo quadro 80×64 do original (corpo do tofu em x 14–66, y 16–58).
fn draw_outfit(p: &egui::Painter, body_tl: Pos2, s: f32, o: Outfit, a: f32) {
    let u = s / 10.0;
    let o0 = pos2(body_tl.x - 14.0 * u, body_tl.y - 16.0 * u);
    let m = |x: f32, y: f32| pos2(o0.x + x * u, o0.y + y * u);
    let rr = |x: f32, y: f32, w: f32, h: f32, r: f32, c: u32| p.rect_filled(Rect::from_min_size(m(x, y), vec2(w * u, h * u)), CornerRadius::same((r * u) as u8), fade(hex(c), a));
    let circle = |x: f32, y: f32, r: f32, c: u32| p.circle_filled(m(x, y), r * u, fade(hex(c), a));
    let dome = |x0: f32, y0: f32, cx: f32, cy: f32, x1: f32, y1: f32, c: u32| { let pts = quad(m(x0, y0), m(cx, cy), m(x1, y1), 16); fill_poly(p, &pts, fade(hex(c), a)); };
    match o {
        Outfit::Helmet => { dome(12.0, 22.0, 40.0, -4.0, 68.0, 22.0, 0xF5B82E); rr(8.0, 20.0, 64.0, 6.0, 3.0, 0xE0A11B); rr(37.0, 4.0, 6.0, 16.0, 3.0, 0xE0A11B); }
        Outfit::Headphones => {
            let mut pts = quad(m(14.0, 32.0), m(14.0, 6.0), m(40.0, 6.0), 12);
            pts.extend(quad(m(40.0, 6.0), m(66.0, 6.0), m(66.0, 32.0), 12));
            p.add(Shape::line(pts, Stroke::new(4.0 * u, fade(hex(0x333333), a))));
            rr(7.0, 28.0, 11.0, 16.0, 5.0, 0xE8508F); rr(62.0, 28.0, 11.0, 16.0, 5.0, 0xE8508F);
        }
        Outfit::Sunglasses => { rr(22.0, 31.0, 16.0, 10.0, 4.0, 0x111111); rr(42.0, 31.0, 16.0, 10.0, 4.0, 0x111111); rr(37.0, 34.0, 6.0, 2.5, 0.0, 0x111111); rr(25.0, 33.0, 5.0, 2.0, 1.0, 0x555555); }
        Outfit::Santa => {
            let mut pts = quad(m(16.0, 20.0), m(34.0, -6.0), m(62.0, 12.0), 16); pts.push(m(66.0, 20.0));
            fill_poly(p, &pts, fade(hex(0xD9363E), a));
            rr(12.0, 17.0, 58.0, 7.0, 3.5, 0xFFFFFF); circle(64.0, 10.0, 5.0, 0xFFFFFF);
        }
        Outfit::Cap => { dome(16.0, 22.0, 40.0, 2.0, 64.0, 22.0, 0x3A6FD8); rr(40.0, 18.0, 34.0, 6.0, 3.0, 0x2B57B0); circle(40.0, 9.0, 2.5, 0x2B57B0); }
        Outfit::Bow => {
            fill_poly(p, &[m(40.0, 16.0), m(26.0, 8.0), m(26.0, 24.0)], fade(hex(0xF06AA8), a));
            fill_poly(p, &[m(40.0, 16.0), m(54.0, 8.0), m(54.0, 24.0)], fade(hex(0xF06AA8), a));
            circle(40.0, 16.0, 4.0, 0xD84F90);
        }
        Outfit::Scarf => {
            rr(12.0, 48.0, 56.0, 9.0, 4.0, 0x2BA88A); rr(50.0, 50.0, 9.0, 13.0, 3.0, 0x228E74);
            p.rect_filled(Rect::from_min_size(m(18.0, 50.0), vec2(4.0 * u, 5.0 * u)), CornerRadius::ZERO, fade(Color32::from_white_alpha(153), a));
            p.rect_filled(Rect::from_min_size(m(28.0, 50.0), vec2(4.0 * u, 5.0 * u)), CornerRadius::ZERO, fade(Color32::from_white_alpha(153), a));
        }
        Outfit::Crown => {
            fill_poly(p, &[m(20.0, 20.0), m(24.0, 6.0), m(32.0, 14.0), m(40.0, 4.0), m(48.0, 14.0), m(56.0, 6.0), m(60.0, 20.0)], fade(hex(0xF2C744), a));
            rr(20.0, 18.0, 40.0, 5.0, 2.0, 0xD9AE2C); circle(40.0, 12.0, 2.5, 0xE2483D);
        }
        Outfit::None => {}
    }
}
