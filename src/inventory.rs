use crate::player_log::{self, Direction, LogEvent};
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Wildcards {
    #[serde(rename = "WildCardCommons", default)]
    pub common: u32,
    #[serde(rename = "WildCardUnCommons", default)]
    pub uncommon: u32,
    #[serde(rename = "WildCardRares", default)]
    pub rare: u32,
    #[serde(rename = "WildCardMythics", default)]
    pub mythic: u32,
}

impl Wildcards {
    pub fn add(&mut self, rarity: &str, amount: u32) {
        match rarity {
            "common" => self.common += amount,
            "uncommon" => self.uncommon += amount,
            "rare" => self.rare += amount,
            "mythic" => self.mythic += amount,
            _ => {}
        }
    }

    pub fn covers(&self, needed: &Wildcards) -> bool {
        self.common >= needed.common
            && self.uncommon >= needed.uncommon
            && self.rare >= needed.rare
            && self.mythic >= needed.mythic
    }

    pub fn is_empty(&self) -> bool {
        *self == Wildcards::default()
    }
}

#[derive(Deserialize)]
struct WithInventory {
    #[serde(rename = "InventoryInfo")]
    inventory: Option<Wildcards>,
}

pub fn wildcards_from_logs(log_path: &Path) -> Result<Option<Wildcards>> {
    let mut logs = vec![log_path.to_path_buf()];
    logs.extend(player_log::previous_log(log_path));

    for log in logs {
        let text = player_log::read_log(&log)?;
        if let Some(wildcards) = latest_wildcards(&player_log::events(&text)) {
            return Ok(Some(wildcards));
        }
    }
    Ok(None)
}

pub fn latest_wildcards(events: &[LogEvent]) -> Option<Wildcards> {
    events
        .iter()
        .rev()
        .filter(|event| event.direction == Direction::Response)
        .filter(|event| event.body.contains("\"WildCardRares\""))
        .find_map(|event| {
            serde_json::from_str::<WithInventory>(event.body)
                .ok()
                .and_then(|holder| holder.inventory)
        })
}
