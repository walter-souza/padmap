//! Captura de tecla: o usuário aperta a tecla (ou combinação) no teclado e ela
//! vira o mapeamento.

use eframe::egui::{self, Event};
use padmap_core::{Key, KeyCombo};

const MODIFIERS: [Key; 8] = [
    Key::LeftCtrl,
    Key::RightCtrl,
    Key::LeftShift,
    Key::RightShift,
    Key::LeftAlt,
    Key::RightAlt,
    Key::LeftMeta,
    Key::RightMeta,
];

#[derive(Default)]
pub struct Capture {
    /// Modificadores seguros agora, na ordem em que foram apertados.
    held: Vec<Key>,
    /// Todos os modificadores apertados desde o início da captura.
    seen: Vec<Key>,
}

impl Capture {
    /// Processa os eventos do quadro. Retorna a combinação quando ela termina:
    /// ao apertar uma tecla comum (com os modificadores seguros), ou ao soltar
    /// todos os modificadores sem apertar outra tecla (ex.: só `Shift`).
    pub fn feed(&mut self, events: &[Event]) -> Option<KeyCombo> {
        for event in events {
            let Event::Key {
                key,
                physical_key,
                pressed,
                repeat: false,
                ..
            } = event
            else {
                continue;
            };
            let Some(k) = from_egui(physical_key.unwrap_or(*key)) else {
                continue;
            };
            if MODIFIERS.contains(&k) {
                if *pressed {
                    if !self.held.contains(&k) {
                        self.held.push(k);
                    }
                    if !self.seen.contains(&k) {
                        self.seen.push(k);
                    }
                } else {
                    self.held.retain(|h| *h != k);
                    if self.held.is_empty() && !self.seen.is_empty() {
                        return Some(KeyCombo(std::mem::take(&mut self.seen)));
                    }
                }
            } else if *pressed {
                let mut keys = self.held.clone();
                keys.push(k);
                return Some(KeyCombo(keys));
            }
        }
        None
    }

    pub fn preview(&self) -> String {
        self.held
            .iter()
            .map(|k| k.name())
            .collect::<Vec<_>>()
            .join("+")
    }
}

pub fn from_egui(key: egui::Key) -> Option<Key> {
    use egui::Key as E;
    Some(match key {
        E::A => Key::A,
        E::B => Key::B,
        E::C => Key::C,
        E::D => Key::D,
        E::E => Key::E,
        E::F => Key::F,
        E::G => Key::G,
        E::H => Key::H,
        E::I => Key::I,
        E::J => Key::J,
        E::K => Key::K,
        E::L => Key::L,
        E::M => Key::M,
        E::N => Key::N,
        E::O => Key::O,
        E::P => Key::P,
        E::Q => Key::Q,
        E::R => Key::R,
        E::S => Key::S,
        E::T => Key::T,
        E::U => Key::U,
        E::V => Key::V,
        E::W => Key::W,
        E::X => Key::X,
        E::Y => Key::Y,
        E::Z => Key::Z,
        E::Num0 => Key::Num0,
        E::Num1 => Key::Num1,
        E::Num2 => Key::Num2,
        E::Num3 => Key::Num3,
        E::Num4 => Key::Num4,
        E::Num5 => Key::Num5,
        E::Num6 => Key::Num6,
        E::Num7 => Key::Num7,
        E::Num8 => Key::Num8,
        E::Num9 => Key::Num9,
        E::F1 => Key::F1,
        E::F2 => Key::F2,
        E::F3 => Key::F3,
        E::F4 => Key::F4,
        E::F5 => Key::F5,
        E::F6 => Key::F6,
        E::F7 => Key::F7,
        E::F8 => Key::F8,
        E::F9 => Key::F9,
        E::F10 => Key::F10,
        E::F11 => Key::F11,
        E::F12 => Key::F12,
        E::Escape => Key::Escape,
        E::Tab => Key::Tab,
        E::CapsLock => Key::CapsLock,
        E::Space => Key::Space,
        E::Enter => Key::Enter,
        E::Backspace => Key::Backspace,
        E::Insert => Key::Insert,
        E::Delete => Key::Delete,
        E::Home => Key::Home,
        E::End => Key::End,
        E::PageUp => Key::PageUp,
        E::PageDown => Key::PageDown,
        E::ArrowUp => Key::Up,
        E::ArrowDown => Key::Down,
        E::ArrowLeft => Key::Left,
        E::ArrowRight => Key::Right,
        E::ShiftLeft => Key::LeftShift,
        E::ShiftRight => Key::RightShift,
        E::ControlLeft => Key::LeftCtrl,
        E::ControlRight => Key::RightCtrl,
        E::AltLeft => Key::LeftAlt,
        E::AltRight => Key::RightAlt,
        E::SuperLeft => Key::LeftMeta,
        E::SuperRight => Key::RightMeta,
        E::Minus => Key::Minus,
        E::Equals | E::Plus => Key::Equal,
        E::OpenBracket | E::OpenCurlyBracket => Key::LeftBracket,
        E::CloseBracket | E::CloseCurlyBracket => Key::RightBracket,
        E::Backslash | E::Pipe => Key::Backslash,
        E::Semicolon | E::Colon => Key::Semicolon,
        E::Quote => Key::Apostrophe,
        E::Backtick => Key::Grave,
        E::Comma => Key::Comma,
        E::Period => Key::Period,
        E::Slash | E::Questionmark => Key::Slash,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: egui::Key, pressed: bool) -> Event {
        Event::Key {
            key: k,
            physical_key: Some(k),
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn plain_key() {
        let mut c = Capture::default();
        assert_eq!(
            c.feed(&[key(egui::Key::Space, true)]),
            Some(Key::Space.into())
        );
    }

    #[test]
    fn modifier_plus_key() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::ControlLeft, true)]), None);
        assert_eq!(
            c.feed(&[key(egui::Key::S, true)]),
            Some(KeyCombo(vec![Key::LeftCtrl, Key::S]))
        );
    }

    #[test]
    fn modifier_alone() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&[key(egui::Key::ShiftLeft, true)]), None);
        assert_eq!(
            c.feed(&[key(egui::Key::ShiftLeft, false)]),
            Some(Key::LeftShift.into())
        );
    }
}
