//! Interface do Kon nativo: o notch compacto, o painel e as páginas.

use crate::media::{Media, MediaCmd, MediaState};
use crate::settings::Settings;
#[allow(dead_code)]
const _ICONS: () = ();
use crate::icons as ic;
use crate::tasks::Tasks;
use crate::weather::{self, Weather};
use crate::system::{self, SysInfo};
use crate::clipboard;
use crate::usage;
use crate::quick::{self, Quick};
use crate::claude::{self, Status as CStatus};
use crate::stash::{self, Stash};
use std::sync::atomic::{AtomicBool, Ordering};
use eframe::egui::{self, pos2, vec2, emath, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WIN_W: f32 = 820.0;
const COMPACT: Vec2 = Vec2::new(168.0, 32.0);
const COMPACT_MUSIC: Vec2 = Vec2::new(250.0, 36.0);
const PANEL: Vec2 = Vec2::new(660.0, 278.0);
const BG: Color32 = Color32::from_rgb(0, 0, 0);
const CARD: Color32 = Color32::from_rgb(20, 20, 22);
const DIM: Color32 = Color32::from_rgba_premultiplied(140, 140, 148, 255);
const DAYS: [&str; 7] = ["DOM", "SEG", "TER", "QUA", "QUI", "SEX", "SÁB"];
const MONTHS: [&str; 12] = ["JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum View { Compact, Panel, Perm, Sheet }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Home, Quick, Water, Claude, Files, Clip, Music, Weather, Focus, Todo, System, Settings }

impl Page {
    fn id(self) -> &'static str { match self { Page::Home => "home", Page::Music => "music", Page::Weather => "weather", Page::Focus => "focus", Page::Todo => "todo", Page::Clip => "clip", Page::Files => "files", Page::Claude => "claude", Page::Quick => "quick", Page::Water => "water", Page::System => "system", Page::Settings => "settings" } }
    fn from(s: &str) -> Page { match s { "music" => Page::Music, "weather" => Page::Weather, "focus" => Page::Focus, "todo" => Page::Todo, "clip" => Page::Clip, "files" => Page::Files, "claude" => Page::Claude, "quick" => Page::Quick, "water" => Page::Water, "system" => Page::System, _ => Page::Home } }
    fn label(self) -> &'static str { match self { Page::Home => "INÍCIO", Page::Music => "MÚSICA", Page::Weather => "CLIMA", Page::Focus => "FOCO", Page::Todo => "TAREFAS", Page::Clip => "CLIPS", Page::Files => "ARQUIVOS", Page::Claude => "CLAUDE", Page::Quick => "ATALHOS", Page::Water => "ÁGUA", Page::System => "SISTEMA", Page::Settings => "AJUSTES" } }
    fn icon(self) -> char { match self { Page::Home => ic::LAYOUT_GRID, Page::Music => ic::MUSIC_2, Page::Weather => ic::CLOUD_SUN, Page::Focus => ic::TIMER, Page::Todo => ic::LIST_TODO, Page::Clip => ic::CLIPBOARD_LIST, Page::Files => ic::FOLDER_OPEN, Page::Claude => ic::ASTERISK, Page::Quick => ic::ZAP, Page::Water => ic::GLASS_WATER, Page::System => ic::CPU, Page::Settings => ic::SETTINGS } }
    /// cor do ícone "ligado" (mesmas do Kon atual)
    fn color(self) -> Color32 { match self { Page::Music => Color32::from_rgb(244, 114, 182), Page::Weather => Color32::from_rgb(56, 189, 248), Page::Focus => Color32::from_rgb(255, 107, 91), Page::Todo => Color32::from_rgb(163, 230, 53), _ => Color32::from_rgb(251, 146, 60) } }
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
    weather: Arc<Mutex<Option<Weather>>>,
    tasks: Tasks,
    sys: Arc<Mutex<SysInfo>>,
    sys_on: Arc<AtomicBool>,
    clips: clipboard::Shared,
    clip_scroll: f32,
    stash: Stash,
    claude: claude::Shared,
    hooks_on: bool,
    hooks_err: Option<String>,
    files_scroll: f32,
    drop_back: Option<Page>,
    toast_msg: Option<(String, Instant)>,
    ctoast: Option<(String, Instant, bool)>,
    quick: Vec<Quick>,
    qform: Option<(String, String, String)>, // tipo, alvo, nome
    qedit: bool,
    sys_alerts: Arc<AtomicBool>,
    usage: usage::Shared,
    claude_usage_tab: bool,
    usage_at: Option<Instant>,
    sheet_scroll: f32,
    art: Option<(String, egui::TextureHandle)>,
    adding: Option<String>,
    editing: bool,
    focusable: bool,
    _tray: Option<tray::Tray>,
    quit: bool,
}

impl KonApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let settings = Settings::load();
        let approvals0 = settings.approvals;
        let ctx = cc.egui_ctx.clone();
        let media = Media::start(move || ctx.request_repaint());
        let ctx2 = cc.egui_ctx.clone();
        let weather = weather::start(move || ctx2.request_repaint());
        let sys_on = Arc::new(AtomicBool::new(false));
        let ctx3 = cc.egui_ctx.clone();
        let ctx4 = cc.egui_ctx.clone();
        let clips = clipboard::start(settings.clipboard_on, move || ctx4.request_repaint());
        let sys_alerts = Arc::new(AtomicBool::new(settings.sys_alerts));
        let sys = system::start(sys_on.clone(), sys_alerts.clone(), move || ctx3.request_repaint());
        let hit = Arc::new(Mutex::new(Hit { ppp: 1.0, ..Default::default() }));
        spawn_cursor_watch(cc.egui_ctx.clone(), hit.clone());
        let mins = settings.focus_minutes.clamp(1, 180);
        KonApp {
            page: Page::from(&std::env::var("KON_PAGE").unwrap_or_else(|_| settings.page.clone())),
            settings, media,
            // KON_DEMO=painel abre já com o painel (só para capturas de teste)
            view: match std::env::var("KON_DEMO").as_deref() { Ok("painel") => View::Panel, Ok("ajustes") => View::Sheet, _ => View::Compact },
            w: Spring::new(COMPACT.x), h: Spring::new(COMPACT.y),
            hit, passthrough: true,
            hover_since: None, leave_since: None,
            placed: false,
            last_frame: Instant::now(),
            blink_at: Instant::now() + Duration::from_millis(3800),
            focus: Focus { total: mins * 60, left: (mins * 60) as f32, running_until: None, finished_at: None },
            weather, tasks: Tasks::load(), sys, sys_on, art: None, clips, clip_scroll: 0.0, toast_msg: None, ctoast: None, quick: quick::load(), qform: None, qedit: false, sys_alerts, usage: usage::new(), claude_usage_tab: std::env::var("KON_USAGE").is_ok(), usage_at: None, sheet_scroll: 0.0, stash: Stash::load(), claude: { let c = cc.egui_ctx.clone(); claude::start(approvals0, move || c.request_repaint()) }, hooks_on: claude::hooks_state().0, hooks_err: None, files_scroll: 0.0, drop_back: None, adding: None, editing: false, focusable: false,
            _tray: tray::Tray::new(cc.egui_ctx.clone()),
            quit: false,
        }
    }

    fn target_size(&self, m: &MediaState) -> Vec2 {
        match self.view {
            View::Panel => PANEL,
            View::Perm => vec2(470.0, 100.0),
            View::Sheet => vec2(440.0, 470.0),
            View::Compact => if let Some((t, _, _)) = self.ctoast.as_ref().filter(|c| c.1.elapsed() < Duration::from_secs(5)) { vec2((t.chars().count() as f32 * 6.6 + 78.0).clamp(220.0, 520.0), 36.0) } else if self.claude_busy() { vec2(300.0, 36.0) } else if m.active && m.playing { COMPACT_MUSIC } else { COMPACT },
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
        self.sys_on.store(self.view == View::Panel && self.page == Page::System, Ordering::Relaxed);
        self.update_art(ctx, &m);

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
        // pedido de permissão do Claude Code: a ilha vira o cartão de aprovação
        let has_perm = !self.claude.lock().unwrap().perms.is_empty();
        if has_perm && self.view != View::Perm { self.view = View::Perm; }
        if !has_perm && self.view == View::Perm { self.view = View::Compact; }
        // arquivos arrastados até a ilha: abre a gaveta e guarda ao soltar
        let (hovering_files, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>()));
        if hovering_files && self.page != Page::Files { self.drop_back = Some(self.page); self.page = Page::Files; self.view = View::Panel; self.stash.refresh_missing(); }
        if !dropped.is_empty() {
            let n = self.stash.add(&dropped);
            self.page = Page::Files; self.view = View::Panel; self.drop_back = None;
            self.toast(&match n { 0 => "Já estava guardado".to_string(), 1 => "1 item guardado".into(), k => format!("{k} itens guardados") });
        } else if !hovering_files { if let Some(b) = self.drop_back.take() { if self.page == Page::Files { self.page = b; } } }
        // abrir ao passar o mouse (com um respiro de 90 ms) e fechar ao sair (280 ms de tolerância)
        if inside {
            self.leave_since = None;
            if self.view == View::Compact && self.settings.open_mode != "click" && !has_perm {
                let t = *self.hover_since.get_or_insert(now);
                if now - t > Duration::from_millis(if self.settings.open_mode == "delay" { 450 } else { 110 }) { self.view = View::Panel; }
            }
            if self.view == View::Compact && !has_perm && ctx.input(|i| i.pointer.primary_clicked()) { self.view = View::Panel; }
        } else {
            self.hover_since = None;
            if self.view == View::Panel && std::env::var("KON_DEMO").is_err() {
                let t = *self.leave_since.get_or_insert(now);
                if now - t > Duration::from_millis(280) { self.view = View::Compact; if self.page == Page::Settings { self.page = Page::from(&self.settings.page); } }
            }
        }

        // teclado só enquanto há um campo de texto aberto
        if self.view == View::Sheet && ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.view = View::Panel; }
        let want_focus = ((self.adding.is_some() || self.qform.is_some()) && self.view == View::Panel) || self.view == View::Sheet;
        if want_focus != self.focusable { self.focusable = want_focus; crate::platform::set_focusable(want_focus); }
        if self.view == View::Compact { self.adding = None; self.editing = false; self.qform = None; self.qedit = false; }
        // aviso de PC sobrecarregado vindo da thread do sistema
        if let Some(a) = self.sys.lock().unwrap().alert.take() { self.ctoast = Some((a, now, true)); }
        if self.ctoast.as_ref().map_or(false, |c| c.1.elapsed() >= Duration::from_millis(5300)) { self.ctoast = None; }

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
            if self.view == View::Perm || (self.w.v > 400.0 && self.h.v < 120.0) {
                let a = ((self.h.v - COMPACT.y) / (100.0 - COMPACT.y)).clamp(0.0, 1.0);
                if self.view == View::Perm && a > 0.6 { self.draw_perm(ui, rect, blink, (a - 0.6) / 0.4); }
            } else if open < 0.35 {
                self.draw_compact(ui, rect, &m, blink, 1.0 - open / 0.35);
            }
            if self.view == View::Sheet {
                let a = ((self.h.v - 300.0) / 170.0).clamp(0.0, 1.0);
                if a > 0.3 { self.draw_sheet(ui, rect, (a - 0.3) / 0.7); }
            } else if open > 0.55 && !(self.h.v > PANEL.y + 20.0) {
                let alpha = ((open - 0.55) / 0.45).clamp(0.0, 1.0);
                self.draw_panel(ui, rect, &m, blink, alpha);
            }
        });

        // ── quando redesenhar de novo (a janela é transparente: cada quadro custa) ──
        if animating {
            ctx.request_repaint();
        } else if self.view == View::Panel {
            ctx.request_repaint_after(Duration::from_millis(if self.focus.running_until.is_some() { 250 } else { 1000 }));
        } else if self.claude_busy() {
            ctx.request_repaint_after(Duration::from_millis(50)); // asterisco girando
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
        let p = ui.painter().clone();
        let p = &p;
        if let Some((t, t0, err)) = self.ctoast.clone() {
            if t0.elapsed() < Duration::from_secs(5) && self.view == View::Compact {
                let col = if err { Color32::from_rgb(248, 113, 113) } else { Color32::from_rgb(74, 222, 128) };
                draw_tofu(p, pos2(r.left() + 24.0, r.center().y), 0.75, blink, alpha);
                p.text(pos2(r.left() + 44.0, r.center().y), Align2::LEFT_CENTER, if err { ic::X } else { ic::CHECK }, icon_font(12.0), fade(col, alpha));
                p.text(pos2(r.left() + 60.0, r.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &t, 12.0, r.width() - 74.0), bold(12.0), fade(Color32::WHITE, alpha));
                ui.ctx().request_repaint_after(Duration::from_millis(200));
                return;
            }
        }
        if self.claude_busy() && r.width() > COMPACT.x + 40.0 {
            let l = self.claude.lock().unwrap().live.clone();
            let done = l.status == CStatus::Done;
            let t = ui.input(|i| i.time) as f32;
            let c = pos2(r.left() + 20.0, r.center().y);
            let orange = Color32::from_rgb(251, 146, 60);
            p.circle_filled(c, 9.0, fade(Color32::from_rgba_unmultiplied(251, 146, 60, 40), alpha));
            let sc = if done { 1.0 + 0.15 * ((t * 5.2).sin().max(0.0)) } else { 1.0 };
            let mut job = egui::text::LayoutJob::single_section(ic::ASTERISK.to_string(), egui::TextFormat { font_id: icon_font(15.0 * sc), color: fade(orange, alpha), ..Default::default() });
            job.halign = Align::Center;
            let g = ui.fonts_mut(|f| f.layout_job(job));
            let ang = if done { 0.0 } else { (t / 2.4).fract() * std::f32::consts::TAU };
            let half = g.size() / 2.0;
            let rot = emath::Rot2::from_angle(ang);
            let origin = c - rot * vec2(0.0, half.y);
            p.add(egui::epaint::TextShape::new(origin, g, orange).with_angle(ang));
            p.text(pos2(r.left() + 34.0, r.center().y), Align2::LEFT_CENTER, self.claude_label(&l), bold(12.5), fade(Color32::WHITE, alpha));
            draw_tofu(p, r.center(), 0.85, blink, alpha);
            p.text(pos2(r.right() - 12.0, r.center().y), Align2::RIGHT_CENTER, mmss(claude_elapsed(&l)), FontId::monospace(12.5), fade(Color32::from_white_alpha(190), alpha));
            return;
        }
        draw_tofu(p, r.center(), 0.85, blink, alpha);
        if m.active && m.playing && r.width() > COMPACT.x + 20.0 {
            // capa (cor) à esquerda e barras à direita
            let art = Rect::from_center_size(pos2(r.left() + 22.0, r.center().y), vec2(20.0, 20.0));
            if let Some((_, t)) = &self.art { egui::Image::from_texture(t).corner_radius(5).tint(fade(Color32::WHITE, alpha)).paint_at(ui, art); }
            else { gradient_rr(ui.painter(), art, 5.0, Color32::from_rgb(214, 58, 134), Color32::from_rgb(76, 29, 149), alpha); }
            draw_bars(p, pos2(r.right() - 40.0, r.center().y), alpha);
        }
    }

    fn draw_panel(&mut self, ui: &mut egui::Ui, r: Rect, m: &MediaState, blink: bool, alpha: f32) {
        let now = chrono::Local::now();
        use chrono::{Datelike, Timelike};
        let p = ui.painter().clone();
        // cabeçalho
        let hy = r.top() + 22.0;
        if self.page == Page::Weather {
            let w = self.weather.lock().unwrap().clone();
            p.text(pos2(r.left() + 22.0, hy), Align2::LEFT_CENTER, "Clima", bold(13.0), fade(Color32::WHITE, alpha));
            let city = w.as_ref().map(|w| w.city.clone()).unwrap_or_else(|| "localizando…".into());
            p.text(pos2(r.left() + 66.0, hy), Align2::LEFT_CENTER, format!("{city} · atual"), FontId::proportional(13.0), fade(Color32::from_white_alpha(115), alpha));
            p.text(pos2(r.right() - 22.0, hy), Align2::RIGHT_CENTER, format!("{:02}:{:02}", now.hour(), now.minute()), bold(12.0), fade(Color32::WHITE, alpha));
            let body = Rect::from_min_max(pos2(r.left() + 28.0, r.top() + 42.0), pos2(r.right() - 28.0, r.bottom() - 50.0));
            self.page_weather(ui, body, w.as_ref(), alpha);
            self.draw_toast(ui, r, body, alpha);
        self.nav(ui, Rect::from_min_max(pos2(r.left() + 16.0, r.bottom() - 46.0), pos2(r.right() - 16.0, r.bottom() - 6.0)), alpha);
            return;
        }
        let date = format!("{} {:02} {}", DAYS[now.weekday().num_days_from_sunday() as usize], now.day(), MONTHS[now.month0() as usize]);
        p.text(pos2(r.left() + 22.0, hy), Align2::LEFT_CENTER, date, bold(11.0), fade(DIM, alpha));
        p.text(pos2(r.right() - 22.0, hy), Align2::RIGHT_CENTER, format!("{:02}:{:02}", now.hour(), now.minute()), bold(12.0), fade(Color32::WHITE, alpha));
        draw_tofu(&p, pos2(r.center().x, hy), 0.95, blink, alpha);

        let body = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 42.0), pos2(r.right() - 16.0, r.bottom() - 50.0));
        match self.page {
            Page::Home => self.page_home(ui, body, m, alpha),
            Page::Music => self.page_music(ui, body, m, alpha),
            Page::Focus => self.page_focus(ui, body, alpha),
            Page::Todo => self.page_todo(ui, body, alpha),
            Page::System => self.page_system(ui, body, alpha),
            Page::Clip => self.page_clip(ui, body, alpha),
            Page::Files => self.page_files(ui, body, alpha),
            Page::Claude => self.page_claude(ui, body, blink, alpha),
            Page::Quick => self.page_quick(ui, body, alpha),
            Page::Water => self.page_water(ui, body, alpha),
            Page::Weather => {}
            Page::Settings => {}
        }
        self.draw_toast(ui, r, body, alpha);
        self.nav(ui, Rect::from_min_max(pos2(r.left() + 16.0, r.bottom() - 46.0), pos2(r.right() - 16.0, r.bottom() - 6.0)), alpha);
    }

    fn draw_toast(&mut self, ui: &mut egui::Ui, r: Rect, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        if let Some((msg, t0)) = &self.toast_msg {
            let e = t0.elapsed().as_secs_f32();
            if e < 1.6 {
                let a = alpha * (1.0 - ((e - 1.3) / 0.3).clamp(0.0, 1.0)) * (e / 0.12).clamp(0.0, 1.0);
                let w = text_w(ui, msg, 11.5) + 40.0;
                let tr = Rect::from_center_size(pos2(r.center().x, body.bottom() - 16.0), vec2(w, 26.0));
                p.rect_filled(tr, CornerRadius::same(13), fade(Color32::from_rgb(28, 28, 30), a));
                p.rect_stroke(tr, CornerRadius::same(13), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), a)), egui::StrokeKind::Inside);
                p.text(pos2(tr.left() + 14.0, tr.center().y), Align2::LEFT_CENTER, ic::CHECK, icon_font(12.0), fade(Color32::from_rgb(74, 222, 128), a));
                p.text(pos2(tr.left() + 30.0, tr.center().y), Align2::LEFT_CENTER, msg.as_str(), bold(11.5), fade(Color32::WHITE, a));
                ui.ctx().request_repaint();
            } else { self.toast_msg = None; }
        }
    }

    fn nav(&mut self, ui: &mut egui::Ui, bar: Rect, alpha: f32) {
        // mesma barra do Kon atual: "+" à esquerda, páginas no meio (a ativa vira pílula com nome), engrenagem à direita
        let pages = [Page::Home, Page::Files, Page::Clip, Page::Music, Page::Weather, Page::Focus, Page::Todo, Page::Claude, Page::Quick, Page::System, Page::Water];
        let widths: Vec<f32> = pages.iter().map(|p| if *p == self.page { 30.0 + text_w(ui, p.label(), 10.5) } else { 28.0 }).collect();
        let total: f32 = widths.iter().sum::<f32>() + 2.0 * (pages.len() as f32 - 1.0);
        let mut x = bar.center().x - total / 2.0;
        for (i, pg) in pages.iter().enumerate() {
            let active = self.page == *pg;
            let rr = Rect::from_min_size(pos2(x, bar.center().y - 14.0), vec2(widths[i], 28.0));
            let resp = ui.interact(rr, ui.id().with(pg.id()), Sense::click());
            let p = ui.painter();
            if active { p.rect_filled(rr, CornerRadius::same(14), fade(Color32::from_white_alpha(26), alpha)); }
            else if resp.hovered() { p.rect_filled(rr, CornerRadius::same(14), fade(Color32::from_white_alpha(12), alpha)); }
            let col = if active { Color32::WHITE } else if resp.hovered() { Color32::from_white_alpha(200) } else { Color32::from_white_alpha(102) };
            if active {
                p.text(pos2(rr.left() + 15.0, rr.center().y), Align2::CENTER_CENTER, pg.icon(), icon_font(13.0), fade(col, alpha));
                p.text(pos2(rr.left() + 26.0, rr.center().y), Align2::LEFT_CENTER, pg.label(), bold(10.5), fade(col, alpha));
            } else {
                p.text(rr.center(), Align2::CENTER_CENTER, pg.icon(), icon_font(13.0), fade(col, alpha));
            }
            if resp.clicked() { self.adding = None; self.set_page(*pg); }
            x += widths[i] + 2.0;
        }
        // "+" : nova tarefa
        let plus = Rect::from_center_size(pos2(bar.left() + 14.0, bar.center().y), vec2(28.0, 28.0));
        let r1 = ui.interact(plus, ui.id().with("plus"), Sense::click());
        let p = ui.painter();
        p.circle_filled(plus.center(), 14.0, fade(Color32::from_white_alpha(if r1.hovered() { 30 } else { 16 }), alpha));
        p.text(plus.center(), Align2::CENTER_CENTER, ic::PLUS, icon_font(14.0), fade(Color32::from_white_alpha(180), alpha));
        if r1.clicked() { self.set_page(Page::Todo); self.adding = Some(String::new()); }
        // engrenagem
        let g = Rect::from_center_size(pos2(bar.right() - 14.0, bar.center().y), vec2(28.0, 28.0));
        let resp = ui.interact(g, ui.id().with("gear"), Sense::click());
        let p = ui.painter();
        if resp.hovered() || self.page == Page::Settings { p.circle_filled(g.center(), 14.0, fade(Color32::from_white_alpha(20), alpha)); }
        p.text(g.center(), Align2::CENTER_CENTER, ic::SETTINGS, icon_font(13.0), fade(Color32::from_white_alpha(if resp.hovered() { 204 } else { 102 }), alpha));
        if resp.clicked() { self.view = View::Sheet; self.sheet_scroll = 0.0; self.hooks_on = claude::hooks_state().0; }
    }

    /// Capa do álbum: decodifica uma vez por faixa e guarda como textura da placa de vídeo.
    fn update_art(&mut self, ctx: &egui::Context, m: &MediaState) {
        let Some(bytes) = &m.art else { self.art = None; return };
        if self.art.as_ref().map_or(false, |(k, _)| *k == m.art_key) { return; }
        if let Ok(img) = image::load_from_memory(bytes) {
            let img = img.thumbnail(256, 256).to_rgba8();
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            self.art = Some((m.art_key.clone(), ctx.load_texture("capa", ci, egui::TextureOptions::LINEAR)));
        }
    }

    fn cover(&self, ui: &mut egui::Ui, r: Rect, radius: u8, alpha: f32, big_icon: f32) {
        if let Some((_, t)) = &self.art {
            egui::Image::from_texture(t).corner_radius(radius).tint(fade(Color32::WHITE, alpha)).paint_at(ui, r);
        } else {
            // sem capa: mesmo degradê rosa/roxo do Kon atual com a nota
            gradient_rr(ui.painter(), r, radius as f32, Color32::from_rgb(214, 58, 134), Color32::from_rgb(76, 29, 149), alpha);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, ic::MUSIC_2, icon_font(big_icon), fade(Color32::from_white_alpha(170), alpha));
        }
    }

    fn toast(&mut self, m: &str) { self.toast_msg = Some((m.into(), Instant::now())); }

    fn claude_busy(&self) -> bool {
        let g = self.claude.lock().unwrap();
        g.live.status == CStatus::Working || g.live.done_at.map_or(false, |d| d.elapsed() < Duration::from_secs(12))
    }
    fn claude_label(&self, l: &claude::Live) -> String {
        if l.status == CStatus::Done { "Pronto".into() } else if !l.note.is_empty() { "Precisa de você".into() } else if l.status == CStatus::Working { claude::STAGES[l.stage.min(3)].into() } else { "Parado".into() }
    }

    fn draw_perm(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool, alpha: f32) {
        let p = ui.painter().clone();
        let (id, title, sub, project, more) = {
            let g = self.claude.lock().unwrap();
            let Some(pm) = g.perms.first() else { return };
            let (t, s) = claude::perm_text(pm);
            (pm.id, t, s, claude::base_name(&pm.cwd), g.perms.len() - 1)
        };
        draw_tofu_mood(&p, pos2(r.left() + 16.0 + 23.0, r.center().y), 1.45, blink, alpha, true);
        let x = r.left() + 16.0 + 46.0 + 12.0;
        let bw_deny = text_w(ui, "Negar", 11.5) + 24.0; let bw_ok = text_w(ui, "Permitir", 11.5) + 24.0;
        let right = r.right() - 14.0;
        let ok_r = Rect::from_min_size(pos2(right - bw_ok, r.center().y - 13.0), vec2(bw_ok, 26.0));
        let deny_r = Rect::from_min_size(pos2(ok_r.left() - 6.0 - bw_deny, r.center().y - 13.0), vec2(bw_deny, 26.0));
        let maxw = deny_r.left() - 12.0 - x;
        p.text(pos2(x, r.center().y - 22.0), Align2::LEFT_CENTER, ellipsize_w(ui, &title, 13.5, maxw), bold(13.5), fade(Color32::WHITE, alpha));
        p.text(pos2(x, r.center().y - 3.0), Align2::LEFT_CENTER, ellipsize_chars(&sub, (maxw / 6.7) as usize), FontId::monospace(11.0), fade(Color32::from_white_alpha(140), alpha));
        let mut lx = x; let ly = r.center().y + 17.0;
        let small = FontId::proportional(10.5);
        if !project.is_empty() { let t = ellipsize_w(ui, &project, 10.5, 90.0); p.text(pos2(lx, ly), Align2::LEFT_CENTER, &t, small.clone(), fade(Color32::from_white_alpha(102), alpha)); lx += text_w(ui, &t, 10.5) + 10.0; }
        let link = |ui: &mut egui::Ui, lx: &mut f32, label: &str, icon: Option<char>, col: Color32, key: &str| -> bool {
            let w = text_w(ui, label, 10.5) + if icon.is_some() { 14.0 } else { 0.0 };
            let rr = Rect::from_min_size(pos2(*lx, ly - 8.0), vec2(w, 16.0));
            let resp = ui.interact(rr, ui.id().with(("permlink", key)), Sense::click());
            let c = if resp.hovered() { Color32::WHITE } else { col };
            let mut tx = *lx;
            if let Some(i) = icon { p.text(pos2(tx, ly), Align2::LEFT_CENTER, i, icon_font(10.0), fade(c, alpha)); tx += 14.0; }
            p.text(pos2(tx, ly), Align2::LEFT_CENTER, label, small.clone(), fade(c, alpha));
            *lx += w + 10.0;
            resp.clicked()
        };
        let mut decision: Option<Option<&'static str>> = None;
        if link(ui, &mut lx, "Sempre permitir", None, Color32::from_rgba_unmultiplied(125, 211, 252, 204), "always") { decision = Some(Some("always")); }
        if link(ui, &mut lx, "No terminal", Some(ic::TERMINAL), Color32::from_white_alpha(102), "term") { decision = Some(None); }
        if more > 0 { p.text(pos2(lx, ly), Align2::LEFT_CENTER, format!("+{more}"), small.clone(), fade(Color32::from_rgba_unmultiplied(252, 211, 77, 204), alpha)); }
        if pill(ui, &p, deny_r, "Negar", false, alpha).clicked() { decision = Some(Some("deny")); }
        if pill(ui, &p, ok_r, "Permitir", true, alpha).clicked() { decision = Some(Some("allow")); }
        if let Some(d) = decision { claude::decide(&self.claude, id, d); }
    }

    fn page_claude(&mut self, ui: &mut egui::Ui, body: Rect, blink: bool, alpha: f32) {
        let p = ui.painter().clone();
        let (l, sessions, listening) = {
            let g = self.claude.lock().unwrap();
            let mut ss: Vec<_> = g.sessions.values().cloned().collect();
            ss.sort_by(|a, b| b.at.cmp(&a.at));
            (g.live.clone(), ss, g.listening)
        };
        let done = l.status == CStatus::Done; let running = l.status == CStatus::Working;
        // topo: sessões ativas
        let mut sx = body.left();
        for s in sessions.iter().take(4) {
            let name = { let b = claude::base_name(&s.cwd); if b.is_empty() { "sessão".into() } else { b } };
            let name = ellipsize_w(ui, &name, 10.5, 100.0);
            let w = text_w(ui, &name, 10.5) + 26.0;
            let rr = Rect::from_min_size(pos2(sx, body.top() + 2.0), vec2(w, 20.0));
            p.rect_filled(rr, CornerRadius::same(10), fade(Color32::from_white_alpha(18), alpha));
            let dot = match s.status { "waiting" => Color32::from_rgb(251, 191, 36), "working" => Color32::from_rgb(251, 146, 60), "done" => Color32::from_rgb(52, 211, 153), _ => Color32::from_white_alpha(77) };
            p.circle_filled(pos2(rr.left() + 11.0, rr.center().y), 3.0, fade(dot, alpha));
            p.text(pos2(rr.left() + 18.0, rr.center().y), Align2::LEFT_CENTER, name, FontId::proportional(10.5), fade(Color32::from_white_alpha(178), alpha));
            sx = rr.right() + 4.0;
        }
        // Ao vivo | Uso
        let mut tab = self.claude_usage_tab;
        if seg(ui, &p, pos2(body.right(), body.top() + 12.0), &[("Ao vivo", false), ("Uso", true)], &mut tab, alpha, "cview") {
            self.claude_usage_tab = tab;
            if tab { self.refresh_usage(ui.ctx()); }
        }
        if self.claude_usage_tab {
            if self.usage_at.map_or(true, |t| t.elapsed() > Duration::from_secs(60)) { self.refresh_usage(ui.ctx()); }
            self.page_usage(ui, body, alpha);
            return;
        }
        let area = Rect::from_min_max(pos2(body.left(), body.top() + 30.0), body.max);
        let left = Rect::from_min_size(area.min, vec2(area.width() * 0.32, area.height()));
        let right = Rect::from_min_max(pos2(left.right() + 8.0, area.top()), area.max);
        // brilho âmbar no canto
        // brilho difuso (equivale ao blur-3xl do Kon atual)
        let pc = p.with_clip_rect(body.expand(16.0));
        for k in 0..14 { let rr = 90.0 - k as f32 * 6.0; pc.circle_filled(pos2(left.left() - 10.0, left.top() - 10.0), rr, fade(Color32::from_rgba_unmultiplied(245, 158, 11, 3), alpha)); }
        draw_tofu_mood(&p, pos2(left.center().x, left.top() + 34.0), 2.3, blink, alpha, !l.note.is_empty());
        p.text(pos2(left.center().x, left.center().y + 22.0), Align2::CENTER_CENTER, mmss(claude_elapsed(&l)), FontId::monospace(24.0), fade(Color32::WHITE, alpha));
        let label = self.claude_label(&l);
        let (bg, fg, dot) = if done { (Color32::from_rgba_unmultiplied(236, 72, 153, 38), Color32::from_rgb(251, 207, 232), Color32::from_rgb(244, 114, 182)) }
            else if !l.note.is_empty() { (Color32::from_rgba_unmultiplied(245, 158, 11, 38), Color32::from_rgb(253, 230, 138), Color32::from_rgb(251, 146, 60)) }
            else if running { (Color32::from_rgba_unmultiplied(249, 115, 22, 38), Color32::from_rgb(254, 215, 170), Color32::from_rgb(251, 146, 60)) }
            else { (Color32::from_white_alpha(20), Color32::from_white_alpha(153), Color32::from_rgb(251, 146, 60)) };
        let bw = text_w(ui, &label, 12.0) + 34.0;
        let br = Rect::from_center_size(pos2(left.center().x, left.bottom() - 18.0), vec2(bw, 28.0));
        p.rect_filled(br, CornerRadius::same(14), fade(bg, alpha));
        let pulse = if running { 0.65 + 0.35 * (ui.input(|i| i.time) as f32 * 5.2).sin() } else { 1.0 };
        p.circle_filled(pos2(br.left() + 15.0, br.center().y), 3.0, fade(dot, alpha * pulse));
        p.text(pos2(br.left() + 24.0, br.center().y), Align2::LEFT_CENTER, &label, FontId::proportional(12.0), fade(fg, alpha));
        if running { ui.ctx().request_repaint_after(Duration::from_millis(80)); }
        // direita: título, diff e etapas
        let title = if !l.note.is_empty() { l.note.clone() } else if !l.title.is_empty() { l.title.clone() }
            else if !listening { "Outro Kon já está recebendo o Claude Code".into() }
            else if self.hooks_on { "Esperando o Claude Code…".into() } else { "Conecte o Claude Code em Ajustes".into() };
        let title = if let Some(e) = &self.hooks_err { e.clone() } else { title };
        p.text(pos2(right.left() + 8.0, right.top() + 10.0), Align2::LEFT_CENTER, ellipsize_w(ui, &title, 14.0, right.width() - 12.0), bold(14.0), fade(Color32::WHITE, alpha));
        let dr = Rect::from_min_size(pos2(right.left() + 8.0, right.top() + 26.0), vec2(right.width() - 12.0, 72.0));
        p.rect_filled(dr, CornerRadius::same(12), fade(Color32::from_black_alpha(153), alpha));
        p.rect_stroke(dr, CornerRadius::same(12), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside);
        let hy = dr.top() + 12.0;
        let fname = if l.file.is_empty() { "NENHUM ARQUIVO AINDA".to_string() } else { l.file.clone() };
        p.text(pos2(dr.left() + 12.0, hy), Align2::LEFT_CENTER, ellipsize_w(ui, &fname, 10.0, dr.width() - 90.0), bold(10.0), fade(Color32::from_white_alpha(128), alpha));
        let minus = format!("-{}", l.minus);
        p.text(pos2(dr.right() - 12.0, hy), Align2::RIGHT_CENTER, &minus, FontId::monospace(10.0), fade(Color32::from_rgb(248, 113, 113), alpha));
        p.text(pos2(dr.right() - 16.0 - text_w(ui, &minus, 10.5), hy), Align2::RIGHT_CENTER, format!("+{}", l.plus), FontId::monospace(10.0), fade(Color32::from_rgb(52, 211, 153), alpha));
        p.line_segment([pos2(dr.left(), dr.top() + 24.0), pos2(dr.right(), dr.top() + 24.0)], Stroke::new(1.0_f32, fade(Color32::from_white_alpha(15), alpha)));
        let row = |y: f32, bgc: Color32, sign: &str, sc: Color32, txt: &str, tc: Color32| {
            let rr = Rect::from_min_size(pos2(dr.left() + 1.0, y), vec2(dr.width() - 2.0, 20.0));
            p.rect_filled(rr, CornerRadius::ZERO, fade(bgc, alpha));
            p.text(pos2(rr.left() + 11.0, rr.center().y), Align2::LEFT_CENTER, sign, FontId::monospace(12.0), fade(sc, alpha));
            p.text(pos2(rr.left() + 31.0, rr.center().y), Align2::LEFT_CENTER, ellipsize_chars(txt, ((rr.width() - 40.0) / 7.3) as usize), FontId::monospace(12.0), fade(tc, alpha));
        };
        row(dr.top() + 28.0, Color32::from_rgba_unmultiplied(239, 68, 68, 38), "-", Color32::from_rgba_unmultiplied(252, 165, 165, 128), &l.removed, Color32::from_rgba_unmultiplied(254, 202, 202, 230));
        row(dr.top() + 48.0, Color32::from_rgba_unmultiplied(16, 185, 129, 38), "+", Color32::from_rgba_unmultiplied(110, 231, 183, 128), &l.added, fade(Color32::from_rgb(167, 243, 208), if l.added.is_empty() { 0.35 } else { 1.0 }));
        let stage_idx: i32 = if done { 4 } else if running { l.stage as i32 } else { -1 };
        let gw = (dr.width() - 3.0 * 6.0) / 4.0;
        let col = if done { Color32::from_rgb(244, 114, 182) } else { Color32::from_rgb(251, 146, 60) };
        for (i, sname) in ["LER", "EDITAR", "TESTAR", "PRONTO"].iter().enumerate() {
            let x0 = dr.left() + i as f32 * (gw + 6.0);
            let by = right.bottom() - 22.0;
            let tr = Rect::from_min_size(pos2(x0, by), vec2(gw, 3.0));
            p.rect_filled(tr, CornerRadius::same(2), fade(Color32::from_white_alpha(25), alpha));
            let fill = if (i as i32) < stage_idx { 1.0 } else if i as i32 == stage_idx { 0.55 } else { 0.0 };
            if fill > 0.0 { p.rect_filled(Rect::from_min_size(tr.min, vec2(gw * fill, 3.0)), CornerRadius::same(2), fade(col, alpha)); }
            let active = i as i32 == stage_idx || (done && i == 3);
            let tc = if active { Color32::WHITE } else if (i as i32) < stage_idx { Color32::from_white_alpha(140) } else { Color32::from_white_alpha(77) };
            p.text(pos2(x0, by + 13.0), Align2::LEFT_CENTER, *sname, bold(9.5), fade(tc, alpha));
        }
    }

    fn refresh_usage(&mut self, ctx: &egui::Context) {
        self.usage_at = Some(Instant::now());
        let c = ctx.clone();
        usage::refresh(&self.usage, self.settings.claude_plan.clone(), move || c.request_repaint());
    }

    fn page_usage(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let u = self.usage.lock().unwrap().usage.clone();
        let plan_label = match self.settings.claude_plan.as_str() { "pro" => "Pro", "max5" => "Max 5×", "max20" => "Max 20×", _ => "Automático" };
        p.text(pos2(body.left(), body.top() + 12.0), Align2::LEFT_CENTER, format!("Estimativa local · plano {plan_label} · nada sai do PC"), FontId::proportional(10.5), fade(Color32::from_white_alpha(90), alpha));
        let area = Rect::from_min_max(pos2(body.left(), body.top() + 30.0), body.max);
        let Some(u) = u else { p.text(area.center(), Align2::CENTER_CENTER, "Lendo o histórico do Claude Code…", FontId::proportional(12.0), fade(Color32::from_white_alpha(115), alpha)); return };
        if !u.found {
            p.text(pos2(area.center().x, area.center().y - 9.0), Align2::CENTER_CENTER, "Nenhum histórico do Claude Code neste computador", FontId::proportional(13.0), fade(Color32::from_white_alpha(190), alpha));
            p.text(pos2(area.center().x, area.center().y + 10.0), Align2::CENTER_CENTER, "O Kon lê os registros locais em ~/.claude/projects. Use o Claude Code e o uso aparece aqui.", FontId::proportional(11.0), fade(Color32::from_white_alpha(102), alpha));
            return;
        }
        let gap = 7.0;
        let w1 = area.width() * 0.29; let w3 = area.width() * 0.31;
        let c1 = Rect::from_min_size(area.min, vec2(w1, area.height()));
        let c3 = Rect::from_min_max(pos2(area.right() - w3, area.top()), area.max);
        let c2 = Rect::from_min_max(pos2(c1.right() + gap, area.top()), pos2(c3.left() - gap, area.bottom()));
        let pc = p.with_clip_rect(body.expand(16.0));
        for k in 0..14 { pc.circle_filled(pos2(area.left() + 40.0, area.top() + 10.0), 90.0 - k as f32 * 6.0, fade(Color32::from_rgba_unmultiplied(180, 83, 9, 4), alpha)); }
        let ucard = |r: Rect| { p.rect_filled(r, CornerRadius::same(14), fade(Color32::from_white_alpha(13), alpha)); p.rect_stroke(r, CornerRadius::same(14), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside); };
        let dim = |a: u8| fade(Color32::from_white_alpha(a), alpha);
        // janela de 5 h
        ucard(c1);
        let rc = pos2(c1.center().x, c1.center().y - 14.0);
        let pct = u.window.map_or(0, |w| w.1);
        p.circle_stroke(rc, 28.0, Stroke::new(5.5_f32, dim(25)));
        if pct > 0 { arc(&p, rc, 28.0, pct as f32 / 100.0, Stroke::new(5.5_f32, fade(if pct >= 90 { Color32::from_rgb(248, 113, 113) } else { Color32::from_rgb(251, 146, 60) }, alpha))); }
        p.text(rc, Align2::CENTER_CENTER, if u.window.is_some() { format!("{pct}%") } else { "—".into() }, bold(15.0), dim(255));
        p.text(pos2(c1.center().x, rc.y + 44.0), Align2::CENTER_CENTER, "janela de 5 h", FontId::proportional(10.5), dim(115));
        let now_ms = chrono::Utc::now().timestamp_millis();
        let sub = u.window.map_or("sem uso agora".to_string(), |w| format!("renova em {}", usage::fmt_left(w.2 - now_ms)));
        p.text(pos2(c1.center().x, rc.y + 58.0), Align2::CENTER_CENTER, sub, FontId::proportional(10.5), dim(115));
        ui.interact(c1, ui.id().with("ujan"), Sense::hover()).on_hover_text(format!("Estimativa: {} de ~{} tokens (entrada + saída) · plano {plan_label}", usage::fmt_tok(u.window.map_or(0, |w| w.0)), usage::fmt_tok(u.limit)));
        // hoje / semana
        let hw = (c2.width() - gap) / 2.0;
        for (i, (l, v)) in [("HOJE", u.today), ("SEMANA", u.week)].iter().enumerate() {
            let r = Rect::from_min_size(pos2(c2.left() + i as f32 * (hw + gap), c2.top()), vec2(hw, 62.0));
            ucard(r);
            p.text(pos2(r.left() + 10.0, r.top() + 14.0), Align2::LEFT_CENTER, *l, FontId::proportional(10.0), dim(115));
            p.text(pos2(r.left() + 10.0, r.top() + 32.0), Align2::LEFT_CENTER, usage::fmt_tok(v.0), bold(17.0), dim(255));
            p.text(pos2(r.left() + 10.0, r.top() + 50.0), Align2::LEFT_CENTER, format!("≈ {}", usage::fmt_usd(v.1)), FontId::proportional(10.5), dim(115));
        }
        // barras dos 7 dias
        let br = Rect::from_min_max(pos2(c2.left(), c2.top() + 62.0 + gap), c2.max);
        ucard(br);
        let inner = Rect::from_min_max(pos2(br.left() + 10.0, br.top() + 8.0), pos2(br.right() - 10.0, br.bottom() - 22.0));
        let max_d = u.days.iter().map(|d| d.1).max().unwrap_or(1).max(1);
        let bw = (inner.width() - 6.0 * 5.0) / 7.0;
        use chrono::{Datelike, TimeZone};
        for (i, d) in u.days.iter().enumerate() {
            let h = (inner.height() * (d.1 as f32 / max_d as f32)).max(inner.height() * 0.04);
            let x = inner.left() + i as f32 * (bw + 5.0);
            let r = Rect::from_min_max(pos2(x, inner.bottom() - h), pos2(x + bw, inner.bottom()));
            p.rect_filled(r, CornerRadius::same(3), fade(if i == 6 { Color32::from_rgb(251, 146, 60) } else { Color32::from_white_alpha(51) }, alpha));
            let wd = chrono::Local.timestamp_millis_opt(d.0).single().map_or(0, |t| t.weekday().num_days_from_sunday() as usize);
            let lbl = if i == 6 { "hoje".to_string() } else { "DSTQQSS".chars().nth(wd).unwrap().to_string() };
            p.text(pos2(x + bw / 2.0, br.bottom() - 11.0), Align2::CENTER_CENTER, lbl, FontId::proportional(9.5), dim(if i == 6 { 255 } else { 102 }));
            ui.interact(r.union(Rect::from_min_size(pos2(x, inner.top()), vec2(bw, 1.0))), ui.id().with(("dia", i)), Sense::hover()).on_hover_text(usage::fmt_tok(d.1));
        }
        // modelos
        ucard(c3);
        p.text(pos2(c3.left() + 10.0, c3.top() + 14.0), Align2::LEFT_CENTER, "MODELOS", FontId::proportional(10.0), dim(115));
        let mc = |n: &str| match n { "Opus" => Color32::from_rgb(168, 85, 247), "Sonnet" => Color32::from_rgb(52, 211, 153), "Haiku" => Color32::from_rgb(96, 165, 250), _ => Color32::from_rgb(163, 163, 163) };
        let bar = Rect::from_min_size(pos2(c3.left() + 10.0, c3.top() + 26.0), vec2(c3.width() - 20.0, 5.0));
        p.rect_filled(bar, CornerRadius::same(3), dim(25));
        let mut bx = bar.left();
        for m in &u.models { let w = bar.width() * m.2 as f32 / 100.0; p.rect_filled(Rect::from_min_size(pos2(bx, bar.top()), vec2(w, 5.0)), CornerRadius::ZERO, fade(mc(&m.0), alpha)); bx += w; }
        if u.models.is_empty() { p.text(pos2(c3.left() + 10.0, c3.top() + 46.0), Align2::LEFT_CENTER, "sem uso na semana", FontId::proportional(11.0), dim(102)); }
        for (i, m) in u.models.iter().take(4).enumerate() {
            let y = c3.top() + 46.0 + i as f32 * 17.0;
            p.circle_filled(pos2(c3.left() + 13.0, y), 3.0, fade(mc(&m.0), alpha));
            p.text(pos2(c3.left() + 22.0, y), Align2::LEFT_CENTER, format!("{} {}%", m.0, m.2), FontId::proportional(11.0), dim(255));
        }
        let ss = u.sessions_today;
        p.text(pos2(c3.left() + 10.0, c3.bottom() - 14.0), Align2::LEFT_CENTER, format!("{ss} {} hoje", if ss == 1 { "sessão" } else { "sessões" }), FontId::proportional(10.0), dim(115));
    }

    fn draw_sheet(&mut self, ui: &mut egui::Ui, r: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let inner = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 12.0), pos2(r.right() - 16.0, r.bottom() - 16.0));
        p.text(pos2(inner.left(), inner.top() + 10.0), Align2::LEFT_CENTER, "Configurações", bold(17.0), fade(Color32::WHITE, alpha));
        p.text(pos2(inner.left(), inner.top() + 30.0), Align2::LEFT_CENTER, "Notch, Claude Code e o resto do Kon.", FontId::proportional(12.0), fade(Color32::from_white_alpha(128), alpha));
        let xr = Rect::from_center_size(pos2(inner.right() - 12.0, inner.top() + 10.0), vec2(24.0, 24.0));
        let xresp = ui.interact(xr, ui.id().with("sheetx"), Sense::click());
        if xresp.hovered() { p.circle_filled(xr.center(), 12.0, fade(Color32::from_white_alpha(25), alpha)); }
        p.text(xr.center(), Align2::CENTER_CENTER, ic::X, icon_font(13.0), fade(Color32::from_white_alpha(if xresp.hovered() { 255 } else { 128 }), alpha));
        if xresp.clicked() { self.view = View::Panel; }
        let area = Rect::from_min_max(pos2(inner.left(), inner.top() + 48.0), inner.max);
        if ui.rect_contains_pointer(area) { self.sheet_scroll -= ui.input(|i| i.smooth_scroll_delta.y); }
        let pc = p.with_clip_rect(area);
        let mut y = area.top() - self.sheet_scroll;
        let w = area.width();
        let dim = |a: u8| fade(Color32::from_white_alpha(a), alpha);
        let section = |y: &mut f32, t: &str| { pc.text(pos2(area.left(), *y + 9.0), Align2::LEFT_CENTER, t, bold(10.5), dim(102)); *y += 24.0; };
        let row_bg = |y: f32, h: f32| { let rr = Rect::from_min_size(pos2(area.left(), y), vec2(w, h)); pc.rect_filled(rr, CornerRadius::same(12), dim(10)); rr };
        // CLAUDE CODE
        section(&mut y, "CLAUDE CODE");
        let rr = row_bg(y, 44.0);
        let sq = Rect::from_center_size(pos2(rr.left() + 22.0, rr.center().y), vec2(24.0, 24.0));
        pc.rect_filled(sq, CornerRadius::same(7), fade(Color32::from_rgb(168, 85, 247), alpha));
        pc.text(sq.center(), Align2::CENTER_CENTER, ic::ASTERISK, icon_font(13.0), dim(255));
        pc.text(pos2(sq.right() + 10.0, rr.center().y - 7.0), Align2::LEFT_CENTER, "Claude Code", bold(12.0), dim(255));
        let (msg, warn) = match &self.hooks_err { Some(e) => (e.clone(), true), None => (if self.hooks_on { "Conectado · ao vivo na página Claude".into() } else { "Adiciona hooks em ~/.claude/settings.json".into() }, false) };
        pc.text(pos2(sq.right() + 10.0, rr.center().y + 8.0), Align2::LEFT_CENTER, ellipsize_w(ui, &msg, 10.0, w - 150.0), FontId::proportional(10.0), if warn { fade(Color32::from_rgba_unmultiplied(252, 211, 77, 204), alpha) } else { dim(102) });
        let mut rx = rr.right() - 8.0;
        if area.contains(pos2(rr.right() - 30.0, rr.center().y)) && small_pill(ui, &pc, &mut rx, rr.center().y, None, if self.hooks_on { "Desconectar" } else { "Conectar" }, false, alpha) {
            let res = if self.hooks_on { claude::uninstall_hooks() } else { claude::install_hooks() };
            self.hooks_err = res.err();
            self.hooks_on = claude::hooks_state().0;
        }
        y += 50.0;
        if self.hooks_on {
            let rr = row_bg(y, 40.0);
            pc.text(pos2(rr.left() + 12.0, rr.center().y), Align2::LEFT_CENTER, ic::SHIELD_CHECK, icon_font(13.0), dim(128));
            pc.text(pos2(rr.left() + 32.0, rr.center().y - 7.0), Align2::LEFT_CENTER, "Aprovar pedidos do Claude pelo notch", bold(11.5), dim(255));
            pc.text(pos2(rr.left() + 32.0, rr.center().y + 8.0), Align2::LEFT_CENTER, "Negar / Permitir / Sempre, sem ir ao terminal", FontId::proportional(10.0), dim(90));
            if area.contains(pos2(rr.right() - 30.0, rr.center().y)) && toggle_c(ui, &pc, pos2(rr.right() - 30.0, rr.center().y), self.settings.approvals, alpha, Color32::from_rgb(16, 185, 129)).clicked() {
                self.settings.approvals = !self.settings.approvals; self.settings.save();
                self.claude.lock().unwrap().approvals = self.settings.approvals;
            }
            y += 46.0;
            let rr = row_bg(y, 36.0);
            pc.text(pos2(rr.left() + 12.0, rr.center().y), Align2::LEFT_CENTER, "Plano (para a janela de 5 h)", bold(11.5), dim(255));
            let mut plan = self.settings.claude_plan.clone();
            let opts: [(&str, String); 4] = [("Auto", "auto".into()), ("Pro", "pro".into()), ("Max 5×", "max5".into()), ("Max 20×", "max20".into())];
            if area.contains(pos2(rr.right() - 30.0, rr.center().y)) && seg(ui, &pc, pos2(rr.right() - 8.0, rr.center().y), &opts, &mut plan, alpha, "plan") {
                self.settings.claude_plan = plan; self.settings.save(); self.usage_at = None;
            }
            y += 42.0;
        }
        // NOTCH
        y += 4.0;
        section(&mut y, "NOTCH");
        let rr = row_bg(y, 36.0);
        pc.text(pos2(rr.left() + 12.0, rr.center().y), Align2::LEFT_CENTER, "Abrir o painel", bold(11.5), dim(255));
        let mut om = self.settings.open_mode.clone();
        let opts: [(&str, String); 3] = [("Passar o mouse", "hover".into()), ("Com atraso", "delay".into()), ("Clique", "click".into())];
        if area.contains(pos2(rr.right() - 30.0, rr.center().y)) && seg(ui, &pc, pos2(rr.right() - 8.0, rr.center().y), &opts, &mut om, alpha, "open") { self.settings.open_mode = om; self.settings.save(); }
        y += 42.0;
        let rr = row_bg(y, 36.0);
        pc.text(pos2(rr.left() + 12.0, rr.center().y), Align2::LEFT_CENTER, "Histórico da área de transferência", bold(11.5), dim(255));
        let on = self.clips.lock().unwrap().on;
        if area.contains(pos2(rr.right() - 30.0, rr.center().y)) && toggle_c(ui, &pc, pos2(rr.right() - 30.0, rr.center().y), on, alpha, Color32::from_rgb(20, 184, 166)).clicked() {
            clipboard::set_on(&self.clips, !on); self.settings.clipboard_on = !on; self.settings.save();
        }
        y += 42.0;
        let rr = row_bg(y, 36.0);
        pc.text(pos2(rr.left() + 12.0, rr.center().y), Align2::LEFT_CENTER, "Avisar quando o PC ficar sobrecarregado", bold(11.5), dim(255));
        if area.contains(pos2(rr.right() - 30.0, rr.center().y)) && toggle_c(ui, &pc, pos2(rr.right() - 30.0, rr.center().y), self.settings.sys_alerts, alpha, Color32::from_rgb(16, 185, 129)).clicked() {
            self.settings.sys_alerts = !self.settings.sys_alerts; self.settings.save(); self.sys_alerts.store(self.settings.sys_alerts, Ordering::Relaxed);
        }
        y += 50.0;
        pc.text(pos2(area.left(), y), Align2::LEFT_CENTER, format!("Kon nativo (Rust) {} · versão de testes", env!("CARGO_PKG_VERSION")), FontId::proportional(10.5), dim(90));
        y += 20.0;
        let total = y + self.sheet_scroll - area.top();
        self.sheet_scroll = self.sheet_scroll.clamp(0.0, (total - area.height()).max(0.0));
    }

    fn page_files(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let n = self.stash.list.len();
        let y0 = body.top() + 9.0;
        p.text(pos2(body.left(), y0), Align2::LEFT_CENTER, "Arquivos", bold(15.0), fade(Color32::WHITE, alpha));
        let sub = if n == 0 { "gaveta vazia".to_string() } else { format!("{n} {}", if n == 1 { "item guardado" } else { "itens guardados" }) };
        p.text(pos2(body.left() + text_w(ui, "Arquivos", 15.0) + 8.0, y0 + 1.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), fade(Color32::from_white_alpha(102), alpha));
        p.text(pos2(body.right(), y0), Align2::RIGHT_CENTER, "Clique duas vezes para abrir · nada é enviado para IA", FontId::proportional(10.5), fade(Color32::from_white_alpha(90), alpha));
        let area = Rect::from_min_max(pos2(body.left(), body.top() + 26.0), body.max);
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if n == 0 || dragging {
            if dragging { p.rect_filled(area, CornerRadius::same(16), fade(Color32::from_white_alpha(10), alpha)); }
            dashed_rr(&p, area, 16.0, fade(Color32::from_white_alpha(if dragging { 90 } else { 38 }), alpha));
            let c = area.center();
            p.text(pos2(c.x, c.y - 20.0), Align2::CENTER_CENTER, ic::DOWNLOAD, icon_font(20.0), fade(Color32::from_white_alpha(90), alpha));
            p.text(pos2(c.x, c.y + 2.0), Align2::CENTER_CENTER, if dragging { "Solte para guardar" } else { "Arraste arquivos ou pastas até a ilha" }, FontId::proportional(13.0), fade(Color32::from_white_alpha(178), alpha));
            p.text(pos2(c.x, c.y + 20.0), Align2::CENTER_CENTER, "Eles ficam guardados aqui até você remover", FontId::proportional(11.0), fade(Color32::from_white_alpha(90), alpha));
            return;
        }
        let cols = 2; let gap = 6.0; let row_h = 46.0;
        let cw = (area.width() - gap) / cols as f32;
        let rows = (n + 1) / 2;
        let total = rows as f32 * (row_h + gap) - gap;
        if ui.rect_contains_pointer(area) { self.files_scroll -= ui.input(|i| i.smooth_scroll_delta.y); }
        self.files_scroll = self.files_scroll.clamp(0.0, (total - area.height()).max(0.0));
        let pc = p.with_clip_rect(area);
        let mut act: Option<(u8, usize)> = None;
        for (i, it) in self.stash.list.iter().enumerate() {
            let r = Rect::from_min_size(pos2(area.left() + (i % cols) as f32 * (cw + gap), area.top() + (i / cols) as f32 * (row_h + gap) - self.files_scroll), vec2(cw, row_h));
            if r.bottom() < area.top() || r.top() > area.bottom() { continue; }
            let vis = r.intersect(area);
            let resp = ui.interact(vis, ui.id().with(("stash", it.id)), Sense::click()).on_hover_text(&it.path);
            let a = if it.missing { alpha * 0.45 } else { alpha };
            pc.rect_filled(r, CornerRadius::same(12), fade(Color32::from_white_alpha(if resp.hovered() { 23 } else { 13 }), a));
            pc.rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(15), a)), egui::StrokeKind::Inside);
            let ib = Rect::from_min_size(pos2(r.left() + 8.0, r.center().y - 16.0), vec2(32.0, 32.0));
            pc.rect_filled(ib, CornerRadius::same(8), fade(Color32::from_white_alpha(15), a));
            pc.text(ib.center(), Align2::CENTER_CENTER, stash::icon(&it.ext), icon_font(16.0), fade(Color32::from_white_alpha(178), a));
            let tx = ib.right() + 10.0;
            let btns = 3.0 * 24.0 + 6.0;
            let maxw = r.right() - btns - tx;
            let kind = if it.ext == "folder" { "Pasta".to_string() } else { it.ext.to_uppercase() };
            let meta = if it.missing { "não encontrado".to_string() } else { [kind, stash::fmt_size(it.size)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ") };
            pc.text(pos2(tx, r.center().y - 8.0), Align2::LEFT_CENTER, ellipsize_w(ui, &it.name, 12.5, maxw), FontId::proportional(12.5), fade(Color32::WHITE, a));
            pc.text(pos2(tx, r.center().y + 9.0), Align2::LEFT_CENTER, ellipsize_w(ui, &meta, 10.5, maxw), FontId::proportional(10.5), fade(Color32::from_white_alpha(102), a));
            let mut bx = r.right() - 6.0 - 12.0;
            let mut any_btn = false;
            for (k, icon) in [(2u8, ic::TRASH_2), (1, ic::FOLDER_OPEN), (0, ic::EXTERNAL_LINK)] {
                let br = Rect::from_center_size(pos2(bx, r.center().y), vec2(24.0, 24.0));
                let bresp = ui.interact(br.intersect(area), ui.id().with(("stashb", it.id, k)), Sense::click());
                if bresp.hovered() { pc.rect_filled(br, CornerRadius::same(6), fade(Color32::from_white_alpha(25), a)); any_btn = true; }
                let base = if resp.hovered() || ui.rect_contains_pointer(vis) { if bresp.hovered() { 255 } else { 178 } } else { 110 };
                let col = if k == 2 && bresp.hovered() { Color32::from_rgb(252, 165, 165) } else { Color32::from_white_alpha(base) };
                pc.text(br.center(), Align2::CENTER_CENTER, icon, icon_font(12.0), fade(col, a));
                if bresp.clicked() { act = Some((k, i)); }
                bx -= 24.0;
            }
            if resp.double_clicked() && !any_btn && !it.missing { act = Some((0, i)); }
        }
        if let Some((k, i)) = act {
            let it = self.stash.list[i].clone();
            match k { 0 => stash::open(&it.path), 1 => stash::reveal(&it.path), _ => self.stash.remove(it.id) }
        }
    }

    fn page_clip(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let (list, on) = { let g = self.clips.lock().unwrap(); (g.list.clone(), g.on) };
        let teal = Color32::from_rgb(20, 184, 166);
        // cabeçalho
        let y0 = body.top() + 9.0;
        p.text(pos2(body.left(), y0), Align2::LEFT_CENTER, "Área de transferência", bold(15.0), fade(Color32::WHITE, alpha));
        let tx = body.left() + text_w(ui, "Área de transferência", 15.0) + 8.0;
        let n = list.len();
        let sub = if on { format!("{n} {} · senhas e chaves são ignoradas", if n == 1 { "item" } else { "itens" }) } else { "pausado".into() };
        p.text(pos2(tx, y0 + 1.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.0), fade(Color32::from_white_alpha(102), alpha));
        let mut rx = body.right();
        if list.iter().any(|c| !c.pinned) {
            let w = text_w(ui, "Limpar", 11.0) + 34.0;
            let r = Rect::from_min_size(pos2(rx - w, y0 - 12.0), vec2(w, 24.0));
            let resp = ui.interact(r, ui.id().with("clipclear"), Sense::click());
            p.rect_filled(r, CornerRadius::same(12), fade(Color32::from_white_alpha(if resp.hovered() { 30 } else { 18 }), alpha));
            p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, ic::TRASH_2, icon_font(11.0), fade(Color32::WHITE, alpha));
            p.text(pos2(r.left() + 27.0, r.center().y), Align2::LEFT_CENTER, "Limpar", bold(11.0), fade(Color32::WHITE, alpha));
            if resp.clicked() { clipboard::clear(&self.clips); }
            rx = r.left() - 8.0;
        }
        if toggle_c(ui, &p, pos2(rx - 18.0, y0), on, alpha, teal).clicked() {
            clipboard::set_on(&self.clips, !on); self.settings.clipboard_on = !on; self.settings.save();
        }
        p.text(pos2(rx - 42.0, y0), Align2::RIGHT_CENTER, "Gravar", FontId::proportional(10.5), fade(Color32::from_white_alpha(115), alpha));
        let _ = teal;
        let area = Rect::from_min_max(pos2(body.left(), body.top() + 26.0), body.max);
        if list.is_empty() {
            dashed_rr(&p, area, 16.0, fade(Color32::from_white_alpha(38), alpha));
            let c = area.center();
            p.text(pos2(c.x, c.y - 20.0), Align2::CENTER_CENTER, ic::CLIPBOARD_LIST, icon_font(20.0), fade(Color32::from_white_alpha(90), alpha));
            p.text(pos2(c.x, c.y + 2.0), Align2::CENTER_CENTER, "Copie qualquer texto e ele aparece aqui", FontId::proportional(13.0), fade(Color32::from_white_alpha(178), alpha));
            p.text(pos2(c.x, c.y + 20.0), Align2::CENTER_CENTER, "Fica só neste computador", FontId::proportional(11.0), fade(Color32::from_white_alpha(90), alpha));
            return;
        }
        // lista com rolagem
        let hover_area = ui.rect_contains_pointer(area);
        if hover_area { self.clip_scroll -= ui.input(|i| i.smooth_scroll_delta.y); }
        let row_h = 38.0; let gap = 4.0;
        let total = list.len() as f32 * (row_h + gap) - gap;
        self.clip_scroll = self.clip_scroll.clamp(0.0, (total - area.height()).max(0.0));
        let pc = p.with_clip_rect(area);
        let (mut act_copy, mut act_pin, mut act_rm) = (None, None, None);
        for (i, c) in list.iter().enumerate() {
            let y = area.top() + i as f32 * (row_h + gap) - self.clip_scroll;
            let r = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), row_h));
            if r.bottom() < area.top() || r.top() > area.bottom() { continue; }
            let vis = r.intersect(area);
            let resp = ui.interact(vis, ui.id().with(("clip", c.id)), Sense::click());
            pc.rect_filled(r, CornerRadius::same(12), fade(Color32::from_white_alpha(if resp.hovered() { 23 } else { 13 }), alpha));
            pc.rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(15), alpha)), egui::StrokeKind::Inside);
            let mut x = r.left() + 12.0;
            if c.pinned { pc.text(pos2(x, r.center().y), Align2::LEFT_CENTER, ic::PIN, icon_font(11.0), fade(Color32::from_rgb(94, 234, 212), alpha)); x += 18.0; }
            let hm = chrono::DateTime::from_timestamp_millis(c.at).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default();
            let right_pad = if resp.hovered() { 100.0 } else { 46.0 };
            let mut job = egui::text::LayoutJob::single_section(c.text.replace(['\r', '\t'], " "), egui::TextFormat { font_id: FontId::proportional(12.5), color: fade(Color32::WHITE, alpha), line_height: Some(17.0), ..Default::default() });
            job.wrap = egui::text::TextWrapping { max_width: r.right() - right_pad - x, max_rows: 2, break_anywhere: true, overflow_character: Some('…') };
            let g = ui.fonts_mut(|f| f.layout_job(job));
            pc.galley(pos2(x, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
            let mut bx = r.right() - 12.0;
            if resp.hovered() || ui.rect_contains_pointer(vis) {
                for (k, icon) in [(2, ic::TRASH_2), (1, if c.pinned { ic::PIN_OFF } else { ic::PIN })] {
                    let br = Rect::from_center_size(pos2(bx - 12.0, r.center().y), vec2(24.0, 24.0));
                    let bresp = ui.interact(br.intersect(area), ui.id().with(("clipb", c.id, k)), Sense::click());
                    if bresp.hovered() { pc.rect_filled(br, CornerRadius::same(6), fade(Color32::from_white_alpha(25), alpha)); }
                    let col = if k == 2 && bresp.hovered() { Color32::from_rgb(252, 165, 165) } else { Color32::from_white_alpha(if bresp.hovered() { 255 } else { 153 }) };
                    pc.text(br.center(), Align2::CENTER_CENTER, icon, icon_font(12.0), fade(col, alpha));
                    if bresp.clicked() { if k == 2 { act_rm = Some(c.id) } else { act_pin = Some(c.id) } }
                    bx -= 26.0;
                }
            }
            pc.text(pos2(bx - 2.0, r.center().y), Align2::RIGHT_CENTER, hm, FontId::proportional(10.0), fade(Color32::from_white_alpha(90), alpha));
            if resp.clicked() && act_pin.is_none() && act_rm.is_none() { act_copy = Some(c.id); }
        }
        if let Some(id) = act_rm { clipboard::remove(&self.clips, id); }
        else if let Some(id) = act_pin { clipboard::pin(&self.clips, id); }
        else if let Some(id) = act_copy { if clipboard::copy(&self.clips, id) { self.toast("Copiado"); } }
    }

    fn page_system(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let s = self.sys.lock().unwrap().clone();
        let p = ui.painter().clone();
        if s.mem_total == 0 { p.text(body.center(), Align2::CENTER_CENTER, "Lendo o sistema…", FontId::proportional(12.0), fade(Color32::from_white_alpha(115), alpha)); return; }
        let gap = 10.0;
        let half = (body.width() - gap) / 2.0;
        let lft = Rect::from_min_size(body.min, vec2(half, body.height()));
        let rgt = Rect::from_min_max(pos2(lft.right() + gap, body.top()), body.max);
        let cw = (half - 8.0) / 2.0;
        let c1 = Rect::from_min_size(lft.min, vec2(cw, body.height()));
        let col2 = Rect::from_min_max(pos2(c1.right() + 8.0, body.top()), lft.max);
        let m1 = Rect::from_min_size(col2.min, vec2(col2.width(), (body.height() - 8.0) / 2.0));
        let m2 = Rect::from_min_max(pos2(col2.left(), m1.bottom() + 8.0), col2.max);
        for r in [c1, m1, m2, rgt] {
            p.rect_filled(r, CornerRadius::same(16), fade(Color32::from_white_alpha(8), alpha));
            p.rect_stroke(r, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside);
        }
        let comma = |x: String| x.replace('.', ",");
        let level = |v: f32, hi: f32, mid: f32| if v >= hi { Color32::from_rgb(248, 113, 113) } else if v >= mid { Color32::from_rgb(251, 191, 36) } else { Color32::from_rgb(52, 211, 153) };
        // CPU (anel de 64 px, como o SVG do Kon atual)
        let cpu = s.cpu.round();
        let rc = pos2(c1.center().x, c1.center().y - 8.0);
        p.circle_stroke(rc, 26.0, Stroke::new(5.0_f32, fade(Color32::from_white_alpha(25), alpha)));
        arc(&p, rc, 26.0, (cpu / 100.0).clamp(0.0, 1.0), Stroke::new(5.0_f32, fade(level(cpu, 90.0, 70.0), alpha)));
        p.text(rc, Align2::CENTER_CENTER, format!("{cpu:.0}%"), bold(14.0), fade(Color32::WHITE, alpha));
        p.text(pos2(c1.center().x, rc.y + 44.0), Align2::CENTER_CENTER, format!("CPU · {} núcleos", s.cores), FontId::proportional(10.5), fade(Color32::from_white_alpha(115), alpha));
        // memória e disco
        let gbs = |b: u64| comma(format!("{:.1}", b as f64 / 1_073_741_824.0));
        let mem_pct = (100.0 * s.mem_used as f32 / s.mem_total as f32).round();
        let disk_pct = if s.disk_total > 0 { (100.0 * (s.disk_total - s.disk_free) as f32 / s.disk_total as f32).round() } else { 0.0 };
        for (r, icon, title, pct, sub) in [(m1, ic::MEMORY_STICK, "Memória", mem_pct, format!("{} de {} GB", gbs(s.mem_used), gbs(s.mem_total))), (m2, ic::HARD_DRIVE, "Disco C:", disk_pct, format!("{} GB livres", gbs(s.disk_free)))] {
            let y = r.top() + 18.0;
            p.text(pos2(r.left() + 12.0, y), Align2::LEFT_CENTER, icon, icon_font(11.0), fade(Color32::from_white_alpha(128), alpha));
            p.text(pos2(r.left() + 27.0, y), Align2::LEFT_CENTER, title, FontId::proportional(10.5), fade(Color32::from_white_alpha(128), alpha));
            p.text(pos2(r.right() - 12.0, y), Align2::RIGHT_CENTER, format!("{pct:.0}%"), FontId::proportional(10.5), fade(Color32::from_white_alpha(204), alpha));
            let t = Rect::from_min_size(pos2(r.left() + 12.0, y + 10.0), vec2(r.width() - 24.0, 4.0));
            p.rect_filled(t, CornerRadius::same(2), fade(Color32::from_white_alpha(25), alpha));
            p.rect_filled(Rect::from_min_size(t.min, vec2(t.width() * (pct / 100.0).clamp(0.0, 1.0), 4.0)), CornerRadius::same(2), fade(level(pct, 90.0, 75.0), alpha));
            p.text(pos2(r.left() + 12.0, y + 26.0), Align2::LEFT_CENTER, sub, FontId::proportional(10.5), fade(Color32::from_white_alpha(102), alpha));
        }
        // mais pesados
        let y0 = rgt.top() + 18.0;
        p.text(pos2(rgt.left() + 12.0, y0), Align2::LEFT_CENTER, "MAIS PESADOS AGORA", bold(10.0), fade(Color32::from_white_alpha(102), alpha));
        p.text(pos2(rgt.right() - 12.0, y0), Align2::RIGHT_CENTER, format!("ligado há {}h{:02}", s.uptime_s / 3600, (s.uptime_s % 3600) / 60), FontId::proportional(10.0), fade(Color32::from_white_alpha(102), alpha));
        for (i, pr) in s.top.iter().take(4).enumerate() {
            let y = y0 + 24.0 + i as f32 * 20.0;
            let cx = rgt.right() - 12.0; let mx = cx - 52.0;
            p.text(pos2(rgt.left() + 12.0, y), Align2::LEFT_CENTER, ellipsize_w(ui, &pr.name, 11.5, mx - 64.0 - rgt.left()), FontId::proportional(11.5), fade(Color32::WHITE, alpha));
            let mem = if pr.mem_mb >= 1024 { format!("{} GB", comma(format!("{:.1}", pr.mem_mb as f64 / 1024.0))) } else { format!("{} MB", pr.mem_mb) };
            p.text(pos2(mx, y), Align2::RIGHT_CENTER, mem, FontId::proportional(11.5), fade(Color32::from_white_alpha(128), alpha));
            let cc = if pr.cpu >= 50.0 { Color32::from_rgb(252, 165, 165) } else if pr.cpu >= 20.0 { Color32::from_rgb(252, 211, 77) } else { Color32::from_white_alpha(204) };
            p.text(pos2(cx, y), Align2::RIGHT_CENTER, format!("{}%", comma(format!("{:.1}", pr.cpu))), FontId::proportional(11.5), fade(cc, alpha));
        }
        if s.top.is_empty() { p.text(pos2(rgt.left() + 12.0, y0 + 24.0), Align2::LEFT_CENTER, "Calculando…", FontId::proportional(11.0), fade(Color32::from_white_alpha(102), alpha)); }
        let by = rgt.bottom() - 22.0;
        let mut job = egui::text::LayoutJob::single_section("Avisa no notch se a CPU ou a memória ficarem no limite por 30 s".into(), egui::TextFormat { font_id: FontId::proportional(10.0), color: fade(Color32::from_white_alpha(90), alpha), ..Default::default() });
        job.wrap.max_width = rgt.width() - 80.0;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        p.galley(pos2(rgt.left() + 12.0, by - g.size().y / 2.0), g, Color32::WHITE);
        if toggle_c(ui, &p, pos2(rgt.right() - 32.0, by), self.settings.sys_alerts, alpha, Color32::from_rgb(16, 185, 129)).clicked() {
            self.settings.sys_alerts = !self.settings.sys_alerts; self.settings.save();
            self.sys_alerts.store(self.settings.sys_alerts, Ordering::Relaxed);
        }
    }

    fn water(&self) -> u32 { if self.settings.water_day == chrono::Local::now().format("%Y-%m-%d").to_string() { self.settings.water_count } else { 0 } }
    fn set_water(&mut self, n: u32) { self.settings.water_day = chrono::Local::now().format("%Y-%m-%d").to_string(); self.settings.water_count = n.min(8); self.settings.save(); }

    fn page_water(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let w = self.water();
        let pc = p.with_clip_rect(body.expand(16.0));
        for k in 0..16 { pc.circle_filled(pos2(body.left() + 30.0, body.top() + 30.0), 110.0 - k as f32 * 6.5, fade(Color32::from_rgba_unmultiplied(14, 165, 233, 3), alpha)); }
        let x = body.left() + 12.0;
        let top = body.center().y - 66.0;
        p.text(pos2(x, top), Align2::LEFT_CENTER, "ÁGUA", bold(10.0), fade(Color32::from_rgba_unmultiplied(186, 230, 253, 178), alpha));
        let n = w.to_string();
        p.text(pos2(x, top + 54.0), Align2::LEFT_BOTTOM, &n, bold(44.0), fade(Color32::WHITE, alpha));
        p.text(pos2(x + text_w(ui, &n, 44.0) + 6.0, top + 49.0), Align2::LEFT_BOTTOM, "/ 8 copos", FontId::proportional(15.0), fade(Color32::from_white_alpha(115), alpha));
        let ml = |v: u32| { let s = v.to_string(); if s.len() > 3 { format!("{}.{}", &s[..s.len() - 3], &s[s.len() - 3..]) } else { s } };
        p.text(pos2(x, top + 66.0), Align2::LEFT_CENTER, format!("{} ml de 2.000 ml", ml(w * 250)), FontId::proportional(12.0), fade(Color32::from_white_alpha(115), alpha));
        for i in 0..8u32 {
            let r = Rect::from_min_size(pos2(x + i as f32 * 28.0, top + 74.0), vec2(22.0, 30.0)).translate(vec2(0.0, 12.0));
            let resp = ui.interact(r, ui.id().with(("copo", i)), Sense::click());
            let cr = CornerRadius { nw: 3, ne: 3, sw: 7, se: 7 };
            if i < w {
                p.rect_filled(r.expand(2.0), CornerRadius { nw: 4, ne: 4, sw: 8, se: 8 }, fade(Color32::from_rgba_unmultiplied(56, 189, 248, 40), alpha));
                rr_mesh_cr(&p, r, |pt| fade(lerp_c(Color32::from_rgb(103, 232, 249), Color32::from_rgb(14, 165, 233), ((pt.y - r.top()) / r.height()).clamp(0.0, 1.0)), alpha));
            }
            p.rect_stroke(r, cr, Stroke::new(1.0_f32, fade(Color32::from_rgba_unmultiplied(125, 211, 252, 102), alpha)), egui::StrokeKind::Inside);
            if resp.clicked() { self.set_water(if w == i + 1 { i } else { i + 1 }); }
        }
        let bx = body.right() - 36.0;
        let plus = Rect::from_center_size(pos2(bx, body.center().y - 28.0), vec2(48.0, 48.0));
        let minus = Rect::from_center_size(pos2(bx, body.center().y + 28.0), vec2(48.0, 48.0));
        let rp = ui.interact(plus, ui.id().with("wplus"), Sense::click());
        let rm = ui.interact(minus, ui.id().with("wminus"), Sense::click());
        p.circle_filled(plus.center(), 30.0, fade(Color32::from_rgba_unmultiplied(56, 189, 248, 30), alpha));
        p.circle_filled(plus.center(), if rp.is_pointer_button_down_on() { 21.0 } else { 24.0 }, fade(Color32::from_rgb(56, 189, 248), alpha));
        p.text(plus.center(), Align2::CENTER_CENTER, ic::PLUS, icon_font(20.0), fade(Color32::BLACK, alpha));
        p.circle_filled(minus.center(), if rm.is_pointer_button_down_on() { 21.0 } else { 24.0 }, fade(Color32::from_white_alpha(if rm.hovered() { 34 } else { 25 }), alpha));
        p.text(minus.center(), Align2::CENTER_CENTER, ic::MINUS, icon_font(20.0), fade(Color32::WHITE, alpha));
        if rp.clicked() { self.set_water((w + 1).min(8)); }
        if rm.clicked() { self.set_water(w.saturating_sub(1)); }
    }

    fn page_quick(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let y0 = body.top() + 9.0;
        p.text(pos2(body.left(), y0), Align2::LEFT_CENTER, "Atalhos", bold(15.0), fade(Color32::WHITE, alpha));
        p.text(pos2(body.left() + text_w(ui, "Atalhos", 15.0) + 8.0, y0 + 1.0), Align2::LEFT_CENTER, if self.quick.is_empty() { "seus apps, pastas, sites e comandos" } else { "um clique para abrir" }, FontId::proportional(11.0), fade(Color32::from_white_alpha(102), alpha));
        let mut rx = body.right();
        if self.qform.is_none() {
            if small_pill(ui, &p, &mut rx, y0, Some(ic::PLUS), "Adicionar", false, alpha) { self.qform = Some(("app".into(), String::new(), String::new())); self.qedit = false; }
            if !self.quick.is_empty() { let lbl = if self.qedit { "Pronto" } else { "Editar" }; if small_pill(ui, &p, &mut rx, y0, Some(ic::PENCIL), lbl, self.qedit, alpha) { self.qedit = !self.qedit; } }
        }
        let area = Rect::from_min_max(pos2(body.left(), body.top() + 26.0), body.max);
        if let Some((mut kind, mut target, mut label)) = self.qform.take() {
            // tipo
            let kinds = [("app", "Programa"), ("folder", "Pasta"), ("site", "Site"), ("cmd", "Comando")];
            let mut sx = area.left();
            let seg_h = 26.0;
            let total: f32 = kinds.iter().map(|k| text_w(ui, k.1, 11.0) + 22.0).sum::<f32>() + 6.0;
            let sr = Rect::from_min_size(pos2(sx, area.top()), vec2(total, seg_h));
            p.rect_filled(sr, CornerRadius::same(13), fade(Color32::from_white_alpha(15), alpha));
            sx += 3.0;
            for (k, l) in kinds {
                let w = text_w(ui, l, 11.0) + 22.0;
                let r = Rect::from_min_size(pos2(sx, area.top() + 3.0), vec2(w, seg_h - 6.0));
                let resp = ui.interact(r, ui.id().with(("qk", k)), Sense::click());
                if kind == k { p.rect_filled(r, CornerRadius::same(10), fade(Color32::WHITE, alpha)); }
                p.text(r.center(), Align2::CENTER_CENTER, l, bold(11.0), fade(if kind == k { Color32::BLACK } else { Color32::from_white_alpha(178) }, alpha));
                if resp.clicked() { kind = k.into(); }
                sx += w;
            }
            let hint = match kind.as_str() { "app" => "C:\\Program Files\\…\\app.exe", "folder" => "C:\\Users\\você\\Obras", "site" => "sinapi.caixa.gov.br", _ => "code C:\\Projetos\\kon" };
            let can_pick = kind == "app" || kind == "folder";
            let pick_w = if can_pick && cfg!(windows) { text_w(ui, "Escolher…", 11.0) + 26.0 } else { 0.0 };
            let r1 = Rect::from_min_size(pos2(area.left(), area.top() + 34.0), vec2(area.width() - if pick_w > 0.0 { pick_w + 6.0 } else { 0.0 }, 32.0));
            let r2 = Rect::from_min_size(pos2(area.left(), r1.bottom() + 8.0), vec2(area.width(), 32.0));
            let input = |ui: &mut egui::Ui, r: Rect, text: &mut String, hint: &str, focus: bool| -> egui::Response {
                p.rect_filled(r, CornerRadius::same(8), fade(Color32::from_white_alpha(15), alpha));
                let te = egui::TextEdit::singleline(text).hint_text(hint).frame(false).font(FontId::proportional(12.0)).desired_width(r.width() - 20.0);
                let resp = ui.put(r.shrink2(vec2(10.0, 7.0)), te);
                let bc = if resp.has_focus() { Color32::from_rgba_unmultiplied(129, 140, 248, 153) } else { Color32::from_white_alpha(20) };
                p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0_f32, fade(bc, alpha)), egui::StrokeKind::Inside);
                if focus { resp.request_focus(); }
                resp
            };
            let first = ui.memory(|m| m.focused().is_none());
            let _t = input(ui, r1, &mut target, hint, first);
            let _l = input(ui, r2, &mut label, "Nome (opcional)", false);
            if pick_w > 0.0 {
                let pr = Rect::from_min_size(pos2(r1.right() + 6.0, r1.top() + 3.0), vec2(pick_w, 26.0));
                if pill(ui, &p, pr, "Escolher…", false, alpha).clicked() { if let Some(x) = quick::pick(kind == "folder") { target = x; } }
            }
            let by = area.bottom() - 14.0;
            p.text(pos2(area.left(), by), Align2::LEFT_CENTER, if kind == "cmd" { "O comando abre numa janela do Prompt para você ver a saída." } else { "Enter para salvar · Esc cancela" }, FontId::proportional(10.5), fade(Color32::from_white_alpha(90), alpha));
            let mut bx = area.right();
            let save = small_pill(ui, &p, &mut bx, by, None, "Salvar", true, alpha) || ui.input(|i| i.key_pressed(egui::Key::Enter));
            let cancel = small_pill(ui, &p, &mut bx, by, None, "Cancelar", false, alpha) || ui.input(|i| i.key_pressed(egui::Key::Escape));
            if save && !target.trim().is_empty() {
                let lb = if label.trim().is_empty() { quick::auto_label(&kind, &target) } else { label.trim().to_string() };
                self.quick.push(Quick { id: chrono::Utc::now().timestamp_millis() as u64, kind, label: lb, target: target.trim().to_string() });
                quick::save(&self.quick);
            } else if !cancel { self.qform = Some((kind, target, label)); }
            return;
        }
        if self.quick.is_empty() {
            dashed_rr(&p, area, 16.0, fade(Color32::from_white_alpha(38), alpha));
            let c = area.center();
            p.text(pos2(c.x, c.y - 20.0), Align2::CENTER_CENTER, ic::ZAP, icon_font(20.0), fade(Color32::from_white_alpha(90), alpha));
            p.text(pos2(c.x, c.y + 2.0), Align2::CENTER_CENTER, "Adicione o que você abre todo dia", FontId::proportional(13.0), fade(Color32::from_white_alpha(178), alpha));
            p.text(pos2(c.x, c.y + 20.0), Align2::CENTER_CENTER, "Programas, pastas de obra, sites e até comandos", FontId::proportional(11.0), fade(Color32::from_white_alpha(90), alpha));
            return;
        }
        let gap = 6.0; let cw = (area.width() - 4.0 * gap) / 5.0; let ch = 72.0;
        let mut run = None; let mut del = None;
        for (i, q) in self.quick.iter().enumerate() {
            let r = Rect::from_min_size(pos2(area.left() + (i % 5) as f32 * (cw + gap), area.top() + (i / 5) as f32 * (ch + gap)), vec2(cw, ch));
            if r.bottom() > area.bottom() + 1.0 { break; }
            let resp = ui.interact(r, ui.id().with(("q", q.id)), Sense::click()).on_hover_text(&q.target);
            let pulse = if self.qedit { 0.75 + 0.25 * (ui.input(|i| i.time) as f32 * 3.0).sin() } else { 1.0 };
            let a = alpha * pulse;
            p.rect_filled(r, CornerRadius::same(12), fade(Color32::from_white_alpha(if resp.hovered() { 25 } else { 13 }), a));
            p.rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(15), a)), egui::StrokeKind::Inside);
            let ib = Rect::from_center_size(pos2(r.center().x, r.top() + 26.0), vec2(32.0, 32.0));
            p.rect_filled(ib, CornerRadius::same(8), fade(Color32::from_white_alpha(15), a));
            let icon = match q.kind.as_str() { "folder" => ic::FOLDER_OPEN, "site" => ic::LINK_2, "cmd" => ic::TERMINAL, _ => ic::APP_WINDOW };
            p.text(ib.center(), Align2::CENTER_CENTER, icon, icon_font(16.0), fade(Color32::from_white_alpha(190), a));
            p.text(pos2(r.center().x, r.bottom() - 15.0), Align2::CENTER_CENTER, ellipsize_w(ui, &q.label, 11.0, cw - 12.0), FontId::proportional(11.0), fade(Color32::WHITE, a));
            if self.qedit {
                let x = pos2(r.right() - 2.0, r.top() + 2.0);
                let xr = Rect::from_center_size(x, vec2(20.0, 20.0));
                let xresp = ui.interact(xr, ui.id().with(("qx", q.id)), Sense::click());
                p.circle_filled(x, 10.0, fade(Color32::from_rgb(239, 68, 68), alpha));
                p.text(x, Align2::CENTER_CENTER, ic::X, icon_font(11.0), fade(Color32::WHITE, alpha));
                if xresp.clicked() { del = Some(i); }
                ui.ctx().request_repaint_after(Duration::from_millis(60));
            } else if resp.clicked() { run = Some(i); }
        }
        if let Some(i) = del { self.quick.remove(i); quick::save(&self.quick); if self.quick.is_empty() { self.qedit = false; } }
        if let Some(i) = run {
            match quick::run(&self.quick[i]) { Ok(()) => self.view = View::Compact, Err(e) => { self.ctoast = Some((e, Instant::now(), true)); self.view = View::Compact; } }
        }
    }

    fn page_weather(&mut self, ui: &mut egui::Ui, body: Rect, w: Option<&Weather>, alpha: f32) {
        let p = ui.painter().clone();
        let lw = body.width() * 0.4;
        let (txt, icon) = w.map(|w| weather::wmo(w.code, w.is_day)).unwrap_or(("Carregando o clima…", ic::CLOUD));
        let day = w.map_or(true, |w| w.is_day);
        let cy = body.center().y;
        p.text(pos2(body.left(), cy - 34.0), Align2::LEFT_CENTER, icon, icon_font(30.0), fade(if day { Color32::from_rgb(252, 211, 77) } else { Color32::from_rgb(199, 210, 254) }, alpha));
        p.text(pos2(body.left() + 40.0, cy - 34.0), Align2::LEFT_CENTER, w.map_or("--°".into(), |w| format!("{}°", w.temp)), bold(44.0), fade(Color32::WHITE, alpha));
        p.text(pos2(body.left(), cy + 4.0), Align2::LEFT_CENTER, txt, FontId::proportional(13.0), fade(Color32::WHITE, alpha));
        if let Some(w) = w { p.text(pos2(body.left(), cy + 22.0), Align2::LEFT_CENTER, format!("Máx {}° · Mín {}°", w.max, w.min), FontId::proportional(11.5), fade(Color32::from_white_alpha(115), alpha)); }
        // chips
        let mut x = body.left();
        let chips = [(ic::THERMOMETER, w.map(|w| format!("{}°", w.feels))), (ic::DROPLETS, w.map(|w| format!("{}%", w.humidity))), (ic::WIND, w.map(|w| format!("{} km/h", w.wind))), (ic::CLOUD_RAIN, w.map(|w| format!("{}%", w.rain_max)))];
        for (i, v) in chips {
            let v = v.unwrap_or_else(|| "—".into());
            let cw = 26.0 + text_w(ui, &v, 10.5);
            if x + cw > body.left() + lw { break; }
            let rr = Rect::from_min_size(pos2(x, cy + 38.0), vec2(cw, 22.0));
            p.rect_filled(rr, CornerRadius::same(11), fade(Color32::from_white_alpha(18), alpha));
            p.text(pos2(rr.left() + 12.0, rr.center().y), Align2::CENTER_CENTER, i, icon_font(11.0), fade(Color32::from_white_alpha(115), alpha));
            p.text(pos2(rr.left() + 20.0, rr.center().y), Align2::LEFT_CENTER, v, FontId::proportional(10.5), fade(Color32::from_white_alpha(190), alpha));
            x += cw + 4.0;
        }
        // arco do sol
        let right = Rect::from_min_max(pos2(body.left() + lw + 8.0, body.top()), body.max);
        let base_y = right.top() + 66.0;
        let (x0, x1) = (right.left() + 18.0, right.right() - 18.0);
        let mins = |iso: &str| iso.get(11..16).and_then(|h| Some(h[..2].parse::<f32>().ok()? * 60.0 + h[3..].parse::<f32>().ok()?));
        let (sr, ss, nw) = w.map(|w| (mins(&w.sunrise), mins(&w.sunset), mins(&w.now_iso))).unwrap_or((None, None, None));
        let pts: Vec<Pos2> = (0..=40).map(|i| { let t = i as f32 / 40.0; pos2(x0 + (x1 - x0) * t, base_y - (t * std::f32::consts::PI).sin() * 52.0) }).collect();
        p.add(egui::Shape::line(pts.clone(), Stroke::new(2.2_f32, fade(Color32::from_rgb(253, 186, 116).gamma_multiply(if day { 1.0 } else { 0.4 }), alpha))));
        p.add(egui::Shape::dashed_line(&[pos2(right.left(), base_y), pos2(right.right(), base_y)], Stroke::new(1.0_f32, fade(Color32::from_white_alpha(46), alpha)), 3.0, 4.0));
        if let (Some(a), Some(b), Some(n)) = (sr, ss, nw) {
            let t = ((n - a) / (b - a)).clamp(0.0, 1.0);
            let sp = pos2(x0 + (x1 - x0) * t, if day { base_y - (t * std::f32::consts::PI).sin() * 52.0 } else { base_y });
            p.circle_filled(sp, 13.0, fade(Color32::from_rgba_unmultiplied(253, 230, 138, 60), alpha));
            p.circle_filled(sp, 5.0, fade(Color32::from_rgb(255, 247, 214), alpha));
            let hm = |iso: &str| iso.get(11..16).unwrap_or("--:--").to_string();
            let wv = w.unwrap();
            p.text(pos2(x0, base_y + 12.0), Align2::CENTER_CENTER, hm(&wv.sunrise), FontId::proportional(9.5), fade(Color32::from_white_alpha(115), alpha));
            p.text(pos2(x1, base_y + 12.0), Align2::CENTER_CENTER, hm(&wv.sunset), FontId::proportional(9.5), fade(Color32::from_white_alpha(115), alpha));
            let left = (b - n).max(0.0) as i32;
            if day { p.text(pos2((x0 + x1) / 2.0, base_y + 12.0), Align2::CENTER_CENTER, format!("{}h{:02} de sol", left / 60, left % 60), FontId::proportional(9.5), fade(Color32::from_white_alpha(140), alpha)); }
        }
        // próximas horas
        if let Some(w) = w {
            let cw = right.width() / 6.0;
            for (i, h) in w.hours.iter().enumerate() {
                let cx = right.left() + cw * (i as f32 + 0.5); let y = base_y + 30.0;
                p.text(pos2(cx, y), Align2::CENTER_CENTER, format!("{}h", h.hour), FontId::proportional(10.0), fade(Color32::from_white_alpha(115), alpha));
                p.text(pos2(cx, y + 16.0), Align2::CENTER_CENTER, weather::wmo(h.code, h.is_day).1, icon_font(14.0), fade(if h.rain >= 50 { Color32::from_rgb(125, 211, 252) } else { Color32::from_white_alpha(180) }, alpha));
                p.text(pos2(cx, y + 33.0), Align2::CENTER_CENTER, format!("{}°", h.temp), bold(12.0), fade(Color32::WHITE, alpha));
                if h.rain >= 20 { p.text(pos2(cx, y + 47.0), Align2::CENTER_CENTER, format!("{}%", h.rain), FontId::proportional(9.5), fade(Color32::from_rgb(125, 211, 252), alpha)); }
            }
        }
    }

    fn page_todo(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        use chrono::Datelike;
        let p = ui.painter().clone();
        let green = Color32::from_rgb(34, 197, 94);
        let lw = body.width() * 0.22; let mw = body.width() * 0.43;
        let left = Rect::from_min_size(body.min, vec2(lw, body.height()));
        let mid = Rect::from_min_size(pos2(left.right() + 12.0, body.top()), vec2(mw, body.height()));
        let cal = Rect::from_min_max(pos2(mid.right() + 12.0, body.top()), body.max);
        // coluna esquerda
        p.text(pos2(left.left(), left.top() + 8.0), Align2::LEFT_CENTER, "Tarefas", bold(15.0), fade(Color32::WHITE, alpha));
        let (d, n) = (self.tasks.done(), self.tasks.list.len());
        p.text(pos2(left.left(), left.top() + 40.0), Align2::LEFT_CENTER, format!("{d}/{n}"), bold(30.0), fade(Color32::WHITE, alpha));
        let rc = pos2(left.left() + 20.0 + text_w(ui, &format!("{d}/{n}"), 30.0), left.top() + 40.0);
        p.circle_stroke(rc, 11.0, Stroke::new(3.2_f32, fade(Color32::from_rgba_unmultiplied(34, 197, 94, 50), alpha)));
        if n > 0 { arc(&p, rc, 11.0, d as f32 / n as f32, Stroke::new(3.2_f32, fade(green, alpha))); }
        let now = chrono::Local::now();
        let dt = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"][now.weekday().num_days_from_sunday() as usize];
        let mt = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"][now.month0() as usize];
        p.text(pos2(left.left(), left.top() + 64.0), Align2::LEFT_CENTER, format!("{dt}, {} {mt}", now.day()), FontId::proportional(12.0), fade(Color32::from_white_alpha(115), alpha));
        let b1 = Rect::from_min_size(pos2(left.left(), left.bottom() - 62.0), vec2(lw, 28.0));
        let b2 = Rect::from_min_size(pos2(left.left(), left.bottom() - 30.0), vec2(lw, 28.0));
        let r1 = ui.interact(b1, ui.id().with("addtask"), Sense::click());
        p.rect_filled(b1, CornerRadius::same(14), fade(Color32::from_rgba_unmultiplied(34, 197, 94, if r1.hovered() { 64 } else { 38 }), alpha));
        let gcol = fade(Color32::from_rgb(74, 222, 128), alpha);
        let tw = text_w(ui, "Nova tarefa", 11.5);
        p.text(pos2(b1.center().x - tw / 2.0 - 4.0, b1.center().y), Align2::RIGHT_CENTER, ic::PLUS, icon_font(12.0), gcol);
        p.text(pos2(b1.center().x - tw / 2.0, b1.center().y), Align2::LEFT_CENTER, "Nova tarefa", bold(11.5), gcol);
        if r1.clicked() { self.adding = Some(String::new()); }
        if pill(ui, &p, b2, if self.editing { "Pronto" } else { "Editar tarefas" }, self.editing, alpha).clicked() { self.editing = !self.editing; }
        // lista
        let mut y = mid.top();
        let row_h = 26.0;
        if let Some(mut text) = self.adding.take() {
            let rr = Rect::from_min_size(pos2(mid.left(), y), vec2(mid.width(), row_h));
            p.rect_filled(rr, CornerRadius::same(8), fade(Color32::from_white_alpha(20), alpha));
            let te = egui::TextEdit::singleline(&mut text).hint_text("Nova tarefa — Enter para adicionar").frame(false).font(FontId::proportional(12.5)).desired_width(mid.width() - 20.0);
            let resp = ui.put(rr.shrink2(vec2(10.0, 4.0)), te);
            if !resp.has_focus() && !resp.lost_focus() { resp.request_focus(); }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if enter { self.tasks.add(&text); self.adding = None; }
            else if esc { self.adding = None; }
            else { self.adding = Some(text); }
            y += row_h + 4.0;
        }
        let mut toggle_id = None; let mut remove_id = None;
        for t in self.tasks.list.iter() {
            if y + row_h > mid.bottom() { break; }
            let rr = Rect::from_min_size(pos2(mid.left(), y), vec2(mid.width(), row_h));
            let resp = ui.interact(rr, ui.id().with(("task", t.id)), Sense::click());
            p.rect_filled(rr, CornerRadius::same(8), fade(Color32::from_white_alpha(if resp.hovered() { 22 } else { 12 }), alpha));
            let c = pos2(rr.left() + 14.0, rr.center().y);
            if t.done { p.circle_filled(c, 7.0, fade(green, alpha)); p.text(c, Align2::CENTER_CENTER, ic::CHECK, icon_font(9.0), fade(Color32::BLACK, alpha)); }
            else { p.circle_stroke(c, 6.5, Stroke::new(1.5_f32, fade(Color32::from_white_alpha(110), alpha))); }
            let tc = if t.done { Color32::from_white_alpha(90) } else { Color32::WHITE };
            let txt = ellipsize(&t.text, 34);
            let g = p.layout_no_wrap(txt, FontId::proportional(12.5), fade(tc, alpha));
            let tw = g.size().x;
            p.galley(pos2(rr.left() + 28.0, rr.center().y - g.size().y / 2.0), g, Color32::WHITE);
            if t.done { p.line_segment([pos2(rr.left() + 28.0, rr.center().y), pos2(rr.left() + 28.0 + tw, rr.center().y)], Stroke::new(1.0_f32, fade(Color32::from_white_alpha(90), alpha))); }
            if self.editing {
                let del = Rect::from_center_size(pos2(rr.right() - 14.0, rr.center().y), vec2(22.0, 22.0));
                let rd = ui.interact(del, ui.id().with(("del", t.id)), Sense::click());
                p.text(del.center(), Align2::CENTER_CENTER, ic::TRASH_2, icon_font(12.0), fade(if rd.hovered() { Color32::from_rgb(248, 113, 113) } else { Color32::from_white_alpha(110) }, alpha));
                if rd.clicked() { remove_id = Some(t.id); }
            } else if resp.clicked() { toggle_id = Some(t.id); }
            y += row_h + 4.0;
        }
        if let Some(id) = toggle_id { self.tasks.toggle(id); }
        if let Some(id) = remove_id { self.tasks.remove(id); }
        if self.tasks.list.is_empty() && self.adding.is_none() {
            p.text(mid.center(), Align2::CENTER_CENTER, "Nenhuma tarefa. Clique em Nova tarefa.", FontId::proportional(12.0), fade(Color32::from_white_alpha(100), alpha));
        }
        // calendário da semana
        let months = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];
        p.text(pos2(cal.left(), cal.top() + 8.0), Align2::LEFT_CENTER, format!("{} {}", months[now.month0() as usize], now.year()), bold(12.5), fade(Color32::WHITE, alpha));
        let start = now.date_naive() - chrono::Duration::days(now.weekday().num_days_from_sunday() as i64);
        let cw = cal.width() / 7.0;
        for i in 0..7 {
            let day = start + chrono::Duration::days(i);
            let cx = cal.left() + cw * (i as f32 + 0.5);
            p.text(pos2(cx, cal.top() + 32.0), Align2::CENTER_CENTER, ["D", "S", "T", "Q", "Q", "S", "S"][i as usize], FontId::proportional(10.0), fade(Color32::from_white_alpha(100), alpha));
            let today = day == now.date_naive();
            if today { p.circle_filled(pos2(cx, cal.top() + 52.0), 11.0, fade(green, alpha)); }
            p.text(pos2(cx, cal.top() + 52.0), Align2::CENTER_CENTER, day.day().to_string(), if today { bold(12.0) } else { FontId::proportional(12.0) }, fade(if today { Color32::BLACK } else { Color32::WHITE }, alpha));
        }
    }

    fn page_home(&mut self, ui: &mut egui::Ui, body: Rect, m: &MediaState, alpha: f32) {
        let gap = 10.0;
        let total = body.width() - 2.0 * gap;
        let w1 = total * 1.0 / 3.55; let w2 = total * 1.55 / 3.55;
        let c1 = Rect::from_min_size(body.min, vec2(w1, body.height()));
        let c2 = Rect::from_min_size(pos2(c1.right() + gap, body.top()), vec2(w2, body.height()));
        let c3 = Rect::from_min_max(pos2(c2.right() + gap, body.top()), body.max);
        let p = ui.painter().clone();
        // cartão de música: capa inteira com escurecido embaixo, como no Kon atual
        self.cover(ui, c2, 16, alpha, 40.0);
        gradient_v(&p, Rect::from_min_max(pos2(c2.left(), c2.center().y - 10.0), c2.max), 16.0, Color32::from_rgba_unmultiplied(0, 0, 0, 0), Color32::from_rgba_unmultiplied(0, 0, 0, 215), alpha);
        p.rect_stroke(c2, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(18), alpha)), egui::StrokeKind::Inside);
        p.text(pos2(c2.left() + 14.0, c2.bottom() - 34.0), Align2::LEFT_BOTTOM, ellipsize(if m.active { &m.title } else { "Nada tocando" }, 26), bold(16.0), fade(Color32::WHITE, alpha));
        p.text(pos2(c2.left() + 14.0, c2.bottom() - 14.0), Align2::LEFT_BOTTOM, ellipsize(if m.active { &m.artist } else { "Toque música em qualquer app" }, 34), FontId::proportional(12.0), fade(DIM, alpha));
        if round_button(ui, &p, pos2(c2.right() - 30.0, c2.bottom() - 30.0), 17.0, if m.playing { "\u{e12e}" } else { "\u{e13c}" }, alpha, true).clicked() { self.media.send(MediaCmd::Toggle); }
        // foco
        gradient_rr(&p, c3, 16.0, Color32::from_rgb(46, 16, 74), Color32::from_rgb(10, 10, 10), alpha);
        p.rect_stroke(c3, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(18), alpha)), egui::StrokeKind::Inside);
        p.text(pos2(c3.left() + 14.0, c3.top() + 16.0), Align2::LEFT_CENTER, "FOCO", bold(10.0), fade(Color32::from_rgb(216, 180, 254), alpha));
        p.text(pos2(c3.left() + 14.0, c3.bottom() - 34.0), Align2::LEFT_BOTTOM, mmss(self.focus.left), FontId::monospace(24.0), fade(Color32::WHITE, alpha));
        let st = if self.focus.running_until.is_some() { "Rodando" } else if self.focus.finished_at.is_some() { "Concluído!" } else if self.focus.left < self.focus.total as f32 { "Pausado" } else { "Pronto" };
        p.text(pos2(c3.left() + 14.0, c3.bottom() - 14.0), Align2::LEFT_BOTTOM, st, FontId::proportional(12.0), fade(DIM, alpha));
        if round_button(ui, &p, pos2(c3.right() - 24.0, c3.bottom() - 24.0), 13.0, if self.focus.running_until.is_some() { "\u{e12e}" } else { "\u{e13c}" }, alpha, false).clicked() { self.focus_toggle(); }
        // Claude Code (chega na próxima fase)
        gradient_rr(&p, c1, 16.0, Color32::from_rgb(69, 26, 3), Color32::from_rgb(10, 10, 10), alpha);
        p.rect_stroke(c1, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(18), alpha)), egui::StrokeKind::Inside);
        p.text(pos2(c1.left() + 14.0, c1.top() + 16.0), Align2::LEFT_CENTER, "CLAUDE", bold(10.0), fade(Color32::from_rgb(253, 230, 138), alpha));
        let l = self.claude.lock().unwrap().live.clone();
        p.text(pos2(c1.left() + 14.0, c1.bottom() - 34.0), Align2::LEFT_BOTTOM, mmss(claude_elapsed(&l)), FontId::monospace(24.0), fade(Color32::WHITE, alpha));
        p.text(pos2(c1.left() + 14.0, c1.bottom() - 14.0), Align2::LEFT_BOTTOM, self.claude_label(&l), FontId::proportional(12.0), fade(DIM, alpha));
        if ui.interact(c1, ui.id().with("homeclaude"), Sense::click()).clicked() { self.set_page(Page::Claude); }
    }

    fn page_music(&mut self, ui: &mut egui::Ui, body: Rect, m: &MediaState, alpha: f32) {
        let p = ui.painter().clone();
        let art = Rect::from_min_size(pos2(body.left() + 6.0, body.center().y - 62.0), vec2(124.0, 124.0));
        self.cover(ui, art, 18, alpha, 44.0);
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
        if icon_button(ui, &p, pos2(cx - 56.0, cy), "\u{e15f}", alpha).clicked() { self.media.send(MediaCmd::Prev); }
        if round_button(ui, &p, pos2(cx, cy), 19.0, if m.playing { "\u{e12e}" } else { "\u{e13c}" }, alpha, true).clicked() { self.media.send(MediaCmd::Toggle); }
        if icon_button(ui, &p, pos2(cx + 56.0, cy), "\u{e160}", alpha).clicked() { self.media.send(MediaCmd::Next); }
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

}

