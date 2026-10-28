//! Editor de stick. No modo teclas, o círculo do stick é dividido em setores,
//! cada um com sua tecla: arrastar as alças move as divisas, clicar num setor
//! captura a tecla e o botão direito deixa o setor livre. No modo mouse, o stick
//! move o cursor, com velocidade, curva e uma área para testar.
//!
//! Tudo é editado numa cópia; o perfil só muda em "Salvar Mapeamento".

use crate::app::switch;
use crate::capture::{combo_label, keys_ui, mouse_buttons_ui, Capture, ModalAction};
use crate::diagram::{polar, wedge, Live};
use crate::slots::stick_title;
use crate::theme::{Palette, ACCENT, ACCENT_STRONG, SECTOR_COLORS};
use eframe::egui::{
    self, vec2, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Layout, Margin, Pos2,
    Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
};
use padmap_core::stick::{angle_of, SHARED_OVERLAP};
use padmap_core::{Axis, Key, MouseSettings, Profile, Sector, Stick, StickBinding, StickMode};
use std::collections::VecDeque;

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

struct MousePreset {
    label: &'static str,
    speed: f32,
    curve: f32,
}

const MOUSE_PRESETS: &[MousePreset] = &[
    MousePreset {
        label: "Mouse padrão",
        speed: 1200.0,
        curve: 2.0,
    },
    MousePreset {
        label: "Mouse preciso",
        speed: 700.0,
        curve: 2.6,
    },
    MousePreset {
        label: "Mouse rápido",
        speed: 2000.0,
        curve: 1.6,
    },
];

/// Zona morta sugerida ao passar um stick para o modo mouse.
const MOUSE_DEADZONE: f32 = 0.15;

/// Pontos guardados do rastro do cursor na área de teste.
const TRAIL: usize = 48;

