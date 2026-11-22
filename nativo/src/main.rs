//! Kon nativo: o notch do Windows escrito em Rust, desenhado pela placa de vídeo, sem motor web.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod ai;
mod calc;
mod calendar;
mod claude;
mod clipboard;
mod hotkeys;
mod installer;
mod icons;
mod inuse;
mod mascot;
mod lyrics;
mod media;
mod tasks;
mod translate;
mod updater;
mod usage;
mod weather;
mod platform;
mod quick;
mod reminders;
mod report;
mod secrets;
mod settings;
mod sounds;
mod stash;
mod system;
mod ui;

use eframe::egui;

const WIN_W: f32 = 820.0;
const WIN_H: f32 = 560.0;

fn main() -> eframe::Result<()> {
    platform::log(&format!("início v{} args={:?} exe={:?}", env!("CARGO_PKG_VERSION"), std::env::args().skip(1).collect::<Vec<_>>(), std::env::current_exe().ok()));
    if std::env::args().any(|a| a == "--uninstall") { installer::uninstall(); return Ok(()); }
    if !installer::single_instance() { platform::log("outra cópia já está aberta: saindo"); return Ok(()); }
    if installer::ensure_installed() { platform::log("instalado: abrindo a cópia instalada e saindo"); return Ok(()); }
    installer::close_old_kon();
    let area = platform::primary_work_area();
    // posição em pixels lógicos: o eframe converte com a escala do monitor principal
    let scale = guess_scale(area.w);
    let x = area.x as f32 / scale + (area.w as f32 / scale - WIN_W) / 2.0;

    let make = |renderer: eframe::Renderer| eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KonNativo")
            .with_inner_size([WIN_W, WIN_H])
            .with_position([x, area.y as f32 / scale])
            .with_decorations(false)
            .with_transparent(true)
            .with_drag_and_drop(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_mouse_passthrough(cfg!(windows)),
        vsync: true,
        renderer,
        ..Default::default()
    };
    // erros fatais também vão para o log (o .exe não tem console)
    std::panic::set_hook(Box::new(|info| {
        let d = platform::data_dir();
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("erro.log"), format!("pane: {info}\n"));
        installer::message(&format!("O Kon encontrou um erro e precisou fechar:\n\n{info}\n\nO detalhe ficou salvo em %APPDATA%\\KonNativo\\erro.log"), false);
    }));
    platform::log(&format!("área de trabalho principal: {}x{} em ({},{})", area.w, area.h, area.x, area.y));
    let r = eframe::run_native("Kon", make(eframe::Renderer::Glow), Box::new(|cc| Ok(Box::new(ui::KonApp::new(cc)))));
    platform::log(&format!("fim: {:?}", r.as_ref().err()));
    if let Err(e) = &r {
        // sem console no Windows: o erro vai para %APPDATA%\KonNativo\erro.log
        let d = platform::data_dir();
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("erro.log"), format!("{e:?}\n"));
        installer::message(&format!("O Kon não conseguiu abrir a janela:\n\n{e}\n\nAtualize o driver de vídeo e tente de novo. O detalhe ficou salvo em %APPDATA%\\KonNativo\\erro.log"), false);
    }
    r
}

/// Estimativa inicial da escala (a real chega no primeiro quadro e a janela é recentralizada).
fn guess_scale(_w: i32) -> f32 { 1.0 }
