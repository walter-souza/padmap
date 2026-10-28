// Sem janela de console no Windows em builds de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture;
mod diagram;
mod extras;
mod profiles;
mod slots;
mod stick_editor;
mod theme;
mod touchpad_editor;
mod worker;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("padmap")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1000.0, 680.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/padmap.png"))
                    .expect("ícone embutido é um PNG válido"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "padmap",
        options,
        Box::new(|cc| {
            #[cfg(windows)]
            hide_title_icon(cc);
            Ok(Box::new(app::PadmapApp::new(cc)))
        }),
    )
}

/// Tira o ícone da barra de título, que já aparece no header ao lado do nome.
/// Ele continua na barra de tarefas e no Alt+Tab.
#[cfg(windows)]
fn hide_title_icon(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::Controls::{
        SetWindowThemeAttribute, WTA_NONCLIENT, WTA_OPTIONS, WTNCA_NODRAWICON,
    };
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win) = handle.as_raw() else {
        return;
    };
    let options = WTA_OPTIONS {
        dwFlags: WTNCA_NODRAWICON,
        dwMask: WTNCA_NODRAWICON,
    };
    // SAFETY: `hwnd` é a janela do app, viva enquanto o `CreationContext`
    // existe; `options` vive durante a chamada e o tamanho é o dele.
    unsafe {
        SetWindowThemeAttribute(
            win.hwnd.get() as _,
            WTA_NONCLIENT,
            (&options as *const WTA_OPTIONS).cast(),
            std::mem::size_of::<WTA_OPTIONS>() as u32,
        );
    }
}
