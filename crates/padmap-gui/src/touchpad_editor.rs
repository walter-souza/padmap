//! Editor do touchpad: o touchpad dividido numa grade de zonas, cada uma com
//! sua tecla. Clicar numa zona captura a tecla e o botão direito deixa a zona
//! livre. Os dedos aparecem ao vivo, para testar sem ativar o mapeamento.
//!
//! Tudo é editado numa cópia; o perfil só muda em "Salvar Mapeamento".

use crate::app::segmented;
use crate::capture::{combo_label, Capture, ModalAction};
use crate::diagram::Live;
use crate::stick_editor::{
    capture_overlay, card, heading, list_row, modal_footer, modal_header, title_row,
};
use crate::theme::{Palette, ACCENT, ACTIVE};
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Ui,
};
use padmap_core::touchpad::{MAX_COLS, MAX_ROWS};
use padmap_core::{Button, ExtraSource, Profile, TouchActivate, TouchpadBinding};

const LEFT_W: f32 = 440.0;
const RIGHT_W: f32 = 270.0;
const GAP: f32 = 16.0;
const PAD: i8 = 20;
const WIDTH: f32 = LEFT_W + GAP + RIGHT_W + 2.0 * PAD as f32;
/// Altura do touchpad em relação à largura, perto da do DualShock 4.
const ASPECT: f32 = 0.5;
/// Altura da área do touchpad com a dica; cabe a captura de tecla por cima.
const AREA_H: f32 = 346.0;

/// Divisões prontas: `(nome, colunas, linhas)`.
const PRESETS: &[(&str, u8, u8)] = &[
    ("Inteiro", 1, 1),
    ("2 metades", 2, 1),
    ("3 colunas", 3, 1),
    ("4 quadrantes", 2, 2),
    ("6 zonas (3 × 2)", 3, 2),
];

pub enum Outcome {
    /// `None` quando nenhuma zona tem tecla: o touchpad sai do perfil.
    Save(Option<TouchpadBinding>),
    Cancel,
}

pub struct TouchpadEditor {
    draft: TouchpadBinding,
    capture: Option<(usize, Capture)>,
    /// Tecla que o clique do touchpad tem hoje como botão. No modo "ao
    /// clicar" o clique passa a acionar as zonas e essa tecla sai do perfil.
    click_key: Option<String>,
}

impl TouchpadEditor {
    pub fn new(profile: &Profile) -> Self {
        let click_key = profile
            .buttons
            .get(&Button::Touchpad)
            .or_else(|| {
                profile
                    .extras
                    .iter()
                    .find(|e| e.button == ExtraSource::Known(Button::Touchpad))?
                    .key
                    .as_ref()
            })
            .map(|c| combo_label(c).to_uppercase());
        TouchpadEditor {
            draft: profile.touchpad.clone().unwrap_or_default(),
            capture: None,
            click_key,
        }
    }

