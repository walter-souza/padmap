//! Captura de tecla: o usuário aperta a tecla (ou combinação) no teclado e ela
//! vira o mapeamento.

use crate::theme::{Palette, ACCENT, ACCENT_STRONG, BAD};
use eframe::egui::{
    self, vec2, Align, Align2, CornerRadius, Event, FontId, Layout, Margin, Pos2, Rect, RichText,
    Sense, Shape, Stroke, Ui,
};
use padmap_core::{Key, KeyCombo};

const MODIFIERS: [Key; 8] = [
    Key::LeftCtrl,
    Key::RightCtrl,
    Key::LeftShift,
    Key::RightShift,
    Key::LeftAlt,
    Key::RightAlt,
    Key::LeftMeta,
    Key::RightMeta,
];

#[derive(Default)]
pub struct Capture {
    /// Teclas seguras agora.
    held: Vec<Key>,
    /// Todas as teclas apertadas desde o início da captura, modificadores
    /// primeiro e o resto na ordem em que foram apertadas.
    seen: Vec<Key>,
}

impl Capture {
    /// Processa os eventos do quadro. Junta todas as teclas apertadas e retorna
    /// a combinação quando todas são soltas: `A`, `Ctrl+S`, `A+B` ou só `Shift`.
    pub fn feed(&mut self, events: &[Event]) -> Option<KeyCombo> {
        for event in events {
            let Event::Key {
                key,
                physical_key,
                pressed,
                repeat: false,
                ..
            } = event
            else {
                continue;
            };
            let Some(k) = from_egui(physical_key.unwrap_or(*key)) else {
                continue;
            };
            if *pressed {
                if !self.held.contains(&k) {
                    self.held.push(k);
                }
                if !self.seen.contains(&k) {
                    self.seen.push(k);
                    // Modificadores vão na frente, para `S` antes de `Ctrl`
                    // ainda virar `Ctrl+S` quando o padmap apertar.
                    self.seen.sort_by_key(|k| !MODIFIERS.contains(k));
                }
            } else if self.held.contains(&k) {
                self.held.retain(|h| *h != k);
                if self.held.is_empty() {
                    return Some(KeyCombo(std::mem::take(&mut self.seen)));
                }
            }
        }
        None
    }

    /// As teclas que a combinação vai ter se o usuário soltar tudo agora.
    pub fn keys(&self) -> &[Key] {
        &self.seen
    }
}

/// O que o usuário escolheu na captura, além de apertar teclas no teclado.
pub enum ModalAction {
    Cancel,
    Clear,
    /// Clicou num botão do mouse (com os modificadores que estavam seguros).
    Pick(KeyCombo),
}

