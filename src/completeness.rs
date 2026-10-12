use crate::catalog::{CardFacts, Catalog};
use crate::decks::Deck;
use crate::inventory::Wildcards;
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};

pub type FactsCache = HashMap<u32, CardFacts>;

#[derive(Debug, Clone, PartialEq)]
pub struct MissingCard {
    pub grp_id: u32,
    pub name: String,
    pub missing: u32,
    pub rarity: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completeness {
    pub missing: Vec<MissingCard>,
    pub missing_by_card: HashMap<u32, u32>,
    pub wildcards: Wildcards,
}

impl Completeness {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }

    pub fn missing_total(&self) -> u32 {
        self.missing.iter().map(|card| card.missing).sum()
    }
}

pub fn completeness(
    deck: &Deck,
    owned: &HashMap<String, u32>,
    catalog: &Catalog,
    cache: &mut FactsCache,
) -> Result<Completeness> {
    let mut available: HashMap<String, u32> = HashMap::new();
    let mut missing_by_name: BTreeMap<String, MissingCard> = BTreeMap::new();
    let mut result = Completeness::default();

    for entry in deck.cards_to_own() {
        let facts = facts_for(entry.card_id, catalog, cache)?;
        if facts.basic_land {
            continue;
        }

        let left = available
            .entry(facts.name.clone())
            .or_insert_with(|| owned.get(&facts.name).copied().unwrap_or(0));
        let used = entry.quantity.min(*left);
        *left -= used;

        let missing = entry.quantity - used;
        if missing == 0 {
            continue;
        }

        *result.missing_by_card.entry(entry.card_id).or_default() += missing;
        if let Some(rarity) = &facts.rarity {
            result.wildcards.add(rarity, missing);
        }
        missing_by_name
            .entry(facts.name.clone())
            .or_insert_with(|| MissingCard {
                grp_id: entry.card_id,
                name: facts.name.clone(),
                missing: 0,
                rarity: facts.rarity.clone(),
            })
            .missing += missing;
    }

    result.missing = missing_by_name.into_values().collect();
    Ok(result)
}

pub(crate) fn facts_for(
    grp_id: u32,
    catalog: &Catalog,
    cache: &mut FactsCache,
) -> Result<CardFacts> {
    if let Some(facts) = cache.get(&grp_id) {
        return Ok(facts.clone());
    }

    let facts = catalog.card_facts(grp_id)?;
    cache.insert(grp_id, facts.clone());
    Ok(facts)
}
