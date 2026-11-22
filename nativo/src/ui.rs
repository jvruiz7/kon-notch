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
use crate::ai;
use crate::calc;
use crate::hotkeys::{self, Hot};
use crate::secrets;
use crate::report;
use crate::calendar;
use crate::updater;
use crate::sounds;
use crate::lyrics;
use crate::mascot::{self, Mood, Outfit};
use crate::reminders::{self, Rem};
use crate::translate as tr;
use crate::inuse::{self, InUse};
use crate::usage;
use crate::quick::{self, Quick};
use crate::claude::{self, Status as CStatus};
use crate::stash::{self, Stash};
use std::sync::atomic::{AtomicBool, Ordering};
use eframe::egui::{self, pos2, vec2, emath, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::TimeZone as _;

const WIN_W: f32 = 820.0;
const COMPACT: Vec2 = Vec2::new(168.0, 32.0);
const COMPACT_MUSIC: Vec2 = Vec2::new(250.0, 36.0);
const PANEL: Vec2 = Vec2::new(660.0, 278.0);
const BG: Color32 = Color32::from_rgb(0, 0, 0);
const CARD: Color32 = Color32::from_rgb(20, 20, 22);
const DIM: Color32 = Color32::from_rgba_premultiplied(140, 140, 148, 255);
const DAYS: [&str; 7] = ["DOM", "SEG", "TER", "QUA", "QUI", "SEX", "SÁB"];
const MONTHS: [&str; 12] = ["JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View { Compact, Panel, Perm, Sheet, Ask, Translate, TextAct, Remind, Ocr, Drop }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Sleep, Lock, Home, Quick, Water, Claude, Files, Clip, Music, Weather, Focus, Todo, System, Settings }

impl Page {
    fn id(self) -> &'static str { match self { Page::Home => "home", Page::Music => "music", Page::Weather => "weather", Page::Focus => "focus", Page::Todo => "todo", Page::Clip => "clip", Page::Files => "files", Page::Claude => "claude", Page::Quick => "quick", Page::Water => "water", Page::Sleep => "sleep", Page::Lock => "lock", Page::System => "system", Page::Settings => "settings" } }
    fn from(s: &str) -> Page { match s { "music" => Page::Music, "weather" => Page::Weather, "focus" => Page::Focus, "todo" => Page::Todo, "clip" => Page::Clip, "files" => Page::Files, "claude" => Page::Claude, "quick" => Page::Quick, "water" => Page::Water, "sleep" => Page::Sleep, "lock" => Page::Lock, "system" => Page::System, _ => Page::Home } }
    fn label(self) -> &'static str { match self { Page::Home => "INÍCIO", Page::Music => "MÚSICA", Page::Weather => "CLIMA", Page::Focus => "FOCO", Page::Todo => "TAREFAS", Page::Clip => "CLIPS", Page::Files => "ARQUIVOS", Page::Claude => "CLAUDE", Page::Quick => "ATALHOS", Page::Water => "ÁGUA", Page::Sleep => "DESCANSO", Page::Lock => "TRAVAR", Page::System => "SISTEMA", Page::Settings => "AJUSTES" } }
    fn icon(self) -> char { match self { Page::Home => ic::LAYOUT_GRID, Page::Music => ic::MUSIC_2, Page::Weather => ic::CLOUD_SUN, Page::Focus => ic::TIMER, Page::Todo => ic::LIST_TODO, Page::Clip => ic::CLIPBOARD_LIST, Page::Files => ic::FOLDER_OPEN, Page::Claude => ic::ASTERISK, Page::Quick => ic::ZAP, Page::Water => ic::GLASS_WATER, Page::Sleep => ic::MOON, Page::Lock => ic::LOCK_OPEN, Page::System => ic::CPU, Page::Settings => ic::SETTINGS } }
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
#[derive(Clone, Default)]
struct Tr { text: String, src: String, tgt: String, out: Option<String>, detected: Option<String>, status: u8, engine: String, req: u64, edited: Option<Instant>, sent: String }
// status: 0 parado, 1 traduzindo, 2 pronto, 3 erro
/// Smart OCR: a foto da tela (sem o Kon) e o recorte que você está arrastando.
struct OcrShot { w: u32, h: u32, rgba: Vec<u8>, mx: i32, my: i32, tex: Option<egui::TextureHandle>, start: Option<Pos2>, sel: Option<Rect>, frames: u32 }
#[derive(Clone)]
struct AskFile { path: std::path::PathBuf, name: String, screen: bool, sent: bool }
#[derive(Clone, Default)]
struct OcrRes { reading: bool, text: String, error: String, copied: bool }
#[derive(Clone, Default)]
struct TextAct { text: String, error: String, action: String, status: u8, result: String, req: u64 }
// status: 0 escolhendo, 1 trabalhando, 2 pronto, 3 erro

#[derive(Default)]
struct Hit { rect: Option<Rect>, ppp: f32, inside: bool, compact: bool, follow: bool }

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
    loc: Arc<Mutex<String>>,
    cal_month: bool,
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
    sleep_until: Option<Instant>,
    inuse: Arc<Mutex<InUse>>,
    sheet_tab: u8,
    show_key: bool,
    loc_menu: bool,
    ics_in: Option<String>,
    vol_dragging: bool,
    report_state: u8,
    report_note: String,
    report_shot: bool,
    report_result: Option<(String, Option<std::path::PathBuf>)>,
    ob_step: u8,
    cal: calendar::Shared,
    cal_ctx: egui::Context,
    upd: updater::Shared,
    drag_page: Option<(usize, f32)>,
    ask: ai::Shared,
    ask_q: String,
    ask_rows: f32,
    hot: std::sync::mpsc::Receiver<Hot>,
    key_in: [String; 2],
    model_in: [String; 2],
    copied_at: Option<Instant>,
    dbg_last: String,
    dismissed: Vec<String>,
    meeting_sounded: Option<String>,
    key_sounded: bool,
    fg: Arc<Mutex<(bool, bool, Option<(u32, bool)>)>>,
    hud_at: Option<Instant>,
    vol_seen: Option<(u32, bool)>,
    perm_count: usize,
    dnd_prev: Option<i32>,
    hidden: bool,
    capture_applied: Option<bool>,
    rems: Vec<Rem>,
    due: Vec<Rem>,
    tr: Arc<Mutex<Tr>>,
    ta: Arc<Mutex<TextAct>>,
    art: Option<(String, egui::TextureHandle)>,
    art_color: Option<Color32>,
    lyr: lyrics::Shared,
    lyr_y: f32,
    success_at: Option<Instant>,
    adding: Option<String>,
    editing: bool,
    focusable: bool,
    ocr_shot: Option<OcrShot>,
    ocr: Arc<Mutex<OcrRes>>,
    ocr_q: String,
    ask_file: Option<AskFile>,
    dict: Arc<Mutex<(bool, Option<Result<String, String>>, u64)>>,
    rates: calc::SharedRates,
    drop_zone: u8,
    diag_at: Option<Instant>,
    diag_n: u32,
    drag_req: Option<String>,
    dragging_out: Option<Instant>,
    drop_prev: Option<View>,
    drop_rects: [Rect; 3],
    zip_res: Arc<Mutex<Option<Result<std::path::PathBuf, String>>>>,
    _tray: Option<tray::Tray>,
    quit: bool,
}

impl KonApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let settings = Settings::load();
        if let Some(gl) = cc.gl.as_ref() { use eframe::glow::HasContext; unsafe { crate::platform::log(&format!("OpenGL: {} · {} · {}", gl.get_parameter_string(eframe::glow::VENDOR), gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION))); } }
        let approvals0 = settings.approvals;
        let ctx = cc.egui_ctx.clone();
        let media = Media::start(move || ctx.request_repaint());
        let ctx2 = cc.egui_ctx.clone();
        let loc = Arc::new(Mutex::new(settings.loc_id.clone()));
        let weather = weather::start(loc.clone(), move || ctx2.request_repaint());
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
            view: match std::env::var("KON_DEMO").as_deref() { Ok("painel") => View::Panel, Ok("traduzir") => View::Translate, Ok("texto") => View::TextAct, Ok("ajustes") => View::Sheet, Ok("perguntar") => View::Ask, Ok("soltar") => View::Drop, _ => View::Compact },
            w: Spring::new(COMPACT.x), h: Spring::new(COMPACT.y),
            hit, passthrough: true,
            hover_since: None, leave_since: None,
            placed: false,
            last_frame: Instant::now(),
            blink_at: Instant::now() + Duration::from_millis(3800),
            focus: Focus { total: mins * 60, left: (mins * 60) as f32, running_until: None, finished_at: None },
            weather, loc, cal_month: false, tasks: Tasks::load(), sys, sys_on, art: None, art_color: None, lyr: lyrics::new(), lyr_y: 0.0, success_at: None, clips, clip_scroll: 0.0, toast_msg: None, ctoast: None, quick: quick::load(), qform: None, qedit: false, sys_alerts, usage: usage::new(), claude_usage_tab: std::env::var("KON_USAGE").is_ok(), usage_at: None, sheet_scroll: 0.0, sleep_until: None, inuse: { let c = cc.egui_ctx.clone(); inuse::start(move || c.request_repaint()) }, sheet_tab: if std::env::var("KON_SHEET").as_deref() == Ok("pages") { 1 } else { 0 }, show_key: false, loc_menu: false, ics_in: None, vol_dragging: false, report_state: 0, report_note: String::new(), report_shot: false, report_result: None, ob_step: 0, cal: Arc::new(Mutex::new(calendar::State::default())), cal_ctx: cc.egui_ctx.clone(), upd: updater::new(), drag_page: None, ask: Arc::new(Mutex::new(ai::Ask::default())), ask_q: std::env::var("KON_Q").unwrap_or_default(), ask_rows: 0.0, hot: { let c = cc.egui_ctx.clone(); hotkeys::start(move || c.request_repaint()) }, key_in: [String::new(), String::new()], model_in: [String::new(), String::new()], copied_at: None, dbg_last: String::new(), dismissed: vec![], meeting_sounded: None, key_sounded: false, fg: start_fg_watch(cc.egui_ctx.clone()), hud_at: None, vol_seen: None, perm_count: 0, dnd_prev: None, hidden: false, capture_applied: None, rems: reminders::load(), due: vec![], tr: Arc::new(Mutex::new(Tr { src: "Detect".into(), tgt: "English".into(), ..Default::default() })), ta: Arc::new(Mutex::new(if std::env::var("KON_DEMO").as_deref() == Ok("texto") { TextAct { text: "oi pessoal, segue o orçamento atualizado da obra, qualquer coisa me avisa".into(), ..Default::default() } } else { TextAct::default() })), stash: Stash::load(), claude: { let c = cc.egui_ctx.clone(); claude::start(approvals0, move || c.request_repaint()) }, hooks_on: claude::hooks_state().0, hooks_err: None, files_scroll: 0.0, drop_back: None, adding: None, editing: false, focusable: false, ocr_shot: None, ocr: Arc::new(Mutex::new(OcrRes::default())), ocr_q: String::new(), ask_file: None, dict: Arc::new(Mutex::new((false, None, 0))), rates: calc::rates_new(), drop_zone: 0, diag_at: None, diag_n: 0, drag_req: None, dragging_out: None, drop_prev: None, drop_rects: [Rect::NOTHING; 3], zip_res: Arc::new(Mutex::new(None)),
            _tray: tray::Tray::new(cc.egui_ctx.clone()),
            quit: false,
        }
    }

    fn target_size(&self, m: &MediaState) -> Vec2 {
        // passar o mouse na ilha fechada aumenta 5%, como no Kon atual
        if self.slim(m) { return vec2(130.0, 5.0); }
        let ns = if self.view == View::Compact { match self.settings.notch_size.as_str() { "small" => 0.88, "large" => 1.14, _ => 1.0 } } else { 1.0 };
        let hov = ns * if self.view == View::Compact && self.hit.lock().unwrap().inside && self.ctoast.is_none() { 1.05 } else { 1.0 };
        hov * match self.view {
            View::Panel => PANEL,
            View::Perm => vec2(470.0, 100.0),
            View::Sheet => vec2(440.0, 470.0),
            View::Translate => vec2(560.0, 250.0),
            View::Remind => vec2(470.0, 84.0),
            View::Drop => vec2(470.0, 112.0),
            View::Ocr => {
                let o = self.ocr.lock().unwrap();
                if o.reading { vec2(280.0, 44.0) } else {
                    let src = if o.error.is_empty() { o.text.as_str() } else { o.error.as_str() };
                    let lines: usize = src.lines().map(|l| ((l.chars().count() as f32 / 62.0).ceil() as usize).max(1)).sum::<usize>().max(1);
                    vec2(540.0, 46.0 + 42.0 + (lines as f32 * 21.0 + 20.0).min(180.0))
                }
            }
            View::TextAct => {
                let t = self.ta.lock().unwrap();
                vec2(540.0, if !t.error.is_empty() { 52.0 } else { match t.status { 0 => 98.0, 1 => 120.0, _ => (128.0 + ((t.result.chars().count() as f32 / 70.0).ceil() + t.result.matches('\n').count() as f32).min(6.0) * 18.0).min(250.0) } })
            }
            View::Ask => {
                let g = self.ask.lock().unwrap();
                if g.phase == ai::Phase::Idle { vec2(540.0, 46.0) } else { vec2(540.0, (46.0 + 44.0 + 24.0 + self.ask_rows * 18.0 + if g.sources.is_empty() { 0.0 } else { 30.0 }).clamp(160.0, 360.0)) }
            }
            View::Compact if self.hud() => vec2(300.0, 36.0),
            View::Compact if self.ctoast.is_none() && self.compact_mode(m) == "meeting" => vec2(320.0, 36.0),
            View::Compact => if let Some((t, _, _)) = self.ctoast.as_ref().filter(|c| c.1.elapsed() < Duration::from_secs(4)) { vec2((90.0 + t.chars().count() as f32 * 6.6).clamp(260.0, 420.0), 40.0) } else if self.claude_busy() { vec2(300.0, 36.0) } else if m.active && m.playing { COMPACT_MUSIC } else { COMPACT },
        }
    }

    fn set_page(&mut self, p: Page) {
        self.page = p;
        if std::env::var("KON_DEBUG").is_ok() { eprintln!("page={}", p.id()); }
        if p != Page::Settings { self.settings.page = p.id().into(); self.settings.save(); }
    }

    fn focus_tick(&mut self) {
        if let Some(until) = self.focus.running_until {
            let left = until.saturating_duration_since(Instant::now()).as_secs_f32();
            self.focus.left = left;
            if left <= 0.0 {
                // igual ao Kon atual: para em 0:00, avisa, registra a sessão e o próximo "Começar" recomeça do total
                self.focus.running_until = None; self.focus.left = 0.0; self.focus.finished_at = Some(Instant::now()); self.focus_dnd(false);
                self.toast("Foco concluído — hora de uma pausa"); self.flash_success();
                self.settings.focus_log.push((chrono::Utc::now().timestamp_millis(), (self.focus.total as f32 / 60.0).round().max(1.0) as u32));
                let n = self.settings.focus_log.len(); if n > 300 { self.settings.focus_log.drain(0..n - 300); }
                self.settings.save();
            }
        }
    }
    /// Não incomodar do Windows enquanto o Foco roda (guarda o modo anterior e devolve no fim).
    fn focus_dnd(&mut self, on: bool) {
        if !self.settings.focus_dnd { return; }
        if on { if self.dnd_prev.is_none() { let prev = crate::platform::dnd_get(); if crate::platform::dnd_set(2) { self.dnd_prev = Some(prev.max(0)); } } }
        else if let Some(p) = self.dnd_prev.take() { crate::platform::dnd_set(p); }
    }
    fn focus_toggle(&mut self) {
        if self.focus.running_until.is_some() { self.focus.running_until = None; self.focus_dnd(false); }
        else { if self.focus.left <= 0.0 { self.focus.left = self.focus.total as f32; } self.focus.finished_at = None; self.focus.running_until = Some(Instant::now() + Duration::from_secs_f32(self.focus.left.max(1.0))); self.focus_dnd(true); }
    }
    fn focus_set(&mut self, total: u32, left: f32) {
        self.focus.total = total; self.focus.left = left;
        if self.focus.running_until.is_some() { self.focus.running_until = Some(Instant::now() + Duration::from_secs_f32(left)); }
    }
}

