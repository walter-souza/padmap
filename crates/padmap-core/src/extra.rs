//! Botões extras: qualquer botão do controle que não está no desenho
//! principal, com um nome dado pelo usuário. Pode ser um botão que o SDL
//! conhece mas o desenho não mostra (paddles, `misc1`, touchpad) ou um botão
//! cru do joystick, que o mapeamento padrão do controle não cobre.

use crate::gamepad::Button;
use crate::key::KeyCombo;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// De onde vem o botão extra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExtraSource {
    /// Um botão com nome no SDL.
    Known(Button),
    /// O botão de índice `n` do joystick (contando de 0), sem nome no
    /// mapeamento do controle. Escrito no perfil como `other_<n>`.
    Other(u8),
}

impl fmt::Display for ExtraSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtraSource::Known(b) => f.write_str(b.name()),
            ExtraSource::Other(n) => write!(f, "other_{n}"),
        }
    }
}

impl FromStr for ExtraSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(n) = s.strip_prefix("other_") {
            return n
                .parse()
                .map(ExtraSource::Other)
                .map_err(|_| format!("botão extra inválido: {s:?}"));
        }
        s.parse().map(ExtraSource::Known)
    }
}

impl Serialize for ExtraSource {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ExtraSource {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraButton {
    pub button: ExtraSource,
    /// Nome que aparece na lista ("Atrás · esquerda").
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyCombo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_round_trips_as_text() {
        for s in [
            ExtraSource::Other(11),
            ExtraSource::Known(Button::LeftPaddle1),
        ] {
            assert_eq!(s.to_string().parse::<ExtraSource>(), Ok(s));
        }
        assert_eq!("other_3".parse(), Ok(ExtraSource::Other(3)));
        assert!("other_x".parse::<ExtraSource>().is_err());
        assert!("banana".parse::<ExtraSource>().is_err());
    }
}
