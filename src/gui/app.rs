use super::TabOutput;
use super::card_detail::{self, CardDetail};
use super::cards_tab::CardsTab;
use super::collection_panel::{CollectionAction, CollectionInfo};
use super::collection_sync::{CollectionSync, SyncMessage};
use super::decks_tab::{Collection, DecksTab};
use super::images::Images;
use super::settings::{self, SettingsAction, SettingsInfo};
use anyhow::Result;
use eframe::egui;
use rust_mtg::card_store::{CollectionSummary, unix_now};
use rust_mtg::card_update::{self, UpdateOutcome};
use rust_mtg::catalog::Catalog;
use rust_mtg::config::Config;
use rust_mtg::decks::{Change, Decks};
use rust_mtg::language::Language;
use rust_mtg::live::LiveDecks;
use rust_mtg::{card_db, claude_session, paths};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Cards,
    Decks,
}

enum JobMessage {
    Step(String),
    Done(Result<UpdateOutcome>),
}

pub struct MtgApp {
    config: Config,
    log_path: PathBuf,
    card_db: PathBuf,
    catalog: Catalog,
    scryfall_cards: Option<i64>,
    live: LiveDecks,
    shown_decks: Decks,
    last_poll: Instant,
    tab: Tab,
    cards_tab: CardsTab,
    decks_tab: DecksTab,
    detail: Option<CardDetail>,
    images: Images,
    status: String,
    show_settings: bool,
    scryfall_job: Option<Receiver<JobMessage>>,
    collection_sync: CollectionSync,
    collection_summary: Option<CollectionSummary>,
    owned: Option<HashMap<String, u32>>,
    collection_status: String,
    syncing: bool,
}

impl MtgApp {
    pub fn new(
        ctx: &egui::Context,
        config: Config,
        log_path: PathBuf,
        card_db: PathBuf,
    ) -> Result<Self> {
        let catalog = Catalog::open(&card_db, config.language.unwrap_or_default())?;
        let (live, warnings) = LiveDecks::start(&log_path)?;
        let shown_decks = catalog.localized_decks(live.decks())?;

        let status = match warnings.first() {
            Some(warning) => format!("aviso: {warning:#}"),
            None => format!(
                "{} decks carregados · acompanhando o Player.log",
                shown_decks.len()
            ),
        };

        let collection_summary = catalog.collection_summary()?;
        let owned = match collection_summary {
            Some(_) => Some(catalog.owned_by_name()?),
            None => None,
        };
        let collection_sync = CollectionSync::start(ctx.clone(), card_db.clone());

        Ok(Self {
            collection_status: collection_text(collection_summary.as_ref()),
            syncing: false,
            collection_summary,
            owned,
            collection_sync,
            scryfall_cards: catalog.scryfall_count(),
            tab: if catalog.has_scryfall() {
                Tab::Cards
            } else {
                Tab::Decks
            },
            catalog,
            config,
            log_path,
            card_db,
            live,
            shown_decks,
            last_poll: Instant::now(),
            cards_tab: CardsTab::default(),
            decks_tab: DecksTab::default(),
            detail: None,
            images: Images::new(ctx.clone()),
            status,
            show_settings: false,
            scryfall_job: None,
        })
    }

    fn language(&self) -> Language {
        self.config.language.unwrap_or_default()
    }

    fn poll_live(&mut self) {
        if self.last_poll.elapsed() < POLL_INTERVAL {
            return;
        }
        self.last_poll = Instant::now();

        match self.live.poll() {
            Ok(poll) => {
                if let Some(warning) = poll.warnings.first() {
                    self.status = format!("aviso: {warning:#}");
                }
                if !poll.changes.is_empty() {
                    self.status = self.describe(&poll.changes);
                    self.refresh_decks();
                }
            }
            Err(error) => self.status = format!("aviso: {error:#}"),
        }
    }

    fn describe(&self, changes: &[Change]) -> String {
        changes
            .iter()
            .map(|change| {
                let (verb, deck) = match change {
                    Change::Added(deck) => ("criado", deck),
                    Change::Updated(deck) => ("atualizado", deck),
                    Change::Removed(deck) => ("apagado", deck),
                };
                let name = self
                    .catalog
                    .deck_name(&deck.name)
                    .unwrap_or_else(|_| deck.name.clone());
                format!("Deck \"{name}\" {verb}")
            })
            .collect::<Vec<_>>()
            .join("  ·  ")
    }

