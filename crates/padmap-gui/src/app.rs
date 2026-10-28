use crate::capture::{self, Capture};
use crate::diagram::{self, Click, Live};
use crate::extras;
use crate::profiles::{self, Entry, Library, DEFAULT_PROFILE};
use crate::slots::{extra_default_name, Slot, OFF_DIAGRAM};
use crate::stick_editor::{Outcome, StickEditor};
use crate::theme::{self, Palette, ACCENT, ACCENT_STRONG, ACTIVE, BAD, GOOD};
use crate::touchpad_editor::{self, TouchpadEditor};
use crate::worker::{Command, Status, Worker};
use eframe::egui::{self, vec2, Color32, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Theme};
use padmap_core::{
    Button, ExtraButton, ExtraSource, InputEvent, Key, KeyAction, KeyCombo, Profile,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

const LOG_LINES: usize = 300;
const THEME_KEY: &str = "theme";
const AUTOSAVE_KEY: &str = "autosave";
/// Caminho do último perfil aberto, para reabrir na próxima vez.
const LAST_PROFILE_KEY: &str = "last_profile";

pub struct PadmapApp {
    worker: Worker,
    profile: Profile,
    path: Option<PathBuf>,
    dirty: bool,
    /// Salvar sozinho a cada alteração do perfil.
    autosave: bool,
    /// Pasta de perfis. `None` se não deu para usá-la; aí salvar pergunta onde.
    library: Option<Library>,
    /// Perfis da pasta, relidos ao abrir o menu do perfil e após cada ação.
    entries: Vec<Entry>,
    /// Texto da busca no menu do perfil.
    search: String,
    /// Janela de renomear, excluir ou trocar com alterações pendentes.
    profile_dialog: Option<ProfileDialog>,
    capture: Option<(Slot, Capture)>,
    /// Editor de setores do stick, quando aberto.
    stick_editor: Option<StickEditor>,
    touchpad_editor: Option<TouchpadEditor>,
    /// Aba da área central: o desenho do controle ou os botões extras.
    tab: Tab,
    /// Esperando um botão do controle para a lista de extras. Guarda o aviso
    /// mostrado quando o botão apertado não serve (ex.: já está no desenho).
    adding: Option<Option<String>>,
    active: bool,
    dry_run: bool,
    /// Controles conectados: nome e se tem touchpad.
    pads: BTreeMap<u32, (String, bool)>,
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
        // Os atalhos do egui (Ctrl+Q fecha, Ctrl+/−/0 dão zoom) pegariam as
        // combinações que o usuário mapeia ou que o próprio padmap envia.
        cc.egui_ctx.options_mut(|o| {
            o.quit_shortcuts.clear();
            o.zoom_with_keyboard = false;
        });
        let theme = cc
            .storage
            .and_then(|s| s.get_string(THEME_KEY))
            .and_then(|name| theme::from_name(&name))
            .or_else(|| cc.egui_ctx.system_theme())
            .unwrap_or(Theme::Light);
        cc.egui_ctx.set_theme(theme);

        // Reabre o último perfil usado; senão, o primeiro da pasta.
        let (library, error) = match Library::open() {
            Ok(lib) => (Some(lib), None),
            Err(e) => (None, Some(format!("Pasta de perfis indisponível: {e}"))),
        };
        let last = cc
            .storage
            .and_then(|s| s.get_string(LAST_PROFILE_KEY))
            .map(PathBuf::from);
        let entries = library.as_ref().map_or_else(Vec::new, Library::list);
        let (profile, path) = last
            .filter(|p| entries.iter().any(|e| &e.path == p))
            .or_else(|| entries.first().map(|e| e.path.clone()))
            .and_then(|p| Profile::load(&p).ok().map(|profile| (profile, Some(p))))
            .unwrap_or_else(|| {
                let p = Profile::from_toml(DEFAULT_PROFILE).expect("perfil padrão válido");
                (p, None)
            });

        let ctx = cc.egui_ctx.clone();
        let worker = Worker::spawn(profile.clone(), move || ctx.request_repaint());
        PadmapApp {
            worker,
            profile,
            path,
            dirty: false,
            autosave: cc
                .storage
                .and_then(|s| s.get_string(AUTOSAVE_KEY))
                .is_some_and(|v| v == "on"),
            library,
            entries,
            search: String::new(),
            profile_dialog: None,
            capture: None,
            stick_editor: None,
            touchpad_editor: None,
            tab: Tab::Controller,
            adding: None,
            active: false,
            dry_run: true,
            pads: BTreeMap::new(),
            live: Live::default(),
            held: BTreeSet::new(),
            log: VecDeque::new(),
            show_log: false,
            error,
            theme,
        }
    }

    fn profile_changed(&mut self) {
        self.dirty = true;
        self.worker
            .send(Command::SetProfile(Box::new(self.profile.clone())));
        if self.autosave && self.path.is_some() {
            self.save();
        }
    }

    fn set_autosave(&mut self, on: bool) {
        self.autosave = on;
        // Ligar com alterações pendentes já grava, para não ficar nada para trás.
        if on && self.dirty && self.path.is_some() {
            self.save();
        }
    }

    /// Volta o perfil ao que está salvo no arquivo, descartando as alterações.
    fn revert(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match Profile::load(&path) {
            Ok(p) => {
                self.push_log(format!("Alterações desfeitas: {}", p.name));
                self.replace_profile(p, Some(path));
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn clear_slot(&mut self, slot: Slot) {
        if slot.get(&self.profile).is_some() {
            self.push_log(format!("{}: livre", slot.title(&self.profile)));
            self.set_slot(slot, None);
        }
    }

    fn set_slot(&mut self, slot: Slot, combo: Option<KeyCombo>) {
        slot.set(&mut self.profile, combo);
        self.profile_changed();
    }

    fn replace_profile(&mut self, profile: Profile, path: Option<PathBuf>) {
        self.profile = profile;
        self.path = path;
        self.dirty = false;
        self.worker
            .send(Command::SetProfile(Box::new(self.profile.clone())));
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
                Status::Connected { id, name, touchpad } => {
                    self.push_log(format!("Controle conectado: {name}"));
                    self.pads.insert(id, (name, touchpad));
                }
                Status::Disconnected { id } => {
                    if let Some((name, _)) = self.pads.remove(&id) {
                        self.push_log(format!("Controle desconectado: {name}"));
                    }
                    if self.pads.is_empty() {
                        self.live = Live::default();
                    }
                }
                Status::Input(event) => {
                    match event {
                        InputEvent::ButtonDown(b) => {
                            self.live.pressed.insert(b);
                        }
                        InputEvent::ButtonUp(b) => {
                            self.live.pressed.remove(&b);
                        }
                        InputEvent::AxisMotion(a, v) => {
                            self.live.axes.insert(a, v);
                        }
                        InputEvent::OtherButtonDown(n) => {
                            self.live.pressed_other.insert(n);
                        }
                        InputEvent::OtherButtonUp(n) => {
                            self.live.pressed_other.remove(&n);
                        }
                        InputEvent::TouchDown(f, x, y) => {
                            self.live.touch.insert(f, (x, y));
                            self.live.touch_start.insert(f, (x, y));
                        }
                        InputEvent::TouchMove(f, x, y) => {
                            self.live.touch.insert(f, (x, y));
                        }
                        InputEvent::TouchUp(f) => {
                            self.live.touch.remove(&f);
                            self.live.touch_start.remove(&f);
                        }
                    }
                    if self.adding.is_some() {
                        self.add_extra(event);
                    }
                }
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

    fn refresh(&mut self) {
        self.entries = self.library.as_ref().map_or_else(Vec::new, Library::list);
    }

    /// Abre o perfil em `path`. Com alterações não salvas, pergunta antes.
    fn request_open(&mut self, path: PathBuf) {
        if self.path.as_ref() == Some(&path) {
            return;
        }
        if self.dirty {
            self.profile_dialog = Some(ProfileDialog::Unsaved { next: path });
        } else {
            self.open_path(&path);
        }
    }

    fn open_path(&mut self, path: &Path) {
        match Profile::load(path) {
            Ok(p) => {
                self.push_log(format!("Perfil aberto: {}", p.name));
                self.replace_profile(p, Some(path.to_owned()));
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Pede o nome do perfil novo. Ele só é criado ao confirmar.
    fn new_profile(&mut self) {
        if self.library.is_some() {
            self.profile_dialog = Some(ProfileDialog::Create {
                name: "Novo perfil".into(),
                error: None,
            });
        }
    }

    /// Cria um perfil vazio chamado `name` na pasta e abre.
    fn create(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Dê um nome ao perfil.".into());
        }
        if self
            .entries
            .iter()
            .any(|e| e.name.to_lowercase() == name.to_lowercase())
        {
            return Err(format!(
                "Já existe um perfil chamado \u{201c}{name}\u{201d}."
            ));
        }
        let Some(lib) = &self.library else {
            return Ok(());
        };
        let mut empty = Profile::from_toml("name = \"\"").expect("perfil vazio válido");
        empty.name = name.to_owned();
        let path = lib.create(&empty)?;
        self.push_log(format!("Perfil criado: {name}"));
        self.refresh();
        self.request_open(path);
        Ok(())
    }

    /// Traz um perfil de fora para a pasta e abre.
    fn import(&mut self) {
        let Some(lib) = &self.library else {
            return;
        };
        let Some(src) = rfd::FileDialog::new()
            .add_filter("Perfil padmap", &["toml"])
            .pick_file()
        else {
            return;
        };
        match lib.import(&src) {
            Ok(path) => {
                self.push_log(format!("Perfil importado: {}", src.display()));
                self.refresh();
                self.request_open(path);
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Salva uma cópia do perfil onde o usuário quiser, para compartilhar.
    fn export(&mut self, path: &Path) {
        let name = path
            .file_name()
            .map_or("perfil.toml".into(), |n| n.to_string_lossy().into_owned());
        let Some(dest) = rfd::FileDialog::new()
            .add_filter("Perfil padmap", &["toml"])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        match std::fs::copy(path, &dest) {
            Ok(_) => self.push_log(format!("Perfil exportado: {}", dest.display())),
            Err(e) => self.error = Some(format!("Não foi possível exportar: {e}")),
        }
    }

    fn duplicate(&mut self, path: &Path) {
        let Some(lib) = &self.library else {
            return;
        };
        match lib.duplicate(path) {
            Ok(copy) => {
                self.push_log(format!("Perfil duplicado: {}", copy.display()));
                self.refresh();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Grava o perfil no arquivo dele. Sem arquivo (pasta de perfis
    /// indisponível), pergunta onde.
    fn save(&mut self) {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => {
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
                self.refresh();
            }
            Err(e) => self.error = Some(format!("Não foi possível salvar: {e}")),
        }
    }

    /// Renomeia o perfil em `path`, o aberto ou outro da lista. No aberto,
    /// alterações ainda não salvas continuam pendentes.
    fn rename(&mut self, path: &Path, new: &str) -> Result<(), String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("Dê um nome ao perfil.".into());
        }
        let target = profiles::rename(path, new)?;
        self.push_log(format!("Perfil renomeado: {new}"));
        if self.path.as_deref() == Some(path) {
            self.path = Some(target);
            self.profile.name = new.to_owned();
        }
        self.refresh();
        Ok(())
    }

    /// Manda o perfil para a lixeira. Se era o aberto, abre outro da pasta.
    fn delete(&mut self, path: &Path) -> Result<(), String> {
        profiles::delete(path)?;
        self.push_log(format!("Perfil enviado para a lixeira: {}", path.display()));
        if self.path.as_deref() == Some(path) {
            self.dirty = false;
            self.path = None;
            let next = self.library.as_ref().and_then(|lib| {
                lib.ensure_not_empty().ok()?;
                lib.list().into_iter().next().map(|e| e.path)
            });
            match next {
                Some(p) => self.open_path(&p),
                None => {
                    let p = Profile::from_toml(DEFAULT_PROFILE).expect("perfil padrão válido");
                    self.replace_profile(p, None);
                }
            }
        }
        self.refresh();
        Ok(())
    }

    fn profile_dialog(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let Some(dialog) = &mut self.profile_dialog else {
            return;
        };
        // Lido antes de desenhar: o campo de texto consome o Esc.
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        // O conteúdo e os botões podem decidir; vale a última escolha.
        let choice = std::cell::Cell::new(None);
        let current = self.profile.name.clone();
        let (title, action) = match dialog {
            ProfileDialog::Create { .. } => ("Novo perfil", "Criar"),
            ProfileDialog::RenameExtra { .. } => ("Renomear botão", "Renomear"),
            _ => ("Renomear perfil", "Renomear"),
        };
        match dialog {
            ProfileDialog::Rename { name, error, .. }
            | ProfileDialog::Create { name, error }
            | ProfileDialog::RenameExtra { name, error, .. } => {
                dialog_window(
                    ui.ctx(),
                    pal,
                    title,
                    |ui| {
                        field_style(ui, pal);
                        let mut out = egui::TextEdit::singleline(name)
                            .desired_width(f32::INFINITY)
                            .margin(vec2(10.0, 8.0))
                            .show(ui);
                        let edit = out.response;
                        // Ao abrir, foca com o nome todo selecionado: digitar substitui.
                        if !edit.has_focus() && !edit.lost_focus() {
                            edit.request_focus();
                            let end = egui::text::CCursor::new(name.chars().count());
                            out.state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    end,
                                )));
                            out.state.store(ui.ctx(), edit.id);
                        }
                        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            choice.set(Some(Choice::Confirm));
                        }
                        if let Some(e) = error {
                            ui.add_space(6.0);
                            ui.label(RichText::new(e.as_str()).size(12.0).color(BAD));
                        }
                    },
                    |ui| {
                        if ui.add(primary_button(action, ACCENT_STRONG)).clicked() {
                            choice.set(Some(Choice::Confirm));
                        }
                        if ui.button("Cancelar").clicked() {
                            choice.set(Some(Choice::Cancel));
                        }
                    },
                );
            }
            ProfileDialog::Delete { name, error, .. } => {
                dialog_window(
                    ui.ctx(),
                    pal,
                    "Excluir perfil",
                    |ui| {
                        ui.label(
                            RichText::new(format!("Excluir o perfil \u{201c}{name}\u{201d}?"))
                                .strong()
                                .color(pal.text),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "O arquivo vai para a lixeira, de onde ainda dá para recuperar.",
                            )
                            .size(12.0)
                            .color(pal.text_muted),
                        );
                        if let Some(e) = error {
                            ui.add_space(6.0);
                            ui.label(RichText::new(e.as_str()).size(12.0).color(BAD));
                        }
                    },
                    |ui| {
                        if ui.add(primary_button("Excluir", BAD)).clicked() {
                            choice.set(Some(Choice::Confirm));
                        }
                        if ui.button("Cancelar").clicked() {
                            choice.set(Some(Choice::Cancel));
                        }
                    },
                );
            }
            ProfileDialog::Revert => {
                dialog_window(
                    ui.ctx(),
                    pal,
                    "Desfazer alterações",
                    |ui| {
                        ui.label(
                            RichText::new(format!(
                                "Voltar o perfil \u{201c}{current}\u{201d} ao último salvamento?"
                            ))
                            .strong()
                            .color(pal.text),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("As alterações feitas desde então serão perdidas.")
                                .size(12.0)
                                .color(pal.text_muted),
                        );
                    },
                    |ui| {
                        if ui.add(primary_button("Desfazer", BAD)).clicked() {
                            choice.set(Some(Choice::Confirm));
                        }
                        if ui.button("Cancelar").clicked() {
                            choice.set(Some(Choice::Cancel));
                        }
                    },
                );
            }
            ProfileDialog::Unsaved { .. } => {
                dialog_window(
                    ui.ctx(),
                    pal,
                    "Alterações não salvas",
                    |ui| {
                        ui.label(
                            RichText::new(format!(
                                "O perfil \u{201c}{current}\u{201d} tem alterações não salvas."
                            ))
                            .color(pal.text),
                        );
                    },
                    |ui| {
                        if ui
                            .add(primary_button("Salvar e trocar", ACCENT_STRONG))
                            .clicked()
                        {
                            choice.set(Some(Choice::Confirm));
                        }
                        if ui.button("Descartar").clicked() {
                            choice.set(Some(Choice::Discard));
                        }
                        if ui.button("Cancelar").clicked() {
                            choice.set(Some(Choice::Cancel));
                        }
                    },
                );
            }
        }
        let choice = if escape {
            Some(Choice::Cancel)
        } else {
            choice.get()
        };
        let Some(choice) = choice else {
            return;
        };
        let Some(dialog) = self.profile_dialog.take() else {
            return;
        };
        let result = match (dialog, choice) {
            (_, Choice::Cancel) => Ok(()),
            (ProfileDialog::RenameExtra { source, name, .. }, _) => self
                .rename_extra(source, &name)
                .map_err(|e| ProfileDialog::RenameExtra {
                    source,
                    name,
                    error: Some(e),
                }),
            (ProfileDialog::Create { name, .. }, _) => {
                self.create(&name).map_err(|e| ProfileDialog::Create {
                    name,
                    error: Some(e),
                })
            }
            (ProfileDialog::Rename { path, name, .. }, _) => {
                self.rename(&path, &name)
                    .map_err(|e| ProfileDialog::Rename {
                        path,
                        name,
                        error: Some(e),
                    })
            }
            (ProfileDialog::Delete { path, name, .. }, _) => {
                self.delete(&path).map_err(|e| ProfileDialog::Delete {
                    path,
                    name,
                    error: Some(e),
                })
            }
            (ProfileDialog::Revert, _) => {
                self.revert();
                Ok(())
            }
            (ProfileDialog::Unsaved { next }, choice) => {
                if choice == Choice::Confirm {
                    self.save();
                } else {
                    self.dirty = false;
                }
                // Se salvar falhou, o perfil continua com alterações: não troca.
                if !self.dirty {
                    self.open_path(&next);
                }
                Ok(())
            }
        };
        // Com erro, a janela continua aberta mostrando o motivo.
        if let Err(dialog) = result {
            self.profile_dialog = Some(dialog);
        }
    }

    /// Conteúdo do menu do perfil: busca, lista da pasta e ações gerais.
    fn profile_menu(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.set_width(300.0);
        if self.library.is_none() {
            ui.label(RichText::new("A pasta de perfis não está disponível.").color(pal.text_muted));
            return;
        }
        field_style(ui, pal);
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Buscar perfil")
                .desired_width(f32::INFINITY)
                .margin(vec2(8.0, 6.0)),
        );
        ui.add_space(6.0);

        let filter = self.search.to_lowercase();
        let mut action = None;
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                let mut shown = 0;
                for e in &self.entries {
                    if !e.name.to_lowercase().contains(&filter) {
                        continue;
                    }
                    shown += 1;
                    let current = self.path.as_ref() == Some(&e.path);
                    if let Some(a) = profile_row(ui, pal, e, current, current && self.dirty) {
                        action = Some((a, e.path.clone(), e.name.clone()));
                    }
                }
                if shown == 0 {
                    ui.label(RichText::new("Nenhum perfil encontrado.").color(pal.text_muted));
                }
            });

        ui.separator();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new("Salvar automaticamente").color(pal.text));
                ui.label(
                    RichText::new("Grava cada alteração na hora.")
                        .size(11.0)
                        .color(pal.text_muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if switch(ui, pal, self.autosave).clicked() {
                    self.set_autosave(!self.autosave);
                }
            });
        });
        ui.separator();
        let mut new = false;
        let mut import = false;
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            new = ui
                .add(primary_button("+  Novo perfil", ACCENT_STRONG).min_size(vec2(w, 32.0)))
                .clicked();
            import = ui
                .add(egui::Button::new("Importar…").min_size(vec2(w, 32.0)))
                .on_hover_text("Traz para a pasta do padmap um perfil salvo em outro lugar.")
                .clicked();
        });

        if action.is_some() || new || import {
            egui::Popup::close_all(ui.ctx());
        }
        if new {
            self.new_profile();
        }
        if import {
            self.import();
        }
        let Some((action, path, name)) = action else {
            return;
        };
        match action {
            RowAction::Open => self.request_open(path),
            RowAction::Rename => {
                self.profile_dialog = Some(ProfileDialog::Rename {
                    // O aberto pode ter sido renomeado sem salvar; vale o da lista.
                    name: name.clone(),
                    path,
                    error: None,
                })
            }
            RowAction::Duplicate => self.duplicate(&path),
            RowAction::Export => self.export(&path),
            RowAction::Delete => {
                self.profile_dialog = Some(ProfileDialog::Delete {
                    path,
                    name,
                    error: None,
                })
            }
        }
    }

    // ---------------------------------------------------------- botões extras

    /// Com a janela "Adicionar botão" aberta, o primeiro botão apertado entra
    /// na lista e a captura da tecla já abre.
    fn add_extra(&mut self, event: InputEvent) {
        let source = match event {
            InputEvent::ButtonDown(b) => {
                if !OFF_DIAGRAM.contains(&b) {
                    self.adding = Some(Some(format!(
                        "\u{201c}{}\u{201d} já está no desenho do controle. Mapeie esse botão na aba Controle.",
                        Slot::Button(b).title(&self.profile)
                    )));
                    return;
                }
                let touchpad_owns_click = self
                    .profile
                    .touchpad
                    .as_ref()
                    .is_some_and(|tp| tp.owns_click());
                if b == Button::Touchpad && touchpad_owns_click {
                    self.adding = Some(Some(
                        "O clique do touchpad já é usado pelo touchpad (zonas ao clicar ou mouse). Para dar uma tecla só ao clique, use as zonas \u{201c}Ao tocar\u{201d} ou os gestos.".to_owned(),
                    ));
                    return;
                }
                ExtraSource::Known(b)
            }
            InputEvent::OtherButtonDown(n) => ExtraSource::Other(n),
            _ => return,
        };
        if let Some(e) = self.profile.extras.iter().find(|e| e.button == source) {
            self.adding = Some(Some(format!(
                "Esse botão já está na lista, como \u{201c}{}\u{201d}.",
                if e.name.is_empty() {
                    extra_default_name(source)
                } else {
                    e.name.clone()
                }
            )));
            return;
        }
        let name = extra_default_name(source);
        self.push_log(format!("Botão adicionado: {name}"));
        self.profile.extras.push(ExtraButton {
            button: source,
            name,
            key: None,
        });
        self.profile_changed();
        self.adding = None;
        self.capture = Some((Slot::Extra(source), Capture::default()));
    }

    fn remove_extra(&mut self, source: ExtraSource) {
        let title = Slot::Extra(source).title(&self.profile);
        self.profile.extras.retain(|e| e.button != source);
        self.push_log(format!("Botão removido: {title}"));
        self.profile_changed();
    }

    fn rename_extra(&mut self, source: ExtraSource, name: &str) -> Result<(), String> {
        let name = name.trim();
        if let Some(e) = self.profile.extras.iter_mut().find(|e| e.button == source) {
            e.name = if name.is_empty() {
                extra_default_name(source)
            } else {
                name.to_owned()
            };
        }
        self.profile_changed();
        Ok(())
    }

    fn adding_dialog(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let Some(notice) = &self.adding else {
            return;
        };
        let notice = notice.clone();
        let mut cancel = false;
        let mut to_controller = false;
        dialog_window(
            ui.ctx(),
            pal,
            "Adicionar botão",
            |ui| {
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 27.0, pal.key_bg);
                    gamepad_icon(ui.painter(), r.center(), ACCENT);
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Aperte qualquer botão no controle")
                            .strong()
                            .color(pal.text),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Ele entra na lista, e em seguida você escolhe a tecla.")
                            .size(12.0)
                            .color(pal.text_muted),
                    );
                    if let Some(n) = &notice {
                        ui.add_space(10.0);
                        ui.label(RichText::new(n.as_str()).size(12.0).color(BAD));
                    }
                });
            },
            |ui| {
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
                if notice.as_deref().is_some_and(|n| n.contains("desenho"))
                    && ui.button("Ir para a aba Controle").clicked()
                {
                    to_controller = true;
                }
            },
        );
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if to_controller {
            self.tab = Tab::Controller;
            cancel = true;
        }
        if cancel {
            self.adding = None;
        }
    }

    // ---------------------------------------------------------------- topo

    fn header(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.horizontal_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
            app_icon(ui.painter(), r);
            ui.add_space(2.0);
            ui.label(RichText::new("padmap").size(18.0).strong().color(pal.text));
            ui.add_space(14.0);

            let resp = profile_button(ui, pal, &self.profile.name).on_hover_text(if self.dirty {
                "Perfil atual (alterações não salvas)"
            } else {
                "Perfil atual"
            });
            if resp.clicked() {
                self.refresh();
                self.search.clear();
            }
            egui::Popup::menu(&resp).show(|ui| self.profile_menu(ui, pal));
            ui.add_space(4.0);
            if self.autosave && self.path.is_some() {
                ui.label(
                    RichText::new("Salvo automaticamente")
                        .size(11.5)
                        .color(pal.chrome_muted),
                )
                .on_hover_text(
                    "Desligue em \u{201c}Salvar automaticamente\u{201d}, no menu do perfil.",
                );
            } else {
                ui.spacing_mut().item_spacing.x = 2.0;
                let save_tip = if self.dirty {
                    "Salvar (há alterações não salvas)"
                } else {
                    "Salvar"
                };
                if icon_button(ui, pal, Icon::Save, self.dirty)
                    .on_hover_text(save_tip)
                    .clicked()
                {
                    self.save();
                }
                if self.dirty
                    && self.path.is_some()
                    && icon_button(ui, pal, Icon::Revert, false)
                        .on_hover_text("Desfazer alterações não salvas")
                        .clicked()
                {
                    self.profile_dialog = Some(ProfileDialog::Revert);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let tip = match (self.active, self.dry_run) {
                    (false, _) => "Começa a mapear o controle.",
                    (true, true) => "Simulando: as teclas só aparecem no log. Clique para parar.",
                    (true, false) => "Enviando teclas ao sistema. Clique para parar.",
                };
                if run_button(ui, pal, self.active, self.dry_run)
                    .on_hover_text(tip)
                    .clicked()
                {
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
                ui.label(RichText::new("Simulação").color(pal.chrome_muted));
                ui.add_space(12.0);

                if let Some(t) = theme_toggle(ui, pal, self.theme) {
                    self.set_theme(ui.ctx(), t);
                }
                ui.add_space(12.0);

                let (color, name) = match self.pads.values().next() {
                    Some((n, _)) => (GOOD, n.as_str()),
                    None => (pal.chrome_muted, "Nenhum controle conectado"),
                };
                ui.label(RichText::new(name).color(pal.chrome_muted));
                let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                gamepad_icon(ui.painter(), r.center(), color);
            });
        });
    }

    // ---------------------------------------------------------------- rodapé

    fn footer(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("Teclas seguradas").color(pal.chrome_muted));
            if self.held.is_empty() {
                ui.label(RichText::new("nenhuma").color(pal.chrome_muted).weak());
            }
            for k in &self.held {
                egui::Frame::new()
                    .fill(ACCENT)
                    .corner_radius(5)
                    .inner_margin(Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(capture::key_label(*k).to_uppercase())
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
                    .color(pal.chrome_muted),
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
        let Some(editor) = &mut self.stick_editor else {
            return;
        };
        match editor.show(ui.ctx(), pal, &self.live) {
            Some(Outcome::Save(binding)) => {
                self.profile.sticks.insert(editor.stick, binding);
                self.stick_editor = None;
                self.profile_changed();
            }
            Some(Outcome::Cancel) => self.stick_editor = None,
            None => {}
        }
    }

    fn touchpad_modal(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let Some(editor) = &mut self.touchpad_editor else {
            return;
        };
        match editor.show(ui.ctx(), pal, &self.live) {
            Some(touchpad_editor::Outcome::Save(binding)) => {
                // Nas zonas "ao clicar" e no mouse, o clique é do touchpad.
                if binding.as_ref().is_some_and(|b| b.owns_click()) {
                    self.profile.buttons.remove(&Button::Touchpad);
                    self.profile
                        .extras
                        .retain(|e| e.button != ExtraSource::Known(Button::Touchpad));
                }
                self.profile.touchpad = binding;
                self.touchpad_editor = None;
                self.profile_changed();
            }
            Some(touchpad_editor::Outcome::Cancel) => self.touchpad_editor = None,
            None => {}
        }
    }

    fn capture_modal(&mut self, ui: &mut egui::Ui) {
        let Some((slot, capture)) = &mut self.capture else {
            return;
        };
        let slot = *slot;
        let events = ui.input(|i| i.events.clone());
        let result = capture.feed(&events);
        let held = capture.keys().to_vec();
        let current = slot.get(&self.profile).cloned();
        let pal = theme::palette(self.theme);
        let title = slot.title(&self.profile);
        let action = capture::modal(ui.ctx(), pal, &title, current.as_ref(), &held);

        if let Some(combo) = result {
            self.capture = None;
            self.set_slot(slot, Some(combo));
        } else if let Some(action) = action {
            self.capture = None;
            match action {
                capture::ModalAction::Cancel => {}
                capture::ModalAction::Clear => {
                    self.push_log(format!("{title}: livre"));
                    self.set_slot(slot, None);
                }
                capture::ModalAction::Pick(combo) => self.set_slot(slot, Some(combo)),
            }
        }
    }
}

