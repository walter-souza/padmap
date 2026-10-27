//! Cores da interface nos modos claro e escuro.
//!
//! O desenho do controle é escuro nos dois modos (grafite no claro, quase
//! preto no escuro); o que muda é o fundo, as barras e as etiquetas.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Theme};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const ACCENT: Color32 = rgb(0x3B82F6);
pub const ACCENT_STRONG: Color32 = rgb(0x2563EB);
pub const ACTIVE: Color32 = rgb(0xEAB308);
pub const GOOD: Color32 = rgb(0x22C55E);
pub const BAD: Color32 = rgb(0xDC2626);

/// Cor de cada setor no editor de stick, na ordem dos setores.
pub const SECTOR_COLORS: [Color32; 10] = [
    rgb(0x3B82F6),
    rgb(0x22C55E),
    rgb(0xEF4444),
    rgb(0x84CC16),
    rgb(0xEC4899),
    rgb(0x06B6D4),
    rgb(0xA855F7),
    rgb(0xF97316),
    rgb(0xF59E0B),
    rgb(0x14B8A6),
];

pub struct Palette {
    /// Fundo da área central.
    pub bg: Color32,
    /// Fundo de cartões e janelas.
    pub bar: Color32,
    pub bar_border: Color32,
    /// Barra do topo e rodapé (e cabeçalho dos modais), e o texto apagado
    /// sobre elas.
    pub chrome: Color32,
    pub chrome_border: Color32,
    pub chrome_muted: Color32,
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
    /// Etiqueta de botão mapeado, o selo da tecla dentro dela e a linha que
    /// a liga ao botão.
    pub mapped_bg: Color32,
    pub mapped_border: Color32,
    pub mapped_text: Color32,
    pub key_bg: Color32,
    pub key_text: Color32,
    /// Tinta vermelha para estados de perigo (ex.: enviando teclas de verdade).
    pub bad_bg: Color32,
    pub bad_text: Color32,
    pub mapped_line: Color32,
    /// Fundo das etiquetas amarelas (stick acionado).
    pub active_bg: Color32,
    pub switch_off: Color32,
    pub toggle_bg: Color32,
    pub toggle_on: Color32,
    /// Disco do editor de stick e as linhas tracejadas sobre ele.
    pub dial_bg: Color32,
    pub dial_line: Color32,
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
    chrome: rgb(0xD9DFE8),
    chrome_border: rgb(0xC3CCD9),
    chrome_muted: rgb(0x4B5563),
    text: rgb(0x111827),
    text_muted: rgb(0x6B7280),
    button: rgb(0xFFFFFF),
    button_border: rgb(0xDDE2EA),
    free_bg: rgb(0xFFFFFF),
    free_border: rgb(0xE2E7EF),
    free_text: rgb(0x6B7280),
    chip_bg: rgb(0xF1F4F8),
    chip_border: rgb(0xE2E7EF),
    chip_text: rgb(0x8A93A3),
    free_line: rgb(0xD5DCE6),
    mapped_bg: rgb(0xFFFFFF),
    mapped_border: rgb(0xC9D6EA),
    mapped_text: rgb(0x1F2937),
    key_bg: rgb(0xDBEAFE),
    key_text: rgb(0x1D4ED8),
    bad_bg: rgb(0xFEE2E2),
    bad_text: rgb(0xB91C1C),
    mapped_line: rgb(0x93B4E8),
    active_bg: rgb(0xFFFBEB),
    switch_off: rgb(0xD5DBE5),
    toggle_bg: rgb(0xC9D1DD),
    toggle_on: rgb(0xFFFFFF),
    dial_bg: rgb(0xEEF2F7),
    dial_line: rgb(0xC3CCD9),
    body: rgb(0x2E3547),
    body_stroke: rgb(0x4B556E),
    trigger: rgb(0x363D51),
    trigger_stroke: rgb(0x535C76),
    socket: rgb(0x232939),
    part: rgb(0x3A4256),
    part_stroke: rgb(0x505A73),
    glyph: rgb(0x8A93AB),
    stick_bg: rgb(0x262C3C),
    stick_spoke: rgb(0x5A6A90),
    stick_ring: rgb(0x4B556E),
};

pub const DARK: Palette = Palette {
    bg: rgb(0x0B0F1A),
    bar: rgb(0x0F1424),
    bar_border: rgb(0x1A2238),
    chrome: rgb(0x10151F),
    chrome_border: rgb(0x1C2335),
    chrome_muted: rgb(0x8B93A7),
    text: rgb(0xF3F4F6),
    text_muted: rgb(0x8B93A7),
    button: rgb(0x121829),
    button_border: rgb(0x232B3F),
    free_bg: rgb(0x0F141F),
    free_border: rgb(0x222A3B),
    free_text: rgb(0x8B93A7),
    chip_bg: rgb(0x171D2A),
    chip_border: rgb(0x222A3B),
    chip_text: rgb(0x6B7385),
    free_line: rgb(0x1F2738),
    mapped_bg: rgb(0x151B2A),
    mapped_border: rgb(0x2C3650),
    mapped_text: rgb(0xE5E9F0),
    key_bg: rgb(0x1E3A6B),
    key_text: rgb(0xBFDBFE),
    bad_bg: rgb(0x3B1418),
    bad_text: rgb(0xFCA5A5),
    mapped_line: rgb(0x33507F),
    active_bg: rgb(0x2A2306),
    switch_off: rgb(0x26345A),
    toggle_bg: rgb(0x141B2D),
    toggle_on: rgb(0x26345A),
    dial_bg: rgb(0x121828),
    dial_line: rgb(0x2E3A55),
    body: rgb(0x1A2030),
    body_stroke: rgb(0x323C55),
    trigger: rgb(0x2A2F44),
    trigger_stroke: rgb(0x4A5270),
    socket: rgb(0x0E121C),
    part: rgb(0x262D40),
    part_stroke: rgb(0x3A4460),
    glyph: rgb(0x6E7694),
    stick_bg: rgb(0x141A28),
    stick_spoke: rgb(0x334466),
    stick_ring: rgb(0x2C3650),
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
