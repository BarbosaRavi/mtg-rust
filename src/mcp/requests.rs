use anyhow::{Result, anyhow};
use clap::ValueEnum;
use rust_mtg::card_search::{CardFilter, Rarity};
use rust_mtg::decks::Deck;
use schemars::JsonSchema;
use serde::Deserialize;

const DEFAULT_LIMIT: usize = 30;
const MAX_LIMIT: usize = 100;

#[derive(Deserialize, JsonSchema)]
pub struct SearchCards {
    #[schemars(description = "Part of the English card name")]
    pub name: Option<String>,
    #[schemars(description = "Part of the rules text, e.g. 'draw a card'")]
    pub text: Option<String>,
    #[schemars(description = "Part of the type line, e.g. 'Creature', 'Dragon', 'Instant'")]
    pub type_line: Option<String>,
    #[schemars(
        description = "Color letters from WUBRG, e.g. 'R' or 'WU' ('C' = colorless). Cards must \
                       have all of them, and may have more unless exact_colors is true"
    )]
    pub colors: Option<String>,
    #[serde(default)]
    #[schemars(description = "Cards must have exactly the given colors")]
    pub exact_colors: bool,
    #[schemars(description = "Minimum mana value")]
    pub mana_value_min: Option<f64>,
    #[schemars(description = "Maximum mana value")]
    pub mana_value_max: Option<f64>,
    #[schemars(description = "common, uncommon, rare or mythic")]
    pub rarity: Option<String>,
    #[schemars(description = "Set code, e.g. 'FDN'")]
    pub set: Option<String>,
    #[schemars(
        description = "Format the card must be legal in, using Scryfall names: standard, \
                       alchemy, historic, timeless, explorer, pioneer, brawl, standardbrawl, \
                       gladiator"
    )]
    pub format: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "true = only cards the player owns; false = every Arena card, owned or not"
    )]
    pub owned_only: bool,
    #[schemars(description = "Maximum number of results (default 30, at most 100)")]
    pub limit: Option<usize>,
}

impl SearchCards {
    pub fn to_filter(&self) -> Result<CardFilter> {
        let rarity = match &self.rarity {
            Some(rarity) => Some(
                Rarity::from_str(rarity, true)
                    .map_err(|_| anyhow!("raridade inválida: {rarity}"))?,
            ),
            None => None,
        };

        Ok(CardFilter {
            name: self.name.clone(),
            text: self.text.clone(),
            type_line: self.type_line.clone(),
            color: self.colors.clone(),
            exact_color: self.exact_colors,
            cmc: None,
            cmc_min: self.mana_value_min,
            cmc_max: self.mana_value_max,
            rarity,
            set: self.set.clone(),
            format: self.format.clone(),
            owned: self.owned_only,
            limit: self.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT),
        })
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CardNames {
    #[schemars(description = "English card names, e.g. ['Lightning Strike', 'Shock']")]
    pub names: Vec<String>,
}

#[derive(Deserialize, JsonSchema, Default, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeckOrigin {
    #[default]
    All,
    Mine,
    Precon,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListDecks {
    #[serde(default)]
    #[schemars(
        description = "all (default), mine (decks the player built) or precon (ready-made decks)"
    )]
    pub origin: DeckOrigin,
    #[schemars(
        description = "Arena format name, e.g. 'Standard', 'Historic', 'Alchemy', 'HistoricBrawl'"
    )]
    pub format: Option<String>,
}

impl ListDecks {
    pub fn matches(&self, deck: &Deck) -> bool {
        let origin = match self.origin {
            DeckOrigin::All => true,
            DeckOrigin::Mine => !deck.precon,
            DeckOrigin::Precon => deck.precon,
        };
        let format = self.format.as_ref().is_none_or(|format| {
            deck.format
                .as_ref()
                .is_some_and(|deck_format| deck_format.eq_ignore_ascii_case(format))
        });
        origin && format
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct DeckName {
    #[schemars(description = "Deck name, part of it, or the deck id shown by list_decks")]
    pub name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CheckDeck {
    #[schemars(
        description = "Decklist in MTG Arena import format, one card per line, e.g. \
                       '4 Lightning Strike' or '4 Lightning Strike (FDN) 155'. Sideboard lines \
                       count too"
    )]
    pub decklist: String,
    #[schemars(
        description = "Optional format to check legality in, using Scryfall names, e.g. 'standard'"
    )]
    pub format: Option<String>,
}
