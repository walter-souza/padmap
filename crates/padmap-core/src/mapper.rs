//! Motor de mapeamento: transforma eventos de controle em pressionar/soltar
//! teclas, de acordo com um perfil.
//!
//! As teclas têm contagem de referência: se dois botões estão ligados a
//! `Shift`, a tecla só é solta quando os dois forem soltos.

use crate::extra::ExtraSource;
use crate::gamepad::{Axis, Button, InputEvent};
use crate::key::{Key, KeyCombo};
use crate::profile::Profile;
use crate::stick::{angle_of, Stick, StickMode};
use crate::touchpad::{Swipe, TouchActivate, TouchMode, ASPECT};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

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
    Other(u8),
    /// Zona do touchpad acionada por um dedo: `(dedo, zona)`.
    Zone(u8, usize),
    /// Gesto de um dedo, apertado até o dedo sair.
    Swipe(u8, Swipe),
    /// Touchpad apertado no modo mouse; `true` com dois dedos encostados.
    TouchClick(bool),
}

/// Histerese angular, em graus: um setor só aciona com o stick essa folga para
/// dentro da borda e só solta com o stick essa folga para fora. Evita que a
/// tecla pisque, ou que dois setores vizinhos acionem juntos, na divisa.
const SECTOR_HYSTERESIS: f32 = 3.0;

/// Maior intervalo que um `tick` considera. Depois de uma pausa longa (a thread
/// atrasou, o PC travou) o cursor não dá um salto.
const MAX_TICK: Duration = Duration::from_millis(50);

pub struct Mapper {
    profile: Profile,
    active: HashSet<Source>,
    held: HashMap<Key, u32>,
    /// Última posição de cada eixo, para calcular o ângulo dos sticks.
    axes: HashMap<Axis, f32>,
    /// Frações de pixel que sobraram dos `tick`s anteriores, para o cursor
    /// andar mesmo bem devagar.
    mouse_rest: (f32, f32),
    /// Dedos no touchpad agora e onde estão.
    fingers: HashMap<u8, (f32, f32)>,
    /// Onde cada dedo encostou, para medir os gestos.
    touch_start: HashMap<u8, (f32, f32)>,
    /// O touchpad está apertado.
    touch_clicked: bool,
    /// No modo mouse, o dedo que move o cursor: o primeiro a encostar.
    mouse_finger: Option<u8>,
    /// Pixels que o dedo andou desde o último `tick`, no modo mouse.
    touch_mouse: (f32, f32),
}

