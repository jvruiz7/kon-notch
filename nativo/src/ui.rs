//! Interface do Kon nativo: o notch compacto, o painel e as páginas.

use crate::media::{Media, MediaCmd, MediaState};
use crate::settings::Settings;
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WIN_W: f32 = 820.0;
const COMPACT: Vec2 = Vec2::new(196.0, 34.0);
const COMPACT_MUSIC: Vec2 = Vec2::new(300.0, 34.0);
const PANEL: Vec2 = Vec2::new(660.0, 278.0);
const BG: Color32 = Color32::from_rgb(0, 0, 0);
const CARD: Color32 = Color32::from_rgb(20, 20, 22);
const DIM: Color32 = Color32::from_rgba_premultiplied(140, 140, 148, 255);
const DAYS: [&str; 7] = ["DOM", "SEG", "TER", "QUA", "QUI", "SEX", "SÁB"];
const MONTHS: [&str; 12] = ["JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum View { Compact, Panel }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Home, Music, Focus, Settings }

impl Page {
    fn id(self) -> &'static str { match self { Page::Home => "home", Page::Music => "music", Page::Focus => "focus", Page::Settings => "settings" } }
    fn from(s: &str) -> Page { match s { "music" => Page::Music, "focus" => Page::Focus, _ => Page::Home } }
    fn label(self) -> &'static str { match self { Page::Home => "INÍCIO", Page::Music => "MÚSICA", Page::Focus => "FOCO", Page::Settings => "AJUSTES" } }
}

/// Mola criticamente amortecida: movimento suave, sem passar do ponto.
#[derive(Clone, Copy)]
struct Spring { v: f32, vel: f32 }
impl Spring {
    fn new(v: f32) -> Self { Spring { v, vel: 0.0 } }
    fn step(&mut self, target: f32, dt: f32, stiffness: f32) -> bool {
        let w = stiffness; // frequência
        let x = self.v - target;
        let a = -w * w * x - 2.0 * w * self.vel;
        self.vel += a * dt;
        self.v += self.vel * dt;
        if (self.v - target).abs() < 0.3 && self.vel.abs() < 2.0 { self.v = target; self.vel = 0.0; false } else { true }
    }
}

/// Estado compartilhado com a thread que vigia o cursor (a janela é "fantasma" quando o mouse está fora do notch).
#[derive(Default)]
struct Hit { rect: Option<Rect>, ppp: f32, inside: bool }

struct Focus { total: u32, left: f32, running_until: Option<Instant>, finished_at: Option<Instant> }

pub struct KonApp {
    settings: Settings,
    media: Media,
    view: View,
    page: Page,
    w: Spring, h: Spring,
    hit: Arc<Mutex<Hit>>,
    passthrough: bool,
    hover_since: Option<Instant>,
    leave_since: Option<Instant>,
    placed: bool,
    last_frame: Instant,
    blink_at: Instant,
    focus: Focus,
    _tray: Option<tray::Tray>,
    quit: bool,
}

impl KonApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let settings = Settings::load();
        let ctx = cc.egui_ctx.clone();
        let media = Media::start(move || ctx.request_repaint());
        let hit = Arc::new(Mutex::new(Hit { ppp: 1.0, ..Default::default() }));
        spawn_cursor_watch(cc.egui_ctx.clone(), hit.clone());
        let mins = settings.focus_minutes.clamp(1, 180);
        KonApp {
            page: Page::from(&std::env::var("KON_PAGE").unwrap_or_else(|_| settings.page.clone())),
            settings, media,
            // KON_DEMO=painel abre já com o painel (só para capturas de teste)
            view: if std::env::var("KON_DEMO").as_deref() == Ok("painel") { View::Panel } else { View::Compact },
            w: Spring::new(COMPACT.x), h: Spring::new(COMPACT.y),
            hit, passthrough: true,
            hover_since: None, leave_since: None,
            placed: false,
            last_frame: Instant::now(),
            blink_at: Instant::now() + Duration::from_millis(3800),
            focus: Focus { total: mins * 60, left: (mins * 60) as f32, running_until: None, finished_at: None },
            _tray: tray::Tray::new(cc.egui_ctx.clone()),
            quit: false,
        }
    }

    fn target_size(&self, m: &MediaState) -> Vec2 {
        match self.view {
            View::Panel => PANEL,
            View::Compact => if m.active && m.playing { COMPACT_MUSIC } else { COMPACT },
        }
    }

    fn set_page(&mut self, p: Page) {
        self.page = p;
        if p != Page::Settings { self.settings.page = p.id().into(); self.settings.save(); }
    }

    fn focus_tick(&mut self) {
        if let Some(until) = self.focus.running_until {
            let left = until.saturating_duration_since(Instant::now()).as_secs_f32();
            self.focus.left = left;
            if left <= 0.0 { self.focus.running_until = None; self.focus.left = self.focus.total as f32; self.focus.finished_at = Some(Instant::now()); }
        }
    }
    fn focus_toggle(&mut self) {
        if self.focus.running_until.is_some() { self.focus.running_until = None; }
        else { self.focus.finished_at = None; self.focus.running_until = Some(Instant::now() + Duration::from_secs_f32(self.focus.left.max(1.0))); }
    }
}