/* ───────── desenho ───────── */

fn icon_font(size: f32) -> FontId { FontId::new(size, FontFamily::Name("icons".into())) }
fn text_w(ui: &egui::Ui, t: &str, size: f32) -> f32 { ui.painter().layout_no_wrap(t.to_string(), bold(size), Color32::WHITE).size().x }
/// arco de progresso (fração 0..1) começando no topo
fn arc(p: &egui::Painter, c: Pos2, r: f32, frac: f32, st: Stroke) {
    let n = (48.0 * frac).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n).map(|i| { let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * frac * i as f32 / n as f32; pos2(c.x + r * a.cos(), c.y + r * a.sin()) }).collect();
    p.add(egui::Shape::line(pts, st));
}
/// Retângulo arredondado com degradê diagonal (canto de cima à esquerda → canto de baixo à direita).
fn gradient_rr(p: &egui::Painter, r: Rect, rad: f32, a: Color32, b: Color32, alpha: f32) {
    rr_mesh(p, r, rad, |pt| { let t = (((pt.x - r.left()) / r.width()) * 0.45 + ((pt.y - r.top()) / r.height()) * 0.55).clamp(0.0, 1.0); fade(lerp_c(a, b, t), alpha) });
}
/// Degradê vertical (de cima para baixo), com cantos de baixo arredondados.
fn gradient_v(p: &egui::Painter, r: Rect, rad: f32, top: Color32, bottom: Color32, alpha: f32) {
    rr_mesh(p, r, rad, |pt| fade(lerp_c(top, bottom, ((pt.y - r.top()) / r.height()).clamp(0.0, 1.0)), alpha));
}
fn lerp_c(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
/// Polígono convexo do retângulo arredondado, em leque a partir do centro, com cor por vértice.
fn rr_mesh(p: &egui::Painter, r: Rect, rad: f32, color: impl Fn(Pos2) -> Color32) {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0);
    let mut pts = vec![];
    let corners = [(r.right() - rad, r.bottom() - rad, 0.0f32), (r.left() + rad, r.bottom() - rad, 90.0), (r.left() + rad, r.top() + rad, 180.0), (r.right() - rad, r.top() + rad, 270.0)];
    for (cx, cy, start) in corners {
        for i in 0..=8 { let a = (start + i as f32 * 90.0 / 8.0).to_radians(); pts.push(pos2(cx + rad * a.cos(), cy + rad * a.sin())); }
    }
    let mut mesh = egui::Mesh::default();
    let c = r.center();
    mesh.colored_vertex(c, color(c));
    for pt in &pts { mesh.colored_vertex(*pt, color(*pt)); }
    let n = pts.len() as u32;
    for i in 0..n { mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n); }
    p.add(egui::Shape::mesh(mesh));
}
/// Borda tracejada arredondada (estado vazio, como o border-dashed do Kon atual).
fn dashed_rr(p: &egui::Painter, r: Rect, rad: f32, col: Color32) {
    let mut pts = vec![];
    let corners = [(r.right() - rad, r.bottom() - rad, 0.0f32), (r.left() + rad, r.bottom() - rad, 90.0), (r.left() + rad, r.top() + rad, 180.0), (r.right() - rad, r.top() + rad, 270.0)];
    for (cx, cy, st) in corners { for i in 0..=6 { let a = (st + i as f32 * 15.0).to_radians(); pts.push(pos2(cx + rad * a.cos(), cy + rad * a.sin())); } }
    pts.push(pts[0]);
    for s in egui::Shape::dashed_line(&pts, Stroke::new(1.0_f32, col), 4.0, 3.0) { p.add(s); }
}
/// Corta o texto com "…" para caber na largura (em pontos).
fn ellipsize_w(ui: &egui::Ui, s: &str, size: f32, max: f32) -> String {
    if text_w(ui, s, size) <= max { return s.to_string(); }
    let ch: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, ch.len());
    while lo < hi { let mid = (lo + hi + 1) / 2; let t: String = ch[..mid].iter().collect::<String>() + "…"; if text_w(ui, &t, size) <= max { lo = mid } else { hi = mid - 1 } }
    ch[..lo].iter().collect::<String>() + "…"
}
fn ellipsize_chars(s: &str, n: usize) -> String {
    let n = n.max(2);
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
fn claude_elapsed(l: &claude::Live) -> f32 {
    match (l.started, l.done_at) { (Some(s), Some(d)) => (d - s).as_secs_f32(), (Some(s), None) => s.elapsed().as_secs_f32(), _ => 0.0 }
}
/// Botãozinho em pílula do cabeçalho (PillBtn do Kon atual), alinhado à direita a partir de `rx`.
fn small_pill(ui: &mut egui::Ui, p: &egui::Painter, rx: &mut f32, cy: f32, icon: Option<char>, label: &str, primary: bool, a: f32) -> bool {
    let w = text_w(ui, label, 11.0) + if icon.is_some() { 34.0 } else { 24.0 };
    let r = Rect::from_min_size(pos2(*rx - w, cy - 12.0), vec2(w, 24.0));
    let resp = ui.interact(r, ui.id().with(("sp", label)), Sense::click());
    let bg = if primary { Color32::WHITE } else { Color32::from_white_alpha(if resp.hovered() { 30 } else { 18 }) };
    let fg = if primary { Color32::BLACK } else { Color32::WHITE };
    p.rect_filled(r, CornerRadius::same(12), fade(bg, a));
    let mut x = r.left() + 12.0;
    if let Some(i) = icon { p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, i, icon_font(11.0), fade(fg, a)); x += 15.0; }
    p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, label, bold(11.0), fade(fg, a));
    *rx = r.left() - 6.0;
    resp.clicked()
}
/// Copo d'água: cantos de cima 3 px e de baixo 7 px, com degradê.
fn rr_mesh_cr(p: &egui::Painter, r: Rect, color: impl Fn(Pos2) -> Color32) {
    let mut pts = vec![];
    for (cx, cy, rad, st) in [(r.right() - 7.0, r.bottom() - 7.0, 7.0f32, 0.0f32), (r.left() + 7.0, r.bottom() - 7.0, 7.0, 90.0), (r.left() + 3.0, r.top() + 3.0, 3.0, 180.0), (r.right() - 3.0, r.top() + 3.0, 3.0, 270.0)] {
        for i in 0..=6 { let a = (st + i as f32 * 15.0).to_radians(); pts.push(pos2(cx + rad * a.cos(), cy + rad * a.sin())); }
    }
    let mut mesh = egui::Mesh::default();
    let c = r.center(); mesh.colored_vertex(c, color(c));
    for pt in &pts { mesh.colored_vertex(*pt, color(*pt)); }
    let n = pts.len() as u32;
    for i in 0..n { mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n); }
    p.add(egui::Shape::mesh(mesh));
}
fn toggle_c(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, on: bool, a: f32, col: Color32) -> egui::Response {
    let r = Rect::from_center_size(c, vec2(40.0, 22.0));
    let resp = ui.interact(r, ui.id().with(("tgc", (c.x as i32, c.y as i32))), Sense::click());
    p.rect_filled(r, CornerRadius::same(11), fade(if on { col } else { Color32::from_white_alpha(40) }, a));
    let kx = if on { r.right() - 11.0 } else { r.left() + 11.0 };
    p.circle_filled(pos2(kx, c.y), 8.5, fade(Color32::WHITE, a));
    resp
}
/// Controle segmentado (Seg do Kon atual, versão pequena), alinhado à direita em `right`.
fn seg<T: PartialEq + Clone>(ui: &mut egui::Ui, p: &egui::Painter, right: Pos2, opts: &[(&str, T)], val: &mut T, a: f32, id: &str) -> bool {
    let ws: Vec<f32> = opts.iter().map(|o| text_w(ui, o.0, 10.0) + 20.0).collect();
    let total: f32 = ws.iter().sum::<f32>() + 4.0;
    let r = Rect::from_min_size(pos2(right.x - total, right.y - 10.0), vec2(total, 20.0));
    p.rect_filled(r, CornerRadius::same(10), fade(Color32::from_white_alpha(20), a));
    let mut x = r.left() + 2.0; let mut changed = false;
    for (i, (l, v)) in opts.iter().enumerate() {
        let br = Rect::from_min_size(pos2(x, r.top() + 2.0), vec2(ws[i], 16.0));
        let resp = ui.interact(br, ui.id().with((id, i)), Sense::click());
        let on = *val == *v;
        if on { p.rect_filled(br, CornerRadius::same(8), fade(Color32::WHITE, a)); }
        p.text(br.center(), Align2::CENTER_CENTER, *l, bold(10.0), fade(if on { Color32::BLACK } else if resp.hovered() { Color32::from_white_alpha(217) } else { Color32::from_white_alpha(153) }, a));
        if resp.clicked() && !on { *val = v.clone(); changed = true; }
        x += ws[i];
    }
    changed
}
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
    let rad = if view == View::Panel || view == View::Sheet { (r.height() / 10.0).clamp(14.0, 28.0) } else { (r.height() / 2.0).min(17.0) } as u8;
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

