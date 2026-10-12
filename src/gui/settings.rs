use super::collection_panel::{self, CollectionAction, CollectionInfo};
use clap::ValueEnum;
use eframe::egui;
use rust_mtg::language::Language;
use std::path::Path;

pub enum SettingsAction {
    PickLog,
    PickCardDb,
    ChangeLanguage(Language),
    UpdateScryfall,
    Collection(CollectionAction),
}

pub struct SettingsInfo<'a> {
    pub log_path: &'a Path,
    pub card_db: &'a Path,
    pub language: Language,
    pub scryfall_cards: Option<i64>,
    pub updating: bool,
    pub collection: CollectionInfo<'a>,
}

pub fn show(ctx: &egui::Context, open: &mut bool, info: &SettingsInfo) -> Option<SettingsAction> {
    let mut action = None;

    egui::Window::new("Configurações")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            egui::Grid::new("settings")
                .num_columns(3)
                .spacing([12.0, 10.0])
                .show(ui, |ui| {
                    ui.strong("Player.log");
                    ui.label(info.log_path.display().to_string());
                    if ui.button("Escolher…").clicked() {
                        action = Some(SettingsAction::PickLog);
                    }
                    ui.end_row();

                    ui.strong("Banco de cartas");
                    ui.label(info.card_db.display().to_string());
                    if ui.button("Escolher…").clicked() {
                        action = Some(SettingsAction::PickCardDb);
                    }
                    ui.end_row();

                    ui.strong("Idioma");
                    let mut language = info.language;
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(language.label())
                        .show_ui(ui, |ui| {
                            for option in Language::value_variants() {
                                ui.selectable_value(&mut language, *option, option.label());
                            }
                        });
                    if language != info.language {
                        action = Some(SettingsAction::ChangeLanguage(language));
                    }
                    ui.end_row();

                    ui.strong("Scryfall");
                    ui.label(match info.scryfall_cards {
                        Some(count) => format!("{count} cartas"),
                        None => "não baixado".to_string(),
                    });
                    let button = egui::Button::new("Atualizar");
                    if ui.add_enabled(!info.updating, button).clicked() {
                        action = Some(SettingsAction::UpdateScryfall);
                    }
                    ui.end_row();
                });

            ui.separator();
            ui.strong("Coleção");
            if let Some(collection_action) = collection_panel::controls(ui, &info.collection) {
                action = Some(SettingsAction::Collection(collection_action));
            }
        });

    action
}