impl eframe::App for KonApp {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] { [0.0, 0.0, 0.0, 0.0] }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        // 1º quadro: centraliza no topo do monitor e tira da barra de tarefas
        if !self.placed {
            if let Some(ms) = ctx.input(|i| i.viewport().monitor_size) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos2(((ms.x - WIN_W) / 2.0).round(), 0.0)));
                crate::platform::make_tool_window();
                self.placed = true;
            }
        }
        if let Some(t) = &self._tray { for a in t.poll() { match a { tray::Action::Open => { self.view = View::Panel; } tray::Action::Quit => { self.quit = true; } } } }
        if self.quit { ctx.send_viewport_cmd(egui::ViewportCommand::Close); return; }

        let m = self.media.snapshot();
        self.focus_tick();

        // ── cursor: dentro ou fora do notch? ──
        let island = self.island_rect();
        let inside = {
            let mut h = self.hit.lock().unwrap();
            h.rect = Some(island.expand2(vec2(6.0, 4.0)));
            h.ppp = ctx.pixels_per_point();
            if cfg!(windows) { h.inside } else { ctx.input(|i| i.pointer.hover_pos()).map_or(false, |p| island.contains(p)) }
        };
        if self.passthrough == inside {
            self.passthrough = !inside;
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.passthrough));
        }
        // abrir ao passar o mouse (com um respiro de 90 ms) e fechar ao sair (280 ms de tolerância)
        if inside {
            self.leave_since = None;
            if self.view == View::Compact && self.settings.open_on_hover {
                let t = *self.hover_since.get_or_insert(now);
                if now - t > Duration::from_millis(90) { self.view = View::Panel; }
            }
        } else {
            self.hover_since = None;
            if self.view == View::Panel && std::env::var("KON_DEMO").is_err() {
                let t = *self.leave_since.get_or_insert(now);
                if now - t > Duration::from_millis(280) { self.view = View::Compact; if self.page == Page::Settings { self.page = Page::from(&self.settings.page); } }
            }
        }

        // ── animação do tamanho ──
        let target = self.target_size(&m);
        let a = self.w.step(target.x, dt, 22.0);
        let b = self.h.step(target.y, dt, 22.0);
        let animating = a || b;

        // ── desenho ──
        let blink = now >= self.blink_at;
        if now > self.blink_at + Duration::from_millis(140) { self.blink_at = now + Duration::from_millis(3600 + (now.elapsed().subsec_millis() as u64 % 1400)); }

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let rect = self.island_rect();
            draw_island(ui.painter(), rect, self.view);
            let open = ((self.h.v - COMPACT.y) / (PANEL.y - COMPACT.y)).clamp(0.0, 1.0);
            if open < 0.35 {
                self.draw_compact(ui, rect, &m, blink, 1.0 - open / 0.35);
            }
            if open > 0.55 {
                let alpha = ((open - 0.55) / 0.45).clamp(0.0, 1.0);
                self.draw_panel(ui, rect, &m, blink, alpha);
            }
        });

        // ── quando redesenhar de novo (a janela é transparente: cada quadro custa) ──
        if animating {
            ctx.request_repaint();
        } else if self.view == View::Panel {
            ctx.request_repaint_after(Duration::from_millis(if self.focus.running_until.is_some() { 250 } else { 1000 }));
        } else if m.playing {
            ctx.request_repaint_after(Duration::from_millis(100)); // barras de música a 10 quadros/s
        } else {
            let to_blink = self.blink_at.saturating_duration_since(now);
            ctx.request_repaint_after(to_blink.max(Duration::from_millis(16)).min(Duration::from_secs(5)));
        }
    }
}