    fn refresh_decks(&mut self) {
        match self.catalog.localized_decks(self.live.decks()) {
            Ok(decks) => self.shown_decks = decks,
            Err(error) => self.status = format!("erro: {error:#}"),
        }
    }

    fn reload_catalog(&mut self) {
        match Catalog::open(&self.card_db, self.language()) {
            Ok(catalog) => {
                self.scryfall_cards = catalog.scryfall_count();
                self.catalog = catalog;
                self.refresh_decks();
                self.reload_collection();
            }
            Err(error) => self.status = format!("erro: {error:#}"),
        }
    }

    fn reload_collection(&mut self) {
        let loaded = self.catalog.collection_summary().and_then(|summary| {
            let owned = match summary {
                Some(_) => Some(self.catalog.owned_by_name()?),
                None => None,
            };
            Ok((summary, owned))
        });

        match loaded {
            Ok((summary, owned)) => {
                self.collection_summary = summary;
                self.owned = owned;
            }
            Err(error) => self.status = format!("erro lendo a coleção: {error:#}"),
        }
        self.cards_tab.invalidate();
        self.decks_tab.invalidate();
    }

    fn poll_collection(&mut self) {
        for message in self.collection_sync.messages() {
            self.syncing = false;
            match message {
                SyncMessage::Synced { changed } => {
                    if !self.catalog.has_scryfall() {
                        self.reload_catalog();
                    } else if changed || self.owned.is_none() {
                        self.reload_collection();
                    } else if let Ok(summary) = self.catalog.collection_summary() {
                        self.collection_summary = summary;
                    }
                    self.collection_status = collection_text(self.collection_summary.as_ref());
                }
                SyncMessage::Failed(error) => {
                    let saved = match &self.collection_summary {
                        Some(_) => " · usando a última coleção salva",
                        None => "",
                    };
                    self.collection_status = format!("Coleção indisponível ({error}){saved}");
                }
            }
        }
    }

    fn handle_collection_action(&mut self, action: CollectionAction) {
        match action {
            CollectionAction::Sync => {
                self.syncing = true;
                self.collection_status = "Sincronizando a coleção…".to_string();
                self.collection_sync.sync_now();
            }
        }
    }

