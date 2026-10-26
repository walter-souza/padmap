use crate::codes::windows_scancode;
use crate::{KeyAction, KeyboardSink, OutputError};
use std::mem::size_of;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE,
};

/// Injeta teclas com `SendInput` usando scancodes, que funcionam tanto em
/// aplicativos comuns quanto na maioria dos jogos.
///
/// Observação: o Windows bloqueia a injeção em janelas de processos com
/// privilégio maior (UIPI). Para controlar um jogo rodando como administrador,
/// o padmap também precisa rodar como administrador.
pub struct SendInputSink;

impl SendInputSink {
    pub fn new() -> Self {
        SendInputSink
    }
}

impl KeyboardSink for SendInputSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError> {
        let (key, up) = match action {
            KeyAction::Press(k) => (k, false),
            KeyAction::Release(k) => (k, true),
        };
        let (scan, extended) = windows_scancode(key);
        let mut flags = KEYEVENTF_SCANCODE;
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: 0,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        // SAFETY: um único INPUT válido, com o tamanho correto da struct.
        let sent = unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) };
        if sent == 1 {
            Ok(())
        } else {
            Err(OutputError::Send(format!(
                "SendInput recusou {key} ({})",
                std::io::Error::last_os_error()
            )))
        }
    }
}