impl KonApp {
    fn island_rect(&self) -> Rect {
        let w = self.w.v; let h = self.h.v;
        Rect::from_min_size(pos2((WIN_W - w) / 2.0, 0.0), vec2(w, h))
    }

    fn draw_compact(&mut self, ui: &mut egui::Ui, r: Rect, m: &MediaState, blink: bool, alpha: f32) {
        let p = ui.painter();
        draw_tofu(p, r.center(), 0.85, blink, alpha);
        if m.active && m.playing && r.width() > COMPACT.x + 20.0 {
            // capa (cor) à esquerda e barras à direita
            let art = Rect::from_center_size(pos2(r.left() + 22.0, r.center().y), vec2(20.0, 20.0));
            p.rect_filled(art, CornerRadius::same(5), fade(Color32::from_rgb(214, 58, 134), alpha));
            p.text(art.center(), Align2::CENTER_CENTER, "♫", FontId::proportional(11.0), fade(Color32::WHITE, alpha));
            draw_bars(p, pos2(r.right() - 40.0, r.center().y), alpha);
        }
    }

    fn draw_panel(&mut self, ui: &mut egui::Ui, r: Rect, m: &MediaState, blink: bool, alpha: f32) {
        let now = chrono::Local::now();
        use chrono::{Datelike, Timelike};
        let p = ui.painter().clone();
        // cabeçalho
        let hy = r.top() + 22.0;
        let date = format!("{} {:02} {}", DAYS[now.weekday().num_days_from_sunday() as usize], now.day(), MONTHS[now.month0() as usize]);
        p.text(pos2(r.left() + 22.0, hy), Align2::LEFT_CENTER, date, bold(11.0), fade(DIM, alpha));
        p.text(pos2(r.right() - 22.0, hy), Align2::RIGHT_CENTER, format!("{:02}:{:02}", now.hour(), now.minute()), bold(12.0), fade(Color32::WHITE, alpha));
        draw_tofu(&p, pos2(r.center().x, hy), 0.95, blink, alpha);

        let body = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 42.0), pos2(r.right() - 16.0, r.bottom() - 50.0));
        match self.page {
            Page::Home => self.page_home(ui, body, m, alpha),
            Page::Music => self.page_music(ui, body, m, alpha),
            Page::Focus => self.page_focus(ui, body, alpha),
            Page::Settings => self.page_settings(ui, body, alpha),
        }
        self.nav(ui, Rect::from_min_max(pos2(r.left() + 16.0, r.bottom() - 46.0), pos2(r.right() - 16.0, r.bottom() - 6.0)), alpha);
    }

    fn nav(&mut self, ui: &mut egui::Ui, bar: Rect, alpha: f32) {
        let pages = [(Page::Home, Icon::Grid), (Page::Music, Icon::Note), (Page::Focus, Icon::Timer)];
        let mut x = bar.center().x - 150.0;
        for (pg, icon) in pages {
            let active = self.page == pg;
            let w = if active { 104.0 } else { 34.0 };
            let rr = Rect::from_min_size(pos2(x, bar.center().y - 14.0), vec2(w, 28.0));
            let resp = ui.interact(rr, ui.id().with(pg.id()), Sense::click());
            let p = ui.painter();
            if active { p.rect_filled(rr, CornerRadius::same(14), fade(Color32::from_white_alpha(28), alpha)); }
            else if resp.hovered() { p.rect_filled(rr, CornerRadius::same(14), fade(Color32::from_white_alpha(12), alpha)); }
            let col = if active { Color32::WHITE } else { Color32::from_gray(150) };
            if active {
                draw_icon(p, icon, pos2(rr.left() + 18.0, rr.center().y), fade(col, alpha));
                p.text(pos2(rr.left() + 32.0, rr.center().y), Align2::LEFT_CENTER, pg.label(), bold(10.5), fade(col, alpha));
            } else {
                draw_icon(p, icon, rr.center(), fade(col, alpha));
            }
            if resp.clicked() { self.set_page(pg); }
            x += w + 4.0;
        }
        // engrenagem à direita
        let g = Rect::from_center_size(pos2(bar.right() - 16.0, bar.center().y), vec2(28.0, 28.0));
        let resp = ui.interact(g, ui.id().with("gear"), Sense::click());
        let p = ui.painter();
        if resp.hovered() || self.page == Page::Settings { p.rect_filled(g, CornerRadius::same(14), fade(Color32::from_white_alpha(20), alpha)); }
        p.text(g.center(), Align2::CENTER_CENTER, "⚙", FontId::proportional(14.0), fade(Color32::from_gray(170), alpha));
        if resp.clicked() { self.page = if self.page == Page::Settings { Page::from(&self.settings.page) } else { Page::Settings }; }
    }

    fn page_home(&mut self, ui: &mut egui::Ui, body: Rect, m: &MediaState, alpha: f32) {
        let gap = 10.0;
        let total = body.width() - 2.0 * gap;
        let w1 = total * 1.0 / 3.55; let w2 = total * 1.55 / 3.55;
        let c1 = Rect::from_min_size(body.min, vec2(w1, body.height()));
        let c2 = Rect::from_min_size(pos2(c1.right() + gap, body.top()), vec2(w2, body.height()));
        let c3 = Rect::from_min_max(pos2(c2.right() + gap, body.top()), body.max);
        let p = ui.painter().clone();
        // cartão de música
        card(&p, c2, Color32::from_rgb(58, 28, 110), alpha);
        p.text(pos2(c2.left() + 14.0, c2.bottom() - 34.0), Align2::LEFT_BOTTOM, ellipsize(if m.active { &m.title } else { "Nada tocando" }, 26), bold(16.0), fade(Color32::WHITE, alpha));
        p.text(pos2(c2.left() + 14.0, c2.bottom() - 14.0), Align2::LEFT_BOTTOM, ellipsize(if m.active { &m.artist } else { "Toque música em qualquer app" }, 34), FontId::proportional(12.0), fade(DIM, alpha));
        if round_button(ui, &p, pos2(c2.right() - 30.0, c2.bottom() - 30.0), 17.0, if m.playing { "⏸" } else { "▶" }, alpha, true).clicked() { self.media.send(MediaCmd::Toggle); }
        // foco
        card(&p, c3, Color32::from_rgb(40, 18, 60), alpha);
        p.text(pos2(c3.left() + 14.0, c3.top() + 16.0), Align2::LEFT_CENTER, "FOCO", bold(10.0), fade(Color32::from_rgb(216, 180, 254), alpha));
        p.text(pos2(c3.left() + 14.0, c3.bottom() - 34.0), Align2::LEFT_BOTTOM, mmss(self.focus.left), FontId::monospace(24.0), fade(Color32::WHITE, alpha));
        let st = if self.focus.running_until.is_some() { "Rodando" } else if self.focus.finished_at.is_some() { "Concluído!" } else if self.focus.left < self.focus.total as f32 { "Pausado" } else { "Pronto" };
        p.text(pos2(c3.left() + 14.0, c3.bottom() - 14.0), Align2::LEFT_BOTTOM, st, FontId::proportional(12.0), fade(DIM, alpha));
        if round_button(ui, &p, pos2(c3.right() - 24.0, c3.bottom() - 24.0), 13.0, if self.focus.running_until.is_some() { "⏸" } else { "▶" }, alpha, false).clicked() { self.focus_toggle(); }
        // Claude Code (chega na próxima fase)
        card(&p, c1, Color32::from_rgb(45, 28, 8), alpha);
        p.text(pos2(c1.left() + 14.0, c1.top() + 16.0), Align2::LEFT_CENTER, "CLAUDE", bold(10.0), fade(Color32::from_rgb(253, 230, 138), alpha));
        p.text(pos2(c1.left() + 14.0, c1.bottom() - 34.0), Align2::LEFT_BOTTOM, "0:00", FontId::monospace(24.0), fade(Color32::WHITE, alpha));
        p.text(pos2(c1.left() + 14.0, c1.bottom() - 14.0), Align2::LEFT_BOTTOM, "Parado", FontId::proportional(12.0), fade(DIM, alpha));
    }

    fn page_music(&mut self, ui: &mut egui::Ui, body: Rect, m: &MediaState, alpha: f32) {
        let p = ui.painter().clone();
        let art = Rect::from_min_size(pos2(body.left() + 6.0, body.center().y - 62.0), vec2(124.0, 124.0));
        p.rect_filled(art, CornerRadius::same(18), fade(Color32::from_rgb(120, 40, 140), alpha));
        p.text(art.center(), Align2::CENTER_CENTER, "♫", FontId::proportional(44.0), fade(Color32::from_white_alpha(170), alpha));
        let x = art.right() + 22.0; let right = body.right() - 8.0;
        p.text(pos2(x, art.top() + 10.0), Align2::LEFT_TOP, ellipsize(if m.active { &m.title } else { "Nada tocando" }, 36), bold(19.0), fade(Color32::WHITE, alpha));
        p.text(pos2(x, art.top() + 36.0), Align2::LEFT_TOP, ellipsize(if m.active { &m.artist } else { "Toque música em qualquer app" }, 44), FontId::proportional(12.5), fade(DIM, alpha));
        // progresso
        let py = art.top() + 70.0;
        let track = Rect::from_min_max(pos2(x, py), pos2(right, py + 4.0));
        p.rect_filled(track, CornerRadius::same(2), fade(Color32::from_white_alpha(45), alpha));
        if m.duration > 0.0 {
            let f = (m.position / m.duration).clamp(0.0, 1.0) as f32;
            p.rect_filled(Rect::from_min_max(track.min, pos2(track.left() + track.width() * f, track.bottom())), CornerRadius::same(2), fade(Color32::WHITE, alpha));
            p.text(pos2(x, py + 14.0), Align2::LEFT_CENTER, mmss(m.position as f32), FontId::monospace(10.5), fade(DIM, alpha));
            p.text(pos2(right, py + 14.0), Align2::RIGHT_CENTER, format!("-{}", mmss((m.duration - m.position) as f32)), FontId::monospace(10.5), fade(DIM, alpha));
        }
        let cy = art.bottom() - 16.0; let cx = (x + right) / 2.0;
        if icon_button(ui, &p, pos2(cx - 56.0, cy), "⏮", alpha).clicked() { self.media.send(MediaCmd::Prev); }
        if round_button(ui, &p, pos2(cx, cy), 19.0, if m.playing { "⏸" } else { "▶" }, alpha, true).clicked() { self.media.send(MediaCmd::Toggle); }
        if icon_button(ui, &p, pos2(cx + 56.0, cy), "⏭", alpha).clicked() { self.media.send(MediaCmd::Next); }
    }

    fn page_focus(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        p.text(pos2(body.left() + 8.0, body.top() + 30.0), Align2::LEFT_CENTER, mmss(self.focus.left), FontId::monospace(42.0), fade(Color32::WHITE, alpha));
        let running = self.focus.running_until.is_some();
        let sub = if running {
            let end = chrono::Local::now() + chrono::Duration::seconds(self.focus.left as i64);
            format!("Termina às {}", end.format("%H:%M"))
        } else if self.focus.finished_at.is_some() { "Sessão concluída. Bom trabalho!".to_string() } else { format!("Sessão de {} minutos", self.focus.total / 60) };
        p.text(pos2(body.left() + 8.0, body.top() + 64.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.0), fade(DIM, alpha));
        // botões
        let by = body.top() + 26.0;
        if pill(ui, &p, Rect::from_min_size(pos2(body.right() - 118.0, by - 15.0), vec2(110.0, 30.0)), if running { "Pausar" } else { "Começar" }, true, alpha).clicked() { self.focus_toggle(); }
        if pill(ui, &p, Rect::from_min_size(pos2(body.right() - 196.0, by - 15.0), vec2(70.0, 30.0)), "Zerar", false, alpha).clicked() {
            self.focus.running_until = None; self.focus.left = self.focus.total as f32; self.focus.finished_at = None;
        }
        if pill(ui, &p, Rect::from_min_size(pos2(body.right() - 274.0, by - 15.0), vec2(70.0, 30.0)), "+5 min", false, alpha).clicked() {
            self.focus.left += 300.0; self.focus.total += 300;
            if let Some(u) = self.focus.running_until { self.focus.running_until = Some(u + Duration::from_secs(300)); }
        }
        // escolha da duração
        let opts = [15u32, 25, 45, 60];
        let mut x = body.left() + 8.0; let y = body.bottom() - 30.0;
        p.text(pos2(x, y - 26.0), Align2::LEFT_CENTER, "DURAÇÃO", bold(10.0), fade(DIM, alpha));
        for o in opts {
            let rr = Rect::from_min_size(pos2(x, y - 14.0), vec2(64.0, 28.0));
            let on = self.focus.total / 60 == o && !running;
            if pill(ui, &p, rr, &format!("{o} min"), on, alpha).clicked() && !running {
                self.focus.total = o * 60; self.focus.left = (o * 60) as f32; self.focus.finished_at = None;
                self.settings.focus_minutes = o; self.settings.save();
            }
            x += 70.0;
        }
    }

    fn page_settings(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        p.text(pos2(body.left() + 6.0, body.top() + 12.0), Align2::LEFT_CENTER, "Ajustes", bold(16.0), fade(Color32::WHITE, alpha));
        p.text(pos2(body.left() + 6.0, body.top() + 32.0), Align2::LEFT_CENTER, "Kon nativo (Rust) · versão de testes", FontId::proportional(11.5), fade(DIM, alpha));
        let row = Rect::from_min_size(pos2(body.left(), body.top() + 52.0), vec2(body.width(), 44.0));
        card(&p, row, CARD, alpha);
        p.text(pos2(row.left() + 14.0, row.center().y), Align2::LEFT_CENTER, "Abrir o painel ao passar o mouse", FontId::proportional(13.0), fade(Color32::WHITE, alpha));
        if toggle(ui, &p, pos2(row.right() - 34.0, row.center().y), self.settings.open_on_hover, alpha).clicked() {
            self.settings.open_on_hover = !self.settings.open_on_hover; self.settings.save();
        }
        p.text(pos2(body.left() + 6.0, row.bottom() + 20.0), Align2::LEFT_CENTER, "Sem o hover, abra pelo ícone perto do relógio.", FontId::proportional(11.0), fade(DIM, alpha));
    }
}

