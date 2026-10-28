//! Leitura de controles via SDL3.
//!
//! O SDL traz a base de dados de mapeamentos da comunidade (gamecontrollerdb)
//! e drivers HIDAPI próprios, então controles de Xbox, PlayStation (DS4/DualSense),
//! Switch e a maioria dos genéricos aparecem com o mesmo layout posicional.
//!
//! Botões que o mapeamento do controle não cobre (traseiros, turbo, modo…)
//! chegam como botões crus do joystick e viram `InputEvent::OtherButton*`.

use padmap_core::{Axis, Button, InputEvent};
use sdl3::event::Event;
use sdl3::gamepad::{self, Gamepad};
use std::collections::{HashMap, HashSet};
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
    open: HashMap<u32, OpenPad>,
}

struct OpenPad {
    // Mantém o controle aberto enquanto estiver conectado.
    _pad: Gamepad,
    /// Botões do joystick que o mapeamento do controle já transforma em botões
    /// com nome. `None` se não deu para ler o mapeamento: aí nenhum botão cru é
    /// repassado, para nenhum botão acionar duas vezes.
    mapped: Option<HashSet<u8>>,
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
                let mapped = mapped_buttons(&pad);
                log_buttons(&pad, &name, mapped.as_ref());
                if pad.touchpads_count() > 0 {
                    log::info!(
                        "{name}: touchpad com até {} dedos",
                        pad.supported_touchpad_fingers(0)
                    );
                }
                self.open.insert(which.raw(), OpenPad { _pad: pad, mapped });
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
            Event::JoyButtonDown {
                which, button_idx, ..
            } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::OtherButtonDown(self.unmapped(which.raw(), button_idx)?),
            }),
            Event::JoyButtonUp {
                which, button_idx, ..
            } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::OtherButtonUp(self.unmapped(which.raw(), button_idx)?),
            }),
            // Só o primeiro touchpad: os controles conhecidos têm um.
            Event::GamepadTouchpadDown {
                which,
                touchpad: 0,
                finger,
                x,
                y,
                ..
            } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::TouchDown(finger as u8, x, y),
            }),
            Event::GamepadTouchpadMotion {
                which,
                touchpad: 0,
                finger,
                x,
                y,
                ..
            } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::TouchMove(finger as u8, x, y),
            }),
            Event::GamepadTouchpadUp {
                which,
                touchpad: 0,
                finger,
                ..
            } => Some(SourceEvent::Input {
                id: which.raw(),
                event: InputEvent::TouchUp(finger as u8),
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

impl GamepadSource {
    /// `button` se ele for um botão cru de um controle aberto que o mapeamento
    /// do controle não cobre; os outros já chegam como botões com nome.
    fn unmapped(&self, id: u32, button: u8) -> Option<u8> {
        let mapped = self.open.get(&id)?.mapped.as_ref()?;
        (!mapped.contains(&button)).then_some(button)
    }
}

/// Registra quantos botões o controle informa e quais o mapeamento cobre:
/// os que sobram são os que a aba de botões extras pode usar.
fn log_buttons(pad: &Gamepad, name: &str, mapped: Option<&HashSet<u8>>) {
    use sdl3_sys::gamepad::SDL_GetGamepadJoystick;
    use sdl3_sys::joystick::SDL_GetNumJoystickButtons;
    // SAFETY: `pad.raw()` é um gamepad aberto; o joystick dele vive junto.
    let total = unsafe { SDL_GetNumJoystickButtons(SDL_GetGamepadJoystick(pad.raw())) };
    let mut mapped: Vec<u8> = mapped.into_iter().flatten().copied().collect();
    mapped.sort_unstable();
    log::info!("{name}: {total} botões no joystick; mapeados: {mapped:?}");
}

/// Botões do joystick usados pelo mapeamento do controle.
fn mapped_buttons(pad: &Gamepad) -> Option<HashSet<u8>> {
    use sdl3_sys::gamepad::{SDL_GetGamepadBindings, SDL_GAMEPAD_BINDTYPE_BUTTON};
    let mut count = 0;
    // SAFETY: `pad.raw()` é um gamepad aberto. A lista devolvida tem `count`
    // ponteiros válidos e é uma única alocação, liberada com `SDL_free` no fim.
    unsafe {
        let list = SDL_GetGamepadBindings(pad.raw(), &mut count);
        if list.is_null() {
            log::warn!("não foi possível ler o mapeamento do controle; botões extras desligados");
            return None;
        }
        let mut mapped = HashSet::new();
        for i in 0..count.max(0) as usize {
            let binding = *list.add(i);
            if !binding.is_null() && (*binding).input_type == SDL_GAMEPAD_BINDTYPE_BUTTON {
                mapped.insert((*binding).input.button as u8);
            }
        }
        sdl3_sys::stdinc::SDL_free(list.cast());
        Some(mapped)
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