impl Mapper {
    pub fn new(profile: Profile) -> Self {
        Mapper {
            profile,
            active: HashSet::new(),
            held: HashMap::new(),
            axes: HashMap::new(),
            mouse_rest: (0.0, 0.0),
            fingers: HashMap::new(),
            touch_start: HashMap::new(),
            touch_clicked: false,
            mouse_finger: None,
            touch_mouse: (0.0, 0.0),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Troca o perfil soltando antes tudo o que estava pressionado.
    pub fn set_profile(&mut self, profile: Profile) -> Vec<KeyAction> {
        let out = self.release_all();
        self.profile = profile;
        self.mouse_rest = (0.0, 0.0);
        self.mouse_finger = None;
        self.touch_mouse = (0.0, 0.0);
        out
    }

    /// Movimento do mouse, em pixels inteiros, desde o último `tick`. O stick
    /// parado inclinado não gera eventos, então quem roda o `Mapper` chama isto
    /// a cada volta do laço, passando o tempo que passou. O que o dedo andou no
    /// touchpad em modo mouse entra junto.
    pub fn tick(&mut self, dt: Duration) -> (i32, i32) {
        let dt = dt.min(MAX_TICK).as_secs_f32();
        let (mut vx, mut vy) = (0.0, 0.0);
        for (&stick, b) in &self.profile.sticks {
            if b.mode != StickMode::Mouse {
                continue;
            }
            let (x, y) = self.stick_pos(stick);
            let (sx, sy) = b.mouse.velocity(x, y, self.profile.deadzone(stick));
            vx += sx;
            vy += sy;
        }
        let (tx, ty) = std::mem::take(&mut self.touch_mouse);
        if vx == 0.0 && vy == 0.0 && tx == 0.0 && ty == 0.0 {
            self.mouse_rest = (0.0, 0.0);
            return (0, 0);
        }
        let x = self.mouse_rest.0 + vx * dt + tx;
        let y = self.mouse_rest.1 + vy * dt + ty;
        let (dx, dy) = (x.trunc(), y.trunc());
        self.mouse_rest = (x - dx, y - dy);
        (dx as i32, dy as i32)
    }

    /// Se algum stick em modo mouse está fora da zona morta, ou um dedo move o
    /// cursor pelo touchpad. Enquanto isso, vale chamar `tick` com mais
    /// frequência para o cursor andar liso.
    pub fn mouse_moving(&self) -> bool {
        self.mouse_finger.is_some()
            || self.profile.sticks.iter().any(|(&stick, b)| {
                let (x, y) = self.stick_pos(stick);
                b.mode == StickMode::Mouse && x.hypot(y) > self.profile.deadzone(stick)
            })
    }

    fn stick_pos(&self, stick: Stick) -> (f32, f32) {
        let (ax, ay) = self.profile.sticks[&stick].axes(stick);
        (
            self.axes.get(&ax).copied().unwrap_or(0.0),
            self.axes.get(&ay).copied().unwrap_or(0.0),
        )
    }

    pub fn handle(&mut self, event: InputEvent) -> Vec<KeyAction> {
        let mut out = Vec::new();
        match event {
            InputEvent::ButtonDown(b) => {
                if b == Button::Touchpad {
                    self.touch_click(true, &mut out);
                }
                self.activate(Source::Button(b), &mut out);
            }
            InputEvent::ButtonUp(b) => {
                if b == Button::Touchpad {
                    self.touch_click(false, &mut out);
                }
                self.deactivate(Source::Button(b), &mut out);
            }
            InputEvent::TouchDown(f, x, y) => self.touch_down(f, (x, y), &mut out),
            InputEvent::TouchMove(f, x, y) => self.touch_move(f, (x, y), &mut out),
            InputEvent::TouchUp(f) => self.touch_up(f, &mut out),
            InputEvent::OtherButtonDown(n) => self.activate(Source::Other(n), &mut out),
            InputEvent::OtherButtonUp(n) => self.deactivate(Source::Other(n), &mut out),
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

    fn touch_down(&mut self, f: u8, pos: (f32, f32), out: &mut Vec<KeyAction>) {
        self.fingers.insert(f, pos);
        self.touch_start.insert(f, pos);
        let Some(tp) = &self.profile.touchpad else {
            return;
        };
        match tp.mode {
            // Um dedo que encosta com o touchpad já apertado também conta: o
            // SDL pode mandar o clique antes do toque.
            TouchMode::Zones => {
                if tp.activate == TouchActivate::Touch || self.touch_clicked {
                    self.press_zone(f, out);
                }
            }
            TouchMode::Mouse => {
                self.mouse_finger.get_or_insert(f);
            }
            TouchMode::Swipe => {}
        }
    }

    fn touch_move(&mut self, f: u8, pos: (f32, f32), out: &mut Vec<KeyAction>) {
        let prev = self.fingers.insert(f, pos);
        let Some(tp) = &self.profile.touchpad else {
            return;
        };
        match tp.mode {
            // A zona fica a mesma até o dedo sair: deslizar pela divisa não
            // troca a tecla no meio.
            TouchMode::Zones => {}
            TouchMode::Mouse => {
                if let (Some(p), true) = (prev, self.mouse_finger == Some(f)) {
                    let k = tp.mouse.speed;
                    self.touch_mouse.0 += (pos.0 - p.0) * k;
                    self.touch_mouse.1 += (pos.1 - p.1) * k * ASPECT;
                }
            }
            // Um gesto por toque: reconhecido, fica apertado até o dedo sair.
            TouchMode::Swipe => {
                if self
                    .active
                    .iter()
                    .any(|s| matches!(s, Source::Swipe(g, _) if *g == f))
                {
                    return;
                }
                let dir = self
                    .touch_start
                    .get(&f)
                    .and_then(|&start| tp.swipe.direction(start, pos));
                if let Some(dir) = dir {
                    self.activate(Source::Swipe(f, dir), out);
                }
            }
        }
    }

    fn touch_up(&mut self, f: u8, out: &mut Vec<KeyAction>) {
        self.fingers.remove(&f);
        self.touch_start.remove(&f);
        if self.mouse_finger == Some(f) {
            // Outro dedo encostado continua movendo o cursor.
            self.mouse_finger = self.fingers.keys().next().copied();
        }
        self.release_where(
            |s| matches!(s, Source::Zone(g, _) | Source::Swipe(g, _) if g == f),
            out,
        );
    }

    /// Apertar o touchpad: nas zonas "ao clicar" aciona a zona sob cada dedo;
    /// no mouse, clica (com dois dedos, o outro clique). Soltar solta tudo.
    fn touch_click(&mut self, down: bool, out: &mut Vec<KeyAction>) {
        self.touch_clicked = down;
        let Some(tp) = &self.profile.touchpad else {
            return;
        };
        match (tp.mode, down) {
            (TouchMode::Zones, true) if tp.activate == TouchActivate::Click => {
                let fingers: Vec<u8> = self.fingers.keys().copied().collect();
                for f in fingers {
                    self.press_zone(f, out);
                }
            }
            (TouchMode::Zones, false) if tp.activate == TouchActivate::Click => {
                self.release_where(|s| matches!(s, Source::Zone(..)), out);
            }
            (TouchMode::Mouse, true) => {
                let two = self.fingers.len() >= 2;
                self.activate(Source::TouchClick(two), out);
            }
            (TouchMode::Mouse, false) => {
                self.release_where(|s| matches!(s, Source::TouchClick(_)), out);
            }
            _ => {}
        }
    }

    fn press_zone(&mut self, finger: u8, out: &mut Vec<KeyAction>) {
        let (Some(tp), Some(&(x, y))) = (&self.profile.touchpad, self.fingers.get(&finger)) else {
            return;
        };
        let zone = tp.zone_at(x, y);
        self.activate(Source::Zone(finger, zone), out);
    }

    fn release_where(&mut self, which: impl Fn(Source) -> bool, out: &mut Vec<KeyAction>) {
        let sources: Vec<Source> = self.active.iter().copied().filter(|&s| which(s)).collect();
        for s in sources {
            self.deactivate(s, out);
        }
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
        if binding.mode == StickMode::Mouse {
            return;
        }
        let (x, y) = self.stick_pos(stick);
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
            Source::Button(b) => self
                .profile
                .buttons
                .get(&b)
                .cloned()
                .or_else(|| self.extra_key(ExtraSource::Known(b))),
            Source::Other(n) => self.extra_key(ExtraSource::Other(n)),
            Source::AxisNeg(a) => self.profile.axes.get(&a)?.negative.clone(),
            Source::AxisPos(a) => self.profile.axes.get(&a)?.positive.clone(),
            Source::Sector(s, i) => self.profile.sticks.get(&s)?.sectors.get(i)?.key.clone(),
            Source::Zone(_, z) => self.profile.touchpad.as_ref()?.zones.get(z)?.key.clone(),
            Source::Swipe(_, d) => self.profile.touchpad.as_ref()?.swipe.key(d).cloned(),
            Source::TouchClick(two) => {
                let m = &self.profile.touchpad.as_ref()?.mouse;
                if two {
                    m.two_finger_click.clone()
                } else {
                    m.click.clone()
                }
            }
        }
    }

    fn extra_key(&self, source: ExtraSource) -> Option<KeyCombo> {
        self.profile
            .extras
            .iter()
            .find(|e| e.button == source)?
            .key
            .clone()
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

    const MOUSE_STICK: &str = "name='t'
[sticks.right]
mode='mouse'
deadzone=0.2
mouse={speed=1000.0,curve=1.0}
sectors=[{start=45.0,key='Up'}]
";

    #[test]
    fn mouse_stick_moves_by_time_not_keys() {
        let mut m = mapper(MOUSE_STICK);
        assert!(m
            .handle(InputEvent::AxisMotion(Axis::RightX, 1.0))
            .is_empty());
        assert!(m.mouse_moving());
        assert_eq!(m.tick(Duration::from_millis(10)), (10, 0));
        // Parado inclinado continua andando a cada tick.
        assert_eq!(m.tick(Duration::from_millis(10)), (10, 0));
        // Um tick longo demais é limitado.
        assert_eq!(m.tick(Duration::from_secs(1)), (50, 0));
        m.handle(InputEvent::AxisMotion(Axis::RightX, 0.0));
        assert!(!m.mouse_moving());
        assert_eq!(m.tick(Duration::from_millis(10)), (0, 0));
    }

    #[test]
    fn slow_mouse_accumulates_fractions() {
        let mut m = mapper(MOUSE_STICK);
        // Força 0.1 → 100 px/s → 0.1 px a cada ms.
        m.handle(InputEvent::AxisMotion(Axis::RightY, 0.28));
        let total: i32 = (0..25).map(|_| m.tick(Duration::from_millis(1)).1).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn mouse_click_is_counted_like_a_key() {
        let mut m = mapper("name='t'\n[buttons]\nsouth='MouseLeft'\neast='MouseLeft'\n");
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::South)),
            [Press(Key::MouseLeft)]
        );
        assert!(m.handle(InputEvent::ButtonDown(Button::East)).is_empty());
        assert!(m.handle(InputEvent::ButtonUp(Button::South)).is_empty());
        // Desativar no meio do arrasto solta o clique.
        assert_eq!(m.release_all(), [Release(Key::MouseLeft)]);
    }

    #[test]
    fn extra_buttons_map_like_any_button() {
        let mut m = mapper(
            "name='t'
[[extras]]
button='other_11'
key='F'
[[extras]]
button='left_paddle1'
key='G'
[[extras]]
button='other_12'
",
        );
        assert_eq!(m.handle(InputEvent::OtherButtonDown(11)), [Press(Key::F)]);
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::LeftPaddle1)),
            [Press(Key::G)]
        );
        // Sem tecla, ou fora da lista: nada.
        assert!(m.handle(InputEvent::OtherButtonDown(12)).is_empty());
        assert!(m.handle(InputEvent::OtherButtonDown(30)).is_empty());
        let mut released = m.release_all();
        released.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(released, [Release(Key::F), Release(Key::G)]);
    }

    const TOUCHPAD: &str = "name='t'
