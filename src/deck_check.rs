use crate::card_store::CardRecord;
use crate::catalog::Catalog;
use crate::inventory::Wildcards;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckLine {
    pub quantity: u32,
    pub name: String,
}

pub struct CheckedCard {
    pub line: DeckLine,
    pub record: Option<CardRecord>,
    pub craft_rarity: Option<String>,
}

pub struct DeckCheck {
    pub cards: Vec<CheckedCard>,
    pub wildcards: Wildcards,
}

impl CheckedCard {
    pub fn owned(&self) -> u32 {
        self.record.as_ref().map_or(0, |card| card.owned)
    }

    pub fn missing(&self) -> u32 {
        match &self.record {
            Some(card) if card.is_basic_land() => 0,
            _ => self.line.quantity.saturating_sub(self.owned()),
        }
    }

    pub fn is_legal(&self, format: &str) -> Option<bool> {
        let card = self.record.as_ref()?;
        Some(card.legal_formats().iter().any(|legal| legal == format))
    }
}

impl DeckCheck {
    pub fn total_cards(&self) -> u32 {
        self.cards.iter().map(|card| card.line.quantity).sum()
    }

    pub fn missing_total(&self) -> u32 {
        self.cards.iter().map(CheckedCard::missing).sum()
    }
}

pub fn parse_decklist(text: &str) -> Vec<DeckLine> {
    let mut lines: Vec<DeckLine> = Vec::new();

    for line in text.lines().filter_map(parse_line) {
        match lines
            .iter_mut()
            .find(|seen| seen.name.eq_ignore_ascii_case(&line.name))
        {
            Some(seen) => seen.quantity += line.quantity,
            None => lines.push(line),
        }
    }
    lines
}

pub fn check(lines: &[DeckLine], catalog: &Catalog) -> Result<DeckCheck> {
    let mut result = DeckCheck {
        cards: Vec::new(),
        wildcards: Wildcards::default(),
    };

    for line in lines {
        let record = catalog.card_by_name(&line.name)?;
        let craft_rarity = match &record {
            Some(card) => catalog.cheapest_rarity(card)?,
            None => None,
        };
        let card = CheckedCard {
            line: line.clone(),
            record,
            craft_rarity,
        };

        if let Some(rarity) = &card.craft_rarity {
            result.wildcards.add(rarity, card.missing());
        }
        result.cards.push(card);
    }

    Ok(result)
}

fn parse_line(line: &str) -> Option<DeckLine> {
    let (count, rest) = line.trim().split_once(char::is_whitespace)?;
    let quantity = count.trim_end_matches(['x', 'X']).parse().ok()?;
    let name = rest.split(" (").next().unwrap_or(rest).trim();

    if quantity == 0 || name.is_empty() {
        return None;
    }
    Some(DeckLine {
        quantity,
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(quantity: u32, name: &str) -> DeckLine {
        DeckLine {
            quantity,
            name: name.to_string(),
        }
    }

    #[test]
    fn parses_arena_export_format() {
        let text =
            "Deck\n4 Lightning Strike (FDN) 155\n2x Shock\n\nSideboard\n1 Abrade (LCI) 136\n";

        assert_eq!(
            parse_decklist(text),
            vec![
                line(4, "Lightning Strike"),
                line(2, "Shock"),
                line(1, "Abrade")
            ]
        );
    }

    #[test]
    fn merges_repeated_cards_and_skips_junk() {
        let text = "Companion\n2 Shock\nnot a card\n0 Opt\n1 shock\n";

        assert_eq!(parse_decklist(text), vec![line(3, "Shock")]);
    }
}