/// Modal de captura de um botão: cabeçalho com o nome, tecla atual, área das
/// teclas e rodapé com "Deixar livre" e "Cancelar". Esc não fecha, porque
/// também é uma tecla que dá para mapear.
pub fn modal(
    ctx: &egui::Context,
    pal: &Palette,
    title: &str,
    current: Option<&KeyCombo>,
    held: &[Key],
) -> Option<ModalAction> {
    let mut action = None;
    let resp = egui::Modal::new(egui::Id::unique("padmap-capture"))
        .frame(
            egui::Frame::new()
                .fill(pal.bar)
                .stroke(Stroke::new(1.0, pal.chrome_border))
                .corner_radius(14),
        )
        .show(ctx, |ui| {
            ui.set_width(400.0);
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            let head = egui::Frame::new()
                .fill(pal.chrome)
                .corner_radius(CornerRadius {
                    nw: 14,
                    ne: 14,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(Margin::symmetric(18, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Mapear").size(15.0).strong().color(pal.text));
                        ui.label(
                            RichText::new(format!("· {title}"))
                                .size(15.0)
                                .color(pal.chrome_muted),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let x = egui::Button::new(
                                RichText::new("×").size(18.0).color(pal.chrome_muted),
                            )
                            .frame(false);
                            if ui.add(x).on_hover_text("Cancelar").clicked() {
                                action = Some(ModalAction::Cancel);
                            }
                        });
                    });
                });
            let r = head.response.rect;
            ui.painter()
                .hline(r.x_range(), r.bottom(), Stroke::new(1.0, pal.chrome_border));

            egui::Frame::new()
                .inner_margin(Margin {
                    left: 18,
                    right: 18,
                    top: 14,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Atual").size(12.0).color(pal.text_muted));
                        match current {
                            Some(c) => {
                                egui::Frame::new()
                                    .fill(pal.key_bg)
                                    .corner_radius(4)
                                    .inner_margin(Margin::symmetric(7, 2))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(combo_label(c).to_uppercase())
                                                .size(11.0)
                                                .strong()
                                                .color(pal.key_text),
                                        );
                                    });
                            }
                            None => {
                                ui.label(RichText::new("livre").size(12.0).color(pal.text_muted));
                            }
                        }
                    });
                    ui.add_space(12.0);
                    keys_ui(ui, pal, held);
                    ui.add_space(14.0);
                    if let Some(combo) = mouse_buttons_ui(ui, pal, held) {
                        action = Some(ModalAction::Pick(combo));
                    }
                });

            let top = ui.cursor().top();
            ui.painter().hline(
                ui.max_rect().x_range(),
                top,
                Stroke::new(1.0, pal.bar_border),
            );
            egui::Frame::new()
                .inner_margin(Margin::symmetric(18, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if current.is_some() {
                            let clear = egui::Button::new(RichText::new("Deixar livre").color(BAD))
                                .frame(false)
                                .min_size(vec2(0.0, 30.0));
                            if ui
                                .add(clear)
                                .on_hover_text("Tira a tecla deste botão")
                                .clicked()
                            {
                                action = Some(ModalAction::Clear);
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let cancel =
                                egui::Button::new(RichText::new("Cancelar").color(pal.text))
                                    .min_size(vec2(0.0, 30.0));
                            if ui.add(cancel).clicked() {
                                action = Some(ModalAction::Cancel);
                            }
                        });
                    });
                });
        });
    if resp.backdrop_response.clicked() {
        action = Some(ModalAction::Cancel);
    }
    action
}

/// Nome curto e amigável da tecla, como aparece no teclado.
pub fn key_label(k: Key) -> String {
    match k {
        Key::LeftShift => "Shift".into(),
        Key::RightShift => "Shift dir.".into(),
        Key::LeftCtrl => "Ctrl".into(),
        Key::RightCtrl => "Ctrl dir.".into(),
        Key::LeftAlt => "Alt".into(),
        Key::RightAlt => "AltGr".into(),
        Key::LeftMeta => "Win".into(),
        Key::RightMeta => "Win dir.".into(),
        Key::MouseLeft => "Mouse esq.".into(),
        Key::MouseRight => "Mouse dir.".into(),
        Key::MouseMiddle => "Mouse meio".into(),
        _ => k.name().to_uppercase(),
    }
}

/// A combinação com os nomes amigáveis de [`key_label`] (ex.: "Ctrl+Shift+S").
pub fn combo_label(c: &KeyCombo) -> String {
    c.0.iter()
        .map(|&k| key_label(k))
        .collect::<Vec<_>>()
        .join("+")
}

/// Divisória "ou um botão do mouse" e os três botões. Clique de mouse não dá
/// para capturar pelo teclado, então ele é escolhido aqui. Retorna a combinação
/// com as teclas que estavam seguras (ex.: `Ctrl+MouseLeft`).
pub fn mouse_buttons_ui(ui: &mut Ui, pal: &Palette, held: &[Key]) -> Option<KeyCombo> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 14.0), Sense::hover());
    let label = ui.painter().layout_no_wrap(
        "ou um botão do mouse".into(),
        FontId::proportional(11.0),
        pal.text_muted,
    );
    let gap = label.size().x / 2.0 + 8.0;
    let y = rect.center().y;
    let line = Stroke::new(1.0, pal.bar_border);
    ui.painter()
        .hline(rect.left()..=rect.center().x - gap, y, line);
    ui.painter()
        .hline(rect.center().x + gap..=rect.right(), y, line);
    ui.painter()
        .galley(rect.center() - label.size() / 2.0, label, pal.text_muted);
    ui.add_space(8.0);

    let mut picked = None;
    let bw = (w - 2.0 * 6.0) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (key, text) in [
            (Key::MouseLeft, "Esquerdo"),
            (Key::MouseRight, "Direito"),
            (Key::MouseMiddle, "Meio"),
        ] {
            let (r, resp) = ui.allocate_exact_size(vec2(bw, 50.0), Sense::click());
            let hot = resp.hovered();
            let (fill, border, color) = if hot {
                (ACCENT_STRONG, ACCENT_STRONG, egui::Color32::WHITE)
            } else {
                (pal.free_bg, pal.free_border, pal.text)
            };
            ui.painter().rect(
                r,
                8.0,
                fill,
                Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            );
            mouse_icon(
                ui.painter(),
                r.center_top() + vec2(0.0, 17.0),
                key,
                hot,
                pal,
            );
            ui.painter().text(
                r.center_bottom() - vec2(0.0, 8.0),
                Align2::CENTER_BOTTOM,
                text,
                FontId::proportional(12.0),
                color,
            );
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                let mut keys: Vec<Key> = held.iter().copied().filter(|k| *k != key).collect();
                keys.push(key);
                picked = Some(KeyCombo(keys));
            }
        }
    });
    ui.add_space(6.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("Segure Ctrl, Shift ou Alt antes de clicar para combinar")
                .size(11.0)
                .color(pal.text_muted),
        );
    });
    picked
}

