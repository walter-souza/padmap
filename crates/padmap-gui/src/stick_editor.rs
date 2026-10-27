//! Editor de stick por ângulo: o círculo do stick dividido em setores, cada um
//! com sua tecla. Arrastar as alças move as divisas, clicar num setor captura a
//! tecla e o botão direito deixa o setor livre.
//!
//! Tudo é editado numa cópia; o perfil só muda em "Salvar Mapeamento".

use crate::app::switch;
use crate::capture::Capture;
use crate::diagram::{polar, wedge, Live};
use crate::slots::stick_title;
use crate::theme::{Palette, ACCENT, ACCENT_STRONG, ACTIVE, SECTOR_COLORS};
use eframe::egui::{
    self, vec2, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Layout, Margin, Pos2,
    Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
};
use padmap_core::stick::angle_of;
use padmap_core::{Axis, Profile, Sector, Stick, StickBinding};

const LEFT_W: f32 = 400.0;
const RIGHT_W: f32 = 300.0;
const GAP: f32 = 16.0;
const PAD: i8 = 20;
const WIDTH: f32 = LEFT_W + GAP + RIGHT_W + 2.0 * PAD as f32;
const DIAL_H: f32 = 340.0;
/// Raio, em pixels, em que uma alça da divisa pode ser agarrada.
const HANDLE_GRAB: f32 = 11.0;
/// Quanto o stick precisa sair do centro para o eixo ser escolhido.
const PICK_THRESHOLD: f32 = 0.6;
const STICK_AXES: [Axis; 4] = [Axis::LeftX, Axis::LeftY, Axis::RightX, Axis::RightY];

struct Preset {
    label: &'static str,
    /// Teclas a partir do setor de cima (4 direções) ou de cima-direita (8),
    /// no sentido anti-horário.
    keys: &'static [&'static str],
}

const PRESETS: &[Preset] = &[
    Preset {
        label: "4 Direções — WASD",
        keys: &["W", "A", "S", "D"],
    },
    Preset {
        label: "4 Direções — Setas Direcionais",
        keys: &["Up", "Left", "Down", "Right"],
    },
    Preset {
        label: "8 Direções — WASD + ECZQ",
        keys: &["E", "W", "Q", "A", "Z", "S", "C", "D"],
    },
    Preset {
        label: "8 Direções — WASD (WD SD SA WA)",
        keys: &["W+D", "W", "W+A", "A", "S+A", "S", "S+D", "D"],
    },
];

impl Preset {
    /// Setores de mesma largura. Com `w = 360 / n`, o setor `k` fica centrado em
    /// `w * (k + 1)`: cima em 4 direções, cima-direita em 8.
    fn sectors(&self) -> Vec<Sector> {
        let w = 360.0 / self.keys.len() as f32;
        self.keys
            .iter()
            .enumerate()
            .map(|(k, key)| {
                Sector::new(
                    w * (k as f32 + 0.5),
                    Some(key.parse().expect("tecla de preset válida")),
                )
            })
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    X,
    Y,
}

pub enum Outcome {
    Save(StickBinding),
    Cancel,
}

pub struct StickEditor {
    pub stick: Stick,
    /// Cópia em edição, sempre com eixos e zona morta preenchidos.
    draft: StickBinding,
    /// `settings.axis_threshold` do perfil: a zona morta que vale sem uma própria.
    default_deadzone: f32,
    selected: Option<usize>,
    capture: Option<(usize, Capture)>,
    picking: Option<Field>,
    dragging: Option<usize>,
}

impl StickEditor {
    pub fn new(stick: Stick, profile: &Profile) -> Self {
        let mut draft = profile.sticks.get(&stick).cloned().unwrap_or_else(|| {
            let free = PRESETS[0]
                .sectors()
                .into_iter()
                .map(|s| Sector::new(s.start, None))
                .collect();
            StickBinding::new(free, false)
        });
        let (x, y) = draft.axes(stick);
        draft.x_axis = Some(x);
        draft.y_axis = Some(y);
        draft.deadzone = Some(profile.deadzone(stick));
        StickEditor {
            stick,
            draft,
            default_deadzone: profile.settings.axis_threshold,
            selected: None,
            capture: None,
            picking: None,
            dragging: None,
        }
    }

