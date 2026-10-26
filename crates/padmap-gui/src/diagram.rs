//! Desenho do controle, atualizado ao vivo. Clicar numa parte do desenho
//! devolve o `Slot` correspondente para iniciar a captura de tecla.

use crate::slots::Slot;
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
};
use padmap_core::{Axis, Button, Profile};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Live {
    pub pressed: HashSet<Button>,
    pub axes: HashMap<Axis, f32>,
}

impl Live {
    pub fn axis(&self, a: Axis) -> f32 {
        self.axes.get(&a).copied().unwrap_or(0.0)
    }

    pub fn is_active(&self, slot: Slot, threshold: f32) -> bool {
        match slot {
            Slot::Button(b) => self.pressed.contains(&b),
            Slot::AxisNeg(a) => -self.axis(a) >= threshold,
            Slot::AxisPos(a) => self.axis(a) >= threshold,
        }
    }
}

const SIZE: egui::Vec2 = vec2(420.0, 222.0);

struct Painter<'a> {
    ui: &'a mut Ui,
    origin: Pos2,
    live: &'a Live,
    profile: &'a Profile,
    clicked: Option<Slot>,
    body: Color32,
    part: Color32,
    on: Color32,
    text: Color32,
}

impl Painter<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y)
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }

    fn tooltip(&self, slots: &[Slot]) -> String {
        slots
            .iter()
            .map(|s| {
                let combo = s
                    .get(self.profile)
                    .map_or("sem tecla".to_string(), |c| c.to_string());
                format!("{}: {combo}", s.label())
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n\nClique para mapear"
    }

    fn hit(&mut self, rect: Rect, slot: Slot, extra: &[Slot]) {
        let id = self.ui.scope_id().with(("diagram", format!("{slot:?}")));
        let mut all = vec![slot];
        all.extend_from_slice(extra);
        let resp = self
            .ui
            .interact(rect, id, Sense::click())
            .on_hover_text(self.tooltip(&all));
        if resp.hovered() {
            self.ui.painter().rect_stroke(
                rect.expand(2.0),
                4.0,
                Stroke::new(1.5, self.on),
                StrokeKind::Outside,
            );
        }
        if resp.clicked() {
            self.clicked = Some(slot);
        }
    }

    fn button_round(&mut self, b: Button, x: f32, y: f32, r: f32, label: &str) {
        let c = self.at(x, y);
        let fill = if self.live.pressed.contains(&b) {
            self.on
        } else {
            self.part
        };
        self.ui.painter().circle_filled(c, r, fill);
        self.ui.painter().text(
            c,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(r * 0.9),
            self.text,
        );
        self.hit(
            Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)),
            Slot::Button(b),
            &[],
        );
    }

    fn button_rect(&mut self, b: Button, rect: Rect, label: &str) {
        let fill = if self.live.pressed.contains(&b) {
            self.on
        } else {
            self.part
        };
        self.ui.painter().rect_filled(rect, 4.0, fill);
        if !label.is_empty() {
            self.ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                self.text,
            );
        }
        self.hit(rect, Slot::Button(b), &[]);
    }

    fn trigger(&mut self, a: Axis, rect: Rect, label: &str) {
        let v = self.live.axis(a).clamp(0.0, 1.0);
        self.ui.painter().rect_filled(rect, 4.0, self.part);
        let mut fill = rect;
        fill.set_width(rect.width() * v);
        self.ui.painter().rect_filled(fill, 4.0, self.on);
        self.ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            self.text,
        );
        self.hit(rect, Slot::AxisPos(a), &[]);
    }

    fn stick(&mut self, x: f32, y: f32, ax: Axis, ay: Axis, click: Button) {
        let c = self.at(x, y);
        let r = 30.0;
        let fill = if self.live.pressed.contains(&click) {
            self.on
        } else {
            self.part
        };
        self.ui.painter().circle_filled(c, r, fill);
        self.ui
            .painter()
            .circle_stroke(c, r * 0.5, Stroke::new(1.0, self.body));
        let dot = c + vec2(self.live.axis(ax), self.live.axis(ay)) * (r - 8.0);
        self.ui.painter().circle_filled(dot, 8.0, self.text);
        self.hit(
            Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)),
            Slot::Button(click),
            &[
                Slot::AxisNeg(ay),
                Slot::AxisPos(ay),
                Slot::AxisNeg(ax),
                Slot::AxisPos(ax),
            ],
        );
    }
}

/// Desenha o controle e retorna a parte clicada, se houver.
pub fn show(ui: &mut Ui, live: &Live, profile: &Profile) -> Option<Slot> {
    let (rect, _) = ui.allocate_exact_size(SIZE, Sense::hover());
    let dark = ui.visuals().dark_mode;
    let mut p = Painter {
        origin: rect.min,
        live,
        profile,
        clicked: None,
        body: if dark {
            Color32::from_gray(45)
        } else {
            Color32::from_gray(215)
        },
        part: if dark {
            Color32::from_gray(75)
        } else {
            Color32::from_gray(245)
        },
        on: Color32::from_rgb(64, 160, 255),
        text: if dark {
            Color32::from_gray(230)
        } else {
            Color32::from_gray(30)
        },
        ui,
    };

    let body = p.rect(30.0, 44.0, 390.0, 215.0);
    p.ui.painter().rect_filled(body, 70.0, p.body);

    p.trigger(Axis::LeftTrigger, p.rect(60.0, 4.0, 160.0, 20.0), "LT · L2");
    p.trigger(
        Axis::RightTrigger,
        p.rect(260.0, 4.0, 360.0, 20.0),
        "RT · R2",
    );
    p.button_rect(
        Button::LeftShoulder,
        p.rect(60.0, 24.0, 160.0, 40.0),
        "LB · L1",
    );
    p.button_rect(
        Button::RightShoulder,
        p.rect(260.0, 24.0, 360.0, 40.0),
        "RB · R1",
    );

    p.stick(105.0, 100.0, Axis::LeftX, Axis::LeftY, Button::LeftStick);
    p.stick(270.0, 165.0, Axis::RightX, Axis::RightY, Button::RightStick);

    p.button_rect(Button::DpadUp, p.rect(142.0, 133.0, 160.0, 151.0), "↑");
    p.button_rect(Button::DpadDown, p.rect(142.0, 179.0, 160.0, 197.0), "↓");
    p.button_rect(Button::DpadLeft, p.rect(119.0, 156.0, 137.0, 174.0), "←");
    p.button_rect(Button::DpadRight, p.rect(165.0, 156.0, 183.0, 174.0), "→");

    p.button_round(Button::North, 320.0, 72.0, 13.0, "Y");
    p.button_round(Button::South, 320.0, 128.0, 13.0, "A");
    p.button_round(Button::West, 292.0, 100.0, 13.0, "X");
    p.button_round(Button::East, 348.0, 100.0, 13.0, "B");

    p.button_round(Button::Back, 175.0, 92.0, 9.0, "");
    p.button_round(Button::Guide, 210.0, 70.0, 12.0, "");
    p.button_round(Button::Start, 245.0, 92.0, 9.0, "");
    p.button_rect(Button::Touchpad, p.rect(180.0, 108.0, 240.0, 126.0), "");

    p.clicked
}
