//! Tela principal: o controle desenhado ao vivo, uma etiqueta para cada botão
//! ligada a ele por uma linha, e os dois sticks embaixo.
//!
//! Tudo é desenhado num espaço de 1000 × 640 unidades e escalado para caber na
//! janela. Clicar numa etiqueta ou numa parte do controle devolve um `Click`.

use crate::slots::{stick_click, stick_short, stick_title, Slot};
use crate::theme::{Palette, ACCENT, ACCENT_STRONG, ACTIVE};
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Galley, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui,
};
use padmap_core::{Axis, Button, Profile, Stick};
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;
use std::sync::Arc;

#[derive(Default)]
pub struct Live {
    pub pressed: HashSet<Button>,
    pub axes: HashMap<Axis, f32>,
}

impl Live {
    pub fn axis(&self, a: Axis) -> f32 {
        self.axes.get(&a).copied().unwrap_or(0.0)
    }

    pub fn is_active(&self, slot: Slot, profile: &Profile) -> bool {
        match slot {
            Slot::Button(b) => self.pressed.contains(&b),
            Slot::Trigger(a) => self.axis(a) >= threshold(profile, a),
        }
    }

    /// Setores com tecla que o stick está acionando agora.
    pub fn stick_sectors(&self, profile: &Profile, stick: Stick) -> Vec<usize> {
        let Some(binding) = profile.sticks.get(&stick) else {
            return Vec::new();
        };
        let (ax, ay) = binding.axes(stick);
        binding
            .sectors_at(self.axis(ax), self.axis(ay), profile.deadzone(stick))
            .into_iter()
            .filter(|&i| binding.sectors[i].key.is_some())
            .collect()
    }
}

/// Ponto em que o gatilho aciona a tecla, como o `Mapper` calcula.
pub fn threshold(profile: &Profile, axis: Axis) -> f32 {
    profile
        .axes
        .get(&axis)
        .and_then(|b| b.threshold)
        .unwrap_or(profile.settings.axis_threshold)
}

/// Eixos que o stick usa no perfil.
pub fn stick_axes(profile: &Profile, stick: Stick) -> (Axis, Axis) {
    profile
        .sticks
        .get(&stick)
        .map_or(stick.default_axes(), |b| b.axes(stick))
}

/// Ponto a `r` do centro `c` no ângulo `deg` (anti-horário a partir da direita).
pub fn polar(c: Pos2, r: f32, deg: f32) -> Pos2 {
    let a = deg.to_radians();
    c + vec2(a.cos(), -a.sin()) * r
}

/// Fatia de pizza de `start` até `start + width` graus.
pub fn wedge(painter: &egui::Painter, c: Pos2, r: f32, start: f32, width: f32, fill: Color32) {
    // Em pedaços de até 45° para cada polígono continuar convexo.
    let pieces = (width / 45.0).ceil().max(1.0) as usize;
    let step = width / pieces as f32;
    for p in 0..pieces {
        let a0 = start + step * p as f32;
        let mut points = vec![c];
        for k in 0..=8 {
            points.push(polar(c, r, a0 + step * k as f32 / 8.0));
        }
        painter.add(Shape::convex_polygon(points, fill, Stroke::NONE));
    }
}

pub enum Click {
    /// Capturar uma tecla para este botão.
    Map(Slot),
    /// Deixar o botão livre (botão direito do mouse).
    Clear(Slot),
    EditStick(Stick),
}

const WIDTH: f32 = 1000.0;
const HEIGHT: f32 = 640.0;

enum Side {
    /// Etiqueta à esquerda do controle, alinhada à direita em x = 240.
    Left,
    /// Etiqueta à direita do controle, começando em x = 760.
    Right,
    /// Etiqueta acima do controle, centrada em `x`. `key_first` põe a tecla antes do nome.
    Top { x: f32, key_first: bool },
}

