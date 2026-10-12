use super::requests::{CheckDeck, ListDecks, SearchCards};
use crate::output;
use anyhow::{Result, bail};
use rust_mtg::card_store::CardRecord;
use rust_mtg::catalog::Catalog;
use rust_mtg::completeness::{self, Completeness, FactsCache};
use rust_mtg::deck_check::{self, CheckedCard};
use rust_mtg::deck_colors::deck_colors;
use rust_mtg::decks::{self, Deck, Decks};
use rust_mtg::inventory::Wildcards;

const ARENA_FORMATS: [&str; 9] = [
    "standard",
    "alchemy",
    "historic",
    "timeless",
    "explorer",
    "pioneer",
    "brawl",
    "standardbrawl",
    "gladiator",
];

const NOT_SYNCED: &str = "warning: the collection was never synced, so owned counts are all 0";

pub fn wildcards(have: Option<Wildcards>) -> Result<String> {
    match have {
        Some(have) => Ok(wildcards_text(&have)),
        None => bail!("o Player.log não tem os wildcards: abra o MTGA e faça login"),
    }
}

pub fn search_cards(catalog: &Catalog, request: &SearchCards) -> Result<String> {
    let result = catalog.search_names(&request.to_filter()?)?;
    let mut lines = vec![format!(
        "{} cards match (showing {})",
        result.total,
        result.cards.len()
    )];

    for card in &result.cards {
        lines.push(String::new());
        lines.push(card_text(catalog, card, false)?);
    }
    Ok(lines.join("\n"))
}

pub fn card_details(catalog: &Catalog, names: &[String]) -> Result<String> {
    let mut blocks = Vec::new();

    for name in names {
        blocks.push(match catalog.card_by_name(name)? {
            Some(card) => card_text(catalog, &card, true)?,
            None => format!("{name}: not found (use search_cards to find the exact English name)"),
        });
    }
    Ok(blocks.join("\n\n"))
}

pub fn list_decks(catalog: &Catalog, decks: &Decks, request: &ListDecks) -> Result<String> {
    let owned = catalog.synced_collection()?;
    let mut cache = FactsCache::new();
    let mut lines = Vec::new();

    for deck in decks::sorted_by_name(decks) {
        if !request.matches(deck) {
            continue;
        }

        let colors = deck_colors(deck, catalog, &mut cache)?;
        let status = match &owned {
            Some(owned) => status_text(&completeness::completeness(
                deck, owned, catalog, &mut cache,
            )?),
            None => "collection not synced".to_string(),
        };
        lines.push(format!(
            "{} · {} · {} cards · {status}",
            deck_label(deck),
            if colors.is_empty() {
                "colorless"
            } else {
                &colors
            },
            deck.main_deck_size(),
        ));
    }

    lines.push(format!("\n{} decks", lines.len()));
    Ok(lines.join("\n"))
}

pub fn show_deck(catalog: &Catalog, decks: &Decks, name: &str) -> Result<String> {
    match decks::find_by_name(decks, name).as_slice() {
        [] => bail!("nenhum deck com \"{name}\" no nome"),
        [deck] => {
            let completeness = match catalog.synced_collection()? {
                Some(owned) => Some(completeness::completeness(
                    deck,
                    &owned,
                    catalog,
                    &mut FactsCache::new(),
                )?),
                None => None,
            };
            output::deck(deck, catalog, completeness.as_ref())
        }
        matches => {
            let labels: Vec<String> = matches.iter().map(|deck| deck_label(deck)).collect();
            Ok(format!(
                "More than one deck matches \"{name}\". Call show_deck again with the id of one \
                 of them:\n{}",
                labels.join("\n")
            ))
        }
    }
}

