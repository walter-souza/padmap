use crate::codes::macos_keycode;
use crate::{KeyAction, OutputError, OutputSink};
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use padmap_core::MouseButton;

/// Injeta teclas e move o mouse com Quartz Event Services.
///
/// O macOS exige que o app (ou o Terminal que o executa) tenha permissão de
/// Acessibilidade em Ajustes do Sistema > Privacidade e Segurança.
pub struct CgEventSink {
    source: CGEventSource,
    /// Botão do mouse seguro agora: com ele, mover vira arrastar.
    dragging: Option<MouseButton>,
}

impl CgEventSink {
    pub fn new() -> Result<Self, OutputError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| OutputError::Init("CGEventSourceCreate falhou".into()))?;
        Ok(CgEventSink {
            source,
            dragging: None,
        })
    }
}

impl OutputSink for CgEventSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError> {
        let (key, down) = match action {
            KeyAction::Press(k) => (k, true),
            KeyAction::Release(k) => (k, false),
        };
        if let Some(button) = key.mouse_button() {
            return self.click(button, down);
        }
        let code = macos_keycode(key).expect("tecla de teclado tem keycode");
        let event = CGEvent::new_keyboard_event(self.source.clone(), code, down).map_err(|_| {
            OutputError::Send(format!("CGEventCreateKeyboardEvent falhou para {key}"))
        })?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    /// O macOS só aceita posição absoluta: lê onde o cursor está e soma o
    /// deslocamento. Os campos de delta vão junto para jogos que leem o
    /// movimento relativo. O sistema mantém o cursor dentro das telas.
    fn move_mouse(&mut self, dx: i32, dy: i32) -> Result<(), OutputError> {
        let here = self.cursor()?;
        let to = CGPoint::new(here.x + dx as f64, here.y + dy as f64);
        let (kind, button) = match self.dragging {
            None => (CGEventType::MouseMoved, CGMouseButton::Left),
            Some(MouseButton::Left) => (CGEventType::LeftMouseDragged, CGMouseButton::Left),
            Some(MouseButton::Right) => (CGEventType::RightMouseDragged, CGMouseButton::Right),
            Some(MouseButton::Middle) => (CGEventType::OtherMouseDragged, CGMouseButton::Center),
        };
        let event = CGEvent::new_mouse_event(self.source.clone(), kind, to, button)
            .map_err(|_| OutputError::Send("CGEventCreateMouseEvent falhou".into()))?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, dx as i64);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, dy as i64);
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
}

impl CgEventSink {
    fn cursor(&self) -> Result<CGPoint, OutputError> {
        Ok(CGEvent::new(self.source.clone())
            .map_err(|_| OutputError::Send("CGEventCreate falhou".into()))?
            .location())
    }

    /// Clique na posição atual do cursor.
    fn click(&mut self, button: MouseButton, down: bool) -> Result<(), OutputError> {
        let (kind, cg) = match (button, down) {
            (MouseButton::Left, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
            (MouseButton::Left, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (MouseButton::Right, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
            (MouseButton::Right, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
            (MouseButton::Middle, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
            (MouseButton::Middle, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
        };
        let event = CGEvent::new_mouse_event(self.source.clone(), kind, self.cursor()?, cg)
            .map_err(|_| OutputError::Send("CGEventCreateMouseEvent falhou".into()))?;
        event.post(CGEventTapLocation::HID);
        self.dragging = if down { Some(button) } else { None };
        Ok(())
    }
}