struct Callout {
    slot: Slot,
    label: &'static str,
    side: Side,
    /// Centro vertical (ignorado nas etiquetas de cima).
    y: f32,
    /// Linha até o botão. Nas etiquetas de cima o primeiro ponto é a base da etiqueta.
    line: &'static [(f32, f32)],
}

const fn left(slot: Slot, label: &'static str, y: f32, line: &'static [(f32, f32)]) -> Callout {
    Callout {
        slot,
        label,
        side: Side::Left,
        y,
        line,
    }
}

const fn right(slot: Slot, label: &'static str, y: f32, line: &'static [(f32, f32)]) -> Callout {
    Callout {
        slot,
        label,
        side: Side::Right,
        y,
        line,
    }
}

const CALLOUTS: &[Callout] = &[
    left(
        Slot::Trigger(Axis::LeftTrigger),
        "L2",
        56.0,
        &[(240.0, 56.0), (310.0, 56.0)],
    ),
    left(
        Slot::Button(Button::LeftShoulder),
        "L1",
        90.0,
        &[(240.0, 90.0), (280.0, 90.0), (308.0, 83.0)],
    ),
    left(
        Slot::Button(Button::LeftStick),
        "L3",
        160.0,
        &[(240.0, 160.0), (290.0, 160.0), (348.0, 162.0)],
    ),
    left(
        Slot::Button(Button::DpadUp),
        "D-Cima",
        212.0,
        &[(240.0, 212.0), (290.0, 212.0), (440.0, 204.0)],
    ),
    left(
        Slot::Button(Button::DpadLeft),
        "D-Esquerda",
        252.0,
        &[(240.0, 252.0), (300.0, 252.0), (412.0, 232.0)],
    ),
    left(
        Slot::Button(Button::DpadRight),
        "D-Direita",
        292.0,
        &[(240.0, 292.0), (310.0, 292.0), (468.0, 232.0)],
    ),
    left(
        Slot::Button(Button::DpadDown),
        "D-Baixo",
        332.0,
        &[(240.0, 332.0), (320.0, 332.0), (440.0, 260.0)],
    ),
    Callout {
        slot: Slot::Button(Button::Back),
        label: "Select",
        side: Side::Top {
            x: 365.0,
            key_first: true,
        },
        y: 10.0,
        line: &[(404.0, 26.0), (464.0, 164.0)],
    },
    Callout {
        slot: Slot::Button(Button::Guide),
        label: "Home",
        side: Side::Top {
            x: 500.0,
            key_first: true,
        },
        y: 10.0,
        line: &[(500.0, 26.0), (500.0, 121.0)],
    },
    Callout {
        slot: Slot::Button(Button::Start),
        label: "Start",
        side: Side::Top {
            x: 635.0,
            key_first: false,
        },
        y: 10.0,
        line: &[(596.0, 26.0), (536.0, 164.0)],
    },
    right(
        Slot::Trigger(Axis::RightTrigger),
        "R2",
        56.0,
        &[(760.0, 56.0), (690.0, 56.0)],
    ),
    right(
        Slot::Button(Button::RightShoulder),
        "R1",
        90.0,
        &[(760.0, 90.0), (720.0, 90.0), (692.0, 83.0)],
    ),
    right(
        Slot::Button(Button::North),
        "Y",
        150.0,
        &[(760.0, 150.0), (720.0, 150.0), (631.0, 138.0)],
    ),
    right(
        Slot::Button(Button::East),
        "B",
        194.0,
        &[(760.0, 194.0), (720.0, 194.0), (661.0, 168.0)],
    ),
    right(
        Slot::Button(Button::West),
        "X",
        238.0,
        &[(760.0, 238.0), (720.0, 238.0), (588.0, 168.0)],
    ),
    right(
        Slot::Button(Button::South),
        "A",
        282.0,
        &[(760.0, 282.0), (720.0, 282.0), (618.0, 198.0)],
    ),
    right(
        Slot::Button(Button::RightStick),
        "R3",
        340.0,
        &[(760.0, 340.0), (720.0, 340.0), (572.0, 252.0)],
    ),
];