pub fn check_deck(
    catalog: &Catalog,
    request: &CheckDeck,
    have: Option<Wildcards>,
) -> Result<String> {
    let deck_lines = deck_check::parse_decklist(&request.decklist);
    if deck_lines.is_empty() {
        bail!("nenhuma carta na lista: use linhas como '4 Lightning Strike'");
    }

    let check = deck_check::check(&deck_lines, catalog)?;
    let format = request.format.as_deref().map(str::to_lowercase);

    let mut lines: Vec<String> = check
        .cards
        .iter()
        .map(|card| checked_card_text(card, format.as_deref()))
        .collect();

    lines.push(String::new());
    lines.push(format!("Total: {} cards", check.total_cards()));
    if check.missing_total() == 0 {
        lines.push("The player owns every card in this list.".to_string());
    } else {
        lines.push(format!(
            "Missing {} cards. Wildcards needed: {}",
            check.missing_total(),
            wildcards_text(&check.wildcards)
        ));
        lines.push(match have {
            Some(have) if have.covers(&check.wildcards) => format!(
                "The player's wildcards ({}) are enough.",
                wildcards_text(&have)
            ),
            Some(have) => format!(
                "The player's wildcards ({}) are NOT enough.",
                wildcards_text(&have)
            ),
            None => "The player's wildcards are unknown.".to_string(),
        });
    }

    if catalog.collection_summary()?.is_none() {
        lines.push(NOT_SYNCED.to_string());
    }
    Ok(lines.join("\n"))
}

fn card_text(catalog: &Catalog, card: &CardRecord, with_formats: bool) -> Result<String> {
    let craft = catalog.cheapest_rarity(card)?;
    let mut lines = vec![format!(
        "{} — {} · {} · owned {} · craft: {} wildcard",
        card.name,
        card.mana_cost
            .as_deref()
            .filter(|cost| !cost.is_empty())
            .unwrap_or("no cost"),
        card.type_line.as_deref().unwrap_or("?"),
        card.owned,
        craft.as_deref().unwrap_or("?"),
    )];

    if let Some(text) = &card.oracle_text {
        lines.push(text.clone());
    }
    if with_formats {
        let formats: Vec<String> = card
            .legal_formats()
            .into_iter()
            .filter(|format| ARENA_FORMATS.contains(&format.as_str()))
            .collect();
        lines.push(format!("Legal on Arena in: {}", formats.join(", ")));
    }
    Ok(lines.join("\n"))
}

fn checked_card_text(card: &CheckedCard, format: Option<&str>) -> String {
    let Some(record) = &card.record else {
        return format!(
            "{}x {} · NOT FOUND on Arena (check the English name)",
            card.line.quantity, card.line.name
        );
    };

    let mut notes = vec![format!("owned {}", card.owned())];
    if card.missing() > 0 {
        notes.push(format!(
            "missing {} (craft: {})",
            card.missing(),
            card.craft_rarity.as_deref().unwrap_or("?")
        ));
    }
    if let Some(format) = format
        && card.is_legal(format) == Some(false)
    {
        notes.push(format!("NOT legal in {format}"));
    }

    format!(
        "{}x {} · {}",
        card.line.quantity,
        record.name,
        notes.join(" · ")
    )
}

fn deck_label(deck: &Deck) -> String {
    format!(
        "{} · id {} · {} · {}",
        deck.name,
        deck.id.get(..8).unwrap_or(&deck.id),
        deck.format.as_deref().unwrap_or("-"),
        if deck.precon {
            "precon"
        } else {
            "built by the player"
        },
    )
}

fn status_text(status: &Completeness) -> String {
    if status.is_complete() {
        "complete".to_string()
    } else {
        format!(
            "missing {} cards (wildcards needed: {})",
            status.missing_total(),
            wildcards_text(&status.wildcards)
        )
    }
}

fn wildcards_text(wildcards: &Wildcards) -> String {
    format!(
        "{} common, {} uncommon, {} rare, {} mythic",
        wildcards.common, wildcards.uncommon, wildcards.rare, wildcards.mythic
    )
}
