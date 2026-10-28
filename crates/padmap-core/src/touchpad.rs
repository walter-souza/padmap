//! Touchpad em três modos: zonas (uma grade com uma tecla por zona), mouse
//! (deslizar o dedo move o cursor, como num notebook) e gestos (deslizar numa
//! direção aperta uma tecla). A posição do dedo vem normalizada em `0..=1`,
//! com `(0, 0)` no canto de cima à esquerda.

use crate::key::{Key, KeyCombo};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maior grade aceita: 4 colunas por 3 linhas.
pub const MAX_COLS: u8 = 4;
pub const MAX_ROWS: u8 = 3;
/// Altura do touchpad em relação à largura (o do DualShock 4 é perto de
/// 2:1). Multiplica o `y` normalizado para medir distâncias nas duas direções
/// na mesma escala.
pub const ASPECT: f32 = 0.5;

/// O que o touchpad faz.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchMode {
    #[default]
    Zones,
    Mouse,
    Swipe,
}

impl TouchMode {
    fn is_zones(&self) -> bool {
        *self == TouchMode::Zones
    }
}

/// Direção de um gesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Swipe {
    Up,
    Down,
    Left,
    Right,
}

impl Swipe {
    pub const ALL: [Swipe; 4] = [Swipe::Up, Swipe::Down, Swipe::Left, Swipe::Right];
}

impl fmt::Display for Swipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Swipe::Up => "up",
            Swipe::Down => "down",
            Swipe::Left => "left",
            Swipe::Right => "right",
        })
    }
}

fn mouse_left() -> Option<KeyCombo> {
    Some(Key::MouseLeft.into())
}

fn mouse_right() -> Option<KeyCombo> {
    Some(Key::MouseRight.into())
}

/// Modo mouse: o cursor anda o quanto o dedo andou.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TouchMouse {
    /// Pixels que o cursor anda quando o dedo atravessa o touchpad de lado a lado.
    pub speed: f32,
    /// Apertar o touchpad com um dedo.
    pub click: Option<KeyCombo>,
    /// Apertar o touchpad com dois dedos encostados.
    pub two_finger_click: Option<KeyCombo>,
}

impl Default for TouchMouse {
    fn default() -> Self {
        TouchMouse {
            speed: 1500.0,
            click: mouse_left(),
            two_finger_click: mouse_right(),
        }
    }
}

impl TouchMouse {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Modo gestos: deslizar numa direção aperta a tecla dela, que fica apertada
/// até o dedo sair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TouchSwipes {
    /// Quanto o dedo precisa andar, em fração da largura do touchpad.
    pub distance: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<KeyCombo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down: Option<KeyCombo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<KeyCombo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<KeyCombo>,
}

impl Default for TouchSwipes {
    fn default() -> Self {
        TouchSwipes {
            distance: 0.25,
            up: None,
            down: None,
            left: None,
            right: None,
        }
    }
}

impl TouchSwipes {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn key(&self, dir: Swipe) -> Option<&KeyCombo> {
        match dir {
            Swipe::Up => self.up.as_ref(),
            Swipe::Down => self.down.as_ref(),
            Swipe::Left => self.left.as_ref(),
            Swipe::Right => self.right.as_ref(),
        }
    }

    pub fn key_mut(&mut self, dir: Swipe) -> &mut Option<KeyCombo> {
        match dir {
            Swipe::Up => &mut self.up,
            Swipe::Down => &mut self.down,
            Swipe::Left => &mut self.left,
            Swipe::Right => &mut self.right,
        }
    }

    /// Direção do gesto do dedo que encostou em `from` e está em `to`, se ele
    /// já andou o bastante. Vale a direção em que ele andou mais.
    pub fn direction(&self, from: (f32, f32), to: (f32, f32)) -> Option<Swipe> {
        let dx = to.0 - from.0;
        let dy = (to.1 - from.1) * ASPECT;
        if dx.hypot(dy) < self.distance {
            return None;
        }
        Some(if dx.abs() >= dy.abs() {
            if dx > 0.0 {
                Swipe::Right
            } else {
                Swipe::Left
            }
        } else if dy > 0.0 {
            Swipe::Down
        } else {
            Swipe::Up
        })
    }
}

/// Quando uma zona aciona.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchActivate {
    /// Ao apertar o touchpad, na zona sob o dedo. Encostar sem apertar não
    /// faz nada, então o clique do touchpad não pode ter tecla própria.
    #[default]
    Click,
    /// Assim que o dedo encosta, até ele sair.
    Touch,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyCombo>,
}

/// Configuração do touchpad. `zones` tem `cols * rows` zonas, linha por
/// linha, de cima para baixo e da esquerda para a direita. A configuração dos
/// outros modos fica guardada, para quem voltar a um modo não perder nada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TouchpadBinding {
    #[serde(skip_serializing_if = "TouchMode::is_zones")]
    pub mode: TouchMode,
    pub activate: TouchActivate,
    pub cols: u8,
    pub rows: u8,
    pub zones: Vec<Zone>,
    #[serde(skip_serializing_if = "TouchMouse::is_default")]
    pub mouse: TouchMouse,
    #[serde(skip_serializing_if = "TouchSwipes::is_default")]
    pub swipe: TouchSwipes,
}

impl Default for TouchpadBinding {
    fn default() -> Self {
        TouchpadBinding {
            mode: TouchMode::default(),
            activate: TouchActivate::default(),
            cols: 2,
            rows: 1,
            zones: vec![Zone::default(); 2],
            mouse: TouchMouse::default(),
            swipe: TouchSwipes::default(),
        }
    }
}

