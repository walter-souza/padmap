//! Cores da interface nos modos claro e escuro.
//!
//! O desenho do controle é escuro nos dois modos; o que muda é o fundo, as
//! barras e as etiquetas das teclas livres.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Theme};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const ACCENT: Color32 = rgb(0x3B82F6);
pub const ACCENT_STRONG: Color32 = rgb(0x2563EB);
pub const ACTIVE: Color32 = rgb(0xEAB308);
pub const GOOD: Color32 = rgb(0x22C55E);
pub const BAD: Color32 = rgb(0xDC2626);

pub struct Palette {
    /// Fundo da área central.
    pub bg: Color32,
    /// Barra do topo e rodapé.
    pub bar: Color32,
    pub bar_border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    /// Botões comuns (Novo, Abrir…).
    pub button: Color32,
    pub button_border: Color32,
    /// Etiqueta de botão sem tecla.
    pub free_bg: Color32,
    pub free_border: Color32,
    pub free_text: Color32,
    /// Selo "LIVRE" / "INATIVO".
    pub chip_bg: Color32,
    pub chip_border: Color32,
    pub chip_text: Color32,
    /// Linha que liga uma etiqueta livre ao botão.
    pub free_line: Color32,
    /// Etiqueta de botão mapeado.
    pub mapped_bg: Color32,
    pub mapped_text: Color32,
    /// Fundo das etiquetas amarelas (stick acionado).
    pub active_bg: Color32,
    pub switch_off: Color32,
    pub toggle_bg: Color32,
    pub toggle_on: Color32,
    // Controle.
    pub body: Color32,
    pub body_stroke: Color32,
    pub trigger: Color32,
    pub trigger_stroke: Color32,
    pub socket: Color32,
    pub part: Color32,
    pub part_stroke: Color32,
    pub glyph: Color32,
    pub stick_bg: Color32,
    pub stick_spoke: Color32,
    pub stick_ring: Color32,
}

pub const LIGHT: Palette = Palette {
    bg: rgb(0xF6F8FB),
    bar: rgb(0xFFFFFF),
    bar_border: rgb(0xE5E9F0),
    text: rgb(0x111827),
    text_muted: rgb(0x6B7280),
    button: rgb(0xFFFFFF),
    button_border: rgb(0xDDE2EA),
    free_bg: rgb(0xFFFFFF),
    free_border: rgb(0xDDE2EA),
    free_text: rgb(0x374151),
    chip_bg: rgb(0xEEF1F5),
    chip_border: rgb(0xDDE2EA),
    chip_text: rgb(0x6B7280),
    free_line: rgb(0xCBD3DF),
    mapped_bg: rgb(0x0B1533),
    mapped_text: rgb(0xDCE7FF),
    active_bg: rgb(0xFFFBEB),
    switch_off: rgb(0xD5DBE5),
    toggle_bg: rgb(0xEEF1F5),
    toggle_on: rgb(0xFFFFFF),
    body: rgb(0x1A1E2E),
    body_stroke: rgb(0x3E4766),
    trigger: rgb(0x2A2F44),
    trigger_stroke: rgb(0x4A5270),
    socket: rgb(0x0C0F18),
    part: rgb(0x252B40),
    part_stroke: rgb(0x3A4260),
    glyph: rgb(0x6E7694),
    stick_bg: rgb(0x121828),
    stick_spoke: rgb(0x2E4A8A),
    stick_ring: rgb(0x2B3550),
};

pub const DARK: Palette = Palette {
    bg: rgb(0x0A0E1A),
    bar: rgb(0x0F1424),
    bar_border: rgb(0x1A2238),
    text: rgb(0xF3F4F6),
    text_muted: rgb(0x8B93A7),
    button: rgb(0x121829),
    button_border: rgb(0x232B3F),
    free_bg: rgb(0x111725),
    free_border: rgb(0x273046),
    free_text: rgb(0xC7CEDB),
    chip_bg: rgb(0x1A2133),
    chip_border: rgb(0x2A3349),
    chip_text: rgb(0x9CA3AF),
    free_line: rgb(0x2A3349),
    mapped_bg: rgb(0x0B1533),
    mapped_text: rgb(0xDCE7FF),
    active_bg: rgb(0x2A2306),
    switch_off: rgb(0x26345A),
    toggle_bg: rgb(0x141B2D),
    toggle_on: rgb(0x26345A),
    body: rgb(0x1A1E2E),
    body_stroke: rgb(0x4A5578),
    trigger: rgb(0x2A2F44),
    trigger_stroke: rgb(0x4A5270),
    socket: rgb(0x0C0F18),
    part: rgb(0x252B40),
    part_stroke: rgb(0x3A4260),
    glyph: rgb(0x6E7694),
    stick_bg: rgb(0x121828),
    stick_spoke: rgb(0x2E4A8A),
    stick_ring: rgb(0x2B3550),
};

pub fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
    }
}

fn visuals(theme: Theme) -> egui::Visuals {
    let p = palette(theme);
    let mut v = match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    };
    v.panel_fill = p.bg;
    v.window_fill = p.bar;
    v.window_stroke = Stroke::new(1.0, p.bar_border);
    v.extreme_bg_color = p.button;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = ACCENT;
    v.override_text_color = Some(p.text);
    let radius = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.bar_border);
    v.widgets.inactive.weak_bg_fill = p.button;
    v.widgets.inactive.bg_fill = p.button;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.button_border);
    v.widgets.hovered.weak_bg_fill = p.button;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v
}

/// Registra as cores dos dois modos. A troca é feita com `Context::set_theme`.
pub fn install(ctx: &egui::Context) {
    for theme in [Theme::Light, Theme::Dark] {
        ctx.set_visuals_of(theme, visuals(theme));
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(12.0, 6.0);
        });
    }
}

pub fn from_name(name: &str) -> Option<Theme> {
    match name {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

pub fn name(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}