/* ───────── desenho ───────── */

fn fade(c: Color32, a: f32) -> Color32 { c.gamma_multiply(a.clamp(0.0, 1.0)) }
fn bold(size: f32) -> FontId { FontId::new(size, FontFamily::Name("bold".into())) }
fn mmss(s: f32) -> String { let s = s.max(0.0).round() as u32; format!("{}:{:02}", s / 60, s % 60) }
fn ellipsize(s: &str, n: usize) -> String { if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" } }

fn card(p: &egui::Painter, r: Rect, c: Color32, a: f32) {
    p.rect_filled(r, CornerRadius::same(16), fade(c, a));
    p.rect_stroke(r, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(18), a)), egui::StrokeKind::Inside);
}

/// A ilha: retângulo preto com cantos de baixo arredondados e "orelhas" côncavas no topo, como o notch de verdade.
fn draw_island(p: &egui::Painter, r: Rect, view: View) {
    let rad = if view == View::Panel { (r.height() / 10.0).clamp(14.0, 28.0) } else { (r.height() / 2.0).min(17.0) } as u8;
    p.rect_filled(r, CornerRadius { nw: 0, ne: 0, sw: rad, se: rad }, BG);
    let ear = 8.0;
    for side in [-1.0f32, 1.0] {
        let corner = if side < 0.0 { r.left_top() } else { r.right_top() };
        let c = pos2(corner.x + side * ear, corner.y + ear); // centro do arco, fora da ilha
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(corner, BG); // leque a partir do canto
        let n = 10;
        for i in 0..=n {
            let t = i as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
            mesh.colored_vertex(pos2(c.x - side * ear * t.sin(), c.y - ear * t.cos()), BG);
        }
        for i in 1..=n as u32 { mesh.add_triangle(0, i, i + 1); }
        p.add(egui::Shape::mesh(mesh));
    }
}

