//! Editor do touchpad, em três modos. Zonas: o touchpad dividido numa grade,
//! cada zona com sua tecla. Mouse: deslizar o dedo move o cursor e apertar o
//! touchpad clica. Gestos: deslizar numa direção aperta uma tecla. Clicar numa
//! tecla captura outra e o botão direito deixa livre; os dedos aparecem ao
//! vivo, para testar sem ativar o mapeamento.
//!
//! Tudo é editado numa cópia; o perfil só muda em "Salvar Mapeamento".

use crate::app::segmented;
use crate::capture::{combo_label, Capture, ModalAction};
use crate::diagram::Live;
use crate::stick_editor::{
    capture_overlay, card, dashed_circle, heading, list_row, modal_footer, modal_header, slider,
    title_row,
};
use crate::theme::{Palette, ACCENT, ACTIVE};
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Margin, Pos2, Rect, Response, RichText, Sense, Shape,
    Stroke, StrokeKind, Ui,
};
use padmap_core::touchpad::{ASPECT, MAX_COLS, MAX_ROWS};
use padmap_core::{
    Button, ExtraSource, KeyCombo, Profile, Swipe, TouchActivate, TouchMode, TouchpadBinding,
};
use std::collections::VecDeque;

const LEFT_W: f32 = 440.0;
const RIGHT_W: f32 = 270.0;
const GAP: f32 = 16.0;
const PAD: i8 = 20;
const WIDTH: f32 = LEFT_W + GAP + RIGHT_W + 2.0 * PAD as f32;
/// Altura da área do touchpad com a dica; cabe a captura de tecla por cima.
const AREA_H: f32 = 346.0;
/// Pontos guardados do rastro do dedo no modo mouse.
const TRAIL: usize = 40;

/// Divisões prontas: `(nome, colunas, linhas)`.
const PRESETS: &[(&str, u8, u8)] = &[
    ("Inteiro", 1, 1),
    ("2 metades", 2, 1),
    ("3 colunas", 3, 1),
    ("4 quadrantes", 2, 2),
    ("6 zonas (3 × 2)", 3, 2),
];

pub enum Outcome {
    /// `None` quando o modo escolhido não aciona nada: o touchpad sai do perfil.
    Save(Option<TouchpadBinding>),
    Cancel,
}

/// O que recebe a tecla que está sendo capturada.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Zone(usize),
    Click,
    TwoFingerClick,
    Swipe(Swipe),
}

impl Target {
    fn title(self) -> String {
        match self {
            Target::Zone(i) => format!("Mapeando Zona {}", i + 1),
            Target::Click => "Mapeando o Clique".to_owned(),
            Target::TwoFingerClick => "Mapeando o Clique com Dois Dedos".to_owned(),
            Target::Swipe(d) => format!("Mapeando Deslizar {}", swipe_label(d)),
        }
    }
}