enum ProfileDialog {
    /// Nome do perfil novo; nada é criado até confirmar.
    Create { name: String, error: Option<String> },
    /// Nome de um botão da lista de extras.
    RenameExtra {
        source: ExtraSource,
        name: String,
        error: Option<String>,
    },
    Rename {
        path: PathBuf,
        name: String,
        error: Option<String>,
    },
    Delete {
        path: PathBuf,
        name: String,
        error: Option<String>,
    },
    /// Trocar para `next` com o perfil aberto ainda com alterações.
    Unsaved { next: PathBuf },
    /// Voltar o perfil aberto ao que está salvo.
    Revert,
}

#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Confirm,
    Discard,
    Cancel,
}

#[derive(Clone, Copy)]
enum RowAction {
    Open,
    Rename,
    Duplicate,
    Export,
    Delete,
}

/// Linha da lista de perfis: marca no aberto, nome, resumo e um botão "•••"
/// com as ações daquele perfil.
fn profile_row(
    ui: &mut egui::Ui,
    pal: &Palette,
    entry: &Entry,
    current: bool,
    dirty: bool,
) -> Option<RowAction> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
    let more_rect = Rect::from_center_size(
        egui::pos2(rect.right() - 18.0, rect.center().y),
        vec2(26.0, 26.0),
    );
    let hovered = resp.hovered() || ui.rect_contains_pointer(more_rect);
    let p = ui.painter();
    if current {
        p.rect_filled(rect, 7.0, pal.key_bg);
    } else if hovered {
        p.rect_filled(rect, 7.0, pal.toggle_bg);
    }
    let name_color = if current { pal.key_text } else { pal.text };
    if current {
        // Marca de "aberto".
        let c = egui::pos2(rect.left() + 14.0, rect.center().y);
        let stroke = Stroke::new(1.8, pal.key_text);
        p.line_segment([c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.0)], stroke);
        p.line_segment([c + vec2(-1.0, 3.0), c + vec2(4.5, -3.5)], stroke);
    }
    let left = rect.left() + 28.0;
    let width = more_rect.left() - 6.0 - left;
    // Uma linha só; o que não couber termina em "…".
    let line = |text: &str, size: f32, color: Color32| {
        let mut job = egui::text::LayoutJob::single_section(
            text.to_owned(),
            egui::TextFormat::simple(egui::FontId::proportional(size), color),
        );
        job.wrap.max_width = width;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        ui.painter().layout_job(job)
    };
    let name = line(&entry.name, 13.0, name_color);
    let summary = line(&entry.summary, 11.0, pal.text_muted);
    let name_pos = egui::pos2(left, rect.top() + 6.0);
    if dirty {
        p.circle_filled(
            name_pos + vec2(name.size().x + 7.0, name.size().y / 2.0),
            3.0,
            ACTIVE,
        );
    }
    p.galley(name_pos, name, name_color);
    p.galley(
        egui::pos2(left, rect.bottom() - 18.0),
        summary,
        pal.text_muted,
    );

    // "•••" abre as ações do perfil. É um submenu, para abrir sem fechar a
    // lista em volta.
    let mut action = None;
    let more = ui.interact(
        more_rect,
        ui.scope_id().with(("more", &entry.path)),
        Sense::click(),
    );
    if more.hovered() {
        p.rect_filled(more_rect, 6.0, pal.bar);
    }
    if hovered || current {
        for dx in [-5.0, 0.0, 5.0] {
            p.circle_filled(more_rect.center() + vec2(dx, 0.0), 1.6, pal.text_muted);
        }
    }
    egui::containers::menu::SubMenu::new().show(ui, &more, |ui| {
        ui.set_min_width(150.0);
        if ui.button("Renomear…").clicked() {
            action = Some(RowAction::Rename);
        }
        if ui.button("Duplicar").clicked() {
            action = Some(RowAction::Duplicate);
        }
        if ui
            .button("Exportar…")
            .on_hover_text("Salva uma cópia em outro lugar, para compartilhar.")
            .clicked()
        {
            action = Some(RowAction::Export);
        }
        ui.separator();
        if ui
            .button(RichText::new("Excluir…").color(BAD))
            .on_hover_text("Manda o perfil para a lixeira.")
            .clicked()
        {
            action = Some(RowAction::Delete);
        }
    });
    if action.is_none() && resp.clicked() {
        action = Some(RowAction::Open);
    }
    action
}