impl eframe::App for KonApp {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] { [0.0, 0.0, 0.0, 0.0] }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        // 1º quadro: centraliza no topo do monitor e tira da barra de tarefas
        if !self.placed {
            if let Some(ms) = ctx.input(|i| i.viewport().monitor_size) {
                if !cfg!(windows) { ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos2(((ms.x - WIN_W) / 2.0).round(), 0.0))); }
                crate::platform::make_tool_window();
                crate::platform::place(self.settings.display == "cursor");
                // primeira vez: boas-vindas; depois de atualizar: limpa a versão antiga; sempre: início com o Windows e atualização
                if std::env::args().any(|a| a == "--atualizado") { updater::cleanup(); self.toast(&format!("Kon atualizado para {}", env!("CARGO_PKG_VERSION"))); }
                if !self.settings.onboarded && std::env::var("KON_DEMO").is_err() { self.view = View::Sheet; self.sheet_tab = 2; self.ob_step = 0; }
                crate::platform::set_autostart(self.settings.auto_start);
                if std::env::var("KON_DEMO").as_deref() == Ok("ocr") { self.start_ocr(ctx); }
                self.placed = true;
            }
        }
        if let Some(t) = &self._tray { for a in t.poll() { match a { tray::Action::Open => { self.view = View::Panel; } tray::Action::Ocr => self.start_ocr(ctx), tray::Action::Update => self.check_update(true), tray::Action::Quit => { self.quit = true; } } } }
        self.ocr_overlay(ctx);
        if self.quit { ctx.send_viewport_cmd(egui::ViewportCommand::Close); return; }
        if let Some(path) = self.drag_req.take() {
            self.dragging_out = Some(Instant::now());
            drag_out(frame, &path);
            self.dragging_out = Some(Instant::now());
        }

        let m = self.media.snapshot();
        self.focus_tick();
        self.sys_on.store(self.view == View::Panel && self.page == Page::System, Ordering::Relaxed);
        self.update_art(ctx, &m);
        if m.active && self.settings.lyrics_on && self.page == Page::Music && self.view == View::Panel { let c = ctx.clone(); lyrics::want(&self.lyr, &m.title, &m.artist, &m.album, m.duration, move || c.request_repaint()); }

        // ── cursor: dentro ou fora do notch? ──
        let island = self.island_rect();
        let inside = {
            let mut h = self.hit.lock().unwrap();
            h.rect = Some(island.expand2(vec2(6.0, 4.0)));
            h.ppp = ctx.pixels_per_point();
            h.compact = self.view == View::Compact;
            h.follow = self.settings.display == "cursor";
            if cfg!(windows) { h.inside } else { ctx.input(|i| i.pointer.hover_pos()).map_or(false, |p| island.contains(p)) }
        };
        if self.passthrough == inside && cfg!(windows) {
            self.passthrough = !inside;
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.passthrough));
        }
        if !matches!(self.page, Page::Sleep | Page::Lock | Page::Settings) && self.settings.pages_off.iter().any(|x| x == self.page.id()) {
            if let Some(f) = self.visible_pages().first() { self.page = *f; }
        }
        // pedido de permissão do Claude Code: a ilha vira o cartão de aprovação
        let has_perm = !self.claude.lock().unwrap().perms.is_empty();
        if has_perm && self.view != View::Perm { self.view = View::Perm; }
        if !has_perm && self.view == View::Perm { self.view = View::Compact; }
        // sons e avisos dos eventos do Claude Code (iguais ao Kon atual)
        {
            let mut g = self.claude.lock().unwrap();
            let np = g.perms.len();
            let done = g.just_done.take().is_some();
            let note = std::mem::take(&mut g.just_note);
            drop(g);
            if np > self.perm_count { self.sfx("attention"); }
            self.perm_count = np;
            if done { self.sfx("done"); self.ctoast = Some(("Claude Code terminou a tarefa".into(), now, false)); self.usage_at = None; }
            if note { self.sfx("attention"); }
        }
        // volume do Windows: mostra a barra por 1,6 s quando muda (não no Não incomodar)
        let vol = self.fg.lock().unwrap().2;
        if vol != self.vol_seen { if self.vol_seen.is_some() && vol.is_some() && !self.settings.dnd { self.hud_at = Some(now); } self.vol_seen = vol; }
        if self.hud() { ctx.request_repaint_after(Duration::from_millis(200)); }
        // esconder em tela cheia (jogo, vídeo, apresentação) — volta sozinho; pedidos e lembretes aparecem mesmo assim
        let full = self.fg.lock().unwrap().0 && self.settings.hide_fullscreen && self.claude.lock().unwrap().perms.is_empty() && self.due.is_empty();
        if full != self.hidden { self.hidden = full; crate::platform::set_visible(!full); crate::platform::log(&format!("tela cheia={full} → {}", if full { "escondido" } else { "visível" })); }
        if self.diag_at.map_or(true, |t| t.elapsed() > Duration::from_secs(if self.diag_n < 10 { 2 } else { 30 })) {
            self.diag_at = Some(now); self.diag_n += 1;
            if self.diag_n <= 60 { let f = *self.fg.lock().unwrap(); crate::platform::log(&format!("quadro: view={:?} tam={:.0}x{:.0} ppp={:.2} fg=(cheia {}, max {}) escondido={} posicionado={} | {}", self.view, self.w.v, self.h.v, ctx.pixels_per_point(), f.0, f.1, self.hidden, self.placed, crate::platform::window_info())); }
        }
        // fora de prints e compartilhamento de tela
        if self.capture_applied != Some(self.settings.hide_in_capture) && self.placed { crate::platform::exclude_from_capture(self.settings.hide_in_capture); self.capture_applied = Some(self.settings.hide_in_capture); }
        // lembretes: dispara os que venceram e mostra o cartão (depois dos pedidos do Claude)
        self.reminder_tick(ctx);
        let act = self.action_kind();
        if !has_perm && act.is_some() && matches!(self.view, View::Compact) { self.view = View::Remind; }
        if act.is_none() && self.view == View::Remind { self.view = View::Compact; }
        if act == Some("meeting") && self.meeting_sounded != self.meeting_soon().map(|e| e.id) { self.meeting_sounded = self.meeting_soon().map(|e| e.id.clone()); self.sfx("meeting"); }
        if act == Some("key") && !self.key_sounded { self.key_sounded = true; self.sfx("attention"); }
        if self.settings.auto_update && std::env::var("KON_DEMO").is_err() { self.check_update(false); }
        // agenda: atualiza a cada 10 min
        if !self.settings.ics_url.is_empty() { self.cal_refresh(false); }
        // arquivos arrastados até a ilha: abre a gaveta e guarda ao soltar
        let (hovering_files, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>()));
        // arquivos arrastados até a ilha: três zonas (Guardar · Perguntar à IA · Compactar), como no Kon atual
        // o arquivo que você está arrastando para fora não abre as zonas de soltar
        let hovering_files = hovering_files && self.dragging_out.map_or(true, |t| t.elapsed() > Duration::from_millis(1500));
        let dropped = if self.dragging_out.map_or(false, |t| t.elapsed() < Duration::from_millis(1500)) { vec![] } else { dropped };
        if hovering_files {
            if self.view != View::Drop { self.drop_prev = Some(self.view); self.view = View::Drop; self.drop_zone = 0; }
            let ppp = ctx.pixels_per_point();
            let pt = crate::platform::cursor_in_window().map(|(x, y)| pos2(x as f32 / ppp, y as f32 / ppp)).or_else(|| ctx.input(|i| i.pointer.hover_pos()));
            if let Some(pt) = pt { if let Some(z) = self.drop_rects.iter().position(|r| r.contains(pt)) { self.drop_zone = z as u8; } }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if !dropped.is_empty() {
            let zone = self.drop_zone;
            self.view = match self.drop_prev.take() { Some(View::Drop) | None => View::Compact, Some(v) => v };
            self.drop_zone = 0;
            match zone {
                1 => {
                    // você escolheu enviar: só agora o arquivo é lido, e só para a IA selecionada
                    self.open_ask();
                    let p0 = dropped[0].clone();
                    self.ask_file = Some(AskFile { name: p0.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), path: p0, screen: false, sent: false });
                    if dropped.len() > 1 { self.toast("Anexei só o primeiro arquivo"); }
                }
                2 => { self.toast("Compactando…"); self.sfx("file"); zip_files(dropped, self.zip_res.clone(), ctx.clone()); }
                _ => {
                    let n = self.stash.add(&dropped);
                    if n > 0 { self.sfx("file"); self.success_at = Some(Instant::now()); }
                    let first = self.stash.list.first().map(|i| i.name.clone()).unwrap_or_default();
                    self.toast(&match n { 0 => "Esses itens já estavam na ilha".to_string(), 1 => format!("Guardado na ilha: {first}"), k => format!("{k} itens guardados na ilha") });
                }
            }
        } else if !hovering_files && self.view == View::Drop && std::env::var("KON_DEMO").as_deref() != Ok("soltar") { self.view = match self.drop_prev.take() { Some(View::Drop) | None => View::Compact, Some(v) => v }; }
        let zr = self.zip_res.lock().unwrap().take();
        if let Some(r) = zr {
            match r {
                Ok(p) => { self.stash.add(&[p.clone()]); self.success_at = Some(Instant::now()); self.toast(&format!("Compactado: {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())); }
                Err(e) => self.toast_err(&e),
            }
        }
        // abrir ao passar o mouse (com um respiro de 90 ms) e fechar ao sair (280 ms de tolerância)
        if inside {
            self.leave_since = None;
            if self.view == View::Compact && self.settings.open_mode != "click" && !has_perm && act.is_none() && !self.settings.locked {
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
        if self.floating() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.view = View::Compact; }
        self.translate_tick(ctx);
        while let Ok(h) = self.hot.try_recv() {
            match h {
                Hot::Ask => self.open_ask(),
                Hot::Panel => { self.view = if self.view == View::Panel { View::Compact } else { View::Panel }; }
                Hot::Clip => { self.page = Page::Clip; self.view = View::Panel; }
                Hot::Text => self.grab_selection(ctx),
                Hot::Ocr => self.start_ocr(ctx),
            }
        }
        // terminou de responder: soma os tokens do chat do Kon e guarda o modelo que funcionou
        {
            let mut g = self.ask.lock().unwrap();
            if g.phase == ai::Phase::Done && g.tokens > 0 { ai::add_kon_tokens(g.tokens); g.tokens = 0; }
            if let Some(m) = g.new_model.take() { self.settings.gemini_model = m; self.settings.save(); }
        }
        let want_focus = ((self.adding.is_some() || self.qform.is_some()) && self.view == View::Panel) || self.view == View::Sheet || self.view == View::Ask || self.view == View::Translate || (self.view == View::Ocr && !self.ocr.lock().unwrap().reading);
        if want_focus != self.focusable { self.focusable = want_focus; crate::platform::set_focusable(want_focus); }
        if self.view == View::Compact { self.adding = None; self.editing = false; self.qform = None; self.qedit = false; }
        // aviso de PC sobrecarregado vindo da thread do sistema
        if let Some(a) = self.sys.lock().unwrap().alert.take() { if !self.settings.dnd { self.ctoast = Some((a, now, true)); } }
        // timer para dormir: pausa a música quando acaba
        if self.sleep_until.map_or(false, |t| now >= t) { self.sleep_until = None; if m.playing { self.media.send(MediaCmd::Toggle); } }
        if self.ctoast.as_ref().map_or(false, |c| c.1.elapsed() >= Duration::from_millis(4300)) { self.ctoast = None; }
        // o aviso só "começa a contar" quando a ilha está fechada (com o painel aberto ele espera)
        if self.view != View::Compact { if let Some(c) = self.ctoast.as_mut() { c.1 = now; } }

        // ── animação do tamanho ──
        if std::env::var("KON_DEBUG").is_ok() { let v = format!("{:?}/{}", self.view, self.page.id()); if v != self.dbg_last { eprintln!("view={v}"); self.dbg_last = v; } }
        let target = self.target_size(&m);
        let a = self.w.step(target.x, dt, 22.0);
        let b = self.h.step(target.y, dt, 22.0);
        let animating = a || b;

        // ── desenho ──
        let blink = now >= self.blink_at;
        if blink { ctx.request_repaint_after(Duration::from_millis(150)); } // a piscada dura 140 ms, nunca um quadro inteiro de 1 s
        if now > self.blink_at + Duration::from_millis(140) { self.blink_at = now + Duration::from_millis(3600 + (now.elapsed().subsec_millis() as u64 % 1400)); }

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let rect = self.island_rect();
            let look = self.island_look(ui, &m);
            if look.processing { ui.ctx().request_repaint_after(Duration::from_millis(50)); }
            draw_island(ui.painter(), rect, self.view, &look);
            let open = ((self.h.v - COMPACT.y) / (PANEL.y - COMPACT.y)).clamp(0.0, 1.0);
            if self.view == View::Remind || self.view == View::Ocr || self.view == View::Drop {}
            else if self.view == View::Perm || (self.w.v > 400.0 && self.h.v < 120.0) {
                let a = ((self.h.v - COMPACT.y) / (100.0 - COMPACT.y)).clamp(0.0, 1.0);
                if self.view == View::Perm && a > 0.6 { self.draw_perm(ui, rect, blink, (a - 0.6) / 0.4); }
            } else if open < 0.35 {
                self.draw_compact(ui, rect, &m, blink, 1.0 - open / 0.35);
            }
            if self.view == View::Remind { if self.h.v > 60.0 { self.draw_remind(ui, rect, blink); } }
            else if self.view == View::Translate { self.draw_translate(ui, rect, blink); }
            else if self.view == View::TextAct { self.draw_textact(ui, rect, blink); }
            else if self.view == View::Ocr { self.draw_ocr(ui, rect, blink); }
            else if self.view == View::Drop { self.draw_drop(ui, rect, blink); }
            else if self.view == View::Ask {
                self.draw_ask(ui, rect, blink);
            } else if self.view == View::Sheet {
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
        // Perguntar flutua 12 px abaixo do topo, como no Kon atual
        Rect::from_min_size(pos2((WIN_W - w) / 2.0, if self.floating() { 12.0 } else { 0.0 }), vec2(w, h))
    }

    fn draw_compact(&mut self, ui: &mut egui::Ui, r: Rect, m: &MediaState, blink: bool, alpha: f32) {
        let p = ui.painter().clone();
        let p = &p;
        if let Some((t, t0, err)) = self.ctoast.clone() {
            if t0.elapsed() < Duration::from_secs(4) && self.view == View::Compact {
                self.tofu(ui, pos2(r.left() + 16.0 + 10.0, r.center().y), 0.78, if err { Mood::Error } else { Mood::Done }, blink, alpha);
                p.text(pos2(r.left() + 16.0 + 20.0 + 10.0, r.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &t, 12.5, r.width() - 62.0), FontId::proportional(12.5), fade(Color32::WHITE, alpha));
                ui.ctx().request_repaint_after(Duration::from_millis(250));
                return;
            }
        }
        if self.slim(m) || r.height() < 12.0 { return; }
        let mood = self.compact_mood(m);
        match self.compact_mode(m) {
            "meeting" if r.width() > COMPACT.x + 40.0 => {
                if let Some(mt) = self.meeting_soon() {
                    let mins = ((mt.start - chrono::Utc::now().timestamp_millis()) as f64 / 60_000.0).ceil() as i64;
                    p.text(pos2(r.left() + 10.0 + 7.5, r.center().y), Align2::CENTER_CENTER, ic::CALENDAR_CLOCK, icon_font(15.0), fade(Color32::from_rgb(252, 211, 77), alpha));
                    let mut right = r.right() - 10.0;
                    if let Some(link) = &mt.link {
                        let bw = text_w(ui, "Entrar", 11.0) + 20.0;
                        let br = Rect::from_min_size(pos2(right - bw, r.center().y - 11.0), vec2(bw, 22.0));
                        let rsp = ui.interact(br, ui.id().with("mtenter"), Sense::click());
                        p.rect_filled(br, CornerRadius::same(11), fade(Color32::WHITE, alpha));
                        p.text(br.center(), Align2::CENTER_CENTER, "Entrar", bold(11.0), fade(Color32::BLACK, alpha));
                        if rsp.clicked() { let _ = quick::open_shell(link); }
                        right = br.left() - 8.0;
                    }
                    let t = if mins > 0 { format!("Em {mins} min · {}", mt.title) } else { format!("Agora · {}", mt.title) };
                    p.text(pos2(r.left() + 10.0 + 15.0 + 8.0, r.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &t, 12.0, right - r.left() - 33.0), FontId::proportional(12.0), fade(Color32::WHITE, alpha));
                }
            }
            "volume" if r.width() > COMPACT.x + 40.0 => {
                let (lv, mu) = self.vol_seen.unwrap_or((0, false));
                let lvl = if mu { 0 } else { lv };
                let ic_ = if mu || lv == 0 { ic::VOLUME_X } else if lv < 50 { ic::VOLUME_1 } else { ic::VOLUME_2 };
                p.text(pos2(r.left() + 16.0 + 8.0, r.center().y), Align2::CENTER_CENTER, ic_, icon_font(16.0), fade(Color32::WHITE, alpha));
                let bar = Rect::from_min_max(pos2(r.left() + 16.0 + 16.0 + 12.0, r.center().y - 2.0), pos2(r.right() - 16.0, r.center().y + 2.0));
                p.rect_filled(bar, CornerRadius::same(2), fade(Color32::from_white_alpha(51), alpha));
                p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * lvl as f32 / 100.0, 4.0)), CornerRadius::same(2), fade(Color32::WHITE, alpha));
            }
            "claude" if r.width() > COMPACT.x + 40.0 => {
                let l = self.claude.lock().unwrap().live.clone();
                let done = l.status == CStatus::Done;
                let t = ui.input(|i| i.time) as f32;
                let c = pos2(r.left() + 11.0 + 7.5, r.center().y);
                let orange = Color32::from_rgb(251, 146, 60);
                p.circle_filled(c, 9.0, fade(Color32::from_rgba_unmultiplied(251, 146, 60, 40), alpha));
                let sc = if done { 1.0 + 0.15 * ((t * std::f32::consts::TAU / 1.2).sin().max(0.0)) } else { 1.0 };
                let mut job = egui::text::LayoutJob::single_section(ic::ASTERISK.to_string(), egui::TextFormat { font_id: icon_font(15.0 * sc), color: fade(orange, alpha), ..Default::default() });
                job.halign = Align::Center;
                let g = ui.fonts_mut(|f| f.layout_job(job));
                let ang = if done { 0.0 } else { (t / 2.4).fract() * std::f32::consts::TAU };
                let half = g.size() / 2.0;
                let rot = emath::Rot2::from_angle(ang);
                p.add(egui::epaint::TextShape::new(c - rot * vec2(0.0, half.y), g, orange).with_angle(ang));
                p.text(pos2(c.x + 7.5 + 6.0, r.center().y), Align2::LEFT_CENTER, self.claude_label(&l), bold(12.5), fade(Color32::WHITE, alpha));
                self.tofu(ui, r.center(), 0.82, mood, blink, alpha);
                p.text(pos2(r.right() - 11.0, r.center().y), Align2::RIGHT_CENTER, mmss(claude_elapsed(&l)), FontId::monospace(12.5), fade(Color32::from_white_alpha(191), alpha));
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
            "timer" if r.width() > COMPACT.x + 40.0 => {
                let c = pos2(r.left() + 9.0 + 10.0, r.center().y);
                p.circle_stroke(c, 8.7, Stroke::new(2.6_f32, fade(Color32::from_rgba_unmultiplied(168, 85, 247, 64), alpha)));
                let frac = if self.focus.total > 0 { self.focus.left / self.focus.total as f32 } else { 0.0 };
                arc(p, c, 8.7, frac.clamp(0.0, 1.0), Stroke::new(2.6_f32, fade(Color32::from_rgb(168, 85, 247), alpha)));
                self.tofu(ui, r.center(), 0.82, mood, blink, alpha);
                p.text(pos2(r.right() - 9.0 - 6.0, r.center().y), Align2::RIGHT_CENTER, mmss(self.focus.left), FontId::monospace(13.0), fade(Color32::from_rgb(216, 180, 254), alpha));
            }
            "usage" if r.width() > COMPACT.x + 40.0 => {
                let (pct, reset) = self.usage.lock().unwrap().usage.as_ref().and_then(|u| u.window).map_or((0, 0), |w| (w.1, w.2));
                self.tofu(ui, pos2(r.left() + 11.0 + 8.0, r.center().y), 0.62, Mood::Attention, blink, alpha);
                let lbl = format!("{pct}% · {}", usage::fmt_left(reset - chrono::Utc::now().timestamp_millis()));
                let tw = text_w(ui, &lbl, 11.5);
                p.text(pos2(r.right() - 13.0, r.center().y), Align2::RIGHT_CENTER, &lbl, FontId::proportional(11.5), fade(Color32::from_white_alpha(230), alpha));
                let bar = Rect::from_min_max(pos2(r.left() + 11.0 + 16.0 + 10.0, r.center().y - 2.0), pos2(r.right() - 13.0 - tw - 10.0, r.center().y + 2.0));
                p.rect_filled(bar, CornerRadius::same(2), fade(Color32::from_white_alpha(51), alpha));
                p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * pct as f32 / 100.0, 4.0)), CornerRadius::same(2), fade(if pct >= 95 { Color32::from_rgb(248, 113, 113) } else { Color32::from_rgb(251, 146, 60) }, alpha));
            }
            "music" if r.width() > COMPACT.x + 20.0 => {
                let sc = if m.playing { 1.0 } else { 0.88 };
                let art = Rect::from_center_size(pos2(r.left() + 7.0 + 11.5, r.center().y), vec2(23.0 * sc, 23.0 * sc));
                if let Some((_, t)) = &self.art { egui::Image::from_texture(t).corner_radius(6).tint(fade(Color32::WHITE, alpha)).paint_at(ui, art); }
                else { gradient_rr(p, art, 6.0, Color32::from_rgb(49, 46, 129), Color32::from_rgb(157, 23, 77), alpha); }
                self.tofu(ui, r.center(), 0.82, mood, blink, alpha);
                draw_bars(p, pos2(r.right() - 13.0 - 15.25, r.center().y), self.art_color.unwrap_or(Color32::from_rgb(255, 122, 184)), m.playing, alpha);
            }
            _ => { self.tofu(ui, r.center(), 0.82, mood, blink, alpha); }
        }
        // microfone/câmera em uso: dois pontinhos no canto, como no iPhone
        let iu = self.inuse.lock().unwrap().clone();
        let mut dx = r.right() - 9.0 - 2.5;
        if !iu.cam.is_empty() { p.circle_filled(pos2(dx, r.top() + 6.5), 2.5, fade(Color32::from_rgb(52, 211, 153), alpha)); dx -= 8.0; }
        if !iu.mic.is_empty() { p.circle_filled(pos2(dx, r.top() + 6.5), 2.5, fade(Color32::from_rgb(251, 146, 60), alpha)); }
    }

    fn draw_panel(&mut self, ui: &mut egui::Ui, r: Rect, m: &MediaState, blink: bool, alpha: f32) {
        let now = chrono::Local::now();
        use chrono::{Datelike, Timelike};
        let p = ui.painter().clone();
        // cabeçalho
        let hy = r.top() + 22.0;
        if self.page == Page::Weather || self.page == Page::Todo {
            if self.page == Page::Weather {
                let w = self.weather.lock().unwrap().clone();
                self.weather_header(ui, r, alpha);
                let body = Rect::from_min_max(pos2(r.left(), r.top() + 42.0), pos2(r.right(), r.bottom() - 46.0));
                self.page_weather(ui, body, w.as_ref(), alpha);
            } else {
                let body = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 12.0), pos2(r.right() - 16.0, r.bottom() - 46.0 - 4.0));
                self.page_todo(ui, body, alpha);
            }
            self.nav(ui, Rect::from_min_max(pos2(r.left() + 16.0, r.bottom() - 46.0), pos2(r.right() - 16.0, r.bottom())), alpha);
            return;
        }
        let date = format!("{} {:02} {}", DAYS[now.weekday().num_days_from_sunday() as usize], now.day(), MONTHS[now.month0() as usize]);
        // data e temperatura: 11 px seminegrito com espaçamento de 0,08em (tracking do Kon atual)
        let dw = spaced(ui, &p, pos2(r.left() + 20.0, hy), &date, 11.0, 0.88, fade(Color32::from_white_alpha(115), alpha));
        let temp = self.weather.lock().unwrap().as_ref().map_or("–°".to_string(), |w| format!("{}°", w.temp));
        spaced(ui, &p, pos2(r.left() + 20.0 + dw + 6.0, hy), &temp, 11.0, 0.88, fade(Color32::WHITE, alpha));
        let clock = format!("{:02}:{:02}", now.hour(), now.minute());
        p.text(pos2(r.right() - 20.0, hy), Align2::RIGHT_CENTER, &clock, FontId::proportional(12.0), fade(Color32::from_white_alpha(230), alpha));
        // microfone e câmera em uso (laranja e verde, como no Kon atual)
        let iu = self.inuse.lock().unwrap().clone();
        let mut ix = r.right() - 20.0 - text_w(ui, &clock, 12.0) - 8.0;
        for (names, icon, col, tip) in [(&iu.cam, ic::CAMERA, Color32::from_rgb(110, 231, 183), "Câmera em uso"), (&iu.mic, ic::MIC, Color32::from_rgb(253, 186, 116), "Microfone em uso")] {
            if names.is_empty() { continue; }
            let t = names.iter().take(2).cloned().collect::<Vec<_>>().join(", ");
            let tw = text_w(ui, &t, 10.5);
            p.text(pos2(ix, hy), Align2::RIGHT_CENTER, &t, FontId::proportional(10.5), fade(col, alpha));
            p.text(pos2(ix - tw - 4.0, hy), Align2::RIGHT_CENTER, icon, icon_font(11.0), fade(col, alpha));
            let rr = Rect::from_min_max(pos2(ix - tw - 18.0, hy - 8.0), pos2(ix, hy + 8.0));
            ui.interact(rr, ui.id().with(tip), Sense::hover()).on_hover_text(tip);
            ix -= tw + 26.0;
        }
        let pm = self.panel_mood(m);
        self.tofu(ui, pos2(r.center().x, hy + 1.0), 0.82, pm, blink, alpha);

        let body = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 42.0), pos2(r.right() - 16.0, r.bottom() - 46.0));
        match self.page {
            Page::Home => self.page_home(ui, body, m, alpha),
            Page::Music => self.page_music(ui, body, m, alpha),
            Page::Focus => self.page_focus(ui, body, alpha),
            Page::Todo => {}
            Page::System => self.page_system(ui, body, alpha),
            Page::Clip => self.page_clip(ui, body, alpha),
            Page::Files => self.page_files(ui, body, alpha),
            Page::Claude => self.page_claude(ui, body, blink, alpha),
            Page::Quick => self.page_quick(ui, body, alpha),
            Page::Water => self.page_water(ui, body, alpha),
            Page::Sleep => self.page_sleep(ui, body, alpha),
            Page::Lock => self.page_lock(ui, body, alpha),
            Page::Weather => {}
            Page::Settings => {}
        }
        self.draw_toast(ui, r, body, alpha);
        self.nav(ui, Rect::from_min_max(pos2(r.left() + 16.0, r.bottom() - 46.0), pos2(r.right() - 16.0, r.bottom())), alpha);
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
        // mesma barra do Kon atual: "+" à esquerda; no meio as páginas, um divisor, Descanso e Travar, e a engrenagem;
        // à direita a bolinha de status (verde com música, roxa com o Foco rodando)
        let pages = self.visible_pages();
        let util = [(Page::Sleep, self.settings.dnd || self.sleep_until.is_some(), Color32::from_rgb(165, 180, 252)), (Page::Lock, self.settings.locked || self.settings.privacy, Color32::from_rgb(252, 211, 77))];
        let wbtn = |ui: &egui::Ui, pg: Page, active: bool| if active { 29.0 + 6.0 + label_w(ui, pg.label()) } else { 29.0 };
        let mut total: f32 = pages.iter().map(|p| wbtn(ui, *p, self.page == *p)).sum::<f32>() + 2.0 * pages.len() as f32;
        total += 9.0 + 2.0 + util.iter().map(|u| wbtn(ui, u.0, self.page == u.0) + 2.0).sum::<f32>() + 28.0;
        // o grupo fica entre o "+" (28 px) e o grupo da direita (40 px), com espaços iguais
        let mut x = bar.center().x - 6.0 - total / 2.0;
        let mut btn = |ui: &mut egui::Ui, x: &mut f32, pg: Page, on: Option<Color32>, me: &mut Self| {
            let active = me.page == pg;
            let w = wbtn(ui, pg, active);
            let rr = Rect::from_min_size(pos2(*x, bar.center().y - 14.0), vec2(w, 28.0));
            let resp = ui.interact(rr, ui.id().with(pg.id()), Sense::click());
            let p = ui.painter();
            if active { p.rect_filled(rr, CornerRadius::same(14), fade(Color32::from_white_alpha(38), alpha)); }
            let icon = if pg == Page::Lock && me.settings.locked { ic::LOCK } else { pg.icon() };
            let col = if active { Color32::WHITE } else if let Some(c) = on { c } else if resp.hovered() { Color32::from_white_alpha(170) } else { Color32::from_white_alpha(102) };
            p.text(pos2(rr.left() + 14.5, rr.center().y), Align2::CENTER_CENTER, icon, icon_font(13.0), fade(col, alpha));
            if active { spaced(ui, &ui.painter().clone(), pos2(rr.left() + 27.0, rr.center().y), pg.label(), 10.5, 1.05, fade(Color32::WHITE, alpha)); }
            if resp.clicked() { me.adding = None; if matches!(pg, Page::Sleep | Page::Lock) { me.page = pg; } else { me.set_page(pg); } }
            *x += w + 2.0;
        };
        for pg in pages.iter().copied() { btn(ui, &mut x, pg, None, self); }
        ui.painter().line_segment([pos2(x + 4.5, bar.center().y - 8.0), pos2(x + 4.5, bar.center().y + 8.0)], Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)));
        x += 9.0 + 2.0;
        for (pg, on, c) in util { btn(ui, &mut x, pg, if on { Some(c) } else { None }, self); }
        // engrenagem
        let g = Rect::from_min_size(pos2(x, bar.center().y - 14.0), vec2(28.0, 28.0));
        let resp = ui.interact(g, ui.id().with("gear"), Sense::click());
        ui.painter().text(g.center(), Align2::CENTER_CENTER, ic::SETTINGS, icon_font(13.0), fade(Color32::from_white_alpha(if resp.hovered() { 204 } else { 102 }), alpha));
        if resp.clicked() { self.view = View::Sheet; self.sheet_scroll = 0.0; self.hooks_on = claude::hooks_state().0; }
        // "+" : nova tarefa
        let plus = Rect::from_center_size(pos2(bar.left() + 14.0, bar.center().y), vec2(28.0, 28.0));
        let r1 = ui.interact(plus, ui.id().with("plus"), Sense::click());
        let p = ui.painter();
        p.circle_filled(plus.center(), 14.0, fade(Color32::from_white_alpha(if r1.hovered() { 30 } else { 16 }), alpha));
        p.text(plus.center(), Align2::CENTER_CENTER, ic::PLUS, icon_font(14.0), fade(Color32::from_white_alpha(180), alpha));
        if r1.clicked() { self.set_page(Page::Todo); self.adding = Some(String::new()); }
        // bolinha de status
        let playing = self.media.snapshot().playing;
        let timer = self.focus.running_until.is_some();
        let pulse = if playing || timer { 0.7 + 0.3 * (ui.input(|i| i.time) as f32 * 4.5).cos() } else { 1.0 };
        let dc = if playing { Color32::from_rgb(52, 211, 153) } else if timer { Color32::from_rgb(192, 132, 252) } else { Color32::from_white_alpha(64) };
        p.circle_filled(pos2(bar.right() - 37.0, bar.center().y), 3.0, fade(dc, alpha * pulse));
        let sr = Rect::from_center_size(pos2(bar.right() - 14.0, bar.center().y), vec2(28.0, 28.0));
        let srsp = ui.interact(sr, ui.id().with("asknav"), Sense::click()).on_hover_text("Perguntar ao Kon (Ctrl+Alt+Space)");
        let p = ui.painter();
        p.circle_filled(sr.center(), 14.0, fade(Color32::from_white_alpha(if srsp.hovered() { 30 } else { 16 }), alpha));
        p.text(sr.center(), Align2::CENTER_CENTER, ic::SEARCH, icon_font(13.0), fade(Color32::from_white_alpha(180), alpha));
        if srsp.clicked() { self.open_ask(); }
    }

    /// Páginas na ordem escolhida em Configurações → Pages (as novas entram no fim, ligadas).
    fn page_ids(&self) -> Vec<String> {
        let all = ["home", "files", "clip", "music", "weather", "focus", "todo", "claude", "quick", "system", "water"];
        let mut v: Vec<String> = self.settings.page_order.iter().filter(|x| all.contains(&x.as_str())).cloned().collect();
        for a in all { if !v.iter().any(|x| x == a) { v.push(a.into()); } }
        v
    }
    fn visible_pages(&self) -> Vec<Page> {
        self.page_ids().iter().filter(|id| !self.settings.pages_off.contains(id)).map(|id| Page::from(id)).collect()
    }

    fn reminder_tick(&mut self, ctx: &egui::Context) {
        let now = chrono::Utc::now().timestamp_millis();
        let mut changed = false;
        let mut fired_now = false;
        for r in self.rems.iter_mut() {
            if r.fired || r.at > now { continue; }
            r.fired = true; changed = true;
            if !self.due.iter().any(|d| d.id == r.id) { self.due.push(r.clone()); fired_now = true; }
        }
        let before = self.rems.len();
        self.rems.retain(|r| !r.fired || now - r.at < 86_400_000);
        if changed || self.rems.len() != before { reminders::save(&self.rems); }
        if fired_now { self.sfx("reminder"); }
        // acorda sozinho na hora do próximo lembrete
        if let Some(next) = self.rems.iter().filter(|r| !r.fired).map(|r| r.at).min() {
            ctx.request_repaint_after(Duration::from_millis(((next - now).max(0) as u64).min(60_000)));
        }
    }

    /// Reunião da agenda começando (de 10 min antes até 5 min depois), como o Kon atual.
    fn meeting_soon(&self) -> Option<calendar::Event> {
        let now = chrono::Utc::now().timestamp_millis();
        self.cal.lock().unwrap().events.iter().find(|e| !e.all_day && e.start - now <= 10 * 60_000 && now < e.start + 5 * 60_000).cloned()
    }
    /// Cartão de ação da ilha: chave copiada > lembrete > atualização pronta > reunião (≤ 2 min).
    fn action_kind(&self) -> Option<&'static str> {
        if self.clips.lock().unwrap().key_offer.is_some() { return Some("key"); }
        if !self.due.is_empty() { return Some("reminder"); }
        if let updater::St::Ready(v) = &self.upd.lock().unwrap().st { if !self.dismissed.contains(&format!("upd-{v}")) { return Some("update"); } }
        if let Some(m) = self.meeting_soon() { if m.start - chrono::Utc::now().timestamp_millis() <= 2 * 60_000 && !self.dismissed.contains(&format!("mt-{}", m.id)) { return Some("meeting"); } }
        None
    }

    fn draw_remind(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool) {
        let p = ui.painter().clone();
        let Some(kind) = self.action_kind() else { return };
        let fmt_hm = |t: i64| chrono::Local.timestamp_millis_opt(t).single().map_or(String::new(), |d| d.format("%H:%M").to_string());
        let (title, sub, mood) = match kind {
            "key" => { let prov = self.clips.lock().unwrap().key_offer.clone().unwrap_or_default(); let has = !(if prov == "anthropic" { &self.settings.anthropic_key_enc } else { &self.settings.gemini_key_enc }).is_empty();
                (format!("Chave do {} copiada", if prov == "anthropic" { "Claude" } else { "Gemini" }), if has { "Trocar pela chave salva no Kon?".to_string() } else { "Usar no Kon? Fica criptografada só neste computador.".to_string() }, Mood::Attention) }
            "reminder" => { let rem = self.due[0].clone(); (rem.text.clone(), format!("Lembrete das {}{}", fmt_hm(rem.at), if self.due.len() > 1 { format!(" · +{}", self.due.len() - 1) } else { String::new() }), Mood::Attention) }
            "update" => { let v = self.upd.lock().unwrap().version.clone(); (format!("Kon {v} pronto para instalar"), "Baixado em segundo plano · reinicia em segundos".to_string(), Mood::Done) }
            _ => { let m = self.meeting_soon().unwrap(); let now = chrono::Utc::now().timestamp_millis(); (m.title.clone(), if m.start > now { format!("Começa às {}{}", fmt_hm(m.start), if m.link.is_some() { " · link pronto" } else { "" }) } else { format!("Começou às {}", fmt_hm(m.start)) }, Mood::Attention) }
        };
        self.tofu(ui, pos2(r.left() + 16.0 + 23.0, r.center().y), 1.45, mood, blink, 1.0);
        let x = r.left() + 16.0 + 46.0 + 12.0;
        let mut rx = r.right() - 14.0;
        let cy = r.center().y;
        let (mut primary, mut secondary) = (false, false);
        match kind {
            "key" => { primary = small_pill(ui, &p, &mut rx, cy, Some(ic::KEY_ROUND), "Usar", true, 1.0); secondary = small_pill(ui, &p, &mut rx, cy, None, "Agora não", false, 1.0); }
            "reminder" => { primary = small_pill(ui, &p, &mut rx, cy, Some(ic::CHECK), "Ok", true, 1.0); secondary = small_pill(ui, &p, &mut rx, cy, None, "Adiar 10 min", false, 1.0); }
            "update" => { primary = small_pill(ui, &p, &mut rx, cy, Some(ic::REFRESH_CW), "Reiniciar", true, 1.0); secondary = small_pill(ui, &p, &mut rx, cy, None, "Depois", false, 1.0); }
            _ => { if self.meeting_soon().and_then(|m| m.link).is_some() { primary = small_pill(ui, &p, &mut rx, cy, Some(ic::VIDEO), "Entrar", true, 1.0); } secondary = small_pill(ui, &p, &mut rx, cy, None, "Ok", false, 1.0); }
        }
        let maxw = rx - 8.0 - x;
        p.text(pos2(x, cy - 9.0), Align2::LEFT_CENTER, ellipsize_w(ui, &title, 13.5, maxw), bold(13.5), Color32::WHITE);
        p.text(pos2(x, cy + 9.0), Align2::LEFT_CENTER, ellipsize_w(ui, &sub, 11.0, maxw), FontId::proportional(11.0), Color32::from_white_alpha(140));
        match kind {
            "key" if primary || secondary => {
                let prov = self.clips.lock().unwrap().key_offer.take().unwrap_or_default();
                self.key_sounded = false;
                if primary {
                    // lê a chave da área de transferência agora, guarda criptografada e limpa a área de transferência
                    let v = clipboard::read_text().trim().to_string();
                    if secrets::valid(&prov, &v) {
                        let enc = secrets::protect(&v);
                        if !enc.is_empty() { if prov == "anthropic" { self.settings.anthropic_key_enc = enc } else { self.settings.gemini_key_enc = enc } self.settings.save(); clipboard::suspend(&self.clips, 1500); clipboard::write_text(""); self.toast("Chave salva e criptografada"); }
                    } else { self.toast_err("A chave não está mais na área de transferência"); }
                }
            }
            "reminder" if primary || secondary => {
                let rem = self.due.remove(0);
                if secondary {
                    if let Some(x) = self.rems.iter_mut().find(|x| x.id == rem.id) { x.at = chrono::Utc::now().timestamp_millis() + 10 * 60_000; x.fired = false; }
                    reminders::save(&self.rems);
                    self.ctoast = Some(("Adiado 10 min".into(), Instant::now(), false));
                }
            }
            "update" => { if primary { self.install_update(); } else if secondary { let v = self.upd.lock().unwrap().version.clone(); self.dismissed.push(format!("upd-{v}")); } }
            _ if primary || secondary => {
                if let Some(m) = self.meeting_soon() { if primary { if let Some(l) = &m.link { let _ = quick::open_shell(l); } } self.dismissed.push(format!("mt-{}", m.id)); }
            }
            _ => {}
        }
    }

    fn outfit(&self) -> Outfit {
        use chrono::Datelike;
        if self.settings.outfit != "auto" { return Outfit::from_id(&self.settings.outfit); }
        let all_done = !self.tasks.list.is_empty() && self.tasks.list.iter().all(|t| t.done);
        if self.claude.lock().unwrap().live.status == CStatus::Working { Outfit::Helmet }
        else if self.media.snapshot().playing { Outfit::Headphones }
        else if self.focus.running_until.is_some() { Outfit::Sunglasses }
        else if all_done { Outfit::Crown }
        else if chrono::Local::now().month() == 12 { Outfit::Santa } else { Outfit::None }
    }
    fn success(&self) -> bool { self.success_at.map_or(false, |t| t.elapsed() < Duration::from_millis(1300)) }
    fn flash_success(&mut self) { self.success_at = Some(Instant::now()); }
    /// Modo da ilha fechada (mesma prioridade do Kon atual).
    fn compact_mode(&self, m: &MediaState) -> &'static str {
        if self.hud() { "volume" } else if self.claude_busy() { "claude" } else if self.meeting_soon().is_some() { "meeting" } else if self.focus.running_until.is_some() { "timer" } else if m.active && m.playing { "music" }
        else if self.usage.lock().unwrap().usage.as_ref().and_then(|u| u.window).map_or(false, |w| w.1 >= 80) { "usage" } else { "idle" }
    }
    fn compact_mood(&self, m: &MediaState) -> Mood {
        if self.success() { return Mood::Done; }
        let l = self.claude.lock().unwrap().live.clone();
        if !l.note.is_empty() { return Mood::Attention; }
        match self.compact_mode(m) { "music" => Mood::Music, "timer" => Mood::Calm, "usage" => Mood::Attention, "claude" => if l.status == CStatus::Done { Mood::Done } else { Mood::Working }, _ => if self.settings.dnd { Mood::Sleep } else { Mood::Idle } }
    }
    fn panel_mood(&self, m: &MediaState) -> Mood {
        let l = self.claude.lock().unwrap().live.clone();
        match self.page {
            Page::Music => Mood::Music, Page::Focus => Mood::Calm,
            Page::Claude => if l.status == CStatus::Done { Mood::Done } else if l.status == CStatus::Working { Mood::Working } else { Mood::Idle },
            Page::Sleep => if self.settings.dnd { Mood::Sleep } else { Mood::Calm },
            _ => self.compact_mood(m),
        }
    }
    fn tofu(&self, ui: &egui::Ui, c: Pos2, size: f32, mood: Mood, blink: bool, a: f32) {
        let t = ui.input(|i| i.time) as f32;
        mascot::draw(ui.painter(), c, size, mood, self.outfit(), blink, t, a);
        if mascot::animates(mood) { ui.ctx().request_repaint_after(Duration::from_millis(33)); }
    }

    fn island_look(&self, ui: &egui::Ui, m: &MediaState) -> IslandLook {
        let askst = self.ask.lock().unwrap().clone();
        let trst = self.tr.lock().unwrap().status;
        let l = self.claude.lock().unwrap().live.clone();
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let processing = dragging || (self.view == View::Ask && matches!(askst.phase, ai::Phase::Thinking | ai::Phase::Streaming))
            || (self.view == View::Translate && trst == 1) || (self.view == View::Compact && self.compact_mode(m) == "claude" && l.status == CStatus::Working);
        let ask_err = self.view == View::Ask && askst.error.as_ref().map_or(false, |e| e.0 != "NO_KEY");
        let toast_err = self.view == View::Compact && self.ctoast.as_ref().map_or(false, |c| c.2 && c.1.elapsed() < Duration::from_secs(4));
        let claude_recent = l.done_at.map_or(false, |d| d.elapsed() < Duration::from_secs(12));
        let tone = if dragging { Some(Color32::from_rgba_unmultiplied(29, 158, 117, 128)) }
            else if ask_err || toast_err { Some(Color32::from_rgba_unmultiplied(127, 119, 221, 140)) }
            else if !l.note.is_empty() || self.view == View::Perm || self.view == View::Remind || (self.view == View::Ask && askst.error.as_ref().map_or(false, |e| e.0 == "NO_KEY")) { Some(Color32::from_rgba_unmultiplied(239, 159, 39, 140)) }
            else if self.success() || (self.view == View::Compact && claude_recent) { Some(Color32::from_rgba_unmultiplied(29, 158, 117, 128)) } else { None };
        let accent = ACCENTS.iter().find(|a| a.0 == self.settings.accent).map_or(ACCENTS[0].2, |a| a.2);
        IslandLook { docked: !self.floating(), tone, accent, glow: self.settings.glow, processing, t: ui.input(|i| i.time) as f32 }
    }

    /// Toca um som do Kon (respeita "Sons" e o Não incomodar, como o original).
    fn sfx(&self, ev: &str) { if self.settings.sounds_on && !self.settings.dnd { sounds::play(ev, &self.settings.sound_pack, self.settings.sound_vol); } }
    fn hud(&self) -> bool { self.hud_at.map_or(false, |t| t.elapsed() < Duration::from_millis(1600)) }
    fn slim(&self, m: &MediaState) -> bool {
        self.view == View::Compact && self.settings.slim_over_max && self.fg.lock().unwrap().1 && !self.hit.lock().unwrap().inside
            && self.ctoast.is_none() && self.compact_mode(m) == "idle" && !self.hud()
    }

    fn cal_refresh(&mut self, force: bool) { let c = self.cal_ctx.clone(); calendar::refresh(&self.cal, &self.settings.ics_url, force, move || c.request_repaint()); }
    fn cal_status(&self) -> String { self.cal.lock().unwrap().status.clone() }
    fn update_ready(&self) -> bool { matches!(self.upd.lock().unwrap().st, updater::St::Ready(_)) }
    fn update_status_text(&self) -> (String, Color32) {
        match self.upd.lock().unwrap().st.clone() {
            updater::St::Checking => ("Procurando…".into(), Color32::from_white_alpha(102)),
            updater::St::Downloading(p) => (format!("Baixando a versão nova · {p}%"), Color32::from_white_alpha(102)),
            updater::St::Ready(v) => (format!("Kon {v} pronto para instalar"), Color32::from_rgb(110, 231, 183)),
            updater::St::Latest => ("Você tem a versão mais nova".into(), Color32::from_white_alpha(102)),
            updater::St::Error(e) => (format!("Erro: {e}"), Color32::from_rgba_unmultiplied(252, 211, 77, 204)),
            updater::St::Idle => ("Procura sozinho ao abrir e a cada 4 h".into(), Color32::from_white_alpha(102)),
        }
    }
    fn check_update(&mut self, force: bool) { let c = self.cal_ctx.clone(); updater::check(&self.upd, force, move || c.request_repaint()); }
    fn install_update(&mut self) {
        match updater::install(&self.upd) { Ok(()) => self.quit = true, Err(e) => self.toast_err(&format!("Não consegui atualizar: {e}")) }
    }
    fn finish_onboarding(&mut self) {
        self.settings.onboarded = true; self.settings.save();
        self.view = View::Compact; self.sheet_tab = 0;
        self.toast(if self.settings.open_mode == "click" { "Tudo pronto · clique no notch para abrir" } else { "Tudo pronto · passe o mouse no notch" });
    }

    fn draw_welcome(&mut self, ui: &mut egui::Ui, area: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let dim = |a: u8| fade(Color32::from_white_alpha(a), alpha);
        const STEPS: [(&str, &str); 3] = [("Escolha sua IA", "Gemini pesquisa na web; Claude é ótimo para texto e código. Dá para pular e configurar depois."), ("Conecte o Claude Code", "O Kon mostra o progresso, deixa você aprovar pedidos pelo notch e acompanha seu uso."), ("O que mostrar", "Escolha as páginas e como o notch abre. Tudo pode mudar depois em Ajustes.")];
        let st = self.ob_step as usize;
        let mood = [Mood::Idle, Mood::Working, Mood::Done][st];
        self.tofu(ui, pos2(area.left() + 18.0, area.top() + 26.0), 1.4, mood, false, alpha);
        let tx = area.left() + 36.0 + 12.0;
        spaced(ui, &p, pos2(tx, area.top() + 7.0), &format!("PASSO {} DE 3", st + 1), 10.5, 1.05, dim(102));
        p.text(pos2(tx, area.top() + 22.0), Align2::LEFT_CENTER, STEPS[st].0, bold(15.0), dim(255));
        let mut job = egui::text::LayoutJob::single_section(STEPS[st].1.into(), egui::TextFormat { font_id: FontId::proportional(11.5), color: dim(128), line_height: Some(15.0), ..Default::default() });
        job.wrap.max_width = area.right() - tx;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        let gh = g.size().y;
        p.galley(pos2(tx, area.top() + 31.0), g, Color32::WHITE);
        let mut y = area.top() + 31.0 + gh + 12.0;
        let w = area.width();
        let green = Color32::from_rgb(16, 185, 129);
        match st {
            0 => {
                p.text(pos2(area.left(), y + 14.0), Align2::LEFT_CENTER, "IA padrão", FontId::proportional(12.0), dim(179));
                let mut pv = self.settings.provider.clone();
                if seg_h(ui, &p, pos2(area.right(), y + 14.0), &[("Gemini", "gemini".to_string()), ("Claude", "claude".to_string())], &mut pv, alpha, "obprov", 28.0, 11.0) { self.settings.provider = pv; self.settings.save(); }
                y += 38.0;
                let k = if self.settings.provider == "claude" { 1 } else { 0 };
                let (prov, label, hint, link) = if k == 1 { ("anthropic", "ANTHROPIC_API_KEY", "sk-ant-…", "https://console.anthropic.com/settings/keys") } else { ("gemini", "GEMINI_API_KEY", "AQ.… ou AIza…", "https://aistudio.google.com/apikey") };
                let has = !(if k == 0 { &self.settings.gemini_key_enc } else { &self.settings.anthropic_key_enc }).is_empty();
                p.text(pos2(area.left(), y + 8.0), Align2::LEFT_CENTER, ic::KEY_ROUND, icon_font(11.0), dim(191));
                p.text(pos2(area.left() + 17.0, y + 8.0), Align2::LEFT_CENTER, label, bold(11.5), dim(191));
                let lr = Rect::from_min_size(pos2(area.right() - 80.0, y), vec2(80.0, 16.0));
                let lrsp = ui.interact(lr, ui.id().with("oblink"), Sense::click());
                let lc = fade(Color32::from_rgba_unmultiplied(125, 211, 252, if lrsp.hovered() { 255 } else { 204 }), alpha);
                p.text(pos2(area.right() - 13.0, y + 8.0), Align2::RIGHT_CENTER, "Obter chave", FontId::proportional(10.5), lc);
                p.text(pos2(area.right(), y + 8.0), Align2::RIGHT_CENTER, ic::EXTERNAL_LINK, icon_font(9.0), lc);
                if lrsp.clicked() { let _ = quick::open_shell(link); }
                y += 20.0;
                let fr = Rect::from_min_size(pos2(area.left(), y), vec2(w, 32.0));
                p.rect_filled(fr, CornerRadius::same(8), dim(15));
                p.rect_stroke(fr, CornerRadius::same(8), Stroke::new(1.0_f32, dim(20)), egui::StrokeKind::Inside);
                let te = egui::TextEdit::singleline(&mut self.key_in[k]).password(true).hint_text(egui::RichText::new(if has { "•••••••• salva — cole para trocar" } else { hint }).color(dim(77))).frame(false).font(FontId::monospace(12.0)).text_color(Color32::WHITE).desired_width(w - 40.0);
                let resp = ui.put(Rect::from_min_max(pos2(fr.left() + 10.0, fr.top() + 7.0), pos2(fr.right() - 30.0, fr.bottom() - 7.0)), te);
                if has { p.text(pos2(fr.right() - 14.0, fr.center().y), Align2::CENTER_CENTER, ic::CHECK, icon_font(12.0), fade(Color32::from_rgb(52, 211, 153), alpha)); }
                if ((resp.lost_focus() || resp.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter))) || (resp.lost_focus() && !self.key_in[k].trim().is_empty()) {
                    let v = self.key_in[k].trim().to_string();
                    if secrets::valid(prov, &v) { let enc = secrets::protect(&v); if !enc.is_empty() { if k == 0 { self.settings.gemini_key_enc = enc } else { self.settings.anthropic_key_enc = enc } self.settings.save(); self.flash_success(); } }
                    else if !v.is_empty() { self.toast_err("Esse não parece o formato da chave"); }
                    self.key_in[k].clear();
                }
                y += 40.0;
                p.text(pos2(area.left(), y), Align2::LEFT_CENTER, "A chave fica criptografada pelo Windows, só neste computador.", FontId::proportional(10.0), dim(90));
            }
            1 => {
                for (icon, t, d) in [(ic::ASTERISK, "Ao vivo no notch", "lendo, editando, testando, concluído"), (ic::SHIELD_CHECK, "Aprovar pelo notch", "Negar, Permitir ou Sempre, sem trocar de janela"), (ic::SPARKLES, "Seu uso", "janela de 5 h, tokens do dia e da semana")] {
                    let rr = Rect::from_min_size(pos2(area.left(), y), vec2(w, 40.0));
                    p.rect_filled(rr, CornerRadius::same(12), dim(10));
                    p.text(pos2(rr.left() + 10.0 + 7.0, rr.center().y), Align2::CENTER_CENTER, icon, icon_font(14.0), fade(Color32::from_rgb(216, 180, 254), alpha));
                    p.text(pos2(rr.left() + 34.0, rr.center().y - 7.0), Align2::LEFT_CENTER, t, FontId::proportional(12.0), dim(255));
                    p.text(pos2(rr.left() + 34.0, rr.center().y + 8.0), Align2::LEFT_CENTER, d, FontId::proportional(10.0), dim(102));
                    y += 40.0 + 10.0;
                }
                let lbl = if self.hooks_on { "Conectado" } else { "Conectar o Claude Code" };
                let bw = text_w(ui, lbl, 11.5) + 24.0 + if self.hooks_on { 16.0 } else { 0.0 };
                let br = Rect::from_min_size(pos2(area.left(), y), vec2(bw, 28.0));
                let brsp = ui.interact(br, ui.id().with("obhooks"), Sense::click());
                p.rect_filled(br, CornerRadius::same(14), if self.hooks_on { dim(20) } else { fade(Color32::from_rgba_unmultiplied(168, 85, 247, if brsp.hovered() { 89 } else { 64 }), alpha) });
                let mut bx = br.left() + 12.0;
                if self.hooks_on { p.text(pos2(bx, br.center().y), Align2::LEFT_CENTER, ic::CHECK, icon_font(11.0), fade(Color32::from_rgb(52, 211, 153), alpha)); bx += 16.0; }
                p.text(pos2(bx, br.center().y), Align2::LEFT_CENTER, lbl, FontId::proportional(11.5), if self.hooks_on { dim(217) } else { fade(Color32::from_rgb(243, 232, 255), alpha) });
                if brsp.clicked() { let r = if self.hooks_on { claude::uninstall_hooks() } else { claude::install_hooks() }; self.hooks_err = r.err(); self.hooks_on = claude::hooks_state().0; }
                let msg = self.hooks_err.clone().unwrap_or_else(|| "Adiciona hooks em ~/.claude/settings.json (com backup)".into());
                p.text(pos2(br.right() + 8.0, br.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &msg, 10.0, area.right() - br.right() - 8.0), FontId::proportional(10.0), dim(102));
            }
            _ => {
                let ids = self.page_ids();
                let cw = (w - 2.0 * 6.0) / 3.0;
                for (i, id) in ids.iter().enumerate() {
                    let rr = Rect::from_min_size(pos2(area.left() + (i % 3) as f32 * (cw + 6.0), y + (i / 3) as f32 * 38.0), vec2(cw, 32.0));
                    let on = !self.settings.pages_off.contains(id);
                    let rsp = ui.interact(rr, ui.id().with(("obpg", id.as_str())), Sense::click());
                    p.rect_filled(rr, CornerRadius::same(8), if on { dim(25) } else { Color32::TRANSPARENT });
                    p.rect_stroke(rr, CornerRadius::same(8), Stroke::new(1.0_f32, dim(if on { 51 } else { 20 })), egui::StrokeKind::Inside);
                    let pg = Page::from(id);
                    let label = match id.as_str() { "clip" => "Área de transferência", "claude" => "Claude Code", _ => "" };
                    let label = if label.is_empty() { let l = pg.label(); let mut c = l.chars(); c.next().map(|f| f.to_string() + &c.as_str().to_lowercase()).unwrap_or_default() } else { label.to_string() };
                    p.text(pos2(rr.left() + 8.0 + 6.0, rr.center().y), Align2::CENTER_CENTER, pg.icon(), icon_font(12.0), dim(if on { 255 } else { 102 }));
                    p.text(pos2(rr.left() + 26.0, rr.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &label, 11.0, cw - 32.0), FontId::proportional(11.0), dim(if on { 255 } else { 102 }));
                    if rsp.clicked() {
                        let n_on = ids.iter().filter(|x| !self.settings.pages_off.contains(x)).count();
                        if on && n_on > 1 { self.settings.pages_off.push(id.clone()); } else if !on { self.settings.pages_off.retain(|x| x != id); }
                        self.settings.save();
                    }
                }
                y += ((ids.len() + 2) / 3) as f32 * 38.0 + 6.0;
                p.text(pos2(area.left(), y + 10.0), Align2::LEFT_CENTER, "Abrir o painel", FontId::proportional(12.0), dim(179));
                let mut om = self.settings.open_mode.clone();
                let opts: [(&str, String); 3] = [("Passar o mouse", "hover".into()), ("Com atraso", "delay".into()), ("Clique", "click".into())];
                if seg(ui, &p, pos2(area.right(), y + 10.0), &opts, &mut om, alpha, "obopen") { self.settings.open_mode = om; self.settings.save(); }
            }
        }
        let _ = green;
        // rodapé: pontinhos e botões
        let fy = area.bottom() - 14.0;
        let mut dx = area.left();
        for i in 0..3 { let wd = if i == st { 20.0 } else { 6.0 }; p.rect_filled(Rect::from_min_size(pos2(dx, fy - 3.0), vec2(wd, 6.0)), CornerRadius::same(3), dim(if i == st { 255 } else { 64 })); dx += wd + 6.0; }
        let mut rx = area.right();
        if small_pill(ui, &p, &mut rx, fy, None, if st < 2 { "Continuar" } else { "Concluir" }, true, alpha) { if st < 2 { self.ob_step += 1; } else { self.finish_onboarding(); return; } }
        if st > 0 { if small_pill(ui, &p, &mut rx, fy, None, "Voltar", false, alpha) { self.ob_step -= 1; } }
        else if small_pill(ui, &p, &mut rx, fy, None, "Pular", false, alpha) { self.finish_onboarding(); }
    }

    fn draw_report(&mut self, ui: &mut egui::Ui, area: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let dim = |a: u8| fade(Color32::from_white_alpha(a), alpha);
        if self.report_state < 2 {
            let tr = Rect::from_min_size(area.min, vec2(area.width(), 120.0));
            p.rect_filled(tr, CornerRadius::same(12), dim(15));
            let te = egui::TextEdit::multiline(&mut self.report_note).hint_text(egui::RichText::new("O que aconteceu? Ex.: cliquei em Música e o painel fechou sozinho").color(dim(77))).frame(false).font(FontId::proportional(12.5)).text_color(Color32::WHITE).char_limit(2000).desired_width(tr.width() - 24.0);
            let resp = ui.put(tr.shrink(12.0), te);
            if self.report_state == 0 { resp.request_focus(); self.report_state = 1; }
            p.rect_stroke(tr, CornerRadius::same(12), Stroke::new(1.0_f32, if resp.has_focus() { fade(Color32::from_rgba_unmultiplied(129, 140, 248, 153), alpha) } else { dim(20) }), egui::StrokeKind::Inside);
            let rr = Rect::from_min_size(pos2(area.left(), tr.bottom() + 10.0), vec2(area.width(), 36.0));
            p.rect_filled(rr, CornerRadius::same(12), dim(10));
            p.text(pos2(rr.left() + 10.0 + 6.5, rr.center().y), Align2::CENTER_CENTER, ic::CAMERA, icon_font(13.0), dim(128));
            p.text(pos2(rr.left() + 32.0, rr.center().y), Align2::LEFT_CENTER, "Incluir um print da tela", FontId::proportional(11.5), dim(255));
            if toggle_c(ui, &p, pos2(rr.right() - 10.0 - 19.0, rr.center().y), self.report_shot, alpha, Color32::from_rgb(16, 185, 129)).clicked() { self.report_shot = !self.report_shot; }
            let mut job = egui::text::LayoutJob::single_section("O relatório leva a versão, os registros de erro e seus ajustes, sem chaves de API, sem o link da agenda e sem o conteúdo dos seus arquivos ou da área de transferência.".into(), egui::TextFormat { font_id: FontId::proportional(10.5), color: dim(102), line_height: Some(14.0), ..Default::default() });
            job.wrap.max_width = area.width();
            let g = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(pos2(area.left(), rr.bottom() + 10.0), g, Color32::WHITE);
            let fy = area.bottom() - 14.0;
            let mut rx = area.right();
            if small_pill(ui, &p, &mut rx, fy, Some(ic::ALERT_TRIANGLE), "Gerar relatório", true, alpha) {
                let extra = serde_json::json!({ "pagina": self.page.id(), "tela": self.settings.display, "hooks": self.hooks_on, "telaCheia": *self.fg.lock().unwrap() == (true, false, None) });
                match report::make(&self.report_note, self.report_shot, &self.settings, extra) {
                    Ok(r) => { self.report_result = Some(r); self.report_state = 2; }
                    Err(e) => self.toast_err(&format!("Não consegui gerar: {e}")),
                }
            }
            if small_pill(ui, &p, &mut rx, fy, None, "Cancelar", false, alpha) { self.sheet_tab = 0; self.report_state = 0; }
        } else {
            let (url, zip) = self.report_result.clone().unwrap_or_default();
            self.tofu(ui, pos2(area.left() + 17.0, area.top() + 20.0), 1.3, Mood::Done, false, alpha);
            p.text(pos2(area.left() + 46.0, area.top() + 12.0), Align2::LEFT_CENTER, "Relatório pronto", bold(14.0), dim(255));
            p.text(pos2(area.left() + 46.0, area.top() + 30.0), Align2::LEFT_CENTER, if zip.is_some() { "Salvei o arquivo .zip na sua Área de Trabalho." } else { "Abra a página para enviar." }, FontId::proportional(11.5), dim(128));
            let mut job = egui::text::LayoutJob::single_section("Abra a página no GitHub, arraste o arquivo para lá e clique em Submit new issue. Também pode mandar o arquivo direto no chat.".into(), egui::TextFormat { font_id: FontId::proportional(11.5), color: dim(153), line_height: Some(15.0), ..Default::default() });
            job.wrap.max_width = area.width();
            let g = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(pos2(area.left(), area.top() + 54.0), g, Color32::WHITE);
            let mut rx = area.left() + 400.0;
            let by = area.top() + 110.0;
            if let Some(z) = &zip { if small_pill(ui, &p, &mut rx, by, Some(ic::FOLDER_OPEN), "Mostrar arquivo", false, alpha) { stash::reveal(&z.to_string_lossy()); } }
            if small_pill(ui, &p, &mut rx, by, Some(ic::EXTERNAL_LINK), "Abrir no GitHub", true, alpha) { let _ = quick::open_shell(&url); }
        }
    }

    fn floating(&self) -> bool { matches!(self.view, View::Ask | View::Translate | View::TextAct | View::Ocr) }

    /// IA para pedidos únicos: a escolhida, ou a que tiver chave.
    fn once_req(&self, system: &str) -> Option<ai::Req> {
        let has = |p: &str| !self.key_for(p).is_empty();
        let pv = if has(&self.settings.provider) { self.settings.provider.clone() } else if has("gemini") { "gemini".into() } else if has("claude") { "claude".into() } else { return None };
        Some(ai::Req { key: self.key_for(&pv), model: if pv == "claude" { self.settings.claude_model.clone() } else { self.settings.gemini_model.clone() }, web: false, provider: pv, system: system.into() })
    }

    fn open_translate(&mut self, text: String) {
        let mut t = self.tr.lock().unwrap();
        let tgt = t.tgt.clone();
        *t = Tr { text, src: "Detect".into(), tgt, req: t.req + 1, edited: Some(Instant::now()), ..Default::default() };
        drop(t);
        self.view = View::Translate;
    }
    /// Traduz 500 ms depois da última mudança (no texto ou nos idiomas), como no Kon atual.
    fn translate_tick(&mut self, ctx: &egui::Context) {
        if self.view != View::Translate { return; }
        let mut t = self.tr.lock().unwrap();
        let Some(ed) = t.edited else { return };
        if t.text.trim().is_empty() { t.req += 1; t.out = None; t.detected = None; t.status = 0; t.edited = None; return; }
        if ed.elapsed() < Duration::from_millis(500) { t.status = 1; ctx.request_repaint_after(Duration::from_millis(520) - ed.elapsed()); return; }
        t.edited = None; t.status = 1; t.req += 1;
        let (id, text, src, tgt) = (t.req, t.text.clone(), t.src.clone(), t.tgt.clone());
        drop(t);
        let local = move |st: &mut Tr| {
            let det = if src == "Detect" { tr::detect(&text).to_string() } else { src.clone() };
            st.detected = Some(det.clone()); st.engine = "local".into();
            match tr::local(&text, &det, &tgt) { Some(o) => { st.out = Some(o); st.status = 2; } None => { st.out = Some("Sem tradução offline para este texto. Adicione uma chave do Gemini ou do Claude em Configurações para traduzir qualquer coisa.".into()); st.status = 3; } }
        };
        match self.once_req(tr::SYSTEM) {
            None => { let mut g = self.tr.lock().unwrap(); local(&mut g); }
            Some(req) => {
                let engine = if req.provider == "claude" { "Claude" } else { "Gemini" };
                let st = self.tr.clone(); let c = ctx.clone();
                let prompt = tr::prompt(&src2(&self.tr), &self.tr.lock().unwrap().tgt.clone(), &self.tr.lock().unwrap().text.clone());
                ai::once(req, prompt, move |r, tokens, _| {
                    ai::add_kon_tokens(tokens);
                    let mut g = st.lock().unwrap();
                    if g.req != id { return; }
                    match r.ok().and_then(|t| tr::parse(&t)) {
                        Some((det, out)) => { g.detected = det; g.out = Some(out); g.engine = engine.into(); g.status = 2; }
                        None => local(&mut g),
                    }
                    drop(g); c.request_repaint();
                });
            }
        }
        fn src2(t: &Arc<Mutex<Tr>>) -> String { t.lock().unwrap().src.clone() }
    }

    fn draw_translate(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool) {
        let p = ui.painter().clone();
        let inner = r.shrink2(vec2(16.0, 0.0));
        let mut t = self.tr.lock().unwrap().clone();
        let hy = r.top() + 22.0;
        self.tofu(ui, pos2(inner.left() + 12.0, hy), 0.82, match t.status { 1 => Mood::Working, 2 => Mood::Done, 3 => Mood::Attention, _ => Mood::Idle }, blink, 1.0);
        p.text(pos2(inner.left() + 32.0, hy), Align2::LEFT_CENTER, "Traduzir", bold(14.0), Color32::WHITE);
        let xr = Rect::from_center_size(pos2(inner.right() - 14.0, hy), vec2(28.0, 28.0));
        let xresp = ui.interact(xr, ui.id().with("trx"), Sense::click());
        if xresp.hovered() { p.circle_filled(xr.center(), 14.0, Color32::from_white_alpha(25)); }
        p.text(xr.center(), Align2::CENTER_CENTER, ic::X, icon_font(15.0), Color32::from_white_alpha(if xresp.hovered() { 255 } else { 128 }));
        let lb = pos2(xr.left() - 20.0, hy);
        p.circle_filled(lb, 18.0, Color32::from_rgba_unmultiplied(59, 130, 246, 50));
        p.circle_filled(lb, 14.0, Color32::from_rgb(59, 130, 246));
        p.text(lb, Align2::CENTER_CENTER, ic::LANGUAGES, icon_font(14.0), Color32::WHITE);
        if xresp.clicked() { self.view = View::Compact; return; }
        // idiomas
        let ly = r.top() + 44.0 + 15.0;
        let cw = (inner.width() - 28.0 - 16.0) / 2.0;
        let srcl = if t.src == "Detect" { match (&t.detected, t.text.trim().is_empty()) { (Some(d), false) => format!("Detectar · {}", tr::pt(d)), _ => "Detectar idioma".into() } } else { tr::pt(&t.src).to_string() };
        let mut changed = false;
        for (k, (label, x)) in [(srcl, inner.left()), (tr::pt(&t.tgt).to_string(), inner.right() - cw)].into_iter().enumerate() {
            let cr = Rect::from_min_size(pos2(x, ly - 14.0), vec2(cw, 28.0));
            p.rect_filled(cr, CornerRadius::same(8), Color32::from_white_alpha(13));
            p.rect_stroke(cr, CornerRadius::same(8), Stroke::new(1.0_f32, Color32::from_white_alpha(15)), egui::StrokeKind::Inside);
            p.text(cr.center(), Align2::CENTER_CENTER, ellipsize_w(ui, &label, 12.0, cw - 50.0), FontId::proportional(12.0), Color32::from_white_alpha(230));
            for (d, ch, cx) in [(-1i32, ic::CHEVRON_LEFT, cr.left() + 14.0), (1, ic::CHEVRON_RIGHT, cr.right() - 14.0)] {
                let br = Rect::from_center_size(pos2(cx, ly), vec2(20.0, 20.0));
                let rsp = ui.interact(br, ui.id().with(("trc", k, d)), Sense::click());
                p.text(br.center(), Align2::CENTER_CENTER, ch, icon_font(14.0), Color32::from_white_alpha(if rsp.hovered() { 255 } else { 102 }));
                if rsp.clicked() {
                    let cycle = |list: &[&str], cur: &str| { let i = list.iter().position(|x| *x == cur).unwrap_or(0) as i32; list[((i + d).rem_euclid(list.len() as i32)) as usize].to_string() };
                    if k == 0 { let mut l = vec!["Detect"]; l.extend(tr::LANGS); t.src = cycle(&l, &t.src); } else { t.tgt = cycle(&tr::TARGETS, &t.tgt); }
                    changed = true;
                }
            }
        }
        let sw = pos2(inner.center().x, ly);
        let swr = Rect::from_center_size(sw, vec2(28.0, 28.0));
        let swresp = ui.interact(swr, ui.id().with("trswap"), Sense::click());
        p.circle_filled(sw, 14.0, Color32::from_white_alpha(if swresp.hovered() { 38 } else { 20 }));
        p.text(sw, Align2::CENTER_CENTER, ic::ARROW_LEFT_RIGHT, icon_font(13.0), Color32::from_white_alpha(204));
        if swresp.clicked() {
            let from = if t.src == "Detect" { t.detected.clone().unwrap_or("English".into()) } else { t.src.clone() };
            if t.status == 2 { if let Some(o) = t.out.clone() { t.text = o; } }
            t.src = t.tgt.clone();
            t.tgt = if tr::TARGETS.contains(&from.as_str()) { from } else { "English".into() };
            changed = true;
        }
        // caixas
        let by = ly + 14.0 + 10.0;
        let bh = r.bottom() - 34.0 - by;
        let bw = (inner.width() - 10.0) / 2.0;
        let a = Rect::from_min_size(pos2(inner.left(), by), vec2(bw, bh));
        let b = Rect::from_min_size(pos2(a.right() + 10.0, by), vec2(bw, bh));
        for x in [a, b] { p.rect_filled(x, CornerRadius::same(12), Color32::from_white_alpha(10)); p.rect_stroke(x, CornerRadius::same(12), Stroke::new(1.0_f32, Color32::from_white_alpha(20)), egui::StrokeKind::Inside); }
        let before = t.text.clone();
        let te = egui::TextEdit::multiline(&mut t.text).hint_text("Digite ou cole um texto").frame(false).font(FontId::proportional(13.0)).text_color(Color32::WHITE).char_limit(800).desired_width(bw - 24.0).desired_rows(4);
        let resp = ui.put(a.shrink(12.0).with_max_y(a.bottom() - 18.0), te);
        if !resp.has_focus() && ui.memory(|m| m.focused().is_none()) { resp.request_focus(); }
        // Enter traduz já; Shift+Enter quebra a linha
        let enter = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
        if enter && t.text.ends_with('\n') && !before.ends_with('\n') { t.text.pop(); }
        if t.text != before { changed = true; }
        p.text(pos2(a.right() - 10.0, a.bottom() - 10.0), Align2::RIGHT_CENTER, format!("{}/800", t.text.chars().count()), FontId::monospace(10.0), Color32::from_white_alpha(90));
        if changed || enter { t.edited = Some(if enter { Instant::now() - Duration::from_millis(600) } else { Instant::now() }); t.status = if t.text.trim().is_empty() { 0 } else { 1 }; }
        let (status, out, engine) = (t.status, t.out.clone(), t.engine.clone());
        { let mut g = self.tr.lock().unwrap(); g.text = t.text.clone(); g.src = t.src.clone(); g.tgt = t.tgt.clone(); if changed || enter { g.edited = t.edited; g.status = t.status; } }
        let ob = b.shrink(12.0);
        match status {
            1 => { let dots = ".".repeat(1 + ((ui.input(|i| i.time) * 3.0) as usize % 3)); p.text(ob.left_top(), Align2::LEFT_TOP, format!("Traduzindo{dots}"), FontId::proportional(13.0), Color32::from_white_alpha(115)); ui.ctx().request_repaint_after(Duration::from_millis(150)); }
            2 | 3 => {
                let col = if status == 3 { Color32::from_rgba_unmultiplied(253, 230, 138, 230) } else { Color32::WHITE };
                let mut job = egui::text::LayoutJob::single_section(out.clone().unwrap_or_default(), egui::TextFormat { font_id: FontId::proportional(13.0), color: col, line_height: Some(19.0), ..Default::default() });
                job.wrap.max_width = ob.width();
                let g = ui.fonts_mut(|f| f.layout_job(job));
                p.with_clip_rect(b.shrink(4.0)).galley(ob.left_top(), g, col);
                if status == 2 {
                    let copied = self.copied_at.map_or(false, |c| c.elapsed() < Duration::from_millis(1500));
                    let cr = Rect::from_min_size(pos2(b.right() - 76.0, b.bottom() - 30.0), vec2(68.0, 24.0));
                    let rsp = ui.interact(cr, ui.id().with("trcopy"), Sense::click());
                    p.rect_filled(cr, CornerRadius::same(6), if rsp.hovered() { Color32::from_white_alpha(25) } else { Color32::from_black_alpha(102) });
                    p.text(pos2(cr.left() + 12.0, cr.center().y), Align2::LEFT_CENTER, if copied { ic::CHECK } else { ic::COPY }, icon_font(11.0), if copied { Color32::from_rgb(52, 211, 153) } else { Color32::from_white_alpha(153) });
                    p.text(pos2(cr.left() + 27.0, cr.center().y), Align2::LEFT_CENTER, "Copiar", FontId::proportional(11.0), Color32::from_white_alpha(if rsp.hovered() { 255 } else { 153 }));
                    if rsp.clicked() { ui.ctx().copy_text(out.unwrap_or_default()); self.copied_at = Some(Instant::now()); }
                    if copied { ui.ctx().request_repaint_after(Duration::from_millis(300)); }
                }
            }
            _ => { p.text(ob.left_top(), Align2::LEFT_TOP, "Tradução", FontId::proportional(13.0), Color32::from_white_alpha(64)); }
        }
        let foot = if status == 2 { if engine == "local" { "Tradução local (offline)".to_string() } else { format!("Traduzido por {engine}") } } else { "Enter traduz · Shift+Enter nova linha · Esc fecha".into() };
        p.text(pos2(r.center().x, r.bottom() - 17.0), Align2::CENTER_CENTER, foot, FontId::proportional(11.0), Color32::from_white_alpha(115));
    }

    /// Ctrl+Alt+R: copia o texto selecionado no programa da frente (sem tirar o foco dele), devolve a sua
    /// área de transferência como estava e abre as ações. Nada vai para o histórico.
    fn grab_selection(&mut self, ctx: &egui::Context) {
        clipboard::suspend(&self.clips, 2500);
        let before = clipboard::read_text();
        clipboard::write_text("");
        crate::platform::send_ctrl(b'C');
        let mut text = String::new();
        for _ in 0..20 { std::thread::sleep(Duration::from_millis(40)); text = clipboard::read_text(); if !text.is_empty() { break; } }
        clipboard::write_text(&before);
        let mut t = self.ta.lock().unwrap();
        let id = t.req + 1;
        *t = if text.trim().is_empty() { TextAct { error: "Selecione um texto em qualquer programa e aperte Ctrl+Alt+R".into(), req: id, ..Default::default() } }
            else { TextAct { text: text.chars().take(12000).collect(), req: id, ..Default::default() } };
        drop(t);
        self.view = View::TextAct;
        ctx.request_repaint();
    }

    fn draw_textact(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool) {
        const ACTIONS: [(&str, &str, &str); 6] = [
            ("resumir", "Resumir", "Resuma o texto abaixo em português, em até 3 frases curtas."),
            ("traduzir", "Traduzir", "Se o texto abaixo estiver em português, traduza para o inglês; se estiver em outro idioma, traduza para o português do Brasil. Mantenha o tom e a formatação."),
            ("corrigir", "Corrigir", "Corrija ortografia, gramática e pontuação do texto abaixo, mantendo o idioma, o tom e a formatação."),
            ("formal", "Mais formal", "Reescreva o texto abaixo em tom mais formal e profissional, no mesmo idioma."),
            ("curto", "Mais curto", "Reescreva o texto abaixo mais curto e direto, no mesmo idioma, sem perder o sentido."),
            ("explicar", "Explicar", "Explique em português, de forma simples, o que o texto abaixo quer dizer."),
        ];
        let p = ui.painter().clone();
        let t = self.ta.lock().unwrap().clone();
        let inner = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 10.0), pos2(r.right() - 16.0, r.bottom() - 10.0));
        let hy = inner.top() + 15.0;
        self.tofu(ui, pos2(inner.left() + 11.0, hy), 0.8, if !t.error.is_empty() || t.status == 3 { Mood::Attention } else { match t.status { 1 => Mood::Working, 2 => Mood::Done, _ => Mood::Idle } }, blink, 1.0);
        let tx = inner.left() + 32.0;
        let xr = Rect::from_center_size(pos2(inner.right() - 12.0, hy), vec2(24.0, 24.0));
        let xresp = ui.interact(xr, ui.id().with("tax"), Sense::click());
        if xresp.hovered() { p.circle_filled(xr.center(), 12.0, Color32::from_white_alpha(25)); }
        p.text(xr.center(), Align2::CENTER_CENTER, ic::X, icon_font(13.0), Color32::from_white_alpha(if xresp.hovered() { 255 } else { 128 }));
        if xresp.clicked() { self.view = View::Compact; return; }
        if !t.error.is_empty() { p.text(pos2(tx, hy), Align2::LEFT_CENTER, &t.error, bold(12.5), Color32::WHITE); return; }
        p.text(pos2(tx, hy - 7.0), Align2::LEFT_CENTER, "Texto selecionado", bold(12.5), Color32::WHITE);
        let one = t.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let one = if self.settings.privacy { mask(&one) } else { one };
        p.text(pos2(tx, hy + 8.0), Align2::LEFT_CENTER, ellipsize_w(ui, &one, 11.0, xr.left() - tx - 8.0), FontId::proportional(11.0), Color32::from_white_alpha(115));
        // ações
        let mut x = inner.left(); let ay = inner.top() + 30.0 + 8.0 + 13.0;
        let mut run: Option<usize> = None;
        for (i, (id, label, _)) in ACTIONS.iter().enumerate() {
            let w = text_w(ui, label, 11.5) + 20.0;
            let br = Rect::from_min_size(pos2(x, ay - 13.0), vec2(w, 26.0));
            let rsp = ui.interact(br, ui.id().with(("ta", i)), Sense::click());
            let on = t.action == *id;
            p.rect_filled(br, CornerRadius::same(13), if on { Color32::WHITE } else { Color32::from_white_alpha(if rsp.hovered() { 36 } else { 20 }) });
            p.text(br.center(), Align2::CENTER_CENTER, *label, FontId::proportional(11.5), if on { Color32::BLACK } else { Color32::from_white_alpha(204) });
            if rsp.clicked() && t.status != 1 { run = Some(i); }
            x += w + 6.0;
        }
        if let Some(i) = run {
            let (id, _, instr) = ACTIONS[i];
            let mut g = self.ta.lock().unwrap();
            g.action = id.into(); g.req += 1; let req_id = g.req;
            match self.once_req("Você edita textos para o usuário. Responda somente com o texto final, sem aspas, sem comentários e sem explicar o que fez (exceto quando pedirem uma explicação).") {
                None => { g.status = 3; g.result = "Adicione uma chave do Gemini ou do Claude em Ajustes.".into(); }
                Some(req) => {
                    g.status = 1; g.result.clear();
                    let text = format!("{instr}\n\n\"\"\"\n{}\n\"\"\"", g.text);
                    let st = self.ta.clone(); let c = ui.ctx().clone();
                    ai::once(req, text, move |res, tokens, _| {
                        ai::add_kon_tokens(tokens);
                        let mut g = st.lock().unwrap();
                        if g.req != req_id { return; }
                        match res { Ok(s) => { g.status = 2; g.result = s.trim().trim_start_matches("\"\"\"").trim_end_matches("\"\"\"").trim().to_string(); }
                            Err((code, msg)) => { g.status = 3; g.result = if code == "NO_KEY" { "Adicione sua chave em Ajustes.".into() } else if msg.is_empty() { "Não deu certo.".into() } else { msg }; } }
                        drop(g); c.request_repaint();
                    });
                }
            }
        }
        let t = self.ta.lock().unwrap().clone();
        let ry = ay + 13.0 + 10.0;
        if t.status == 1 {
            let dots = ".".repeat(1 + ((ui.input(|i| i.time) * 3.0) as usize % 3));
            p.text(pos2(inner.left(), ry + 8.0), Align2::LEFT_CENTER, format!("Escrevendo{dots}"), FontId::proportional(12.0), Color32::from_white_alpha(115));
            ui.ctx().request_repaint_after(Duration::from_millis(150));
        }
        if t.status >= 2 {
            let col = if t.status == 3 { Color32::from_rgba_unmultiplied(253, 230, 138, 230) } else { Color32::WHITE };
            let mut job = egui::text::LayoutJob::single_section(t.result.clone(), egui::TextFormat { font_id: FontId::proportional(12.5), color: col, line_height: Some(18.0), ..Default::default() });
            job.wrap.max_width = inner.width();
            let g = ui.fonts_mut(|f| f.layout_job(job));
            let area = Rect::from_min_max(pos2(inner.left(), ry), pos2(inner.right(), inner.bottom() - if t.status == 2 { 32.0 } else { 0.0 }));
            p.with_clip_rect(area).galley(area.left_top(), g, col);
            if t.status == 2 {
                let by = inner.bottom() - 12.0;
                let mut rx = inner.right();
                if t.action != "explicar" && small_pill(ui, &p, &mut rx, by, Some(ic::CHECK), "Colar no lugar", true, 1.0) {
                    // escreve o resultado e manda Ctrl+V para o mesmo programa (o Kon não tem o foco)
                    clipboard::suspend(&self.clips, 1500);
                    clipboard::write_text(&t.result);
                    self.view = View::Compact;
                    std::thread::spawn(|| { std::thread::sleep(Duration::from_millis(120)); crate::platform::send_ctrl(b'V'); });
                }
                if small_pill(ui, &p, &mut rx, by, Some(ic::COPY), "Copiar", false, 1.0) { ui.ctx().copy_text(t.result.clone()); self.toast("Copiado"); self.view = View::Compact; }
            }
        }
    }

    fn open_ask(&mut self) {
        ai::cancel(&self.ask);
        self.ask_q.clear(); self.ask_rows = 0.0; self.ask_file = None;
        self.view = View::Ask;
    }
    /// "Perguntar sobre a tela": foto do monitor (sem o Kon) anexada à próxima pergunta.
    fn ask_screen(&mut self) {
        crate::platform::exclude_from_capture(true);
        let shot = crate::platform::capture_screen();
        crate::platform::exclude_from_capture(self.settings.hide_in_capture);
        self.capture_applied = Some(self.settings.hide_in_capture);
        let Some((w, h, rgba, _, _)) = shot else { self.toast_err("Não consegui capturar a tela"); return };
        let dir = crate::platform::data_dir().join("telas");
        let _ = std::fs::create_dir_all(&dir);
        // guarda só as últimas capturas
        if let Ok(rd) = std::fs::read_dir(&dir) { let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect(); v.sort(); while v.len() > 4 { let _ = std::fs::remove_file(v.remove(0)); } }
        let path = dir.join(format!("tela-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        // a IA não precisa da resolução inteira: reduz para no máximo 1600 px de largura
        let saved = image::RgbaImage::from_raw(w, h, rgba).map(|im| if w > 1600 { image::imageops::resize(&im, 1600, (h as f32 * 1600.0 / w as f32) as u32, image::imageops::FilterType::Triangle) } else { im })
            .map(|im| image::DynamicImage::ImageRgba8(im).to_rgb8().save(&path).is_ok()).unwrap_or(false);
        if !saved { self.toast_err("Não consegui capturar a tela"); return; }
        self.ask_file = Some(AskFile { path, name: "Tela".into(), screen: true, sent: false });
    }
    fn toggle_dictation(&mut self, ctx: &egui::Context) {
        let mut g = self.dict.lock().unwrap();
        if g.0 { g.0 = false; g.2 += 1; return; } // para: ignora o que vier
        g.0 = true; g.2 += 1; let id = g.2;
        drop(g);
        let st = self.dict.clone(); let c = ctx.clone();
        std::thread::spawn(move || {
            let r = crate::platform::dictate();
            let mut g = st.lock().unwrap();
            if g.2 == id { g.0 = false; g.1 = Some(r); }
            drop(g); c.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    fn key_for(&self, provider: &str) -> String {
        secrets::unprotect(if provider == "claude" { &self.settings.anthropic_key_enc } else { &self.settings.gemini_key_enc })
    }
    fn submit_ask(&mut self, ctx: &egui::Context) {
        let q = self.ask_q.trim().to_string();
        if q.is_empty() { return; }
        self.ask_q.clear();
        let file = self.ask_file.as_mut().filter(|f| !f.sent).map(|f| { f.sent = true; f.path.clone() });
        if file.is_some() {
            let req = self.ask_req(); let c = ctx.clone();
            ai::ask_with(&self.ask, q, None, file, req, move || c.request_repaint());
            return;
        }
        if let Some((text, at)) = reminders::parse(&q, chrono::Local::now()) {
            let id = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
            self.rems.push(Rem { id, text: text.chars().take(200).collect(), at, fired: false });
            reminders::save(&self.rems);
            let mut g = self.ask.lock().unwrap();
            let n = g.id + 1;
            *g = ai::Ask { id: n, phase: ai::Phase::Done, question: q, answer: format!("Combinado: {} eu te aviso no notch.\n{text}", reminders::describe(at, chrono::Local::now())), ..Default::default() };
            return;
        }
        let req = self.ask_req();
        let c = ctx.clone();
        ai::ask(&self.ask, q, req, move || c.request_repaint());
    }
    fn ask_req(&self) -> ai::Req {
        let pv = self.settings.provider.clone();
        ai::Req { key: self.key_for(&pv), model: if pv == "claude" { self.settings.claude_model.clone() } else { self.settings.gemini_model.clone() }, web: pv == "gemini" && self.settings.web_search, provider: pv, system: ai::SYSTEM.into() }
    }

    /* ───────── Smart OCR (Ctrl+Alt+T) ───────── */
    fn start_ocr(&mut self, ctx: &egui::Context) {
        if self.ocr_shot.is_some() { return; }
        // a foto sai sem o Kon (fora da captura só durante a foto) e do monitor onde está o mouse
        crate::platform::exclude_from_capture(true);
        let shot = crate::platform::capture_screen();
        crate::platform::exclude_from_capture(self.settings.hide_in_capture);
        self.capture_applied = Some(self.settings.hide_in_capture);
        match shot {
            None => self.toast_err("Não consegui capturar a tela"),
            Some((w, h, rgba, mx, my)) => {
                if matches!(self.view, View::Ask | View::Translate | View::TextAct | View::Ocr | View::Sheet) { self.view = View::Compact; }
                self.ocr_shot = Some(OcrShot { w, h, rgba, mx, my, tex: None, start: None, sel: None, frames: 0 });
                // a foto já está sem o Kon; enquanto recorta, a ilha some para não ficar por cima do recorte
                crate::platform::set_visible(false);
                ctx.request_repaint();
            }
        }
    }

    /// A janela de recorte: cobre o monitor inteiro com a foto da tela e deixa arrastar um retângulo.
    fn ocr_overlay(&mut self, ctx: &egui::Context) {
        let Some(shot) = self.ocr_shot.as_mut() else { return };
        let scale = ctx.pixels_per_point();
        let vb = egui::ViewportBuilder::default().with_title("Kon OCR").with_decorations(false).with_always_on_top().with_taskbar(false).with_resizable(false)
            .with_position([shot.mx as f32 / scale, shot.my as f32 / scale]).with_inner_size([shot.w as f32 / scale, shot.h as f32 / scale]);
        // 0 = continua, 1 = cancelou, 2 = recortou
        let out = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("kon-ocr"), vb, |c, _| -> (u8, Option<(u32, u32, Vec<u8>)>) {
            // o monitor pode ter outra escala: a posição exata em pixels físicos vem do Windows
            if shot.frames < 8 { crate::platform::cover_rect("Kon OCR", shot.mx, shot.my, shot.w as i32, shot.h as i32); shot.frames += 1; c.request_repaint(); }
            if shot.tex.is_none() {
                let img = egui::ColorImage::from_rgba_unmultiplied([shot.w as usize, shot.h as usize], &shot.rgba);
                shot.tex = Some(c.load_texture("kon-ocr-shot", img, egui::TextureOptions::LINEAR));
            }
            if c.input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().close_requested()) { return (1, None); }
            let mut res = (0u8, None);
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(c, |ui| {
                let full = c.content_rect();
                let p = ui.painter();
                if let Some(t) = &shot.tex { p.image(t.id(), full, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE); }
                let resp = ui.interact(full, ui.id().with("ocr-drag"), Sense::drag());
                c.set_cursor_icon(egui::CursorIcon::Crosshair);
                if resp.drag_started() { shot.start = resp.interact_pointer_pos(); shot.sel = shot.start.map(|s| Rect::from_min_size(s, Vec2::ZERO)); }
                if resp.dragged() { if let (Some(s), Some(q)) = (shot.start, resp.interact_pointer_pos()) { shot.sel = Some(Rect::from_two_pos(s, q)); } }
                // escurece só fora do recorte
                let dim = Color32::from_black_alpha(77);
                match shot.sel {
                    Some(r) if r.width() > 0.0 || r.height() > 0.0 => {
                        p.rect_filled(Rect::from_min_max(full.min, pos2(full.right(), r.top())), 0.0, dim);
                        p.rect_filled(Rect::from_min_max(pos2(full.left(), r.bottom()), full.max), 0.0, dim);
                        p.rect_filled(Rect::from_min_max(pos2(full.left(), r.top()), pos2(r.left(), r.bottom())), 0.0, dim);
                        p.rect_filled(Rect::from_min_max(pos2(r.right(), r.top()), pos2(full.right(), r.bottom())), 0.0, dim);
                        for (i, a) in [(10.0f32, 18u8), (6.0, 30), (3.0, 45)] { p.rect_stroke(r.expand(i), CornerRadius::same((12.0 + i) as u8), Stroke::new(3.0_f32, Color32::from_rgba_unmultiplied(99, 102, 241, a)), egui::StrokeKind::Middle); }
                        p.rect_stroke(r, CornerRadius::same(12), Stroke::new(2.0_f32, Color32::from_rgb(129, 140, 248)), egui::StrokeKind::Inside);
                    }
                    _ => { p.rect_filled(full, 0.0, dim); }
                }
                // aviso no topo
                let msg = "Arraste sobre o texto para capturar · Esc cancela";
                let tw = p.layout_no_wrap(msg.into(), FontId::proportional(12.5), Color32::WHITE).size().x;
                let pr = Rect::from_center_size(pos2(full.center().x, full.top() + 56.0 + 18.0), vec2(tw + 32.0 + 22.0, 36.0));
                p.rect_filled(pr.translate(vec2(0.0, 6.0)).expand(6.0), CornerRadius::same(24), Color32::from_black_alpha(40));
                p.rect_filled(pr, CornerRadius::same(18), Color32::from_black_alpha(217));
                p.rect_stroke(pr, CornerRadius::same(18), Stroke::new(1.0_f32, Color32::from_white_alpha(25)), egui::StrokeKind::Inside);
                p.text(pos2(pr.left() + 16.0, pr.center().y), Align2::LEFT_CENTER, ic::SPARKLES, icon_font(14.0), Color32::from_rgb(165, 180, 252));
                p.text(pos2(pr.left() + 16.0 + 22.0, pr.center().y), Align2::LEFT_CENTER, msg, FontId::proportional(12.5), Color32::from_white_alpha(230));
                if resp.drag_stopped() {
                    let sel = shot.sel.take(); shot.start = None;
                    if let Some(r) = sel.filter(|r| r.width() >= 6.0 && r.height() >= 6.0) {
                        let kx = shot.w as f32 / full.width(); let ky = shot.h as f32 / full.height();
                        let x0 = (((r.left() - full.left()) * kx).round().max(0.0) as u32).min(shot.w - 1);
                        let y0 = (((r.top() - full.top()) * ky).round().max(0.0) as u32).min(shot.h - 1);
                        let cw = ((r.width() * kx).round() as u32).clamp(1, shot.w - x0);
                        let ch = ((r.height() * ky).round() as u32).clamp(1, shot.h - y0);
                        let mut buf = Vec::with_capacity((cw * ch * 4) as usize);
                        for y in y0..y0 + ch { let i = ((y * shot.w + x0) * 4) as usize; buf.extend_from_slice(&shot.rgba[i..i + (cw * 4) as usize]); }
                        res = (2, Some((cw, ch, buf)));
                    }
                }
            });
            res
        });
        if out.0 != 0 { crate::platform::set_visible(!self.hidden); }
        match out {
            (0, _) => {}
            (1, _) => { self.ocr_shot = None; }
            (_, crop) => {
                self.ocr_shot = None;
                if let Some((w, h, buf)) = crop { self.run_ocr(ctx, w, h, buf); }
            }
        }
    }

    fn run_ocr(&mut self, ctx: &egui::Context, w: u32, h: u32, buf: Vec<u8>) {
        *self.ocr.lock().unwrap() = OcrRes { reading: true, ..Default::default() };
        self.ocr_q.clear();
        self.view = View::Ocr;
        let st = self.ocr.clone(); let c = ctx.clone();
        std::thread::spawn(move || {
            // ampliar 2x ajuda o OCR; o Windows aceita até ~2600 px no maior lado
            let k = (2.0f32).min(2600.0 / w.max(h) as f32);
            let (w2, h2, img) = match image::RgbaImage::from_raw(w, h, buf) {
                Some(im) if (k - 1.0).abs() > 0.01 => { let (nw, nh) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1)); (nw, nh, image::imageops::resize(&im, nw, nh, image::imageops::FilterType::CatmullRom).into_raw()) }
                Some(im) => (w, h, im.into_raw()),
                None => (w, h, vec![]),
            };
            let r = if img.is_empty() { Err("imagem inválida".to_string()) } else { crate::platform::ocr(w2, h2, &img) };
            let mut g = st.lock().unwrap();
            g.reading = false;
            match r {
                Ok(t) => { let t = t.trim().to_string(); if !t.is_empty() { clipboard::write_text(&t); g.copied = true; } g.text = t; }
                Err(e) => { g.error = e; }
            }
            drop(g); c.request_repaint();
        });
    }

    fn draw_drop(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool) {
        let p = ui.painter().clone();
        let a = ((self.h.v - 40.0) / 60.0).clamp(0.0, 1.0);
        let inner = Rect::from_min_max(pos2(r.left() + 10.0, r.top() + 8.0), pos2(r.right() - 10.0, r.bottom() - 10.0));
        let tw = 5.2 * 1.35 * 5.0 + 8.0;
        self.tofu(ui, pos2(inner.left() + tw / 2.0, inner.center().y), 1.35, Mood::Eating, blink, a);
        let zx = inner.left() + tw + 8.0;
        let zw = (inner.right() - zx - 16.0) / 3.0;
        let ai_name = if self.settings.provider == "claude" { "Claude" } else { "Gemini" };
        let zones: [(char, &str, String, Color32, Color32, Color32); 3] = [
            (ic::DOWNLOAD, "Guardar", "só a referência · nada é lido".into(), Color32::from_rgba_unmultiplied(110, 231, 183, 179), Color32::from_rgba_unmultiplied(52, 211, 153, 26), Color32::from_rgb(110, 231, 183)),
            (ic::SPARKLES, "Perguntar à IA", format!("envia o arquivo para o {ai_name}"), Color32::from_rgba_unmultiplied(196, 181, 253, 179), Color32::from_rgba_unmultiplied(167, 139, 250, 26), Color32::from_rgb(196, 181, 253)),
            (ic::ARCHIVE, "Compactar", "cria um .zip ao lado".into(), Color32::from_rgba_unmultiplied(125, 211, 252, 179), Color32::from_rgba_unmultiplied(56, 189, 248, 26), Color32::from_rgb(125, 211, 252)),
        ];
        for (i, (icon, label, sub, border, bg, fg)) in zones.iter().enumerate() {
            let zr = Rect::from_min_size(pos2(zx + i as f32 * (zw + 8.0), inner.top()), vec2(zw, inner.height()));
            self.drop_rects[i] = zr;
            let on = self.drop_zone as usize == i;
            if on { p.rect_filled(zr, CornerRadius::same(16), fade(*bg, a)); }
            dashed_rr(&p, zr.shrink(1.0), 15.0, fade(if on { *border } else { Color32::from_white_alpha(38) }, a));
            dashed_rr(&p, zr.shrink(1.6), 14.4, fade(if on { *border } else { Color32::from_white_alpha(38) }, a));
            let c = zr.center();
            p.text(pos2(c.x, c.y - 17.0), Align2::CENTER_CENTER, *icon, icon_font(17.0), fade(if on { *fg } else { Color32::from_white_alpha(128) }, a));
            p.text(pos2(c.x, c.y + 3.0), Align2::CENTER_CENTER, *label, bold(12.0), fade(Color32::WHITE, a));
            let mut job = egui::text::LayoutJob::single_section(sub.clone(), egui::TextFormat { font_id: FontId::proportional(9.5), color: fade(Color32::from_white_alpha(115), a), line_height: Some(11.0), ..Default::default() });
            job.wrap.max_width = zw - 12.0; job.halign = Align::Center;
            let g = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(pos2(c.x, c.y + 11.0), g, Color32::WHITE);
        }
    }

    fn draw_ocr(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool) {
        let p = ui.painter().clone();
        let o = self.ocr.lock().unwrap().clone();
        if o.reading {
            let label = "Lendo texto";
            let tw = ui.painter().layout_no_wrap(label.to_string(), bold(13.5), Color32::WHITE).size().x;
            let total = 22.0 + 10.0 + tw + 14.0;
            let x0 = r.center().x - total / 2.0; let cy = r.top() + 22.0;
            self.tofu(ui, pos2(x0 + 11.0, cy), 0.82, Mood::Working, blink, 1.0);
            p.text(pos2(x0 + 32.0, cy), Align2::LEFT_CENTER, label, bold(13.5), Color32::WHITE);
            let t = ui.input(|i| i.time) as f32;
            for i in 0..3 {
                let ph = ((t / 0.9 - i as f32 * 0.15 / 0.9).rem_euclid(1.0) * std::f32::consts::TAU).cos();
                let a = 0.6 - 0.4 * ph;
                p.text(pos2(x0 + 32.0 + tw + 1.0 + i as f32 * 4.5, cy), Align2::LEFT_CENTER, ".", FontId::proportional(13.5), fade(Color32::WHITE, a));
            }
            ui.ctx().request_repaint_after(Duration::from_millis(33));
            return;
        }
        let hy = r.top() + 23.0;
        let err = !o.error.is_empty();
        self.tofu(ui, pos2(r.left() + 16.0 + 14.0, hy), 0.82, if err { Mood::Error } else { Mood::Done }, blink, 1.0);
        p.hline(r.left()..=r.right(), r.top() + 46.0, Stroke::new(1.0_f32, Color32::from_white_alpha(15)));
        let sr = Rect::from_center_size(pos2(r.right() - 10.0 - 14.0, hy), vec2(28.0, 28.0));
        let srsp = ui.interact(sr, ui.id().with("ocrs"), Sense::click());
        if srsp.hovered() { p.circle_filled(sr.center(), 14.0, Color32::from_white_alpha(25)); }
        p.text(sr.center(), Align2::CENTER_CENTER, ic::SEARCH, icon_font(15.0), Color32::from_white_alpha(if srsp.hovered() { 255 } else { 179 }));
        let fr = Rect::from_min_max(pos2(r.left() + 16.0 + 28.0 + 12.0, hy - 11.0), pos2(sr.left() - 10.0, hy + 11.0));
        let te = egui::TextEdit::singleline(&mut self.ocr_q).hint_text(egui::RichText::new("Pergunte sobre este texto").color(Color32::from_white_alpha(90))).frame(false).font(FontId::proportional(14.5)).text_color(Color32::WHITE).desired_width(fr.width());
        let resp = ui.put(fr, te);
        if !resp.has_focus() && ui.memory(|m| m.focused().is_none()) { resp.request_focus(); }
        let enter = (resp.lost_focus() || resp.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (enter || srsp.clicked()) && !self.ocr_q.trim().is_empty() && !o.text.is_empty() {
            let q = std::mem::take(&mut self.ocr_q).trim().to_string();
            ai::cancel(&self.ask);
            self.ask_q.clear(); self.ask_rows = 0.0;
            let req = self.ask_req(); let c = ui.ctx().clone();
            ai::ask_with(&self.ask, q, Some(o.text.clone()), None, req, move || c.request_repaint());
            self.view = View::Ask;
            return;
        }
        // texto lido
        let body = Rect::from_min_max(pos2(r.left() + 20.0, r.top() + 46.0 + 10.0), pos2(r.right() - 20.0, r.bottom() - 42.0 - 10.0));
        let (txt, col) = if err { (format!("Não consegui ler o texto: {}", o.error), Color32::from_rgba_unmultiplied(253, 230, 138, 230)) } else if o.text.is_empty() { ("(nenhum texto encontrado na seleção)".to_string(), Color32::WHITE) } else { (o.text.clone(), Color32::WHITE) };
        let mut job = egui::text::LayoutJob::single_section(txt, egui::TextFormat { font_id: FontId::monospace(13.0), color: col, line_height: Some(21.0), ..Default::default() });
        job.wrap.max_width = body.width();
        let g = ui.fonts_mut(|f| f.layout_job(job));
        let over = (g.size().y - body.height()).max(0.0);
        let brsp = ui.interact(body, ui.id().with("ocrb"), Sense::hover());
        if brsp.hovered() { let d = ui.input(|i| i.smooth_scroll_delta.y); self.clip_scroll = (self.clip_scroll - d).clamp(0.0, over); } else if over == 0.0 { self.clip_scroll = 0.0; }
        let sc = self.clip_scroll.min(over);
        p.with_clip_rect(body).galley(pos2(body.left(), body.top() - sc), g, col);
        // rodapé
        let fy = r.bottom() - 21.0;
        p.text(pos2(r.left() + 16.0, fy), Align2::LEFT_CENTER, format!("Smart OCR · Windows · {}", if o.copied { "Copiado para a área de transferência" } else { "Pronto para copiar" }), FontId::proportional(11.0), Color32::from_white_alpha(102));
        let mut rx = r.right() - 16.0;
        if small_pill(ui, &p, &mut rx, fy, Some(ic::X), "Fechar", false, 1.0) { self.view = View::Compact; }
        let copied = self.copied_at.map_or(false, |t| t.elapsed() < Duration::from_millis(1500));
        if small_pill(ui, &p, &mut rx, fy, Some(if copied { ic::CHECK } else { ic::COPY }), "Copiar", false, 1.0) && !o.text.is_empty() {
            ui.ctx().copy_text(o.text.clone()); self.copied_at = Some(Instant::now()); self.ocr.lock().unwrap().copied = true;
        }
    }

    fn draw_ask(&mut self, ui: &mut egui::Ui, r: Rect, blink: bool) {
        let p = ui.painter().clone();
        let st = self.ask.lock().unwrap().clone();
        let pv = self.settings.provider.clone();
        let pname = if pv == "claude" { "Claude" } else { "Gemini" };
        let hy = r.top() + 23.0;
        let am = match st.phase { ai::Phase::Thinking | ai::Phase::Streaming => Mood::Working, ai::Phase::Done => match &st.error { Some(e) if e.0 == "NO_KEY" => Mood::Attention, Some(_) => Mood::Error, None => Mood::Done }, _ => Mood::Idle };
        self.tofu(ui, pos2(r.left() + 16.0 + 14.0, hy), 0.82, am, blink, 1.0);
        // o que aparece ao lado do campo enquanto você digita (conta, unidade, câmbio)
        let c2 = ui.ctx().clone();
        let inl = calc::inline(&self.ask_q, &self.rates, move || c2.request_repaint());
        let listening = self.dict.lock().unwrap().0;
        let dres = self.dict.lock().unwrap().1.take();
        if let Some(res) = dres {
            match res { Ok(t) if !t.trim().is_empty() => { self.ask_q = t.trim().to_string(); } Ok(_) => {} Err(e) => self.toast_err(&e) }
        }
        let file = self.ask_file.clone();
        let can_ask = (!self.ask_q.trim().is_empty() || file.as_ref().map_or(false, |f| !f.sent)) && inl.is_none() && !self.ask_q.trim_end().ends_with('=');
        let default_q = if file.as_ref().map_or(false, |f| f.screen) { "O que está na minha tela? Resuma o que importa." } else { "Resuma este arquivo." };
        // botões à direita: [IA] [ditar] [tela] [traduzir] [enviar]
        let btn = |ui: &mut egui::Ui, p: &egui::Painter, x: f32, id: &str, icon: char, tip: &str, red: bool| -> egui::Response {
            let br = Rect::from_min_size(pos2(x - 28.0, hy - 14.0), vec2(28.0, 28.0));
            let rsp = ui.interact(br, ui.id().with(("askb", id)), Sense::click()).on_hover_text(tip);
            if red { p.circle_filled(br.center(), 14.0, Color32::from_rgba_unmultiplied(239, 68, 68, 230)); } else if rsp.hovered() { p.circle_filled(br.center(), 14.0, Color32::from_white_alpha(25)); }
            p.text(br.center(), Align2::CENTER_CENTER, icon, icon_font(if icon == ic::SQUARE { 11.0 } else { 15.0 }), if red { Color32::WHITE } else { Color32::from_white_alpha(if rsp.hovered() { 255 } else if id == "send" { 178 } else { 115 }) });
            rsp
        };
        let mut bx = r.right() - 10.0;
        let srsp = btn(ui, &p, bx, "send", ic::SEARCH, "", false);
        bx -= 28.0 + 8.0;
        let mut left_edge = bx;
        if inl.is_none() {
            let trsp = btn(ui, &p, bx, "tr", ic::LANGUAGES, "Traduzir (Tab)", false); bx -= 36.0;
            let mrsp = btn(ui, &p, bx, "scr", ic::MONITOR, "Perguntar sobre a tela", false); bx -= 36.0;
            let vrsp = btn(ui, &p, bx, "mic", if listening { ic::SQUARE } else { ic::MIC }, "Ditar (pt-BR)", listening); bx -= 36.0;
            let pw = text_w(ui, pname, 10.5) + 16.0;
            let pr = Rect::from_min_size(pos2(bx - pw, hy - 12.0), vec2(pw, 24.0));
            let prsp = ui.interact(pr, ui.id().with("askprov"), Sense::click()).on_hover_text("Alternar IA");
            p.rect_filled(pr, CornerRadius::same(12), Color32::from_white_alpha(if prsp.hovered() { 38 } else { 20 }));
            p.text(pr.center(), Align2::CENTER_CENTER, pname, bold(10.5), Color32::from_white_alpha(178));
            if prsp.clicked() { self.settings.provider = if pv == "claude" { "gemini".into() } else { "claude".into() }; self.settings.save(); }
            left_edge = pr.left() - 8.0;
            if trsp.clicked() { let q = std::mem::take(&mut self.ask_q); self.open_translate(q); return; }
            if mrsp.clicked() { self.ask_screen(); }
            if vrsp.clicked() { self.toggle_dictation(ui.ctx()); }
        } else if let Some(calc::Inline::Currency(_)) = &inl {
            let g = self.rates.lock().unwrap().clone();
            if g.at > 1 {
                let stale = calc::rates_stale(&g);
                let lbl = format!("ExchangeRate-API · {}{}", chrono::Local.timestamp_millis_opt(g.at).single().map_or(String::new(), |d| d.format("%H:%M").to_string()), if stale { " · offline" } else { "" });
                let lw = ui.painter().layout_no_wrap(lbl.clone(), FontId::proportional(10.5), Color32::WHITE).size().x;
                let lr = Rect::from_min_size(pos2(bx - lw, hy - 8.0), vec2(lw, 16.0));
                let lrsp = ui.interact(lr, ui.id().with("rates"), Sense::click()).on_hover_text("Rates By Exchange Rate API");
                p.text(pos2(lr.right(), hy), Align2::RIGHT_CENTER, &lbl, FontId::proportional(10.5), if stale { Color32::from_rgba_unmultiplied(252, 211, 77, 204) } else if lrsp.hovered() { Color32::from_rgb(221, 214, 254) } else { Color32::from_rgba_unmultiplied(196, 181, 253, 179) });
                if lrsp.clicked() { let _ = quick::open_shell("https://www.exchangerate-api.com"); }
                left_edge = lr.left() - 8.0;
            }
        }
        // anexo
        let mut fx = r.left() + 52.0;
        if let (Some(f), true) = (&file, inl.is_none()) {
            let name_w = ui.painter().layout_no_wrap(f.name.clone(), FontId::proportional(10.5), Color32::WHITE).size().x.min(150.0 - 6.0 - 15.0 - 4.0 - if f.sent { 0.0 } else { 20.0 });
            let cw = 6.0 + 11.0 + 4.0 + name_w + if f.sent { 4.0 } else { 4.0 + 16.0 + 4.0 };
            let cr = Rect::from_min_size(pos2(fx, hy - 12.0), vec2(cw, 24.0));
            p.rect_filled(cr, CornerRadius::same(12), if f.sent { Color32::from_white_alpha(15) } else { Color32::from_rgba_unmultiplied(139, 92, 246, 51) });
            let fg = if f.sent { Color32::from_white_alpha(115) } else { Color32::from_rgb(237, 233, 254) };
            p.text(pos2(cr.left() + 6.0, hy), Align2::LEFT_CENTER, ic::PAPERCLIP, icon_font(11.0), fg);
            p.text(pos2(cr.left() + 21.0, hy), Align2::LEFT_CENTER, ellipsize_w(ui, &f.name, 10.5, name_w + 1.0), FontId::proportional(10.5), fg);
            ui.interact(cr, ui.id().with("askfile"), Sense::hover()).on_hover_text(f.path.display().to_string());
            if !f.sent {
                let xr = Rect::from_center_size(pos2(cr.right() - 4.0 - 8.0, hy), vec2(16.0, 16.0));
                let xrsp = ui.interact(xr, ui.id().with("askfilex"), Sense::click());
                if xrsp.hovered() { p.circle_filled(xr.center(), 8.0, Color32::from_white_alpha(38)); }
                p.text(xr.center(), Align2::CENTER_CENTER, ic::X, icon_font(10.0), fg);
                if xrsp.clicked() { self.ask_file = None; }
            }
            fx = cr.right() + 8.0;
        }
        // campo (com resposta instantânea, o campo encolhe para caber só o texto)
        let qw = ui.painter().layout_no_wrap(self.ask_q.clone(), FontId::proportional(14.5), Color32::WHITE).size().x + 8.0;
        let field_right = if inl.is_some() { (fx + 4.0 + qw).min(left_edge - 120.0) } else { left_edge };
        let fr = Rect::from_min_max(pos2(fx + if inl.is_some() { 4.0 } else { 0.0 }, hy - 11.0), pos2(field_right.max(fx + 30.0), hy + 11.0));
        let hint = if listening { "Ouvindo… fale em português".to_string() } else if let Some(f) = file.as_ref().filter(|f| !f.sent) { if f.screen { "Pergunte sobre a tela".into() } else { format!("Pergunte sobre {}", f.name) } } else if st.phase == ai::Phase::Idle { "Pergunte ao Kon".into() } else { "Pergunte de novo".into() };
        let before = self.ask_q.clone();
        let te = egui::TextEdit::singleline(&mut self.ask_q).hint_text(egui::RichText::new(hint).color(Color32::from_white_alpha(90))).frame(false).font(FontId::proportional(14.5)).text_color(Color32::WHITE).desired_width(fr.width());
        let resp = ui.put(fr, te);
        // só números e operadores: o * vira × enquanto você digita
        if self.ask_q != before && !self.ask_q.is_empty() && self.ask_q.chars().all(|c| c.is_ascii_digit() || c.is_whitespace() || ".,+-*×xX÷/%()=".contains(c)) && self.ask_q.contains('*') { self.ask_q = self.ask_q.replace('*', "×"); }
        if !resp.has_focus() && ui.memory(|m| m.focused().is_none()) { resp.request_focus(); }
        if resp.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) { let q = std::mem::take(&mut self.ask_q); self.open_translate(q); return; }
        // resultado instantâneo: clique (ou Enter) copia
        let mut copy_inline = false;
        if let Some(inl) = &inl {
            let txt = inl.text().to_string();
            let tw = ui.painter().layout_no_wrap(txt.clone(), bold(15.0), Color32::WHITE).size().x;
            let ir = Rect::from_min_size(pos2(fr.right() + 10.0, hy - 12.0), vec2(1.0 + 10.0 + tw + 10.0 + 20.0, 24.0));
            let irsp = ui.interact(ir, ui.id().with(("inl", txt.as_str())), Sense::click()).on_hover_text("Clique para copiar");
            p.vline(ir.left(), (hy - 10.0)..=(hy + 10.0), Stroke::new(1.0_f32, Color32::from_white_alpha(51)));
            let tx = ir.left() + 11.0;
            for (d, a) in [(2.5f32, 10u8), (1.2, 16)] { for (dx, dy) in [(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)] { p.text(pos2(tx + dx, hy + dy), Align2::LEFT_CENTER, &txt, bold(15.0), Color32::from_white_alpha(a)); } }
            p.text(pos2(tx, hy), Align2::LEFT_CENTER, &txt, bold(15.0), Color32::WHITE);
            let copied = self.copied_at.map_or(false, |t| t.elapsed() < Duration::from_millis(1500));
            p.text(pos2(tx + tw + 10.0 + 10.0, hy), Align2::CENTER_CENTER, if copied { ic::CHECK } else { ic::COPY }, icon_font(if copied { 13.0 } else { 12.0 }), if copied { Color32::from_rgb(52, 211, 153) } else { Color32::from_white_alpha(if irsp.hovered() { 255 } else { 115 }) });
            copy_inline = irsp.clicked();
        }
        let enter = (resp.lost_focus() || resp.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if std::env::var("KON_DEBUG").is_ok() && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { .. }))) { eprintln!("keys: focus={} lost={} enter={}", resp.has_focus(), resp.lost_focus(), enter); }
        let inline_ok = inl.as_ref().map_or(false, |i| !matches!(i, calc::Inline::Unit(_, true)) && !i.text().contains("carregando") && !i.text().contains("desconhecida"));
        if copy_inline || (enter && !can_ask && inline_ok) {
            if let Some(i) = &inl { ui.ctx().copy_text(i.text().to_string()); self.copied_at = Some(Instant::now()); }
            resp.request_focus();
        } else if (enter || srsp.clicked()) && can_ask {
            if self.ask_q.trim().is_empty() { self.ask_q = default_q.into(); }
            self.submit_ask(ui.ctx()); resp.request_focus();
        } else if srsp.clicked() { resp.request_focus(); }
        if st.phase == ai::Phase::Idle { return; }
        // resposta
        let body = Rect::from_min_max(pos2(r.left() + 20.0, r.top() + 46.0), pos2(r.right() - 20.0, r.bottom() - 42.0));
        let pc = p.with_clip_rect(body);
        pc.text(pos2(body.left(), body.top() + 6.0), Align2::LEFT_CENTER, ellipsize_w(ui, &st.question, 11.5, body.width()), FontId::proportional(11.5), Color32::from_white_alpha(102));
        let mut y = body.top() + 16.0;
        let mut rows = 0.0;
        let text = if st.phase == ai::Phase::Thinking { "Pensando…".to_string() } else { st.answer.clone() };
        for (i, line) in text.split('\n').enumerate() {
            if line.trim().is_empty() { y += 8.0; rows += 0.45; continue; }
            let col = if st.error.is_some() { Color32::from_rgba_unmultiplied(253, 230, 138, 230) } else if i == 0 { Color32::WHITE } else { Color32::from_white_alpha(217) };
            let font = if i == 0 && st.error.is_none() { bold(12.5) } else { FontId::proportional(12.5) };
            let mut job = egui::text::LayoutJob::single_section(line.to_string(), egui::TextFormat { font_id: font, color: col, line_height: Some(18.0), ..Default::default() });
            job.wrap.max_width = body.width();
            let g = ui.fonts_mut(|f| f.layout_job(job));
            let n = g.rows.len() as f32;
            pc.galley(pos2(body.left(), y), g, col);
            y += n * 18.0; rows += n;
        }
        if st.phase == ai::Phase::Streaming || st.phase == ai::Phase::Thinking {
            let on = (ui.input(|i| i.time) * 2.5).fract() < 0.5;
            if on { pc.rect_filled(Rect::from_min_size(pos2(body.left(), y + 2.0), vec2(6.0, 14.0)), CornerRadius::same(2), Color32::from_rgba_unmultiplied(165, 180, 252, 204)); }
            ui.ctx().request_repaint_after(Duration::from_millis(120));
        }
        // fontes da busca do Google
        if !st.sources.is_empty() {
            let mut sx = body.left(); let sy = y + 14.0;
            for (i, (url, title)) in st.sources.iter().enumerate() {
                let t = ellipsize_w(ui, title, 10.5, 130.0);
                let w = text_w(ui, &t, 10.5) + 30.0;
                if sx + w > body.right() { break; }
                let sr = Rect::from_min_size(pos2(sx, sy - 11.0), vec2(w, 22.0));
                let rsp = ui.interact(sr.intersect(body), ui.id().with(("src", i)), Sense::click()).on_hover_text(url);
                pc.rect_filled(sr, CornerRadius::same(11), Color32::from_white_alpha(if rsp.hovered() { 36 } else { 18 }));
                pc.text(pos2(sr.left() + 9.0, sy), Align2::LEFT_CENTER, format!("{}", i + 1), FontId::proportional(10.5), Color32::from_rgba_unmultiplied(125, 211, 252, 204));
                pc.text(pos2(sr.left() + 20.0, sy), Align2::LEFT_CENTER, t, FontId::proportional(10.5), Color32::from_white_alpha(178));
                if rsp.clicked() { let _ = quick::open_shell(url); }
                sx += w + 4.0;
            }
        }
        self.ask_rows = rows;
        // rodapé
        let fy = r.bottom() - 21.0;
        let label = format!("{pname}{}{}", if pv == "gemini" && self.settings.web_search { " + Google Search" } else { "" },
            st.fell_back.as_ref().map_or(" · Enter para continuar".to_string(), |(a, b)| format!(" · {a} indisponível, usei {b}")));
        let mut rx = r.right() - 16.0;
        if small_pill(ui, &p, &mut rx, fy, Some(ic::PLUS), "Novo chat", false, 1.0) { self.open_ask(); }
        if st.error.as_ref().map_or(false, |e| e.0 == "NO_KEY") {
            if small_pill(ui, &p, &mut rx, fy, Some(ic::KEY_ROUND), "Configurar", false, 1.0) { self.view = View::Sheet; self.sheet_tab = 0; self.sheet_scroll = 0.0; }
        } else {
            let copied = self.copied_at.map_or(false, |t| t.elapsed() < Duration::from_millis(1500));
            if small_pill(ui, &p, &mut rx, fy, Some(if copied { ic::CHECK } else { ic::COPY }), "Copiar", false, 1.0) {
                ui.ctx().copy_text(st.answer.clone()); self.copied_at = Some(Instant::now());
            }
            if copied { ui.ctx().request_repaint_after(Duration::from_millis(300)); }
        }
        p.text(pos2(r.left() + 16.0, fy), Align2::LEFT_CENTER, ellipsize_w(ui, &label, 11.0, rx - r.left() - 24.0), FontId::proportional(11.0), Color32::from_white_alpha(102));
    }

    fn page_sleep(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let gap = 10.0;
        let lw = (body.width() - gap) * 1.0 / 2.25;
        let l = Rect::from_min_size(body.min, vec2(lw, body.height()));
        let r = Rect::from_min_max(pos2(l.right() + gap, body.top()), body.max);
        let dnd = self.settings.dnd;
        gradient_rr(&p, l, 16.0, Color32::from_rgb(30, 27, 75), Color32::from_rgb(10, 10, 10), alpha);
        p.rect_stroke(l, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside);
        let mc = pos2(l.left() + 30.0, l.top() + 30.0);
        p.circle_filled(mc, 18.0, fade(if dnd { Color32::from_rgba_unmultiplied(129, 140, 248, 230) } else { Color32::from_white_alpha(20) }, alpha));
        p.text(mc, Align2::CENTER_CENTER, ic::MOON, icon_font(17.0), fade(Color32::WHITE, alpha));
        let tg = toggle_c(ui, &p, pos2(l.right() - 32.0, l.top() + 30.0), dnd, alpha, Color32::from_rgb(99, 102, 241));
        p.text(pos2(l.left() + 12.0, l.bottom() - 40.0), Align2::LEFT_CENTER, "Não incomodar", bold(16.0), fade(Color32::WHITE, alpha));
        p.text(pos2(l.left() + 12.0, l.bottom() - 20.0), Align2::LEFT_CENTER, if dnd { "Ligado — avisos do notch em silêncio" } else { "Desligado — o notch pode mostrar avisos" }, FontId::proportional(12.0), fade(Color32::from_white_alpha(128), alpha));
        if tg.clicked() { self.settings.dnd = !dnd; self.settings.save(); }
        // timer para dormir
        p.rect_filled(r, CornerRadius::same(16), fade(Color32::from_white_alpha(8), alpha));
        p.rect_stroke(r, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside);
        p.text(pos2(r.left() + 12.0, r.top() + 20.0), Align2::LEFT_CENTER, "TIMER PARA DORMIR", bold(10.0), fade(Color32::from_rgba_unmultiplied(199, 210, 254, 178), alpha));
        p.text(pos2(r.right() - 12.0, r.top() + 20.0), Align2::RIGHT_CENTER, "Pausa a música", FontId::proportional(11.0), fade(Color32::from_white_alpha(102), alpha));
        let left = self.sleep_until.map(|t| t.saturating_duration_since(Instant::now()).as_secs_f32());
        match left {
            None => { p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, "Desligado", FontId::monospace(30.0), fade(Color32::from_white_alpha(90), alpha)); }
            Some(s) => { p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, mmss(s), FontId::monospace(30.0), fade(Color32::WHITE, alpha)); ui.ctx().request_repaint_after(Duration::from_millis(500)); }
        }
        let opts: [Option<u64>; 5] = [None, Some(15), Some(30), Some(45), Some(60)];
        let bw = (r.width() - 24.0 - 4.0 * 6.0) / 5.0;
        let lm = left.map(|s| (s / 60.0).ceil() as u64);
        for (i, o) in opts.iter().enumerate() {
            let br = Rect::from_min_size(pos2(r.left() + 12.0 + i as f32 * (bw + 6.0), r.bottom() - 40.0), vec2(bw, 28.0));
            let active = match (o, lm) { (None, None) => true, (Some(m), Some(l)) => l <= *m && l + 15 > *m, _ => false };
            let lbl = o.map_or("Não".to_string(), |m| format!("{m} min"));
            let resp = ui.interact(br, ui.id().with(("slp", i)), Sense::click());
            p.rect_filled(br, CornerRadius::same(14), fade(if active { Color32::WHITE } else { Color32::from_white_alpha(if resp.hovered() { 38 } else { 25 }) }, alpha));
            p.text(br.center(), Align2::CENTER_CENTER, lbl, FontId::proportional(11.5), fade(if active { Color32::BLACK } else { Color32::from_white_alpha(204) }, alpha));
            if resp.clicked() { self.sleep_until = o.map(|m| Instant::now() + Duration::from_secs(m * 60)); }
        }
    }

    fn page_lock(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let gap = 10.0;
        let hw = (body.width() - gap) / 2.0;
        let l = Rect::from_min_size(body.min, vec2(hw, body.height()));
        let r = Rect::from_min_max(pos2(l.right() + gap, body.top()), body.max);
        let locked = self.settings.locked; let privacy = self.settings.privacy;
        // travar
        if locked { gradient_rr(&p, l, 16.0, Color32::from_rgb(70, 40, 10), Color32::from_rgb(10, 10, 10), alpha); }
        else { gradient_rr(&p, l, 16.0, Color32::from_rgb(25, 25, 25), Color32::from_rgb(10, 10, 10), alpha); }
        p.rect_stroke(l, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside);
        let c = pos2(l.left() + 30.0, l.top() + 30.0);
        p.circle_filled(c, 18.0, fade(if locked { Color32::from_rgb(251, 191, 36) } else { Color32::from_white_alpha(20) }, alpha));
        p.text(c, Align2::CENTER_CENTER, if locked { ic::LOCK } else { ic::LOCK_OPEN }, icon_font(16.0), fade(if locked { Color32::BLACK } else { Color32::WHITE }, alpha));
        let t1 = toggle_c(ui, &p, pos2(l.right() - 32.0, l.top() + 30.0), locked, alpha, Color32::from_rgb(245, 158, 11)).clicked();
        p.text(pos2(l.left() + 12.0, l.bottom() - 46.0), Align2::LEFT_CENTER, if locked { "Ilha travada" } else { "Travar a ilha" }, bold(16.0), fade(Color32::WHITE, alpha));
        let sub = if locked { "Passar o mouse não abre. Clique na ilha ou use o ícone perto do relógio." } else { "O painel não abre ao passar o mouse" };
        let mut job = egui::text::LayoutJob::single_section(sub.into(), egui::TextFormat { font_id: FontId::proportional(12.0), color: fade(Color32::from_white_alpha(128), alpha), ..Default::default() });
        job.wrap.max_width = l.width() - 24.0;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        p.galley(pos2(l.left() + 12.0, l.bottom() - 34.0), g, Color32::WHITE);
        let card_click = ui.interact(l, ui.id().with("lockcard"), Sense::click()).clicked();
        if t1 || card_click { self.settings.locked = !locked; self.settings.save(); }
        // privacidade
        p.rect_filled(r, CornerRadius::same(16), fade(Color32::from_white_alpha(8), alpha));
        p.rect_stroke(r, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(25), alpha)), egui::StrokeKind::Inside);
        let c = pos2(r.left() + 30.0, r.top() + 30.0);
        p.circle_filled(c, 18.0, fade(if privacy { Color32::from_rgb(56, 189, 248) } else { Color32::from_white_alpha(20) }, alpha));
        p.text(c, Align2::CENTER_CENTER, ic::EYE_OFF, icon_font(16.0), fade(if privacy { Color32::BLACK } else { Color32::WHITE }, alpha));
        let t2 = toggle_c(ui, &p, pos2(r.right() - 32.0, r.top() + 30.0), privacy, alpha, Color32::from_rgb(14, 165, 233)).clicked();
        p.text(pos2(r.left() + 12.0, r.bottom() - 46.0), Align2::LEFT_CENTER, "Modo privacidade", bold(16.0), fade(Color32::WHITE, alpha));
        p.text(pos2(r.left() + 12.0, r.bottom() - 26.0), Align2::LEFT_CENTER, "Esconde letras e código dos avisos do notch", FontId::proportional(12.0), fade(Color32::from_white_alpha(128), alpha));
        let card2 = ui.interact(r, ui.id().with("privcard"), Sense::click()).clicked();
        if t2 || card2 { self.settings.privacy = !privacy; self.settings.save(); }
    }

    /// Capa do álbum: decodifica uma vez por faixa e guarda como textura da placa de vídeo.
    fn update_art(&mut self, ctx: &egui::Context, m: &MediaState) {
        let fallback;
        let bytes = match &m.art {
            Some(b) => b,
            None if m.active => { let c = ctx.clone(); fallback = lyrics::cover_fallback(&self.lyr, &m.title, &m.artist, move || c.request_repaint()); match &fallback { Some(b) => b, None => { self.art = None; self.art_color = None; return } } }
            None => { self.art = None; self.art_color = None; return }
        };
        if self.art.as_ref().map_or(false, |(k, _)| *k == m.art_key) { return; }
        if let Ok(img) = image::load_from_memory(bytes) {
            // cor dominante da capa (puxada para as mais saturadas), como no Kon atual: pinta o brilho e as barras
            let small = img.thumbnail_exact(12, 12).to_rgba8();
            let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
            for px in small.pixels() { let [pr, pg, pb, _] = px.0; let sat = pr.max(pg).max(pb) as f32 - pr.min(pg).min(pb) as f32; let w = 1.0 + sat / 40.0; r += pr as f32 * w; g += pg as f32 * w; b += pb as f32 * w; n += w; }
            self.art_color = if n > 0.0 { Some(Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)) } else { None };
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
            // sem capa: from-indigo-900 via-purple-900 to-pink-800 com a nota, como no Kon atual
            rr_mesh(ui.painter(), r, radius as f32, |pt| { let t = (((pt.x - r.left()) / r.width()) * 0.5 + ((pt.y - r.top()) / r.height()) * 0.5).clamp(0.0, 1.0); let c = if t < 0.5 { lerp_c(Color32::from_rgb(49, 46, 129), Color32::from_rgb(88, 28, 135), t * 2.0) } else { lerp_c(Color32::from_rgb(88, 28, 135), Color32::from_rgb(157, 23, 77), t * 2.0 - 1.0) }; fade(c, alpha) });
            ui.painter().text(r.center(), Align2::CENTER_CENTER, ic::MUSIC_2, icon_font(r.width() / 3.0), fade(Color32::from_white_alpha(153), alpha));
        }
    }

    /// Avisos aparecem na ilha fechada, como no Kon atual (com o painel aberto, assim que ele fecha).
    fn toast(&mut self, m: &str) { self.ctoast = Some((m.into(), Instant::now(), false)); }
    fn toast_err(&mut self, m: &str) { self.sfx("error"); self.ctoast = Some((m.into(), Instant::now(), true)); }

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
        self.tofu(ui, pos2(r.left() + 16.0 + 23.0, r.center().y), 1.45, Mood::Attention, blink, alpha);
        let x = r.left() + 16.0 + 46.0 + 12.0;
        let bw_deny = text_w(ui, "Negar", 11.5) + 24.0; let bw_ok = text_w(ui, "Permitir", 11.5) + 24.0;
        let right = r.right() - 14.0;
        let ok_r = Rect::from_min_size(pos2(right - bw_ok, r.center().y - 13.0), vec2(bw_ok, 26.0));
        let deny_r = Rect::from_min_size(pos2(ok_r.left() - 6.0 - bw_deny, r.center().y - 13.0), vec2(bw_deny, 26.0));
        let maxw = deny_r.left() - 12.0 - x;
        p.text(pos2(x, r.center().y - 22.0), Align2::LEFT_CENTER, ellipsize_w(ui, &title, 13.5, maxw), bold(13.5), fade(Color32::WHITE, alpha));
        let sub = if self.settings.privacy { mask(&sub) } else { sub };
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
        // brilho âmbar difuso (w-60 h-60 bg-amber-500/20 blur-3xl em -left-16 -top-16)
        soft_glow(&p.with_clip_rect(island_of(body)), pos2(area.left() - 64.0 + 120.0, area.top() - 64.0 + 120.0), 120.0, 120.0, fade(Color32::from_rgba_unmultiplied(245, 158, 11, 51), alpha));
        let has_perm = !self.claude.lock().unwrap().perms.is_empty();
        let cm = if has_perm { Mood::Attention } else if done { Mood::Done } else if running { Mood::Working } else { Mood::Idle };
        self.tofu(ui, pos2(left.center().x, left.top() + 34.0), 2.7 * 0.85, cm, blink, alpha);
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
        let dr = Rect::from_min_size(pos2(right.left() + 8.0, right.top() + 43.0), vec2(right.width() - 12.0, 72.0));
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
        if self.settings.privacy {
            p.rect_filled(dr.shrink(1.0), CornerRadius::same(11), fade(Color32::from_rgb(12, 12, 12), alpha));
            p.text(dr.center(), Align2::CENTER_CENTER, "Oculto pelo modo privacidade", FontId::proportional(11.0), fade(Color32::from_white_alpha(153), alpha));
        }
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
        p.text(pos2(c3.left() + 10.0, c3.bottom() - 14.0), Align2::LEFT_CENTER, format!("{ss} {} hoje · chat do Kon {}", if ss == 1 { "sessão" } else { "sessões" }, usage::fmt_tok(ai::kon_tokens_today())), FontId::proportional(10.0), dim(115));
    }

    fn draw_sheet(&mut self, ui: &mut egui::Ui, r: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let dim = |a: u8| fade(Color32::from_white_alpha(a), alpha);
        let inner = Rect::from_min_max(pos2(r.left() + 16.0, r.top() + 12.0), pos2(r.right() - 16.0, r.bottom() - 16.0));
        let tab = self.sheet_tab;
        let (title, sub) = match tab { 2 => ("Bem-vindo ao Kon", "Três passos e pronto."), 3 => ("Relatar problema", "Junte as informações para eu conseguir corrigir."), 1 => ("Pages", "Arraste para reordenar. Desligue o que não usa."), _ => ("Configurações", "IA, notch, Claude Code, agenda e atualizações.") };
        p.text(pos2(inner.left(), inner.top() + 12.0), Align2::LEFT_CENTER, title, bold(17.0), dim(255));
        let mut job = egui::text::LayoutJob::single_section(sub.into(), egui::TextFormat { font_id: FontId::proportional(12.0), color: dim(128), line_height: Some(17.0), ..Default::default() });
        job.wrap.max_width = 200.0;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        let sub_h = g.size().y;
        p.galley(pos2(inner.left(), inner.top() + 26.0), g, Color32::WHITE);
        let xr = Rect::from_center_size(pos2(inner.right() - 12.0, inner.top() + 12.0), vec2(24.0, 24.0));
        let xresp = ui.interact(xr, ui.id().with("sheetx"), Sense::click());
        if xresp.hovered() { p.circle_filled(xr.center(), 12.0, dim(25)); }
        p.text(xr.center(), Align2::CENTER_CENTER, ic::X, icon_font(13.0), dim(if xresp.hovered() { 255 } else { 128 }));
        if xresp.clicked() { if tab == 2 { self.finish_onboarding(); } else { self.view = View::Panel; } return; }
        if tab <= 1 {
            let mut t = tab;
            if seg_h(ui, &p, pos2(xr.left() - 6.0, inner.top() + 12.0), &[("Ajustes", 0u8), ("Pages", 1u8)], &mut t, alpha, "sheettab", 24.0, 10.5) { self.sheet_tab = t; self.sheet_scroll = 0.0; }
        }
        let area = Rect::from_min_max(pos2(inner.left(), inner.top() + 26.0 + sub_h + 12.0), inner.max);
        match tab { 1 => { self.sheet_pages_list(ui, area, alpha); return; } 2 => { self.draw_welcome(ui, area, alpha); return; } 3 => { self.draw_report(ui, area, alpha); return; } _ => {} }
        if ui.rect_contains_pointer(area) { self.sheet_scroll -= ui.input(|i| i.smooth_scroll_delta.y); }
        let pc = p.with_clip_rect(area);
        let w = area.width();
        let mut y = area.top() - self.sheet_scroll;
        let vis = |rr: Rect| area.contains(rr.center());
        let gap = 10.0;
        let row = |y: f32, x: f32, wd: f32, h: f32| { let rr = Rect::from_min_size(pos2(x, y), vec2(wd, h)); pc.rect_filled(rr, CornerRadius::same(12), dim(10)); rr };
        let ctx_s = ui.ctx().clone();
        let section = |y: &mut f32, t: &str| {
            let mut job = egui::text::LayoutJob::single_section(t.to_string(), egui::TextFormat { font_id: bold(10.5), color: dim(102), extra_letter_spacing: 1.05, ..Default::default() });
            job.wrap.max_width = f32::INFINITY;
            let g = ctx_s.fonts_mut(|f| f.layout_job(job));
            pc.galley(pos2(area.left(), *y + 7.0 - g.size().y / 2.0), g, Color32::WHITE);
            *y += 14.0 + gap;
        };
        let green = Color32::from_rgb(16, 185, 129);
        // ── IA ──
        pc.text(pos2(area.left(), y + 14.0), Align2::LEFT_CENTER, "IA padrão", FontId::proportional(12.0), dim(179));
        let mut pv = self.settings.provider.clone();
        if vis(Rect::from_min_size(pos2(area.right() - 60.0, y), vec2(10.0, 28.0))) && seg_h(ui, &pc, pos2(area.right(), y + 14.0), &[("Gemini", "gemini".to_string()), ("Claude", "claude".to_string())], &mut pv, alpha, "prov", 28.0, 11.0) { self.settings.provider = pv; self.settings.save(); }
        y += 28.0 + gap;
        for (k, (prov, label, hint, link)) in [("gemini", "GEMINI_API_KEY", "AQ.… ou AIza…", "https://aistudio.google.com/apikey"), ("anthropic", "ANTHROPIC_API_KEY", "sk-ant-…", "https://console.anthropic.com/settings/keys")].into_iter().enumerate() {
            let has = !(if k == 0 { &self.settings.gemini_key_enc } else { &self.settings.anthropic_key_enc }).is_empty();
            pc.text(pos2(area.left(), y + 8.0), Align2::LEFT_CENTER, ic::KEY_ROUND, icon_font(11.0), dim(191));
            pc.text(pos2(area.left() + 17.0, y + 8.0), Align2::LEFT_CENTER, label, bold(11.5), dim(191));
            let lt = "Obter chave";
            let lw = text_w(ui, lt, 10.5) + 14.0;
            let lr = Rect::from_min_size(pos2(area.right() - lw, y), vec2(lw, 16.0));
            let lrsp = ui.interact(lr.intersect(area), ui.id().with(("keylink", k)), Sense::click());
            let lc = fade(Color32::from_rgba_unmultiplied(125, 211, 252, if lrsp.hovered() { 255 } else { 204 }), alpha);
            pc.text(pos2(area.right() - 13.0, y + 8.0), Align2::RIGHT_CENTER, lt, FontId::proportional(10.5), lc);
            pc.text(pos2(area.right(), y + 8.0), Align2::RIGHT_CENTER, ic::EXTERNAL_LINK, icon_font(9.0), lc);
            if lrsp.clicked() { let _ = quick::open_shell(link); }
            y += 16.0 + 4.0;
            let fr = Rect::from_min_size(pos2(area.left(), y), vec2(w, 32.0));
            pc.rect_filled(fr, CornerRadius::same(8), dim(15));
            let ir = Rect::from_min_max(pos2(fr.left() + 10.0, fr.top() + 7.0), pos2(fr.right() - if has { 46.0 } else { 30.0 }, fr.bottom() - 7.0));
            let mut focused = false;
            if vis(fr) {
                let ph = if has { "•••••••• salva — cole para trocar" } else { hint };
                let te = egui::TextEdit::singleline(&mut self.key_in[k]).password(!self.show_key).hint_text(egui::RichText::new(ph).color(dim(77))).frame(false).font(FontId::monospace(12.0)).text_color(Color32::WHITE).desired_width(ir.width());
                let resp = ui.put(ir, te);
                focused = resp.has_focus();
                let enter = (resp.lost_focus() || resp.has_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter || (resp.lost_focus() && !self.key_in[k].trim().is_empty()) {
                    let v = self.key_in[k].trim().to_string();
                    if !v.is_empty() {
                        if secrets::valid(prov, &v) {
                            let enc = secrets::protect(&v);
                            if enc.is_empty() { self.toast_err("Não consegui criptografar a chave"); }
                            else { if k == 0 { self.settings.gemini_key_enc = enc } else { self.settings.anthropic_key_enc = enc } self.settings.save(); self.flash_success(); }
                        } else { self.toast_err("Esse não parece o formato da chave"); }
                    }
                    self.key_in[k].clear(); // a chave nunca fica guardada na tela
                }
                let er = Rect::from_center_size(pos2(fr.right() - if has { 32.0 } else { 14.0 }, fr.center().y), vec2(18.0, 18.0));
                let ersp = ui.interact(er, ui.id().with(("eye", k)), Sense::click());
                pc.text(er.center(), Align2::CENTER_CENTER, if self.show_key { ic::EYE_OFF } else { ic::EYE }, icon_font(12.0), dim(if ersp.hovered() { 255 } else { 102 }));
                if ersp.clicked() { self.show_key = !self.show_key; }
                if has { pc.text(pos2(fr.right() - 14.0, fr.center().y), Align2::CENTER_CENTER, ic::CHECK, icon_font(12.0), fade(Color32::from_rgb(52, 211, 153), alpha)); }
            }
            pc.rect_stroke(fr, CornerRadius::same(8), Stroke::new(1.0_f32, if focused { fade(Color32::from_rgba_unmultiplied(129, 140, 248, 153), alpha) } else { dim(20) }), egui::StrokeKind::Inside);
            y += 32.0 + gap;
        }
        pc.text(pos2(area.left(), y - 4.0 + 6.0), Align2::LEFT_CENTER, "Chaves criptografadas pelo Windows (DPAPI), só neste computador.", FontId::proportional(10.0), dim(90));
        y += 12.0 + gap - 4.0;
        let half = (w - 8.0) / 2.0;
        for (k, label) in ["Modelo Gemini", "Modelo Claude"].iter().enumerate() {
            let x = area.left() + k as f32 * (half + 8.0);
            pc.text(pos2(x, y + 7.0), Align2::LEFT_CENTER, *label, FontId::proportional(10.5), dim(140));
            let fr = Rect::from_min_size(pos2(x, y + 18.0), vec2(half, 32.0));
            pc.rect_filled(fr, CornerRadius::same(8), dim(15));
            pc.rect_stroke(fr, CornerRadius::same(8), Stroke::new(1.0_f32, dim(20)), egui::StrokeKind::Inside);
            if vis(fr) {
                if self.model_in[k].is_empty() { self.model_in[k] = if k == 0 { self.settings.gemini_model.clone() } else { self.settings.claude_model.clone() }; }
                let te = egui::TextEdit::singleline(&mut self.model_in[k]).frame(false).font(FontId::monospace(11.5)).text_color(Color32::WHITE).desired_width(half - 20.0);
                let resp = ui.put(fr.shrink2(vec2(10.0, 7.0)), te);
                if resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    let v = self.model_in[k].trim().to_string();
                    if !v.is_empty() { if k == 0 { self.settings.gemini_model = v } else { self.settings.claude_model = v } self.settings.save(); }
                }
            }
        }
        y += 50.0 + gap;
        let rr = row(y, area.left(), w, 36.0);
        pc.text(pos2(rr.left() + 10.0 + 7.0, rr.center().y), Align2::CENTER_CENTER, ic::GLOBE, icon_font(14.0), fade(Color32::from_rgba_unmultiplied(125, 211, 252, 204), alpha));
        pc.text(pos2(rr.left() + 36.0, rr.center().y), Align2::LEFT_CENTER, "Busca Google no Gemini (com fontes)", FontId::proportional(12.0), dim(255));
        if vis(rr) && toggle_c(ui, &pc, pos2(rr.right() - 10.0 - 19.0, rr.center().y), self.settings.web_search, alpha, green).clicked() { self.settings.web_search = !self.settings.web_search; self.settings.save(); }
        y += 36.0 + gap;
        // Local
        let rr = row(y, area.left(), w, 36.0);
        pc.text(pos2(rr.left() + 10.0 + 7.0, rr.center().y), Align2::CENTER_CENTER, ic::MAP_PIN, icon_font(14.0), fade(Color32::from_rgba_unmultiplied(110, 231, 183, 204), alpha));
        pc.text(pos2(rr.left() + 32.0, rr.center().y), Align2::LEFT_CENTER, "Local", FontId::proportional(12.0), dim(255));
        let cur = match weather::BR_CITIES.iter().find(|c| c.0 == self.settings.loc_id) { Some(c) => c.1.to_string(), None => format!("Atual ({})", self.weather.lock().unwrap().as_ref().filter(|w| w.loc_id == "auto").map_or("detectando / São Paulo".to_string(), |w| w.city.clone())) };
        let sel = Rect::from_min_max(pos2(rr.left() + 72.0, rr.top() + 4.5), pos2(rr.right() - 6.0, rr.bottom() - 4.5));
        pc.rect_filled(sel, CornerRadius::same(6), dim(15));
        pc.rect_stroke(sel, CornerRadius::same(6), Stroke::new(1.0_f32, dim(20)), egui::StrokeKind::Inside);
        pc.text(pos2(sel.left() + 8.0, sel.center().y), Align2::LEFT_CENTER, &cur, FontId::proportional(11.5), dim(255));
        pc.text(pos2(sel.right() - 10.0, sel.center().y), Align2::CENTER_CENTER, ic::CHEVRON_DOWN, icon_font(13.0), dim(204));
        if vis(sel) && ui.interact(sel, ui.id().with("locsel"), Sense::click()).clicked() { self.loc_menu = !self.loc_menu; }
        let loc_menu_at = sel;
        y += 36.0 + gap;
        // ── Claude Code ──
        let rr = row(y, area.left(), w, 42.0);
        let sq = Rect::from_center_size(pos2(rr.left() + 10.0 + 12.0, rr.center().y), vec2(24.0, 24.0));
        pc.rect_filled(sq, CornerRadius::same(7), fade(Color32::from_rgb(168, 85, 247), alpha));
        pc.text(sq.center(), Align2::CENTER_CENTER, ic::ASTERISK, icon_font(13.0), dim(255));
        pc.text(pos2(sq.right() + 12.0, rr.center().y - 7.0), Align2::LEFT_CENTER, "Claude Code", FontId::proportional(12.0), dim(255));
        let (msg, warn) = match &self.hooks_err { Some(e) => (e.clone(), true), None => (if self.hooks_on { "Conectado · ao vivo na página Claude".into() } else { "Adiciona hooks em ~/.claude/settings.json".into() }, false) };
        pc.text(pos2(sq.right() + 12.0, rr.center().y + 8.0), Align2::LEFT_CENTER, ellipsize_w(ui, &msg, 10.0, w - 150.0), FontId::proportional(10.0), if warn { fade(Color32::from_rgba_unmultiplied(252, 211, 77, 204), alpha) } else { dim(102) });
        let mut rx = rr.right() - 8.0;
        if vis(rr) {
            let lbl = if self.hooks_on { "Desconectar" } else { "Conectar" };
            let bw = text_w(ui, lbl, 11.5) + 24.0;
            let br = Rect::from_min_size(pos2(rx - bw, rr.center().y - 14.0), vec2(bw, 28.0));
            let brsp = ui.interact(br, ui.id().with("hooksbtn"), Sense::click());
            pc.rect_filled(br, CornerRadius::same(14), if self.hooks_on { dim(if brsp.hovered() { 36 } else { 20 }) } else { fade(Color32::from_rgba_unmultiplied(168, 85, 247, if brsp.hovered() { 89 } else { 64 }), alpha) });
            pc.text(br.center(), Align2::CENTER_CENTER, lbl, FontId::proportional(11.5), if self.hooks_on { dim(217) } else { fade(Color32::from_rgb(243, 232, 255), alpha) });
            if brsp.clicked() {
                let res = if self.hooks_on { claude::uninstall_hooks() } else { claude::install_hooks() };
                self.hooks_err = res.err();
                self.hooks_on = claude::hooks_state().0;
            }
            rx = br.left();
        }
        let _ = rx;
        y += 42.0 + gap;
        if self.hooks_on {
            let x0 = area.left() + 8.0 + 12.0;
            let ww = w - 20.0;
            let line_top = y - 4.0;
            let rr = row(y, x0, ww, 40.0);
            pc.text(pos2(rr.left() + 10.0 + 6.5, rr.center().y), Align2::CENTER_CENTER, ic::SHIELD_CHECK, icon_font(13.0), dim(128));
            pc.text(pos2(rr.left() + 33.0, rr.center().y - 7.0), Align2::LEFT_CENTER, "Aprovar pedidos do Claude pelo notch", FontId::proportional(11.5), dim(255));
            pc.text(pos2(rr.left() + 33.0, rr.center().y + 8.0), Align2::LEFT_CENTER, "Negar / Permitir / Sempre, sem ir ao terminal", FontId::proportional(10.0), dim(90));
            if vis(rr) && toggle_c(ui, &pc, pos2(rr.right() - 10.0 - 19.0, rr.center().y), self.settings.approvals, alpha, green).clicked() { self.settings.approvals = !self.settings.approvals; self.settings.save(); self.claude.lock().unwrap().approvals = self.settings.approvals; }
            y += 40.0 + 6.0;
            let rr = row(y, x0, ww, 36.0);
            pc.text(pos2(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, "Plano (para a janela de 5 h)", FontId::proportional(11.5), dim(255));
            let mut plan = self.settings.claude_plan.clone();
            let opts: [(&str, String); 4] = [("Auto", "auto".into()), ("Pro", "pro".into()), ("Max 5×", "max5".into()), ("Max 20×", "max20".into())];
            if vis(rr) && seg(ui, &pc, pos2(rr.right() - 10.0, rr.center().y), &opts, &mut plan, alpha, "plan") { self.settings.claude_plan = plan; self.settings.save(); self.usage_at = None; }
            y += 36.0;
            pc.line_segment([pos2(area.left() + 8.0, line_top), pos2(area.left() + 8.0, y)], Stroke::new(1.0_f32, dim(25)));
            y += gap;
        }
        // ── NOTCH ──
        section(&mut y, "NOTCH");
        let rr = row(y, area.left(), w, 36.0);
        pc.text(pos2(rr.left() + 10.0 + 7.0, rr.center().y), Align2::CENTER_CENTER, ic::MOUSE_POINTER_CLICK, icon_font(14.0), dim(128));
        pc.text(pos2(rr.left() + 32.0, rr.center().y), Align2::LEFT_CENTER, "Abrir o painel", FontId::proportional(11.5), dim(255));
        let mut om = self.settings.open_mode.clone();
        let opts: [(&str, String); 3] = [("Passar o mouse", "hover".into()), ("Com atraso", "delay".into()), ("Clique", "click".into())];
        if vis(rr) && seg(ui, &pc, pos2(rr.right() - 10.0, rr.center().y), &opts, &mut om, alpha, "open") { self.settings.open_mode = om; self.settings.save(); }
        y += 36.0 + gap;
        let hw = (w - 8.0) / 2.0;
        let r1 = row(y, area.left(), hw, 36.0);
        pc.text(pos2(r1.left() + 10.0, r1.center().y), Align2::LEFT_CENTER, "Tamanho", FontId::proportional(11.5), dim(255));
        let mut ns = self.settings.notch_size.clone();
        let opts: [(&str, String); 3] = [("P", "small".into()), ("M", "normal".into()), ("G", "large".into())];
        if vis(r1) && seg(ui, &pc, pos2(r1.right() - 10.0, r1.center().y), &opts, &mut ns, alpha, "nsize") { self.settings.notch_size = ns; self.settings.save(); }
        let r2 = row(y, area.left() + hw + 8.0, hw, 36.0);
        pc.text(pos2(r2.left() + 10.0 + 6.5, r2.center().y), Align2::CENTER_CENTER, ic::MONITOR, icon_font(13.0), dim(128));
        pc.text(pos2(r2.left() + 28.0, r2.center().y), Align2::LEFT_CENTER, "Tela", FontId::proportional(11.5), dim(255));
        let mut disp = self.settings.display.clone();
        let opts: [(&str, String); 2] = [("Principal", "primary".into()), ("Do mouse", "cursor".into())];
        if vis(r2) && seg(ui, &pc, pos2(r2.right() - 10.0, r2.center().y), &opts, &mut disp, alpha, "disp") { self.settings.display = disp; self.settings.save(); }
        y += 36.0 + gap;
        let switch_rows: [(char, &str, &str, u8); 3] = [(ic::EYE_OFF, "Esconder em tela cheia", "jogos, vídeos e apresentações", 0), (ic::MINUS, "Linha fina sobre janelas maximizadas", "passe o mouse para ver o notch", 1), (ic::SHIELD_CHECK, "Fora de prints e compartilhamento de tela", "Teams, Meet, Discord e gravações não mostram o Kon", 2)];
        for (icon, label, sub, k) in switch_rows {
            let rr = row(y, area.left(), w, 40.0);
            pc.text(pos2(rr.left() + 10.0 + 6.5, rr.center().y), Align2::CENTER_CENTER, icon, icon_font(13.0), dim(128));
            pc.text(pos2(rr.left() + 33.0, rr.center().y - 7.0), Align2::LEFT_CENTER, label, FontId::proportional(11.5), dim(255));
            pc.text(pos2(rr.left() + 33.0, rr.center().y + 8.0), Align2::LEFT_CENTER, ellipsize_w(ui, sub, 10.0, w - 100.0), FontId::proportional(10.0), dim(90));
            let on = match k { 0 => self.settings.hide_fullscreen, 1 => self.settings.slim_over_max, _ => self.settings.hide_in_capture };
            if vis(rr) && toggle_c(ui, &pc, pos2(rr.right() - 10.0 - 19.0, rr.center().y), on, alpha, green).clicked() {
                match k { 0 => self.settings.hide_fullscreen = !on, 1 => self.settings.slim_over_max = !on, _ => self.settings.hide_in_capture = !on }
                self.settings.save();
            }
            y += 40.0 + gap;
        }
        // Cor
        let rr = row(y, area.left(), w, 36.0);
        pc.text(pos2(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, "Cor", FontId::proportional(11.5), dim(255));
        let mut cx = rr.right() - 10.0 - 11.0;
        for (id, label, stops) in ACCENTS.iter().rev() {
            let c = pos2(cx, rr.center().y);
            let br = Rect::from_center_size(c, vec2(26.0, 26.0));
            let rsp = ui.interact(br, ui.id().with(("acc", *id)), Sense::click()).on_hover_text(*label);
            let on = self.settings.accent == *id;
            if on || rsp.hovered() { pc.circle_stroke(c, 12.5, Stroke::new(2.0_f32, dim(if on { 255 } else { 77 }))); }
            let b = Rect::from_center_size(c, vec2(22.0, 22.0));
            rr_mesh(&pc, b, 11.0, |pt| { let t = (((pt.x - b.left()) + (pt.y - b.top())) / (b.width() + b.height())).clamp(0.0, 1.0); fade(lerp_c(hexc(stops[0]), hexc(stops[3]), t), alpha) });
            if vis(br) && rsp.clicked() { self.settings.accent = id.to_string(); self.settings.save(); }
            cx -= 22.0 + 6.0;
        }
        y += 36.0 + gap;
        // Brilho
        let rr = row(y, area.left(), w, 36.0);
        pc.text(pos2(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, "Brilho", FontId::proportional(11.5), dim(255));
        let mut gl = self.settings.glow;
        if vis(rr) && slider(ui, &pc, Rect::from_min_max(pos2(rr.left() + 58.0, rr.top()), pos2(rr.right() - 10.0 - 36.0 - 10.0, rr.bottom())), &mut gl, 0.0, 1.5, 0.05, alpha, "glow") { self.settings.glow = gl; self.settings.save(); }
        pc.text(pos2(rr.right() - 10.0, rr.center().y), Align2::RIGHT_CENTER, format!("{}%", (self.settings.glow * 100.0).round()), FontId::proportional(10.5), dim(128));
        y += 36.0 + gap;
        let rr = row(y, area.left(), w, 40.0);
        pc.text(pos2(rr.left() + 10.0 + 6.5, rr.center().y), Align2::CENTER_CENTER, ic::MOON, icon_font(13.0), dim(128));
        pc.text(pos2(rr.left() + 33.0, rr.center().y - 7.0), Align2::LEFT_CENTER, "Não incomodar do Windows durante o Focus", FontId::proportional(11.5), dim(255));
        pc.text(pos2(rr.left() + 33.0, rr.center().y + 8.0), Align2::LEFT_CENTER, "silencia notificações enquanto o timer roda", FontId::proportional(10.0), dim(90));
        if vis(rr) && toggle_c(ui, &pc, pos2(rr.right() - 10.0 - 19.0, rr.center().y), self.settings.focus_dnd, alpha, green).clicked() { self.settings.focus_dnd = !self.settings.focus_dnd; self.settings.save(); }
        y += 40.0 + gap;
        // ── MASCOTE E SONS ──
        section(&mut y, "MASCOTE E SONS");
        let cw = (w - 4.0 * 6.0) / 5.0;
        let mut items: Vec<(String, String, String)> = vec![("auto".into(), "Automática".into(), "muda com o que está acontecendo".into()), ("none".into(), "Sem roupa".into(), String::new())];
        items.extend(mascot::Outfit::ALL.iter().map(|o| (o.1.to_string(), o.2.to_string(), o.3.to_string())));
        let t = ui.input(|i| i.time) as f32;
        let auto_outfit = { let mut s2 = self.settings.clone(); s2.outfit = "auto".into(); let o = std::mem::replace(&mut self.settings, s2); let r = self.outfit(); self.settings = o; r };
        for (i, (id, label, hint)) in items.iter().enumerate() {
            let rr = Rect::from_min_size(pos2(area.left() + (i % 5) as f32 * (cw + 6.0), y + (i / 5) as f32 * (62.0 + 6.0)), vec2(cw, 62.0));
            let rsp = ui.interact(rr.intersect(area), ui.id().with(("outfit", id.as_str())), Sense::click()).on_hover_text(if hint.is_empty() { label.as_str() } else { hint.as_str() });
            let on = self.settings.outfit == *id;
            pc.rect_filled(rr, CornerRadius::same(12), dim(if on { 25 } else if rsp.hovered() { 18 } else { 8 }));
            pc.rect_stroke(rr, CornerRadius::same(12), Stroke::new(1.0_f32, dim(if on { 102 } else { 15 })), egui::StrokeKind::Inside);
            let o = if id == "auto" { auto_outfit } else { mascot::Outfit::from_id(id) };
            mascot::draw(&pc, pos2(rr.center().x, rr.top() + 12.0 + 15.0), 1.05, Mood::Idle, o, false, t, alpha);
            pc.text(pos2(rr.center().x, rr.bottom() - 10.0), Align2::CENTER_CENTER, ellipsize_w(ui, label, 9.5, cw - 8.0), FontId::proportional(9.5), dim(179));
            if vis(rr) && rsp.clicked() { self.settings.outfit = id.clone(); self.settings.save(); }
        }
        y += 2.0 * (62.0 + 6.0) - 6.0 + gap;
        let rr = row(y, area.left(), w, 36.0);
        pc.text(pos2(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, "Sons", FontId::proportional(11.5), dim(255));
        let son = self.settings.sounds_on;
        if vis(rr) && toggle_c(ui, &pc, pos2(rr.right() - 10.0 - 19.0, rr.center().y), son, alpha, green).clicked() { self.settings.sounds_on = !son; self.settings.save(); }
        let mut pk = self.settings.sound_pack.clone();
        let opts: Vec<(&str, String)> = sounds::PACKS.iter().map(|(k, l)| (*l, k.to_string())).collect();
        if vis(rr) && seg(ui, &pc, pos2(rr.right() - 10.0 - 38.0 - 8.0, rr.center().y), &opts, &mut pk, alpha, "pack") { self.settings.sound_pack = pk.clone(); self.settings.save(); sounds::play("done", &pk, self.settings.sound_vol); }
        y += 36.0 + gap;
        if son {
            let rr = row(y, area.left(), w, 36.0);
            pc.text(pos2(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, "Volume", FontId::proportional(11.5), dim(255));
            let mut v = self.settings.sound_vol;
            let srect = Rect::from_min_max(pos2(rr.left() + 62.0, rr.top()), pos2(rr.right() - 10.0 - 36.0 - 10.0, rr.bottom()));
            if vis(rr) && slider(ui, &pc, srect, &mut v, 0.05, 1.0, 0.05, alpha, "svol") { self.settings.sound_vol = v; }
            if ui.input(|i| i.pointer.any_released()) && self.vol_dragging { self.vol_dragging = false; self.settings.save(); sounds::play("attention", &self.settings.sound_pack, self.settings.sound_vol); }
            if ui.ctx().is_being_dragged(ui.id().with(("slider", "svol"))) { self.vol_dragging = true; }
            pc.text(pos2(rr.right() - 10.0, rr.center().y), Align2::RIGHT_CENTER, format!("{}%", (self.settings.sound_vol * 100.0).round()), FontId::proportional(10.5), dim(128));
            y += 36.0 + gap;
        }
        // ── AGENDA ──
        section(&mut y, "AGENDA");
        let fr = Rect::from_min_size(pos2(area.left(), y), vec2(w, 32.0));
        pc.rect_filled(fr, CornerRadius::same(8), dim(15));
        pc.text(pos2(fr.left() + 10.0 + 6.5, fr.center().y), Align2::CENTER_CENTER, ic::CALENDAR_CLOCK, icon_font(13.0), dim(115));
        let mut focused = false;
        if vis(fr) {
            if self.ics_in.is_none() { self.ics_in = Some(self.settings.ics_url.clone()); }
            let mut v = self.ics_in.clone().unwrap_or_default();
            let te = egui::TextEdit::singleline(&mut v).hint_text(egui::RichText::new("Link .ics da sua agenda (Google, Outlook, iCloud)").color(dim(77))).frame(false).font(FontId::proportional(11.5)).text_color(Color32::WHITE).desired_width(w - 60.0);
            let resp = ui.put(Rect::from_min_max(pos2(fr.left() + 30.0, fr.top() + 8.0), pos2(fr.right() - 30.0, fr.bottom() - 8.0)), te);
            focused = resp.has_focus();
            if resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) { let t = v.trim().to_string(); if t != self.settings.ics_url { self.settings.ics_url = t; self.settings.save(); self.cal_refresh(true); } }
            self.ics_in = Some(v);
            if !self.settings.ics_url.is_empty() {
                let rr2 = Rect::from_center_size(pos2(fr.right() - 15.0, fr.center().y), vec2(18.0, 18.0));
                let rsp = ui.interact(rr2, ui.id().with("icsref"), Sense::click()).on_hover_text("Atualizar agora");
                pc.text(rr2.center(), Align2::CENTER_CENTER, ic::REFRESH_CW, icon_font(12.0), dim(if rsp.hovered() { 255 } else { 102 }));
                if rsp.clicked() { self.cal_refresh(true); }
            }
        }
        pc.rect_stroke(fr, CornerRadius::same(8), Stroke::new(1.0_f32, if focused { fade(Color32::from_rgba_unmultiplied(129, 140, 248, 153), alpha) } else { dim(20) }), egui::StrokeKind::Inside);
        y += 32.0 + 4.0;
        let cs = self.cal_status();
        let mut job = egui::text::LayoutJob::single_section(if cs.is_empty() { "No Google Agenda: Configurações › sua agenda › Endereço secreto em formato iCal. O Kon avisa 2 min antes e mostra o botão Entrar.".to_string() } else { cs.clone() }, egui::TextFormat { font_id: FontId::proportional(10.0), color: if cs.starts_with("Erro") { fade(Color32::from_rgba_unmultiplied(252, 211, 77, 204), alpha) } else { dim(90) }, line_height: Some(13.0), ..Default::default() });
        job.wrap.max_width = w;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        let gh = g.size().y;
        pc.galley(pos2(area.left(), y), g, Color32::WHITE);
        y += gh + gap;
        // ── GERAL ──
        section(&mut y, "GERAL");
        let rr = row(y, area.left(), hw, 36.0);
        pc.text(pos2(rr.left() + 10.0, rr.center().y), Align2::LEFT_CENTER, "Iniciar com o Windows", FontId::proportional(11.5), dim(255));
        if vis(rr) && toggle_c(ui, &pc, pos2(rr.right() - 10.0 - 19.0, rr.center().y), self.settings.auto_start, alpha, green).clicked() {
            self.settings.auto_start = !self.settings.auto_start; self.settings.save(); crate::platform::set_autostart(self.settings.auto_start);
        }
        y += 36.0 + gap;
        let rr = row(y, area.left(), w, 40.0);
        pc.text(pos2(rr.left() + 10.0 + 6.5, rr.center().y), Align2::CENTER_CENTER, ic::REFRESH_CW, icon_font(13.0), dim(128));
        pc.text(pos2(rr.left() + 33.0, rr.center().y - 7.0), Align2::LEFT_CENTER, format!("Kon {} · atualizar sozinho", env!("CARGO_PKG_VERSION")), FontId::proportional(11.5), dim(255));
        let (ust, ucol) = self.update_status_text();
        pc.text(pos2(rr.left() + 33.0, rr.center().y + 8.0), Align2::LEFT_CENTER, ellipsize_w(ui, &ust, 10.0, w - 150.0), FontId::proportional(10.0), fade(ucol, alpha));
        if vis(rr) {
            let mut bx = rr.right() - 8.0;
            if self.update_ready() { if small_pill(ui, &pc, &mut bx, rr.center().y, Some(ic::REFRESH_CW), "Reiniciar", true, alpha) { self.install_update(); } }
            else if small_pill(ui, &pc, &mut bx, rr.center().y, None, "Procurar", false, alpha) { self.check_update(true); }
            if toggle_c(ui, &pc, pos2(bx - 19.0 - 2.0, rr.center().y), self.settings.auto_update, alpha, green).clicked() { self.settings.auto_update = !self.settings.auto_update; self.settings.save(); }
        }
        y += 40.0 + gap;
        let mut lx = area.left();
        for (k, (label, col, icon)) in [("Relatar problema", Color32::from_rgb(252, 211, 77), Some(ic::ALERT_TRIANGLE)), ("Rever as boas-vindas", Color32::from_rgb(125, 211, 252), None)].into_iter().enumerate() {
            let tw = text_w(ui, label, 11.0) + if icon.is_some() { 15.0 } else { 0.0 };
            let lr = Rect::from_min_size(pos2(lx, y), vec2(tw, 16.0));
            let rsp = ui.interact(lr.intersect(area), ui.id().with(("geral", k)), Sense::click());
            let c = fade(col.gamma_multiply(if rsp.hovered() { 1.0 } else { 0.8 }), alpha);
            let mut tx = lx;
            if let Some(i) = icon { pc.text(pos2(tx, y + 8.0), Align2::LEFT_CENTER, i, icon_font(11.0), c); tx += 15.0; }
            pc.text(pos2(tx, y + 8.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), c);
            if vis(lr) && rsp.clicked() { if k == 0 { self.sheet_tab = 3; self.report_state = 0; } else { self.sheet_tab = 2; self.ob_step = 0; } self.sheet_scroll = 0.0; }
            lx += tw + 16.0;
        }
        y += 16.0 + gap;
        pc.text(pos2(area.left(), y + 6.0), Align2::LEFT_CENTER, "Ctrl+Alt+Space Perguntar · Ctrl+Alt+T OCR · Ctrl+Alt+E Painel · Ctrl+Alt+V Clips", FontId::proportional(10.0), dim(90));
        y += 16.0;
        let total = y + self.sheet_scroll - area.top();
        self.sheet_scroll = self.sheet_scroll.clamp(0.0, (total - area.height()).max(0.0));
        // lista das cidades (por cima de tudo)
        if self.loc_menu {
            let ids = self.loc_ids();
            let mr = Rect::from_min_size(pos2(loc_menu_at.left(), loc_menu_at.bottom() + 2.0), vec2(loc_menu_at.width(), ids.len() as f32 * 24.0 + 6.0));
            let top = p.clone();
            top.rect_filled(mr, CornerRadius::same(8), fade(Color32::from_rgb(28, 28, 30), alpha));
            top.rect_stroke(mr, CornerRadius::same(8), Stroke::new(1.0_f32, dim(31)), egui::StrokeKind::Inside);
            let mut pick = None;
            for (i, id) in ids.iter().enumerate() {
                let ir = Rect::from_min_size(pos2(mr.left() + 3.0, mr.top() + 3.0 + i as f32 * 24.0), vec2(mr.width() - 6.0, 24.0));
                let rsp = ui.interact(ir, ui.id().with(("locopt", i)), Sense::click());
                if rsp.hovered() || *id == self.settings.loc_id { top.rect_filled(ir, CornerRadius::same(5), dim(if rsp.hovered() { 31 } else { 15 })); }
                let lbl = if id == "auto" { format!("📍 Atual ({})", self.weather.lock().unwrap().as_ref().filter(|w| w.loc_id == "auto").map_or("detectando".to_string(), |w| w.city.clone())) } else { weather::BR_CITIES.iter().find(|c| c.0 == id).map_or(String::new(), |c| c.1.to_string()) };
                top.text(pos2(ir.left() + 8.0, ir.center().y), Align2::LEFT_CENTER, lbl.replace("📍 ", ""), FontId::proportional(11.5), dim(255));
                if rsp.clicked() { pick = Some(id.clone()); }
            }
            if let Some(id) = pick { self.settings.loc_id = id; self.settings.save(); *self.loc.lock().unwrap() = self.settings.loc_id.clone(); self.loc_menu = false; }
            else if ui.input(|i| i.pointer.primary_clicked()) && !ui.rect_contains_pointer(mr) && !ui.rect_contains_pointer(loc_menu_at) { self.loc_menu = false; }
        }
    }

    fn sheet_pages_list(&mut self, ui: &mut egui::Ui, area: Rect, alpha: f32) {
        let p = ui.painter().with_clip_rect(area);
        let ids = self.page_ids();
        let row_h = 40.0; let gap = 4.0;
        if self.drag_page.is_none() && ui.rect_contains_pointer(area) { self.sheet_scroll -= ui.input(|i| i.smooth_scroll_delta.y); }
        let total = ids.len() as f32 * (row_h + gap) - gap;
        self.sheet_scroll = self.sheet_scroll.clamp(0.0, (total - area.height()).max(0.0));
        // enquanto arrasta perto das bordas, rola sozinho
        if let (Some(_), Some(pp)) = (self.drag_page, ui.input(|i| i.pointer.interact_pos())) {
            if pp.y > area.bottom() - 24.0 { self.sheet_scroll = (self.sheet_scroll + 6.0).min((total - area.height()).max(0.0)); }
            if pp.y < area.top() + 24.0 { self.sheet_scroll = (self.sheet_scroll - 6.0).max(0.0); }
        }
        let area = area.translate(vec2(0.0, -self.sheet_scroll));
        let vis = area.translate(vec2(0.0, self.sheet_scroll));
        let meta = |id: &str| -> (&'static str, Color32) { match id {
            "home" => ("Início", Color32::from_rgb(249, 115, 22)), "files" => ("Arquivos", Color32::from_rgb(251, 191, 36)), "clip" => ("Área de transferência", Color32::from_rgb(20, 184, 166)),
            "music" => ("Música", Color32::from_rgb(236, 72, 153)), "focus" => ("Foco", Color32::from_rgb(255, 107, 91)), "weather" => ("Clima", Color32::from_rgb(14, 165, 233)),
            "todo" => ("Tarefas", Color32::from_rgb(132, 204, 22)), "claude" => ("Claude Code", Color32::from_rgb(168, 85, 247)), "quick" => ("Atalhos", Color32::from_rgb(250, 204, 21)),
            "system" => ("Sistema", Color32::from_rgb(148, 163, 184)), _ => ("Água", Color32::from_rgb(56, 189, 248)) } };
        let on_count = ids.iter().filter(|i| !self.settings.pages_off.contains(i)).count();
        // arrastando: a linha segue o mouse e as outras abrem espaço
        let ptr = ui.input(|i| i.pointer.interact_pos());
        let mut order: Vec<usize> = (0..ids.len()).collect();
        let mut target = None;
        if let (Some((from, grab_dy)), Some(pp)) = (self.drag_page, ptr) {
            let y = pp.y - grab_dy;
            let t = (((y - area.top()) / (row_h + gap)).round() as isize).clamp(0, ids.len() as isize - 1) as usize;
            let it = order.remove(from); order.insert(t, it); target = Some((t, y));
        }
        let mut toggle_id: Option<String> = None;
        let mut drag_start: Option<(usize, f32)> = None;
        for (slot, &i) in order.iter().enumerate() {
            let id = &ids[i];
            let dragging = self.drag_page.map_or(false, |d| d.0 == i);
            let y = if dragging { target.map_or(area.top(), |t| t.1) } else { area.top() + slot as f32 * (row_h + gap) };
            let rr = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), row_h));
            if dragging {
                p.rect_filled(rr.expand(2.0).translate(vec2(0.0, 6.0)), CornerRadius::same(14), fade(Color32::from_black_alpha(110), alpha));
                p.rect_filled(rr, CornerRadius::same(12), fade(Color32::from_rgb(38, 38, 40), alpha));
            } else { p.rect_filled(rr, CornerRadius::same(12), fade(Color32::from_white_alpha(10), alpha)); }
            let on = !self.settings.pages_off.contains(id);
            let grip = Rect::from_center_size(pos2(rr.left() + 18.0, rr.center().y), vec2(24.0, 32.0));
            let g = ui.interact(grip.intersect(vis), ui.id().with(("grip", id.as_str())), Sense::drag());
            if g.hovered() || dragging { ui.ctx().set_cursor_icon(if dragging { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab }); }
            p.text(grip.center(), Align2::CENTER_CENTER, ic::GRIP_HORIZONTAL, icon_font(16.0), fade(Color32::from_white_alpha(if g.hovered() || dragging { 178 } else { 90 }), alpha));
            if g.drag_started() { if let Some(pp) = ptr { drag_start = Some((i, pp.y - rr.top())); } }
            let (label, col) = meta(id);
            let sq = Rect::from_center_size(pos2(rr.left() + 48.0, rr.center().y), vec2(28.0, 28.0));
            p.rect_filled(sq, CornerRadius::same(8), fade(col, alpha));
            p.text(sq.center(), Align2::CENTER_CENTER, Page::from(id).icon(), icon_font(15.0), fade(Color32::WHITE, alpha));
            p.text(pos2(sq.right() + 12.0, rr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.5), fade(Color32::from_white_alpha(if on { 255 } else { 102 }), alpha));
            if vis.contains(pos2(rr.right() - 28.0, rr.center().y)) && toggle_c(ui, &p, pos2(rr.right() - 28.0, rr.center().y), on, alpha, Color32::from_rgb(16, 185, 129)).clicked() && !(on && on_count <= 1) { toggle_id = Some(id.clone()); }
        }
        if let Some(d) = drag_start { self.drag_page = Some(d); }
        if self.drag_page.is_some() && !ui.input(|i| i.pointer.primary_down()) {
            // soltou: grava a nova ordem
            let new: Vec<String> = order.iter().map(|&i| ids[i].clone()).collect();
            self.settings.page_order = new; self.settings.save(); self.drag_page = None;
        }
        if self.drag_page.is_some() { ui.ctx().request_repaint(); }
        if let Some(id) = toggle_id {
            if let Some(k) = self.settings.pages_off.iter().position(|x| *x == id) { self.settings.pages_off.remove(k); } else { self.settings.pages_off.push(id); }
            self.settings.save();
        }
    }

    fn page_files(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let n = self.stash.list.len();
        let y0 = body.top() + 9.0;
        p.text(pos2(body.left(), y0), Align2::LEFT_CENTER, "Arquivos", bold(15.0), fade(Color32::WHITE, alpha));
        let sub = if n == 0 { "gaveta vazia".to_string() } else { format!("{n} {}", if n == 1 { "item guardado" } else { "itens guardados" }) };
        p.text(pos2(body.left() + text_w(ui, "Arquivos", 15.0) + 8.0, y0 + 1.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), fade(Color32::from_white_alpha(102), alpha));
        p.text(pos2(body.right(), y0), Align2::RIGHT_CENTER, if cfg!(windows) { "Arraste para fora para usar · nada é enviado para IA" } else { "Clique duas vezes para abrir · nada é enviado para IA" }, FontId::proportional(10.5), fade(Color32::from_white_alpha(90), alpha));
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
            let resp = ui.interact(vis, ui.id().with(("stash", it.id)), Sense::click_and_drag()).on_hover_text(&it.path);
            // arrastar para fora: solta em qualquer programa (Explorer, e-mail, WhatsApp...)
            if resp.drag_started() && !it.missing { self.drag_req = Some(it.path.clone()); }
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
            let shown = if self.settings.privacy { mask(&c.text) } else { c.text.clone() };
            let mut job = egui::text::LayoutJob::single_section(shown.replace(['\r', '\t'], " "), egui::TextFormat { font_id: FontId::proportional(12.5), color: fade(Color32::WHITE, alpha), line_height: Some(17.0), ..Default::default() });
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
            match quick::run(&self.quick[i]) { Ok(()) => self.view = View::Compact, Err(e) => { self.toast_err(&e); self.view = View::Compact; } }
        }
    }

    fn loc_ids(&self) -> Vec<String> { let mut v = vec!["auto".to_string()]; v.extend(weather::BR_CITIES.iter().map(|c| c.0.to_string())); v }
    fn cycle_loc(&mut self, d: i32) {
        let ids = self.loc_ids();
        let i = ids.iter().position(|x| *x == self.settings.loc_id).unwrap_or(0) as i32;
        self.settings.loc_id = ids[((i + d).rem_euclid(ids.len() as i32)) as usize].clone();
        self.settings.save();
        *self.loc.lock().unwrap() = self.settings.loc_id.clone();
    }
    fn weather_header(&mut self, ui: &mut egui::Ui, r: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let hy = r.top() + 23.0;
        let white = |a2: u8| fade(Color32::from_white_alpha(a2), alpha);
        let tw = text_w(ui, "Clima", 13.0);
        p.text(pos2(r.left() + 20.0, hy), Align2::LEFT_CENTER, "Clima", bold(13.0), white(255));
        // bandeirinha do Brasil (o original usa o emoji 🇧🇷)
        let fx = r.left() + 20.0 + tw + 6.0;
        let fl = Rect::from_min_size(pos2(fx, hy - 5.5), vec2(15.0, 11.0));
        p.rect_filled(fl, CornerRadius::same(2), fade(Color32::from_rgb(0, 156, 59), alpha));
        p.add(egui::Shape::convex_polygon(vec![pos2(fl.center().x, fl.top() + 1.5), pos2(fl.right() - 1.5, fl.center().y), pos2(fl.center().x, fl.bottom() - 1.5), pos2(fl.left() + 1.5, fl.center().y)], fade(Color32::from_rgb(255, 223, 0), alpha), Stroke::NONE));
        p.circle_filled(fl.center(), 2.6, fade(Color32::from_rgb(0, 39, 118), alpha));
        let w = self.weather.lock().unwrap().clone();
        let label = match weather::BR_CITIES.iter().find(|c| c.0 == self.settings.loc_id) {
            Some(c) => c.1.to_string(),
            None => w.as_ref().filter(|w| w.loc_id == "auto").map_or("localizando…".to_string(), |w| format!("{} · atual", w.city)),
        };
        p.text(pos2(fx + 15.0 + 5.0, hy), Align2::LEFT_CENTER, label, FontId::proportional(13.0), white(115));
        // relógio da cidade escolhida
        let tz = weather::BR_CITIES.iter().find(|c| c.0 == self.settings.loc_id).map(|c| c.4);
        let clock = match tz.and_then(|t| t.parse::<chrono_tz::Tz>().ok()) { Some(z) => chrono::Utc::now().with_timezone(&z).format("%H:%M").to_string(), None => chrono::Local::now().format("%H:%M").to_string() };
        let cw = text_w(ui, &clock, 12.0);
        p.text(pos2(r.right() - 20.0, hy), Align2::RIGHT_CENTER, &clock, FontId::proportional(12.0), white(217));
        // pontinhos das cidades
        let ids = self.loc_ids();
        let mut x = r.right() - 20.0 - cw - 8.0;
        let mut pick = None;
        for id in ids.iter().rev() {
            let on = *id == self.settings.loc_id;
            let wdt = if on { 20.0 } else { 6.0 };
            let d = Rect::from_min_size(pos2(x - wdt, hy - 3.0), vec2(wdt, 6.0));
            let rsp = ui.interact(d.expand(3.0), ui.id().with(("loc", id.as_str())), Sense::click()).on_hover_text(if id == "auto" { "Localização atual" } else { weather::BR_CITIES.iter().find(|c| c.0 == id).map_or("", |c| c.1) });
            p.rect_filled(d, CornerRadius::same(3), if on { fade(Color32::from_rgb(56, 189, 248), alpha) } else { white(if rsp.hovered() { 128 } else { 64 }) });
            if rsp.clicked() { pick = Some(id.clone()); }
            x -= wdt + 6.0;
        }
        if let Some(id) = pick { self.settings.loc_id = id; self.settings.save(); *self.loc.lock().unwrap() = self.settings.loc_id.clone(); }
    }

    fn page_weather(&mut self, ui: &mut egui::Ui, area: Rect, w: Option<&Weather>, alpha: f32) {
        let p = ui.painter().clone();
        let white = |a2: u8| fade(Color32::from_white_alpha(a2), alpha);
        // setas ‹ › nas laterais trocam a cidade
        for (d, ch, x) in [(-1, ic::CHEVRON_LEFT, area.left() + 12.0), (1, ic::CHEVRON_RIGHT, area.right() - 12.0)] {
            let br = Rect::from_center_size(pos2(x, area.center().y), vec2(24.0, 40.0));
            let rsp = ui.interact(br, ui.id().with(("wxarr", d)), Sense::click());
            p.text(br.center(), Align2::CENTER_CENTER, ch, icon_font(18.0), white(if rsp.hovered() { 255 } else { 102 }));
            if rsp.clicked() { self.cycle_loc(d); }
        }
        let w = w.filter(|w| w.loc_id == self.settings.loc_id);
        let body = area.shrink2(vec2(28.0, 0.0));
        let lw = (body.width() - 8.0) * 0.4;
        let (txt, icon) = w.map(|w| weather::wmo(w.code, w.is_day)).unwrap_or(("Carregando o clima…", ic::CLOUD));
        let mins = |iso: &str| iso.get(11..16).and_then(|h| Some(h[..2].parse::<f32>().ok()? * 60.0 + h[3..].parse::<f32>().ok()?));
        let (rise, set, nowm) = w.map(|w| (mins(&w.sunrise).unwrap_or(360.0), mins(&w.sunset).unwrap_or(1080.0), mins(&w.now_iso).unwrap_or(720.0))).unwrap_or((360.0, 1080.0, { use chrono::Timelike; let n = chrono::Local::now(); (n.hour() * 60 + n.minute()) as f32 }));
        let sun_t = (nowm - rise) / (set - rise).max(1.0);
        let day = (0.0..=1.0).contains(&sun_t);
        let cy = body.center().y;
        p.text(pos2(body.left(), cy - 33.0), Align2::LEFT_CENTER, icon, icon_font(30.0), fade(if day { Color32::from_rgb(252, 211, 77) } else { Color32::from_rgb(199, 210, 254) }, alpha));
        p.text(pos2(body.left() + 40.0, cy - 33.0), Align2::LEFT_CENTER, w.map_or("--°".into(), |w| format!("{}°", w.temp)), bold(44.0), white(255));
        p.text(pos2(body.left(), cy + 2.0), Align2::LEFT_CENTER, ellipsize_w(ui, txt, 13.0, lw), FontId::proportional(13.0), white(255));
        p.text(pos2(body.left(), cy + 19.0), Align2::LEFT_CENTER, w.map_or(String::new(), |w| format!("Máx {}° · Mín {}°", w.max, w.min)), FontId::proportional(11.5), white(115));
        let mut x = body.left();
        let chips = [(ic::THERMOMETER, w.map(|w| format!("{}°", w.feels)), "Sensação"), (ic::DROPLETS, w.map(|w| format!("{}%", w.humidity)), "Umidade"), (ic::WIND, w.map(|w| format!("{} km/h", w.wind)), "Vento"), (ic::CLOUD_RAIN, w.map(|w| format!("{}%", w.rain_max)), "Chuva hoje")];
        let mut cyc = cy + 30.0 + 11.0;
        for (i, v, tip) in chips {
            let v = v.unwrap_or_else(|| "—".into());
            let cw = 8.0 + 11.0 + 4.0 + text_w(ui, &v, 10.5) + 8.0;
            if x + cw > body.left() + lw { x = body.left(); cyc += 26.0; }
            let rr = Rect::from_min_size(pos2(x, cyc - 11.0), vec2(cw, 22.0));
            ui.interact(rr, ui.id().with(("chip", tip)), Sense::hover()).on_hover_text(tip);
            p.rect_filled(rr, CornerRadius::same(11), white(18));
            p.text(pos2(rr.left() + 8.0 + 5.5, rr.center().y), Align2::CENTER_CENTER, i, icon_font(11.0), white(115));
            p.text(pos2(rr.left() + 8.0 + 15.0, rr.center().y), Align2::LEFT_CENTER, v, FontId::proportional(10.5), white(191));
            x += cw + 4.0;
        }
        // arco do sol (SVG 320×84 do original, esticado na largura da coluna)
        let right = Rect::from_min_max(pos2(body.left() + lw + 8.0, body.top()), body.max);
        // viewBox 320×84 dentro de uma caixa de 78 px de altura (preserveAspectRatio "meet": centralizado)
        let sc = (right.width() / 320.0).min(78.0 / 84.0);
        let svg = Rect::from_min_size(pos2(right.center().x - 160.0 * sc, cy - 62.0), vec2(320.0 * sc, 84.0 * sc));
        let sx = |x: f32| svg.left() + x / 320.0 * svg.width();
        let sy = |y: f32| svg.top() + y / 84.0 * svg.height();
        let arc_x = |t: f32| sx(18.0 + t.clamp(0.0, 1.0) * 284.0);
        let arc_y = |t: f32| sy(70.0 - (std::f32::consts::PI * t.clamp(0.0, 1.0)).sin() * 54.0);
        let pts: Vec<Pos2> = (0..=40).map(|i| { let t = i as f32 / 40.0; pos2(arc_x(t), arc_y(t)) }).collect();
        // preenchimento âmbar suave embaixo do arco
        let mut mesh = egui::Mesh::default();
        for q in &pts { let k = ((sy(70.0) - q.y) / (sy(70.0) - sy(16.0))).clamp(0.0, 1.0); mesh.colored_vertex(*q, fade(Color32::from_rgba_unmultiplied(251, 191, 36, (51.0 * k.max(0.15)) as u8), alpha)); mesh.colored_vertex(pos2(q.x, sy(70.0)), fade(Color32::from_rgba_unmultiplied(251, 191, 36, 0), alpha)); }
        for i in 0..40u32 { mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2); mesh.add_triangle(2 * i + 1, 2 * i + 3, 2 * i + 2); }
        p.add(egui::Shape::mesh(mesh));
        p.add(egui::Shape::dashed_line(&[pos2(svg.left(), sy(70.0)), pos2(svg.right(), sy(70.0))], Stroke::new(1.0_f32, white(46)), 3.0, 4.0));
        for wnd in pts.windows(2) {
            let t = ((wnd[0].x - pts[0].x) / (pts[40].x - pts[0].x)).clamp(0.0, 1.0);
            let c = if t < 0.5 { lerp_c(Color32::from_rgb(245, 158, 11), Color32::from_rgb(253, 230, 138), t * 2.0) } else { lerp_c(Color32::from_rgb(253, 230, 138), Color32::from_rgb(249, 115, 22), t * 2.0 - 1.0) };
            p.line_segment([wnd[0], wnd[1]], Stroke::new(2.2_f32, fade(c, alpha * if day { 1.0 } else { 0.4 })));
        }
        let sp = pos2(arc_x(sun_t), if day { arc_y(sun_t) } else { sy(70.0) });
        if day { for k in 0..12 { let f = 1.0 - k as f32 / 12.0; let c = if f > 0.45 { lerp_c(Color32::from_rgb(253, 230, 138), Color32::from_rgba_unmultiplied(251, 191, 36, 0), (f - 0.45) / 0.55) } else { lerp_c(Color32::WHITE, Color32::from_rgb(253, 230, 138), f / 0.45) }; p.circle_filled(sp, 13.0 * sc * f, fade(c, alpha * 0.22)); } }
        else { p.circle_filled(sp, 13.0, fade(Color32::from_rgba_unmultiplied(199, 210, 254, 56), alpha)); }
        p.circle_filled(sp, 7.0, fade(if day { Color32::from_rgba_unmultiplied(251, 191, 36, 60) } else { Color32::from_rgba_unmultiplied(165, 180, 252, 60) }, alpha));
        p.circle_filled(sp, 5.0, fade(if day { Color32::from_rgb(255, 247, 214) } else { Color32::from_rgb(224, 231, 255) }, alpha));
        let hm = |iso: Option<&str>| iso.and_then(|i| i.get(11..16)).unwrap_or("--:--").to_string();
        p.text(pos2(sx(18.0), sy(80.0)), Align2::CENTER_CENTER, hm(w.map(|w| w.sunrise.as_str())), FontId::proportional(9.5), white(115));
        p.text(pos2(sx(302.0), sy(80.0)), Align2::CENTER_CENTER, hm(w.map(|w| w.sunset.as_str())), FontId::proportional(9.5), white(115));
        if day { let left = (set - nowm).max(0.0) as i32; p.text(pos2(sx(160.0), sy(80.0)), Align2::CENTER_CENTER, format!("{}h{:02} de sol", left / 60, left % 60), FontId::proportional(9.5), white(140)); }
        // próximas 6 horas
        let gy = svg.bottom() + 6.0;
        let cw = right.width() / 6.0;
        for i in 0..6 {
            let cx = right.left() + cw * (i as f32 + 0.5);
            let h = w.and_then(|w| w.hours.get(i));
            p.text(pos2(cx, gy + 6.0), Align2::CENTER_CENTER, h.map_or("--".into(), |h| format!("{}h", h.hour)), FontId::proportional(10.0), white(115));
            let hi = h.map_or(ic::CLOUD, |h| weather::wmo(h.code, h.is_day).1);
            p.text(pos2(cx, gy + 21.0), Align2::CENTER_CENTER, hi, icon_font(14.0), if h.map_or(false, |h| h.rain >= 50) { fade(Color32::from_rgb(125, 211, 252), alpha) } else { white(179) });
            p.text(pos2(cx, gy + 37.0), Align2::CENTER_CENTER, h.map_or("--".into(), |h| format!("{}°", h.temp)), bold(12.0), white(255));
            if let Some(h) = h { if h.rain >= 20 { p.text(pos2(cx, gy + 51.0), Align2::CENTER_CENTER, format!("{}%", h.rain), FontId::proportional(9.5), fade(Color32::from_rgba_unmultiplied(125, 211, 252, 230), alpha)); } }
        }
    }

    fn page_todo(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        use chrono::Datelike;
        use chrono::Timelike;
        let p = ui.painter().clone();
        let white = |a2: u8| fade(Color32::from_white_alpha(a2), alpha);
        let green = Color32::from_rgb(34, 197, 94);
        let gap = 12.0;
        let tot = body.width() - 2.0 * gap;
        let left = Rect::from_min_size(body.min, vec2(tot * 0.22, body.height()));
        let mid = Rect::from_min_size(pos2(left.right() + gap, body.top()), vec2(tot * 0.43, body.height()));
        let cal = Rect::from_min_max(pos2(mid.right() + gap, body.top()), body.max);
        let now = chrono::Local::now();
        const DT: [&str; 7] = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];
        const MT: [&str; 12] = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];
        // coluna da esquerda
        p.text(pos2(left.left(), left.top() + 10.0), Align2::LEFT_CENTER, "Tarefas", bold(15.0), white(255));
        let (d, n) = (self.tasks.done(), self.tasks.list.len());
        let frac = format!("{d}/{n}");
        p.text(pos2(left.left(), left.top() + 41.0), Align2::LEFT_CENTER, &frac, bold(30.0), white(255));
        let rc = pos2(left.left() + text_w(ui, &frac, 30.0) + 8.0 + 13.0, left.top() + 41.0);
        p.circle_stroke(rc, 11.4, Stroke::new(3.2_f32, fade(Color32::from_rgba_unmultiplied(34, 197, 94, 51), alpha)));
        if n > 0 && d > 0 { arc(&p, rc, 11.4, d as f32 / n as f32, Stroke::new(3.2_f32, fade(green, alpha))); }
        p.text(pos2(left.left(), left.top() + 66.0), Align2::LEFT_CENTER, format!("{}, {} {}", DT[now.weekday().num_days_from_sunday() as usize], now.day(), MT[now.month0() as usize]), FontId::proportional(12.0), white(115));
        let b2 = Rect::from_min_size(pos2(left.left(), left.bottom() - 28.0), vec2(left.width(), 28.0));
        let b1 = Rect::from_min_size(pos2(left.left(), b2.top() - 6.0 - 28.0), vec2(left.width(), 28.0));
        let r1 = ui.interact(b1, ui.id().with("addtask"), Sense::click());
        p.rect_filled(b1, CornerRadius::same(14), fade(Color32::from_rgba_unmultiplied(34, 197, 94, if r1.hovered() { 64 } else { 38 }), alpha));
        let gcol = fade(Color32::from_rgb(74, 222, 128), alpha);
        let tw = text_w(ui, "Nova tarefa", 11.5);
        p.text(pos2(b1.center().x - tw / 2.0 - 4.0, b1.center().y), Align2::RIGHT_CENTER, ic::PLUS, icon_font(12.0), gcol);
        p.text(pos2(b1.center().x - tw / 2.0 + 2.0, b1.center().y), Align2::LEFT_CENTER, "Nova tarefa", FontId::proportional(11.5), gcol);
        if r1.clicked() { self.adding = Some(String::new()); }
        let r2 = ui.interact(b2, ui.id().with("edittasks"), Sense::click());
        let lbl = if self.editing { "Pronto" } else { "Editar tarefas" };
        p.rect_filled(b2, CornerRadius::same(14), if self.editing { white(255) } else { white(if r2.hovered() { 36 } else { 20 }) });
        let ec = if self.editing { fade(Color32::BLACK, alpha) } else { white(217) };
        let tw2 = text_w(ui, lbl, 11.5);
        p.text(pos2(b2.center().x - tw2 / 2.0 - 4.0, b2.center().y), Align2::RIGHT_CENTER, ic::PENCIL, icon_font(11.0), ec);
        p.text(pos2(b2.center().x - tw2 / 2.0 + 2.0, b2.center().y), Align2::LEFT_CENTER, lbl, FontId::proportional(11.5), ec);
        if r2.clicked() { self.editing = !self.editing; }
        // lista do meio
        p.text(pos2(mid.center().x, mid.top() + 8.0), Align2::CENTER_CENTER, format!("{} {} {} {:02}:{:02}", DT[now.weekday().num_days_from_sunday() as usize], now.day(), MT[now.month0() as usize], now.hour(), now.minute()), FontId::proportional(11.5), white(179));
        let list = Rect::from_min_max(pos2(mid.left(), mid.top() + 22.0), pos2(mid.right() - 4.0, mid.bottom()));
        let pc = p.with_clip_rect(list);
        let row_h = 28.0; let rgap = 4.0;
        let mut y = list.top();
        if let Some(mut text) = self.adding.take() {
            let rr = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), row_h));
            pc.rect_filled(rr, CornerRadius::same(8), white(20));
            pc.rect_stroke(rr, CornerRadius::same(8), Stroke::new(1.0_f32, fade(Color32::from_rgba_unmultiplied(34, 197, 94, 102), alpha)), egui::StrokeKind::Inside);
            pc.circle_stroke(pos2(rr.left() + 12.0 + 7.0, rr.center().y), 6.25, Stroke::new(1.5_f32, fade(Color32::from_rgba_unmultiplied(34, 197, 94, 179), alpha)));
            let te = egui::TextEdit::singleline(&mut text).hint_text("Nova tarefa — Enter para adicionar").frame(false).font(FontId::proportional(12.5)).desired_width(rr.width() - 44.0);
            let resp = ui.put(Rect::from_min_max(pos2(rr.left() + 34.0, rr.top() + 5.0), pos2(rr.right() - 8.0, rr.bottom() - 5.0)), te);
            if !resp.has_focus() && !resp.lost_focus() { resp.request_focus(); }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if enter || (resp.lost_focus() && !esc) { if !text.trim().is_empty() { self.tasks.add(text.trim()); } self.adding = None; }
            else if esc { self.adding = None; }
            else { self.adding = Some(text); }
            y += row_h + rgap;
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut rem_items: Vec<Rem> = self.rems.iter().filter(|r| !r.fired && r.at - now_ms < 36 * 3_600_000).cloned().collect();
        rem_items.sort_by_key(|r| r.at);
        let mut rem_del = None;
        for rm in &rem_items {
            let rr = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), row_h));
            pc.rect_filled(rr, CornerRadius::same(8), white(13));
            pc.text(pos2(rr.left() + 12.0 + 6.5, rr.center().y), Align2::CENTER_CENTER, ic::BELL, icon_font(13.0), fade(Color32::from_rgb(252, 211, 77), alpha));
            let dd = chrono::Local.timestamp_millis_opt(rm.at).single();
            let tm = dd.map_or(String::new(), |d| if d.date_naive() == now.date_naive() { d.format("%H:%M").to_string() } else { format!("amanhã {}", d.format("%H:%M")) });
            let right = if self.editing { rr.right() - 30.0 } else { rr.right() - 12.0 };
            pc.text(pos2(right, rr.center().y), Align2::RIGHT_CENTER, &tm, FontId::proportional(11.0), white(128));
            pc.text(pos2(rr.left() + 34.0, rr.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &rm.text, 12.5, right - text_w(ui, &tm, 11.0) - 8.0 - rr.left() - 34.0), FontId::proportional(12.5), white(230));
            if self.editing {
                let del = Rect::from_center_size(pos2(rr.right() - 16.0, rr.center().y), vec2(22.0, 22.0));
                let rd = ui.interact(del.intersect(list), ui.id().with(("remdel", rm.id)), Sense::click());
                pc.text(del.center(), Align2::CENTER_CENTER, ic::TRASH_2, icon_font(12.0), if rd.hovered() { fade(Color32::from_rgb(248, 113, 113), alpha) } else { white(102) });
                if rd.clicked() { rem_del = Some(rm.id); }
            }
            y += row_h + rgap;
        }
        if let Some(id) = rem_del { self.rems.retain(|r| r.id != id); reminders::save(&self.rems); }
        // eventos de hoje da agenda (quando conectada)
        if !self.settings.ics_url.is_empty() {
            let evs: Vec<calendar::Event> = self.cal.lock().unwrap().events.iter().filter(|e| chrono::Local.timestamp_millis_opt(e.start).single().map_or(false, |d| d.date_naive() == now.date_naive())).cloned().collect();
            if evs.is_empty() { pc.text(pos2(list.left() + 4.0, y + 8.0), Align2::LEFT_CENTER, "Nenhum evento hoje na agenda", FontId::proportional(11.0), white(90)); y += 20.0; }
            for ev in evs {
                let rr = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), row_h));
                y += row_h + rgap;
                let past = ev.end < now_ms;
                let a2 = if past { 0.45 } else { 1.0 };
                let resp = ui.interact(rr.intersect(list), ui.id().with(("ev", ev.id.as_str())), Sense::click());
                let resp = if ev.link.is_some() { resp.on_hover_text("Abrir a reunião") } else { resp };
                pc.rect_filled(rr, CornerRadius::same(8), fade(Color32::from_white_alpha(if resp.hovered() && ev.link.is_some() { 20 } else { 13 }), alpha * a2));
                let sq = Rect::from_center_size(pos2(rr.left() + 12.0 + 6.5, rr.center().y), vec2(13.0, 13.0));
                pc.rect_filled(sq.expand(2.0), CornerRadius::same(4), fade(Color32::from_rgba_unmultiplied(168, 85, 247, 60), alpha * a2));
                pc.rect_filled(sq, CornerRadius::same(3), fade(Color32::from_rgb(168, 85, 247), alpha * a2));
                let tm = if ev.all_day { "dia todo".to_string() } else { chrono::Local.timestamp_millis_opt(ev.start).single().map_or(String::new(), |d| d.format("%H:%M").to_string()) };
                let mut right = rr.right() - 12.0;
                pc.text(pos2(right, rr.center().y), Align2::RIGHT_CENTER, &tm, FontId::proportional(11.0), fade(Color32::from_white_alpha(128), alpha * a2));
                right -= text_w(ui, &tm, 11.0) + 8.0;
                if ev.link.is_some() { pc.text(pos2(right, rr.center().y), Align2::RIGHT_CENTER, ic::VIDEO, icon_font(11.0), fade(Color32::from_rgba_unmultiplied(216, 180, 254, 204), alpha * a2)); right -= 18.0; }
                pc.text(pos2(rr.left() + 34.0, rr.center().y), Align2::LEFT_CENTER, ellipsize_w(ui, &ev.title, 12.5, right - rr.left() - 34.0), FontId::proportional(12.5), fade(Color32::from_white_alpha(230), alpha * a2));
                if resp.clicked() { if let Some(l) = &ev.link { let _ = quick::open_shell(l); } }
            }
        }
        let mut toggle_id = None; let mut remove_id = None;
        for t in self.tasks.list.iter() {
            let rr = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), row_h));
            y += row_h + rgap;
            if rr.top() > list.bottom() { continue; }
            let resp = ui.interact(rr.intersect(list), ui.id().with(("task", t.id)), Sense::click());
            pc.rect_filled(rr, CornerRadius::same(8), white(if resp.hovered() && !self.editing { 20 } else { 13 }));
            let c = pos2(rr.left() + 12.0 + 7.0, rr.center().y);
            if t.done { pc.circle_filled(c, 7.0, fade(green, alpha)); pc.text(c, Align2::CENTER_CENTER, ic::CHECK, icon_font(9.0), fade(Color32::BLACK, alpha)); }
            else { pc.circle_stroke(c, 6.25, Stroke::new(1.5_f32, fade(Color32::from_rgb(59, 130, 246), alpha))); }
            let tc = if t.done { white(102) } else { white(230) };
            let txt = ellipsize_w(ui, &t.text, 12.5, rr.right() - (if self.editing { 34.0 } else { 12.0 }) - rr.left() - 34.0);
            let g = pc.layout_no_wrap(txt, FontId::proportional(12.5), tc);
            let tw = g.size().x;
            pc.galley(pos2(rr.left() + 34.0, rr.center().y - g.size().y / 2.0), g, Color32::WHITE);
            if t.done { pc.line_segment([pos2(rr.left() + 34.0, rr.center().y), pos2(rr.left() + 34.0 + tw, rr.center().y)], Stroke::new(1.0_f32, white(102))); }
            if self.editing {
                let del = Rect::from_center_size(pos2(rr.right() - 16.0, rr.center().y), vec2(22.0, 22.0));
                let rd = ui.interact(del.intersect(list), ui.id().with(("del", t.id)), Sense::click());
                pc.text(del.center(), Align2::CENTER_CENTER, ic::TRASH_2, icon_font(12.0), if rd.hovered() { fade(Color32::from_rgb(248, 113, 113), alpha) } else { white(102) });
                if rd.clicked() { remove_id = Some(t.id); }
            } else if resp.clicked() { toggle_id = Some(t.id); }
        }
        if let Some(id) = toggle_id { self.tasks.toggle(id); }
        if let Some(id) = remove_id { self.tasks.remove(id); }
        // calendário
        const MF: [&str; 12] = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];
        p.text(pos2(cal.left(), cal.top() + 10.0), Align2::LEFT_CENTER, format!("{} {}", MF[now.month0() as usize], now.year()), bold(12.5), white(255));
        let mut month = self.cal_month;
        if seg(ui, &p, pos2(cal.right(), cal.top() + 10.0), &[("Semana", false), ("Mês", true)], &mut month, alpha, "calv") { self.cal_month = month; }
        let cal_days: Vec<chrono::NaiveDate> = self.cal.lock().unwrap().events.iter().filter_map(|e| chrono::Local.timestamp_millis_opt(e.start).single().map(|x| x.date_naive())).collect();
        let has_day = |d: chrono::NaiveDate| -> bool { cal_days.contains(&d) || self.rems.iter().any(|r| !r.fired && chrono::Local.timestamp_millis_opt(r.at).single().map_or(false, |x| x.date_naive() == d)) };
        const WK: [&str; 7] = ["D", "S", "T", "Q", "Q", "S", "S"];
        let today = now.date_naive();
        if !self.cal_month {
            let start = today - chrono::Duration::days(now.weekday().num_days_from_sunday() as i64);
            let cw = cal.width() / 7.0;
            for i in 0..7 {
                let day = start + chrono::Duration::days(i);
                let cx = cal.left() + cw * (i as f32 + 0.5);
                p.text(pos2(cx, cal.top() + 36.0), Align2::CENTER_CENTER, WK[i as usize], bold(9.5), white(90));
                let is_t = day == today;
                let dc = pos2(cx, cal.top() + 57.0);
                if is_t {
                    p.circle_filled(dc, 16.0, fade(Color32::from_rgba_unmultiplied(16, 185, 129, 40), alpha));
                    p.circle_filled(dc, 12.0, fade(Color32::from_rgb(16, 185, 129), alpha));
                }
                p.text(dc, Align2::CENTER_CENTER, day.day().to_string(), if is_t { bold(11.5) } else { FontId::proportional(11.5) }, if is_t { fade(Color32::BLACK, alpha) } else { white(191) });
                if has_day(day) { p.circle_filled(pos2(cx, cal.top() + 75.0), 2.0, if is_t { fade(Color32::from_rgb(52, 211, 153), alpha) } else { white(102) }); }
            }
            // linha do dia: agora (verde) e os lembretes de hoje (roxo)
            let tl = Rect::from_min_max(pos2(cal.left(), cal.bottom() - 4.0 - 16.0 - 8.0), pos2(cal.right(), cal.bottom() - 4.0 - 16.0));
            p.rect_filled(tl, CornerRadius::same(4), white(15));
            for ev in self.cal.lock().unwrap().events.iter().filter(|e| !e.all_day) {
                if let Some(dt) = chrono::Local.timestamp_millis_opt(ev.start).single() { if dt.date_naive() == today { let f = (dt.hour() * 60 + dt.minute()) as f32 / 1440.0; p.rect_filled(Rect::from_min_size(pos2(tl.left() + tl.width() * f, tl.top()), vec2(tl.width() * 0.04, tl.height())), CornerRadius::same(4), fade(Color32::from_rgba_unmultiplied(168, 85, 247, 179), alpha)); } }
            }
            for rm in self.rems.iter().filter(|r| !r.fired) {
                if let Some(dt) = chrono::Local.timestamp_millis_opt(rm.at).single() {
                    if dt.date_naive() == today { let f = (dt.hour() * 60 + dt.minute()) as f32 / 1440.0; p.rect_filled(Rect::from_min_size(pos2(tl.left() + tl.width() * f, tl.top()), vec2(tl.width() * 0.04, tl.height())), CornerRadius::same(4), fade(Color32::from_rgba_unmultiplied(168, 85, 247, 179), alpha)); }
                }
            }
            let nf = (now.hour() * 60 + now.minute()) as f32 / 1440.0;
            let nx = tl.left() + tl.width() * nf;
            p.rect_filled(Rect::from_min_size(pos2(nx - 1.0, tl.top() - 4.0), vec2(2.0, 16.0)), CornerRadius::same(1), fade(Color32::from_rgb(52, 211, 153), alpha));
            for (i, l) in ["00", "06", "12", "18", "24"].iter().enumerate() {
                let x = tl.left() + tl.width() * i as f32 / 4.0;
                let al = if i == 0 { Align2::LEFT_CENTER } else if i == 4 { Align2::RIGHT_CENTER } else { Align2::CENTER_CENTER };
                p.text(pos2(x, tl.bottom() + 11.0), al, *l, FontId::monospace(9.0), white(90));
            }
        } else {
            let first = today.with_day(1).unwrap_or(today);
            let off = first.weekday().num_days_from_sunday() as usize;
            let dim = { let nm = if first.month() == 12 { chrono::NaiveDate::from_ymd_opt(first.year() + 1, 1, 1) } else { chrono::NaiveDate::from_ymd_opt(first.year(), first.month() + 1, 1) }; nm.map_or(30, |x| (x - first).num_days() as usize) };
            let cw = cal.width() / 7.0;
            let top = cal.top() + 24.0;
            for (i, wd) in WK.iter().enumerate() { p.text(pos2(cal.left() + cw * (i as f32 + 0.5), top + 6.0), Align2::CENTER_CENTER, *wd, bold(8.5), white(90)); }
            for dnum in 0..dim {
                let k = off + dnum;
                let (col, row) = (k % 7, k / 7);
                let c = pos2(cal.left() + cw * (col as f32 + 0.5), top + 12.0 + 9.0 + row as f32 * 19.0);
                let day = first + chrono::Duration::days(dnum as i64);
                let is_t = day == today;
                if is_t { p.circle_filled(c, 8.5, fade(Color32::from_rgb(16, 185, 129), alpha)); }
                p.text(c, Align2::CENTER_CENTER, (dnum + 1).to_string(), if is_t { bold(9.5) } else { FontId::proportional(9.5) }, if is_t { fade(Color32::BLACK, alpha) } else { white(179) });
                if has_day(day) && !is_t { p.circle_filled(pos2(c.x, c.y + 8.0), 1.5, white(102)); }
            }
        }
    }

    fn page_home(&mut self, ui: &mut egui::Ui, body: Rect, m: &MediaState, alpha: f32) {
        // grade 1 : 1,55 : 1 com 10 px entre os cartões (igual ao Kon atual)
        let gap = 10.0;
        let total = body.width() - 2.0 * gap;
        let w1 = total / 3.55; let w2 = total * 1.55 / 3.55;
        let c1 = Rect::from_min_size(body.min, vec2(w1, body.height()));
        let c2 = Rect::from_min_size(pos2(c1.right() + gap, body.top()), vec2(w2, body.height()));
        let c3 = Rect::from_min_max(pos2(c2.right() + gap, body.top()), body.max);
        let p = ui.painter().clone();
        let white = |a2: u8| fade(Color32::from_white_alpha(a2), alpha);
        // Claude
        let r1 = ui.interact(c1, ui.id().with("homeclaude"), Sense::click());
        radial_card(&p, c1, Color32::from_rgb(41, 16, 2), alpha, r1.hovered());
        spaced(ui, &p, pos2(c1.left() + 12.0, c1.top() + 19.0), "CLAUDE", 10.0, 1.2, fade(Color32::from_rgba_unmultiplied(253, 230, 138, 179), alpha));
        let l = self.claude.lock().unwrap().live.clone();
        p.text(pos2(c1.left() + 12.0, c1.bottom() - 33.0), Align2::LEFT_BOTTOM, mmss(claude_elapsed(&l)), FontId::monospace(26.0), white(255));
        p.text(pos2(c1.left() + 12.0, c1.bottom() - 12.0), Align2::LEFT_BOTTOM, self.claude_label(&l), FontId::proportional(12.0), white(128));
        if r1.clicked() { self.set_page(Page::Claude); }
        // música: capa inteira, escurecido de baixo para cima
        self.cover(ui, c2, 16, alpha, 40.0);
        gradient_stops(&p, c2, 16.0, &[(0.0, Color32::from_black_alpha(0)), (0.5, Color32::from_black_alpha(51)), (1.0, Color32::from_black_alpha(217))], alpha);
        p.rect_stroke(c2, CornerRadius::same(16), Stroke::new(1.0_f32, white(25)), egui::StrokeKind::Inside);
        let bc = pos2(c2.right() - 12.0 - 18.0, c2.bottom() - 12.0 - 18.0);
        let title = if m.active { m.title.clone() } else { "Nada tocando".into() };
        p.text(pos2(c2.left() + 12.0, c2.bottom() - 12.0 - 17.0), Align2::LEFT_BOTTOM, ellipsize_w(ui, &title, 17.0, bc.x - 18.0 - 8.0 - c2.left() - 12.0), bold(17.0), white(255));
        p.text(pos2(c2.left() + 12.0, c2.bottom() - 12.0), Align2::LEFT_BOTTOM, ellipsize_w(ui, if m.active { &m.artist } else { "" }, 12.0, bc.x - 26.0 - c2.left() - 12.0), FontId::proportional(12.0), white(153));
        if play_button(ui, &p, bc, 18.0, m.playing, true, 15.0, alpha).clicked() { self.media.send(MediaCmd::Toggle); }
        // foco
        radial_card(&p, c3, Color32::from_rgb(35, 5, 51), alpha, false);
        spaced(ui, &p, pos2(c3.left() + 12.0, c3.top() + 19.0), "FOCO", 10.0, 1.2, fade(Color32::from_rgba_unmultiplied(233, 213, 255, 179), alpha));
        p.text(pos2(c3.left() + 12.0, c3.bottom() - 33.0), Align2::LEFT_BOTTOM, mmss(self.focus.left), FontId::monospace(26.0), white(255));
        let st = if self.focus.running_until.is_some() { "Rodando" } else if (self.focus.left - self.focus.total as f32).abs() < 0.5 { "Pronto" } else { "Pausado" };
        p.text(pos2(c3.left() + 12.0, c3.bottom() - 12.0), Align2::LEFT_BOTTOM, st, FontId::proportional(12.0), white(128));
        if play_button(ui, &p, pos2(c3.right() - 12.0 - 14.0, c3.bottom() - 12.0 - 14.0), 14.0, self.focus.running_until.is_some(), false, 12.0, alpha).clicked() { self.focus_toggle(); }
    }

    fn page_music(&mut self, ui: &mut egui::Ui, body: Rect, m: &MediaState, alpha: f32) {
        let p = ui.painter().clone();
        let white = |a2: u8| fade(Color32::from_white_alpha(a2), alpha);
        let lyr_on = self.settings.lyrics_on;
        // brilhos: a cor da capa (ou rosa) no canto de cima, índigo embaixo à direita
        let clip = p.with_clip_rect(body.expand2(vec2(16.0, 42.0)).with_max_y(body.bottom() + 46.0));
        let glow = |c: Pos2, rx: f32, ry: f32, col: Color32| { for k in 0..18 { let f = 1.0 - k as f32 / 18.0; let e = Rect::from_center_size(c, vec2(rx * 2.0 * f, ry * 2.0 * f)); clip.rect_filled(e, CornerRadius::same(255), col.gamma_multiply(1.0 / 18.0 * 1.6)); } };
        let pink = self.art_color.map_or(Color32::from_rgba_unmultiplied(236, 72, 153, 64), |c| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 90));
        glow(pos2(body.left() + 4.0 - 40.0 + 112.0, body.top() - 40.0 + 112.0), 150.0, 150.0, fade(pink, alpha));
        glow(pos2(body.right() - 128.0, body.bottom() + 64.0 - 96.0 + 24.0), 170.0, 130.0, fade(Color32::from_rgba_unmultiplied(79, 70, 229, 64), alpha));
        // capa (104 com letra, 132 sem), um pouco menor quando pausado
        let side = if lyr_on { 104.0 } else { 132.0 } * if m.playing { 1.0 } else { 0.9 };
        let ac = pos2(body.left() + 4.0 + if lyr_on { 52.0 } else { 66.0 }, body.center().y);
        let art = Rect::from_center_size(ac, vec2(side, side));
        p.add(egui::epaint::Shadow { offset: [0, 12], blur: 30, spread: 0, color: fade(Color32::from_rgba_unmultiplied(214, 58, 134, 89), alpha) }.as_shape(art, CornerRadius::same(16)));
        self.cover(ui, art, 16, alpha, side / 3.0);
        let x0 = body.left() + 4.0 + if lyr_on { 104.0 } else { 132.0 } + 20.0;
        let title = if m.active { m.title.clone() } else { "Nada tocando".into() };
        let pos = m.live_position();
        // barra de progresso (clique para pular)
        let progress = |ui: &mut egui::Ui, x: f32, w: f32, y: f32, me: &Self| {
            let track = Rect::from_min_size(pos2(x, y), vec2(w, 4.0));
            let hit = ui.interact(track.expand2(vec2(0.0, 5.0)), ui.id().with(("seek", x as i32)), Sense::click());
            p.rect_filled(track, CornerRadius::same(2), white(51));
            let f = if m.duration > 0.0 { (pos / m.duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
            p.rect_filled(Rect::from_min_size(track.min, vec2(w * f, 4.0)), CornerRadius::same(2), white(255));
            p.text(pos2(x, y + 4.0 + 4.0 + 7.0), Align2::LEFT_CENTER, mmss(pos as f32), FontId::monospace(10.5), white(115));
            p.text(pos2(x + w, y + 15.0), Align2::RIGHT_CENTER, format!("-{}", mmss((m.duration - pos).max(0.0) as f32)), FontId::monospace(10.5), white(115));
            if hit.clicked() && m.duration > 0.0 { if let Some(pp) = hit.interact_pointer_pos() { me.media.send(MediaCmd::Seek(((pp.x - x) / w).clamp(0.0, 1.0) as f64 * m.duration)); } }
        };
        // botão da letra
        let lyr_btn = |ui: &mut egui::Ui, c: Pos2| -> bool {
            let r = Rect::from_center_size(c, vec2(28.0, 28.0));
            let rsp = ui.interact(r, ui.id().with("lyrbtn"), Sense::click()).on_hover_text(if lyr_on { "Esconder a letra" } else { "Mostrar a letra" });
            if lyr_on { p.circle_filled(c, 14.0, white(38)); }
            p.text(c, Align2::CENTER_CENTER, ic::MESSAGE_SQUARE_QUOTE, icon_font(14.0), white(if lyr_on || rsp.hovered() { 230 } else { 115 }));
            rsp.clicked()
        };
        let skip = |ui: &mut egui::Ui, c: Pos2, size: f32, fwd: bool| -> bool {
            let r = Rect::from_center_size(c, vec2(size + 8.0, size + 8.0));
            let rsp = ui.interact(r, ui.id().with(("skip", fwd)), Sense::click());
            let k = if rsp.is_pointer_button_down_on() { 0.85 } else { 1.0 };
            let u = size * k / 24.0; let s = if fwd { 1.0 } else { -1.0 };
            // lucide SkipForward cheio: triângulo + barra
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-7.0 * u * s, -7.0 * u), c + vec2(5.0 * u * s, 0.0), c + vec2(-7.0 * u * s, 7.0 * u)], white(255), Stroke::new(1.6 * u, white(255))));
            p.rect_filled(Rect::from_center_size(c + vec2(7.0 * u * s, 0.0), vec2(2.2 * u, 14.0 * u)), CornerRadius::same(1), white(255));
            rsp.clicked()
        };
        let mut toggle_lyrics = false;
        if !lyr_on {
            // só o player: título grande, progresso e controles completos
            let w = body.right() - 4.0 - x0;
            let cy = body.center().y;
            p.text(pos2(x0, cy - 52.0), Align2::LEFT_CENTER, ellipsize_w(ui, &title, 19.0, w), bold(19.0), white(255));
            p.text(pos2(x0, cy - 32.0), Align2::LEFT_CENTER, ellipsize_w(ui, if m.active { &m.artist } else { "" }, 12.5, w), FontId::proportional(12.5), white(140));
            progress(ui, x0, w, cy - 8.0, self);
            let ry = cy + 34.0;
            if lyr_btn(ui, pos2(x0 + 14.0 - 4.0, ry)) { toggle_lyrics = true; }
            let mid = x0 + w / 2.0;
            let sh = Rect::from_center_size(pos2(mid - 18.0 - 24.0 - 18.0 - 24.0 - 7.5, ry), vec2(24.0, 24.0));
            let shr = ui.interact(sh, ui.id().with("shuf"), Sense::click());
            p.text(sh.center(), Align2::CENTER_CENTER, ic::SHUFFLE, icon_font(15.0), white(if m.shuffle { 255 } else if shr.hovered() { 179 } else { 102 }));
            if shr.clicked() { self.media.send(MediaCmd::Shuffle(!m.shuffle)); }
            if skip(ui, pos2(mid - 18.0 - 24.0 - 9.0, ry), 18.0, false) { self.media.send(MediaCmd::Prev); }
            if play_button(ui, &p, pos2(mid, ry), 18.0, m.playing, true, 16.0, alpha).clicked() { self.media.send(MediaCmd::Toggle); }
            if skip(ui, pos2(mid + 18.0 + 24.0 + 9.0, ry), 18.0, true) { self.media.send(MediaCmd::Next); }
            let rp = Rect::from_center_size(pos2(mid + 18.0 + 24.0 + 18.0 + 24.0 + 7.5, ry), vec2(24.0, 24.0));
            let rpr = ui.interact(rp, ui.id().with("rep"), Sense::click());
            p.text(rp.center(), Align2::CENTER_CENTER, ic::REPEAT, icon_font(15.0), white(if m.repeat { 255 } else if rpr.hovered() { 179 } else { 102 }));
            if rpr.clicked() { self.media.send(MediaCmd::Repeat(!m.repeat)); }
        } else {
            // com letra: controles compactos à esquerda e a letra rolando à direita (linha atual em destaque)
            let cw = 176.0;
            let cy = body.center().y;
            p.text(pos2(x0, cy - 42.0), Align2::LEFT_CENTER, ellipsize_w(ui, &title, 14.0, cw), bold(14.0), white(255));
            p.text(pos2(x0, cy - 25.0), Align2::LEFT_CENTER, ellipsize_w(ui, if m.active { &m.artist } else { "" }, 11.5, cw), FontId::proportional(11.5), white(140));
            progress(ui, x0, cw, cy - 6.0, self);
            let ry = cy + 30.0;
            if lyr_btn(ui, pos2(x0 + 14.0 - 4.0, ry)) { toggle_lyrics = true; }
            let mid = x0 + (cw - 0.0) / 2.0 + 14.0;
            if skip(ui, pos2(mid - 16.0 - 16.0 - 8.0, ry), 16.0, false) { self.media.send(MediaCmd::Prev); }
            if play_button(ui, &p, pos2(mid, ry), 16.0, m.playing, true, 14.0, alpha).clicked() { self.media.send(MediaCmd::Toggle); }
            if skip(ui, pos2(mid + 16.0 + 16.0 + 8.0, ry), 16.0, true) { self.media.send(MediaCmd::Next); }
            // letra
            let lx = x0 + cw + 16.0;
            let area = Rect::from_min_max(pos2(lx, cy - 66.0 + 12.0), pos2(body.right(), cy + 66.0));
            p.line_segment([pos2(lx, area.top()), pos2(lx, area.bottom())], Stroke::new(1.0_f32, white(25)));
            let lyr = self.lyr.lock().unwrap().cur.clone();
            let lines: Vec<String> = if self.settings.privacy { vec!["Letra oculta (modo privacidade)".into()] }
                else if !lyr.lines.is_empty() { lyr.lines.iter().map(|l| l.1.clone()).collect() }
                else { vec![if lyr.loading { "Procurando a letra…".into() } else if m.active { title.clone() } else { "Nada tocando".into() }, if m.active && !lyr.loading { "Sem letra sincronizada".into() } else { String::new() }] };
            let li = if !lyr.lines.is_empty() && lyr.synced && !self.settings.privacy { lyrics::at(&lyr.lines, pos) } else { 0 };
            let target = 36.0 - li as f32 * 30.0;
            self.lyr_y += (target - self.lyr_y) * 0.18;
            if (target - self.lyr_y).abs() > 0.5 { ui.ctx().request_repaint(); } else { self.lyr_y = target; }
            let pc = p.with_clip_rect(area);
            let tx = lx + 16.0;
            for (i, l) in lines.iter().enumerate() {
                let y = area.top() + self.lyr_y + i as f32 * 30.0 + 15.0;
                if y < area.top() - 30.0 || y > area.bottom() + 30.0 { continue; }
                let base = if i == li { 1.0 } else if (i as i32 - li as i32).abs() == 1 { 0.38 } else { 0.14 };
                // máscara: some nas bordas de cima e de baixo (24%)
                let fy = ((y - area.top()) / area.height()).clamp(0.0, 1.0);
                let mask = (fy / 0.24).min((1.0 - fy) / 0.24).clamp(0.0, 1.0);
                pc.text(pos2(tx, y), Align2::LEFT_CENTER, ellipsize_w(ui, l, 15.0, area.right() - tx), bold(15.0), white((255.0 * base * mask) as u8));
                let lr = Rect::from_min_max(pos2(tx, y - 15.0), pos2(area.right(), y + 15.0)).intersect(area);
                if !lyr.lines.is_empty() && lyr.synced && ui.interact(lr, ui.id().with(("ly", i)), Sense::click()).clicked() { self.media.send(MediaCmd::Seek(lyr.lines[i].0)); }
            }
        }
        if toggle_lyrics { self.settings.lyrics_on = !lyr_on; self.settings.save(); }
        if m.playing { ui.ctx().request_repaint_after(Duration::from_millis(250)); }
    }

    fn page_focus(&mut self, ui: &mut egui::Ui, body: Rect, alpha: f32) {
        let p = ui.painter().clone();
        let white = |a2: u8| fade(Color32::from_white_alpha(a2), alpha);
        let inner = Rect::from_min_max(pos2(body.left() + 8.0, body.top() + 4.0), pos2(body.right() - 8.0, body.bottom()));
        let running = self.focus.running_until.is_some();
        p.text(pos2(inner.left(), inner.top() + 18.0), Align2::LEFT_CENTER, mmss(self.focus.left), FontId::monospace(36.0), white(255));
        let done_at = chrono::Local::now() + chrono::Duration::seconds(self.focus.left.round() as i64);
        p.text(pos2(inner.left(), inner.top() + 52.0), Align2::LEFT_CENTER, format!("Termina às {}", done_at.format("%H:%M")), FontId::proportional(12.0), white(115));
        // sessões dos últimos 7 dias (barrinhas + total)
        use chrono::TimeZone;
        let today0 = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|d| chrono::Local.from_local_datetime(&d).earliest()).map_or(0, |d| d.timestamp_millis());
        let d0 = today0 - 6 * 86_400_000;
        let week: Vec<&(i64, u32)> = self.settings.focus_log.iter().filter(|f| f.0 >= d0).collect();
        let mins: u32 = week.iter().map(|f| f.1).sum();
        let per: Vec<u32> = (0..7).map(|i| { let a = d0 + i * 86_400_000; week.iter().filter(|f| f.0 >= a && f.0 < a + 86_400_000).map(|f| f.1).sum() }).collect();
        let mx = per.iter().copied().max().unwrap_or(0).max(25) as f32;
        let by = inner.top() + 66.0 + 9.0;
        for (i, m) in per.iter().enumerate() {
            let h = (18.0 * (*m as f32 / mx)).max(18.0 * 0.12);
            let r = Rect::from_min_max(pos2(inner.left() + i as f32 * 8.0, by - h / 2.0 + 9.0 - 9.0 + (18.0 - h) / 2.0), pos2(inner.left() + i as f32 * 8.0 + 5.0, by + 9.0 - 9.0 + 9.0));
            let col = if i == 6 { Color32::from_rgb(192, 132, 252) } else { Color32::from_white_alpha(64) };
            p.rect_filled(r, CornerRadius::same(2), fade(col, alpha * if *m > 0 { 1.0 } else { 0.4 }));
        }
        let n = week.len();
        p.text(pos2(inner.left() + 7.0 * 8.0 + 5.0, by + 3.0), Align2::LEFT_CENTER, format!("{n} {} na semana · {}h{:02}", if n == 1 { "sessão" } else { "sessões" }, mins / 60, mins % 60), FontId::proportional(11.0), white(128));
        // botões
        let mut rx = inner.right();
        let by2 = inner.top() + 16.0;
        let pw = 72f32.max(text_w(ui, "Começar", 12.5) + 32.0);
        let pr = Rect::from_min_size(pos2(rx - pw, by2 - 16.0), vec2(pw, 32.0));
        let prr = ui.interact(pr, ui.id().with("fstart"), Sense::click());
        p.rect_filled(pr, CornerRadius::same(16), white(if prr.hovered() { 230 } else { 255 }));
        p.text(pr.center(), Align2::CENTER_CENTER, if running { "Pausar" } else { "Começar" }, FontId::proportional(12.5), fade(Color32::BLACK, alpha));
        if prr.clicked() { self.focus_toggle(); }
        rx = pr.left() - 8.0;
        for (label, id) in [("Parar", "fstop"), ("+5 min", "fplus")] {
            let w = text_w(ui, label, 12.5) + 28.0;
            let r = Rect::from_min_size(pos2(rx - w, by2 - 16.0), vec2(w, 32.0));
            let rsp = ui.interact(r, ui.id().with(id), Sense::click());
            p.rect_filled(r, CornerRadius::same(16), white(if rsp.hovered() { 38 } else { 25 }));
            p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.5), white(255));
            if rsp.clicked() {
                if id == "fstop" { self.focus.running_until = None; self.focus_dnd(false); self.focus_set(1500, 1500.0); }
                else { let t = (self.focus.total + 300).min(3600); let l = (self.focus.left + 300.0).min(3600.0); self.focus_set(t, l); }
            }
            rx = r.left() - 8.0;
        }
        // régua de 0 a 60 min (clique escolhe a duração)
        let rr = Rect::from_min_max(pos2(inner.left() + 4.0, inner.bottom() - 44.0), pos2(inner.right() - 4.0, inner.bottom()));
        let pct = (self.focus.left / 60.0).min(60.0) / 60.0;
        let fill = Rect::from_min_size(rr.min, vec2(rr.width() * pct, 22.0));
        rr_mesh(&p, fill, 2.0, |pt| fade(Color32::from_rgba_unmultiplied(168, 85, 247, (51.0 * ((pt.x - fill.left()) / fill.width().max(1.0)).clamp(0.0, 1.0)) as u8), alpha));
        for i in 0..=60 {
            let x = rr.left() + rr.width() * i as f32 / 60.0;
            let (h, a2) = if i % 10 == 0 { (18.0, 153) } else { (9.0, 51) };
            p.line_segment([pos2(x, rr.top()), pos2(x, rr.top() + h)], Stroke::new(1.0_f32, white(a2)));
        }
        for n in [0, 10, 20, 30, 40, 50, 60] {
            p.text(pos2(rr.left() + rr.width() * n as f32 / 60.0, rr.top() + 25.0 + 6.0), Align2::CENTER_CENTER, n.to_string(), FontId::monospace(10.0), white(102));
        }
        let hx = rr.left() + rr.width() * pct;
        let hr = Rect::from_center_size(pos2(hx, rr.top() - 5.0 + 15.0), vec2(3.0, 30.0));
        p.add(egui::epaint::Shadow { offset: [0, 0], blur: 10, spread: 0, color: fade(Color32::from_rgba_unmultiplied(168, 85, 247, 230), alpha) }.as_shape(hr, CornerRadius::same(2)));
        rr_mesh(&p, hr, 1.5, |pt| fade(lerp_c(Color32::WHITE, Color32::from_rgb(192, 132, 252), ((pt.y - hr.top()) / hr.height()).clamp(0.0, 1.0)), alpha));
        let rsp = ui.interact(rr, ui.id().with("ruler"), Sense::click());
        if rsp.clicked() { if let Some(pp) = rsp.interact_pointer_pos() { let m = (((pp.x - rr.left()) / rr.width()) * 60.0).round().clamp(1.0, 60.0) as u32; self.focus_set(m * 60, (m * 60) as f32); } }
        if running { ui.ctx().request_repaint_after(Duration::from_millis(250)); }
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
/// Modo privacidade: troca letras e números por pontos, mantendo espaços (dá para ver o tamanho, não o conteúdo).
fn mask(s: &str) -> String { s.chars().map(|c| if c.is_whitespace() { ' ' } else { '•' }).collect() }
fn ellipsize_chars(s: &str, n: usize) -> String {
    let n = n.max(2);
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
fn claude_elapsed(l: &claude::Live) -> f32 {
    match (l.started, l.done_at) { (Some(s), Some(d)) => (d - s).as_secs_f32(), (Some(s), None) => s.elapsed().as_secs_f32(), _ => 0.0 }
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
/// Cores de destaque (linha aurora), iguais ao ACCENTS do Kon atual.
pub const ACCENTS: [(&str, &str, [u32; 4]); 6] = [
    ("aurora", "Aurora", [0x06b6d4, 0x2563eb, 0x4f46e5, 0x9333ea]), ("laranja", "Laranja", [0xf59e0b, 0xf97316, 0xea580c, 0xdc2626]),
    ("verde", "Verde", [0x22c55e, 0x10b981, 0x14b8a6, 0x06b6d4]), ("rosa", "Rosa", [0xf472b6, 0xec4899, 0xd946ef, 0xa855f7]),
    ("azul", "Azul", [0x38bdf8, 0x3b82f6, 0x2563eb, 0x1d4ed8]), ("branco", "Branco", [0xe5e5e5, 0xffffff, 0xd4d4d4, 0xa3a3a3]),
];
fn hexc(h: u32) -> Color32 { Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8) }

pub struct IslandLook { pub docked: bool, pub tone: Option<Color32>, pub accent: [u32; 4], pub glow: f32, pub processing: bool, pub t: f32 }

/// A ilha: fundo preto, sombra, borda fina, orelhas côncavas de 16 px, brilho do estado e a linha aurora na base.
fn draw_island(p: &egui::Painter, r: Rect, view: View, look: &IslandLook) {
    let (top, bot) = if look.docked { (0.0, if matches!(view, View::Panel | View::Sheet) { 24.0 } else { (r.height() / 2.0).min(24.0) }) } else { let f = (r.height() / 2.0).min(26.0); (f, f) };
    let cr = CornerRadius { nw: top as u8, ne: top as u8, sw: bot as u8, se: bot as u8 };
    // sombra suave (aproxima o box-shadow 0 22px 55px do Kon atual). Encostada no topo, a sombra parte só da
    // metade de baixo, com cantos todos arredondados: canto reto no egui vira riscos diagonais e uma faixa escura
    // dos lados da ilha no topo da tela.
    let sr = if look.docked { Rect::from_min_max(pos2(r.left() + bot * 0.4, r.top() + r.height() * 0.45), pos2(r.right() - bot * 0.4, r.bottom())) } else { r };
    let srad = (bot.min(sr.height() / 2.0)) as u8;
    for (oy, blur, alpha) in [(3i8, 10u8, 40u8), (9, 24, 30), (16, 44, 22), (24, 72, 14), (30, 104, 8)] {
        p.add(egui::epaint::Shadow { offset: [0, oy], blur, spread: 0, color: Color32::from_black_alpha(alpha) }.as_shape(sr, CornerRadius::same(srad)));
    }
    p.rect_filled(r, cr, BG);
    if look.docked {
        let ear = 16.0;
        for side in [-1.0f32, 1.0] {
            let corner = if side < 0.0 { r.left_top() } else { r.right_top() };
            let c = pos2(corner.x + side * ear, corner.y + ear);
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(corner, BG);
            let n = 14;
            for i in 0..=n { let t = i as f32 / n as f32 * std::f32::consts::FRAC_PI_2; mesh.colored_vertex(pos2(c.x - side * ear * t.sin(), c.y - ear * t.cos()), BG); }
            for i in 1..=n as u32 { mesh.add_triangle(0, i, i + 1); }
            p.add(egui::Shape::mesh(mesh));
        }
    }
    let k = look.glow.clamp(0.0, 1.5);
    if let Some(tc) = look.tone {
        let op = (0.4 + 0.6 * k).min(1.0);
        let focus = pos2(r.left() + r.width() * 0.3, r.top() + r.height() * 1.15);
        let (rx, ry) = (r.width() * 1.2, r.height() * 0.9);
        rr_radial(p, r, cr, |pt| { let d = (((pt.x - focus.x) / rx).powi(2) + ((pt.y - focus.y) / ry).powi(2)).sqrt(); let a = (1.0 - d / 0.62).clamp(0.0, 1.0); tc.gamma_multiply(a * op) });
    }
    let stops: Vec<Color32> = look.accent.iter().map(|h| hexc(*h)).collect();
    let pulse = if look.processing { 0.8 + 0.2 * (look.t * std::f32::consts::TAU / 1.2).sin() } else { 0.55 };
    let base_op = pulse * k;
    let clip = p.with_clip_rect(r);
    let gx = |x: f32, from: f32, to: f32| -> Color32 { let t = ((x - from) / (to - from)).clamp(0.0, 1.0) * 3.0; let i = (t.floor() as usize).min(2); lerp_c(stops[i], stops[i + 1], t - i as f32) };
    let (bl, br) = (r.left() + r.width() * 0.1, r.right() - r.width() * 0.1);
    let bh = if look.processing { 18.0 } else { 8.0 };
    let bop = if look.processing { 0.6 } else { 0.3 };
    for layer in 0..5 {
        let spread = layer as f32 * 3.0;
        let mut mesh = egui::Mesh::default();
        let n = 24;
        let a = base_op * bop * 0.07;
        for i in 0..=n {
            let x = bl - spread + (br - bl + 2.0 * spread) * i as f32 / n as f32;
            let c = gx(x, bl, br).gamma_multiply(a);
            mesh.colored_vertex(pos2(x, r.bottom() - bh / 2.0 - spread), c.gamma_multiply(0.0));
            mesh.colored_vertex(pos2(x, r.bottom() + bh / 2.0), c);
        }
        for i in 0..n as u32 { mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2); mesh.add_triangle(2 * i + 1, 2 * i + 3, 2 * i + 2); }
        clip.add(egui::Shape::mesh(mesh));
    }
    let (ll, lr) = if look.processing { (r.left() + r.width() * 0.06, r.right() - r.width() * 0.06) } else { (r.left() + r.width() * 0.22, r.right() - r.width() * 0.22) };
    let mut mesh = egui::Mesh::default();
    let n = 24;
    for i in 0..=n {
        let f = i as f32 / n as f32;
        let x = ll + (lr - ll) * f;
        let edge = (f.min(1.0 - f) * 4.0).min(1.0);
        let t3 = f * 2.0;
        let c = if t3 < 1.0 { lerp_c(stops[0], stops[1], t3) } else { lerp_c(stops[1], stops[2], t3 - 1.0) };
        let c = c.gamma_multiply(edge * base_op);
        mesh.colored_vertex(pos2(x, r.bottom() - 1.5), c);
        mesh.colored_vertex(pos2(x, r.bottom()), c);
    }
    for i in 0..n as u32 { mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2); mesh.add_triangle(2 * i + 1, 2 * i + 3, 2 * i + 2); }
    clip.add(egui::Shape::mesh(mesh));
    if look.docked {
        p.with_clip_rect(Rect::from_min_max(pos2(r.left() - 2.0, r.top() + 0.5), pos2(r.right() + 2.0, r.bottom() + 2.0)))
            .rect_stroke(r.with_min_y(r.top() - 30.0), CornerRadius { nw: 0, ne: 0, sw: bot as u8, se: bot as u8 }, Stroke::new(1.0_f32, Color32::from_white_alpha(20)), egui::StrokeKind::Inside);
    } else {
        p.rect_stroke(r, cr, Stroke::new(1.0_f32, Color32::from_white_alpha(31)), egui::StrokeKind::Inside);
    }
}

