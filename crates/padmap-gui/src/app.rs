use crate::capture::Capture;
use crate::diagram::{self, Click, Live, Stick};
use crate::slots::Slot;
use crate::theme::{self, Palette, ACCENT, ACCENT_STRONG, BAD, GOOD};
use crate::worker::{Command, Status, Worker};
use eframe::egui::{self, vec2, Color32, Margin, RichText, Sense, Stroke, StrokeKind, Theme};
use padmap_core::{InputEvent, Key, KeyAction, KeyCombo, Profile};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

const DEFAULT_PROFILE: &str = include_str!("../../../profiles/default.toml");
const LOG_LINES: usize = 300;
const THEME_KEY: &str = "theme";

pub struct PadmapApp {
    worker: Worker,
    profile: Profile,
    path: Option<PathBuf>,
    dirty: bool,
    capture: Option<(Slot, Capture)>,
    /// Editor de direções aberto. Fica escondido enquanto uma captura acontece.
    stick_editor: Option<Stick>,
    active: bool,
    dry_run: bool,
    pads: BTreeMap<u32, String>,
    live: Live,
    held: BTreeSet<Key>,
    log: VecDeque<String>,
    show_log: bool,
    error: Option<String>,
    theme: Theme,
}

impl PadmapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let theme = cc
            .storage
            .and_then(|s| s.get_string(THEME_KEY))
            .and_then(|name| theme::from_name(&name))
            .or_else(|| cc.egui_ctx.system_theme())
            .unwrap_or(Theme::Light);
        cc.egui_ctx.set_theme(theme);

        let profile = Profile::from_toml(DEFAULT_PROFILE).expect("perfil padrão válido");
        let ctx = cc.egui_ctx.clone();
        let worker = Worker::spawn(profile.clone(), move || ctx.request_repaint());
        PadmapApp {
            worker,
            profile,
            path: None,
            dirty: false,
            capture: None,
            stick_editor: None,
            active: false,
            dry_run: true,
            pads: BTreeMap::new(),
            live: Live::default(),
            held: BTreeSet::new(),
            log: VecDeque::new(),
            show_log: false,
            error: None,
            theme,
        }
    }

    fn profile_changed(&mut self) {
        self.dirty = true;
        self.worker.send(Command::SetProfile(self.profile.clone()));
    }

    fn set_slot(&mut self, slot: Slot, combo: Option<KeyCombo>) {
        slot.set(&mut self.profile, combo);
        self.profile_changed();
    }

    fn replace_profile(&mut self, profile: Profile, path: Option<PathBuf>) {
        self.profile = profile;
        self.path = path;
        self.dirty = false;
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

    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.theme = theme;
        ctx.set_theme(theme);
    }

    // ---------------------------------------------------------------- arquivos

    fn new_profile(&mut self) {
        let empty = Profile::from_toml("name = \"Novo perfil\"").unwrap();
        self.replace_profile(empty, None);
        self.dirty = true;
    }

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

    // ---------------------------------------------------------------- topo

    fn header(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("padmap").size(18.0).strong().color(pal.text));
            ui.add_space(14.0);

            let name = if self.dirty {
                format!("{}  ●", self.profile.name)
            } else {
                self.profile.name.clone()
            };
            ui.scope(|ui| {
                ui.spacing_mut().button_padding = vec2(12.0, 8.0);
                ui.menu_button(format!("{name}   ⏷"), |ui| {
                    ui.set_min_width(240.0);
                    ui.label(
                        RichText::new("Nome do perfil")
                            .small()
                            .color(pal.text_muted),
                    );
                    if ui.text_edit_singleline(&mut self.profile.name).changed() {
                        self.dirty = true;
                    }
                    if let Some(path) = &self.path {
                        ui.label(
                            RichText::new(path.display().to_string())
                                .small()
                                .color(pal.text_muted),
                        );
                    }
                    ui.separator();
                    if ui.button("Salvar como…").clicked() {
                        ui.close();
                        self.save(true);
                    }
                })
                .response
                .on_hover_text(if self.dirty {
                    "Alterações não salvas"
                } else {
                    "Perfil atual"
                });
                if bar_button(ui, "Novo").clicked() {
                    self.new_profile();
                }
                if bar_button(ui, "Abrir…").clicked() {
                    self.open();
                }
                if bar_button(ui, "Salvar").clicked() {
                    self.save(false);
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, fill) = if self.active {
                    ("■  Parar mapeamento", BAD)
                } else {
                    ("▶  Ativar mapeamento", ACCENT_STRONG)
                };
                let btn = egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
                    .fill(fill)
                    .stroke(Stroke::NONE)
                    .corner_radius(9)
                    .min_size(vec2(0.0, 38.0));
                if ui.add(btn).clicked() {
                    self.set_active(!self.active);
                }
                ui.add_space(8.0);

                let switch = switch(ui, pal, self.dry_run).on_hover_text(
                    "Ligado: só mostra no log as teclas que seriam enviadas.\n\
                     Desligado: aperta as teclas de verdade no sistema.",
                );
                if switch.clicked() {
                    self.dry_run = !self.dry_run;
                    self.worker.send(Command::SetDryRun(self.dry_run));
                }
                ui.label(RichText::new("Simulação").color(pal.free_text));
                ui.add_space(12.0);

                if let Some(t) = theme_toggle(ui, pal, self.theme) {
                    self.set_theme(ui.ctx(), t);
                }
                ui.add_space(12.0);

                let (dot, name) = match self.pads.values().next() {
                    Some(n) => (GOOD, n.as_str()),
                    None => (pal.text_muted, "Nenhum controle conectado"),
                };
                ui.label(RichText::new(name).color(pal.free_text));
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, dot);
            });
        });
    }

    // ---------------------------------------------------------------- rodapé

    fn footer(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("Teclas seguradas").color(pal.text_muted));
            if self.held.is_empty() {
                ui.label(RichText::new("nenhuma").color(pal.text_muted).weak());
            }
            for k in &self.held {
                egui::Frame::new()
                    .fill(ACCENT)
                    .corner_radius(5)
                    .inner_margin(Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(k.name().to_uppercase())
                                .size(11.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                    });
            }
            if let Some(e) = &self.error {
                ui.add_space(12.0);
                ui.label(RichText::new(format!("⚠ {e}")).color(BAD));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let log = if self.show_log { "Esconder log" } else { "Log" };
                if ui
                    .add(egui::Button::new(RichText::new(log).color(pal.free_text)).frame(false))
                    .clicked()
                {
                    self.show_log = !self.show_log;
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "Clique numa etiqueta e aperte a tecla. Botão direito deixa livre.",
                    )
                    .color(pal.text_muted),
                );
            });
        });
    }

    fn log_panel(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let state = match (self.active, self.dry_run) {
                (false, _) => RichText::new("Mapeamento parado").color(pal.text_muted),
                (true, true) => RichText::new("Ativo (simulação)").color(ACCENT),
                (true, false) => RichText::new("Ativo: enviando teclas").color(GOOD),
            };
            ui.label(state);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Limpar log").clicked() {
                    self.log.clear();
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log {
                    ui.label(RichText::new(line).monospace().color(pal.free_text));
                }
            });
    }

    // ---------------------------------------------------------------- janelas

    fn stick_modal(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let Some(stick) = self.stick_editor else {
            return;
        };
        if self.capture.is_some() {
            return;
        }
        let mut close = false;
        let mut action = None;
        let modal = egui::Modal::new(egui::Id::unique("padmap-stick")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            ui.heading(stick.title());
            ui.label(
                RichText::new("Tecla de cada direção e do clique do stick.").color(pal.text_muted),
            );
            ui.add_space(10.0);
            egui::Grid::new("stick-grid")
                .num_columns(3)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for (label, slot) in stick.slots() {
                        ui.label(label);
                        let text = slot
                            .get(&self.profile)
                            .map_or("LIVRE".to_owned(), |c| c.to_string().to_uppercase());
                        let mapped = slot.get(&self.profile).is_some();
                        egui::Frame::new()
                            .fill(if mapped { ACCENT } else { pal.chip_bg })
                            .stroke(Stroke::new(
                                1.0,
                                if mapped { ACCENT } else { pal.chip_border },
                            ))
                            .corner_radius(5)
                            .inner_margin(Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.set_min_width(60.0);
                                ui.label(RichText::new(text).size(11.0).strong().color(
                                    if mapped {
                                        Color32::WHITE
                                    } else {
                                        pal.chip_text
                                    },
                                ));
                            });
                        ui.horizontal(|ui| {
                            if ui.button("Capturar").clicked() {
                                action = Some(Click::Map(slot));
                            }
                            if ui
                                .add_enabled(mapped, egui::Button::new("Limpar"))
                                .clicked()
                            {
                                action = Some(Click::Clear(slot));
                            }
                        });
                        ui.end_row();
                    }
                });
            ui.add_space(10.0);
            if ui.button("Fechar").clicked() {
                close = true;
            }
        });
        if close || modal.should_close() {
            self.stick_editor = None;
        }
        match action {
            Some(Click::Map(slot)) => self.capture = Some((slot, Capture::default())),
            Some(Click::Clear(slot)) => self.set_slot(slot, None),
            _ => {}
        }
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