/// Mouse pequeno, centrado em `c`, com o botão `key` em destaque.
fn mouse_icon(painter: &egui::Painter, c: Pos2, key: Key, hot: bool, pal: &Palette) {
    let (line, mark) = if hot {
        (egui::Color32::WHITE, egui::Color32::WHITE)
    } else {
        (pal.text_muted, ACCENT)
    };
    let body = Rect::from_center_size(c, vec2(14.0, 20.0));
    let stroke = Stroke::new(1.4, line);
    let split = body.top() + 8.0;
    match key {
        Key::MouseLeft => painter.rect_filled(
            Rect::from_min_max(body.left_top(), Pos2::new(c.x, split)),
            egui::CornerRadius {
                nw: 7,
                ..Default::default()
            },
            mark,
        ),
        Key::MouseRight => painter.rect_filled(
            Rect::from_min_max(Pos2::new(c.x, body.top()), Pos2::new(body.right(), split)),
            egui::CornerRadius {
                ne: 7,
                ..Default::default()
            },
            mark,
        ),
        _ => painter.rect_filled(
            Rect::from_center_size(Pos2::new(c.x, body.top() + 4.5), vec2(3.0, 5.0)),
            1.5,
            mark,
        ),
    };
    painter.rect_stroke(body, 7.0, stroke, egui::StrokeKind::Middle);
    painter.hline(body.x_range(), split, stroke);
    painter.vline(c.x, body.top()..=split, stroke);
}

/// Altura fixa da área de captura, para o modal não pular entre os estados.
const AREA_H: f32 = 112.0;

/// Área central da captura: um teclado esperando a primeira tecla ou as
/// teclas já apertadas, desenhadas, e um espaço para mais uma.
pub fn keys_ui(ui: &mut Ui, pal: &Palette, held: &[Key]) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), AREA_H), Sense::hover());
    let painter = ui.painter();
    let (title, hint) = if held.is_empty() {
        let c = rect.center_top() + vec2(0.0, 30.0);
        painter.circle(c, 27.0, pal.key_bg, Stroke::NONE);
        dashed_circle(painter, c, 27.0, ACCENT);
        keyboard_icon(painter, c);
        (
            "Pressione a tecla no teclado",
            "Segure várias teclas juntas para combinar",
        )
    } else {
        let font = FontId::proportional(14.0);
        let caps: Vec<_> = held
            .iter()
            .map(|&k| painter.layout_no_wrap(key_label(k), font.clone(), egui::Color32::WHITE))
            .collect();
        let plus = painter.layout_no_wrap("+".into(), font.clone(), pal.text_muted);
        let (h, pad, gap, slot_w) = (32.0, 12.0, 8.0, 40.0);
        let widths: Vec<f32> = caps
            .iter()
            .map(|g| (g.size().x + 2.0 * pad).max(36.0))
            .collect();
        let total =
            widths.iter().sum::<f32>() + caps.len() as f32 * (plus.size().x + 2.0 * gap) + slot_w;
        let mut x = rect.center().x - total / 2.0;
        let y = rect.top() + 14.0;
        for (g, w) in caps.into_iter().zip(widths) {
            let r = Rect::from_min_size(Pos2::new(x, y), vec2(w, h));
            // Base mais escura embaixo dá a cara de tecla física.
            painter.rect_filled(r.translate(vec2(0.0, 3.0)), 7.0, ACCENT_STRONG);
            painter.rect_filled(r, 7.0, ACCENT);
            painter.galley(r.center() - g.size() / 2.0, g, egui::Color32::WHITE);
            x += w + gap;
            painter.galley(
                Pos2::new(x, r.center().y - plus.size().y / 2.0),
                plus.clone(),
                pal.text_muted,
            );
            x += plus.size().x + gap;
        }
        let slot = Rect::from_min_size(Pos2::new(x, y), vec2(slot_w, h + 3.0));
        dashed_rect(painter, slot, pal.text_muted);
        (
            "Solte para confirmar",
            "ou aperte mais teclas para a combinação",
        )
    };
    let y = rect.bottom() - 36.0;
    painter.text(
        Pos2::new(rect.center().x, y),
        Align2::CENTER_TOP,
        title,
        FontId::proportional(14.0),
        pal.text,
    );
    painter.text(
        Pos2::new(rect.center().x, y + 20.0),
        Align2::CENTER_TOP,
        hint,
        FontId::proportional(12.0),
        pal.text_muted,
    );
}

