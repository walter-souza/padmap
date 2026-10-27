// Sem janela de console no Windows em builds de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture;
mod diagram;
mod slots;
mod stick_editor;
mod theme;
mod worker;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("padmap")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1000.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "padmap",
        options,
        Box::new(|cc| Ok(Box::new(app::PadmapApp::new(cc)))),
    )
}
