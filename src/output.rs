use anyhow::Result;
use rust_mtg::card_store::{CollectionSummary, SearchResult};
use rust_mtg::catalog::{CardView, Catalog};
use rust_mtg::completeness::Completeness;
use rust_mtg::decks::{CardCount, Change, Deck};
use rust_mtg::inventory::Wildcards;
use rust_mtg::player_log::EventSummary;
use std::collections::BTreeMap;

pub fn deck_list(decks: &[&Deck]) -> String {
    let mut lines: Vec<String> = decks
        .iter()
        .map(|deck| {
            format!(
                "{}  {:>3}  {:<19}  {}",
                deck.id.get(..8).unwrap_or(&deck.id),
                deck.main_deck_size(),
                deck.format.as_deref().unwrap_or("-"),
                deck.name
            )
        })
        .collect();
    lines.push(format!("\n{} decks", decks.len()));
    lines.join("\n")
}

pub fn deck(deck: &Deck, catalog: &Catalog, completeness: Option<&Completeness>) -> Result<String> {
    let mut lines = vec![format!(
        "{} ({})",
        deck.name,
        deck.format.as_deref().unwrap_or("-")
    )];
    section(
        &mut lines,
        "Deck principal",
        &deck.main_deck,
        catalog,
        completeness,
    )?;
    section(
        &mut lines,
        "Sideboard",
        &deck.sideboard,
        catalog,
        completeness,
    )?;
    section(
        &mut lines,
        "Zona de comando",
        &deck.command_zone,
        catalog,
        completeness,
    )?;
    section(&mut lines, "Companheiros", &deck.companions, catalog, None)?;

    match completeness {
        None => lines.push("\n(rode `sync-collection` para ver o que falta neste deck)".into()),
        Some(completeness) if completeness.is_complete() => {
            lines.push("\nVocê tem todas as cartas deste deck.".into());
        }
        Some(completeness) => {
            lines.push(format!(
                "\nFaltam {} cartas · wildcards necessários: {}",
                completeness.missing_total(),
                format_wildcards(&completeness.wildcards)
            ));
            for card in &completeness.missing {
                lines.push(format!(
                    "  {}x {} ({})",
                    card.missing,
                    card.name,
                    card.rarity.as_deref().unwrap_or("?")
                ));
            }
        }
    }

    if !catalog.has_scryfall() {
        lines.push("\n(rode `update-cards` para ver custo e tipo das cartas)".into());
    }
    Ok(lines.join("\n"))
}

pub fn collection(summary: &CollectionSummary, changed: bool) -> String {
    format!(
        "Coleção: {} cartas diferentes, {} no total{}",
        summary.unique,
        summary.total,
        if changed { "" } else { " (sem mudanças)" }
    )
}

pub fn format_wildcards(wildcards: &Wildcards) -> String {
    format!(
        "{} comuns, {} incomuns, {} raras, {} míticas",
        wildcards.common, wildcards.uncommon, wildcards.rare, wildcards.mythic
    )
}

pub fn card(card: &CardView) -> String {
    let mut lines = vec![format!("{}  (id {})", card.name, card.grp_id)];

    let Some(details) = &card.details else {
        lines.push("Sem dados da Scryfall para esta carta.".into());
        return lines.join("\n");
    };

    if details.name != card.name {
        lines.push(format!("Em inglês:  {}", details.name));
    }
    lines.push(format!(
        "Custo:      {} (CMC {})",
        details.mana_cost.as_deref().unwrap_or("-"),
        details.cmc
    ));
    lines.push(format!(
        "Tipo:       {}",
        details.type_line.as_deref().unwrap_or("-")
    ));
    lines.push(format!("Cores:      {}", or_dash(&details.colors)));
    lines.push(format!(
        "Raridade:   {} ({})",
        details.rarity, details.set_code
    ));
    lines.push(format!(
        "Legal em:   {}",
        details.legal_formats().join(", ")
    ));
    lines.push(format!(
        "Imagem:     {}",
        details.image_normal.as_deref().unwrap_or("-")
    ));

    if let Some(text) = &details.oracle_text {
        lines.push(format!("\n{text}"));
    }
    lines.join("\n")
}

pub fn search_results(
    result: &SearchResult,
    catalog: &Catalog,
    show_owned: bool,
) -> Result<String> {
    let mut lines = Vec::new();

    for card in &result.cards {
        let owned = if show_owned {
            format!("{:>3}  ", format!("{}x", card.owned))
        } else {
            String::new()
        };
        lines.push(format!(
            "{owned}{:<34} {:<16} {:<8} {:<5} {}",
            catalog.display_name(card)?,
            card.mana_cost.as_deref().unwrap_or(""),
            card.rarity,
            card.set_code,
            card.type_line.as_deref().unwrap_or("")
        ));
    }

    lines.push(format!(
        "\n{} cartas encontradas (mostrando {})",
        result.total,
        result.cards.len()
    ));
    Ok(lines.join("\n"))
}

pub fn changes(changes: &[Change], catalog: &Catalog) -> Result<String> {
    let mut lines = Vec::new();

    for change in changes {
        let (symbol, deck) = match change {
            Change::Added(deck) => ("+", deck),
            Change::Updated(deck) => ("~", deck),
            Change::Removed(deck) => ("-", deck),
        };
        lines.push(format!(
            "{symbol} {} ({}, {} cartas)",
            catalog.deck_name(&deck.name)?,
            deck.format.as_deref().unwrap_or("-"),
            deck.main_deck_size()
        ));
    }
    Ok(lines.join("\n"))
}

pub fn event_summary(summary: &BTreeMap<String, EventSummary>) -> String {
    let mut lines = Vec::new();

    for (name, event) in summary {
        lines.push(format!("{name} ({}x)", event.count));
        for key in &event.keys {
            lines.push(format!("    {key}"));
        }
    }
    lines.join("\n")
}

fn section(
    lines: &mut Vec<String>,
    title: &str,
    cards: &[CardCount],
    catalog: &Catalog,
    completeness: Option<&Completeness>,
) -> Result<()> {
    if cards.is_empty() {
        return Ok(());
    }

    let total: u32 = cards.iter().map(|card| card.quantity).sum();
    lines.push(format!("\n{title} ({total})"));

    for card in cards {
        let view = catalog.card(card.card_id)?;
        let missing = completeness
            .and_then(|completeness| completeness.missing_by_card.get(&card.card_id))
            .map(|missing| format!("  [faltam {missing}]"))
            .unwrap_or_default();

        lines.push(match &view.details {
            Some(details) => format!(
                "  {}x {:<32} {:<14} {}{missing}",
                card.quantity,
                view.name,
                details.mana_cost.as_deref().unwrap_or(""),
                details.type_line.as_deref().unwrap_or("")
            ),
            None => format!("  {}x {}{missing}", card.quantity, view.name),
        });
    }

    Ok(())
}

fn or_dash(text: &str) -> &str {
    if text.is_empty() { "-" } else { text }
}