const FACE_BG: Color32 = Color32::from_rgb(0x14, 0x18, 0x27);
const FACE_PRESSED_TEXT: Color32 = Color32::from_rgb(0x0B, 0x10, 0x20);
const HOME: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
const ARROW: Color32 = Color32::from_rgb(0x7C, 0x86, 0xA8);
const KNOB_IDLE: Color32 = Color32::from_rgb(0x9C, 0xA3, 0xAF);

struct Stage<'a> {
    ui: &'a mut Ui,
    painter: egui::Painter,
    origin: Pos2,
    s: f32,
    pal: &'a Palette,
    live: &'a Live,
    profile: &'a Profile,
    capturing: Option<Slot>,
    clicked: Option<Click>,
}

impl Stage<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.s
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }

    fn font(&self, size: f32) -> FontId {
        FontId::proportional(size * self.s)
    }

    fn text(&self, text: &str, size: f32, color: Color32) -> Arc<Galley> {
        self.painter
            .layout_no_wrap(text.to_owned(), self.font(size), color)
    }

    fn stroke(&self, width: f32, color: Color32) -> Stroke {
        Stroke::new(width * self.s, color)
    }

    fn interact(&mut self, rect: Rect, salt: impl std::hash::Hash + std::fmt::Debug) -> Response {
        let id = self.ui.scope_id().with(("stage", salt));
        self.ui.interact(rect, id, Sense::click())
    }

    fn pressed(&self, b: Button) -> bool {
        self.live.pressed.contains(&b)
    }

    /// Clique esquerdo captura, clique direito deixa livre.
    fn slot_clicks(&mut self, resp: &Response, slot: Slot) {
        if resp.clicked() {
            self.clicked = Some(Click::Map(slot));
        } else if resp.secondary_clicked() {
            self.clicked = Some(Click::Clear(slot));
        }
    }

    fn hover_ring(&self, resp: &Response, center: Pos2, r: f32) {
        if resp.hovered() {
            self.painter
                .circle_stroke(center, (r + 4.0) * self.s, self.stroke(1.5, ACCENT));
        }
    }

    // ------------------------------------------------------------ controle

    fn controller_back(&mut self) {
        let pal = self.pal;
        for (slot, x0, label) in [
            (Slot::Trigger(Axis::LeftTrigger), 310.0, "L2"),
            (Slot::Trigger(Axis::RightTrigger), 604.0, "R2"),
        ] {
            let rect = self.rect(x0, 42.0, x0 + 86.0, 70.0);
            let r = 13.0 * self.s;
            self.painter.rect(
                rect,
                r,
                pal.trigger,
                self.stroke(1.0, pal.trigger_stroke),
                StrokeKind::Inside,
            );
            let Slot::Trigger(axis) = slot else {
                unreachable!()
            };
            let v = self.live.axis(axis).clamp(0.0, 1.0);
            if v > 0.01 {
                let mut fill = rect;
                fill.set_width((rect.width() * v).max(2.0 * r));
                self.painter
                    .rect_filled(fill, r, ACCENT.gamma_multiply(0.6));
            }
            self.painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                self.font(13.0),
                pal.glyph,
            );
            let resp = self.interact(rect, label);
            if resp.hovered() {
                self.painter.rect_stroke(
                    rect.expand(3.0 * self.s),
                    r + 3.0,
                    self.stroke(1.5, ACCENT),
                    StrokeKind::Inside,
                );
            }
            self.slot_clicks(&resp, slot);
        }
        for (b, x0, label) in [
            (Button::LeftShoulder, 308.0, "L1"),
            (Button::RightShoulder, 610.0, "R1"),
        ] {
            let rect = self.rect(x0, 74.0, x0 + 82.0, 92.0);
            let fill = if self.pressed(b) { ACCENT } else { pal.socket };
            self.painter.rect(
                rect,
                9.0 * self.s,
                fill,
                self.stroke(1.0, pal.part_stroke),
                StrokeKind::Inside,
            );
            self.painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                self.font(10.0),
                pal.glyph,
            );
            let resp = self.interact(rect, label);
            if resp.hovered() {
                self.painter.rect_stroke(
                    rect.expand(3.0 * self.s),
                    12.0 * self.s,
                    self.stroke(1.5, ACCENT),
                    StrokeKind::Inside,
                );
            }
            self.slot_clicks(&resp, Slot::Button(b));
        }
        let body = self.rect(300.0, 96.0, 700.0, 294.0);
        self.painter.rect(
            body,
            38.0 * self.s,
            pal.body,
            self.stroke(1.5, pal.body_stroke),
            StrokeKind::Inside,
        );
    }

    fn controller_front(&mut self) {
        self.stick(Stick::Left, 382.0, 162.0);
        self.stick(Stick::Right, 550.0, 232.0);
        self.dpad();

        for (b, x, y, label, color) in [
            (Button::North, 618.0, 138.0, "Y", ACTIVE),
            (
                Button::West,
                588.0,
                168.0,
                "X",
                Color32::from_rgb(0x10, 0xB9, 0x81),
            ),
            (
                Button::East,
                648.0,
                168.0,
                "B",
                Color32::from_rgb(0xF4, 0x3F, 0x5E),
            ),
            (Button::South, 618.0, 198.0, "A", ACCENT),
        ] {
            let c = self.at(x, y);
            let r = 13.0 * self.s;
            let pressed = self.pressed(b);
            let (fill, text) = if pressed {
                (color, FACE_PRESSED_TEXT)
            } else {
                (FACE_BG, color)
            };
            self.painter.circle(c, r, fill, self.stroke(2.0, color));
            self.painter
                .text(c, Align2::CENTER_CENTER, label, self.font(12.0), text);
            let resp = self.interact(Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r)), label);
            self.hover_ring(&resp, c, 13.0);
            self.slot_clicks(&resp, Slot::Button(b));
        }

        for (b, x) in [(Button::Back, 464.0), (Button::Start, 536.0)] {
            let rect = Rect::from_center_size(self.at(x, 168.0), vec2(22.0, 8.0) * self.s);
            let fill = if self.pressed(b) {
                ACCENT
            } else {
                self.pal.socket
            };
            self.painter.rect(
                rect,
                4.0 * self.s,
                fill,
                self.stroke(1.0, self.pal.part_stroke),
                StrokeKind::Inside,
            );
            let resp = self.interact(rect.expand(6.0 * self.s), b);
            if resp.hovered() {
                self.painter.rect_stroke(
                    rect.expand(3.0 * self.s),
                    6.0 * self.s,
                    self.stroke(1.5, ACCENT),
                    StrokeKind::Outside,
                );
            }
            self.slot_clicks(&resp, Slot::Button(b));
        }

        let c = self.at(500.0, 132.0);
        let fill = if self.pressed(Button::Guide) {
            HOME
        } else {
            FACE_BG
        };
        self.painter
            .circle(c, 11.0 * self.s, fill, self.stroke(2.0, HOME));
        let resp = self.interact(Rect::from_center_size(c, vec2(22.0, 22.0) * self.s), "home");
        self.hover_ring(&resp, c, 11.0);
        self.slot_clicks(&resp, Slot::Button(Button::Guide));
    }

    fn stick_engaged(&self, stick: Stick) -> bool {
        !self.live.stick_sectors(self.profile, stick).is_empty()
    }

    fn stick(&mut self, stick: Stick, x: f32, y: f32) {
        let pal = self.pal;
        let (ax, ay) = stick_axes(self.profile, stick);
        let click = stick_click(stick);
        let c = self.at(x, y);
        let ring = if self.stick_engaged(stick) {
            ACTIVE
        } else {
            ACCENT.gamma_multiply(0.85)
        };
        self.painter
            .circle_stroke(c, 34.0 * self.s, self.stroke(2.0, ring));
        self.painter.circle(
            c,
            28.0 * self.s,
            pal.socket,
            self.stroke(1.0, Color32::from_rgb(0x2C, 0x34, 0x50)),
        );
        let cap = c + vec2(self.live.axis(ax), self.live.axis(ay)) * 10.0 * self.s;
        let cap_fill = if self.pressed(click) {
            ACCENT
        } else {
            Color32::from_rgb(0x1C, 0x21, 0x32)
        };
        self.painter.circle(
            cap,
            18.0 * self.s,
            cap_fill,
            self.stroke(1.0, Color32::from_rgb(0x38, 0x40, 0x64)),
        );
        self.painter
            .circle_filled(cap, 4.0 * self.s, Color32::from_rgb(0x0A, 0x0C, 0x14));
        let resp = self
            .interact(Rect::from_center_size(c, vec2(68.0, 68.0) * self.s), stick)
            .on_hover_text(format!(
                "{}: clique para editar os setores",
                stick_title(stick)
            ));
        self.hover_ring(&resp, c, 34.0);
        if resp.clicked() {
            self.clicked = Some(Click::EditStick(stick));
        }
    }

    fn dpad(&mut self) {
        let pal = self.pal;
        let c = self.at(440.0, 232.0);
        self.painter.circle(
            c,
            36.0 * self.s,
            FACE_BG,
            self.stroke(1.0, Color32::from_rgb(0x2C, 0x34, 0x50)),
        );
        let vertical = self.rect(432.0, 204.0, 448.0, 260.0);
        let horizontal = self.rect(412.0, 224.0, 468.0, 240.0);
        for r in [vertical, horizontal] {
            self.painter.rect(
                r,
                2.0 * self.s,
                pal.part,
                self.stroke(1.0, pal.part_stroke),
                StrokeKind::Inside,
            );
        }
        // Cobre a borda do braço vertical que cruza o centro.
        self.painter
            .rect_filled(self.rect(432.5, 224.5, 447.5, 239.5), 0.0, pal.part);

        let arms = [
            (
                Button::DpadUp,
                self.rect(432.0, 204.0, 448.0, 224.0),
                (0.0, -1.0),
            ),
            (
                Button::DpadDown,
                self.rect(432.0, 240.0, 448.0, 260.0),
                (0.0, 1.0),
            ),
            (
                Button::DpadLeft,
                self.rect(412.0, 224.0, 432.0, 240.0),
                (-1.0, 0.0),
            ),
            (
                Button::DpadRight,
                self.rect(448.0, 224.0, 468.0, 240.0),
                (1.0, 0.0),
            ),
        ];
        for (b, rect, (dx, dy)) in arms {
            if self.pressed(b) {
                self.painter
                    .rect_filled(rect.shrink(1.0), 2.0 * self.s, ACCENT);
            }
            // Setinha apontando para fora.
            let tip = rect.center() + vec2(dx, dy) * 4.0 * self.s;
            let base = rect.center() - vec2(dx, dy) * 2.0 * self.s;
            let side = vec2(-dy, dx) * 4.0 * self.s;
            self.painter.add(Shape::convex_polygon(
                vec![tip, base + side, base - side],
                if self.pressed(b) {
                    Color32::WHITE
                } else {
                    ARROW
                },
                Stroke::NONE,
            ));
            let resp = self.interact(rect, b);
            if resp.hovered() {
                self.painter.rect_stroke(
                    rect.expand(1.5 * self.s),
                    3.0 * self.s,
                    self.stroke(1.5, ACCENT),
                    StrokeKind::Outside,
                );
            }
            self.slot_clicks(&resp, Slot::Button(b));
        }
    }

    // ------------------------------------------------------------ etiquetas

    fn lines(&self) {
        for co in CALLOUTS {
            let color = if co.slot.get(self.profile).is_some() {
                self.pal.mapped_line
            } else {
                self.pal.free_line
            };
            let points = co.line.iter().map(|&(x, y)| self.at(x, y)).collect();
            self.painter
                .add(Shape::line(points, self.stroke(1.2, color)));
        }
    }

    fn callout(&mut self, co: &Callout) {
        let pal = self.pal;
        let s = self.s;
        let combo = co.slot.get(self.profile);
        let capturing = self.capturing == Some(co.slot);
        let pressed = self.live.is_active(co.slot, self.profile);

        // Cores: (fundo, borda, texto, fundo do selo, borda do selo, texto do selo)
        let (bg, border, text, chip_bg, chip_border, chip_text) = if capturing {
            (
                pal.active_bg,
                ACTIVE,
                pal.text,
                ACTIVE,
                ACTIVE,
                FACE_PRESSED_TEXT,
            )
        } else if combo.is_none() {
            (
                pal.free_bg,
                pal.free_border,
                pal.free_text,
                pal.chip_bg,
                pal.chip_border,
                pal.chip_text,
            )
        } else if pressed {
            (
                ACCENT_STRONG,
                ACCENT_STRONG,
                Color32::WHITE,
                Color32::WHITE,
                Color32::WHITE,
                ACCENT_STRONG,
            )
        } else {
            (
                pal.mapped_bg,
                pal.mapped_border,
                pal.mapped_text,
                pal.key_bg,
                pal.key_bg,
                pal.key_text,
            )
        };
        let key = if capturing {
            "PRESSIONE…".to_owned()
        } else {
            combo.map_or("LIVRE".to_owned(), |c| c.to_string().to_uppercase())
        };

        let key_g = self.text(&key, 10.0, chip_text);
        let label_g = self.text(co.label, 12.0, text);
        let chip_w = (key_g.size().x + 16.0 * s).max(24.0 * s);
        let (outer, inner, gap, dot) = (5.0 * s, 12.0 * s, 10.0 * s, 3.0 * s);
        let w = outer + chip_w + gap + dot + gap + label_g.size().x + inner;
        let h = 32.0 * s;

        let (rect, key_first) = match co.side {
            Side::Left => (
                Rect::from_min_size(self.at(240.0, co.y - 16.0) - vec2(w, 0.0), vec2(w, h)),
                true,
            ),
            Side::Right => (
                Rect::from_min_size(self.at(760.0, co.y - 16.0), vec2(w, h)),
                false,
            ),
            Side::Top { x, key_first } => (
                Rect::from_center_size(self.at(x, co.y), vec2(w, h)),
                key_first,
            ),
        };

        let resp = self
            .interact(rect, ("callout", co.label))
            .on_hover_text("Clique para trocar a tecla.\nBotão direito deixa livre.");
        let r = 8.0 * s;
        let border_w = if resp.hovered() { 2.0 } else { 1.0 };
        let border = if resp.hovered() && !capturing && !pressed {
            ACCENT
        } else {
            border
        };
        self.painter.rect(
            rect,
            r,
            bg,
            self.stroke(border_w, border),
            StrokeKind::Inside,
        );

        // Posições do selo, do ponto e do nome, da esquerda para a direita.
        let cy = rect.center().y;
        let (chip_x, dot_x, label_x) = if key_first {
            let chip_x = rect.left() + outer;
            let dot_x = chip_x + chip_w + gap + dot / 2.0;
            (chip_x, dot_x, dot_x + dot / 2.0 + gap)
        } else {
            let label_x = rect.left() + inner;
            let dot_x = label_x + label_g.size().x + gap + dot / 2.0;
            (dot_x + dot / 2.0 + gap, dot_x, label_x)
        };
        let chip = Rect::from_min_size(Pos2::new(chip_x, cy - 11.0 * s), vec2(chip_w, 22.0 * s));
        self.painter.rect(
            chip,
            5.0 * s,
            chip_bg,
            self.stroke(1.0, chip_border),
            StrokeKind::Inside,
        );
        self.painter
            .galley(chip.center() - key_g.size() / 2.0, key_g, chip_text);
        self.painter
            .circle_filled(Pos2::new(dot_x, cy), dot / 2.0, text.gamma_multiply(0.5));
        self.painter.galley(
            Pos2::new(label_x, cy - label_g.size().y / 2.0),
            label_g,
            text,
        );

        self.slot_clicks(&resp, co.slot);
    }

    // ------------------------------------------------------------ sticks embaixo

    fn stick_panel(&mut self, stick: Stick, cx: f32) {
        let pal = self.pal;
        let s = self.s;
        let (ax, ay) = stick_axes(self.profile, stick);
        let binding = self.profile.sticks.get(&stick);
        let sectors = self.live.stick_sectors(self.profile, stick);
        let active: Vec<String> = sectors
            .iter()
            .filter_map(|&i| binding?.sectors[i].key.as_ref())
            .map(|c| c.to_string().to_uppercase())
            .collect();
        let engaged = !active.is_empty();
        let any_mapped = binding.is_some_and(|b| b.sectors.iter().any(|s| s.key.is_some()));

        // Selo de cima: tecla acionada agora, ou INATIVO / LIVRE.
        let (badge, bg, border, color) = if engaged {
            (active.join(" + "), pal.active_bg, ACTIVE, pal.text)
        } else if any_mapped {
            (
                "INATIVO".to_owned(),
                pal.chip_bg,
                pal.chip_border,
                pal.chip_text,
            )
        } else {
            (
                "LIVRE".to_owned(),
                pal.chip_bg,
                pal.chip_border,
                pal.chip_text,
            )
        };
        let g = self.text(&badge, 10.0, color);
        let rect =
            Rect::from_center_size(self.at(cx, 409.0), vec2(g.size().x + 16.0 * s, 22.0 * s));
        self.painter.rect(
            rect,
            5.0 * s,
            bg,
            self.stroke(1.5, border),
            StrokeKind::Inside,
        );
        self.painter
            .galley(rect.center() - g.size() / 2.0, g, color);

        // Círculo com os setores do perfil. 44 → 61.6 px: mesma escala do protótipo (100 → 140).
        let c = self.at(cx, 500.0);
        let reach = 61.6 * s;
        let ring = if engaged { ACTIVE } else { pal.stick_ring };
        self.painter
            .circle(c, 64.4 * s, pal.stick_bg, self.stroke(3.5, ring));

        let (x, y) = (self.live.axis(ax), self.live.axis(ay));
        if let Some(b) = binding {
            for &i in &sectors {
                wedge(
                    &self.painter,
                    c,
                    reach,
                    b.sectors[i].start,
                    b.width(i),
                    Color32::from_rgba_unmultiplied(0xEA, 0xB3, 0x08, 115),
                );
            }
            if b.sectors.len() > 1 {
                for sector in &b.sectors {
                    self.painter.line_segment(
                        [c, polar(c, reach, sector.start)],
                        self.stroke(1.4, pal.stick_spoke),
                    );
                }
            }
        }
        // Anel tracejado no ponto de acionamento.
        let t = self.profile.deadzone(stick) * reach;
        let n = 48;
        for i in (0..n).step_by(2) {
            let a0 = TAU * i as f32 / n as f32;
            let a1 = TAU * (i + 1) as f32 / n as f32;
            self.painter.line_segment(
                [
                    c + vec2(a0.cos(), a0.sin()) * t,
                    c + vec2(a1.cos(), a1.sin()) * t,
                ],
                self.stroke(1.4, ACCENT),
            );
        }
        let mut knob = vec2(x, y) * reach;
        if knob.length() > reach {
            knob = knob.normalized() * reach;
        }
        let knob_fill = if engaged { ACTIVE } else { KNOB_IDLE };
        self.painter.circle(
            c + knob,
            (if engaged { 8.4 } else { 7.0 }) * s,
            knob_fill,
            self.stroke(2.8, pal.stick_bg),
        );

        let resp = self
            .interact(
                Rect::from_center_size(c, vec2(130.0, 130.0) * s),
                ("panel", stick),
            )
            .on_hover_text(format!(
                "{}: clique para editar os setores",
                stick_title(stick)
            ));
        if resp.hovered() {
            self.painter
                .circle_stroke(c, 68.0 * s, self.stroke(1.5, ACCENT));
        }
        if resp.clicked() {
            self.clicked = Some(Click::EditStick(stick));
        }

        // Botão LS / RS e a legenda.
        let g = self.text(
            stick_short(stick),
            11.0,
            if engaged { pal.text } else { Color32::WHITE },
        );
        let rect =
            Rect::from_center_size(self.at(cx, 593.0), vec2(g.size().x + 24.0 * s, 26.0 * s));
        let resp = self.interact(rect, ("button", stick));
        let (bg, border) = if engaged {
            (pal.active_bg, ACTIVE)
        } else {
            (ACCENT, ACCENT)
        };
        let bg = if resp.hovered() && !engaged {
            ACCENT_STRONG
        } else {
            bg
        };
        self.painter.rect(
            rect,
            6.0 * s,
            bg,
            self.stroke(1.5, border),
            StrokeKind::Inside,
        );
        self.painter
            .galley(rect.center() - g.size() / 2.0, g, Color32::WHITE);
        if resp.clicked() {
            self.clicked = Some(Click::EditStick(stick));
        }
        self.painter.text(
            self.at(cx, 623.0),
            Align2::CENTER_CENTER,
            "CLIQUE PARA EDITAR",
            self.font(10.0),
            pal.text_muted,
        );
    }
}