pub struct TouchpadEditor {
    draft: TouchpadBinding,
    capture: Option<(Target, Capture)>,
    /// Tecla que o clique do touchpad tem hoje como botão. Nos modos em que o
    /// clique é do touchpad, essa tecla sai do perfil ao salvar.
    click_key: Option<String>,
    /// Rastro do dedo no modo mouse, em `0..=1`.
    trail: VecDeque<(f32, f32)>,
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
            trail: VecDeque::new(),
        }
    }

    /// Esperando uma tecla.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    fn finish(&self) -> Option<TouchpadBinding> {
        (!self.draft.is_unused()).then(|| self.draft.clone())
    }

    fn key_mut(&mut self, target: Target) -> &mut Option<KeyCombo> {
        match target {
            Target::Zone(i) => &mut self.draft.zones[i].key,
            Target::Click => &mut self.draft.mouse.click,
            Target::TwoFingerClick => &mut self.draft.mouse.two_finger_click,
            Target::Swipe(d) => self.draft.swipe.key_mut(d),
        }
    }

    /// Clique esquerdo captura uma tecla; o direito deixa livre.
    fn key_clicks(&mut self, resp: &Response, target: Target) {
        if resp.clicked() {
            self.capture = Some((target, Capture::default()));
        } else if resp.secondary_clicked() {
            *self.key_mut(target) = None;
        }
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
                let closed = modal_header(ui, pal, "Mapeamento do Touchpad", |ui| {
                    let options = [
                        (TouchMode::Zones, "Zonas"),
                        (TouchMode::Mouse, "Mouse"),
                        (TouchMode::Swipe, "Gestos"),
                    ];
                    if let Some(mode) = segmented(ui, pal, &options, self.draft.mode, 78.0) {
                        self.draft.mode = mode;
                        self.capture = None;
                    }
                });
                if closed {
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
                                match self.draft.mode {
                                    TouchMode::Zones => {
                                        self.grid_card(ui, pal);
                                        self.activate_card(ui, pal);
                                    }
                                    TouchMode::Mouse => self.mouse_card(ui, pal),
                                    TouchMode::Swipe => self.swipe_card(ui, pal),
                                }
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
        let Some((target, capture)) = &mut self.capture else {
            return;
        };
        let events = ctx.input(|inp| inp.events.clone());
        if let Some(combo) = capture.feed(&events) {
            let target = *target;
            *self.key_mut(target) = Some(combo);
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
            let clicked = live.pressed.contains(&Button::Touchpad);
            ui.painter().rect(
                rect,
                18.0,
                pal.dial_bg,
                Stroke::new(1.5, if clicked { ACTIVE } else { pal.dial_line }),
                StrokeKind::Inside,
            );
            let hint = match self.draft.mode {
                TouchMode::Zones => {
                    "Clique numa zona para escolher a tecla; botão direito deixa livre.\nEncoste o dedo no touchpad para testar."
                }
                TouchMode::Mouse => {
                    "Deslize o dedo no touchpad para testar. O cursor de verdade\nsó anda com o mapeamento ativo."
                }
                TouchMode::Swipe => {
                    "Clique numa seta para escolher a tecla; botão direito deixa livre.\nO círculo mostra quanto o dedo precisa andar."
                }
            };
            match self.draft.mode {
                TouchMode::Zones => self.zones(ui, pal, live, rect),
                TouchMode::Mouse => self.mouse_test(ui, pal, live, rect),
                TouchMode::Swipe => self.swipes(ui, pal, live, rect),
            }
            for &(x, y) in live.touch.values() {
                let p = rect.min + vec2(x * rect.width(), y * rect.height());
                ui.painter()
                    .circle(p, 8.0, ACTIVE, Stroke::new(2.0, pal.dial_bg));
            }
            ui.painter().text(
                Pos2::new(area.left(), rect.bottom() + 12.0),
                Align2::LEFT_TOP,
                hint,
                FontId::proportional(11.5),
                pal.text_muted,
            );
            if let Some((target, capture)) = &self.capture {
                let (target, held) = (*target, capture.keys().to_vec());
                match capture_overlay(ui, pal, area, &target.title(), &held) {
                    Some(ModalAction::Pick(combo)) => {
                        *self.key_mut(target) = Some(combo);
                        self.capture = None;
                    }
                    Some(_) => self.capture = None,
                    None => {}
                }
            }
        });
    }

    // ------------------------------------------------------------ zonas

    fn zones(&mut self, ui: &Ui, pal: &Palette, live: &Live, rect: Rect) {
        let resp = ui.interact(rect, ui.scope_id().with("zones"), Sense::click());
        let painter = ui.painter_at(rect.expand(2.0));
        let tp = &self.draft;
        let (cols, rows) = (tp.cols as usize, tp.rows as usize);
        let cell = vec2(rect.width() / cols as f32, rect.height() / rows as f32);
        let zone_rect = |i: usize| {
            let min = rect.min + vec2((i % cols) as f32 * cell.x, (i / cols) as f32 * cell.y);
            Rect::from_min_size(min, cell).shrink(5.0)
        };

        let hovered = resp
            .hover_pos()
            .filter(|_| self.capture.is_none())
            .map(|p| {
                tp.zone_at(
                    (p.x - rect.left()) / rect.width(),
                    (p.y - rect.top()) / rect.height(),
                )
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
            key_chip(&painter, pal, c + vec2(0.0, 9.0), zone.key.as_ref(), false);
        }

        // Divisas tracejadas entre as zonas.
        let stroke = Stroke::new(1.0, pal.dial_line);
        for c in 1..cols {
            let x = rect.left() + c as f32 * cell.x;
            painter.extend(Shape::dashed_line(
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
            painter.extend(Shape::dashed_line(
                &[
                    Pos2::new(rect.left() + 8.0, y),
                    Pos2::new(rect.right() - 8.0, y),
                ],
                stroke,
                5.0,
                4.0,
            ));
        }

        if let Some(i) = hovered {
            self.key_clicks(&resp, Target::Zone(i));
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
            self.click_warning(ui, pal);
        });
    }

    /// Avisa que a tecla própria do clique do touchpad sai do perfil, se o
    /// modo escolhido usa o clique.
    fn click_warning(&self, ui: &mut Ui, pal: &Palette) {
        if let (true, Some(key)) = (self.draft.owns_click(), &self.click_key) {
            ui.label(
                RichText::new(format!(
                    "O clique do touchpad hoje aperta {key}. Ao salvar, ele passa a ser do touchpad e essa tecla sai do perfil."
                ))
                .size(11.5)
                .color(pal.bad_text),
            );
        }
    }

    // ------------------------------------------------------------ mouse

    /// Rastro do primeiro dedo, como o cursor andaria.
    fn mouse_test(&mut self, ui: &Ui, pal: &Palette, live: &Live, rect: Rect) {
        let finger = live.touch.keys().min().and_then(|f| live.touch.get(f));
        match finger {
            Some(&p) if self.trail.back() != Some(&p) => {
                self.trail.push_back(p);
                if self.trail.len() > TRAIL {
                    self.trail.pop_front();
                }
            }
            Some(_) => {}
            None => self.trail.clear(),
        }
        let painter = ui.painter_at(rect);
        if self.trail.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Deslize para mover o mouse",
                FontId::proportional(12.0),
                pal.text_muted,
            );
            return;
        }
        let n = self.trail.len();
        let points: Vec<Pos2> = self
            .trail
            .iter()
            .map(|&(x, y)| rect.min + vec2(x * rect.width(), y * rect.height()))
            .collect();
        for (i, w) in points.windows(2).enumerate() {
            let alpha = (i + 1) as f32 / n as f32;
            painter.line_segment([w[0], w[1]], Stroke::new(3.0, ACCENT.gamma_multiply(alpha)));
        }
    }

    fn mouse_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            let m = &mut self.draft.mouse;
            title_row(ui, pal, "VELOCIDADE", &format!("{:.0} px", m.speed));
            slider(ui, pal, &mut m.speed, 300.0..=4000.0).on_hover_text(
                "Quanto o cursor anda quando o dedo atravessa o touchpad de lado a lado.",
            );
            m.speed = (m.speed / 50.0).round() * 50.0;
            ui.label(
                RichText::new("Pixels que o cursor anda com o dedo de um lado ao outro.")
                    .size(11.5)
                    .color(pal.text_muted),
            );
            ui.separator();
            ui.label(heading(pal, "CLIQUES"));
            for (target, label) in [
                (Target::Click, "Apertar o touchpad"),
                (Target::TwoFingerClick, "Dois dedos + apertar"),
            ] {
                let key = match target {
                    Target::Click => self.draft.mouse.click.clone(),
                    _ => self.draft.mouse.two_finger_click.clone(),
                };
                let resp = key_row(ui, pal, label, key.as_ref());
                self.key_clicks(&resp, target);
            }
            self.click_warning(ui, pal);
        });
    }

    // ------------------------------------------------------------ gestos

    fn swipes(&mut self, ui: &Ui, pal: &Palette, live: &Live, rect: Rect) {
        let painter = ui.painter_at(rect.expand(2.0));
        let s = &self.draft.swipe;
        // A distância vale nas duas direções na escala da largura.
        let reach = s.distance * rect.width();
        let to_px = |(x, y): (f32, f32)| rect.min + vec2(x * rect.width(), y * rect.height());

        let mut lit = Vec::new();
        for (f, &now) in &live.touch {
            let Some(&start) = live.touch_start.get(f) else {
                continue;
            };
            let (a, b) = (to_px(start), to_px(now));
            dashed_circle(&painter, a, reach, Stroke::new(1.0, pal.dial_line));
            painter.line_segment([a, b], Stroke::new(2.5, ACCENT));
            if let Some(d) = s.direction(start, now) {
                lit.push(d);
            }
        }
        if live.touch.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "deslize",
                FontId::proportional(12.0),
                pal.text_muted,
            );
        }

        let c = rect.center();
        let (hx, hy) = (rect.width() / 2.0 - 52.0, rect.height() / 2.0 - 24.0);
        let mut clicks = Vec::new();
        for d in Swipe::ALL {
            let at = c + match d {
                Swipe::Up => vec2(0.0, -hy),
                Swipe::Down => vec2(0.0, hy),
                Swipe::Left => vec2(-hx, 0.0),
                Swipe::Right => vec2(hx, 0.0),
            };
            let r = key_chip(&painter, pal, at, s.key(d), lit.contains(&d));
            // Seta entre o centro e o selo, apontando para fora.
            let (arrow, off) = match d {
                Swipe::Up => ("↑", vec2(0.0, 22.0)),
                Swipe::Down => ("↓", vec2(0.0, -22.0)),
                Swipe::Left => ("←", vec2(r.width() / 2.0 + 16.0, 0.0)),
                Swipe::Right => ("→", vec2(-r.width() / 2.0 - 16.0, 0.0)),
            };
            painter.text(
                at + off,
                Align2::CENTER_CENTER,
                arrow,
                FontId::proportional(15.0),
                pal.text_muted,
            );
            let r = r.expand(4.0);
            let resp = ui.interact(r, ui.scope_id().with(("swipe", d)), Sense::click());
            if resp.hovered() && self.capture.is_none() {
                painter.rect_stroke(r, 7.0, Stroke::new(1.5, ACCENT), StrokeKind::Inside);
            }
            clicks.push((resp, d));
        }
        for (resp, d) in clicks {
            self.key_clicks(&resp, Target::Swipe(d));
        }
    }

    fn swipe_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            let s = &mut self.draft.swipe;
            title_row(ui, pal, "DISTÂNCIA", &format!("{:.0}%", s.distance * 100.0));
            slider(ui, pal, &mut s.distance, 0.1..=0.6)
                .on_hover_text("Quanto o dedo precisa andar, em parte da largura do touchpad.");
            s.distance = (s.distance * 100.0).round() / 100.0;
            ui.label(
                RichText::new(
                    "Deslize numa direção: a tecla dela é apertada quando o dedo anda o bastante e fica apertada até ele sair. Um gesto por toque.",
                )
                .size(11.5)
                .color(pal.text_muted),
            );
            ui.separator();
            ui.label(
                RichText::new(
                    "Neste modo o clique do touchpad fica livre: dá para mapeá-lo na aba Botões extras.",
                )
                .size(11.5)
                .color(pal.text_muted),
            );
        });
    }
}

