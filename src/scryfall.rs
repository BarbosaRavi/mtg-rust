use crate::{config, storage};
use anyhow::{Context, Result};
use flate2::read::MultiGzDecoder;
use reqwest::blocking::Client;
use reqwest::header::ACCEPT;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

const BULK_INFO_URL: &str = "https://api.scryfall.com/bulk-data/default-cards";
const USER_AGENT: &str = concat!("rust-mtg/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Deserialize)]
pub struct BulkFile {
    pub updated_at: String,
    pub jsonl_download_uri: String,
    #[serde(default)]
    pub compressed_size: u64,
}

#[derive(Deserialize)]
pub struct ScryfallCard {
    pub arena_id: Option<u32>,
    pub name: String,
    pub mana_cost: Option<String>,
    #[serde(default)]
    pub cmc: f64,
    pub colors: Option<Vec<String>>,
    #[serde(default)]
    pub color_identity: Vec<String>,
    pub type_line: Option<String>,
    pub oracle_text: Option<String>,
    pub rarity: String,
    pub set: String,
    #[serde(default)]
    pub legalities: BTreeMap<String, String>,
    pub image_uris: Option<ImageUris>,
    #[serde(default)]
    pub card_faces: Vec<CardFace>,
    #[serde(default)]
    pub games: Vec<String>,
}

impl ScryfallCard {
    fn is_on_arena(&self) -> bool {
        self.arena_id.is_some() || self.games.iter().any(|game| game == "arena")
    }
}

#[derive(Deserialize)]
pub struct CardFace {
    pub mana_cost: Option<String>,
    pub oracle_text: Option<String>,
    pub colors: Option<Vec<String>>,
    pub image_uris: Option<ImageUris>,
}

#[derive(Clone, Deserialize)]
pub struct ImageUris {
    pub normal: Option<String>,
}

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
        .build()?)
}

pub fn bulk_file(client: &Client) -> Result<BulkFile> {
    client
        .get(BULK_INFO_URL)
        .header(ACCEPT, "application/json")
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json())
        .context("não consegui consultar os dados em lote da Scryfall")
}

pub fn download_path() -> Result<PathBuf> {
    Ok(config::project_dirs()?
        .cache_dir()
        .join("default-cards.jsonl.gz"))
}

pub fn download(client: &Client, url: &str, destination: &Path) -> Result<()> {
    storage::ensure_parent_dir(destination)?;
    let partial = destination.with_extension("part");

    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .with_context(|| format!("não consegui baixar {url}"))?;
    let mut file = File::create(&partial)
        .with_context(|| format!("não consegui criar {}", partial.display()))?;
    response
        .copy_to(&mut file)
        .context("o download da Scryfall foi interrompido")?;

    fs::rename(&partial, destination)?;
    Ok(())
}

pub fn read_arena_cards(path: &Path) -> Result<Vec<ScryfallCard>> {
    let file =
        File::open(path).with_context(|| format!("não consegui abrir {}", path.display()))?;
    let reader = BufReader::new(MultiGzDecoder::new(BufReader::new(file)));
    let mut cards = Vec::new();

    for line in reader.lines() {
        let line = line.context("arquivo da Scryfall corrompido")?;
        if !line.contains("\"arena") {
            continue;
        }

        let card: ScryfallCard =
            serde_json::from_str(&line).context("carta inválida no arquivo da Scryfall")?;
        if card.is_on_arena() {
            cards.push(card);
        }
    }

    Ok(cards)
}
