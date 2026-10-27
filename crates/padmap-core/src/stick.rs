//! Sticks mapeados por ângulo: o círculo do stick é dividido em setores e cada
//! setor pode acionar uma tecla. No modo mouse, o stick move o cursor.
//!
//! Ângulos em graus, no sentido anti-horário a partir da direita (0° = direita,
//! 90° = cima), como o stick é visto. O eixo Y do controle cresce para baixo, por
//! isso o sinal de `y` é invertido em `angle_of`.

use crate::gamepad::Axis;
use crate::key::KeyCombo;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Menor largura que um setor pode ter ao ser editado.
pub const MIN_SECTOR_WIDTH: f32 = 10.0;
/// Com o disparo compartilhado, cada setor avança essa fração da própria
/// largura sobre os vizinhos. Em quatro setores de 90°, a diagonal aciona os dois.
pub const SHARED_OVERLAP: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stick {
    Left,
    Right,
}

impl Stick {
    pub const ALL: [Stick; 2] = [Stick::Left, Stick::Right];

    /// Eixos que o stick usa quando o perfil não diz outros.
    pub fn default_axes(self) -> (Axis, Axis) {
        match self {
            Stick::Left => (Axis::LeftX, Axis::LeftY),
            Stick::Right => (Axis::RightX, Axis::RightY),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Stick::Left => "left",
            Stick::Right => "right",
        }
    }
}

impl fmt::Display for Stick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// O que o stick faz: aciona teclas pelos setores ou move o mouse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StickMode {
    #[default]
    Sectors,
    Mouse,
}

impl StickMode {
    fn is_sectors(&self) -> bool {
        *self == StickMode::Sectors
    }
}

/// Como o stick move o mouse no modo `Mouse`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MouseSettings {
    /// Velocidade do cursor, em pixels por segundo, com o stick no fim do curso.
    pub speed: f32,
    /// Expoente da curva de resposta: 1 é linear; maior deixa o começo do curso
    /// mais lento, para mirar, sem perder a velocidade no fim.
    pub curve: f32,
    /// Para cima no stick move o cursor para baixo.
    pub invert_y: bool,
}

impl Default for MouseSettings {
    fn default() -> Self {
        MouseSettings {
            speed: 1200.0,
            curve: 2.0,
            invert_y: false,
        }
    }
}

impl MouseSettings {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Velocidade do cursor em pixels por segundo, `(x, y)` com y para baixo
    /// como na tela, para o stick em `(x, y)`. Dentro da zona morta é zero; dela
    /// até o fim do curso a força vai de 0 a 1 e passa pela curva.
    pub fn velocity(&self, x: f32, y: f32, deadzone: f32) -> (f32, f32) {
        let r = x.hypot(y);
        if r <= deadzone || deadzone >= 1.0 {
            return (0.0, 0.0);
        }
        let t = ((r.min(1.0) - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
        let scale = self.speed * t.powf(self.curve) / r;
        let y = if self.invert_y { -y } else { y };
        (x * scale, y * scale)
    }
}

/// Um setor começa em `start` e vai até o início do próximo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sector {
    pub start: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyCombo>,
}

impl Sector {
    pub fn new(start: f32, key: Option<KeyCombo>) -> Self {
        Sector { start, key }
    }
}

/// Configuração de um stick. Os setores ficam em ordem crescente de `start`,
/// todos em `[0, 360)`, e juntos cobrem o círculo inteiro: o último vai até o
/// início do primeiro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_axis: Option<Axis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_axis: Option<Axis>,
    /// Setores ou mouse. Os setores ficam guardados no modo mouse, para quem
    /// voltar às teclas não perder o mapeamento.
    #[serde(default, skip_serializing_if = "StickMode::is_sectors")]
    pub mode: StickMode,
    #[serde(default, skip_serializing_if = "MouseSettings::is_default")]
    pub mouse: MouseSettings,
    /// Zona morta: fração do curso a partir da qual os setores acionam (ou o
    /// mouse começa a andar). Sem valor, vale `settings.axis_threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadzone: Option<f32>,
    /// Disparo compartilhado: setores vizinhos se sobrepõem perto da divisa.
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub sectors: Vec<Sector>,
}

/// Ângulo do stick em graus, em `[0, 360)`.
pub fn angle_of(x: f32, y: f32) -> f32 {
    (-y).atan2(x).to_degrees().rem_euclid(360.0)
}

impl StickBinding {
    pub fn new(sectors: Vec<Sector>, shared: bool) -> Self {
        StickBinding {
            x_axis: None,
            y_axis: None,
            mode: StickMode::Sectors,
            mouse: MouseSettings::default(),
            deadzone: None,
            shared,
            sectors,
        }
    }

    pub fn axes(&self, stick: Stick) -> (Axis, Axis) {
        let (x, y) = stick.default_axes();
        (self.x_axis.unwrap_or(x), self.y_axis.unwrap_or(y))
    }