    /// Esperando uma tecla para algum setor.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    fn axes(&self) -> (Axis, Axis) {
        self.draft.axes(self.stick)
    }

    fn deadzone(&self) -> f32 {
        self.draft.deadzone.unwrap_or(self.default_deadzone)
    }

    /// A configuração como vai para o perfil: o que é padrão não é escrito.
    fn finish(&self) -> StickBinding {
        let mut b = self.draft.clone();
        let (x, y) = self.stick.default_axes();
        if b.x_axis == Some(x) {
            b.x_axis = None;
        }
        if b.y_axis == Some(y) {
            b.y_axis = None;
        }
        if b.deadzone == Some(self.default_deadzone) {
            b.deadzone = None;
        }
        b
    }

    /// Desenha o editor. Retorna o resultado quando ele fecha.
    pub fn show(&mut self, ctx: &egui::Context, pal: &Palette, live: &Live) -> Option<Outcome> {
        let was_busy = self.capture.is_some() || self.picking.is_some();
        self.feed_capture(ctx);
        self.pick_axis(live);

        let mut outcome = None;
        egui::Modal::new(egui::Id::unique("padmap-stick"))
            .frame(
                egui::Frame::new()
                    .fill(pal.bg)
                    .stroke(Stroke::new(1.0, pal.bar_border))
                    .corner_radius(14),
            )
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                if self.header(ui, pal) {
                    outcome = Some(Outcome::Cancel);
                }
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(PAD, 16))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = GAP;
                            ui.vertical(|ui| {
                                ui.set_width(LEFT_W);
                                self.axes_card(ui, pal);
                                self.dial_card(ui, pal, live);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(RIGHT_W);
                                self.deadzone_card(ui, pal);
                                self.presets_card(ui, pal);
                            });
                        });
                    });
                if let Some(o) = self.footer(ui, pal) {
                    outcome = Some(o);
                }
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.picking.is_some() {
                self.picking = None;
            } else if !was_busy && outcome.is_none() {
                outcome = Some(Outcome::Cancel);
            }
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
            self.draft.sectors[i].key = Some(combo);
            self.capture = None;
        }
    }

    /// Escolhe o eixo que o usuário mexeu, se um campo está esperando.
    fn pick_axis(&mut self, live: &Live) {
        let Some(field) = self.picking else {
            return;
        };
        let Some(axis) = STICK_AXES
            .into_iter()
            .filter(|a| live.axis(*a).abs() >= PICK_THRESHOLD)
            .max_by(|a, b| live.axis(*a).abs().total_cmp(&live.axis(*b).abs()))
        else {
            return;
        };
        // Se o eixo já está no outro campo, os dois trocam de lugar.
        let (x, y) = self.axes();
        let (x, y) = match field {
            Field::X => (axis, if axis == y { x } else { y }),
            Field::Y => (if axis == x { y } else { x }, axis),
        };
        self.draft.x_axis = Some(x);
        self.draft.y_axis = Some(y);
        self.picking = None;
    }

    /// Move a divisa `i` sem perder o setor selecionado, que pode mudar de
    /// índice quando a lista é reordenada.
    fn move_boundary(&mut self, i: usize, angle: f32) {
        let keep = self
            .selected
            .filter(|&s| s != i)
            .map(|s| self.draft.sectors[s].start);
        let moved = self.draft.move_start(i, angle);
        self.dragging = Some(moved);
        self.selected = if self.selected == Some(i) {
            Some(moved)
        } else {
            keep.and_then(|start| self.draft.sectors.iter().position(|s| s.start == start))
        };
    }

    // ---------------------------------------------------------------- partes

    /// Faixa de título. Retorna `true` se o × foi clicado.
    fn header(&self, ui: &mut Ui, pal: &Palette) -> bool {
        let mut close = false;
        let frame = egui::Frame::new()
            .fill(pal.chrome)
            .corner_radius(CornerRadius {
                nw: 14,
                ne: 14,
                sw: 0,
                se: 0,
            })
            .inner_margin(Margin::symmetric(PAD, 14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Mapeamento por Ângulo · {}",
                            stick_title(self.stick)
                        ))
                        .size(16.0)
                        .strong()
                        .color(pal.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let x = egui::Button::new(
                            RichText::new("×").size(18.0).color(pal.chrome_muted),
                        )
                        .frame(false);
                        if ui.add(x).on_hover_text("Fechar sem salvar").clicked() {
                            close = true;
                        }
                    });
                });
            });
        // Mesma linha que separa o cabeçalho da tela principal.
        let r = frame.response.rect;
        ui.painter()
            .hline(r.x_range(), r.bottom(), Stroke::new(1.0, pal.chrome_border));
        close
    }

    fn axes_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            ui.horizontal(|ui| {
                ui.label(heading(pal, "EIXOS DO CONTROLE"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("Selecione e mova o analógico")
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                });
            });
            let (x, y) = self.axes();
            let mut clicked = None;
            ui.columns(2, |cols| {
                for (col, field, axis, label) in
                    [(0, Field::X, x, "EIXO X"), (1, Field::Y, y, "EIXO Y")]
                {
                    let picking = self.picking == Some(field);
                    if axis_field(&mut cols[col], pal, label, axis, picking).clicked() {
                        clicked = Some(field);
                    }
                }
            });
            if let Some(field) = clicked {
                self.picking = if self.picking == Some(field) {
                    None
                } else {
                    Some(field)
                };
            }
        });
    }

    fn dial_card(&mut self, ui: &mut Ui, pal: &Palette, live: &Live) {
        card(ui, pal, |ui| {
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), DIAL_H), Sense::click_and_drag());
            self.dial(ui, pal, live, rect, &resp);
            if let Some((i, capture)) = &self.capture {
                let preview = capture.preview();
                if capture_overlay(ui, pal, rect, *i, &preview) {
                    self.capture = None;
                }
            }
        });
    }

    fn dial(&mut self, ui: &Ui, pal: &Palette, live: &Live, rect: Rect, resp: &Response) {
        let c = rect.center();
        let r_out = rect.width().min(rect.height()) / 2.0 - 22.0;
        let band = 16.0;
        let r_in = r_out - band;
        let r_handle = r_in + band / 2.0;
        let n = self.draft.sectors.len();

        // Interação antes do desenho, para o quadro já mostrar o resultado.
        if self.capture.is_none() {
            let handle_at = |b: &StickBinding, p: Pos2| {
                (n > 1)
                    .then(|| {
                        (0..n).find(|&i| {
                            polar(c, r_handle, b.sectors[i].start).distance(p) <= HANDLE_GRAB
                        })
                    })
                    .flatten()
            };
            if resp.drag_started() {
                let origin = ui.input(|i| i.pointer.press_origin());
                self.dragging = origin.and_then(|p| handle_at(&self.draft, p));
            }
            if let (Some(i), Some(p)) = (self.dragging, resp.interact_pointer_pos()) {
                if resp.dragged() {
                    self.move_boundary(i, angle_at(c, p).round());
                }
            }
            if resp.drag_stopped() {
                self.dragging = None;
            }
            let over_handle = resp
                .hover_pos()
                .is_some_and(|p| handle_at(&self.draft, p).is_some());
            if self.dragging.is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if over_handle {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }

            let pointer = resp.interact_pointer_pos();
            let sector_under = pointer
                .filter(|p| !over_handle && p.distance(c) <= r_out)
                .and_then(|p| self.draft.sector_at_angle(angle_at(c, p)));
            if let Some(i) = sector_under {
                if resp.clicked() {
                    self.selected = Some(i);
                    self.capture = Some((i, Capture::default()));
                } else if resp.secondary_clicked() {
                    self.selected = Some(i);
                    self.draft.sectors[i].key = None;
                }
            }
            if resp.hovered() && !over_handle && self.dragging.is_none() {
                resp.clone().on_hover_text(
                    "Clique num setor para escolher a tecla.\n\
                     Botão direito deixa o setor livre.\n\
                     Arraste as alças para mudar as divisas.",
                );
            }
        }

        let painter = ui.painter_at(rect);
        let b = &self.draft;
        let color = |i: usize| SECTOR_COLORS[i % SECTOR_COLORS.len()];

        painter.circle_stroke(c, r_out + 5.0, Stroke::new(1.0, pal.bar_border));
        for i in 0..n {
            let mut col = color(i);
            if b.sectors[i].key.is_none() {
                col = col.gamma_multiply(0.35);
            }
            let thick = if self.selected == Some(i) {
                band + 6.0
            } else {
                band
            };
            arc(
                &painter,
                c,
                r_handle,
                b.sectors[i].start,
                b.width(i),
                thick,
                col,
            );
        }
        painter.circle_filled(c, r_in, pal.dial_bg);

        let (ax, ay) = self.axes();
        let (x, y) = (live.axis(ax), live.axis(ay));
        for i in b.sectors_at(x, y, self.deadzone()) {
            wedge(
                &painter,
                c,
                r_in,
                b.sectors[i].start,
                b.width(i),
                color(i).gamma_multiply(0.22),
            );
        }

        let line = Stroke::new(1.0, pal.dial_line);
        painter.line_segment([c - vec2(r_in, 0.0), c + vec2(r_in, 0.0)], line);
        painter.line_segment([c - vec2(0.0, r_in), c + vec2(0.0, r_in)], line);
        let font = FontId::proportional(11.0);
        painter.text(
            c - vec2(0.0, r_out + 14.0),
            Align2::CENTER_CENTER,
            "Y",
            font.clone(),
            pal.text_muted,
        );
        painter.text(
            c + vec2(r_out + 14.0, 0.0),
            Align2::CENTER_CENTER,
            "X",
            font,
            pal.text_muted,
        );
        if n > 1 {
            for s in &b.sectors {
                dashed_line(&painter, c, polar(c, r_in, s.start), line);
            }
        }
        dashed_circle(
            &painter,
            c,
            self.deadzone() * r_in,
            Stroke::new(1.2, ACCENT.gamma_multiply(0.8)),
        );

        for i in 0..n {
            let text = b.sectors[i]
                .key
                .as_ref()
                .map_or("—".to_owned(), |k| k.to_string().to_uppercase());
            let g = painter.layout_no_wrap(text, FontId::monospace(10.0), pal.text);
            let chip = Rect::from_center_size(
                polar(c, r_in * 0.68, b.center(i)),
                vec2(g.size().x + 14.0, 20.0),
            );
            let border = if self.selected == Some(i) {
                Stroke::new(1.5, ACCENT)
            } else {
                Stroke::new(1.0, pal.free_border)
            };
            painter.rect(chip, 6.0, pal.free_bg, border, StrokeKind::Inside);
            painter.galley(chip.center() - g.size() / 2.0, g, pal.text);
        }

        if n > 1 {
            for i in 0..n {
                let p = polar(c, r_handle, b.sectors[i].start);
                let ring = if self.dragging == Some(i) {
                    ACCENT
                } else {
                    pal.bar_border
                };
                painter.circle(p, 8.0, pal.bar, Stroke::new(1.5, ring));
                painter.circle_filled(p, 3.5, color(i));
            }
        }

        let mut knob = vec2(x, y);
        if knob.length() > 1.0 {
            knob = knob.normalized();
        }
        painter.circle(c, 5.0, pal.text_muted, Stroke::new(2.0, pal.dial_bg));
        painter.circle(c + knob * r_in, 6.5, ACCENT, Stroke::new(2.0, pal.bar));
    }

    fn deadzone_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            let mut dz = self.deadzone();
            ui.horizontal(|ui| {
                ui.label(heading(pal, "ZONA MORTA"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    egui::Frame::new()
                        .fill(ACCENT.gamma_multiply(0.12))
                        .stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.5)))
                        .corner_radius(5)
                        .inner_margin(Margin::symmetric(6, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{:.0}%", dz * 100.0))
                                    .size(11.0)
                                    .strong()
                                    .color(ACCENT),
                            );
                        });
                });
            });
            ui.scope(|ui| {
                ui.spacing_mut().slider_width = ui.available_width();
                ui.visuals_mut().widgets.inactive.bg_fill = pal.switch_off;
                let slider = egui::Slider::new(&mut dz, 0.05..=0.95)
                    .show_value(false)
                    .trailing_fill(true);
                if ui
                    .add(slider)
                    .on_hover_text("Quanto o stick precisa sair do centro para acionar um setor.")
                    .changed()
                {
                    self.draft.deadzone = Some((dz * 100.0).round() / 100.0);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new("Disparo Compartilhado")
                            .strong()
                            .color(pal.text),
                    );
                    ui.label(
                        RichText::new("Ativa múltiplos setores nas diagonais.")
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let resp = switch(ui, pal, self.draft.shared).on_hover_text(
                        "Setores vizinhos se sobrepõem perto da divisa e acionam juntos.",
                    );
                    if resp.clicked() {
                        self.draft.shared = !self.draft.shared;
                    }
                });
            });
        });
    }

    fn presets_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            ui.label(heading(pal, "PRESETS DE DIREÇÕES"));
            ui.spacing_mut().item_spacing.y = 6.0;
            let current = PRESETS
                .iter()
                .position(|p| p.sectors() == self.draft.sectors);
            for (k, preset) in PRESETS.iter().enumerate() {
                if list_row(ui, pal, preset.label, current == Some(k), true).clicked() {
                    self.draft.sectors = preset.sectors();
                    self.selected = None;
                }
            }
            list_row(ui, pal, "⚙  Personalizado", current.is_none(), false)
                .on_hover_text("Fica marcado quando os setores não são iguais a nenhum preset.");

            ui.add_space(4.0);
            ui.separator();
            let n = self.draft.sectors.len();
            ui.label(heading(pal, &format!("GERENCIAR REGIÕES ({n})")));
            let w = (ui.available_width() - 8.0) / 2.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let add = ui
                    .add(egui::Button::new("+  Adicionar").min_size(vec2(w, 32.0)))
                    .on_hover_text("Divide o setor selecionado (ou o maior) ao meio.");
                if add.clicked() {
                    let target = self.selected.unwrap_or_else(|| {
                        (0..n)
                            .max_by(|&a, &b| self.draft.width(a).total_cmp(&self.draft.width(b)))
                            .unwrap_or(0)
                    });
                    if let Some(i) = self.draft.split(target) {
                        self.selected = Some(i);
                    }
                }
                let remove = ui
                    .add_enabled(
                        n > 1,
                        egui::Button::new("−  Remover").min_size(vec2(w, 32.0)),
                    )
                    .on_hover_text("Remove o setor selecionado (ou o último).");
                if remove.clicked() {
                    self.draft.remove(self.selected.unwrap_or(n - 1));
                    self.selected = None;
                }
            });
            let clear = ui.add(
                egui::Button::new("×  Limpar Mapeamentos")
                    .min_size(vec2(ui.available_width(), 32.0)),
            );
            if clear.clicked() {
                for s in &mut self.draft.sectors {
                    s.key = None;
                }
            }
        });
    }

    fn footer(&self, ui: &mut Ui, pal: &Palette) -> Option<Outcome> {
        let mut outcome = None;
        let top = ui.cursor().top();
        ui.painter().hline(
            ui.max_rect().x_range(),
            top,
            Stroke::new(1.0, pal.bar_border),
        );
        egui::Frame::new()
            .fill(pal.bar)
            .corner_radius(CornerRadius {
                nw: 0,
                ne: 0,
                sw: 14,
                se: 14,
            })
            .inner_margin(Margin::symmetric(PAD, 14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let save = egui::Button::new(
                        RichText::new("Salvar Mapeamento")
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_STRONG)
                    .stroke(Stroke::NONE)
                    .corner_radius(9)
                    .min_size(vec2(0.0, 38.0));
                    let busy = self.capture.is_some();
                    if ui.add_enabled(!busy, save).clicked() {
                        outcome = Some(Outcome::Save(self.finish()));
                    }
                    let cancel = egui::Button::new(RichText::new("Cancelar").color(pal.free_text))
                        .frame(false)
                        .min_size(vec2(0.0, 38.0));
                    if ui.add(cancel).clicked() {
                        outcome = Some(Outcome::Cancel);
                    }
                });
            });
        outcome
    }
}

