//! Vocabulário de entrada de controle, independente do backend.
//!
//! Os nomes seguem o layout posicional do SDL ("south" é A no Xbox, X no
//! PlayStation, B no Nintendo), de modo que um mesmo perfil vale para qualquer
//! controle reconhecido.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Misc1,
    LeftPaddle1,
    LeftPaddle2,
    RightPaddle1,
    RightPaddle2,
    Touchpad,
}

impl Button {
    pub const ALL: [Button; 21] = [
        Button::South,
        Button::East,
        Button::West,
        Button::North,
        Button::Back,
        Button::Guide,
        Button::Start,
        Button::LeftStick,
        Button::RightStick,
        Button::LeftShoulder,
        Button::RightShoulder,
        Button::DpadUp,
        Button::DpadDown,
        Button::DpadLeft,
        Button::DpadRight,
        Button::Misc1,
        Button::LeftPaddle1,
        Button::LeftPaddle2,
        Button::RightPaddle1,
        Button::RightPaddle2,
        Button::Touchpad,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Button::South => "south",
            Button::East => "east",
            Button::West => "west",
            Button::North => "north",
            Button::Back => "back",
            Button::Guide => "guide",
            Button::Start => "start",
            Button::LeftStick => "left_stick",
            Button::RightStick => "right_stick",
            Button::LeftShoulder => "left_shoulder",
            Button::RightShoulder => "right_shoulder",
            Button::DpadUp => "dpad_up",
            Button::DpadDown => "dpad_down",
            Button::DpadLeft => "dpad_left",
            Button::DpadRight => "dpad_right",
            Button::Misc1 => "misc1",
            Button::LeftPaddle1 => "left_paddle1",
            Button::LeftPaddle2 => "left_paddle2",
            Button::RightPaddle1 => "right_paddle1",
            Button::RightPaddle2 => "right_paddle2",
            Button::Touchpad => "touchpad",
        }
    }
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl FromStr for Button {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Button::ALL
            .into_iter()
            .find(|b| b.name() == s)
            .ok_or_else(|| format!("botão desconhecido: {s}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

impl Axis {
    pub fn name(self) -> &'static str {
        match self {
            Axis::LeftX => "left_x",
            Axis::LeftY => "left_y",
            Axis::RightX => "right_x",
            Axis::RightY => "right_y",
            Axis::LeftTrigger => "left_trigger",
            Axis::RightTrigger => "right_trigger",
        }
    }
}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// Evento normalizado vindo de qualquer backend de entrada.
///
/// Eixos são normalizados para `-1.0..=1.0` (sticks; Y positivo é para baixo,
/// como no SDL) ou `0.0..=1.0` (gatilhos).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    ButtonDown(Button),
    ButtonUp(Button),
    AxisMotion(Axis, f32),
}