/// Cursor de mentira da área de teste, em coordenadas relativas ao centro.
#[derive(Default)]
struct TestCursor {
    pos: egui::Vec2,
    trail: VecDeque<egui::Vec2>,
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
    /// Quanto a lista de setores girou desde que o editor abriu. Os setores
    /// ficam ordenados a partir de 0°, então uma divisa que cruza 0° muda o
    /// índice de todos; somar isso ao índice mantém a cor de cada setor.
    color_shift: usize,
    test: TestCursor,
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
        // Um stick em modo mouse pode vir do perfil sem setores; ao voltar para
        // teclas, começa com os de 4 direções livres.
        if draft.sectors.is_empty() {
            draft.sectors = PRESETS[0]
                .sectors()
                .into_iter()
                .map(|s| Sector::new(s.start, None))
                .collect();
        }
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
            color_shift: 0,
            test: TestCursor::default(),
        }
    }

    /// Esperando uma tecla para algum setor.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// Troca o modo. A zona morta que ainda está no padrão de um modo passa para
    /// o padrão do outro: metade do curso serve para teclas, mas é demais para
    /// mirar com o mouse.
    fn set_mode(&mut self, mode: StickMode) {
        let dz = self.deadzone();
        match mode {
            StickMode::Mouse if dz == self.default_deadzone => {
                self.draft.deadzone = Some(MOUSE_DEADZONE)
            }
            StickMode::Sectors if dz == MOUSE_DEADZONE => {
                self.draft.deadzone = Some(self.default_deadzone)
            }
            _ => {}
        }
        self.draft.mode = mode;
        self.capture = None;
        self.selected = None;
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
                        let mouse = self.draft.mode == StickMode::Mouse;
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = GAP;
                            ui.vertical(|ui| {
                                ui.set_width(LEFT_W);
                                self.axes_card(ui, pal);
                                if mouse {
                                    self.test_card(ui, pal, live);
                                } else {
                                    self.dial_card(ui, pal, live);
                                }
                            });
                            ui.vertical(|ui| {
                                ui.set_width(RIGHT_W);
                                if mouse {
                                    self.mouse_card(ui, pal);
                                    self.mouse_presets_card(ui, pal);
                                } else {
                                    self.deadzone_card(ui, pal);
                                    self.presets_card(ui, pal);
                                }
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
        // A divisa não passa da vizinha: mudar de índice só acontece ao cruzar
        // 0°, e aí a lista inteira gira de `i - moved` posições.
        let n = self.draft.sectors.len();
        self.color_shift = (self.color_shift + i + n - moved) % n;
        self.dragging = Some(moved);
        self.selected = if self.selected == Some(i) {
            Some(moved)
        } else {
            keep.and_then(|start| self.draft.sectors.iter().position(|s| s.start == start))
        };
    }

    // ---------------------------------------------------------------- partes

    /// Faixa de título. Retorna `true` se o × foi clicado.
    fn header(&mut self, ui: &mut Ui, pal: &Palette) -> bool {
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
                        RichText::new(format!("Mapeamento do Stick · {}", stick_title(self.stick)))
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
                        ui.add_space(8.0);
                        if let Some(mode) = mode_toggle(ui, pal, self.draft.mode) {
                            self.set_mode(mode);
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
                let (i, held) = (*i, capture.keys().to_vec());
                match capture_overlay(ui, pal, rect, i, &held) {
                    Some(ModalAction::Pick(combo)) => {
                        self.draft.sectors[i].key = Some(combo);
                        self.capture = None;
                    }
                    Some(_) => self.capture = None,
                    None => {}
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
        let color = |i: usize| sector_color(i, self.color_shift, n);

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

        // Disparo compartilhado: nas divisas, a faixa em que os dois setores
        // vizinhos acionam juntos, com as cores dos dois se misturando.
        if b.shared && n > 1 {
            for i in 0..n {
                let prev = (i + n - 1) % n;
                if b.sectors[i].key.is_none() || b.sectors[prev].key.is_none() {
                    continue;
                }
                let start = b.sectors[i].start;
                // Cada setor avança uma fração da própria largura sobre o vizinho.
                let from = start - b.width(i) * SHARED_OVERLAP;
                let to = start + b.width(prev) * SHARED_OVERLAP;
                blend_band(
                    &painter,
                    c,
                    self.deadzone() * r_in,
                    r_in,
                    (from, to),
                    // Cada lado da faixa com a cor do setor em que está.
                    (color(prev), color(i)),
                );
            }
        }

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
                .map_or("—".to_owned(), |k| combo_label(k).to_uppercase());
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
            self.deadzone_row(ui, pal);
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

    fn deadzone_row(&mut self, ui: &mut Ui, pal: &Palette) {
        let mut dz = self.deadzone();
        title_row(ui, pal, "ZONA MORTA", &format!("{:.0}%", dz * 100.0));
        let resp = slider(ui, pal, &mut dz, 0.05..=0.95)
            .on_hover_text("Quanto o stick precisa sair do centro para agir.");
        if resp.changed() {
            self.draft.deadzone = Some((dz * 100.0).round() / 100.0);
        }
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
                    self.color_shift = 0;
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

    // ------------------------------------------------------------ modo mouse

    /// Área onde um cursor de mentira anda com o stick, na mesma velocidade
    /// que o jogo vai receber, sem precisar ativar o mapeamento.
    fn test_card(&mut self, ui: &mut Ui, pal: &Palette, live: &Live) {
        let (ax, ay) = self.axes();
        let (x, y) = (live.axis(ax), live.axis(ay));
        let (vx, vy) = self.draft.mouse.velocity(x, y, self.deadzone());
        let speed = vx.hypot(vy);
        card(ui, pal, |ui| {
            let chip = if speed > 0.0 {
                format!("~{:.0} px/s", speed)
            } else {
                "parado".to_owned()
            };
            title_row(ui, pal, "ÁREA DE TESTE", &chip);
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), DIAL_H - 30.0), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 8.0, pal.dial_bg);
            // Grade de pontos.
            let mut gy = rect.top() + 10.0;
            while gy < rect.bottom() {
                let mut gx = rect.left() + 10.0;
                while gx < rect.right() {
                    painter.circle_filled(Pos2::new(gx, gy), 1.0, pal.dial_line);
                    gx += 20.0;
                }
                gy += 20.0;
            }

            // Pixels de tela viram pontos do egui, para a velocidade ser a real.
            let dt = ui.input(|i| i.stable_dt).min(0.05);
            let step = vec2(vx, vy) * dt / ui.ctx().pixels_per_point();
            let t = &mut self.test;
            if step != egui::Vec2::ZERO {
                let half = rect.size() / 2.0;
                let mut p = t.pos + step;
                // Saindo por um lado, volta pelo outro; o rastro recomeça.
                let wrapped = p.x.abs() > half.x || p.y.abs() > half.y;
                if p.x.abs() > half.x {
                    p.x -= p.x.signum() * rect.width();
                }
                if p.y.abs() > half.y {
                    p.y -= p.y.signum() * rect.height();
                }
                if wrapped {
                    t.trail.clear();
                }
                t.pos = p;
                t.trail.push_back(p);
                if t.trail.len() > TRAIL {
                    t.trail.pop_front();
                }
                ui.ctx().request_repaint();
            } else if !t.trail.is_empty() {
                t.trail.pop_front();
                ui.ctx().request_repaint();
            }
            let c = rect.center();
            let points: Vec<Pos2> = t.trail.iter().map(|&p| c + p).collect();
            if points.len() > 1 {
                painter.add(Shape::line(
                    points,
                    Stroke::new(2.0, ACCENT.gamma_multiply(0.7)),
                ));
            }
            cursor_icon(&painter, c + t.pos, pal);

            // Stick pequeno no canto, com a zona morta.
            let sc = rect.left_bottom() + vec2(36.0, -36.0);
            painter.circle(sc, 26.0, pal.bar, Stroke::new(1.5, pal.dial_line));
            dashed_circle(
                &painter,
                sc,
                26.0 * self.deadzone(),
                Stroke::new(1.0, ACCENT),
            );
            let knob = sc + vec2(x, y).clamp(vec2(-1.0, -1.0), vec2(1.0, 1.0)) * 20.0;
            painter.line_segment([sc, knob], Stroke::new(2.0, ACCENT));
            painter.circle_filled(knob, 4.5, ACCENT);
            painter.text(
                rect.right_bottom() - vec2(10.0, 10.0),
                Align2::RIGHT_BOTTOM,
                "Mova o stick para testar",
                FontId::proportional(11.0),
                pal.text_muted,
            );
        });
    }

    fn mouse_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            let m = &mut self.draft.mouse;
            title_row(ui, pal, "VELOCIDADE", &format!("{:.0} px/s", m.speed));
            slider(ui, pal, &mut m.speed, 200.0..=3000.0)
                .on_hover_text("Velocidade do cursor com o stick no fim do curso.");
            m.speed = (m.speed / 50.0).round() * 50.0;
            ui.separator();

            let feel = if m.curve < 1.4 {
                "Linear"
            } else if m.curve < 2.3 {
                "Suave"
            } else {
                "Preciso"
            };
            title_row(ui, pal, "PRECISÃO", feel);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(64.0, 44.0), Sense::hover());
                curve_preview(ui.painter(), r, m.curve, pal);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    slider(ui, pal, &mut m.curve, 1.0..=3.0).on_hover_text(
                        "Mais precisão deixa o começo do curso lento, para mirar, \
                         sem perder a velocidade no fim.",
                    );
                    m.curve = (m.curve * 10.0).round() / 10.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Linear").size(11.0).color(pal.text_muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("Preciso").size(11.0).color(pal.text_muted));
                        });
                    });
                });
            });
            ui.separator();
            self.deadzone_row(ui, pal);
            ui.separator();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new("Inverter eixo Y").strong().color(pal.text));
                    ui.label(
                        RichText::new("Para cima move o cursor para baixo.")
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let m = &mut self.draft.mouse;
                    if switch(ui, pal, m.invert_y).clicked() {
                        m.invert_y = !m.invert_y;
                    }
                });
            });
        });
    }

    fn mouse_presets_card(&mut self, ui: &mut Ui, pal: &Palette) {
        card(ui, pal, |ui| {
            ui.label(heading(pal, "PRESETS DE MOUSE"));
            ui.spacing_mut().item_spacing.y = 6.0;
            let m = &mut self.draft.mouse;
            let current = MOUSE_PRESETS
                .iter()
                .position(|p| p.speed == m.speed && p.curve == m.curve);
            for (k, preset) in MOUSE_PRESETS.iter().enumerate() {
                if list_row(ui, pal, preset.label, current == Some(k), true).clicked() {
                    *m = MouseSettings {
                        speed: preset.speed,
                        curve: preset.curve,
                        ..*m
                    };
                }
            }
            list_row(ui, pal, "⚙  Personalizado", current.is_none(), false).on_hover_text(
                "Fica marcado quando a velocidade e a precisão não são de um preset.",
            );
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

/// Seletor Teclas | Mouse. Retorna o modo escolhido quando muda.
fn mode_toggle(ui: &mut Ui, pal: &Palette, current: StickMode) -> Option<StickMode> {
    let options = [(StickMode::Sectors, "Teclas"), (StickMode::Mouse, "Mouse")];
    let cell = vec2(78.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(
        vec2(cell.x * options.len() as f32 + 6.0, cell.y + 6.0),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 8.0, pal.toggle_bg);
    let mut chosen = None;
    for (i, (mode, label)) in options.into_iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(3.0 + cell.x * i as f32, 3.0), cell);
        let resp = ui.interact(r, ui.scope_id().with(("mode", i)), Sense::click());
        let selected = mode == current;
        if selected {
            ui.painter().rect(
                r,
                6.0,
                pal.toggle_on,
                Stroke::new(1.0, pal.button_border),
                StrokeKind::Inside,
            );
        }
        let color = if selected || resp.hovered() {
            pal.text
        } else {
            pal.text_muted
        };
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.5),
            color,
        );
        if resp.clicked() && !selected {
            chosen = Some(mode);
        }
    }
    chosen
}

