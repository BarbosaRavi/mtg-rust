use super::TabOutput;
use super::card_detail::CardDetail;
use super::card_grid::{self, Badge, Tile};
use super::collection_panel::{self, CollectionInfo};
use super::images::Images;
use anyhow::Result;
use clap::ValueEnum;
use eframe::egui;
use rust_mtg::card_search::{CardFilter, Rarity};
use rust_mtg::card_store::CardRecord;
use rust_mtg::catalog::Catalog;

const RESULT_LIMIT: usize = 3000;

const COLORS: [(char, &str); 6] = [
    ('W', "Branco"),
    ('U', "Azul"),
    ('B', "Preto"),
    ('R', "Vermelho"),
    ('G', "Verde"),
    ('C', "Incolor"),
];

const FORMATS: [&str; 8] = [
    "standard",
    "alchemy",
    "historic",
    "timeless",
    "pioneer",
    "brawl",
    "standardbrawl",
    "gladiator",
];

#[derive(Clone, PartialEq)]
struct FilterForm {
    owned_only: bool,
    name: String,
    text: String,
    type_line: String,
    colors: [bool; 6],
    exact_color: bool,
    use_cmc: bool,
    cmc_min: f64,
    cmc_max: f64,
    rarity: Option<Rarity>,
    set: String,
    format: Option<&'static str>,
}

struct Found {
    name: String,
    card: CardRecord,
}

pub struct CardsTab {
    form: FilterForm,
    searched: Option<FilterForm>,
    found: Vec<Found>,
    total: i64,
    error: Option<String>,
    show_filters: bool,
    card_width: f32,
}

impl Default for CardsTab {
    fn default() -> Self {
        Self {
            form: FilterForm::default(),
            searched: None,
            found: Vec::new(),
            total: 0,
            error: None,
            show_filters: true,
            card_width: 180.0,
        }
    }
}

impl CardsTab {
    pub fn invalidate(&mut self) {
        self.searched = None;
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        catalog: &Catalog,
        collection: &CollectionInfo,
        images: &mut Images,
    ) -> TabOutput {
        let mut action = None;

        self.toolbar(ui);
        if self.show_filters {
            egui::Panel::left("card_filters")
                .resizable(false)
                .default_size(240.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Filtros");
                        ui.add_enabled_ui(collection.has_collection, |ui| {
                            ui.checkbox(&mut self.form.owned_only, "Só cartas que eu tenho");
                        });
                        if !collection.has_collection {
                            action = collection_panel::controls(ui, collection);
                        }
                        ui.separator();
                        self.form.ui(ui);
                    });
                });
        }

        self.search_if_changed(catalog);
        TabOutput {
            detail: self.results_ui(ui, collection.has_collection, images),
            action,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = if self.show_filters {
                "Esconder filtros"
            } else {
                "Mostrar filtros"
            };
            if ui.button(label).clicked() {
                self.show_filters = !self.show_filters;
            }
            ui.add(
                egui::Slider::new(
                    &mut self.card_width,
                    card_grid::MIN_CARD_WIDTH..=card_grid::MAX_CARD_WIDTH,
                )
                .text("tamanho das cartas")
                .show_value(false),
            );
            ui.separator();
            ui.label(format!(
                "{} cartas encontradas (mostrando {})",
                self.total,
                self.found.len()
            ));
        });
        ui.separator();
    }

    fn search_if_changed(&mut self, catalog: &Catalog) {
        if self.searched.as_ref() == Some(&self.form) {
            return;
        }
        self.searched = Some(self.form.clone());

        if let Err(error) = self.search(catalog) {
            self.error = Some(format!("{error:#}"));
            self.found.clear();
            self.total = 0;
        }
    }

    fn search(&mut self, catalog: &Catalog) -> Result<()> {
        let result = catalog.search(&self.form.to_filter())?;

        self.found = result
            .cards
            .into_iter()
            .map(|card| {
                Ok(Found {
                    name: catalog.display_name(&card)?,
                    card,
                })
            })
            .collect::<Result<_>>()?;
        self.total = result.total;
        self.error = None;
        Ok(())
    }

    fn results_ui(
        &self,
        ui: &mut egui::Ui,
        has_collection: bool,
        images: &mut Images,
    ) -> Option<CardDetail> {
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            return None;
        }

        let tiles: Vec<Tile> = self
            .found
            .iter()
            .map(|found| Tile {
                name: &found.name,
                image_url: found.card.image_normal.as_deref(),
                badge: (has_collection && found.card.owned > 0).then(|| Badge {
                    text: format!("{}x", found.card.owned),
                    color: card_grid::OWNED_COLOR,
                }),
                dimmed: has_collection && found.card.owned == 0,
            })
            .collect();

        let clicked = card_grid::scrolling(ui, "card_results", &tiles, self.card_width, images)?;
        let found = &self.found[clicked];
        Some(CardDetail {
            name: found.name.clone(),
            record: Some(found.card.clone()),
        })
    }
}

