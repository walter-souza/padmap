//! Leitura de controles via SDL3.
//!
//! O SDL traz a base de dados de mapeamentos da comunidade (gamecontrollerdb)
//! e drivers HIDAPI próprios, então controles de Xbox, PlayStation (DS4/DualSense),
//! Switch e a maioria dos genéricos aparecem com o mesmo layout posicional.

use padmap_core::{Axis, Button, InputEvent};
use sdl3::event::Event;
use sdl3::gamepad::{self, Gamepad};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum SourceEvent {
    Connected {
        id: u32,
        name: String,
    },
    Disconnected {
        id: u32,
    },
    Input {
        id: u32,
        event: InputEvent,
    },
    /// O SDL recebeu um pedido de saída (ex.: Ctrl+C no console).
    Quit,
}

pub struct GamepadSource {
    // Mantém o SDL vivo enquanto a fonte existir.
    _sdl: sdl3::Sdl,
    subsystem: sdl3::GamepadSubsystem,
    pump: sdl3::EventPump,
    open: HashMap<u32, Gamepad>,
}

impl GamepadSource {
    pub fn new() -> Result<Self, String> {
        // Sem isso o SDL só entrega eventos quando a janela do app tem foco, e
        // o padmap não tem janela: o foco está sempre no jogo.
        sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        let sdl = sdl3::init().map_err(|e| e.to_string())?;
        let subsystem = sdl.gamepad().map_err(|e| e.to_string())?;
        let pump = sdl.event_pump().map_err(|e| e.to_string())?;
        Ok(GamepadSource {
            _sdl: sdl,
            subsystem,
            pump,
            open: HashMap::new(),
        })
    }

    /// Controles conectados agora, como `(id, nome)`.
    pub fn connected(&self) -> Vec<(u32, String)> {
        self.subsystem
            .gamepads()
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                let name = self
                    .subsystem
                    .name_for_id(id)
                    .unwrap_or_else(|_| "controle desconhecido".into());
                (id.raw(), name)
            })
            .collect()
    }

    /// Espera até `timeout` pelo próximo evento relevante.
    pub fn next_event(&mut self, timeout: Duration) -> Option<SourceEvent> {
        let event = self.pump.wait_event_timeout(timeout)?;
        self.translate(event)
    }

    fn translate(&mut self, event: Event) -> Option<SourceEvent> {
        match event {
            Event::Quit { .. } => Some(SourceEvent::Quit),
            Event::GamepadAdded { which, .. } => {
                let pad = self
                    .subsystem
                    .open(which)
                    .map_err(|e| {
                        log::warn!("não foi possível abrir o controle {}: {e}", which.raw())
                    })
                    .ok()?;
                let name = pad.name().unwrap_or_else(|| "controle desconhecido".into());
                self.open.insert(which.raw(), pad);
                Some(SourceEvent::Connected {
                    id: which.raw(),
                    name,
                })
            }
            Event::GamepadRemoved { which, .. } => {
                self.open.remove(&which.raw());
                Some(SourceEvent::Disconnected { id: which.raw() })
            }
            Event::GamepadButtonDown { which, button, .. } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::ButtonDown(map_button(button)?),
            }),
            Event::GamepadButtonUp { which, button, .. } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::ButtonUp(map_button(button)?),
            }),
            Event::GamepadAxisMotion {
                which, axis, value, ..
            } => {
                let axis = map_axis(axis);
                Some(SourceEvent::Input {
                    id: which.raw(),
                    event: InputEvent::AxisMotion(axis, normalize(value)),
                })
            }
            _ => None,
        }
    }
}

fn normalize(value: i16) -> f32 {
    (value as f32 / i16::MAX as f32).clamp(-1.0, 1.0)
}

fn map_button(b: gamepad::Button) -> Option<Button> {
    use gamepad::Button as S;
    Some(match b {
        S::South => Button::South,
        S::East => Button::East,
        S::West => Button::West,
        S::North => Button::North,
        S::Back => Button::Back,
        S::Guide => Button::Guide,
        S::Start => Button::Start,
        S::LeftStick => Button::LeftStick,
        S::RightStick => Button::RightStick,
        S::LeftShoulder => Button::LeftShoulder,
        S::RightShoulder => Button::RightShoulder,
        S::DPadUp => Button::DpadUp,
        S::DPadDown => Button::DpadDown,
        S::DPadLeft => Button::DpadLeft,
        S::DPadRight => Button::DpadRight,
        S::Misc1 => Button::Misc1,
        S::LeftPaddle1 => Button::LeftPaddle1,
        S::LeftPaddle2 => Button::LeftPaddle2,
        S::RightPaddle1 => Button::RightPaddle1,
        S::RightPaddle2 => Button::RightPaddle2,
        S::Touchpad => Button::Touchpad,
        _ => return None,
    })
}

fn map_axis(a: gamepad::Axis) -> Axis {
    use gamepad::Axis as S;
    match a {
        S::LeftX => Axis::LeftX,
        S::LeftY => Axis::LeftY,
        S::RightX => Axis::RightX,
        S::RightY => Axis::RightY,
        S::TriggerLeft => Axis::LeftTrigger,
        S::TriggerRight => Axis::RightTrigger,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn normalizes_full_range() {
        assert_eq!(normalize(i16::MAX), 1.0);
        assert_eq!(normalize(i16::MIN), -1.0);
        assert_eq!(normalize(0), 0.0);
    }
}
