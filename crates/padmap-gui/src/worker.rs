//! Thread que lê o controle e envia as teclas e o movimento do mouse.
//!
//! O mapeamento roda aqui, e não no loop da interface, porque a janela só é
//! redesenhada quando algo muda e fica parada quando está sem foco, que é
//! justamente quando o jogo está em primeiro plano.

use padmap_core::{InputEvent, KeyAction, Mapper, Profile};
use padmap_input::{GamepadSource, SourceEvent};
use padmap_output::{platform_sink, OutputSink};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Quanto o laço espera por um evento do controle. Com um stick em modo mouse
/// fora da zona morta a espera encurta, para o cursor andar liso (~250 Hz).
const IDLE_WAIT: Duration = Duration::from_millis(10);
const MOUSE_WAIT: Duration = Duration::from_millis(4);

pub enum Command {
    SetProfile(Box<Profile>),
    SetActive(bool),
    SetDryRun(bool),
    Quit,
}

pub enum Status {
    InitFailed(String),
    Connected {
        id: u32,
        name: String,
        touchpad: bool,
    },
    Disconnected {
        id: u32,
    },
    Input(InputEvent),
    Key {
        action: KeyAction,
        simulated: bool,
    },
    Error(String),
}

pub struct Worker {
    commands: Sender<Command>,
    pub status: Receiver<Status>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(profile: Profile, repaint: impl Fn() + Send + 'static) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (st_tx, st_rx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("padmap-input".into())
            .spawn(move || run(profile, cmd_rx, st_tx, repaint))
            .expect("falha ao criar a thread de entrada");
        Worker {
            commands: cmd_tx,
            status: st_rx,
            handle: Some(handle),
        }
    }

    pub fn send(&self, command: Command) {
        // Se a thread já terminou (ex.: o SDL não iniciou), não há a quem avisar.
        let _ = self.commands.send(command);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.send(Command::Quit);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct State {
    mapper: Mapper,
    active: bool,
    dry_run: bool,
    sink: Option<Box<dyn OutputSink>>,
    status: Sender<Status>,
}

impl State {
    /// O sink nativo, criado na primeira vez. Se falhar, desativa o mapeamento.
    fn sink(&mut self) -> Option<&mut Box<dyn OutputSink>> {
        if self.sink.is_none() {
            match platform_sink() {
                Ok(s) => self.sink = Some(s),
                Err(e) => {
                    let _ = self.status.send(Status::Error(e.to_string()));
                    self.active = false;
                    return None;
                }
            }
        }
        self.sink.as_mut()
    }

    fn emit(&mut self, actions: Vec<KeyAction>) {
        for action in actions {
            if !self.dry_run {
                let Some(sink) = self.sink() else {
                    return;
                };
                if let Err(e) = sink.send(action) {
                    let _ = self.status.send(Status::Error(e.to_string()));
                }
            }
            let _ = self.status.send(Status::Key {
                action,
                simulated: self.dry_run,
            });
        }
    }

    fn release_all(&mut self) {
        let actions = self.mapper.release_all();
        self.emit(actions);
    }

    /// Anda o mouse pelo tempo que passou. Roda a cada volta do laço, com ou
    /// sem evento, porque o stick parado inclinado não gera eventos.
    fn move_mouse(&mut self, dt: Duration) {
        let (dx, dy) = self.mapper.tick(dt);
        if !self.active || self.dry_run || (dx, dy) == (0, 0) {
            return;
        }
        let Some(sink) = self.sink() else {
            return;
        };
        if let Err(e) = sink.move_mouse(dx, dy) {
            // Desativa para não repetir o mesmo erro centenas de vezes por segundo.
            let _ = self.status.send(Status::Error(e.to_string()));
            self.active = false;
        }
    }
}

fn run(profile: Profile, commands: Receiver<Command>, status: Sender<Status>, repaint: impl Fn()) {
    let mut source = match GamepadSource::new() {
        Ok(s) => s,
        Err(e) => {
            let _ = status.send(Status::InitFailed(e));
            repaint();
            return;
        }
    };
    let mut st = State {
        mapper: Mapper::new(profile),
        active: false,
        dry_run: true,
        sink: None,
        status: status.clone(),
    };
    let mut last_tick = Instant::now();

    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::SetProfile(p)) => {
                    let released = st.mapper.set_profile(*p);
                    st.emit(released);
                }
                Ok(Command::SetActive(on)) => {
                    if !on {
                        st.release_all();
                    }
                    st.active = on;
                }
                Ok(Command::SetDryRun(on)) => {
                    // Solta antes de trocar, para não deixar tecla presa no SO.
                    st.release_all();
                    st.dry_run = on;
                }
                Ok(Command::Quit) | Err(TryRecvError::Disconnected) => {
                    st.release_all();
                    return;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        let wait = if st.active && st.mapper.mouse_moving() {
            MOUSE_WAIT
        } else {
            IDLE_WAIT
        };
        let event = source.next_event(wait);
        let now = Instant::now();
        st.move_mouse(now - last_tick);
        last_tick = now;
        let Some(event) = event else {
            continue;
        };
        match event {
            SourceEvent::Connected { id, name, touchpad } => {
                let _ = status.send(Status::Connected { id, name, touchpad });
            }
            SourceEvent::Disconnected { id } => {
                st.release_all();
                let _ = status.send(Status::Disconnected { id });
            }
            SourceEvent::Input { event, .. } => {
                let _ = status.send(Status::Input(event));
                if st.active {
                    let actions = st.mapper.handle(event);
                    st.emit(actions);
                }
            }
            SourceEvent::Quit => {}
        }
        repaint();
    }
}
