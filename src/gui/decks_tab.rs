use super::TabOutput;
use super::card_detail::CardDetail;
use super::card_grid::{self, Badge, Tile};
use super::collection_panel::{self, CollectionAction, CollectionInfo};
use super::deck_filters::{DeckFacts, DeckFilters};
use super::images::Images;
use anyhow::Result;
use eframe::egui::{self, Color32};
use rust_mtg::catalog::{CardView, Catalog};
use rust_mtg::completeness::{self, Completeness, FactsCache};
use rust_mtg::deck_colors::deck_colors;
use rust_mtg::decks::{self, CardCount, Deck, Decks};
use rust_mtg::inventory::Wildcards;
use std::collections::HashMap;

const GOOD: Color32 = Color32::from_rgb(102, 187, 106);
const WARNING: Color32 = Color32::from_rgb(255, 167, 38);
const BAD: Color32 = Color32::from_rgb(239, 83, 80);
const NO_DECKS_HELP: &str = "Nenhum deck no Player.log ainda. No MTGA, ative Options → Account → \
                             \"Detailed Logs (Plugin Support)\", reinicie o jogo e faça login.";

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Grid,
    List,
}

pub struct Collection<'a> {
    pub owned: Option<&'a HashMap<String, u32>>,
    pub wildcards: Option<Wildcards>,
    pub info: CollectionInfo<'a>,
}

pub struct DecksTab {
    filters: DeckFilters,
    selected: Option<String>,
    view: Option<DeckView>,
    error: Option<String>,
    mode: ViewMode,
    card_width: f32,
    info: HashMap<String, DeckInfo>,
    facts: FactsCache,
}

struct DeckInfo {
    deck: Deck,
    colors: String,
    completeness: Option<Completeness>,
}

struct DeckView {
    deck: Deck,
    sections: Vec<Section>,
}

struct Section {
    title: &'static str,
    total: u32,
    rows: Vec<Row>,
}

struct Row {
    quantity: u32,
    card: CardView,
}

impl Default for DecksTab {
    fn default() -> Self {
        Self {
            filters: DeckFilters::default(),
            selected: None,
            view: None,
            error: None,
            mode: ViewMode::Grid,
            card_width: 150.0,
            info: HashMap::new(),
            facts: FactsCache::new(),
        }
    }
}

impl DecksTab {
    pub fn invalidate(&mut self) {
        self.view = None;
        self.info.clear();
        self.facts.clear();
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        decks: &Decks,
        catalog: &Catalog,
        collection: &Collection,
        images: &mut Images,
    ) -> TabOutput {
        self.refresh_info(decks, collection.owned, catalog);

        let action = egui::Panel::left("deck_list")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| self.list_ui(ui, decks, collection))
            .inner;

