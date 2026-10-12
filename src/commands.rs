use crate::output;
use anyhow::{Result, bail};
use rust_mtg::card_search::CardFilter;
use rust_mtg::card_store;
use rust_mtg::card_update::{self, UpdateOutcome};
use rust_mtg::catalog::Catalog;
use rust_mtg::collection;
use rust_mtg::completeness::{self, FactsCache};
use rust_mtg::config;
use rust_mtg::decks;
use rust_mtg::language::Language;
use rust_mtg::live::LiveDecks;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub struct App {
    pub log_path: PathBuf,
    pub card_db: PathBuf,
    pub language: Language,
}

impl App {
    fn catalog(&self) -> Result<Catalog> {
        Catalog::open(&self.card_db, self.language)
    }
}

pub fn list_decks(app: &App) -> Result<()> {
    let catalog = app.catalog()?;
    let decks = catalog.localized_decks(&decks::sync_from_logs(&app.log_path)?)?;
    println!("{}", output::deck_list(&decks::sorted_by_name(&decks)));
    Ok(())
}

pub fn show_deck(app: &App, query: &str) -> Result<()> {
    let catalog = app.catalog()?;
    let decks = catalog.localized_decks(&decks::sync_from_logs(&app.log_path)?)?;

    match decks::find_by_name(&decks, query).as_slice() {
        [] => bail!("nenhum deck com \"{query}\" no nome"),
        [deck] => {
            let completeness = match catalog.collection_summary()? {
                Some(_) => Some(completeness::completeness(
                    deck,
                    &catalog.owned_by_name()?,
                    &catalog,
                    &mut FactsCache::new(),
                )?),
                None => None,
            };
            println!("{}", output::deck(deck, &catalog, completeness.as_ref())?);
        }
        matches => {
            println!("Mais de um deck com \"{query}\" no nome:\n");
            println!("{}", output::deck_list(matches));
        }
    }

    Ok(())
}

pub fn show_card(app: &App, grp_id: u32) -> Result<()> {
    let catalog = app.catalog()?;
    println!("{}", output::card(&catalog.card(grp_id)?));

    if !catalog.has_scryfall() {
        println!("\n(rode `update-cards` para baixar os dados da Scryfall)");
    }
    Ok(())
}

pub fn search(app: &App, filter: &CardFilter) -> Result<()> {
    let catalog = app.catalog()?;
    let show_owned = catalog.collection_summary()?.is_some();
    println!(
        "{}",
        output::search_results(&catalog.search(filter)?, &catalog, show_owned)?
    );
    Ok(())
}

pub fn sync_collection(app: &App) -> Result<()> {
    let outcome = collection::sync_from_game(&app.card_db)?;

    if let Some(summary) = &outcome.summary {
        println!("{}", output::collection(summary, outcome.changed));
    }
    Ok(())
}

pub fn update_cards() -> Result<()> {
    match card_update::update_from_scryfall(|step| println!("{step}"))? {
        UpdateOutcome::AlreadyCurrent(count) => {
            println!("Dados da Scryfall já atualizados ({count} cartas).");
        }
        UpdateOutcome::Imported(count) => println!("{count} cartas importadas."),
    }
    Ok(())
}

pub fn watch(app: &App) -> Result<()> {
    let catalog = app.catalog()?;
    let (mut live, warnings) = LiveDecks::start(&app.log_path)?;
    print_warnings(&warnings);
    println!(
        "{} decks. Acompanhando {} (Ctrl+C para sair)\n",
        live.decks().len(),
        app.log_path.display()
    );

    loop {
        thread::sleep(POLL_INTERVAL);

        match live.poll() {
            Ok(poll) => {
                print_warnings(&poll.warnings);
                if !poll.changes.is_empty() {
                    println!("{}", output::changes(&poll.changes, &catalog)?);
                }
            }
            Err(error) => eprintln!("aviso: {error:#}"),
        }
    }
}

pub fn print_paths(app: &App) -> Result<()> {
    println!("Player.log:      {}", app.log_path.display());
    println!("Banco de cartas: {}", app.card_db.display());
    println!("Config:          {}", config::config_file()?.display());
    println!("Decks salvos:    {}", decks::store_file()?.display());
    println!("Cartas Scryfall: {}", card_store::store_file()?.display());
    Ok(())
}

fn print_warnings(warnings: &[anyhow::Error]) {
    for warning in warnings {
        eprintln!("aviso: {warning:#}");
    }
}
