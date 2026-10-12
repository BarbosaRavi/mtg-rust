use crate::player_log::{self, Direction, LogEvent};
use crate::{config, storage};
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub type Decks = BTreeMap<String, Deck>;

pub const LOC_PREFIX: &str = "?=?Loc/";

pub enum Change {
    Added(Deck),
    Updated(Deck),
    Removed(Deck),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub format: Option<String>,
    pub main_deck: Vec<CardCount>,
    pub sideboard: Vec<CardCount>,
    pub command_zone: Vec<CardCount>,
    pub companions: Vec<CardCount>,
    #[serde(default)]
    pub precon: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardCount {
    #[serde(rename = "cardId")]
    pub card_id: u32,
    pub quantity: u32,
}

impl Deck {
    fn new(summary: Summary, cards: Cards) -> Self {
        let format = summary
            .attributes
            .into_iter()
            .find(|attribute| attribute.name == "Format")
            .map(|attribute| attribute.value);

        Self {
            precon: summary.name.starts_with(LOC_PREFIX),
            id: summary.deck_id,
            name: summary.name,
            format,
            main_deck: cards.main_deck,
            sideboard: cards.sideboard,
            command_zone: cards.command_zone,
            companions: cards.companions,
        }
    }

    pub fn main_deck_size(&self) -> u32 {
        self.main_deck.iter().map(|card| card.quantity).sum()
    }

    pub fn cards_to_own(&self) -> impl Iterator<Item = &CardCount> {
        self.main_deck
            .iter()
            .chain(&self.sideboard)
            .chain(&self.command_zone)
    }
}

pub fn sync_from_logs(log_path: &Path) -> Result<Decks> {
    let decks = read_from_logs(log_path)?;
    save(&decks)?;
    Ok(decks)
}

pub fn read_from_logs(log_path: &Path) -> Result<Decks> {
    let mut decks = load_with_previous_log(log_path)?;

    let text = player_log::read_log(log_path)?;
    apply_log(&mut decks, &text).with_context(|| format!("erro lendo {}", log_path.display()))?;
    Ok(decks)
}

pub fn load_with_previous_log(log_path: &Path) -> Result<Decks> {
    let mut decks = load_saved()?;

    if let Some(previous) = player_log::previous_log(log_path) {
        let text = player_log::read_log(&previous)?;
        apply_log(&mut decks, &text)
            .with_context(|| format!("erro lendo {}", previous.display()))?;
    }

    Ok(decks)
}

pub fn apply_log(decks: &mut Decks, text: &str) -> Result<()> {
    for event in player_log::events(text) {
        apply_event(decks, &event)?;
    }

    Ok(())
}

pub fn apply_event(decks: &mut Decks, event: &LogEvent) -> Result<()> {
    match (event.direction, event.name) {
        (Direction::Response, "StartHook") => apply_start_hook(decks, event.body),
        (Direction::Request, "DeckUpsertDeckV3") => apply_upsert(decks, event.body),
        (Direction::Request, "DeckDeleteDeck") => apply_delete(decks, event.body),
        _ => Ok(()),
    }
}

pub fn changes(before: &Decks, after: &Decks) -> Vec<Change> {
    let mut changes = Vec::new();

    for (id, deck) in after {
        match before.get(id) {
            None => changes.push(Change::Added(deck.clone())),
            Some(old) if old != deck => changes.push(Change::Updated(deck.clone())),
            Some(_) => {}
        }
    }

    for (id, deck) in before {
        if !after.contains_key(id) {
            changes.push(Change::Removed(deck.clone()));
        }
    }

    changes
}

pub fn sorted_by_name(decks: &Decks) -> Vec<&Deck> {
    let mut sorted: Vec<&Deck> = decks.values().collect();
    sorted.sort_by_key(|deck| deck.name.to_lowercase());
    sorted
}

pub fn find_by_name<'a>(decks: &'a Decks, query: &str) -> Vec<&'a Deck> {
    let query = query.to_lowercase();

    let matches: Vec<&Deck> = sorted_by_name(decks)
        .into_iter()
        .filter(|deck| deck.name.to_lowercase().contains(&query) || deck.id.starts_with(&query))
        .collect();

    let exact: Vec<&Deck> = matches
        .iter()
        .copied()
        .filter(|deck| deck.name.to_lowercase() == query)
        .collect();

    if exact.is_empty() { matches } else { exact }
}

pub fn store_file() -> Result<PathBuf> {
    Ok(config::project_dirs()?.data_dir().join("decks.json"))
}

fn load_saved() -> Result<Decks> {
    let file = store_file()?;

    let mut decks: Decks = match storage::read_if_exists(&file)? {
        Some(text) => serde_json::from_str(&text)
            .with_context(|| format!("decks inválidos em {}", file.display()))?,
        None => Decks::new(),
    };

    for deck in decks.values_mut() {
        deck.precon |= deck.name.starts_with(LOC_PREFIX);
    }
    Ok(decks)
}

pub fn save(decks: &Decks) -> Result<()> {
    storage::write_creating_dirs(&store_file()?, &serde_json::to_string_pretty(decks)?)
}

fn apply_start_hook(decks: &mut Decks, body: &str) -> Result<()> {
    let hook: StartHook = serde_json::from_str(body).context("StartHook inválido")?;
    if hook.decks_internal.is_empty() {
        return Ok(());
    }

    let mut cards_by_id = hook.decks_internal;
    decks.clear();

    for summary in hook.deck_summaries {
        if let Some(cards) = cards_by_id.remove(&summary.deck_id) {
            let deck = Deck::new(summary, cards);
            decks.insert(deck.id.clone(), deck);
        }
    }

    Ok(())
}

fn apply_upsert(decks: &mut Decks, body: &str) -> Result<()> {
    let request: UpsertRequest = parse_request(body).context("DeckUpsertDeckV3 inválido")?;
    let deck = Deck::new(request.summary, request.deck);
    decks.insert(deck.id.clone(), deck);
    Ok(())
}

fn apply_delete(decks: &mut Decks, body: &str) -> Result<()> {
    let request: DeleteRequest = parse_request(body).context("DeckDeleteDeck inválido")?;
    decks.remove(&request.deck_id);
    Ok(())
}

fn parse_request<T: DeserializeOwned>(body: &str) -> Result<T> {
    let envelope: RequestEnvelope = serde_json::from_str(body)?;
    Ok(serde_json::from_str(&envelope.request)?)
}

#[derive(Deserialize)]
struct RequestEnvelope {
    request: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StartHook {
    #[serde(default)]
    deck_summaries: Vec<Summary>,
    #[serde(default)]
    decks_internal: HashMap<String, Cards>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Summary {
    #[serde(alias = "DeckIdInternal")]
    deck_id: String,
    name: String,
    #[serde(default)]
    attributes: Vec<Attribute>,
}

#[derive(Deserialize)]
struct Attribute {
    name: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Cards {
    #[serde(default)]
    main_deck: Vec<CardCount>,
    #[serde(default)]
    sideboard: Vec<CardCount>,
    #[serde(default)]
    command_zone: Vec<CardCount>,
    #[serde(default)]
    companions: Vec<CardCount>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UpsertRequest {
    summary: Summary,
    deck: Cards,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DeleteRequest {
    deck_id: String,
}
