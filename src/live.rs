use crate::decks::{self, Change, Decks};
use crate::inventory::{self, Wildcards};
use crate::log_follower::LogFollower;
use crate::player_log;
use anyhow::Result;
use std::path::Path;

pub struct LiveDecks {
    decks: Decks,
    wildcards: Option<Wildcards>,
    follower: LogFollower,
}

pub struct Poll {
    pub changes: Vec<Change>,
    pub warnings: Vec<anyhow::Error>,
}

impl LiveDecks {
    pub fn start(log_path: &Path) -> Result<(Self, Vec<anyhow::Error>)> {
        let mut live = Self {
            decks: decks::load_with_previous_log(log_path)?,
            wildcards: None,
            follower: LogFollower::new(log_path),
        };

        let text = live.follower.read_new_lines()?;
        let warnings = live.apply(&text);
        decks::save(&live.decks)?;

        Ok((live, warnings))
    }

    pub fn decks(&self) -> &Decks {
        &self.decks
    }

    pub fn wildcards(&self) -> Option<Wildcards> {
        self.wildcards
    }

    pub fn poll(&mut self) -> Result<Poll> {
        let text = self.follower.read_new_lines()?;
        if text.is_empty() {
            return Ok(Poll {
                changes: Vec::new(),
                warnings: Vec::new(),
            });
        }

        let before = self.decks.clone();
        let warnings = self.apply(&text);

        let changes = decks::changes(&before, &self.decks);
        if !changes.is_empty() {
            decks::save(&self.decks)?;
        }

        Ok(Poll { changes, warnings })
    }

    fn apply(&mut self, text: &str) -> Vec<anyhow::Error> {
        let events = player_log::events(text);

        if let Some(wildcards) = inventory::latest_wildcards(&events) {
            self.wildcards = Some(wildcards);
        }

        events
            .iter()
            .filter_map(|event| decks::apply_event(&mut self.decks, event).err())
            .collect()
    }
}