/// Campo de texto com borda visível nos dois temas: a de foco padrão do egui
/// é branca e some no tema claro. A cor de `selection.stroke` também é a do
/// texto selecionado, então seleção e borda usam as cores do selo de tecla.
fn field_style(ui: &mut egui::Ui, pal: &Palette) {
    let v = ui.visuals_mut();
    v.selection.bg_fill = pal.key_bg;
    v.selection.stroke = Stroke::new(1.5, pal.key_text);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.button_border);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
}

pub(crate) fn primary_button(text: &str, fill: Color32) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
        .fill(fill)
        .stroke(Stroke::NONE)
        .min_size(vec2(0.0, 30.0))
}

/// Janela pequena no estilo dos outros modais: cabeçalho com a cor da barra,
/// conteúdo e botões à direita no rodapé (o primeiro fica mais à direita).
fn dialog_window(
    ctx: &egui::Context,
    pal: &Palette,
    title: &str,
    body: impl FnOnce(&mut egui::Ui),
    buttons: impl FnOnce(&mut egui::Ui),
) {
    egui::Modal::new(egui::Id::unique("padmap-dialog"))
        .frame(
            egui::Frame::new()
                .fill(pal.bar)
                .stroke(Stroke::new(1.0, pal.chrome_border))
                .corner_radius(14),
        )
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
            let head = egui::Frame::new()
                .fill(pal.chrome)
                .corner_radius(egui::CornerRadius {
                    nw: 14,
                    ne: 14,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(Margin::symmetric(18, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(title).size(15.0).strong().color(pal.text));
                });
            let r = head.response.rect;
            ui.painter()
                .hline(r.x_range(), r.bottom(), Stroke::new(1.0, pal.chrome_border));
            egui::Frame::new()
                .inner_margin(Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    body(ui);
                });
            let top = ui.cursor().top();
            ui.painter().hline(
                ui.max_rect().x_range(),
                top,
                Stroke::new(1.0, pal.bar_border),
            );
            egui::Frame::new()
                .inner_margin(Margin::symmetric(18, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        buttons(ui);
                    });
                });
        });
}

