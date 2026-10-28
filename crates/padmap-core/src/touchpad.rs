//! Touchpad dividido em zonas: uma grade de colunas e linhas, com uma tecla
//! por zona. A posição do dedo vem normalizada em `0..=1`, com `(0, 0)` no
//! canto de cima à esquerda.

use crate::key::KeyCombo;
use serde::{Deserialize, Serialize};

/// Maior grade aceita: 4 colunas por 3 linhas.
pub const MAX_COLS: u8 = 4;
pub const MAX_ROWS: u8 = 3;

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
/// linha, de cima para baixo e da esquerda para a direita.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TouchpadBinding {
    #[serde(default)]
    pub activate: TouchActivate,
    pub cols: u8,
    pub rows: u8,
    pub zones: Vec<Zone>,
}

impl Default for TouchpadBinding {
    fn default() -> Self {
        TouchpadBinding {
            activate: TouchActivate::default(),
            cols: 2,
            rows: 1,
            zones: vec![Zone::default(); 2],
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

    pub fn is_empty(&self) -> bool {
        self.zones.iter().all(|z| z.key.is_none())
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_COLS).contains(&self.cols) || !(1..=MAX_ROWS).contains(&self.rows) {
            return Err(format!(
                "touchpad: a grade vai de 1×1 a {MAX_COLS}×{MAX_ROWS}"
            ));
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
            activate: TouchActivate::Click,
            cols,
            rows,
            zones: vec![Zone::default(); cols as usize * rows as usize],
        }
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
        assert!(g.is_empty());
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
