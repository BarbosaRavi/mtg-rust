use eframe::egui;
use rust_mtg::completeness::Completeness;
use rust_mtg::decks::{Deck, Decks};
use rust_mtg::inventory::Wildcards;
use std::collections::BTreeSet;

const COLORS: [(char, &str); 5] = [
    ('W', "Branco"),
    ('U', "Azul"),
    ('B', "Preto"),
    ('R', "Vermelho"),
    ('G', "Verde"),
];

#[derive(Clone, Copy, PartialEq, Default)]
enum Origin {
    #[default]
    Any,
    Mine,
    Precon,
}

#[derive(Clone, Copy, PartialEq, Default)]
enum Status {
    #[default]
    Any,
    Complete,
    Craftable,
    Incomplete,
}

impl Status {
    const ALL: [Status; 4] = [
        Status::Any,
        Status::Complete,
        Status::Craftable,
        Status::Incomplete,
    ];

    fn label(self) -> &'static str {
        match self {
            Status::Any => "qualquer",
            Status::Complete => "completos",
            Status::Craftable => "dá para completar com wildcards",
            Status::Incomplete => "faltam cartas",
        }
    }
}

pub struct DeckFacts<'a> {
    pub colors: &'a str,
    pub completeness: Option<&'a Completeness>,
    pub wildcards: Option<Wildcards>,
}

#[derive(Default)]
pub struct DeckFilters {
    name: String,
    origin: Origin,
    format: Option<String>,
    colors: [bool; 5],
    exact_colors: bool,
    status: Status,
}

impl DeckFilters {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, decks: &Decks, has_collection: bool) {
        ui.add(
            egui::TextEdit::singleline(&mut self.name)
                .hint_text("Buscar deck")
                .desired_width(f32::INFINITY),
        );

        egui::CollapsingHeader::new("Filtros")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.origin, Origin::Any, "Todos");
                    ui.selectable_value(&mut self.origin, Origin::Mine, "Criados por mim");
                    ui.selectable_value(&mut self.origin, Origin::Precon, "Prontos");
                });
                self.format_ui(ui, decks);
                self.colors_ui(ui);
                ui.add_enabled_ui(has_collection, |ui| self.status_ui(ui));

                if ui.button("Limpar filtros").clicked() {
                    *self = Self::default();
                }
            });
    }

    pub fn matches(&self, deck: &Deck, facts: &DeckFacts) -> bool {
        self.matches_origin(deck)
            && self.matches_format(deck)
            && self.matches_colors(facts.colors)
            && self.matches_status(facts)
    }

    fn format_ui(&mut self, ui: &mut egui::Ui, decks: &Decks) {
        let formats: BTreeSet<&str> = decks
            .values()
            .filter_map(|deck| deck.format.as_deref())
            .collect();

        egui::ComboBox::new("deck_format", "Formato")
            .selected_text(self.format.as_deref().unwrap_or("qualquer"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.format, None, "qualquer");
                for format in formats {
                    ui.selectable_value(&mut self.format, Some(format.to_string()), format);
                }
            });
    }

    fn colors_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Cores");
        ui.horizontal_wrapped(|ui| {
            for (selected, (_, label)) in self.colors.iter_mut().zip(COLORS) {
                ui.checkbox(selected, label);
            }
        });
        ui.checkbox(&mut self.exact_colors, "Exatamente estas cores");
    }

    fn status_ui(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::new("deck_status", "Situação")
            .selected_text(self.status.label())
            .show_ui(ui, |ui| {
                for status in Status::ALL {
                    ui.selectable_value(&mut self.status, status, status.label());
                }
            });
    }

    fn matches_origin(&self, deck: &Deck) -> bool {
        match self.origin {
            Origin::Any => true,
            Origin::Mine => !deck.precon,
            Origin::Precon => deck.precon,
        }
    }

    fn matches_format(&self, deck: &Deck) -> bool {
        self.format
            .as_ref()
            .is_none_or(|format| deck.format.as_ref() == Some(format))
    }

    fn matches_colors(&self, deck_colors: &str) -> bool {
        let selected: String = COLORS
            .iter()
            .zip(self.colors)
            .filter(|(_, selected)| *selected)
            .map(|((letter, _), _)| *letter)
            .collect();

        if selected.is_empty() {
            true
        } else if self.exact_colors {
            deck_colors == selected
        } else {
            selected.chars().all(|letter| deck_colors.contains(letter))
        }
    }

    fn matches_status(&self, facts: &DeckFacts) -> bool {
        let Some(status) = facts.completeness else {
            return true;
        };

        match self.status {
            Status::Any => true,
            Status::Complete => status.is_complete(),
            Status::Craftable => {
                !status.is_complete()
                    && facts
                        .wildcards
                        .is_some_and(|have| have.covers(&status.wildcards))
            }
            Status::Incomplete => !status.is_complete(),
        }
    }
}