/// Preenche o retângulo arredondado com uma cor que varia por ponto (anéis concêntricos: fica sempre dentro da forma).
fn rr_radial(p: &egui::Painter, r: Rect, cr: CornerRadius, color: impl Fn(Pos2) -> Color32) {
    let mut ring = vec![];
    let corners = [(r.right() - cr.se as f32, r.bottom() - cr.se as f32, cr.se as f32, 0.0f32), (r.left() + cr.sw as f32, r.bottom() - cr.sw as f32, cr.sw as f32, 90.0), (r.left() + cr.nw as f32, r.top() + cr.nw as f32, cr.nw as f32, 180.0), (r.right() - cr.ne as f32, r.top() + cr.ne as f32, cr.ne as f32, 270.0)];
    for (cx, cy, rad, st) in corners { for i in 0..=6 { let a = (st + i as f32 * 15.0).to_radians(); ring.push(pos2(cx + rad * a.cos(), cy + rad * a.sin())); } }
    let mut dense = vec![];
    for i in 0..ring.len() { let (a, b) = (ring[i], ring[(i + 1) % ring.len()]); let n = ((b - a).length() / 24.0).ceil().max(1.0) as usize; for k in 0..n { dense.push(a + (b - a) * (k as f32 / n as f32)); } }
    let c = r.center();
    let rings = 10;
    let mut mesh = egui::Mesh::default();
    let m = dense.len() as u32;
    for k in 0..=rings {
        let f = 1.0 - k as f32 / rings as f32;
        for q in &dense { let pt = c + (*q - c) * f; mesh.colored_vertex(pt, color(pt)); }
    }
    for k in 0..rings as u32 {
        for i in 0..m {
            let (a, b) = (k * m + i, k * m + (i + 1) % m);
            let (cc, d) = ((k + 1) * m + i, (k + 1) * m + (i + 1) % m);
            mesh.add_triangle(a, b, cc); mesh.add_triangle(b, d, cc);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

/// Área da ilha a partir do corpo de uma página (os brilhos podem invadir o topo e a barra, como no original).
fn island_of(body: Rect) -> Rect { Rect::from_min_max(pos2(body.left() - 16.0, body.top() - 42.0), pos2(body.right() + 16.0, body.bottom() + 46.0)) }
/// Mancha de luz desfocada (o "blur-3xl" do Tailwind): elipses concêntricas somando a cor no centro.
fn soft_glow(p: &egui::Painter, c: Pos2, rx: f32, ry: f32, col: Color32) {
    let n = 22;
    for k in 0..n {
        let f = 1.0 - k as f32 / n as f32;
        let e = Rect::from_center_size(c, vec2(rx * 2.0 * (0.25 + 0.75 * f) * 1.25, ry * 2.0 * (0.25 + 0.75 * f) * 1.25));
        p.rect_filled(e, CornerRadius::same(255), col.gamma_multiply(1.0 / n as f32 * 1.8));
    }
}
/// Vigia (a cada 0,7 s, numa thread) tela cheia, janela maximizada e o volume do Windows.
fn start_fg_watch(ctx: egui::Context) -> Arc<Mutex<(bool, bool, Option<(u32, bool)>)>> {
    let st = Arc::new(Mutex::new((false, false, None)));
    let s2 = st.clone();
    std::thread::spawn(move || loop {
        let (full, max) = crate::platform::foreground_state();
        let vol = crate::platform::master_volume();
        let changed = { let mut g = s2.lock().unwrap(); let c = *g != (full, max, vol); *g = (full, max, vol); c };
        if changed { ctx.request_repaint(); }
        std::thread::sleep(Duration::from_millis(if vol.is_some() { 250 } else { 700 }));
    });
    st
}
/// Segmentado com altura e fonte ajustáveis (o Seg do Kon atual tem 20 e 28 px).
fn seg_h<T: PartialEq + Clone>(ui: &mut egui::Ui, p: &egui::Painter, right: Pos2, opts: &[(&str, T)], val: &mut T, a: f32, id: &str, h: f32, size: f32) -> bool {
    let ws: Vec<f32> = opts.iter().map(|o| ui.painter().layout_no_wrap(o.0.to_string(), FontId::proportional(size), Color32::WHITE).size().x + 20.0).collect();
    let total: f32 = ws.iter().sum::<f32>() + 4.0;
    let r = Rect::from_min_size(pos2(right.x - total, right.y - h / 2.0), vec2(total, h));
    p.rect_filled(r, CornerRadius::same((h / 2.0) as u8), fade(Color32::from_white_alpha(20), a));
    let mut x = r.left() + 2.0; let mut changed = false;
    for (i, (l, v)) in opts.iter().enumerate() {
        let br = Rect::from_min_size(pos2(x, r.top() + 2.0), vec2(ws[i], h - 4.0));
        let resp = ui.interact(br, ui.id().with((id, i)), Sense::click());
        let on = *val == *v;
        if on { p.rect_filled(br, CornerRadius::same(((h - 4.0) / 2.0) as u8), fade(Color32::WHITE, a)); }
        p.text(br.center(), Align2::CENTER_CENTER, *l, FontId::proportional(size), fade(if on { Color32::BLACK } else if resp.hovered() { Color32::from_white_alpha(217) } else { Color32::from_white_alpha(153) }, a));
        if resp.clicked() && !on { *val = v.clone(); changed = true; }
        x += ws[i];
    }
    changed
}
/// Controle deslizante (input range do Kon atual: trilho fino e bolinha branca).
fn slider(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, val: &mut f32, min: f32, max: f32, step: f32, a: f32, id: &str) -> bool {
    let track = Rect::from_min_max(pos2(r.left(), r.center().y - 2.0), pos2(r.right(), r.center().y + 2.0));
    let resp = ui.interact(r, ui.id().with(("slider", id)), Sense::click_and_drag());
    let mut changed = false;
    if resp.dragged() || resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            let f = ((pp.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let v = ((min + f * (max - min)) / step).round() * step;
            if (v - *val).abs() > 1e-4 { *val = v.clamp(min, max); changed = true; }
        }
    }
    let f = ((*val - min) / (max - min)).clamp(0.0, 1.0);
    p.rect_filled(track, CornerRadius::same(2), fade(Color32::from_white_alpha(38), a));
    p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * f, 4.0)), CornerRadius::same(2), fade(Color32::from_white_alpha(140), a));
    let kc = pos2(track.left() + track.width() * f, track.center().y);
    p.circle_filled(kc + vec2(0.0, 1.0), 7.5, fade(Color32::from_black_alpha(60), a));
    p.circle_filled(kc, 7.0, fade(Color32::WHITE, a));
    changed
}
fn label_w(ui: &egui::Ui, t: &str) -> f32 {
    let mut job = egui::text::LayoutJob::single_section(t.to_string(), egui::TextFormat { font_id: bold(10.5), extra_letter_spacing: 1.05, ..Default::default() });
    job.wrap.max_width = f32::INFINITY;
    ui.fonts_mut(|f| f.layout_job(job)).size().x
}
/// Texto em seminegrito com espaçamento entre letras (devolve a largura).
fn spaced(ui: &egui::Ui, p: &egui::Painter, left_center: Pos2, t: &str, size: f32, spacing: f32, col: Color32) -> f32 {
    let mut job = egui::text::LayoutJob::single_section(t.to_string(), egui::TextFormat { font_id: bold(size), color: col, extra_letter_spacing: spacing, ..Default::default() });
    job.wrap.max_width = f32::INFINITY;
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let w = g.size().x;
    p.galley(pos2(left_center.x, left_center.y - g.size().y / 2.0), g, col);
    w
}
/// Cartão com degradê radial a partir do canto de cima à esquerda (from-…-950/60 to-neutral-950) e borda branca 10%.
fn radial_card(p: &egui::Painter, r: Rect, from: Color32, a: f32, hover: bool) {
    let to = Color32::from_rgb(10, 10, 10);
    rr_radial(p, r, CornerRadius::same(16), |pt| { let t = ((((pt.x - r.left()) / r.width()).powi(2) + ((pt.y - r.top()) / r.height()).powi(2)).sqrt() / std::f32::consts::SQRT_2).clamp(0.0, 1.0); fade(lerp_c(from, to, t), a) });
    p.rect_stroke(r, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(if hover { 51 } else { 25 }), a)), egui::StrokeKind::Inside);
}
/// Degradê vertical com paradas (de baixo para cima, como bg-gradient-to-t).
fn gradient_stops(p: &egui::Painter, r: Rect, rad: f32, stops: &[(f32, Color32)], a: f32) {
    rr_mesh(p, r, rad, |pt| {
        let t = 1.0 - ((pt.y - r.top()) / r.height()).clamp(0.0, 1.0);
        let mut c = stops[0].1;
        for w in stops.windows(2) { if t >= w[0].0 && t <= w[1].0 { c = lerp_c(w[0].1, w[1].1, (t - w[0].0) / (w[1].0 - w[0].0)); } }
        fade(c, a)
    });
}
/// Botão redondo de tocar/pausar com o ícone cheio (fill="currentColor" do original).
fn play_button(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, r: f32, playing: bool, primary: bool, icon: f32, a: f32) -> egui::Response {
    let rect = Rect::from_center_size(c, vec2(r * 2.0, r * 2.0));
    let resp = ui.interact(rect, ui.id().with(("pb", (c.x as i32, c.y as i32))), Sense::click());
    let k = if resp.is_pointer_button_down_on() { 0.88 } else { 1.0 };
    if primary { p.circle_filled(c, r * k, fade(Color32::WHITE, a)); }
    else { p.circle_filled(c, r * k, fade(Color32::from_white_alpha(if resp.hovered() { 46 } else { 31 }), a)); }
    let col = fade(if primary { Color32::BLACK } else { Color32::WHITE }, a);
    play_glyph(p, c, icon * k, playing, col);
    resp
}
fn play_glyph(p: &egui::Painter, c: Pos2, size: f32, playing: bool, col: Color32) {
    let u = size / 24.0;
    if playing {
        for x in [6.0f32, 14.0] { p.rect_filled(Rect::from_min_size(pos2(c.x + (x - 12.0) * u, c.y - 8.0 * u), vec2(4.0 * u, 16.0 * u)), CornerRadius::same((1.0 * u) as u8), col); }
    } else {
        let o = vec2(1.0, 0.0);
        p.add(egui::Shape::convex_polygon(vec![c + o + vec2(-6.0 * u, -9.0 * u), c + o + vec2(8.0 * u, 0.0), c + o + vec2(-6.0 * u, 9.0 * u)], col, Stroke::new(1.6 * u, col)));
    }
}
fn draw_bars(p: &egui::Painter, c: Pos2, col: Color32, playing: bool, a: f32) {
    // mesmas sequências do Waveform do Kon atual: 7 barras de 2,5 px, 12 quadros/s, com interpolação
    let seqs: [[f32; 5]; 7] = [[3., 9., 5., 12., 3.], [6., 14., 8., 5., 6.], [10., 5., 15., 7., 10.], [5., 12., 6., 14., 5.], [12., 7., 10., 4., 12.], [4., 10., 13., 6., 4.], [7., 4., 9., 12., 7.]];
    let n = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() / 83) as usize;
    let total = 7.0 * 2.5 + 6.0 * 2.0;
    for (i, sq) in seqs.iter().enumerate() {
        let k = (n + i * 2) % 15;
        let (x0, x1) = (sq[(k / 3) % 5], sq[(k / 3 + 1) % 5]);
        let h = if playing { x0 + (x1 - x0) * (k % 3) as f32 / 3.0 } else { 3.0 };
        let x = c.x - total / 2.0 + i as f32 * 4.5 + 1.25;
        let r = Rect::from_center_size(pos2(x, c.y), vec2(2.5, h));
        p.rect_filled(r.expand(1.2), CornerRadius::same(3), fade(col, a * if playing { 0.25 } else { 0.1 }));
        p.rect_filled(r, CornerRadius::same(2), fade(col, a * if playing { 1.0 } else { 0.5 }));
    }
}
fn toggle(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, on: bool, a: f32) -> egui::Response { toggle_c(ui, p, c, on, a, Color32::from_rgb(52, 199, 89)) }
fn toggle_c(ui: &mut egui::Ui, p: &egui::Painter, c: Pos2, on: bool, a: f32, col: Color32) -> egui::Response {
    let r = Rect::from_center_size(c, vec2(38.0, 22.0));
    let resp = ui.interact(r, ui.id().with(("tgc", (c.x as i32, c.y as i32))), Sense::click());
    p.rect_filled(r, CornerRadius::same(11), fade(if on { col } else { Color32::from_white_alpha(38) }, a));
    let kx = if on { r.right() - 11.0 } else { r.left() + 11.0 };
    p.circle_filled(pos2(kx, c.y + 1.0), 9.5, fade(Color32::from_black_alpha(50), a));
    p.circle_filled(pos2(kx, c.y), 9.0, fade(Color32::WHITE, a));
    resp
}
/// Botão em pílula (PillBtn do Kon atual: 28 px, texto 11,5), alinhado à direita a partir de `rx`.
fn small_pill(ui: &mut egui::Ui, p: &egui::Painter, rx: &mut f32, cy: f32, icon: Option<char>, label: &str, primary: bool, a: f32) -> bool {
    let font = if primary { bold(11.5) } else { FontId::proportional(11.5) };
    let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE).size().x + if icon.is_some() { 24.0 + 17.0 } else { 24.0 };
    let r = Rect::from_min_size(pos2(*rx - w, cy - 14.0), vec2(w, 28.0));
    let resp = ui.interact(r, ui.id().with(("sp", label, (r.min.x as i32, r.min.y as i32))), Sense::click());
    let bg = if primary { Color32::from_white_alpha(if resp.hovered() { 230 } else { 255 }) } else { Color32::from_white_alpha(if resp.hovered() { 36 } else { 20 }) };
    let fg = if primary { Color32::BLACK } else { Color32::from_white_alpha(217) };
    p.rect_filled(r, CornerRadius::same(14), fade(bg, a));
    let mut x = r.left() + 12.0;
    if let Some(i) = icon { p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, i, icon_font(11.0), fade(fg, a)); x += 17.0; }
    p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, label, font, fade(fg, a));
    *rx = r.left() - 6.0;
    resp.clicked()
}

