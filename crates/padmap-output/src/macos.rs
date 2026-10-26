use crate::codes::macos_keycode;
use crate::{KeyAction, KeyboardSink, OutputError};
use core_graphics::event::{CGEvent, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Injeta teclas com Quartz Event Services.
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

impl KeyboardSink for CgEventSink {
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
}
