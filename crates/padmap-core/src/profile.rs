//! Perfis de mapeamento, salvos em TOML.

use crate::extra::{ExtraButton, ExtraSource};
use crate::gamepad::{Axis, Button};
use crate::key::KeyCombo;
use crate::stick::{Sector, Stick, StickBinding};
use crate::touchpad::TouchpadBinding;
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
    /// Botões fora do desenho principal, na ordem em que o usuário adicionou.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<ExtraButton>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touchpad: Option<TouchpadBinding>,
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
        let mut seen = std::collections::BTreeSet::new();
        for extra in &self.extras {
            if !seen.insert(extra.button) {
                return Err(ProfileError::Invalid(format!(
                    "extras: {} aparece mais de uma vez",
                    extra.button
                )));
            }
            if let ExtraSource::Known(b) = extra.button {
                if self.buttons.contains_key(&b) {
                    return Err(ProfileError::Invalid(format!(
                        "{b} está em buttons e em extras; deixe em um só"
                    )));
                }
            }
        }
        if let Some(tp) = &self.touchpad {
            tp.validate().map_err(ProfileError::Invalid)?;
            let click_mapped = self.buttons.contains_key(&Button::Touchpad)
                || self
                    .extras
                    .iter()
                    .any(|e| e.button == ExtraSource::Known(Button::Touchpad));
            if tp.owns_click() && click_mapped {
                return Err(ProfileError::Invalid(
                    "o clique do touchpad já é do touchpad (zonas ao clicar ou mouse); tire o touchpad de buttons/extras".into(),
                ));
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
    fn extras_round_trip_and_reject_duplicates() {
        let text = "name = 'x'
[[extras]]
button = 'other_11'
name = 'Atrás · esquerda'
key = 'Shift'

[[extras]]
button = 'left_paddle1'
";
        let p = Profile::from_toml(text).unwrap();
        assert_eq!(p.extras.len(), 2);
        assert_eq!(p.extras[0].button, ExtraSource::Other(11));
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);

        let dup = format!("{text}\n[[extras]]\nbutton = 'other_11'\n");
        assert!(Profile::from_toml(&dup).is_err());
        let both = format!("{text}\n[buttons]\nleft_paddle1 = 'F'\n");
        assert!(Profile::from_toml(&both).is_err());
    }

    #[test]
    fn touchpad_round_trips_and_owns_the_click() {
        let text = "name = 'x'
[touchpad]
cols = 2
rows = 1
zones = [{ key = 'Q' }, {}]
";
        let p = Profile::from_toml(text).unwrap();
        let tp = p.touchpad.as_ref().unwrap();
        assert_eq!(tp.activate, crate::touchpad::TouchActivate::Click);
        assert_eq!(tp.zones[0].key, Some(Key::Q.into()));
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);

        let clash = format!("{text}\n[buttons]\ntouchpad = 'Tab'\n");
        assert!(Profile::from_toml(&clash).is_err());
        let touch = clash.replace("rows = 1", "rows = 1\nactivate = 'touch'");
        assert!(Profile::from_toml(&touch).is_ok());
        let short = text.replace(", {}", "");
        assert!(Profile::from_toml(&short).is_err());
    }

    #[test]
    fn touchpad_modes_round_trip() {
        let text = "name = 'x'
[touchpad]
mode = 'swipe'
swipe = { distance = 0.3, up = 'M', left = 'Q' }
[buttons]
touchpad = 'Tab'
";
        let p = Profile::from_toml(text).unwrap();
        let tp = p.touchpad.as_ref().unwrap();
        assert_eq!(tp.zones.len(), 2);
        assert_eq!(tp.swipe.up, Some(Key::M.into()));
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);
        // No mouse o clique é do touchpad.
        assert!(Profile::from_toml(&text.replace("'swipe'", "'mouse'")).is_err());
        let mouse = Profile::from_toml("name='x'\n[touchpad]\nmode='mouse'\n").unwrap();
        let m = &mouse.touchpad.unwrap().mouse;
        assert_eq!(m.click, Some(Key::MouseLeft.into()));
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