/// Botão da barra do topo: fundo da barra, borda fina, 34 px de altura.
fn bar_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(vec2(0.0, 34.0)))
}

/// Interruptor de liga/desliga desenhado à mão.
fn switch(ui: &mut egui::Ui, pal: &Palette, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    let t = ui.ctx().animate_bool(resp.id, on);
    let track = if on { ACCENT } else { pal.switch_off };
    ui.painter().rect_filled(rect, 10.0, track);
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE);
    resp
}

/// Par de botões sol / lua. Retorna o tema escolhido quando muda.
fn theme_toggle(ui: &mut egui::Ui, pal: &Palette, current: Theme) -> Option<Theme> {
    let (rect, _) = ui.allocate_exact_size(vec2(70.0, 34.0), Sense::hover());
    ui.painter().rect_filled(rect, 9.0, pal.toggle_bg);
    let mut chosen = None;
    // Da esquerda para a direita: claro, escuro.
    for (i, theme) in [Theme::Light, Theme::Dark].into_iter().enumerate() {
        let cell = egui::Rect::from_min_size(
            rect.min + vec2(3.0 + 32.0 * i as f32, 3.0),
            vec2(32.0, 28.0),
        );
        let label = match theme {
            Theme::Light => "Modo claro",
            Theme::Dark => "Modo escuro",
        };
        let resp = ui
            .interact(cell, ui.scope_id().with(("theme", i)), Sense::click())
            .on_hover_text(label);
        let selected = theme == current;
        if selected {
            ui.painter().rect(
                cell,
                6.0,
                pal.toggle_on,
                Stroke::new(1.0, pal.button_border),
                StrokeKind::Inside,
            );
        }
        let color = if selected || resp.hovered() {
            pal.text
        } else {
            pal.text_muted
        };
        let c = cell.center();
        match theme {
            Theme::Light => {
                ui.painter().circle_stroke(c, 3.5, Stroke::new(1.6, color));
                for k in 0..8 {
                    let a = std::f32::consts::FRAC_PI_4 * k as f32;
                    let d = vec2(a.cos(), a.sin());
                    ui.painter()
                        .line_segment([c + d * 5.8, c + d * 7.6], Stroke::new(1.6, color));
                }
            }
            Theme::Dark => {
                let bg = if selected {
                    pal.toggle_on
                } else {
                    pal.toggle_bg
                };
                ui.painter().circle_filled(c, 6.5, color);
                ui.painter().circle_filled(c + vec2(3.5, -3.0), 5.5, bg);
            }
        }
        if resp.clicked() && !selected {
            chosen = Some(theme);
        }
    }
    chosen
}

