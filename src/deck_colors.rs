use crate::card_search::in_color_order;
use crate::catalog::Catalog;
use crate::completeness::{FactsCache, facts_for};
use crate::decks::Deck;
use anyhow::Result;

pub fn deck_colors(deck: &Deck, catalog: &Catalog, cache: &mut FactsCache) -> Result<String> {
    let mut letters = String::new();
    for entry in deck.main_deck.iter().chain(&deck.command_zone) {
        letters.push_str(&facts_for(entry.card_id, catalog, cache)?.colors);
    }
    Ok(in_color_order(&letters))
}