/// Botão Ativar / Parar, com largura fixa para não pular entre os estados.
/// Parado, é a ação principal (azul sólido com "play"). Ativo, fica tingido com
/// um ponto aceso: azul em simulação, vermelho enviando teclas de verdade.
fn run_button(ui: &mut egui::Ui, pal: &Palette, active: bool, dry_run: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(112.0, 34.0), Sense::click());
    let hot = resp.hovered();
    let (fill, border, text_color, accent) = match (active, dry_run) {
        (false, _) => {
            let fill = if hot { ACCENT } else { ACCENT_STRONG };
            (fill, Stroke::NONE, Color32::WHITE, Color32::WHITE)
        }
        (true, true) => (
            pal.key_bg,
            Stroke::new(if hot { 2.0 } else { 1.0 }, ACCENT),
            pal.key_text,
            ACCENT,
        ),
        (true, false) => (
            pal.bad_bg,
            Stroke::new(if hot { 2.0 } else { 1.0 }, BAD),
            pal.bad_text,
            BAD,
        ),
    };
    let p = ui.painter();
    p.rect(rect, 8.0, fill, border, StrokeKind::Inside);

    let label = if active { "Parar" } else { "Ativar" };
    let galley = p.layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(13.5),
        text_color,
    );
    // Ícone (12 px) + 8 px + texto, centrados juntos.
    let total = 12.0 + 8.0 + galley.size().x;
    let x = rect.center().x - total / 2.0;
    let c = egui::pos2(x + 6.0, rect.center().y);
    if active {
        p.circle_stroke(c, 6.5, Stroke::new(2.0, accent.gamma_multiply(0.35)));
        p.circle_filled(c, 4.0, accent);
    } else {
        p.add(egui::Shape::convex_polygon(
            vec![
                c + vec2(-3.5, -5.0),
                c + vec2(5.0, 0.0),
                c + vec2(-3.5, 5.0),
            ],
            accent,
            Stroke::NONE,
        ));
    }
    p.galley(
        egui::pos2(x + 20.0, rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Nome do perfil com uma seta, que abre o menu do perfil.
fn profile_button(ui: &mut egui::Ui, pal: &Palette, name: &str) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(name.to_owned(), egui::FontId::proportional(14.0), pal.text);
    let size = vec2(galley.size().x + 40.0, 34.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, pal.toggle_bg);
    }
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.text,
    );
    let c = egui::pos2(rect.right() - 15.0, rect.center().y);
    let stroke = Stroke::new(1.6, pal.chrome_muted);
    ui.painter()
        .line_segment([c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0)], stroke);
    ui.painter()
        .line_segment([c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)], stroke);
    resp
}