    /// Largura do setor `i` em graus.
    pub fn width(&self, i: usize) -> f32 {
        let n = self.sectors.len();
        if n == 1 {
            return 360.0;
        }
        let next = self.sectors[(i + 1) % n].start;
        (next - self.sectors[i].start).rem_euclid(360.0)
    }

    /// Ângulo do meio do setor `i`.
    pub fn center(&self, i: usize) -> f32 {
        (self.sectors[i].start + self.width(i) / 2.0).rem_euclid(360.0)
    }

    /// Se `angle` cai no setor `i`, alargado pela sobreposição do disparo
    /// compartilhado e por `margin` graus de cada lado (negativo estreita).
    pub fn contains(&self, i: usize, angle: f32, margin: f32) -> bool {
        let width = self.width(i);
        if width >= 360.0 {
            return true;
        }
        let grow = if self.shared {
            width * SHARED_OVERLAP
        } else {
            0.0
        } + margin;
        if width + 2.0 * grow >= 360.0 {
            return true;
        }
        (angle - (self.sectors[i].start - grow)).rem_euclid(360.0) < width + 2.0 * grow
    }

    /// Setores acionados com o stick em `(x, y)`, sem histerese. Serve para
    /// mostrar o estado; o `Mapper` faz a mesma conta com histerese.
    pub fn sectors_at(&self, x: f32, y: f32, deadzone: f32) -> Vec<usize> {
        if x.hypot(y) < deadzone {
            return Vec::new();
        }
        let angle = angle_of(x, y);
        (0..self.sectors.len())
            .filter(|&i| self.contains(i, angle, 0.0))
            .collect()
    }

    /// Índice do setor que contém `angle`, sem sobreposição.
    pub fn sector_at_angle(&self, angle: f32) -> Option<usize> {
        let plain = StickBinding {
            shared: false,
            ..self.clone()
        };
        (0..self.sectors.len()).find(|&i| plain.contains(i, angle, 0.0))
    }

    /// Move o início do setor `i` para `angle`, sem passar dos vizinhos.
    /// Devolve o novo índice do setor, que muda se ele cruzar 0°.
    pub fn move_start(&mut self, i: usize, angle: f32) -> usize {
        let n = self.sectors.len();
        let angle = angle.rem_euclid(360.0);
        if n == 1 {
            self.sectors[0].start = angle;
            return 0;
        }
        let prev = self.sectors[(i + n - 1) % n].start;
        let room = (self.sectors[(i + 1) % n].start - prev).rem_euclid(360.0);
        // Distância a partir do início do setor anterior, presa no espaço livre.
        // Fora dele, vai para a ponta mais próxima.
        let mut offset = (angle - prev).rem_euclid(360.0);
        if offset > room {
            offset = if offset - room < 360.0 - offset {
                room
            } else {
                0.0
            };
        }
        let offset = offset.clamp(MIN_SECTOR_WIDTH, room - MIN_SECTOR_WIDTH);
        self.sectors[i].start = (prev + offset).rem_euclid(360.0);
        self.sort_keeping(i)
    }

    /// Divide o setor `i` ao meio. O novo setor fica sem tecla.
    /// Devolve o índice do novo setor, ou `None` se ele ficaria estreito demais.
    pub fn split(&mut self, i: usize) -> Option<usize> {
        let width = self.width(i);
        if width / 2.0 < MIN_SECTOR_WIDTH {
            return None;
        }
        let start = (self.sectors[i].start + width / 2.0).rem_euclid(360.0);
        self.sectors.insert(i + 1, Sector::new(start, None));
        Some(self.sort_keeping(i + 1))
    }

    /// Remove o setor `i`; o anterior passa a cobrir a área dele.
    pub fn remove(&mut self, i: usize) {
        if self.sectors.len() > 1 {
            self.sectors.remove(i);
        }
    }

    /// Reordena por `start` e devolve onde o setor `i` foi parar.
    fn sort_keeping(&mut self, i: usize) -> usize {
        let start = self.sectors[i].start;
        self.sectors.sort_by(|a, b| a.start.total_cmp(&b.start));
        self.sectors
            .iter()
            .position(|s| s.start == start)
            .unwrap_or(0)
    }