#[derive(Clone, Copy)]
enum Icon { Grid, Note, Timer }

/// Ícones desenhados com formas (não dependem de fonte).
fn draw_icon(p: &egui::Painter, i: Icon, c: Pos2, col: Color32) {
    let st = Stroke::new(1.6_f32, col);
    match i {
        Icon::Grid => for (dx, dy) in [(-3.2, -3.2), (3.2, -3.2), (-3.2, 3.2), (3.2, 3.2)] {
            p.rect_stroke(Rect::from_center_size(pos2(c.x + dx, c.y + dy), vec2(4.6, 4.6)), CornerRadius::same(1), st, egui::StrokeKind::Middle);
        },
        Icon::Note => { p.text(c, Align2::CENTER_CENTER, "♫", FontId::proportional(14.0), col); }
        Icon::Timer => {
            p.circle_stroke(pos2(c.x, c.y + 0.8), 5.6, st);
            p.line_segment([pos2(c.x, c.y + 0.8), pos2(c.x, c.y - 2.6)], st);
            p.line_segment([pos2(c.x - 1.8, c.y - 6.6), pos2(c.x + 1.8, c.y - 6.6)], st);
        }
    }
}

/// O mascote: tofu branco com dois olhos (piscam por tempo, não sem parar).
fn draw_tofu(p: &egui::Painter, c: Pos2, s: f32, blink: bool, a: f32) {
    let body = Rect::from_center_size(c, vec2(26.0 * s, 21.0 * s));
    p.rect_filled(body, CornerRadius::same((7.75 * s) as u8), fade(Color32::from_rgb(250, 250, 248), a));
    let eh = if blink { 0.6 * s } else { 4.1 * s };
    for dx in [-5.0f32, 5.0] {
        let e = Rect::from_center_size(pos2(c.x + dx * s, c.y), vec2(3.4 * s, eh));
        p.rect_filled(e, CornerRadius::same((1.7 * s) as u8), fade(Color32::from_rgb(17, 17, 17), a));
    }
}