/// Contorno tracejado de um retângulo.
fn dashed_rect(painter: &egui::Painter, r: Rect, color: egui::Color32) {
    let [a, b, c, d] = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ];
    for (p, q) in [(a, b), (b, c), (c, d), (d, a)] {
        painter.extend(Shape::dashed_line(
            &[p, q],
            Stroke::new(1.5, color),
            4.0,
            3.0,
        ));
    }
}

/// Contorno tracejado de um círculo.
fn dashed_circle(painter: &egui::Painter, c: Pos2, r: f32, color: egui::Color32) {
    const N: usize = 32;
    let at = |k: usize| {
        let t = k as f32 / N as f32 * std::f32::consts::TAU;
        c + vec2(t.cos(), t.sin()) * r
    };
    for i in (0..N).step_by(2) {
        painter.line_segment([at(i), at(i + 1)], Stroke::new(1.5, color));
    }
}

/// Teclado pequeno: moldura e três fileiras de teclas.
fn keyboard_icon(painter: &egui::Painter, c: Pos2) {
    let body = Rect::from_center_size(c, vec2(26.0, 17.0));
    painter.rect_stroke(
        body,
        3.0,
        Stroke::new(1.6, ACCENT),
        egui::StrokeKind::Middle,
    );
    for (row, (n, y)) in [(4, -4.0), (4, 0.5)].into_iter().enumerate() {
        let off = if row == 1 { 1.5 } else { 0.0 };
        for i in 0..n {
            let x = -7.5 + i as f32 * 5.0 + off;
            painter.rect_filled(
                Rect::from_center_size(c + vec2(x, y), vec2(2.4, 2.4)),
                0.5,
                ACCENT,
            );
        }
    }
    painter.line_segment(
        [c + vec2(-5.0, 5.0), c + vec2(5.0, 5.0)],
        Stroke::new(1.6, ACCENT),
    );
}

