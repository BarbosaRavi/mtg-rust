use crate::card_db;
use crate::decks::LOC_PREFIX;
use crate::language::Language;
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

pub struct ArenaDb {
    cards: Connection,
    localization: Option<Connection>,
    language: Language,
}

impl ArenaDb {
    pub fn open(card_db_path: &Path, language: Language) -> Result<Self> {
        let localization = card_db_path
            .parent()
            .and_then(card_db::localization_db_in)
            .map(|path| open_read_only(&path))
            .transpose()?;

        Ok(Self {
            cards: open_read_only(card_db_path)?,
            localization,
            language,
        })
    }

    pub fn card_name(&self, grp_id: u32) -> Result<Option<String>> {
        match self.card_name_in(grp_id, self.language)? {
            None if self.language != Language::EnUs => self.english_card_name(grp_id),
            name => Ok(name),
        }
    }

    pub fn english_card_name(&self, grp_id: u32) -> Result<Option<String>> {
        self.card_name_in(grp_id, Language::EnUs)
    }

    pub fn deck_name(&self, raw_name: &str) -> Result<Option<String>> {
        let (Some(key), Some(localization)) =
            (raw_name.strip_prefix(LOC_PREFIX), &self.localization)
        else {
            return Ok(None);
        };

        let sql = format!(
            "SELECT {} FROM Loc WHERE Key = ?1",
            self.language.arena_code()
        );
        let name: Option<Option<String>> = localization
            .prepare_cached(&sql)?
            .query_row([key], |row| row.get(0))
            .optional()?;
        Ok(name.flatten())
    }

    fn card_name_in(&self, grp_id: u32, language: Language) -> Result<Option<String>> {
        let sql = format!(
            "SELECT l.Loc FROM Cards c
             JOIN Localizations_{} l ON l.LocId = c.TitleId
             WHERE c.GrpId = ?1
             ORDER BY l.Formatted
             LIMIT 1",
            language.arena_code()
        );

        let name: Option<Option<String>> = self
            .cards
            .prepare_cached(&sql)?
            .query_row([grp_id], |row| row.get(0))
            .optional()?;
        Ok(name.flatten().map(|name| strip_tags(&name)))
    }
}

fn open_read_only(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("não consegui abrir {}", path.display()))
}

fn strip_tags(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut inside_tag = false;

    for character in text.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => plain.push(character),
            _ => {}
        }
    }

    plain
}