    pub(crate) fn validate(&self, stick: Stick) -> Result<(), String> {
        let (x, y) = self.axes(stick);
        let is_stick_axis = |a: Axis| !matches!(a, Axis::LeftTrigger | Axis::RightTrigger);
        if x == y || !is_stick_axis(x) || !is_stick_axis(y) {
            return Err(format!(
                "sticks.{stick}: x_axis e y_axis devem ser dois eixos de stick diferentes"
            ));
        }
        if self.deadzone.is_some_and(|d| !(d > 0.0 && d <= 1.0)) {
            return Err(format!("sticks.{stick}.deadzone deve estar em (0, 1]"));
        }
        let m = &self.mouse;
        if !(m.speed > 0.0 && m.speed.is_finite()) || !(m.curve > 0.0 && m.curve.is_finite()) {
            return Err(format!(
                "sticks.{stick}.mouse: speed e curve devem ser números positivos"
            ));
        }
        // No modo mouse os setores são opcionais: só ficam guardados.
        if self.sectors.is_empty() && self.mode == StickMode::Sectors {
            return Err(format!("sticks.{stick}: precisa de pelo menos um setor"));
        }
        if self
            .sectors
            .iter()
            .any(|s| !(0.0..360.0).contains(&s.start))
        {
            return Err(format!("sticks.{stick}: start deve estar em [0, 360)"));
        }
        if self.sectors.windows(2).any(|w| w[0].start >= w[1].start) {
            return Err(format!(
                "sticks.{stick}: os setores devem estar em ordem crescente de start"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    fn four(shared: bool) -> StickBinding {
        StickBinding::new(
            [
                (45.0, Key::W),
                (135.0, Key::A),
                (225.0, Key::S),
                (315.0, Key::D),
            ]
            .into_iter()
            .map(|(s, k)| Sector::new(s, Some(k.into())))
            .collect(),
            shared,
        )
    }

    #[test]
    fn angle_grows_counter_clockwise_from_right() {
        assert_eq!(angle_of(1.0, 0.0), 0.0);
        assert_eq!(angle_of(0.0, -1.0), 90.0);
        assert_eq!(angle_of(-1.0, 0.0), 180.0);
        assert_eq!(angle_of(0.0, 1.0), 270.0);
    }

    #[test]
    fn last_sector_wraps_to_first() {
        let b = four(false);
        assert_eq!(b.width(3), 90.0);
        assert_eq!(b.center(3), 0.0);
        assert_eq!(b.sectors_at(1.0, 0.0, 0.5), [3]); // direita = D
        assert_eq!(b.sectors_at(0.0, -1.0, 0.5), [0]); // cima = W
        assert!(b.sectors_at(0.3, 0.0, 0.5).is_empty()); // dentro da zona morta
    }

    #[test]
    fn shared_fires_both_on_diagonal() {
        let d = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(four(false).sectors_at(d, -d, 0.5).len(), 1);
        assert_eq!(four(true).sectors_at(d, -d, 0.5), [0, 3]);
        assert_eq!(four(true).sectors_at(0.0, -1.0, 0.5), [0]);
    }

    #[test]
    fn move_start_stays_between_neighbours() {
        let mut b = four(false);
        assert_eq!(b.move_start(1, 150.0), 1);
        assert_eq!(b.sectors[1].start, 150.0);
        b.move_start(1, 10.0);
        assert_eq!(b.sectors[1].start, 45.0 + MIN_SECTOR_WIDTH);
    }

    #[test]
    fn move_start_across_zero_keeps_order() {
        let mut b = four(false);
        let i = b.move_start(0, 350.0);
        // 350 passa do vizinho (315 + 10 = 325 é o mínimo) e não do outro lado: fica.
        assert_eq!(b.sectors[i].start, 350.0);
        assert!(b.validate(Stick::Left).is_ok());
        assert_eq!(b.sectors[i].key, Some(Key::W.into()));
    }

    #[test]
    fn split_and_remove() {
        let mut b = four(false);
        let i = b.split(3).unwrap();
        assert_eq!(b.sectors.len(), 5);
        assert_eq!(b.sectors[i].start, 0.0);
        assert!(b.sectors[i].key.is_none());
        assert!(b.validate(Stick::Left).is_ok());
        b.remove(i);
        assert_eq!(b, four(false));
    }

    #[test]
    fn mouse_velocity_follows_curve_and_deadzone() {
        let m = MouseSettings::default();
        assert_eq!(m.velocity(0.1, 0.0, 0.2), (0.0, 0.0));
        assert_eq!(m.velocity(1.0, 0.0, 0.2), (1200.0, 0.0));
        // No meio do curso útil, com curva 2: um quarto da velocidade.
        let (vx, _) = m.velocity(0.6, 0.0, 0.2);
        assert!((vx - 300.0).abs() < 0.01);
        // Para cima no stick (y negativo) sobe o cursor; invertido, desce.
        assert!(m.velocity(0.0, -1.0, 0.2).1 < 0.0);
        let inv = MouseSettings {
            invert_y: true,
            ..m
        };
        assert!(inv.velocity(0.0, -1.0, 0.2).1 > 0.0);
        // Além do círculo (cantos do quadrado do stick) não passa do máximo.
        let (vx, vy) = m.velocity(1.0, 1.0, 0.2);
        assert!((vx.hypot(vy) - 1200.0).abs() < 0.01);
    }

    #[test]
    fn mouse_mode_allows_no_sectors() {
        let mut b = StickBinding::new(Vec::new(), false);
        assert!(b.validate(Stick::Right).is_err());
        b.mode = StickMode::Mouse;
        assert!(b.validate(Stick::Right).is_ok());
        b.mouse.speed = 0.0;
        assert!(b.validate(Stick::Right).is_err());
    }

    #[test]
    fn rejects_unsorted_and_same_axes() {
        let mut b = four(false);
        b.sectors.swap(0, 1);
        assert!(b.validate(Stick::Left).is_err());
        let mut b = four(false);
        b.y_axis = Some(Axis::LeftX);
        assert!(b.validate(Stick::Left).is_err());
    }
}