    /// Esperando uma tecla para alguma zona.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    fn finish(&self) -> Option<TouchpadBinding> {
        (!self.draft.is_empty()).then(|| self.draft.clone())
    }

    /// Desenha o editor. Retorna o resultado quando ele fecha.
    pub fn show(&mut self, ctx: &egui::Context, pal: &Palette, live: &Live) -> Option<Outcome> {
        let was_busy = self.capture.is_some();
        self.feed_capture(ctx);

        let mut outcome = None;
        egui::Modal::new(egui::Id::unique("padmap-touchpad"))
            .frame(
                egui::Frame::new()
                    .fill(pal.bg)
                    .stroke(Stroke::new(1.0, pal.bar_border))
                    .corner_radius(14),
            )
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                if modal_header(ui, pal, "Mapeamento do Touchpad", |_| {}) {
                    outcome = Some(Outcome::Cancel);
                }
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(PAD, 16))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = GAP;
                            ui.vertical(|ui| {
                                ui.set_width(LEFT_W);
                                self.pad_card(ui, pal, live);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(RIGHT_W);
                                self.grid_card(ui, pal);
                                self.activate_card(ui, pal);
                            });
                        });
                    });
                if let Some(save) = modal_footer(ui, pal, self.capture.is_none()) {
                    outcome = Some(if save {
                        Outcome::Save(self.finish())
                    } else {
                        Outcome::Cancel
                    });
                }
            });

        if !was_busy && outcome.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            outcome = Some(Outcome::Cancel);
        }
        outcome
    }

    fn feed_capture(&mut self, ctx: &egui::Context) {
        let Some((i, capture)) = &mut self.capture else {
            return;
        };
        let events = ctx.input(|inp| inp.events.clone());
        if let Some(combo) = capture.feed(&events) {
            let i = *i;
            self.draft.zones[i].key = Some(combo);
            self.capture = None;
        }
    }

    fn pad_card(&mut self, ui: &mut Ui, pal: &Palette, live: &Live) {
        card(ui, pal, |ui| {
            let fingers = live.touch.len();
            let chip = match fingers {
                0 => "sem dedos".to_owned(),
                1 => "1 dedo".to_owned(),
                n => format!("{n} dedos"),
            };
            title_row(ui, pal, "TOUCHPAD", &chip);
            // Área de altura fixa: o touchpad em cima e a dica embaixo. A
            // captura cobre a área toda, sem a janela mudar de tamanho.
            let w = ui.available_width();
            let (area, _) = ui.allocate_exact_size(vec2(w, AREA_H), Sense::hover());
            let rect = Rect::from_min_size(area.min, vec2(w, w * ASPECT));
            let resp = ui.interact(rect, ui.scope_id().with("pad"), Sense::click());
            self.pad(ui, pal, live, rect, &resp);
            ui.painter().text(
                Pos2::new(area.left(), rect.bottom() + 12.0),
                Align2::LEFT_TOP,
                "Clique numa zona para escolher a tecla; botão direito deixa livre.\nEncoste o dedo no touchpad para testar.",
                FontId::proportional(11.5),
                pal.text_muted,
            );
            if let Some((i, capture)) = &self.capture {
                let (i, held) = (*i, capture.keys().to_vec());
                match capture_overlay(ui, pal, area, &format!("Mapeando Zona {}", i + 1), &held) {
                    Some(ModalAction::Pick(combo)) => {
                        self.draft.zones[i].key = Some(combo);
                        self.capture = None;
                    }
                    Some(_) => self.capture = None,
                    None => {}
                }
            }
        });
    }

    fn pad(&mut self, ui: &Ui, pal: &Palette, live: &Live, rect: Rect, resp: &egui::Response) {
        let painter = ui.painter_at(rect.expand(2.0));
        let tp = &self.draft;
        let clicked = live.pressed.contains(&Button::Touchpad);
        let border = if clicked { ACTIVE } else { pal.dial_line };
        painter.rect(
            rect,
            18.0,
            pal.dial_bg,
            Stroke::new(1.5, border),
            StrokeKind::Inside,
        );

        let (cols, rows) = (tp.cols as usize, tp.rows as usize);
        let cell = vec2(rect.width() / cols as f32, rect.height() / rows as f32);
        let zone_rect = |i: usize| {
            let min = rect.min + vec2((i % cols) as f32 * cell.x, (i / cols) as f32 * cell.y);
            Rect::from_min_size(min, cell).shrink(5.0)
        };
        let normalized = |p: Pos2| {
            (
                (p.x - rect.left()) / rect.width(),
                (p.y - rect.top()) / rect.height(),
            )
        };

        let hovered = resp
            .hover_pos()
            .filter(|_| self.capture.is_none())
            .map(|p| {
                let (x, y) = normalized(p);
                tp.zone_at(x, y)
            });
        let active = live.touch_zones(tp);

        for (i, zone) in tp.zones.iter().enumerate() {
            let r = zone_rect(i);
            if active.contains(&i) {
                painter.rect(
                    r,
                    10.0,
                    Color32::from_rgba_unmultiplied(0xEA, 0xB3, 0x08, 50),
                    Stroke::new(1.5, ACTIVE),
                    StrokeKind::Inside,
                );
            } else if hovered == Some(i) {
                painter.rect_stroke(r, 10.0, Stroke::new(1.5, ACCENT), StrokeKind::Inside);
            }
            let c = r.center();
            painter.text(
                c - vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                format!("Zona {}", i + 1),
                FontId::proportional(11.0),
                pal.text_muted,
            );
            let (text, bg, fg) = match &zone.key {
                Some(k) => (combo_label(k).to_uppercase(), pal.key_bg, pal.key_text),
                None => ("LIVRE".to_owned(), pal.chip_bg, pal.chip_text),
            };
            let g = painter.layout_no_wrap(text, FontId::proportional(12.0), fg);
            let chip = Rect::from_center_size(c + vec2(0.0, 9.0), g.size() + vec2(16.0, 8.0));
            painter.rect_filled(chip, 5.0, bg);
            painter.galley(chip.center() - g.size() / 2.0, g, fg);
        }

        // Divisas tracejadas entre as zonas.
        let stroke = Stroke::new(1.0, pal.dial_line);
        for c in 1..cols {
            let x = rect.left() + c as f32 * cell.x;
            painter.extend(egui::Shape::dashed_line(
                &[
                    Pos2::new(x, rect.top() + 8.0),
                    Pos2::new(x, rect.bottom() - 8.0),
                ],
                stroke,
                5.0,
                4.0,
            ));
        }
        for r in 1..rows {
            let y = rect.top() + r as f32 * cell.y;
            painter.extend(egui::Shape::dashed_line(
                &[
                    Pos2::new(rect.left() + 8.0, y),
                    Pos2::new(rect.right() - 8.0, y),
                ],
                stroke,
                5.0,
                4.0,
            ));
        }

        for &(x, y) in live.touch.values() {
            let p = rect.min + vec2(x * rect.width(), y * rect.height());
            painter.circle(p, 8.0, ACTIVE, Stroke::new(2.0, pal.dial_bg));
        }

        if let Some(i) = hovered {
            if resp.clicked() {
                self.capture = Some((i, Capture::default()));
            } else if resp.secondary_clicked() {
                self.draft.zones[i].key = None;
            }
        }
    }

    fn grid_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            ui.label(heading(pal, "DIVISÃO"));
            ui.spacing_mut().item_spacing.y = 6.0;
            for &(label, cols, rows) in PRESETS {
                let selected = self.draft.cols == cols && self.draft.rows == rows;
                if list_row(ui, pal, label, selected, true).clicked() {
                    self.draft.set_grid(cols, rows);
                    self.capture = None;
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let (mut cols, mut rows) = (self.draft.cols, self.draft.rows);
                ui.label(RichText::new("Colunas").size(12.0).color(pal.text_muted));
                ui.add(egui::DragValue::new(&mut cols).range(1..=MAX_COLS));
                ui.add_space(8.0);
                ui.label(RichText::new("Linhas").size(12.0).color(pal.text_muted));
                ui.add(egui::DragValue::new(&mut rows).range(1..=MAX_ROWS));
                if (cols, rows) != (self.draft.cols, self.draft.rows) {
                    self.draft.set_grid(cols, rows);
                    self.capture = None;
                }
            });
        });
    }

    fn activate_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            ui.label(heading(pal, "ACIONA"));
            let options = [
                (TouchActivate::Click, "Ao clicar"),
                (TouchActivate::Touch, "Ao tocar"),
            ];
            let cell = (ui.available_width() - 6.0) / 2.0;
            if let Some(a) = segmented(ui, pal, &options, self.draft.activate, cell) {
                self.draft.activate = a;
            }
            let text = match self.draft.activate {
                TouchActivate::Click => {
                    "A zona sob o dedo dispara quando você aperta o touchpad. Só encostar não faz nada."
                }
                TouchActivate::Touch => {
                    "A zona dispara assim que o dedo encosta e solta quando ele sai. Cada dedo aciona a sua."
                }
            };
            ui.label(RichText::new(text).size(11.5).color(pal.text_muted));
            if let (TouchActivate::Click, Some(key)) = (self.draft.activate, &self.click_key) {
                ui.label(
                    RichText::new(format!(
                        "O clique do touchpad hoje aperta {key}. Ao salvar, ele passa a acionar as zonas e essa tecla sai do perfil."
                    ))
                    .size(11.5)
                    .color(pal.bad_text),
                );
            }
        });
    }
}
