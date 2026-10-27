//! Motor de mapeamento: transforma eventos de controle em pressionar/soltar
//! teclas, de acordo com um perfil.
//!
//! As teclas têm contagem de referência: se dois botões estão ligados a
//! `Shift`, a tecla só é solta quando os dois forem soltos.

use crate::gamepad::{Axis, Button, InputEvent};
use crate::key::{Key, KeyCombo};
use crate::profile::Profile;
use crate::stick::{angle_of, Stick};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Press(Key),
    Release(Key),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Source {
    Button(Button),
    AxisNeg(Axis),
    AxisPos(Axis),
    Sector(Stick, usize),
}

/// Histerese angular, em graus: um setor só aciona com o stick essa folga para
/// dentro da borda e só solta com o stick essa folga para fora. Evita que a
/// tecla pisque, ou que dois setores vizinhos acionem juntos, na divisa.
const SECTOR_HYSTERESIS: f32 = 3.0;

pub struct Mapper {
    profile: Profile,
    active: HashSet<Source>,
    held: HashMap<Key, u32>,
    /// Última posição de cada eixo, para calcular o ângulo dos sticks.
    axes: HashMap<Axis, f32>,
}

impl Mapper {
    pub fn new(profile: Profile) -> Self {
        Mapper {
            profile,
            active: HashSet::new(),
            held: HashMap::new(),
            axes: HashMap::new(),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Troca o perfil soltando antes tudo o que estava pressionado.
    pub fn set_profile(&mut self, profile: Profile) -> Vec<KeyAction> {
        let out = self.release_all();
        self.profile = profile;
        out
    }

    pub fn handle(&mut self, event: InputEvent) -> Vec<KeyAction> {
        let mut out = Vec::new();
        match event {
            InputEvent::ButtonDown(b) => self.activate(Source::Button(b), &mut out),
            InputEvent::ButtonUp(b) => self.deactivate(Source::Button(b), &mut out),
            InputEvent::AxisMotion(axis, value) => {
                self.axes.insert(axis, value);
                self.axis(axis, value, &mut out);
                self.sticks_on(axis, &mut out);
            }
        }
        out
    }

    /// Solta todas as teclas pressionadas (ao desconectar o controle ou sair).
    pub fn release_all(&mut self) -> Vec<KeyAction> {
        let mut out = Vec::new();
        let sources: Vec<Source> = self.active.iter().copied().collect();
        for s in sources {
            self.deactivate(s, &mut out);
        }
        out
    }

    fn axis(&mut self, axis: Axis, value: f32, out: &mut Vec<KeyAction>) {
        let Some(binding) = self.profile.axes.get(&axis) else {
            return;
        };
        let press_at = binding
            .threshold
            .unwrap_or(self.profile.settings.axis_threshold);
        let release_at = press_at * self.profile.settings.release_ratio;

        for (source, magnitude) in [
            (Source::AxisNeg(axis), -value),
            (Source::AxisPos(axis), value),
        ] {
            let is_active = self.active.contains(&source);
            if !is_active && magnitude >= press_at {
                self.activate(source, out);
            } else if is_active && magnitude < release_at {
                self.deactivate(source, out);
            }
        }
    }

    fn sticks_on(&mut self, axis: Axis, out: &mut Vec<KeyAction>) {
        let sticks: Vec<Stick> = self
            .profile
            .sticks
            .iter()
            .filter(|(s, b)| {
                let (x, y) = b.axes(**s);
                x == axis || y == axis
            })
            .map(|(s, _)| *s)
            .collect();
        for stick in sticks {
            self.stick(stick, out);
        }
    }

    fn stick(&mut self, stick: Stick, out: &mut Vec<KeyAction>) {
        let binding = &self.profile.sticks[&stick];
        let (ax, ay) = binding.axes(stick);
        let (x, y) = (
            self.axes.get(&ax).copied().unwrap_or(0.0),
            self.axes.get(&ay).copied().unwrap_or(0.0),
        );
        let press_at = self.profile.deadzone(stick);
        let release_at = press_at * self.profile.settings.release_ratio;
        let (r, angle) = (x.hypot(y), angle_of(x, y));

        let mut on = Vec::new();
        let mut off = Vec::new();
        for i in 0..binding.sectors.len() {
            let source = Source::Sector(stick, i);
            if self.active.contains(&source) {
                if r < release_at || !binding.contains(i, angle, SECTOR_HYSTERESIS) {
                    off.push(source);
                }
            } else if r >= press_at && binding.contains(i, angle, -SECTOR_HYSTERESIS) {
                on.push(source);
            }
        }
        // Pressiona antes de soltar: indo de W para W+D, W não é solta no meio.
        for s in on {
            self.activate(s, out);
        }
        for s in off {
            self.deactivate(s, out);
        }
    }

    fn combo_for(&self, source: Source) -> Option<KeyCombo> {
        match source {
            Source::Button(b) => self.profile.buttons.get(&b).cloned(),
            Source::AxisNeg(a) => self.profile.axes.get(&a)?.negative.clone(),
            Source::AxisPos(a) => self.profile.axes.get(&a)?.positive.clone(),
            Source::Sector(s, i) => self.profile.sticks.get(&s)?.sectors.get(i)?.key.clone(),
        }
    }

    fn activate(&mut self, source: Source, out: &mut Vec<KeyAction>) {
        let Some(combo) = self.combo_for(source) else {
            return;
        };
        if !self.active.insert(source) {
            return;
        }
        for &key in combo.keys() {
            let count = self.held.entry(key).or_insert(0);
            *count += 1;
            if *count == 1 {
                out.push(KeyAction::Press(key));
            }
        }
    }

    fn deactivate(&mut self, source: Source, out: &mut Vec<KeyAction>) {
        if !self.active.remove(&source) {
            return;
        }
        let Some(combo) = self.combo_for(source) else {
            return;
        };
        for &key in combo.keys().iter().rev() {
            if let Some(count) = self.held.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    self.held.remove(&key);
                    out.push(KeyAction::Release(key));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyAction::{Press, Release};

    fn mapper(toml: &str) -> Mapper {
        Mapper::new(Profile::from_toml(toml).unwrap())
    }

    #[test]
    fn button_press_and_release() {
        let mut m = mapper("name='t'\n[buttons]\nsouth='Space'\n");
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::South)),
            [Press(Key::Space)]
        );
        assert_eq!(
            m.handle(InputEvent::ButtonUp(Button::South)),
            [Release(Key::Space)]
        );
    }

    #[test]
    fn unmapped_button_is_ignored() {
        let mut m = mapper("name='t'\n");
        assert!(m.handle(InputEvent::ButtonDown(Button::North)).is_empty());
        assert!(m.handle(InputEvent::ButtonUp(Button::North)).is_empty());
    }

    #[test]
    fn combo_releases_in_reverse_order() {
        let mut m = mapper("name='t'\n[buttons]\nstart='Ctrl+S'\n");
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::Start)),
            [Press(Key::LeftCtrl), Press(Key::S)]
        );
        assert_eq!(
            m.handle(InputEvent::ButtonUp(Button::Start)),
            [Release(Key::S), Release(Key::LeftCtrl)]
        );
    }

    #[test]
    fn shared_key_is_reference_counted() {
        let mut m = mapper("name='t'\n[buttons]\nwest='Shift'\nleft_shoulder='Shift'\n");
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::West)),
            [Press(Key::LeftShift)]
        );
        assert!(m
            .handle(InputEvent::ButtonDown(Button::LeftShoulder))
            .is_empty());
        assert!(m.handle(InputEvent::ButtonUp(Button::West)).is_empty());
        assert_eq!(
            m.handle(InputEvent::ButtonUp(Button::LeftShoulder)),
            [Release(Key::LeftShift)]
        );
    }

    #[test]
    fn axis_threshold_with_hysteresis() {
        let mut m = mapper("name='t'\n[axes.left_x]\nnegative='A'\npositive='D'\n");
        let mv = |m: &mut Mapper, v| m.handle(InputEvent::AxisMotion(Axis::LeftX, v));
        assert!(mv(&mut m, 0.3).is_empty());
        assert_eq!(mv(&mut m, 0.6), [Press(Key::D)]);
        // Entre 0.4 (0.5 * 0.8) e 0.5 continua pressionada.
        assert!(mv(&mut m, 0.45).is_empty());
        assert_eq!(mv(&mut m, 0.2), [Release(Key::D)]);
        // Virar direto para o outro lado solta um e pressiona o outro.
        assert_eq!(mv(&mut m, 0.9), [Press(Key::D)]);
        assert_eq!(mv(&mut m, -0.9), [Press(Key::A), Release(Key::D)]);
    }

    #[test]
    fn stick_sectors_with_hysteresis() {
        let mut m = mapper(
            "name='t'
[sticks.left]
sectors=[{start=45.0,key='W'},{start=135.0,key='A'},{start=225.0,key='S'},{start=315.0,key='D'}]
",
        );
        let mv = |m: &mut Mapper, x: f32, y: f32| {
            let mut out = m.handle(InputEvent::AxisMotion(Axis::LeftX, x));
            out.extend(m.handle(InputEvent::AxisMotion(Axis::LeftY, y)));
            out
        };
        assert!(mv(&mut m, 0.0, -0.3).is_empty());
        assert_eq!(mv(&mut m, 0.0, -0.9), [Press(Key::W)]);
        // Um pouco depois da divisa de 45° continua em W.
        let a = 43f32.to_radians();
        assert!(mv(&mut m, a.cos(), -a.sin()).is_empty());
        let a = 30f32.to_radians();
        assert_eq!(
            mv(&mut m, a.cos(), -a.sin()),
            [Press(Key::D), Release(Key::W)]
        );
        // Voltar ao centro: Y primeiro, então o stick passa por (0.87, 0), ainda D.
        assert!(m
            .handle(InputEvent::AxisMotion(Axis::LeftY, 0.0))
            .is_empty());
        assert_eq!(
            m.handle(InputEvent::AxisMotion(Axis::LeftX, 0.0)),
            [Release(Key::D)]
        );
    }

    #[test]
    fn shared_stick_presses_two_keys_on_diagonal() {
        let mut m = mapper(
            "name='t'
[sticks.right]
shared=true
sectors=[{start=45.0,key='Up'},{start=135.0},{start=225.0},{start=315.0,key='Right'}]
",
        );
        let mut out = m.handle(InputEvent::AxisMotion(Axis::RightY, -0.7));
        out.extend(m.handle(InputEvent::AxisMotion(Axis::RightX, 0.7)));
        out.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(out, [Press(Key::Right), Press(Key::Up)]);
    }

    #[test]
    fn release_all_clears_everything() {
        let mut m =
            mapper("name='t'\n[buttons]\nsouth='Space'\n[axes.right_trigger]\npositive='LShift'\n");
        m.handle(InputEvent::ButtonDown(Button::South));
        m.handle(InputEvent::AxisMotion(Axis::RightTrigger, 1.0));
        let mut released = m.release_all();
        released.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(released, [Release(Key::LeftShift), Release(Key::Space)]);
        assert!(m.release_all().is_empty());
    }
}