/// Tofu com olhar de atenção (olhos arregalados e sobrancelhas âmbar) quando o Claude precisa de você.
fn draw_tofu_mood(p: &egui::Painter, c: Pos2, s: f32, blink: bool, a: f32, attention: bool) {
    draw_tofu(p, c, s, blink && !attention, a);
    if attention {
        for dx in [-5.0f32, 5.0] {
            p.line_segment([pos2(c.x + (dx - 2.2) * s, c.y - 4.6 * s), pos2(c.x + (dx + 2.2) * s, c.y - 4.6 * s - dx.signum() * -0.6 * s)], Stroke::new(1.1 * s, fade(Color32::from_rgb(17, 17, 17), a)));
        }
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
    p.text(c, Align2::CENTER_CENTER, glyph, icon_font(r * 0.85), fade(if primary { Color32::BLACK } else { Color32::WHITE }, a));
    resp
}

fn icon_button(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, glyph: &str, a: f32) -> egui::Response {
    let rect = Rect::from_center_size(c, vec2(32.0, 32.0));
    let resp = ui.interact(rect, ui.id().with(("ib", (c.x as i32, c.y as i32))), Sense::click());
    if resp.hovered() { p.circle_filled(c, 16.0, fade(Color32::from_white_alpha(18), a)); }
    p.text(c, Align2::CENTER_CENTER, glyph, icon_font(18.0), fade(Color32::WHITE, a));
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
    f.font_data.insert("lucide".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/lucide.ttf"))));
    f.families.insert(FontFamily::Name("icons".into()), vec!["lucide".into()]);
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