// -------------------------------------------------------------------- peças

/// Ângulo do ponto `p` em volta de `c`, na mesma convenção dos setores.
fn angle_at(c: Pos2, p: Pos2) -> f32 {
    angle_of(p.x - c.x, p.y - c.y)
}

/// Índice do eixo no SDL, como os controles chamam ("canal").
fn channel(axis: Axis) -> u8 {
    match axis {
        Axis::LeftX => 0,
        Axis::LeftY => 1,
        Axis::RightX => 2,
        Axis::RightY => 3,
        Axis::LeftTrigger => 4,
        Axis::RightTrigger => 5,
    }
}

fn heading(pal: &Palette, text: &str) -> RichText {
    RichText::new(text)
        .size(11.0)
        .strong()
        .color(pal.text_muted)
}

fn card<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(pal.bar)
        .stroke(Stroke::new(1.0, pal.bar_border))
        .corner_radius(12)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 10.0;
            add(ui)
        })
        .inner
}

/// Campo de eixo: nome à esquerda, canal à direita. Clicado, espera o stick mexer.
fn axis_field(ui: &mut Ui, pal: &Palette, label: &str, axis: Axis, picking: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let border = if picking || resp.hovered() {
        ACCENT
    } else {
        pal.free_border
    };
    ui.painter().rect(
        rect,
        8.0,
        pal.free_bg,
        Stroke::new(if picking { 1.5 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_center() + vec2(12.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        pal.free_text,
    );
    let (value, color, font) = if picking {
        (
            "Mova o stick…".to_owned(),
            ACCENT,
            FontId::proportional(12.0),
        )
    } else {
        (
            format!("Canal {}", channel(axis)),
            pal.text,
            FontId::monospace(12.0),
        )
    };
    ui.painter().text(
        rect.right_center() - vec2(12.0, 0.0),
        Align2::RIGHT_CENTER,
        value,
        font,
        color,
    );
    resp.on_hover_text(format!("{axis}. Clique e mova o stick para trocar o eixo."))
}

/// Linha da lista de presets. `clickable: false` só mostra o estado.
fn list_row(ui: &mut Ui, pal: &Palette, text: &str, selected: bool, clickable: bool) -> Response {
    let sense = if clickable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), sense);
    let (fill, border, color) = if selected {
        (ACCENT_STRONG, ACCENT_STRONG, Color32::WHITE)
    } else if clickable && resp.hovered() {
        (pal.free_bg, ACCENT, pal.text)
    } else {
        (pal.free_bg, pal.free_border, pal.free_text)
    };
    ui.painter().rect(
        rect,
        8.0,
        fill,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_center() + vec2(12.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(12.5),
        color,
    );
    resp
}

/// Cobre o círculo enquanto a tecla de um setor é capturada.
/// Retorna `true` se a captura foi cancelada.
fn capture_overlay(ui: &mut Ui, pal: &Palette, rect: Rect, sector: usize, preview: &str) -> bool {
    ui.painter()
        .rect_filled(rect.expand(8.0), 10.0, pal.bar.gamma_multiply(0.94));
    let mut cancel = false;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.add_space(rect.height() * 0.2);
            let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
            ui.painter()
                .circle(r.center(), 30.0, pal.bar, Stroke::new(2.0, ACTIVE));
            ui.painter().circle_filled(r.center(), 9.0, ACCENT);
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Mapeando Região {}", sector + 1))
                    .size(16.0)
                    .strong()
                    .color(pal.text),
            );
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new("Pressione as teclas no seu teclado físico").color(pal.text_muted),
            );
            ui.label(RichText::new("para mapear este setor…").color(pal.text_muted));
            ui.add_space(6.0);
            let shown = if preview.is_empty() {
                "…".to_owned()
            } else {
                format!("{preview}+…")
            };
            ui.label(RichText::new(shown).monospace().size(16.0).color(pal.text));
            ui.add_space(10.0);
            if ui.button("Cancelar Mapeamento").clicked() {
                cancel = true;
            }
        },
    );
    cancel
}