impl TouchpadBinding {
    /// Zona sob o ponto `(x, y)`. Pontos fora de `0..=1` vão para a borda.
    pub fn zone_at(&self, x: f32, y: f32) -> usize {
        let cell = |v: f32, n: u8| ((v.clamp(0.0, 1.0) * n as f32) as usize).min(n as usize - 1);
        cell(y, self.rows) * self.cols as usize + cell(x, self.cols)
    }

    /// Troca a grade. As zonas que continuam na mesma linha e coluna mantêm a
    /// tecla; as novas começam livres.
    pub fn set_grid(&mut self, cols: u8, rows: u8) {
        let cols = cols.clamp(1, MAX_COLS);
        let rows = rows.clamp(1, MAX_ROWS);
        let mut zones = vec![Zone::default(); cols as usize * rows as usize];
        for r in 0..rows.min(self.rows) as usize {
            for c in 0..cols.min(self.cols) as usize {
                zones[r * cols as usize + c] = self.zones[r * self.cols as usize + c].clone();
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.zones = zones;
    }

    /// Se o modo atual não aciona nada: zonas ou gestos todos livres. O
    /// modo mouse sempre move o cursor.
    pub fn is_unused(&self) -> bool {
        match self.mode {
            TouchMode::Zones => self.zones.iter().all(|z| z.key.is_none()),
            TouchMode::Mouse => false,
            TouchMode::Swipe => Swipe::ALL.iter().all(|&d| self.swipe.key(d).is_none()),
        }
    }

    /// Se apertar o touchpad é deste modo: no mouse ele clica, nas zonas
    /// "ao clicar" ele aciona a zona. Aí o botão `touchpad` não pode ter
    /// tecla própria.
    pub fn owns_click(&self) -> bool {
        match self.mode {
            TouchMode::Zones => self.activate == TouchActivate::Click,
            TouchMode::Mouse => true,
            TouchMode::Swipe => false,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_COLS).contains(&self.cols) || !(1..=MAX_ROWS).contains(&self.rows) {
            return Err(format!(
                "touchpad: a grade vai de 1×1 a {MAX_COLS}×{MAX_ROWS}"
            ));
        }
        // `is_finite` também recusa NaN.
        if !self.mouse.speed.is_finite() || self.mouse.speed <= 0.0 {
            return Err("touchpad.mouse.speed deve ser maior que 0".into());
        }
        if !(self.swipe.distance.is_finite()
            && self.swipe.distance > 0.0
            && self.swipe.distance <= 1.0)
        {
            return Err("touchpad.swipe.distance deve estar em (0, 1]".into());
        }
        let want = self.cols as usize * self.rows as usize;
        if self.zones.len() != want {
            return Err(format!(
                "touchpad: uma grade {}×{} precisa de {want} zonas, mas tem {}",
                self.cols,
                self.rows,
                self.zones.len()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    fn grid(cols: u8, rows: u8) -> TouchpadBinding {
        TouchpadBinding {
            cols,
            rows,
            zones: vec![Zone::default(); cols as usize * rows as usize],
            ..TouchpadBinding::default()
        }
    }

    #[test]
    fn swipe_direction_needs_distance() {
        let s = TouchSwipes::default();
        assert_eq!(s.direction((0.5, 0.5), (0.6, 0.5)), None);
        assert_eq!(s.direction((0.5, 0.5), (0.8, 0.55)), Some(Swipe::Right));
        assert_eq!(s.direction((0.5, 0.5), (0.2, 0.5)), Some(Swipe::Left));
        // Na vertical o touchpad é mais curto: metade da altura basta.
        assert_eq!(s.direction((0.5, 0.9), (0.5, 0.35)), Some(Swipe::Up));
        assert_eq!(s.direction((0.5, 0.1), (0.5, 0.4)), None);
    }

    #[test]
    fn unused_and_click_depend_on_mode() {
        let mut g = grid(2, 1);
        assert!(g.is_unused() && g.owns_click());
        g.mode = TouchMode::Mouse;
        assert!(!g.is_unused() && g.owns_click());
        g.mode = TouchMode::Swipe;
        assert!(g.is_unused() && !g.owns_click());
        g.swipe.up = Some(Key::M.into());
        assert!(!g.is_unused());
    }

    #[test]
    fn zone_at_reads_row_by_row() {
        let g = grid(2, 2);
        assert_eq!(g.zone_at(0.1, 0.1), 0);
        assert_eq!(g.zone_at(0.9, 0.1), 1);
        assert_eq!(g.zone_at(0.1, 0.9), 2);
        assert_eq!(g.zone_at(1.0, 1.0), 3);
        assert_eq!(g.zone_at(-0.5, 2.0), 2);
    }

    #[test]
    fn set_grid_keeps_keys_in_place() {
        let mut g = grid(2, 1);
        g.zones[1].key = Some(Key::E.into());
        g.set_grid(2, 2);
        assert_eq!(g.zones.len(), 4);
        assert_eq!(g.zones[1].key, Some(Key::E.into()));
        g.set_grid(1, 1);
        assert!(g.is_unused());
        g.set_grid(9, 9);
        assert_eq!((g.cols, g.rows), (MAX_COLS, MAX_ROWS));
        assert!(g.validate().is_ok());
    }

    #[test]
    fn validate_checks_zone_count() {
        let mut g = grid(2, 2);
        g.zones.pop();
        assert!(g.validate().is_err());
        assert!(grid(5, 1).validate().is_err());
    }
}