/// Desenha a tela e retorna o que foi clicado, se algo foi.
pub fn show(
    ui: &mut Ui,
    pal: &Palette,
    live: &Live,
    profile: &Profile,
    capturing: Option<Slot>,
) -> Option<Click> {
    let avail = ui.available_rect_before_wrap();
    ui.allocate_rect(avail, Sense::hover());
    let s = (avail.width() / WIDTH)
        .min(avail.height() / HEIGHT)
        .clamp(0.5, 1.5);
    let size = vec2(WIDTH, HEIGHT) * s;
    // Centraliza; o desenho começa em y = -10 por causa das etiquetas de cima.
    let origin = avail.center() - size / 2.0 + vec2(0.0, 10.0 * s);

    let mut st = Stage {
        painter: ui.painter_at(avail),
        ui,
        origin,
        s,
        pal,
        live,
        profile,
        capturing,
        clicked: None,
    };
    st.controller_back();
    st.lines();
    st.controller_front();
    for co in CALLOUTS {
        st.callout(co);
    }
    st.stick_panel(Stick::Left, 370.0);
    st.stick_panel(Stick::Right, 630.0);
    st.clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_callout_has_its_own_slot() {
        let mut seen = HashSet::new();
        for co in CALLOUTS {
            assert!(seen.insert(co.slot), "{:?} repetido", co.slot);
        }
    }

    #[test]
    fn per_axis_threshold_wins() {
        let p = Profile::from_toml(
            "name='t'\n[settings]\naxis_threshold=0.5\n[axes.left_trigger]\npositive='LAlt'\nthreshold=0.3\n",
        )
        .unwrap();
        assert_eq!(threshold(&p, Axis::LeftTrigger), 0.3);
        assert_eq!(threshold(&p, Axis::LeftX), 0.5);
        let mut live = Live::default();
        live.axes.insert(Axis::LeftTrigger, 0.35);
        assert!(live.is_active(Slot::Trigger(Axis::LeftTrigger), &p));
    }
}
