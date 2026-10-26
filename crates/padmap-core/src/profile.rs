//! Perfis de mapeamento, salvos em TOML.

use crate::gamepad::{Axis, Button};
use crate::key::KeyCombo;
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
}

impl Profile {
    pub fn from_toml(text: &str) -> Result<Self, ProfileError> {
        let profile: Profile = toml::from_str(text)?;
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
        assert!(p.axes[&Axis::LeftX].negative.is_some());
    }

    #[test]
    fn round_trips_through_toml() {
        let p = Profile::from_toml(include_str!("../../../profiles/default.toml")).unwrap();
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);
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