        self.refresh_view(decks, catalog);
        TabOutput {
            detail: self.deck_ui(ui, collection, images),
            action,
        }
    }

    fn refresh_info(
        &mut self,
        decks: &Decks,
        owned: Option<&HashMap<String, u32>>,
        catalog: &Catalog,
    ) {
        self.info.retain(|id, _| decks.contains_key(id));
        for (id, deck) in decks {
            let fresh = self.info.get(id).is_some_and(|info| &info.deck == deck);
            if fresh {
                continue;
            }

            match DeckInfo::build(deck, owned, catalog, &mut self.facts) {
                Ok(info) => {
                    self.info.insert(id.clone(), info);
                }
                Err(error) => {
                    self.error = Some(format!("{error:#}"));
                    return;
                }
            }
        }
    }

    fn list_ui(
        &mut self,
        ui: &mut egui::Ui,
        decks: &Decks,
        collection: &Collection,
    ) -> Option<CollectionAction> {
        let has_collection = collection.info.has_collection;
        self.filters.ui(ui, decks, has_collection);
        let action = if has_collection {
            None
        } else {
            collection_panel::controls(ui, &collection.info)
        };

        let shown: Vec<&Deck> = decks::find_by_name(decks, self.filters.name())
            .into_iter()
            .filter(|deck| {
                let info = self.info.get(&deck.id);
                let facts = DeckFacts {
                    colors: info.map_or("", |info| info.colors.as_str()),
                    completeness: info.and_then(|info| info.completeness.as_ref()),
                    wildcards: collection.wildcards,
                };
                self.filters.matches(deck, &facts)
            })
            .collect();
        ui.weak(format!(
            "Mostrando {} de {} decks",
            shown.len(),
            decks.len()
        ));
        if decks.is_empty() {
            ui.label(NO_DECKS_HELP);
        }
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for deck in shown {
                    let info = self.info.get(&deck.id);
                    let status = info.and_then(|info| info.completeness.as_ref());

                    let selected = self.selected.as_deref() == Some(deck.id.as_str());
                    ui.horizontal(|ui| {
                        let response = ui
                            .selectable_label(selected, deck.name.as_str())
                            .on_hover_text(hover_text(deck, info));
                        if response.clicked() {
                            self.selected = Some(deck.id.clone());
                        }
                        if let Some(status) = status {
                            status_label(ui, status);
                        }
                    });
                }
            });
        action
    }

    fn refresh_view(&mut self, decks: &Decks, catalog: &Catalog) {
        let current = self.selected.as_ref().and_then(|id| decks.get(id));
        let up_to_date = match (&self.view, current) {
            (Some(view), Some(deck)) => &view.deck == deck,
            (None, None) => true,
            _ => false,
        };
        if up_to_date {
            return;
        }

        self.view = None;
        if let Some(deck) = current {
            match DeckView::build(deck, catalog) {
                Ok(view) => self.view = Some(view),
                Err(error) => self.error = Some(format!("{error:#}")),
            }
        }
    }

    fn deck_ui(
        &mut self,
        ui: &mut egui::Ui,
        collection: &Collection,
        images: &mut Images,
    ) -> Option<CardDetail> {
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            return None;
        }
        let Some(view) = &self.view else {
            ui.label("Selecione um deck na lista.");
            return None;
        };
        let status = self
            .info
            .get(&view.deck.id)
            .and_then(|info| info.completeness.as_ref());

        ui.heading(view.deck.name.as_str());
        ui.label(format!(
            "{}  ·  {} cartas no deck principal",
            view.deck.format.as_deref().unwrap_or("-"),
            view.deck.main_deck_size()
        ));
        summary_ui(ui, status, collection);

        ui.separator();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.mode, ViewMode::Grid, "Grade");
            ui.selectable_value(&mut self.mode, ViewMode::List, "Lista");
            if self.mode == ViewMode::Grid {
                ui.add(
                    egui::Slider::new(
                        &mut self.card_width,
                        card_grid::MIN_CARD_WIDTH..=card_grid::MAX_CARD_WIDTH,
                    )
                    .text("tamanho das cartas")
                    .show_value(false),
                );
            }
        });

        let mut clicked = None;
        egui::ScrollArea::vertical()
            .id_salt("deck_cards")
            .auto_shrink(false)
            .show(ui, |ui| {
                for section in &view.sections {
                    ui.add_space(8.0);
                    ui.strong(format!("{} ({})", section.title, section.total));

                    let picked = match self.mode {
                        ViewMode::Grid => {
                            let tiles = section_tiles(section, status);
                            card_grid::inline(ui, &tiles, self.card_width, images)
                        }
                        ViewMode::List => section_list(ui, section, status, images),
                    };
                    if let Some(index) = picked {
                        clicked = Some(CardDetail::from(&section.rows[index].card));
                    }
                }
            });
        clicked
    }
}

impl DeckInfo {
    fn build(
        deck: &Deck,
        owned: Option<&HashMap<String, u32>>,
        catalog: &Catalog,
        facts: &mut FactsCache,
    ) -> Result<Self> {
        let completeness = owned
            .map(|owned| completeness::completeness(deck, owned, catalog, facts))
            .transpose()?;

        Ok(Self {
            deck: deck.clone(),
            colors: deck_colors(deck, catalog, facts)?,
            completeness,
        })
    }
}

impl DeckView {
    fn build(deck: &Deck, catalog: &Catalog) -> Result<Self> {
        let sections = [
            ("Deck principal", &deck.main_deck),
            ("Sideboard", &deck.sideboard),
            ("Zona de comando", &deck.command_zone),
            ("Companheiros", &deck.companions),
        ]
        .into_iter()
        .filter(|(_, cards)| !cards.is_empty())
        .map(|(title, cards)| Section::build(title, cards, catalog))
        .collect::<Result<_>>()?;

        Ok(Self {
            deck: deck.clone(),
            sections,
        })
    }
}

impl Section {
    fn build(title: &'static str, cards: &[CardCount], catalog: &Catalog) -> Result<Self> {
        let rows = cards
            .iter()
            .map(|card| {
                Ok(Row {
                    quantity: card.quantity,
                    card: catalog.card(card.card_id)?,
                })
            })
            .collect::<Result<_>>()?;

        Ok(Self {
            title,
            total: cards.iter().map(|card| card.quantity).sum(),
            rows,
        })
    }
}

