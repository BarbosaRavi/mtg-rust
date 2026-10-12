use crate::card_store::{CardRecord, CardStore};
use crate::scryfall;
use anyhow::Result;
use std::fs;

pub enum UpdateOutcome {
    AlreadyCurrent(i64),
    Imported(i64),
}

pub fn update_from_scryfall(on_step: impl Fn(&str)) -> Result<UpdateOutcome> {
    let client = scryfall::client()?;
    let bulk = scryfall::bulk_file(&client)?;
    let mut store = CardStore::open()?;

    if store.version()?.as_deref() == Some(bulk.updated_at.as_str()) {
        return Ok(UpdateOutcome::AlreadyCurrent(store.count()?));
    }

    let download = scryfall::download_path()?;
    on_step(&format!(
        "Baixando {:.0} MB da Scryfall...",
        bulk.compressed_size as f64 / 1_048_576.0
    ));
    scryfall::download(&client, &bulk.jsonl_download_uri, &download)?;

    on_step("Importando as cartas do Arena...");
    let cards: Vec<CardRecord> = scryfall::read_arena_cards(&download)?
        .into_iter()
        .map(CardRecord::from)
        .collect();
    store.replace_all(&cards, &bulk.updated_at)?;
    fs::remove_file(&download)?;

    Ok(UpdateOutcome::Imported(store.count()?))
}