/// Título de seção com o valor num selo à direita.
fn title_row(ui: &mut Ui, pal: &Palette, title: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(heading(pal, title));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::Frame::new()
                .fill(pal.key_bg)
                .corner_radius(5)
                .inner_margin(Margin::symmetric(6, 1))
                .show(ui, |ui| {
                    ui.label(RichText::new(value).size(11.0).strong().color(pal.key_text));
                });
        });
    });
}

/// Deslizante na largura toda, com o trilho na cor do tema.
fn slider(
    ui: &mut Ui,
    pal: &Palette,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = ui.available_width();
        ui.visuals_mut().widgets.inactive.bg_fill = pal.switch_off;
        ui.add(
            egui::Slider::new(value, range)
                .show_value(false)
                .trailing_fill(true),
        )
    })
    .inner
}

/// Gráfico pequeno da curva de resposta: força do stick → velocidade.
fn curve_preview(painter: &egui::Painter, r: Rect, curve: f32, pal: &Palette) {
    painter.rect_filled(r, 6.0, pal.dial_bg);
    let inner = r.shrink(6.0);
    painter.line_segment(
        [inner.left_bottom(), inner.right_bottom()],
        Stroke::new(1.0, pal.dial_line),
    );
    painter.line_segment(
        [inner.left_bottom(), inner.left_top()],
        Stroke::new(1.0, pal.dial_line),
    );
    let points = (0..=20)
        .map(|k| {
            let t = k as f32 / 20.0;
            Pos2::new(
                inner.left() + t * inner.width(),
                inner.bottom() - t.powf(curve) * inner.height(),
            )
        })
        .collect();
    painter.add(Shape::line(points, Stroke::new(2.0, ACCENT)));
}