#[derive(Clone, Copy)]
enum Icon {
    Save,
    Revert,
}

/// Ícone clicável da barra do topo. `alert` pinta o ícone em azul com um ponto,
/// para chamar atenção (ex.: perfil com alterações não salvas).
fn icon_button(ui: &mut egui::Ui, pal: &Palette, icon: Icon, alert: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, pal.toggle_bg);
    }
    let color = if alert {
        ACCENT
    } else if resp.hovered() {
        pal.text
    } else {
        pal.chrome_muted
    };
    let p = ui.painter();
    let c = rect.center();
    let stroke = Stroke::new(1.5, color);
    let at = |x: f32, y: f32| c + vec2(x, y);
    match icon {
        Icon::Save => {
            // Disquete: corpo, janela de cima e etiqueta de baixo.
            let body = vec![
                at(-6.5, -6.5),
                at(4.0, -6.5),
                at(6.5, -4.0),
                at(6.5, 6.5),
                at(-6.5, 6.5),
            ];
            p.add(egui::Shape::closed_line(body, stroke));
            p.rect_stroke(
                egui::Rect::from_min_max(at(-3.5, -6.5), at(2.5, -2.5)),
                0.0,
                stroke,
                StrokeKind::Middle,
            );
            p.rect_stroke(
                egui::Rect::from_min_max(at(-4.0, 1.0), at(4.0, 6.5)),
                0.0,
                stroke,
                StrokeKind::Middle,
            );
        }
        Icon::Revert => {
            // Seta curva voltando, sentido anti-horário.
            let r = 6.0;
            let arc: Vec<egui::Pos2> = (0..=16)
                .map(|k| {
                    let a = (-150.0 + 240.0 * k as f32 / 16.0).to_radians();
                    at(a.cos() * r, a.sin() * r + 1.0)
                })
                .collect();
            let tip = arc[0];
            p.add(egui::Shape::line(arc, stroke));
            p.line_segment([tip, tip + vec2(0.5, -4.0)], stroke);
            p.line_segment([tip, tip + vec2(4.0, 0.8)], stroke);
        }
    }
    if alert {
        p.circle(at(7.0, -7.0), 3.5, ACTIVE, Stroke::new(1.5, pal.chrome));
    }
    resp
}

