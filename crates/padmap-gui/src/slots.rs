//! O que pode receber uma tecla no perfil: um botão ou um sentido de um eixo.

use padmap_core::{Axis, Button, KeyCombo, Profile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Button(Button),
    AxisNeg(Axis),
    AxisPos(Axis),
}

impl Slot {
    pub fn get(self, p: &Profile) -> Option<&KeyCombo> {
        match self {
            Slot::Button(b) => p.buttons.get(&b),
            Slot::AxisNeg(a) => p.axes.get(&a)?.negative.as_ref(),
            Slot::AxisPos(a) => p.axes.get(&a)?.positive.as_ref(),
        }
    }

    pub fn set(self, p: &mut Profile, combo: Option<KeyCombo>) {
        match self {
            Slot::Button(b) => match combo {
                Some(c) => {
                    p.buttons.insert(b, c);
                }
                None => {
                    p.buttons.remove(&b);
                }
            },
            Slot::AxisNeg(a) | Slot::AxisPos(a) => {
                let binding = p.axes.entry(a).or_default();
                if matches!(self, Slot::AxisNeg(_)) {
                    binding.negative = combo;
                } else {
                    binding.positive = combo;
                }
                if binding.negative.is_none() && binding.positive.is_none() {
                    p.axes.remove(&a);
                }
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Slot::Button(b) => button_label(b),
            Slot::AxisNeg(a) => match a {
                Axis::LeftX => "Stick esquerdo ←",
                Axis::LeftY => "Stick esquerdo ↑",
                Axis::RightX => "Stick direito ←",
                Axis::RightY => "Stick direito ↑",
                Axis::LeftTrigger | Axis::RightTrigger => "(não usado)",
            },
            Slot::AxisPos(a) => match a {
                Axis::LeftX => "Stick esquerdo →",
                Axis::LeftY => "Stick esquerdo ↓",
                Axis::RightX => "Stick direito →",
                Axis::RightY => "Stick direito ↓",
                Axis::LeftTrigger => "Gatilho esquerdo (LT / L2)",
                Axis::RightTrigger => "Gatilho direito (RT / R2)",
            },
        }
    }
}

pub fn button_label(b: Button) -> &'static str {
    match b {
        Button::South => "A · Cruz",
        Button::East => "B · Círculo",
        Button::West => "X · Quadrado",
        Button::North => "Y · Triângulo",
        Button::Back => "Back · Share",
        Button::Guide => "Guia (Xbox / PS)",
        Button::Start => "Start · Options",
        Button::LeftStick => "Clique stick esq. (L3)",
        Button::RightStick => "Clique stick dir. (R3)",
        Button::LeftShoulder => "LB · L1",
        Button::RightShoulder => "RB · R1",
        Button::DpadUp => "Direcional ↑",
        Button::DpadDown => "Direcional ↓",
        Button::DpadLeft => "Direcional ←",
        Button::DpadRight => "Direcional →",
        Button::Misc1 => "Extra (Share Xbox, Mic PS5)",
        Button::LeftPaddle1 => "Paddle esquerdo 1",
        Button::LeftPaddle2 => "Paddle esquerdo 2",
        Button::RightPaddle1 => "Paddle direito 1",
        Button::RightPaddle2 => "Paddle direito 2",
        Button::Touchpad => "Touchpad (PS)",
    }
}

/// Todos os botões e eixos, agrupados. Hoje só os testes usam a lista.
#[cfg(test)]
pub const GROUPS: &[(&str, &[Slot])] = &[
    (
        "Botões de ação",
        &[
            Slot::Button(Button::South),
            Slot::Button(Button::East),
            Slot::Button(Button::West),
            Slot::Button(Button::North),
        ],
    ),
    (
        "Direcional",
        &[
            Slot::Button(Button::DpadUp),
            Slot::Button(Button::DpadDown),
            Slot::Button(Button::DpadLeft),
            Slot::Button(Button::DpadRight),
        ],
    ),
    (
        "Ombros e gatilhos",
        &[
            Slot::Button(Button::LeftShoulder),
            Slot::Button(Button::RightShoulder),
            Slot::AxisPos(Axis::LeftTrigger),
            Slot::AxisPos(Axis::RightTrigger),
        ],
    ),
    (
        "Sticks",
        &[
            Slot::AxisNeg(Axis::LeftY),
            Slot::AxisPos(Axis::LeftY),
            Slot::AxisNeg(Axis::LeftX),
            Slot::AxisPos(Axis::LeftX),
            Slot::Button(Button::LeftStick),
            Slot::AxisNeg(Axis::RightY),
            Slot::AxisPos(Axis::RightY),
            Slot::AxisNeg(Axis::RightX),
            Slot::AxisPos(Axis::RightX),
            Slot::Button(Button::RightStick),
        ],
    ),
    (
        "Centro",
        &[
            Slot::Button(Button::Back),
            Slot::Button(Button::Guide),
            Slot::Button(Button::Start),
            Slot::Button(Button::Misc1),
            Slot::Button(Button::Touchpad),
        ],
    ),
    (
        "Paddles (Elite, Edge, Steam)",
        &[
            Slot::Button(Button::LeftPaddle1),
            Slot::Button(Button::LeftPaddle2),
            Slot::Button(Button::RightPaddle1),
            Slot::Button(Button::RightPaddle2),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use padmap_core::Key;

    #[test]
    fn set_and_clear_axis_direction() {
        let mut p = Profile::from_toml("name='t'").unwrap();
        Slot::AxisPos(Axis::LeftX).set(&mut p, Some(Key::D.into()));
        assert_eq!(Slot::AxisPos(Axis::LeftX).get(&p), Some(&Key::D.into()));
        Slot::AxisPos(Axis::LeftX).set(&mut p, None);
        assert!(p.axes.is_empty());
    }

    #[test]
    fn every_button_appears_once() {
        for b in Button::ALL {
            let n = GROUPS
                .iter()
                .flat_map(|(_, s)| s.iter())
                .filter(|s| **s == Slot::Button(b))
                .count();
            assert_eq!(n, 1, "{b}");
        }
    }
}