/// Seta de cursor com a ponta em `tip`.
fn cursor_icon(painter: &egui::Painter, tip: Pos2, pal: &Palette) {
    let p = |x: f32, y: f32| tip + vec2(x, y);
    let points = vec![
        p(0.0, 0.0),
        p(0.0, 17.0),
        p(4.5, 13.0),
        p(7.5, 19.5),
        p(10.0, 18.5),
        p(7.0, 12.0),
        p(12.5, 12.0),
    ];
    painter.add(Shape::closed_line(
        points.clone(),
        Stroke::new(2.5, pal.bar),
    ));
    // Não é convexo: pinta em dois triângulos e contorna por cima.
    painter.add(Shape::convex_polygon(
        vec![p(0.0, 0.0), p(0.0, 17.0), p(12.5, 12.0)],
        pal.text,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![p(4.5, 13.0), p(7.5, 19.5), p(10.0, 18.5), p(7.0, 12.0)],
        pal.text,
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(points, Stroke::new(1.0, pal.text)));
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

/// Cobre o círculo enquanto a tecla de um setor é capturada. Retorna
/// `Cancel` se a captura foi cancelada ou `Pick` se um clique foi escolhido.
fn capture_overlay(
    ui: &mut Ui,
    pal: &Palette,
    rect: Rect,
    sector: usize,
    held: &[Key],
) -> Option<ModalAction> {
    ui.painter()
        .rect_filled(rect.expand(8.0), 10.0, pal.bar.gamma_multiply(0.96));
    let mut action = None;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink(16.0))
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Mapeando Região {}", sector + 1))
                    .size(16.0)
                    .strong()
                    .color(pal.text),
            );
            ui.add_space(8.0);
            keys_ui(ui, pal, held);
            ui.add_space(10.0);
            if let Some(combo) = mouse_buttons_ui(ui, pal, held) {
                action = Some(ModalAction::Pick(combo));
            }
            ui.add_space(10.0);
            if ui.button("Cancelar Mapeamento").clicked() {
                action = Some(ModalAction::Cancel);
            }
        },
    );
    action
}

