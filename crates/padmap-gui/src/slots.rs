//! O que pode receber uma tecla: um botão ou gatilho da tela principal, ou um
//! botão extra da aba "Botões extras". Os sticks são mapeados por setores, no
//! editor de stick.

use padmap_core::{Axis, Button, ExtraSource, KeyCombo, Profile, Stick};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Button(Button),
    Trigger(Axis),
    /// Um botão da lista de extras; ele precisa estar em `Profile::extras`.
    Extra(ExtraSource),
}

/// Botões com nome no SDL que o desenho do controle não mostra. Eles só
/// podem ser mapeados como extras.
pub const OFF_DIAGRAM: [Button; 6] = [
    Button::Misc1,
    Button::LeftPaddle1,
    Button::LeftPaddle2,
    Button::RightPaddle1,
    Button::RightPaddle2,
    Button::Touchpad,
];

/// Nome que um botão extra recebe ao entrar na lista.
pub fn extra_default_name(source: ExtraSource) -> String {
    match source {
        ExtraSource::Known(b) => button_label(b).to_owned(),
        // Para gente, a contagem começa em 1.
        ExtraSource::Other(n) => format!("Botão {}", n as u16 + 1),
    }
}

/// Origem do botão extra, para o selo da lista.
pub fn extra_origin(source: ExtraSource) -> String {
    match source {
        ExtraSource::Known(_) => "botão conhecido".to_owned(),
        ExtraSource::Other(n) => format!("botão {} do controle", n as u16 + 1),
    }
}

impl Slot {
    pub fn get(self, p: &Profile) -> Option<&KeyCombo> {
        match self {
            Slot::Button(b) => p.buttons.get(&b),
            Slot::Trigger(a) => p.axes.get(&a)?.positive.as_ref(),
            Slot::Extra(src) => p.extras.iter().find(|e| e.button == src)?.key.as_ref(),
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
            Slot::Trigger(a) => {
                let binding = p.axes.entry(a).or_default();
                binding.positive = combo;
                if binding.negative.is_none() && binding.positive.is_none() {
                    p.axes.remove(&a);
                }
            }
            Slot::Extra(src) => {
                if let Some(e) = p.extras.iter_mut().find(|e| e.button == src) {
                    e.key = combo;
                }
            }
        }
    }

    /// Nome para títulos e para o log. Botões extras usam o nome dado pelo usuário.
    pub fn title(self, p: &Profile) -> String {
        match self {
            Slot::Button(b) => button_label(b).to_owned(),
            Slot::Trigger(Axis::LeftTrigger) => "Gatilho esquerdo (LT / L2)".to_owned(),
            Slot::Trigger(_) => "Gatilho direito (RT / R2)".to_owned(),
            Slot::Extra(src) => p
                .extras
                .iter()
                .find(|e| e.button == src)
                .map(|e| e.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| extra_default_name(src)),
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

pub fn stick_title(stick: Stick) -> &'static str {
    match stick {
        Stick::Left => "Stick esquerdo",
        Stick::Right => "Stick direito",
    }
}

pub fn stick_short(stick: Stick) -> &'static str {
    match stick {
        Stick::Left => "LS",
        Stick::Right => "RS",
    }
}

/// Botão do clique do stick (L3 / R3).
pub fn stick_click(stick: Stick) -> Button {
    match stick {
        Stick::Left => Button::LeftStick,
        Stick::Right => Button::RightStick,
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
            Slot::Trigger(Axis::LeftTrigger),
            Slot::Trigger(Axis::RightTrigger),
        ],
    ),
    (
        "Sticks",
        &[
            Slot::Button(Button::LeftStick),
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
    fn set_and_clear_trigger() {
        let mut p = Profile::from_toml("name='t'").unwrap();
        Slot::Trigger(Axis::LeftTrigger).set(&mut p, Some(Key::D.into()));
        assert_eq!(
            Slot::Trigger(Axis::LeftTrigger).get(&p),
            Some(&Key::D.into())
        );
        Slot::Trigger(Axis::LeftTrigger).set(&mut p, None);
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