/// Controle pequeno: corpo arredondado, cruz e dois botões.
/// O ícone do padmap (`assets/padmap.svg`) desenhado no quadrado `r`. As cores
/// são as do ícone, iguais nos dois temas.
fn app_icon(p: &egui::Painter, r: egui::Rect) {
    let k = r.width() / 64.0;
    let at = |x: f32, y: f32, w: f32, h: f32| {
        egui::Rect::from_min_size(r.min + vec2(x, y) * k, vec2(w, h) * k)
    };
    let blue = Color32::from_rgb(0x3B, 0x82, 0xF6);
    p.rect_filled(
        at(4.0, 6.0, 56.0, 54.0),
        13.0 * k,
        Color32::from_rgb(0x14, 0x1A, 0x28),
    );
    p.rect_filled(
        at(7.0, 5.0, 50.0, 47.0),
        11.0 * k,
        Color32::from_rgb(0x2E, 0x35, 0x47),
    );
    p.rect_filled(at(26.0, 13.0, 12.0, 32.0), 3.0 * k, blue);
    p.rect_filled(at(16.0, 23.0, 32.0, 12.0), 3.0 * k, blue);
    p.circle_filled(
        r.min + vec2(32.0, 29.0) * k,
        3.0 * k,
        Color32::from_rgb(0x1D, 0x4E, 0xD8),
    );
}

