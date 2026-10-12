use crate::arena_collection;
use crate::arena_db::ArenaDb;
use crate::card_store::{self, CardStore, CollectionSummary, OwnedRow};
use crate::language::Language;
use anyhow::Result;
use std::path::Path;

pub struct SyncOutcome {
    pub changed: bool,
    pub summary: Option<CollectionSummary>,
}

pub fn sync_from_game(card_db: &Path) -> Result<SyncOutcome> {
    let cards = arena_collection::read_owned_cards()?;
    let arena = ArenaDb::open(card_db, Language::EnUs)?;

    let mut rows = Vec::new();
    for card in &cards {
        let name = arena
            .english_card_name(card.grp_id)?
            .unwrap_or_else(|| format!("#{}", card.grp_id));
        rows.push(OwnedRow {
            grp_id: card.grp_id,
            name: card_store::front_name(&name).to_string(),
            owned: card.owned,
        });
    }

    let mut store = CardStore::open()?;
    let changed = store.replace_collection(&rows)?;
    Ok(SyncOutcome {
        changed,
        summary: store.collection_summary()?,
    })
}