/// Barras de música: mudam a cada ~100 ms (10 quadros/s), suficiente para parecer viva sem pesar.
fn draw_bars(p: &egui::Painter, c: Pos2, a: f32) {
    let seqs: [[f32; 5]; 6] = [[3., 9., 5., 12., 3.], [6., 14., 8., 5., 6.], [10., 5., 15., 7., 10.], [5., 12., 6., 14., 5.], [12., 7., 10., 4., 12.], [4., 10., 13., 6., 4.]];
    let n = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() / 100) as usize;
    for (i, s) in seqs.iter().enumerate() {
        let h = s[(n + i) % 5];
        let x = c.x + i as f32 * 5.0;
        p.rect_filled(Rect::from_center_size(pos2(x, c.y), vec2(2.6, h)), CornerRadius::same(1), fade(Color32::from_rgb(255, 122, 184), a));
    }
}

fn round_button(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, r: f32, glyph: &str, a: f32, primary: bool) -> egui::Response {
    let rect = Rect::from_center_size(c, vec2(r * 2.0, r * 2.0));
    let resp = ui.interact(rect, ui.id().with(("rb", (c.x as i32, c.y as i32))), Sense::click());
    let bg = if primary { Color32::WHITE } else { Color32::from_white_alpha(if resp.hovered() { 40 } else { 26 }) };
    p.circle_filled(c, r * if resp.is_pointer_button_down_on() { 0.92 } else { 1.0 }, fade(bg, a));
    p.text(c, Align2::CENTER_CENTER, glyph, FontId::proportional(r * 0.85), fade(if primary { Color32::BLACK } else { Color32::WHITE }, a));
    resp
}