fn fade(c: Color32, a: f32) -> Color32 { c.gamma_multiply(a.clamp(0.0, 1.0)) }
fn bold(size: f32) -> FontId { FontId::new(size, FontFamily::Name("bold".into())) }
fn mmss(s: f32) -> String { let s = s.max(0.0).round() as u32; format!("{}:{:02}", s / 60, s % 60) }
fn ellipsize(s: &str, n: usize) -> String { if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" } }

fn card(p: &egui::Painter, r: Rect, c: Color32, a: f32) {
    p.rect_filled(r, CornerRadius::same(16), fade(c, a));
    p.rect_stroke(r, CornerRadius::same(16), Stroke::new(1.0_f32, fade(Color32::from_white_alpha(18), a)), egui::StrokeKind::Inside);
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
    let mut tick: u32 = 0;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(30));
        let Some((x, y)) = crate::platform::cursor_in_window() else { continue };
        let mut h = hit.lock().unwrap();
        let ppp = if h.ppp > 0.0 { h.ppp } else { 1.0 };
        let inside = h.rect.map_or(false, |r| r.contains(pos2(x as f32 / ppp, y as f32 / ppp)));
        // a cada ~0,5 s, com a ilha fechada: mantém centralizada no monitor certo (troca de tela, de resolução ou de escala)
        tick += 1;
        if tick % 16 == 0 && h.compact && !inside && h.rect.is_some() {
            let follow = h.follow; drop(h);
            if crate::platform::place(follow) { ctx.request_repaint(); }
            continue;
        }
        if inside != h.inside { h.inside = inside; drop(h); ctx.request_repaint(); }
    });
}

