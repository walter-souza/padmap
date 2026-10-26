use crate::codes::linux_keycode;
use crate::{KeyAction, KeyboardSink, OutputError};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, KeyCode, KeyEvent};
use padmap_core::Key;

/// Cria um teclado virtual via `/dev/uinput`. Funciona em X11 e Wayland.
///
/// Requer permissão de escrita em `/dev/uinput` (grupo `input` ou uma regra
/// udev; veja docs/arquitetura.md).
pub struct UinputSink {
    device: VirtualDevice,
}

impl UinputSink {
    pub fn new() -> Result<Self, OutputError> {
        let keys: AttributeSet<KeyCode> = Key::ALL
            .iter()
            .map(|&k| KeyCode(linux_keycode(k)))
            .collect();
        let device = VirtualDevice::builder()
            .and_then(|b| b.name("padmap virtual keyboard").with_keys(&keys))
            .and_then(|b| b.build())
            .map_err(|e| OutputError::Init(format!("/dev/uinput: {e}")))?;
        Ok(UinputSink { device })
    }
}

impl KeyboardSink for UinputSink {
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
}
