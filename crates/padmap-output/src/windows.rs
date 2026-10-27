use crate::codes::windows_scancode;
use crate::{KeyAction, OutputError, OutputSink};
use padmap_core::MouseButton;
use std::mem::size_of;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
};

/// Injeta teclas com `SendInput` usando scancodes, que funcionam tanto em
/// aplicativos comuns quanto na maioria dos jogos, e move o mouse com
/// movimento relativo (`MOUSEEVENTF_MOVE`), que jogos com Raw Input também leem.
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

impl OutputSink for SendInputSink {
    fn send(&mut self, action: KeyAction) -> Result<(), OutputError> {
        let (key, up) = match action {
            KeyAction::Press(k) => (k, false),
            KeyAction::Release(k) => (k, true),
        };
        if let Some(button) = key.mouse_button() {
            let flags = match (button, up) {
                (MouseButton::Left, false) => MOUSEEVENTF_LEFTDOWN,
                (MouseButton::Left, true) => MOUSEEVENTF_LEFTUP,
                (MouseButton::Right, false) => MOUSEEVENTF_RIGHTDOWN,
                (MouseButton::Right, true) => MOUSEEVENTF_RIGHTUP,
                (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEDOWN,
                (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEUP,
            };
            return send_mouse(0, 0, flags)
                .map_err(|e| OutputError::Send(format!("SendInput recusou {key} ({e})")));
        }
        let (scan, extended) = windows_scancode(key).expect("tecla de teclado tem scancode");
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
        send_input(&input).map_err(|e| OutputError::Send(format!("SendInput recusou {key} ({e})")))
    }

    fn move_mouse(&mut self, dx: i32, dy: i32) -> Result<(), OutputError> {
        send_mouse(dx, dy, MOUSEEVENTF_MOVE)
            .map_err(|e| OutputError::Send(format!("SendInput recusou o movimento do mouse ({e})")))
    }
}

fn send_mouse(dx: i32, dy: i32, flags: u32) -> Result<(), std::io::Error> {
    send_input(&INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    })
}

fn send_input(input: &INPUT) -> Result<(), std::io::Error> {
    // SAFETY: um único INPUT válido, com o tamanho correto da struct.
    let sent = unsafe { SendInput(1, input, size_of::<INPUT>() as i32) };
    if sent == 1 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
