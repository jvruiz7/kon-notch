//! Kon nativo: o notch do Windows escrito em Rust, desenhado pela placa de vídeo, sem motor web.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod media;
mod platform;
mod settings;
mod ui;

use eframe::egui;

const WIN_W: f32 = 820.0;
const WIN_H: f32 = 560.0;

fn main() -> eframe::Result<()> {
    let area = platform::primary_work_area();
    // posição em pixels lógicos: o eframe converte com a escala do monitor principal
    let scale = guess_scale(area.w);
    let x = area.x as f32 / scale + (area.w as f32 / scale - WIN_W) / 2.0;

    let make = |renderer: eframe::Renderer| eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kon")
            .with_inner_size([WIN_W, WIN_H])
            .with_position([x, area.y as f32 / scale])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_mouse_passthrough(true),
        vsync: true,
        renderer,
        ..Default::default()
    };
    // OpenGL é o mais leve; em PCs sem OpenGL (máquinas virtuais, área de trabalho remota) usa DirectX 12 / WARP
    let first = eframe::run_native("Kon", make(eframe::Renderer::Glow), Box::new(|cc| Ok(Box::new(ui::KonApp::new(cc)))));
    if first.is_ok() { return first; }
    let r = eframe::run_native("Kon", make(eframe::Renderer::Wgpu), Box::new(|cc| Ok(Box::new(ui::KonApp::new(cc)))));
    let r = r.map_err(|e| { eprintln!("glow falhou: {first:?}"); e });
    if let Err(e) = &r {
        // sem console no Windows: o erro vai para %APPDATA%\KonNativo\erro.log
        let d = platform::data_dir();
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("erro.log"), format!("{e:?}\n"));
    }
    r
}

/// Estimativa inicial da escala (a real chega no primeiro quadro e a janela é recentralizada).
fn guess_scale(_w: i32) -> f32 { 1.0 }