/// Arco grosso centrado no raio `r`, de `start` até `start + width` graus.
fn arc(
    painter: &egui::Painter,
    c: Pos2,
    r: f32,
    start: f32,
    width: f32,
    thick: f32,
    color: Color32,
) {
    let steps = (width / 2.0).ceil().max(2.0) as usize;
    let points = (0..=steps)
        .map(|k| polar(c, r, start + width * k as f32 / steps as f32))
        .collect();
    painter.add(Shape::line(points, Stroke::new(thick, color)));
}

fn dashed_line(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    painter.extend(Shape::dashed_line(&[a, b], stroke, 4.0, 4.0));
}

fn dashed_circle(painter: &egui::Painter, c: Pos2, r: f32, stroke: Stroke) {
    let n = 64;
    let points: Vec<Pos2> = (0..=n)
        .map(|k| polar(c, r, 360.0 * k as f32 / n as f32))
        .collect();
    painter.extend(Shape::dashed_line(&points, stroke, 4.0, 4.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use padmap_core::Key;

    #[test]
    fn presets_are_valid_profiles() {
        for preset in PRESETS {
            let b = StickBinding::new(preset.sectors(), false);
            let mut p = Profile::from_toml("name='t'").unwrap();
            p.sticks.insert(Stick::Left, b);
            assert!(Profile::from_toml(&p.to_toml()).is_ok(), "{}", preset.label);
        }
    }

    #[test]
    fn four_way_preset_puts_w_on_top() {
        let b = StickBinding::new(PRESETS[0].sectors(), false);
        let up = b.sector_at_angle(90.0).unwrap();
        assert_eq!(b.sectors[up].key, Some(Key::W.into()));
        let b = StickBinding::new(PRESETS[2].sectors(), false);
        let up_right = b.sector_at_angle(45.0).unwrap();
        assert_eq!(b.sectors[up_right].key, Some(Key::E.into()));
    }

    #[test]
    fn default_profile_left_stick_is_a_preset_layout() {
        let p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        let ed = StickEditor::new(Stick::Left, &p);
        assert_eq!(ed.draft.sectors, PRESETS[0].sectors());
        // Nada mudou: o que volta para o perfil é igual ao que estava lá.
        assert_eq!(&ed.finish(), &p.sticks[&Stick::Left]);
    }
}
