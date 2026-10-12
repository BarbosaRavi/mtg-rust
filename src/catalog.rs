use crate::arena_db::ArenaDb;
use crate::card_search::CardFilter;
use crate::card_store::{self, CardRecord, CardStore, CollectionSummary, SearchResult};
use crate::decks::Decks;
use crate::language::Language;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CardFacts {
    pub name: String,
    pub rarity: Option<String>,
    pub colors: String,
    pub basic_land: bool,
}

pub struct Catalog {
    arena: ArenaDb,
    store: Option<CardStore>,
    language: Language,
}

pub struct CardView {
    pub grp_id: u32,
    pub name: String,
    pub details: Option<CardRecord>,
}

impl Catalog {
    pub fn open(card_db: &Path, language: Language) -> Result<Self> {
        Ok(Self {
            arena: ArenaDb::open(card_db, language)?,
            store: CardStore::open_if_exists()?,
            language,
        })
    }

    pub fn has_scryfall(&self) -> bool {
        self.store.is_some()
    }

    pub fn scryfall_count(&self) -> Option<i64> {
        self.store.as_ref().and_then(|store| store.count().ok())
    }

    pub fn search(&self, filter: &CardFilter) -> Result<SearchResult> {
        self.store()?.search(filter)
    }

    pub fn search_names(&self, filter: &CardFilter) -> Result<SearchResult> {
        self.store()?.search_names(filter)
    }

    pub fn card_by_name(&self, name: &str) -> Result<Option<CardRecord>> {
        self.store()?
            .by_name_ignoring_case(card_store::front_name(name.trim()))
    }

    pub fn cheapest_rarity(&self, card: &CardRecord) -> Result<Option<String>> {
        self.store()?
            .cheapest_rarity(card_store::front_name(&card.name))
    }

    pub fn synced_collection(&self) -> Result<Option<HashMap<String, u32>>> {
        match self.collection_summary()? {
            Some(_) => self.owned_by_name().map(Some),
            None => Ok(None),
        }
    }

    fn store(&self) -> Result<&CardStore> {
        self.store
            .as_ref()
            .context("sem dados da Scryfall: rode `update-cards` primeiro")
    }

    pub fn display_name(&self, card: &CardRecord) -> Result<String> {
        let localized = match (self.language, card.arena_id) {
            (Language::EnUs, _) | (_, None) => None,
            (_, Some(arena_id)) => self.arena.card_name(arena_id)?,
        };
        Ok(localized.unwrap_or_else(|| card.name.clone()))
    }

    pub fn card(&self, grp_id: u32) -> Result<CardView> {
        let name = self
            .arena
            .card_name(grp_id)?
            .unwrap_or_else(|| format!("#{grp_id}"));

        let details = match &self.store {
            Some(store) => self.details(store, grp_id)?,
            None => None,
        };

        Ok(CardView {
            grp_id,
            name,
            details,
        })
    }

    pub fn deck_name(&self, raw_name: &str) -> Result<String> {
        Ok(self
            .arena
            .deck_name(raw_name)?
            .unwrap_or_else(|| raw_name.to_string()))
    }

    pub fn localized_decks(&self, decks: &Decks) -> Result<Decks> {
        decks
            .iter()
            .map(|(id, deck)| {
                let mut deck = deck.clone();
                deck.name = self.deck_name(&deck.name)?;
                Ok((id.clone(), deck))
            })
            .collect()
    }

    pub fn collection_summary(&self) -> Result<Option<CollectionSummary>> {
        match &self.store {
            Some(store) => store.collection_summary(),
            None => Ok(None),
        }
    }

    pub fn owned_by_name(&self) -> Result<HashMap<String, u32>> {
        match &self.store {
            Some(store) => store.owned_by_name(),
            None => Ok(HashMap::new()),
        }
    }

    pub fn card_facts(&self, grp_id: u32) -> Result<CardFacts> {
        let name = self
            .arena
            .english_card_name(grp_id)?
            .map(|name| card_store::front_name(&name).to_string())
            .unwrap_or_else(|| format!("#{grp_id}"));

        let details = match &self.store {
            Some(store) => self.details(store, grp_id)?,
            None => None,
        };

        Ok(CardFacts {
            name,
            rarity: details.as_ref().map(|card| card.rarity.clone()),
            colors: details
                .as_ref()
                .map(|card| card.colors.clone())
                .unwrap_or_default(),
            basic_land: details.as_ref().is_some_and(CardRecord::is_basic_land),
        })
    }

    fn details(&self, store: &CardStore, grp_id: u32) -> Result<Option<CardRecord>> {
        if let Some(card) = store.by_arena_id(grp_id)? {
            return Ok(Some(card));
        }

        match self.arena.english_card_name(grp_id)? {
            Some(name) => store.by_name(card_store::front_name(&name)),
            None => Ok(None),
        }
    }
}