    fn start_scryfall_update(&mut self, ctx: &egui::Context) {
        if self.scryfall_job.is_some() {
            return;
        }

        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let result = card_update::update_from_scryfall(|step| {
                let _ = sender.send(JobMessage::Step(step.to_string()));
                ctx.request_repaint();
            });
            let _ = sender.send(JobMessage::Done(result));
            ctx.request_repaint();
        });

        self.status = "Consultando a Scryfall...".to_string();
        self.scryfall_job = Some(receiver);
    }

    fn poll_scryfall_job(&mut self) {
        let Some(job) = &self.scryfall_job else {
            return;
        };
        let messages: Vec<JobMessage> = job.try_iter().collect();

        for message in messages {
            match message {
                JobMessage::Step(step) => self.status = step,
                JobMessage::Done(result) => {
                    self.scryfall_job = None;
                    self.reload_catalog();
                    self.status = match result {
                        Ok(UpdateOutcome::AlreadyCurrent(count)) => {
                            format!("Dados da Scryfall já atualizados ({count} cartas)")
                        }
                        Ok(UpdateOutcome::Imported(count)) => {
                            format!("{count} cartas importadas da Scryfall")
                        }
                        Err(error) => format!("erro ao atualizar a Scryfall: {error:#}"),
                    };
                }
            }
        }
    }

    fn apply(&mut self, action: SettingsAction, ctx: &egui::Context) {
        match action {
            SettingsAction::PickLog => {
                if let Some(path) = paths::pick_log_file() {
                    self.change_log(path);
                }
            }
            SettingsAction::PickCardDb => {
                if let Some(path) = card_db::pick_card_db() {
                    self.change_card_db(path);
                }
            }
            SettingsAction::ChangeLanguage(language) => {
                self.config.language = Some(language);
                self.save_config();
                self.reload_catalog();
            }
            SettingsAction::UpdateScryfall => self.start_scryfall_update(ctx),
            SettingsAction::Collection(action) => self.handle_collection_action(action),
        }
    }

    fn change_log(&mut self, path: PathBuf) {
        match LiveDecks::start(&path) {
            Ok((live, _)) => {
                self.live = live;
                self.config.log_path = Some(path.clone());
                self.log_path = path;
                self.save_config();
                self.refresh_decks();
                self.status = "Player.log alterado".to_string();
            }
            Err(error) => self.status = format!("erro: {error:#}"),
        }
    }

    fn change_card_db(&mut self, path: PathBuf) {
        self.config.raw_dir = path.parent().map(Path::to_path_buf);
        self.card_db = path;
        self.save_config();
        self.reload_catalog();
    }

    fn save_config(&mut self) {
        if let Err(error) = self.config.save() {
            self.status = format!("erro ao salvar a config: {error:#}");
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Cards, "Cartas");
                let decks_label = format!("Decks ({})", self.shown_decks.len());
                ui.selectable_value(&mut self.tab, Tab::Decks, decks_label);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Configurações").clicked() {
                        self.show_settings = true;
                    }

                    if self.syncing {
                        ui.spinner();
                    } else if ui
                        .button("Sincronizar coleção")
                        .on_hover_text("Lê a coleção direto do MTGA (o jogo precisa estar aberto)")
                        .clicked()
                    {
                        self.handle_collection_action(CollectionAction::Sync);
                    }

                    if ui
                        .button("Perguntar ao Claude")
                        .on_hover_text(
                            "Abre o Claude Code num terminal com acesso à sua coleção e decks",
                        )
                        .clicked()
                    {
                        self.status = match claude_session::open() {
                            Ok(()) => "Claude aberto num terminal novo".to_string(),
                            Err(error) => format!("erro: {error:#}"),
                        };
                    }
                });
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.scryfall_job.is_some() {
                    ui.spinner();
                }
                ui.label(self.status.as_str());

                let missing_scryfall =
                    self.scryfall_cards.unwrap_or(0) == 0 && self.scryfall_job.is_none();
                if missing_scryfall && ui.button("Baixar dados da Scryfall").clicked() {
                    self.start_scryfall_update(ctx);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(wildcards) = self.live.wildcards() {
                        ui.label(format!(
                            "Wildcards: {} C · {} U · {} R · {} M",
                            wildcards.common, wildcards.uncommon, wildcards.rare, wildcards.mythic
                        ));
                        ui.separator();
                    }

                    ui.label(self.collection_status.as_str());
                });
            });
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let info = SettingsInfo {
            log_path: &self.log_path,
            card_db: &self.card_db,
            language: self.config.language.unwrap_or_default(),
            scryfall_cards: self.scryfall_cards,
            updating: self.scryfall_job.is_some(),
            collection: CollectionInfo {
                has_collection: self.owned.is_some(),
                syncing: self.syncing,
                status: &self.collection_status,
            },
        };
        let action = settings::show(ctx, &mut self.show_settings, &info);

        if let Some(action) = action {
            self.apply(action, ctx);
        }
    }
}

fn collection_text(summary: Option<&CollectionSummary>) -> String {
    let Some(summary) = summary else {
        return "Coleção ainda não sincronizada".to_string();
    };

    let age = summary
        .synced_at
        .map(|synced_at| unix_now().saturating_sub(synced_at))
        .map(|seconds| match seconds {
            0..60 => "agora".to_string(),
            60..3600 => format!("há {} min", seconds / 60),
            _ => format!("há {} h", seconds / 3600),
        })
        .unwrap_or_default();

    format!(
        "Coleção: {} cartas ({} diferentes) · sincronizada {age}",
        summary.total, summary.unique
    )
}

impl eframe::App for MtgApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_live();
        self.poll_scryfall_job();
        self.poll_collection();
        ctx.request_repaint_after(POLL_INTERVAL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.top_bar(ui);
        self.status_bar(ui, &ctx);

        let info = CollectionInfo {
            has_collection: self.owned.is_some(),
            syncing: self.syncing,
            status: &self.collection_status,
        };
        let output: TabOutput = egui::CentralPanel::default()
            .show(ui, |ui| match self.tab {
                Tab::Cards => self
                    .cards_tab
                    .ui(ui, &self.catalog, &info, &mut self.images),
                Tab::Decks => {
                    let collection = Collection {
                        owned: self.owned.as_ref(),
                        wildcards: self.live.wildcards(),
                        info,
                    };
                    self.decks_tab.ui(
                        ui,
                        &self.shown_decks,
                        &self.catalog,
                        &collection,
                        &mut self.images,
                    )
                }
            })
            .inner;

        if output.detail.is_some() {
            self.detail = output.detail;
        }
        if let Some(action) = output.action {
            self.handle_collection_action(action);
        }

        card_detail::show(&ctx, &mut self.detail, &mut self.images);
        self.settings_window(&ctx);
    }
}