/// Cor do setor `i` de `n`, descontado o giro da lista (`shift`).
fn sector_color(i: usize, shift: usize, n: usize) -> Color32 {
    SECTOR_COLORS[(i + shift) % n.max(1) % SECTOR_COLORS.len()]
}

/// Faixa de `r0` a `r1` entre os ângulos `from` e `to`, com a cor indo
/// suavemente de `colors.0` (em `from`) a `colors.1` (em `to`).
fn blend_band(
    painter: &egui::Painter,
    c: Pos2,
    r0: f32,
    r1: f32,
    (from, to): (f32, f32),
    (a, b): (Color32, Color32),
) {
    const STEPS: usize = 16;
    const ALPHA: f32 = 0.18;
    let mut mesh = egui::Mesh::default();
    for k in 0..=STEPS {
        let t = k as f32 / STEPS as f32;
        let angle = from + (to - from) * t;
        let color = a.lerp_to_gamma(b, t).gamma_multiply(ALPHA);
        mesh.colored_vertex(polar(c, r0, angle), color);
        mesh.colored_vertex(polar(c, r1, angle), color);
        if k > 0 {
            let i = 2 * k as u32;
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i + 1, i);
        }
    }
    painter.add(Shape::mesh(mesh));
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
    fn mouse_presets_are_valid() {
        for preset in MOUSE_PRESETS {
            let mut b = StickBinding::new(Vec::new(), false);
            b.mode = StickMode::Mouse;
            b.mouse.speed = preset.speed;
            b.mouse.curve = preset.curve;
            let mut p = Profile::from_toml("name='t'").unwrap();
            p.sticks.insert(Stick::Right, b);
            assert!(Profile::from_toml(&p.to_toml()).is_ok(), "{}", preset.label);
        }
        // O padrão do núcleo é o primeiro preset.
        let d = MouseSettings::default();
        assert_eq!(
            (d.speed, d.curve),
            (MOUSE_PRESETS[0].speed, MOUSE_PRESETS[0].curve)
        );
    }

    #[test]
    fn colors_follow_sectors_across_zero() {
        let p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        let mut ed = StickEditor::new(Stick::Left, &p);
        let n = ed.draft.sectors.len();
        let color_of = |ed: &StickEditor, key: &str| {
            let i = ed
                .draft
                .sectors
                .iter()
                .position(|s| s.key == Some(key.parse().unwrap()))
                .unwrap();
            sector_color(i, ed.color_shift, n)
        };
        let before: Vec<_> = ["W", "A", "S", "D"].map(|k| color_of(&ed, k)).into();
        // W começa em 45°; passar para 350° cruza 0° e o reordena para o fim.
        ed.move_boundary(0, 350.0);
        assert_eq!(ed.draft.sectors[n - 1].key, Some(Key::W.into()));
        let after: Vec<_> = ["W", "A", "S", "D"].map(|k| color_of(&ed, k)).into();
        assert_eq!(before, after);
        // E de volta.
        ed.move_boundary(n - 1, 20.0);
        let back: Vec<_> = ["W", "A", "S", "D"].map(|k| color_of(&ed, k)).into();
        assert_eq!(before, back);
    }

    #[test]
    fn mode_switch_moves_default_deadzone() {
        let p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        let mut ed = StickEditor::new(Stick::Right, &p);
        ed.set_mode(StickMode::Mouse);
        assert_eq!(ed.deadzone(), MOUSE_DEADZONE);
        ed.set_mode(StickMode::Sectors);
        assert_eq!(&ed.finish(), &p.sticks[&Stick::Right]);
        // Uma zona morta escolhida pelo usuário fica como está.
        ed.draft.deadzone = Some(0.3);
        ed.set_mode(StickMode::Mouse);
        assert_eq!(ed.deadzone(), 0.3);
    }

    #[test]
    fn mouse_stick_without_sectors_gets_free_ones() {
        let p = Profile::from_toml("name='t'\n[sticks.right]\nmode='mouse'\n").unwrap();
        let ed = StickEditor::new(Stick::Right, &p);
        assert_eq!(ed.draft.sectors.len(), 4);
        assert!(ed.draft.sectors.iter().all(|s| s.key.is_none()));
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