pub(crate) fn gamepad_icon(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    p.rect_stroke(
        egui::Rect::from_center_size(c, vec2(17.0, 11.0)),
        5.0,
        stroke,
        StrokeKind::Middle,
    );
    p.line_segment([c + vec2(-6.0, 0.0), c + vec2(-2.0, 0.0)], stroke);
    p.line_segment([c + vec2(-4.0, -2.0), c + vec2(-4.0, 2.0)], stroke);
    p.circle_filled(c + vec2(3.0, 1.0), 1.2, color);
    p.circle_filled(c + vec2(5.5, -1.5), 1.2, color);
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Controller,
    Extras,
}

/// Seletor de opções lado a lado (ex.: Teclas | Mouse). Retorna a escolhida
/// quando muda.
pub(crate) fn segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    pal: &Palette,
    options: &[(T, &str)],
    current: T,
    cell_width: f32,
) -> Option<T> {
    let cell = vec2(cell_width, 24.0);
    let (rect, _) = ui.allocate_exact_size(
        vec2(cell.x * options.len() as f32 + 6.0, cell.y + 6.0),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 8.0, pal.toggle_bg);
    let mut chosen = None;
    for (i, &(value, label)) in options.iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(3.0 + cell.x * i as f32, 3.0), cell);
        let resp = ui.interact(r, ui.scope_id().with(("segmented", i)), Sense::click());
        let selected = value == current;
        if selected {
            ui.painter().rect(
                r,
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
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.5),
            color,
        );
        if resp.clicked() && !selected {
            chosen = Some(value);
        }
    }
    chosen
}

