//! Perfis de mapeamento, salvos em TOML.

use crate::gamepad::{Axis, Button};
use crate::key::KeyCombo;
use crate::stick::{Sector, Stick, StickBinding};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("não foi possível ler {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("perfil inválido: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("perfil inválido: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Fração do curso do eixo (0..1) a partir da qual a tecla é pressionada.
    pub axis_threshold: f32,
    /// A tecla é solta quando o eixo volta abaixo de `axis_threshold * release_ratio`.
    /// Essa histerese evita que a tecla fique "piscando" perto do limiar.
    pub release_ratio: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            axis_threshold: 0.5,
            release_ratio: 0.8,
        }
    }
}

/// Teclas para cada sentido de um eixo. Para gatilhos só `positive` faz sentido.
/// Eixos de stick escritos aqui viram setores em `sticks` ao carregar o perfil.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisBinding {
    pub negative: Option<KeyCombo>,
    pub positive: Option<KeyCombo>,
    /// Sobrescreve `settings.axis_threshold` só para este eixo.
    pub threshold: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub buttons: BTreeMap<Button, KeyCombo>,
    #[serde(default)]
    pub axes: BTreeMap<Axis, AxisBinding>,
    #[serde(default)]
    pub sticks: BTreeMap<Stick, StickBinding>,
}

impl Profile {
    pub fn from_toml(text: &str) -> Result<Self, ProfileError> {
        let mut profile: Profile = toml::from_str(text)?;
        profile.migrate_stick_axes()?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| ProfileError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml(&text)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("perfil sempre serializável")
    }

    /// Zona morta do stick: a dele, ou `settings.axis_threshold`.
    pub fn deadzone(&self, stick: Stick) -> f32 {
        self.sticks
            .get(&stick)
            .and_then(|b| b.deadzone)
            .unwrap_or(self.settings.axis_threshold)
    }

    /// Perfis antigos mapeavam cada sentido de `left_x`/`left_y` como eixo
    /// solto. Eles viram quatro setores de 90° com disparo compartilhado, que
    /// também acionam as duas teclas na diagonal.
    fn migrate_stick_axes(&mut self) -> Result<(), ProfileError> {
        for stick in Stick::ALL {
            let (x, y) = stick.default_axes();
            if !self.axes.contains_key(&x) && !self.axes.contains_key(&y) {
                continue;
            }
            if self.sticks.contains_key(&stick) {
                return Err(ProfileError::Invalid(format!(
                    "sticks.{stick} e axes.{x}/axes.{y} mapeiam o mesmo stick; use só sticks.{stick}"
                )));
            }
            let xb = self.axes.remove(&x).unwrap_or_default();
            let yb = self.axes.remove(&y).unwrap_or_default();
            let mut binding = StickBinding::new(
                vec![
                    Sector::new(45.0, yb.negative),
                    Sector::new(135.0, xb.negative),
                    Sector::new(225.0, yb.positive),
                    Sector::new(315.0, xb.positive),
                ],
                true,
            );
            binding.deadzone = xb.threshold.or(yb.threshold);
            self.sticks.insert(stick, binding);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ProfileError> {
        let in_range = |v: f32| v > 0.0 && v <= 1.0;
        if !in_range(self.settings.axis_threshold) {
            return Err(ProfileError::Invalid(
                "settings.axis_threshold deve estar em (0, 1]".into(),
            ));
        }
        if !in_range(self.settings.release_ratio) {
            return Err(ProfileError::Invalid(
                "settings.release_ratio deve estar em (0, 1]".into(),
            ));
        }
        for (axis, binding) in &self.axes {
            if binding.threshold.is_some_and(|t| !in_range(t)) {
                return Err(ProfileError::Invalid(format!(
                    "axes.{axis}.threshold deve estar em (0, 1]"
                )));
            }
        }
        for (stick, binding) in &self.sticks {
            binding.validate(*stick).map_err(ProfileError::Invalid)?;
            let (x, y) = binding.axes(*stick);
            if self.axes.contains_key(&x) || self.axes.contains_key(&y) {
                return Err(ProfileError::Invalid(format!(
                    "sticks.{stick} usa {x}/{y}, que também estão em axes"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    #[test]
    fn parses_example_profile() {
        let p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        assert_eq!(p.buttons[&Button::South], Key::Space.into());
        assert_eq!(p.sticks[&Stick::Left].sectors[0].key, Some(Key::W.into()));
    }

    #[test]
    fn migrates_old_stick_axes_to_sectors() {
        let p = Profile::from_toml(
            "name='t'
[axes.left_x]
negative='A'
positive='D'
threshold=0.3
[axes.left_y]
negative='W'
",
        )
        .unwrap();
        assert!(p.axes.is_empty());
        let b = &p.sticks[&Stick::Left];
        let keys: Vec<_> = b.sectors.iter().map(|s| s.key.clone()).collect();
        assert_eq!(
            keys,
            [
                Some(Key::W.into()),
                Some(Key::A.into()),
                None,
                Some(Key::D.into())
            ]
        );
        assert!(b.shared);
        assert_eq!(p.deadzone(Stick::Left), 0.3);
    }

    #[test]
    fn rejects_stick_in_both_formats() {
        let err = Profile::from_toml(
            "name='t'
[axes.left_x]
negative='A'
[sticks.left]
sectors=[{start=0.0}]
",
        );
        assert!(matches!(err, Err(ProfileError::Invalid(_))));
    }

    #[test]
    fn round_trips_through_toml() {
        let p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);
    }

    #[test]
    fn mouse_stick_round_trips_and_keeps_sectors() {
        let mut p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        let b = p.sticks.get_mut(&Stick::Right).unwrap();
        b.mode = crate::stick::StickMode::Mouse;
        b.mouse.speed = 900.0;
        b.mouse.invert_y = true;
        let text = p.to_toml();
        assert!(text.contains("mode = \"mouse\""), "{text}");
        assert_eq!(Profile::from_toml(&text).unwrap(), p);
        // Sem `mode`, o stick continua por setores.
        assert!(
            !Profile::from_toml(include_str!("../../../profiles/default.toml"))
                .unwrap()
                .to_toml()
                .contains("mode")
        );
    }

    #[test]
    fn rejects_unknown_button() {
        let err = Profile::from_toml("name = \"x\"\n[buttons]\nbanana = \"A\"\n");
        assert!(err.is_err());
    }

    #[test]
    fn rejects_bad_threshold() {
        let err = Profile::from_toml("name = \"x\"\n[settings]\naxis_threshold = 0\n");
        assert!(matches!(err, Err(ProfileError::Invalid(_))));
    }
}
