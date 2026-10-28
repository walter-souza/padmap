//! Aba "Botões extras": qualquer botão do controle que não está no desenho
//! (traseiros, turbo, modo, botões sem nome), com nome dado pelo usuário.
//! Cada linha funciona como uma etiqueta da tela principal: clique escolhe a
//! tecla, botão direito deixa livre, e ela acende enquanto o botão está apertado.

use crate::app::primary_button;
use crate::capture::combo_label;
use crate::diagram::Live;
use crate::slots::{extra_origin, Slot};
use crate::theme::{Palette, ACCENT, ACTIVE};
use eframe::egui::{self, vec2, FontId, Rect, RichText, Sense, Stroke, StrokeKind};
use padmap_core::{ExtraSource, Profile};

/// Largura máxima da lista, para não esticar em janelas largas.
const MAX_W: f32 = 620.0;

pub enum Action {
    Add,
    Map(Slot),
    Clear(Slot),
    Rename(ExtraSource),
    Remove(ExtraSource),
}

pub fn show(ui: &mut egui::Ui, pal: &Palette, live: &Live, profile: &Profile) -> Option<Action> {
    let mut action = None;
    let w = ui.available_width().min(MAX_W);
    ui.vertical_centered(|ui| {
        ui.set_max_width(w);
        ui.add_space(8.0);
        if profile.extras.is_empty() {
            empty_state(ui, pal, &mut action);
            return;
        }
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new("Botões extras").size(15.0).strong().color(pal.text));
                ui.label(
                    RichText::new(
                        "Qualquer botão que o controle enviar: traseiros, turbo, modos, botões sem nome.",
                    )
                    .size(11.5)
                    .color(pal.text_muted),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(primary_button("+  Adicionar botão", crate::theme::ACCENT_STRONG))
                    .clicked()
                {
                    action = Some(Action::Add);
                }
            });
        });
        ui.add_space(10.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        for extra in &profile.extras {
            let slot = Slot::Extra(extra.button);
            if let Some(a) = row(ui, pal, live, profile, slot) {
                action = Some(a);
            }
        }
    });
    action
}

fn empty_state(ui: &mut egui::Ui, pal: &Palette, action: &mut Option<Action>) {
    ui.add_space(40.0);
    let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
    ui.painter().rect_filled(r, 14.0, pal.key_bg);
    crate::app::gamepad_icon(ui.painter(), r.center(), pal.key_text);
    ui.add_space(10.0);
    ui.label(
        RichText::new("Use mais botões do seu controle")
            .size(15.0)
            .strong()
            .color(pal.text),
    );
    ui.label(
        RichText::new(
            "Botões traseiros, de turbo, de modo ou qualquer outro que não aparece no desenho.\nAperte o botão e dê uma tecla a ele.",
        )
        .size(12.0)
        .color(pal.text_muted),
    );
    ui.add_space(12.0);
    if ui
        .add(
            primary_button("+  Adicionar botão", crate::theme::ACCENT_STRONG)
                .min_size(vec2(0.0, 34.0)),
        )
        .clicked()
    {
        *action = Some(Action::Add);
    }
}

/// Uma linha da lista: ponto de "apertado", nome, origem, tecla e "•••".
fn row(
    ui: &mut egui::Ui,
    pal: &Palette,
    live: &Live,
    profile: &Profile,
    slot: Slot,
) -> Option<Action> {
    let Slot::Extra(source) = slot else {
        return None;
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
    let more_rect = Rect::from_center_size(
        egui::pos2(rect.right() - 20.0, rect.center().y),
        vec2(26.0, 26.0),
    );
    let more = ui.interact(
        more_rect,
        ui.scope_id().with(("more", source)),
        Sense::click(),
    );
    let resp = resp.on_hover_text("Clique para trocar a tecla.\nBotão direito deixa livre.");

    let combo = slot.get(profile);
    let pressed = live.is_active(slot, profile);
    let (fill, border) = if pressed {
        (pal.active_bg, ACTIVE)
    } else if combo.is_some() {
        (pal.mapped_bg, pal.mapped_border)
    } else {
        (pal.free_bg, pal.free_border)
    };
    let border = if resp.hovered() && !pressed {
        Stroke::new(1.5, ACCENT)
    } else {
        Stroke::new(1.0, border)
    };
    let p = ui.painter();
    p.rect(rect, 8.0, fill, border, StrokeKind::Inside);

    // Ponto aceso enquanto o botão está apertado: ajuda a achar qual é qual.
    let dot = egui::pos2(rect.left() + 16.0, rect.center().y);
    p.circle_filled(dot, 4.0, if pressed { ACTIVE } else { pal.free_border });

    let name = p.layout_no_wrap(slot.title(profile), FontId::proportional(13.0), pal.text);
    let name_pos = egui::pos2(rect.left() + 30.0, rect.center().y - name.size().y / 2.0);
    let origin = p.layout_no_wrap(
        extra_origin(source),
        FontId::proportional(10.5),
        pal.chip_text,
    );
    let origin_rect = Rect::from_min_size(
        egui::pos2(name_pos.x + name.size().x + 10.0, rect.center().y - 9.0),
        vec2(origin.size().x + 12.0, 18.0),
    );
    p.galley(name_pos, name, pal.text);
    p.rect_filled(origin_rect, 4.0, pal.chip_bg);
    p.galley(
        origin_rect.center() - origin.size() / 2.0,
        origin,
        pal.chip_text,
    );

    // Selo da tecla, como nas etiquetas da tela principal.
    let (key, key_bg, key_fg) = match combo {
        Some(c) => (combo_label(c).to_uppercase(), pal.key_bg, pal.key_text),
        None => ("LIVRE".to_owned(), pal.chip_bg, pal.chip_text),
    };
    let key = p.layout_no_wrap(key, FontId::proportional(11.0), key_fg);
    let key_rect = Rect::from_min_size(
        egui::pos2(
            more_rect.left() - 8.0 - key.size().x - 16.0,
            rect.center().y - 10.0,
        ),
        vec2(key.size().x + 16.0, 20.0),
    );
    p.rect_filled(key_rect, 5.0, key_bg);
    p.galley(key_rect.center() - key.size() / 2.0, key, key_fg);

    if more.hovered() {
        p.rect_filled(more_rect, 6.0, pal.toggle_bg);
    }
    for dx in [-5.0, 0.0, 5.0] {
        p.circle_filled(more_rect.center() + vec2(dx, 0.0), 1.6, pal.text_muted);
    }

    let mut action = None;
    egui::Popup::menu(&more).show(|ui| {
        ui.set_min_width(140.0);
        if ui.button("Renomear…").clicked() {
            action = Some(Action::Rename(source));
        }
        ui.separator();
        if ui
            .button(RichText::new("Remover").color(crate::theme::BAD))
            .on_hover_text("Tira o botão da lista e a tecla dele.")
            .clicked()
        {
            action = Some(Action::Remove(source));
        }
    });
    if action.is_none() {
        if resp.clicked() {
            action = Some(Action::Map(slot));
        } else if resp.secondary_clicked() && combo.is_some() {
            action = Some(Action::Clear(slot));
        }
    }
    action
}