fn missing_for(row: &Row, status: Option<&Completeness>) -> u32 {
    status
        .and_then(|status| status.missing_by_card.get(&row.card.grp_id))
        .copied()
        .unwrap_or(0)
}

fn section_tiles<'a>(section: &'a Section, status: Option<&Completeness>) -> Vec<Tile<'a>> {
    section
        .rows
        .iter()
        .map(|row| {
            let missing = missing_for(row, status);
            let badge = if missing > 0 {
                Badge {
                    text: format!("{}x · faltam {missing}", row.quantity),
                    color: card_grid::MISSING_COLOR,
                }
            } else {
                Badge {
                    text: format!("{}x", row.quantity),
                    color: card_grid::NEUTRAL_COLOR,
                }
            };

            Tile {
                name: &row.card.name,
                image_url: row
                    .card
                    .details
                    .as_ref()
                    .and_then(|card| card.image_normal.as_deref()),
                badge: Some(badge),
                dimmed: missing > 0,
            }
        })
        .collect()
}

fn section_list(
    ui: &mut egui::Ui,
    section: &Section,
    status: Option<&Completeness>,
    images: &mut Images,
) -> Option<usize> {
    let mut clicked = None;

    egui::Grid::new(section.title)
        .striped(true)
        .num_columns(5)
        .show(ui, |ui| {
            for (index, row) in section.rows.iter().enumerate() {
                let details = row.card.details.as_ref();
                ui.label(format!("{}x", row.quantity));

                let response = ui.link(row.card.name.as_str());
                if response.clicked() {
                    clicked = Some(index);
                }
                show_image_on_hover(response, &row.card, images);

                ui.label(
                    details
                        .and_then(|card| card.mana_cost.as_deref())
                        .unwrap_or(""),
                );
                ui.label(
                    details
                        .and_then(|card| card.type_line.as_deref())
                        .unwrap_or(""),
                );
                match missing_for(row, status) {
                    0 => ui.label(""),
                    missing => ui.colored_label(BAD, format!("faltam {missing}")),
                };
                ui.end_row();
            }
        });
    clicked
}

fn summary_ui(ui: &mut egui::Ui, status: Option<&Completeness>, collection: &Collection) {
    if collection.owned.is_none() {
        ui.weak("Sincronize a coleção para ver o que falta neste deck.");
        return;
    }
    let Some(status) = status else {
        return;
    };

    if status.is_complete() {
        ui.colored_label(GOOD, "✔ Você tem todas as cartas deste deck.");
        return;
    }

    ui.colored_label(WARNING, format!("Faltam {} cartas", status.missing_total()));
    ui.label(format!(
        "Wildcards necessários: {}",
        wildcard_text(&status.wildcards)
    ));
    if let Some(have) = collection.wildcards {
        ui.label(format!("Seus wildcards: {}", wildcard_text(&have)));
        if have.covers(&status.wildcards) {
            ui.colored_label(GOOD, "✔ Dá para completar com os seus wildcards");
        } else {
            ui.colored_label(BAD, "✖ Os seus wildcards não bastam");
        }
    }
}

fn hover_text(deck: &Deck, info: Option<&DeckInfo>) -> String {
    let colors = match info.map(|info| info.colors.as_str()) {
        Some("") => "incolor",
        Some(colors) => colors,
        None => "-",
    };
    format!("{} · {colors}", deck.format.as_deref().unwrap_or("-"))
}

fn status_label(ui: &mut egui::Ui, status: &Completeness) {
    if status.is_complete() {
        ui.colored_label(GOOD, "✔");
    } else {
        ui.colored_label(WARNING, format!("faltam {}", status.missing_total()));
    }
}

fn wildcard_text(wildcards: &Wildcards) -> String {
    format!(
        "{} comuns · {} incomuns · {} raras · {} míticas",
        wildcards.common, wildcards.uncommon, wildcards.rare, wildcards.mythic
    )
}

fn show_image_on_hover(response: egui::Response, card: &CardView, images: &mut Images) {
    if !response.hovered() {
        return;
    }
    let Some(url) = card
        .details
        .as_ref()
        .and_then(|card| card.image_normal.as_deref())
    else {
        return;
    };

    match images.uri(url) {
        Some(uri) => response.on_hover_ui(|ui| {
            ui.add(egui::Image::new(uri).max_width(250.0));
        }),
        None => response.on_hover_ui(|ui| {
            ui.spinner();
        }),
    };
}