impl Default for FilterForm {
    fn default() -> Self {
        Self {
            owned_only: false,
            name: String::new(),
            text: String::new(),
            type_line: String::new(),
            colors: [false; 6],
            exact_color: false,
            use_cmc: false,
            cmc_min: 0.0,
            cmc_max: 5.0,
            rarity: None,
            set: String::new(),
            format: None,
        }
    }
}

impl FilterForm {
    fn ui(&mut self, ui: &mut egui::Ui) {
        text_field(ui, "Nome (em inglês)", &mut self.name);
        text_field(ui, "Texto", &mut self.text);
        text_field(ui, "Tipo", &mut self.type_line);

        ui.add_space(6.0);
        ui.label("Cores");
        ui.horizontal_wrapped(|ui| {
            for (selected, (_, label)) in self.colors.iter_mut().zip(COLORS) {
                ui.checkbox(selected, label);
            }
        });
        ui.checkbox(&mut self.exact_color, "Exatamente estas cores");

        ui.add_space(6.0);
        ui.checkbox(&mut self.use_cmc, "Filtrar por CMC");
        ui.add_enabled_ui(self.use_cmc, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.cmc_min)
                        .range(0.0..=20.0)
                        .prefix("de "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.cmc_max)
                        .range(0.0..=20.0)
                        .prefix("até "),
                );
            });
        });

        ui.add_space(6.0);
        egui::ComboBox::from_label("Raridade")
            .selected_text(self.rarity.map_or("qualquer", Rarity::as_str))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.rarity, None, "qualquer");
                for rarity in Rarity::value_variants() {
                    ui.selectable_value(&mut self.rarity, Some(*rarity), rarity.as_str());
                }
            });

        egui::ComboBox::from_label("Formato")
            .selected_text(self.format.unwrap_or("qualquer"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.format, None, "qualquer");
                for format in FORMATS {
                    ui.selectable_value(&mut self.format, Some(format), format);
                }
            });

        text_field(ui, "Set (ex.: BLB)", &mut self.set);

        ui.add_space(10.0);
        if ui.button("Limpar filtros").clicked() {
            *self = Self::default();
        }
    }

    fn to_filter(&self) -> CardFilter {
        let colors: String = COLORS
            .iter()
            .zip(self.colors)
            .filter(|(_, selected)| *selected)
            .map(|((letter, _), _)| *letter)
            .collect();

        CardFilter {
            name: non_empty(&self.name),
            text: non_empty(&self.text),
            type_line: non_empty(&self.type_line),
            color: non_empty(&colors),
            exact_color: self.exact_color,
            cmc: None,
            cmc_min: self.use_cmc.then_some(self.cmc_min),
            cmc_max: self.use_cmc.then_some(self.cmc_max),
            rarity: self.rarity,
            set: non_empty(&self.set),
            format: self.format.map(str::to_string),
            owned: self.owned_only,
            limit: RESULT_LIMIT,
        }
    }
}

fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}