/// Interruptor de liga/desliga desenhado à mão.
pub(crate) fn switch(ui: &mut egui::Ui, pal: &Palette, on: bool) -> egui::Response {
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
            pal.chrome_muted
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

        // Capturando uma tecla, Alt+F4 e afins viram mapeamento, não fecham a janela.
        let capturing = self.capture.is_some()
            || self.stick_editor.as_ref().is_some_and(|e| e.capturing())
            || self.touchpad_editor.as_ref().is_some_and(|e| e.capturing());
        if capturing && ui.ctx().input(|i| i.viewport().close_requested()) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        let bar = |top: bool| {
            egui::Frame::new()
                .fill(pal.chrome)
                .inner_margin(Margin::symmetric(28, if top { 11 } else { 8 }))
                .stroke(Stroke::new(1.0, pal.chrome_border))
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
                ui.vertical_centered(|ui| {
                    let n = self.profile.extras.len();
                    let extras_label = if n > 0 {
                        format!("Botões extras · {n}")
                    } else {
                        "Botões extras".to_owned()
                    };
                    let options = [
                        (Tab::Controller, "Controle"),
                        (Tab::Extras, extras_label.as_str()),
                    ];
                    if let Some(tab) = segmented(ui, pal, &options, self.tab, 130.0) {
                        self.tab = tab;
                    }
                });
                ui.add_space(4.0);
                match self.tab {
                    Tab::Controller => {
                        let capturing = self.capture.as_ref().map(|(s, _)| *s);
                        // O touchpad aparece com um controle que tenha um. Sem
                        // controle, aparece se o perfil já o mapeia.
                        let touchpad = if self.pads.is_empty() {
                            self.profile.touchpad.is_some()
                        } else {
                            self.pads.values().any(|&(_, t)| t)
                        };
                        match diagram::show(ui, pal, &self.live, &self.profile, capturing, touchpad)
                        {
                            Some(Click::Map(slot)) => {
                                self.capture = Some((slot, Capture::default()))
                            }
                            Some(Click::Clear(slot)) => self.clear_slot(slot),
                            Some(Click::EditStick(stick)) => {
                                self.stick_editor = Some(StickEditor::new(stick, &self.profile))
                            }
                            Some(Click::EditTouchpad) => {
                                self.touchpad_editor = Some(TouchpadEditor::new(&self.profile))
                            }
                            None => {}
                        }
                    }
                    Tab::Extras => match extras::show(ui, pal, &self.live, &self.profile) {
                        Some(extras::Action::Add) => self.adding = Some(None),
                        Some(extras::Action::Map(slot)) => {
                            self.capture = Some((slot, Capture::default()))
                        }
                        Some(extras::Action::Clear(slot)) => self.clear_slot(slot),
                        Some(extras::Action::Rename(source)) => {
                            self.profile_dialog = Some(ProfileDialog::RenameExtra {
                                name: Slot::Extra(source).title(&self.profile),
                                source,
                                error: None,
                            })
                        }
                        Some(extras::Action::Remove(source)) => self.remove_extra(source),
                        None => {}
                    },
                }
            });

        self.stick_modal(ui, pal);
        self.touchpad_modal(ui, pal);
        self.capture_modal(ui);
        self.profile_dialog(ui, pal);
        self.adding_dialog(ui, pal);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, theme::name(self.theme).to_owned());
        let autosave = if self.autosave { "on" } else { "off" };
        storage.set_string(AUTOSAVE_KEY, autosave.to_owned());
        if let Some(path) = &self.path {
            storage.set_string(LAST_PROFILE_KEY, path.display().to_string());
        }
    }
}
