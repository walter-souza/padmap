use crate::codes::macos_keycode;
use crate::{KeyAction, OutputError, OutputSink};
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

/// Injeta teclas e move o mouse com Quartz Event Services.
///
/// O macOS exige que o app (ou o Terminal que o executa) tenha permissão de
/// Acessibilidade em Ajustes do Sistema > Privacidade e Segurança.
pub struct CgEventSink {
    source: CGEventSource,
}

impl CgEventSink {
    pub fn new() -> Result<Self, OutputError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| OutputError::Init("CGEventSourceCreate falhou".into()))?;
        Ok(CgEventSink { source })
    }
}

impl OutputSink for CgEventSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError> {
        let (key, down) = match action {
            KeyAction::Press(k) => (k, true),
            KeyAction::Release(k) => (k, false),
        };
        let event = CGEvent::new_keyboard_event(self.source.clone(), macos_keycode(key), down)
            .map_err(|_| {
                OutputError::Send(format!("CGEventCreateKeyboardEvent falhou para {key}"))
            })?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    /// O macOS só aceita posição absoluta: lê onde o cursor está e soma o
    /// deslocamento. Os campos de delta vão junto para jogos que leem o
    /// movimento relativo. O sistema mantém o cursor dentro das telas.
    fn move_mouse(&mut self, dx: i32, dy: i32) -> Result<(), OutputError> {
        let here = CGEvent::new(self.source.clone())
            .map_err(|_| OutputError::Send("CGEventCreate falhou".into()))?
            .location();
        let to = CGPoint::new(here.x + dx as f64, here.y + dy as f64);
        let event = CGEvent::new_mouse_event(
            self.source.clone(),
            CGEventType::MouseMoved,
            to,
            CGMouseButton::Left,
        )
        .map_err(|_| OutputError::Send("CGEventCreateMouseEvent falhou".into()))?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as i64);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as i64);
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
}
