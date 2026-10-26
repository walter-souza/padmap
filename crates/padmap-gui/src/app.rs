use crate::capture::Capture;
use crate::diagram::{self, Live};
use crate::slots::{Slot, GROUPS};
use crate::worker::{Command, Status, Worker};
use eframe::egui::{self, Color32, RichText};
use padmap_core::{InputEvent, Key, KeyAction, KeyCombo, Profile};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;

const DEFAULT_PROFILE: &str = include_str!("../../../profiles/default.toml");
const LOG_LINES: usize = 300;
const ACCENT: Color32 = Color32::from_rgb(64, 160, 255);
const GOOD: Color32 = Color32::from_rgb(60, 170, 90);
const BAD: Color32 = Color32::from_rgb(220, 70, 70);

pub struct PadmapApp {
    worker: Worker,
    profile: Profile,
    path: Option<PathBuf>,
    dirty: bool,
    /// Texto de cada campo de tecla, que pode estar inválido enquanto o usuário digita.
    texts: HashMap<Slot, String>,
    capture: Option<(Slot, Capture)>,
    active: bool,
    dry_run: bool,
    pads: BTreeMap<u32, String>,
    live: Live,
    held: BTreeSet<Key>,
    log: VecDeque<String>,
    error: Option<String>,
}

impl PadmapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let profile = Profile::from_toml(DEFAULT_PROFILE).expect("perfil padrão válido");
        let ctx = cc.egui_ctx.clone();
        let worker = Worker::spawn(profile.clone(), move || ctx.request_repaint());
        let mut app = PadmapApp {
            worker,
            profile,
            path: None,
            dirty: false,
            texts: HashMap::new(),
            capture: None,
            active: false,
            dry_run: true,
            pads: BTreeMap::new(),
            live: Live::default(),
            held: BTreeSet::new(),
            log: VecDeque::new(),
            error: None,
        };
        app.refresh_texts();
        app
    }

    fn refresh_texts(&mut self) {
        self.texts = GROUPS
            .iter()
            .flat_map(|(_, slots)| slots.iter())
            .map(|&s| {
                (
                    s,
                    s.get(&self.profile)
                        .map_or(String::new(), |c| c.to_string()),
                )
            })
            .collect();
    }

    fn profile_changed(&mut self) {
        self.dirty = true;
        self.worker.send(Command::SetProfile(self.profile.clone()));
    }

    fn set_slot(&mut self, slot: Slot, combo: Option<KeyCombo>) {
        self.texts.insert(
            slot,
            combo.as_ref().map_or(String::new(), |c| c.to_string()),
        );
        slot.set(&mut self.profile, combo);
        self.profile_changed();
    }

    fn replace_profile(&mut self, profile: Profile, path: Option<PathBuf>) {
        self.profile = profile;
        self.path = path;
        self.dirty = false;
        self.refresh_texts();
        self.worker.send(Command::SetProfile(self.profile.clone()));
    }

    fn push_log(&mut self, line: String) {
        if self.log.len() >= LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    fn poll_worker(&mut self) {
        while let Ok(status) = self.worker.status.try_recv() {
            match status {
                Status::InitFailed(e) => {
                    self.error = Some(format!(
                        "Não foi possível iniciar a leitura de controles: {e}"
                    ))
                }
                Status::Connected { id, name } => {
                    self.push_log(format!("Controle conectado: {name}"));
                    self.pads.insert(id, name);
                }
                Status::Disconnected { id } => {
                    if let Some(name) = self.pads.remove(&id) {
                        self.push_log(format!("Controle desconectado: {name}"));
                    }
                    if self.pads.is_empty() {
                        self.live = Live::default();
                    }
                }
                Status::Input(event) => match event {
                    InputEvent::ButtonDown(b) => {
                        self.live.pressed.insert(b);
                    }
                    InputEvent::ButtonUp(b) => {
                        self.live.pressed.remove(&b);
                    }
                    InputEvent::AxisMotion(a, v) => {
                        self.live.axes.insert(a, v);
                    }
                },
                Status::Key { action, simulated } => {
                    let tag = if simulated { " (simulação)" } else { "" };
                    let line = match action {
                        KeyAction::Press(k) => {
                            self.held.insert(k);
                            format!("▼ {k}{tag}")
                        }
                        KeyAction::Release(k) => {
                            self.held.remove(&k);
                            format!("▲ {k}{tag}")
                        }
                    };
                    self.push_log(line);
                }
                Status::Error(e) => {
                    self.active = false;
                    self.error = Some(e);
                }
            }
        }
    }

    fn set_active(&mut self, on: bool) {
        self.active = on;
        self.worker.send(Command::SetActive(on));
        if on {
            self.error = None;
        }
    }

    // ---------------------------------------------------------------- arquivos

    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Perfil padmap", &["toml"])
            .pick_file()
        else {
            return;
        };
        match Profile::load(&path) {
            Ok(p) => {
                self.push_log(format!("Perfil aberto: {}", path.display()));
                self.replace_profile(p, Some(path));
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn save(&mut self, ask: bool) {
        let path = match (&self.path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let Some(p) = rfd::FileDialog::new()
                    .add_filter("Perfil padmap", &["toml"])
                    .set_file_name(format!("{}.toml", self.profile.name))
                    .save_file()
                else {
                    return;
                };
                p
            }
        };
        match std::fs::write(&path, self.profile.to_toml()) {
            Ok(()) => {
                self.push_log(format!("Perfil salvo: {}", path.display()));
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => self.error = Some(format!("Não foi possível salvar: {e}")),
        }
    }

    // ---------------------------------------------------------------- telas

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("padmap").strong());
            ui.separator();
            ui.label("Perfil:");
            if ui
                .add(egui::TextEdit::singleline(&mut self.profile.name).desired_width(180.0))
                .changed()
            {
                self.dirty = true;
            }
            if self.dirty {
                ui.label(RichText::new("● não salvo").color(Color32::from_rgb(230, 160, 40)));
            }
            if ui.button("Novo").clicked() {
                let empty = Profile::from_toml("name = \"Novo perfil\"").unwrap();
                self.replace_profile(empty, None);
                self.dirty = true;
            }
            if ui.button("Abrir…").clicked() {
                self.open();
            }
            if ui.button("Salvar").clicked() {
                self.save(false);
            }
            if ui.button("Salvar como…").clicked() {
                self.save(true);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, color) = if self.active {
                    ("■  Parar mapeamento", BAD)
                } else {
                    ("▶  Ativar mapeamento", GOOD)
                };
                let btn = egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
                    .fill(color)
                    .min_size(egui::vec2(190.0, 30.0));
                if ui.add(btn).clicked() {
                    self.set_active(!self.active);
                }
                if ui
                    .checkbox(&mut self.dry_run, "Simulação")
                    .on_hover_text(
                        "Ligado: só mostra no log as teclas que seriam enviadas.\n\
                         Desligado: aperta as teclas de verdade no sistema.",
                    )
                    .changed()
                {
                    self.worker.send(Command::SetDryRun(self.dry_run));
                }
            });
        });
        if let Some(path) = &self.path {
            ui.label(RichText::new(path.display().to_string()).small().weak());
        }
    }

    fn controller_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(RichText::new("Controles conectados").strong());
        if self.pads.is_empty() {
            ui.label(RichText::new("Nenhum. Conecte um controle por USB ou Bluetooth.").weak());
        }
        for name in self.pads.values() {
            ui.label(format!("🎮 {name}"));
        }
        ui.add_space(8.0);
        if let Some(slot) = diagram::show(ui, &self.live, &self.profile) {
            self.capture = Some((slot, Capture::default()));
        }
        ui.label(RichText::new("Clique numa parte do controle para escolher a tecla.").weak());

        ui.add_space(12.0);
        ui.label(RichText::new("Sensibilidade dos sticks e gatilhos").strong());
        let s = &mut self.profile.settings;
        let mut changed = ui
            .add(egui::Slider::new(&mut s.axis_threshold, 0.05..=1.0).text("aciona em"))
            .on_hover_text("Quanto do curso o stick ou gatilho precisa andar para apertar a tecla.")
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut s.release_ratio, 0.3..=1.0).text("solta em (× acima)"))
            .on_hover_text("A tecla só é solta abaixo deste ponto, para não ficar piscando.")
            .changed();
        if changed {
            self.profile_changed();
        }
    }

    fn mapping_table(&mut self, ui: &mut egui::Ui) {
        let threshold = self.profile.settings.axis_threshold;
        let mut start_capture = None;
        let mut updates = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (title, slots) in GROUPS {
                    ui.add_space(6.0);
                    ui.label(RichText::new(*title).strong().size(15.0));
                    egui::Grid::new(title)
                        .num_columns(4)
                        .striped(true)
                        .spacing([10.0, 6.0])
                        .min_col_width(20.0)
                        .show(ui, |ui| {
                            for &slot in *slots {
                                let on = self.live.is_active(slot, threshold);
                                let dot = if on {
                                    ACCENT
                                } else {
                                    ui.visuals().weak_text_color()
                                };
                                ui.label(RichText::new(if on { "●" } else { "○" }).color(dot));
                                let mut label = RichText::new(slot.label());
                                if on {
                                    label = label.color(ACCENT).strong();
                                }
                                // Largura fixa para alinhar as colunas entre os grupos.
                                ui.allocate_ui_with_layout(
                                    egui::vec2(200.0, 22.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_width(200.0);
                                        ui.label(label);
                                    },
                                );

                                let text = self.texts.entry(slot).or_default();
                                let parsed = if text.trim().is_empty() {
                                    Ok(None)
                                } else {
                                    text.parse::<KeyCombo>().map(Some)
                                };
                                let mut edit = egui::TextEdit::singleline(text)
                                    .desired_width(170.0)
                                    .hint_text("sem tecla");
                                if parsed.is_err() {
                                    edit = edit.text_color(BAD);
                                }
                                let resp = ui.add_sized([170.0, 22.0], edit);
                                if let Err(e) = &parsed {
                                    resp.clone().on_hover_text(e);
                                }
                                if resp.changed() {
                                    if let Ok(combo) = parsed {
                                        updates.push((slot, combo));
                                    }
                                }

                                ui.horizontal(|ui| {
                                    if ui
                                        .button("⌨ Capturar")
                                        .on_hover_text("Aperte a tecla ou combinação no teclado")
                                        .clicked()
                                    {
                                        start_capture = Some(slot);
                                    }
                                    if ui.button("✕").on_hover_text("Limpar").clicked() {
                                        updates.push((slot, None));
                                        self.texts.insert(slot, String::new());
                                    }
                                });
                                ui.end_row();
                            }
                        });
                }
                ui.add_space(12.0);
            });

        for (slot, combo) in updates {
            slot.set(&mut self.profile, combo);
            self.profile_changed();
        }
        if let Some(slot) = start_capture {
            self.capture = Some((slot, Capture::default()));
        }
    }

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Teclas seguradas:").strong());
            if self.held.is_empty() {
                ui.label(RichText::new("nenhuma").weak());
            }
            for k in &self.held {
                egui::Frame::new()
                    .fill(ACCENT)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(k.name()).color(Color32::WHITE).strong());
                    });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Limpar log").clicked() {
                    self.log.clear();
                }
                let state = match (self.active, self.dry_run) {
                    (false, _) => RichText::new("Mapeamento parado").weak(),
                    (true, true) => RichText::new("Ativo (simulação)").color(ACCENT),
                    (true, false) => RichText::new("Ativo: enviando teclas").color(GOOD),
                };
                ui.label(state);
            });
        });
        if let Some(e) = &self.error {
            ui.label(RichText::new(format!("⚠ {e}")).color(BAD));
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log {
                    ui.label(RichText::new(line).monospace());
                }
            });
    }

    fn capture_modal(&mut self, ui: &mut egui::Ui) {
        let Some((slot, capture)) = &mut self.capture else {
            return;
        };
        let slot = *slot;
        let events = ui.input(|i| i.events.clone());
        let result = capture.feed(&events);
        let preview = capture.preview();

        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("padmap-capture")).show(ui.ctx(), |ui| {
            ui.set_width(340.0);
            ui.heading(slot.label());
            ui.add_space(8.0);
            ui.label("Aperte a tecla ou combinação no teclado.");
            ui.label(
                RichText::new("Para usar só um modificador (Shift, Ctrl, Alt), aperte e solte.")
                    .small()
                    .weak(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(if preview.is_empty() {
                    "…".into()
                } else {
                    format!("{preview}+…")
                })
                .monospace()
                .size(18.0),
            );
            ui.add_space(8.0);
            if ui.button("Cancelar").clicked() {
                cancel = true;
            }
        });

        if let Some(combo) = result {
            self.capture = None;
            self.set_slot(slot, Some(combo));
        } else if cancel {
            self.capture = None;
        }
    }
}

impl eframe::App for PadmapApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_worker();

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::bottom("log")
            .resizable(true)
            .default_size(170.0)
            .show(ui, |ui| self.log_panel(ui));
        egui::Panel::left("controller")
            .resizable(false)
            .exact_size(440.0)
            .show(ui, |ui| self.controller_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.mapping_table(ui));

        self.capture_modal(ui);
    }
}