[touchpad]
cols=2
rows=1
zones=[{key='Q'},{key='E'}]
";

    #[test]
    fn touchpad_zone_fires_on_click() {
        let mut m = mapper(TOUCHPAD);
        // Só encostar não aciona.
        assert!(m.handle(InputEvent::TouchDown(0, 0.8, 0.5)).is_empty());
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::Touchpad)),
            [Press(Key::E)]
        );
        // Deslizar para a outra zona com o touchpad apertado mantém a tecla.
        assert!(m.handle(InputEvent::TouchMove(0, 0.2, 0.5)).is_empty());
        assert_eq!(
            m.handle(InputEvent::ButtonUp(Button::Touchpad)),
            [Release(Key::E)]
        );
        // Encostar e apertar no mesmo instante: o clique chega antes do dedo.
        m.handle(InputEvent::TouchUp(0));
        assert!(m
            .handle(InputEvent::ButtonDown(Button::Touchpad))
            .is_empty());
        assert_eq!(
            m.handle(InputEvent::TouchDown(1, 0.1, 0.5)),
            [Press(Key::Q)]
        );
        assert_eq!(m.release_all(), [Release(Key::Q)]);
    }

    #[test]
    fn touchpad_zone_fires_on_touch_per_finger() {
        let mut m = mapper(&TOUCHPAD.replace("rows=1", "rows=1\nactivate='touch'"));
        assert_eq!(
            m.handle(InputEvent::TouchDown(0, 0.1, 0.5)),
            [Press(Key::Q)]
        );
        assert_eq!(
            m.handle(InputEvent::TouchDown(1, 0.9, 0.5)),
            [Press(Key::E)]
        );
        assert_eq!(m.handle(InputEvent::TouchUp(0)), [Release(Key::Q)]);
        assert_eq!(m.handle(InputEvent::TouchUp(1)), [Release(Key::E)]);
    }

    #[test]
    fn touchpad_mouse_follows_the_first_finger() {
        let mut m = mapper("name='t'\n[touchpad]\nmode='mouse'\nmouse={speed=1000.0}\n");
        m.handle(InputEvent::TouchDown(0, 0.5, 0.5));
        assert!(m.mouse_moving());
        m.handle(InputEvent::TouchMove(0, 0.75, 0.25));
        // Um segundo dedo não move o cursor.
        m.handle(InputEvent::TouchDown(1, 0.1, 0.1));
        m.handle(InputEvent::TouchMove(1, 0.75, 0.75));
        // 1/4 da largura → 250 px; -1/4 da altura → -125 px (o touchpad é 2:1).
        assert_eq!(m.tick(Duration::ZERO), (250, -125));
        assert_eq!(m.tick(Duration::ZERO), (0, 0));
        // Sem cliques definidos, valem os do mouse: dois dedos, botão direito.
        assert_eq!(
            m.handle(InputEvent::ButtonDown(Button::Touchpad)),
            [Press(Key::MouseRight)]
        );
        assert_eq!(
            m.handle(InputEvent::ButtonUp(Button::Touchpad)),
            [Release(Key::MouseRight)]
        );
        // Tirando o primeiro dedo, o outro assume.
        m.handle(InputEvent::TouchUp(0));
        m.handle(InputEvent::TouchMove(1, 0.5, 0.75));
        assert_eq!(m.tick(Duration::ZERO), (-250, 0));
        m.handle(InputEvent::TouchUp(1));
        assert!(!m.mouse_moving());
    }

    #[test]
    fn touchpad_swipe_holds_until_lift() {
        let mut m = mapper("name='t'\n[touchpad]\nmode='swipe'\nswipe={right='E',up='M'}\n");
        m.handle(InputEvent::TouchDown(0, 0.2, 0.5));
        assert!(m.handle(InputEvent::TouchMove(0, 0.35, 0.5)).is_empty());
        assert_eq!(
            m.handle(InputEvent::TouchMove(0, 0.5, 0.5)),
            [Press(Key::E)]
        );
        // Voltando no mesmo toque não aciona outro gesto.
        assert!(m.handle(InputEvent::TouchMove(0, 0.5, 0.0)).is_empty());
        assert_eq!(m.handle(InputEvent::TouchUp(0)), [Release(Key::E)]);
        // O clique do touchpad continua livre neste modo.
        assert!(m
            .handle(InputEvent::ButtonDown(Button::Touchpad))
            .is_empty());
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