fn icon_button(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, glyph: &str, a: f32) -> egui::Response {
    let rect = Rect::from_center_size(c, vec2(32.0, 32.0));
    let resp = ui.interact(rect, ui.id().with(("ib", (c.x as i32, c.y as i32))), Sense::click());
    if resp.hovered() { p.circle_filled(c, 16.0, fade(Color32::from_white_alpha(18), a)); }
    p.text(c, Align2::CENTER_CENTER, glyph, FontId::proportional(18.0), fade(Color32::WHITE, a));
    resp
}

fn pill(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, label: &str, on: bool, a: f32) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(("pill", (r.min.x as i32, r.min.y as i32))), Sense::click());
    let bg = if on { Color32::WHITE } else { Color32::from_white_alpha(if resp.hovered() { 34 } else { 22 }) };
    p.rect_filled(r, CornerRadius::same((r.height() / 2.0) as u8), fade(bg, a));
    p.text(r.center(), Align2::CENTER_CENTER, label, bold(12.0), fade(if on { Color32::BLACK } else { Color32::WHITE }, a));
    resp
}

fn toggle(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, on: bool, a: f32) -> egui::Response {
    let r = Rect::from_center_size(c, vec2(40.0, 22.0));
    let resp = ui.interact(r, ui.id().with(("tg", (c.x as i32, c.y as i32))), Sense::click());
    p.rect_filled(r, CornerRadius::same(11), fade(if on { Color32::from_rgb(34, 197, 94) } else { Color32::from_white_alpha(40) }, a));
    let kx = if on { r.right() - 11.0 } else { r.left() + 11.0 };
    p.circle_filled(pos2(kx, c.y), 8.5, fade(Color32::WHITE, a));
    resp
}