// -------------------------------------------------------------------- peças

fn swipe_label(d: Swipe) -> &'static str {
    match d {
        Swipe::Up => "para Cima",
        Swipe::Down => "para Baixo",
        Swipe::Left => "para a Esquerda",
        Swipe::Right => "para a Direita",
    }
}

/// Texto e cores do selo de tecla, como nas etiquetas da tela principal.
fn chip_style(pal: &Palette, key: Option<&KeyCombo>, lit: bool) -> (String, Color32, Color32) {
    match key {
        Some(k) if lit => (combo_label(k).to_uppercase(), ACTIVE, Color32::BLACK),
        Some(k) => (combo_label(k).to_uppercase(), pal.key_bg, pal.key_text),
        None => ("LIVRE".to_owned(), pal.chip_bg, pal.chip_text),
    }
}

/// Selo de tecla com o centro em `at`. Retorna onde ficou.
fn key_chip(
    painter: &egui::Painter,
    pal: &Palette,
    at: Pos2,
    key: Option<&KeyCombo>,
    lit: bool,
) -> Rect {
    let (text, bg, fg) = chip_style(pal, key, lit);
    let g = painter.layout_no_wrap(text, FontId::proportional(12.0), fg);
    let chip = Rect::from_center_size(at, g.size() + vec2(16.0, 8.0));
    painter.rect_filled(chip, 5.0, bg);
    painter.galley(chip.center() - g.size() / 2.0, g, fg);
    chip
}

/// Linha "nome ……… [TECLA]". Retorna a resposta do selo, que é clicável.
fn key_row(ui: &mut Ui, pal: &Palette, label: &str, key: Option<&KeyCombo>) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        pal.text,
    );
    let (text, _, fg) = chip_style(pal, key, false);
    let width = painter
        .layout_no_wrap(text, FontId::proportional(12.0), fg)
        .size()
        .x
        + 16.0;
    let at = Pos2::new(rect.right() - 3.0 - width / 2.0, rect.center().y);
    let chip = key_chip(&painter, pal, at, key, false);
    let resp = ui
        .interact(
            chip.expand(3.0),
            ui.scope_id().with(("key", label)),
            Sense::click(),
        )
        .on_hover_text(
            "Clique para trocar a tecla.
Botão direito deixa livre.",
        );
    if resp.hovered() {
        painter.rect_stroke(
            chip.expand(3.0),
            7.0,
            Stroke::new(1.5, ACCENT),
            StrokeKind::Inside,
        );
    }
    resp
}
