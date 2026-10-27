//! Teclas de saída, descritas por posição física (layout US), e combinações
//! como `Ctrl+Shift+S`. Cada backend de saída traduz `Key` para o código nativo
//! do seu sistema operacional. Os botões do mouse também são `Key`, para valerem
//! em qualquer mapeamento e combinação (`Ctrl+MouseLeft`).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

macro_rules! keys {
    ($($variant:ident => $name:literal $(| $alias:literal)*),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Key {
            $($variant),*
        }

        impl Key {
            pub const ALL: &'static [Key] = &[$(Key::$variant),*];

            /// Nome canônico usado nos perfis.
            pub fn name(self) -> &'static str {
                match self {
                    $(Key::$variant => $name),*
                }
            }

            /// Busca pelo nome canônico ou por um apelido, sem diferenciar maiúsculas.
            pub fn from_name(s: &str) -> Option<Key> {
                let s = s.trim();
                $(
                    if s.eq_ignore_ascii_case($name) $(|| s.eq_ignore_ascii_case($alias))* {
                        return Some(Key::$variant);
                    }
                )*
                None
            }
        }
    };
}

keys! {
    A => "A", B => "B", C => "C", D => "D", E => "E", F => "F", G => "G",
    H => "H", I => "I", J => "J", K => "K", L => "L", M => "M", N => "N",
    O => "O", P => "P", Q => "Q", R => "R", S => "S", T => "T", U => "U",
    V => "V", W => "W", X => "X", Y => "Y", Z => "Z",
    Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4",
    Num5 => "5", Num6 => "6", Num7 => "7", Num8 => "8", Num9 => "9",
    F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
    F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
    Escape => "Escape" | "Esc",
    Tab => "Tab",
    CapsLock => "CapsLock",
    Space => "Space" | "Espaco",
    Enter => "Enter" | "Return",
    Backspace => "Backspace",
    Insert => "Insert",
    Delete => "Delete" | "Del",
    Home => "Home",
    End => "End",
    PageUp => "PageUp",
    PageDown => "PageDown",
    Up => "Up",
    Down => "Down",
    Left => "Left",
    Right => "Right",
    LeftShift => "LShift" | "Shift",
    RightShift => "RShift",
    LeftCtrl => "LCtrl" | "Ctrl" | "Control",
    RightCtrl => "RCtrl",
    LeftAlt => "LAlt" | "Alt",
    RightAlt => "RAlt" | "AltGr",
    LeftMeta => "LMeta" | "Meta" | "Win" | "Super" | "Cmd",
    RightMeta => "RMeta",
    Minus => "Minus" | "-",
    Equal => "Equal" | "=",
    LeftBracket => "LeftBracket" | "[",
    RightBracket => "RightBracket" | "]",
    Backslash => "Backslash" | "\\",
    Semicolon => "Semicolon" | ";",
    Apostrophe => "Apostrophe" | "'",
    Grave => "Grave" | "`",
    Comma => "Comma" | ",",
    Period => "Period" | ".",
    Slash => "Slash" | "/",
    MouseLeft => "MouseLeft" | "Mouse1" | "LMB",
    MouseRight => "MouseRight" | "Mouse2" | "RMB",
    MouseMiddle => "MouseMiddle" | "Mouse3" | "MMB",
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl Key {
    /// O botão do mouse, se a tecla for um clique.
    pub fn mouse_button(self) -> Option<MouseButton> {
        match self {
            Key::MouseLeft => Some(MouseButton::Left),
            Key::MouseRight => Some(MouseButton::Right),
            Key::MouseMiddle => Some(MouseButton::Middle),
            _ => None,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Key {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Key::from_name(s).ok_or_else(|| format!("tecla desconhecida: {s:?}"))
    }
}

/// Uma ou mais teclas pressionadas juntas, na ordem em que aparecem
/// (e soltas na ordem inversa). Escrita no perfil como `"Ctrl+Shift+S"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo(pub Vec<Key>);

impl KeyCombo {
    pub fn keys(&self) -> &[Key] {
        &self.0
    }
}

impl From<Key> for KeyCombo {
    fn from(k: Key) -> Self {
        KeyCombo(vec![k])
    }
}

impl FromStr for KeyCombo {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // "+" sozinho não é suportado como tecla; use "Equal" com Shift.
        let keys = s
            .split('+')
            .map(str::parse)
            .collect::<Result<Vec<Key>, _>>()?;
        if keys.is_empty() {
            return Err("combinação vazia".into());
        }
        Ok(KeyCombo(keys))
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<_> = self.0.iter().map(|k| k.name()).collect();
        f.write_str(&names.join("+"))
    }
}

impl Serialize for KeyCombo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_case_insensitive() {
        assert_eq!("esc".parse::<Key>(), Ok(Key::Escape));
        assert_eq!("ctrl".parse::<Key>(), Ok(Key::LeftCtrl));
        assert_eq!("7".parse::<Key>(), Ok(Key::Num7));
    }

    #[test]
    fn combo_round_trip() {
        let c: KeyCombo = "Ctrl+Shift+S".parse().unwrap();
        assert_eq!(c.0, vec![Key::LeftCtrl, Key::LeftShift, Key::S]);
        assert_eq!(c.to_string(), "LCtrl+LShift+S");
    }

    #[test]
    fn mouse_buttons_are_keys() {
        let c: KeyCombo = "Ctrl+Mouse1".parse().unwrap();
        assert_eq!(c.0, vec![Key::LeftCtrl, Key::MouseLeft]);
        assert_eq!(c.to_string(), "LCtrl+MouseLeft");
        assert_eq!(Key::MouseRight.mouse_button(), Some(MouseButton::Right));
        assert_eq!(Key::Space.mouse_button(), None);
    }

    #[test]
    fn rejects_unknown() {
        assert!("Ctrl+Banana".parse::<KeyCombo>().is_err());
    }
}