pub fn from_egui(key: egui::Key) -> Option<Key> {
    use egui::Key as E;
    Some(match key {
        E::A => Key::A,
        E::B => Key::B,
        E::C => Key::C,
        E::D => Key::D,
        E::E => Key::E,
        E::F => Key::F,
        E::G => Key::G,
        E::H => Key::H,
        E::I => Key::I,
        E::J => Key::J,
        E::K => Key::K,
        E::L => Key::L,
        E::M => Key::M,
        E::N => Key::N,
        E::O => Key::O,
        E::P => Key::P,
        E::Q => Key::Q,
        E::R => Key::R,
        E::S => Key::S,
        E::T => Key::T,
        E::U => Key::U,
        E::V => Key::V,
        E::W => Key::W,
        E::X => Key::X,
        E::Y => Key::Y,
        E::Z => Key::Z,
        E::Num0 => Key::Num0,
        E::Num1 => Key::Num1,
        E::Num2 => Key::Num2,
        E::Num3 => Key::Num3,
        E::Num4 => Key::Num4,
        E::Num5 => Key::Num5,
        E::Num6 => Key::Num6,
        E::Num7 => Key::Num7,
        E::Num8 => Key::Num8,
        E::Num9 => Key::Num9,
        E::F1 => Key::F1,
        E::F2 => Key::F2,
        E::F3 => Key::F3,
        E::F4 => Key::F4,
        E::F5 => Key::F5,
        E::F6 => Key::F6,
        E::F7 => Key::F7,
        E::F8 => Key::F8,
        E::F9 => Key::F9,
        E::F10 => Key::F10,
        E::F11 => Key::F11,
        E::F12 => Key::F12,
        E::Escape => Key::Escape,
        E::Tab => Key::Tab,
        E::CapsLock => Key::CapsLock,
        E::Space => Key::Space,
        E::Enter => Key::Enter,
        E::Backspace => Key::Backspace,
        E::Insert => Key::Insert,
        E::Delete => Key::Delete,
        E::Home => Key::Home,
        E::End => Key::End,
        E::PageUp => Key::PageUp,
        E::PageDown => Key::PageDown,
        E::ArrowUp => Key::Up,
        E::ArrowDown => Key::Down,
        E::ArrowLeft => Key::Left,
        E::ArrowRight => Key::Right,
        E::ShiftLeft => Key::LeftShift,
        E::ShiftRight => Key::RightShift,
        E::ControlLeft => Key::LeftCtrl,
        E::ControlRight => Key::RightCtrl,
        E::AltLeft => Key::LeftAlt,
        E::AltRight => Key::RightAlt,
        E::SuperLeft => Key::LeftMeta,
        E::SuperRight => Key::RightMeta,
        E::Minus => Key::Minus,
        E::Equals | E::Plus => Key::Equal,
        E::OpenBracket | E::OpenCurlyBracket => Key::LeftBracket,
        E::CloseBracket | E::CloseCurlyBracket => Key::RightBracket,
        E::Backslash | E::Pipe => Key::Backslash,
        E::Semicolon | E::Colon => Key::Semicolon,
        E::Quote => Key::Apostrophe,
        E::Backtick => Key::Grave,
        E::Comma => Key::Comma,
        E::Period => Key::Period,
        E::Slash | E::Questionmark => Key::Slash,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: egui::Key, pressed: bool) -> Event {
        Event::Key {
            key: k,
            physical_key: Some(k),
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn plain_key() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::Space, true)]), None);
        assert_eq!(
            c.feed(&[key(egui::Key::Space, false)]),
            Some(Key::Space.into())
        );
    }

    #[test]
    fn modifier_plus_key() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::ControlLeft, true)]), None);
        assert_eq!(c.feed(&[key(egui::Key::S, true)]), None);
        assert_eq!(c.feed(&[key(egui::Key::S, false)]), None);
        assert_eq!(
            c.feed(&[key(egui::Key::ControlLeft, false)]),
            Some(KeyCombo(vec![Key::LeftCtrl, Key::S]))
        );
    }

    #[test]
    fn two_plain_keys() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::A, true)]), None);
        assert_eq!(c.feed(&[key(egui::Key::B, true)]), None);
        assert_eq!(c.keys(), [Key::A, Key::B]);
        assert_eq!(
            c.feed(&[key(egui::Key::A, false), key(egui::Key::B, false)]),
            Some(KeyCombo(vec![Key::A, Key::B]))
        );
    }

    #[test]
    fn modifiers_go_first() {
        let mut c = Capture::default();
        c.feed(&[key(egui::Key::S, true), key(egui::Key::ControlLeft, true)]);
        assert_eq!(
            c.feed(&[key(egui::Key::S, false), key(egui::Key::ControlLeft, false)]),
            Some(KeyCombo(vec![Key::LeftCtrl, Key::S]))
        );
    }

    #[test]
    fn ignores_release_of_key_held_before() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::Enter, false)]), None);
        assert!(c.keys().is_empty());
    }

    #[test]
    fn modifier_alone() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::ShiftLeft, true)]), None);
        assert_eq!(
            c.feed(&[key(egui::Key::ShiftLeft, false)]),
            Some(Key::LeftShift.into())
        );
    }
}