/* ───────── ícone perto do relógio ───────── */
mod tray {
    pub enum Action { Open, Ocr, Update, Quit }

    #[cfg(windows)]
    pub struct Tray { _icon: tray_icon::TrayIcon, open: tray_icon::menu::MenuId, ocr: tray_icon::menu::MenuId, upd: tray_icon::menu::MenuId, quit: tray_icon::menu::MenuId }

    #[cfg(windows)]
    impl Tray {
        pub fn new(ctx: eframe::egui::Context) -> Option<Self> {
            use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
            let menu = Menu::new();
            let open = MenuItem::new("Abrir painel", true, None);
            let ocr = MenuItem::new("Smart OCR\tCtrl+Alt+T", true, None);
            let upd = MenuItem::new("Procurar atualização", true, None);
            let quit = MenuItem::new("Sair do Kon", true, None);
            menu.append_items(&[&open, &ocr, &PredefinedMenuItem::separator(), &upd, &PredefinedMenuItem::separator(), &quit]).ok()?;
            let icon = tray_icon::Icon::from_rgba(icon_rgba(), 32, 32).ok()?;
            let t = tray_icon::TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Kon").with_icon(icon).build().ok()?;
            let c2 = ctx.clone();
            tray_icon::menu::MenuEvent::set_event_handler(Some(move |_e| c2.request_repaint()));
            tray_icon::TrayIconEvent::set_event_handler(Some(move |_e| ctx.request_repaint()));
            Some(Tray { _icon: t, open: open.id().clone(), ocr: ocr.id().clone(), upd: upd.id().clone(), quit: quit.id().clone() })
        }
        pub fn poll(&self) -> Vec<Action> {
            let mut out = vec![];
            while let Ok(e) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                if e.id == self.open { out.push(Action::Open) } else if e.id == self.ocr { out.push(Action::Ocr) } else if e.id == self.upd { out.push(Action::Update) } else if e.id == self.quit { out.push(Action::Quit) }
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

/// "Compactar" da zona de soltar: cria um .zip ao lado do primeiro item (Compress-Archive do Windows).
fn zip_files(paths: Vec<std::path::PathBuf>, out: Arc<Mutex<Option<Result<std::path::PathBuf, String>>>>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let list: Vec<_> = paths.into_iter().filter(|p| p.exists()).collect();
        let r: Result<std::path::PathBuf, String> = (|| {
            let first = list.first().ok_or("Nada para compactar")?;
            if !cfg!(windows) { return Err("Compactar só funciona no Windows".into()); }
            let dir = first.parent().map(|d| d.to_path_buf()).unwrap_or_default();
            let base = if list.len() == 1 { first.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("Arquivos".into()) } else { "Arquivos".into() };
            let mut dest = dir.join(format!("{base}.zip"));
            let mut i = 2; while dest.exists() { dest = dir.join(format!("{base} ({i}).zip")); i += 1; }
            let q = |p: &std::path::Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
            let cmd = format!("Compress-Archive -LiteralPath {} -DestinationPath {} -CompressionLevel Optimal", list.iter().map(|p| q(p)).collect::<Vec<_>>().join(","), q(&dest));
            let mut c = std::process::Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-Command", &cmd]);
            #[cfg(windows)] { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
            let o = c.output().map_err(|e| e.to_string())?;
            if o.status.success() && dest.exists() { Ok(dest) } else { Err(String::from_utf8_lossy(&o.stderr).lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).unwrap_or("Falhou ao compactar".into())) }
        })();
        *out.lock().unwrap() = Some(r);
        ctx.request_repaint();
    });
}

/// Começa a arrastar um arquivo guardado para fora do Kon (OLE do Windows; o laço modal termina ao soltar).
#[cfg(windows)]
fn drag_out(frame: &eframe::Frame, path: &str) {
    let mut png = vec![];
    let mut im = image::RgbaImage::new(48, 48);
    for (x, y, px) in im.enumerate_pixels_mut() {
        let (dx, dy) = ((x as f32 - 23.5).abs() - 14.0, (y as f32 - 23.5).abs() - 17.0);
        let d = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - 4.0;
        let a = (0.5 - d).clamp(0.0, 1.0);
        *px = image::Rgba([245, 245, 245, (a * 235.0) as u8]);
    }
    let _ = image::DynamicImage::ImageRgba8(im).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
    let _ = drag::start_drag(frame, drag::DragItem::Files(vec![std::path::PathBuf::from(path)]), drag::Image::Raw(png), |_, _| {}, drag::Options::default());
}
#[cfg(not(windows))]
fn drag_out(_frame: &eframe::Frame, _path: &str) {}
