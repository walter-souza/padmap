use crate::codes::linux_keycode;
use crate::{KeyAction, OutputError, OutputSink};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, KeyCode, KeyEvent, RelativeAxisCode, RelativeAxisEvent};
use padmap_core::Key;

/// Cria um teclado e mouse virtual via `/dev/uinput`. Funciona em X11 e Wayland.
///
/// Requer permissão de escrita em `/dev/uinput` (grupo `input` ou uma regra
/// udev; veja docs/arquitetura.md).
pub struct UinputSink {
    device: VirtualDevice,
}

impl UinputSink {
    pub fn new() -> Result<Self, OutputError> {
        let mut keys: AttributeSet<KeyCode> = Key::ALL
            .iter()
            .map(|&k| KeyCode(linux_keycode(k)))
            .collect();
        // Sem um botão de mouse o sistema não trata o dispositivo como
        // apontador e ignora os eixos relativos.
        keys.insert(KeyCode::BTN_LEFT);
        let rel: AttributeSet<RelativeAxisCode> =
            [RelativeAxisCode::REL_X, RelativeAxisCode::REL_Y]
                .into_iter()
                .collect();
        let device = VirtualDevice::builder()
            .and_then(|b| b.name("padmap virtual input").with_keys(&keys))
            .and_then(|b| b.with_relative_axes(&rel))
            .and_then(|b| b.build())
            .map_err(|e| OutputError::Init(format!("/dev/uinput: {e}")))?;
        Ok(UinputSink { device })
    }
}

impl OutputSink for UinputSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError> {
        let (key, value) = match action {
            KeyAction::Press(k) => (k, 1),
            KeyAction::Release(k) => (k, 0),
        };
        let event = KeyEvent::new(KeyCode(linux_keycode(key)), value);
        self.device
            .emit(&[*event])
            .map_err(|e| OutputError::Send(e.to_string()))
    }

    fn move_mouse(&mut self, dx: i32, dy: i32) -> Result<(), OutputError> {
        let events = [
            *RelativeAxisEvent::new(RelativeAxisCode::REL_X, dx),
            *RelativeAxisEvent::new(RelativeAxisCode::REL_Y, dy),
        ];
        self.device
            .emit(&events)
            .map_err(|e| OutputError::Send(e.to_string()))
    }
}