impl eframe::App for PadmapApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_worker();
        let pal = theme::palette(self.theme);

        let bar = |top: bool| {
            egui::Frame::new()
                .fill(pal.bar)
                .inner_margin(Margin::symmetric(28, if top { 11 } else { 8 }))
                .stroke(Stroke::new(1.0, pal.bar_border))
        };
        egui::Panel::top("header")
            .frame(bar(true))
            .show(ui, |ui| self.header(ui, pal));
        egui::Panel::bottom("footer")
            .frame(bar(false))
            .show(ui, |ui| self.footer(ui, pal));
        if self.show_log {
            egui::Panel::bottom("log")
                .resizable(true)
                .default_size(170.0)
                .frame(
                    egui::Frame::new()
                        .fill(pal.bar)
                        .inner_margin(Margin::symmetric(28, 4)),
                )
                .show(ui, |ui| self.log_panel(ui, pal));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal.bg)
                    .inner_margin(Margin::same(16)),
            )
            .show(ui, |ui| {
                let capturing = self.capture.as_ref().map(|(s, _)| *s);
                match diagram::show(ui, pal, &self.live, &self.profile, capturing) {
                    Some(Click::Map(slot)) => self.capture = Some((slot, Capture::default())),
                    Some(Click::Clear(slot)) => {
                        if slot.get(&self.profile).is_some() {
                            self.push_log(format!("{}: livre", slot.label()));
                            self.set_slot(slot, None);
                        }
                    }
                    Some(Click::EditStick(stick)) => self.stick_editor = Some(stick),
                    None => {}
                }
            });

        self.stick_modal(ui, pal);
        self.capture_modal(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, theme::name(self.theme).to_owned());
    }
}