/// Mesmas fontes do Kon atual: no Windows, Segoe UI Variable / Segoe UI e Cascadia Mono (lidas da pasta de
/// fontes do sistema, não embutidas). Fora do Windows, ou se faltarem, usa a Inter embutida.
fn install_fonts(ctx: &egui::Context) {
    fn sys(names: &[&str]) -> Option<Vec<u8>> {
        let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from)?.join("Fonts");
        names.iter().find_map(|n| std::fs::read(dir.join(n)).ok())
    }
    let mut f = egui::FontDefinitions::default();
    let regular = sys(&["SegUIVar.ttf", "segoeui.ttf"]).map(egui::FontData::from_owned)
        .unwrap_or_else(|| egui::FontData::from_static(include_bytes!("../assets/Inter-Medium.otf")));
    let semibold = sys(&["seguisb.ttf", "segoeuib.ttf"]).map(egui::FontData::from_owned)
        .unwrap_or_else(|| egui::FontData::from_static(include_bytes!("../assets/Inter-Bold.otf")));
    f.font_data.insert("kon".into(), Arc::new(regular));
    f.font_data.insert("kon-bold".into(), Arc::new(semibold));
    f.families.entry(FontFamily::Proportional).or_default().insert(0, "kon".into());
    if let Some(m) = sys(&["CascadiaMono.ttf", "consola.ttf"]) {
        f.font_data.insert("kon-mono".into(), Arc::new(egui::FontData::from_owned(m)));
        f.families.entry(FontFamily::Monospace).or_default().insert(0, "kon-mono".into());
    }
    let mut b = vec!["kon-bold".to_string()];
    b.extend(f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    f.families.insert(FontFamily::Name("bold".into()), b);
    ctx.set_fonts(f);
}

/// Vigia o cursor do sistema a cada 30 ms e só pede um quadro quando ele entra ou sai do notch.
fn spawn_cursor_watch(ctx: egui::Context, hit: Arc<Mutex<Hit>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(30));
        let Some((x, y)) = crate::platform::cursor_in_window() else { continue };
        let mut h = hit.lock().unwrap();
        let ppp = if h.ppp > 0.0 { h.ppp } else { 1.0 };
        let inside = h.rect.map_or(false, |r| r.contains(pos2(x as f32 / ppp, y as f32 / ppp)));
        if inside != h.inside { h.inside = inside; drop(h); ctx.request_repaint(); }
    });
}

/* ───────── ícone perto do relógio ───────── */
mod tray {
    pub enum Action { Open, Quit }

    #[cfg(windows)]
    pub struct Tray { _icon: tray_icon::TrayIcon, open: tray_icon::menu::MenuId, quit: tray_icon::menu::MenuId }

    #[cfg(windows)]
    impl Tray {
        pub fn new(ctx: eframe::egui::Context) -> Option<Self> {
            use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
            let menu = Menu::new();
            let open = MenuItem::new("Abrir painel", true, None);
            let quit = MenuItem::new("Sair do Kon", true, None);
            menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit]).ok()?;
            let icon = tray_icon::Icon::from_rgba(icon_rgba(), 32, 32).ok()?;
            let t = tray_icon::TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Kon").with_icon(icon).build().ok()?;
            let c2 = ctx.clone();
            tray_icon::menu::MenuEvent::set_event_handler(Some(move |_e| c2.request_repaint()));
            tray_icon::TrayIconEvent::set_event_handler(Some(move |_e| ctx.request_repaint()));
            Some(Tray { _icon: t, open: open.id().clone(), quit: quit.id().clone() })
        }
        pub fn poll(&self) -> Vec<Action> {
            let mut out = vec![];
            while let Ok(e) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                if e.id == self.open { out.push(Action::Open) } else if e.id == self.quit { out.push(Action::Quit) }
            }
            out
        }
    }

    /// Ícone 32×32 desenhado na hora: o tofu do Kon.
    #[cfg(windows)]
    fn icon_rgba() -> Vec<u8> {
        let mut v = vec![0u8; 32 * 32 * 4];
        for y in 0..32 { for x in 0..32 {
            let i = (y * 32 + x) * 4;
            let inb = x >= 3 && x <= 28 && y >= 6 && y <= 26 && !((x < 7 || x > 24) && (y < 10 || y > 22));
            let eye = (y >= 13 && y <= 19) && ((x >= 10 && x <= 12) || (x >= 19 && x <= 21));
            if inb { let c = if eye { 17 } else { 250 }; v[i] = c; v[i + 1] = c; v[i + 2] = c; v[i + 3] = 255; }
        }}
        v
    }

    #[cfg(not(windows))]
    pub struct Tray;
    #[cfg(not(windows))]
    impl Tray {
        pub fn new(_ctx: eframe::egui::Context) -> Option<Self> { None }
        pub fn poll(&self) -> Vec<Action> { vec![] }
    }
}
