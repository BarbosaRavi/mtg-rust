use crate::card_search::{CardFilter, in_color_order};
use crate::scryfall::{CardFace, ScryfallCard};
use crate::{config, storage};
use anyhow::{Context, Result};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct SearchResult {
    pub total: i64,
    pub cards: Vec<CardRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedRow {
    pub grp_id: u32,
    pub name: String,
    pub owned: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct CollectionSummary {
    pub unique: i64,
    pub total: i64,
    pub synced_at: Option<u64>,
}

const COLLECTION_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS collection (
        grp_id INTEGER PRIMARY KEY,
        name   TEXT NOT NULL,
        owned  INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS collection_name ON collection (name);
    CREATE TABLE IF NOT EXISTS collection_info (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

const OWNED_COLUMN: &str = "(SELECT COALESCE(SUM(owned), 0) FROM collection WHERE collection.name = cards.front_name) AS owned";

const SCHEMA_VERSION: i32 = 3;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS cards (
        arena_id       INTEGER UNIQUE,
        name           TEXT NOT NULL,
        front_name     TEXT NOT NULL,
        mana_cost      TEXT,
        cmc            REAL NOT NULL,
        colors         TEXT NOT NULL,
        color_identity TEXT NOT NULL,
        type_line      TEXT,
        oracle_text    TEXT,
        rarity         TEXT NOT NULL,
        set_code       TEXT NOT NULL,
        legalities     TEXT NOT NULL,
        image_normal   TEXT
    );
    CREATE INDEX IF NOT EXISTS cards_front_name ON cards (front_name);
    CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

const COLUMNS: &str = "arena_id, name, mana_cost, cmc, colors, color_identity, type_line, \
                       oracle_text, rarity, set_code, legalities, image_normal";

#[derive(Debug, Clone)]
pub struct CardRecord {
    pub arena_id: Option<u32>,
    pub name: String,
    pub mana_cost: Option<String>,
    pub cmc: f64,
    pub colors: String,
    pub color_identity: String,
    pub type_line: Option<String>,
    pub oracle_text: Option<String>,
    pub rarity: String,
    pub set_code: String,
    pub legalities: String,
    pub image_normal: Option<String>,
    pub owned: u32,
}

impl CardRecord {
    pub fn legal_formats(&self) -> Vec<String> {
        let legalities: BTreeMap<String, String> =
            serde_json::from_str(&self.legalities).unwrap_or_default();

        legalities
            .into_iter()
            .filter(|(_, status)| status == "legal")
            .map(|(format, _)| format)
            .collect()
    }

    pub fn is_basic_land(&self) -> bool {
        self.type_line
            .as_deref()
            .is_some_and(|type_line| type_line.starts_with("Basic Land"))
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            arena_id: row.get("arena_id")?,
            name: row.get("name")?,
            mana_cost: row.get("mana_cost")?,
            cmc: row.get("cmc")?,
            colors: row.get("colors")?,
            color_identity: row.get("color_identity")?,
            type_line: row.get("type_line")?,
            oracle_text: row.get("oracle_text")?,
            rarity: row.get("rarity")?,
            set_code: row.get("set_code")?,
            legalities: row.get("legalities")?,
            image_normal: row.get("image_normal")?,
            owned: row.get("owned")?,
        })
    }
}

pub fn front_name(name: &str) -> &str {
    name.split(" // ").next().unwrap_or(name)
}

impl From<ScryfallCard> for CardRecord {
    fn from(card: ScryfallCard) -> Self {
        let first_face = card.card_faces.first();

        Self {
            arena_id: card.arena_id,
            mana_cost: card
                .mana_cost
                .clone()
                .or_else(|| join_faces(&card.card_faces, |face| face.mana_cost.as_deref())),
            oracle_text: card
                .oracle_text
                .clone()
                .or_else(|| join_faces(&card.card_faces, |face| face.oracle_text.as_deref())),
            colors: card
                .colors
                .as_ref()
                .or_else(|| first_face.and_then(|face| face.colors.as_ref()))
                .map(|colors| in_color_order(&colors.concat()))
                .unwrap_or_default(),
            image_normal: card
                .image_uris
                .as_ref()
                .or_else(|| first_face.and_then(|face| face.image_uris.as_ref()))
                .and_then(|images| images.normal.clone()),
            color_identity: in_color_order(&card.color_identity.concat()),
            legalities: serde_json::to_string(&card.legalities).unwrap_or_default(),
            name: card.name,
            cmc: card.cmc,
            type_line: card.type_line,
            rarity: card.rarity,
            set_code: card.set.to_uppercase(),
            owned: 0,
        }
    }
}

pub struct CardStore {
    connection: Connection,
}

impl CardStore {
    pub fn open() -> Result<Self> {
        let file = store_file()?;
        storage::ensure_parent_dir(&file)?;

        let connection = Connection::open(&file)
            .with_context(|| format!("não consegui abrir {}", file.display()))?;
        connection.busy_timeout(Duration::from_secs(5))?;

        let version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            connection.execute_batch(&format!(
                "DROP TABLE IF EXISTS cards;
                 DROP TABLE IF EXISTS meta;
                 {SCHEMA}
                 PRAGMA user_version = {SCHEMA_VERSION};"
            ))?;
        }
        connection.execute_batch(COLLECTION_SCHEMA)?;

        Ok(Self { connection })
    }

    pub fn open_if_exists() -> Result<Option<Self>> {
        if store_file()?.exists() {
            Self::open().map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn version(&self) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key = 'scryfall_version'",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self
            .connection
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?)
    }

    pub fn replace_all(&mut self, cards: &[CardRecord], version: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM cards", [])?;

        {
            let mut insert = transaction.prepare(&format!(
                "INSERT OR REPLACE INTO cards ({COLUMNS}, front_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
            ))?;

            for card in cards {
                insert.execute(params![
                    card.arena_id,
                    card.name,
                    card.mana_cost,
                    card.cmc,
                    card.colors,
                    card.color_identity,
                    card.type_line,
                    card.oracle_text,
                    card.rarity,
                    card.set_code,
                    card.legalities,
                    card.image_normal,
                    front_name(&card.name),
                ])?;
            }
        }

        transaction.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('scryfall_version', ?1)",
            [version],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn by_arena_id(&self, arena_id: u32) -> Result<Option<CardRecord>> {
        let sql = format!("SELECT {COLUMNS}, {OWNED_COLUMN} FROM cards WHERE arena_id = ?1");
        Ok(self
            .connection
            .prepare_cached(&sql)?
            .query_row([arena_id], CardRecord::from_row)
            .optional()?)
    }

    pub fn search(&self, filter: &CardFilter) -> Result<SearchResult> {
        let (clause, mut values) = filter.where_clause()?;
        let shown = shown_printings(&clause);
        let owned_only = if filter.owned {
            "WHERE shown.owned > 0"
        } else {
            ""
        };

        let total = self.connection.query_row(
            &format!("{shown} SELECT COUNT(*) FROM shown {owned_only}"),
            params_from_iter(&values),
            |row| row.get(0),
        )?;

        values.push(Value::Integer(filter.limit as i64));
        let mut statement = self.connection.prepare(&format!(
            "{shown} SELECT {COLUMNS}, shown.owned AS owned FROM shown
             JOIN cards ON cards.rowid = shown.printing {owned_only}
             ORDER BY name, arena_id LIMIT ?"
        ))?;
        let cards = statement
            .query_map(params_from_iter(&values), CardRecord::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SearchResult { total, cards })
    }

    pub fn by_name(&self, front_name: &str) -> Result<Option<CardRecord>> {
        let sql = format!(
            "SELECT {COLUMNS}, {OWNED_COLUMN} FROM cards WHERE front_name = ?1
             ORDER BY arena_id IS NULL LIMIT 1"
        );
        Ok(self
            .connection
            .prepare_cached(&sql)?
            .query_row([front_name], CardRecord::from_row)
            .optional()?)
    }

    pub fn search_names(&self, filter: &CardFilter) -> Result<SearchResult> {
        let (clause, mut values) = filter.where_clause()?;
        let owned_only = if filter.owned { "HAVING owned > 0" } else { "" };
        let grouped = format!(
            "SELECT {COLUMNS}, {OWNED_COLUMN}, MAX(COALESCE(arena_id, 0)) FROM cards {clause}
             GROUP BY name {owned_only}"
        );

        let total = self.connection.query_row(
            &format!("SELECT COUNT(*) FROM ({grouped})"),
            params_from_iter(&values),
            |row| row.get(0),
        )?;

        values.push(Value::Integer(filter.limit as i64));
        let mut statement = self
            .connection
            .prepare(&format!("{grouped} ORDER BY name LIMIT ?"))?;
        let cards = statement
            .query_map(params_from_iter(&values), CardRecord::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SearchResult { total, cards })
    }

    pub fn by_name_ignoring_case(&self, front_name: &str) -> Result<Option<CardRecord>> {
        let sql = format!(
            "SELECT {COLUMNS}, {OWNED_COLUMN} FROM cards WHERE front_name = ?1 COLLATE NOCASE
             ORDER BY arena_id IS NULL, arena_id DESC LIMIT 1"
        );
        Ok(self
            .connection
            .prepare_cached(&sql)?
            .query_row([front_name], CardRecord::from_row)
            .optional()?)
    }

    pub fn cheapest_rarity(&self, front_name: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .prepare_cached(
                "SELECT rarity FROM cards WHERE front_name = ?1
                 ORDER BY CASE rarity WHEN 'common' THEN 1 WHEN 'uncommon' THEN 2
                                      WHEN 'rare' THEN 3 WHEN 'mythic' THEN 4 ELSE 5 END
                 LIMIT 1",
            )?
            .query_row([front_name], |row| row.get(0))
            .optional()?)
    }

    pub fn replace_collection(&mut self, rows: &[OwnedRow]) -> Result<bool> {
        let mut sorted = rows.to_vec();
        sorted.sort_by_key(|row| row.grp_id);
        let changed = self.collection_rows()? != sorted;

        let transaction = self.connection.transaction()?;
        if changed {
            transaction.execute("DELETE FROM collection", [])?;
            let mut insert = transaction.prepare(
                "INSERT OR REPLACE INTO collection (grp_id, name, owned) VALUES (?1, ?2, ?3)",
            )?;
            for row in &sorted {
                insert.execute(params![row.grp_id, row.name, row.owned])?;
            }
        }
        transaction.execute(
            "INSERT OR REPLACE INTO collection_info (key, value) VALUES ('synced_at', ?1)",
            [unix_now().to_string()],
        )?;
        transaction.commit()?;

        Ok(changed)
    }

    pub fn collection_summary(&self) -> Result<Option<CollectionSummary>> {
        let synced_at: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM collection_info WHERE key = 'synced_at'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(synced_at) = synced_at else {
            return Ok(None);
        };

        let (unique, total) = self.connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(owned), 0) FROM collection",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        Ok(Some(CollectionSummary {
            unique,
            total,
            synced_at: synced_at.parse().ok(),
        }))
    }

    pub fn owned_by_name(&self) -> Result<HashMap<String, u32>> {
        let mut statement = self
            .connection
            .prepare("SELECT name, SUM(owned) FROM collection GROUP BY name")?;
        let owned = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(owned)
    }

    fn collection_rows(&self) -> Result<Vec<OwnedRow>> {
        let mut statement = self
            .connection
            .prepare("SELECT grp_id, name, owned FROM collection ORDER BY grp_id")?;
        let rows = statement
            .query_map([], |row| {
                Ok(OwnedRow {
                    grp_id: row.get(0)?,
                    name: row.get(1)?,
                    owned: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

// Uma linha por edição que você tem, com a quantidade só daquela edição.
// Cópias que a Scryfall não conhece pelo grp_id contam na edição mais nova da carta.
// Cartas que você não tem aparecem uma vez só, na edição mais nova.
fn shown_printings(clause: &str) -> String {
    format!(
        "WITH matching AS (
             SELECT rowid AS printing, front_name, arena_id,
                    ROW_NUMBER() OVER (PARTITION BY front_name ORDER BY arena_id DESC) AS recency
             FROM cards {clause}
         ),
         owned_printings AS (
             SELECT COALESCE(exact.printing, newest.printing) AS printing,
                    SUM(collection.owned) AS owned
             FROM collection
             LEFT JOIN matching AS exact ON exact.arena_id = collection.grp_id
             LEFT JOIN matching AS newest
                    ON newest.front_name = collection.name AND newest.recency = 1
                   AND NOT EXISTS (SELECT 1 FROM cards WHERE cards.arena_id = collection.grp_id)
             WHERE COALESCE(exact.printing, newest.printing) IS NOT NULL
             GROUP BY 1
         ),
         shown AS (
             SELECT printing, owned FROM owned_printings
             UNION ALL
             SELECT printing, 0 FROM matching
             WHERE recency = 1 AND front_name NOT IN (
                 SELECT matching.front_name FROM owned_printings JOIN matching USING (printing))
         )"
    )
}

pub fn store_file() -> Result<PathBuf> {
    Ok(config::project_dirs()?.data_dir().join("cards.sqlite"))
}

fn join_faces(faces: &[CardFace], field: impl Fn(&CardFace) -> Option<&str>) -> Option<String> {
    let parts: Vec<&str> = faces
        .iter()
        .filter_map(field)
        .filter(|part| !part.is_empty())
        .collect();

    (!parts.is_empty()).then(|| parts.join(" // "))
}
